//! Unit tests for the external engine worker bridge (`cli` parsers, staging, and JSON-stdio protocol).

use std::fs;
use std::path::{Path, PathBuf};

use super::cli_parse::{parse_cli_diagnostics, parse_cli_outline_json, StagedWorkspace};
#[cfg(unix)]
use super::ExternalWorkerEngine;
#[cfg(unix)]
use crate::core::pdf::PdfExportConfig;
#[cfg(unix)]
use crate::engine::types::TypstEngine;
use crate::project::DiskProject;

#[test]
fn test_parse_cli_diagnostics_structured_and_ansi() {
    let root = Path::new("/tmp/myproj");
    let default_file = PathBuf::from("main.typ");
    let stderr = "\x1b[31merror:\x1b[0m unknown variable: foo\n  ┌─ /tmp/myproj/chapters/ch1.typ:14:5\n  = hint: check spelling\n  help: declare `let foo = 1`\n\nwarning: unused import\n  ┌─ /tmp/myproj/main.typ:2:1\n";

    let diags = parse_cli_diagnostics(stderr, root, &default_file, true);
    assert_eq!(diags.len(), 2);

    assert!(diags[0].is_error);
    assert_eq!(diags[0].file, PathBuf::from("chapters/ch1.typ"));
    assert_eq!(diags[0].line, Some(14));
    assert_eq!(diags[0].column, Some(5));
    assert_eq!(diags[0].message, "unknown variable: foo");
    assert_eq!(diags[0].hints, vec!["declare `let foo = 1`".to_string()]);

    assert!(!diags[1].is_error);
    assert_eq!(diags[1].file, PathBuf::from("main.typ"));
    assert_eq!(diags[1].line, Some(2));
    assert_eq!(diags[1].column, Some(1));
    assert_eq!(diags[1].message, "unused import");
}

#[test]
fn test_parse_cli_diagnostics_unstructured_fallback() {
    let diags = parse_cli_diagnostics(
        "fatal binary error",
        Path::new("/tmp"),
        Path::new("main.typ"),
        true,
    );
    assert_eq!(diags.len(), 1);
    assert!(diags[0].is_error);
    assert_eq!(diags[0].message, "fatal binary error");
}

#[test]
fn test_parse_cli_outline_json_handles_levels_and_nested_children() {
    let raw = r#"[
        {"func": "heading", "level": 1, "outlined": true, "body": {"func": "text", "text": "Intro"}},
        {"func": "heading", "level": 2, "outlined": false, "body": {"func": "text", "text": "Hidden"}},
        {"func": "heading", "depth": 2, "body": {"func": "sequence", "children": [
            {"func": "text", "text": "Deep"},
            {"func": "space"},
            {"func": "text", "text": "Dive"}
        ]}}
    ]"#;
    let items = parse_cli_outline_json(raw, Path::new("main.typ")).expect("should parse JSON");
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].level, 1);
    assert_eq!(items[0].title, "Intro");
    assert_eq!(items[1].level, 2);
    assert_eq!(items[1].title, "Deep Dive");
}

#[test]
fn test_staged_workspace_overlays_dirty_buffers_without_touching_disk() {
    let proj_dir = std::env::temp_dir().join(format!("azo-stage-src-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&proj_dir).expect("create test project dir");
    let main_path = proj_dir.join("main.typ");
    fs::write(&main_path, "= Saved on disk").expect("write disk main");

    let mut project = DiskProject::open_folder(&proj_dir, false).expect("open folder");
    if let Some(buf) = project.buffers.get_mut(&PathBuf::from("main.typ")) {
        buf.text = Some("= Unsaved in memory".to_string());
        buf.dirty = true;
    }

    let staged_root = {
        let stage = StagedWorkspace::prepare(&project).expect("stage workspace");
        assert_ne!(stage.root_dir, project.root_dir);
        let staged_text = fs::read_to_string(&stage.entry_abs).expect("read staged file");
        assert_eq!(staged_text, "= Unsaved in memory");
        assert_eq!(
            fs::read_to_string(&main_path).expect("read original disk file"),
            "= Saved on disk"
        );
        stage.root_dir.clone()
    };

    assert!(
        !staged_root.exists(),
        "staging temp dir must clean up on Drop"
    );
    let _ = fs::remove_dir_all(&proj_dir);
}

#[cfg(unix)]
#[test]
fn test_json_stdio_worker_compile_and_pdf_roundtrip() {
    use base64::engine::general_purpose::STANDARD as BASE64;
    use base64::Engine as _;
    use std::os::unix::fs::PermissionsExt;

    let work_dir = std::env::temp_dir().join(format!("azo-mock-worker-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&work_dir).expect("create mock worker dir");
    let script_path = work_dir.join("mock_worker.sh");

    let rgba_b64 = BASE64.encode([255u8, 0, 0, 255, 0, 255, 0, 255]);
    let pdf_b64 = BASE64.encode(b"%PDF-1.7\n");
    let compile_json = format!(
        r#"{{"compiled_ok":true,"pages":[{{"page_number":1,"width_pt":200.0,"height_pt":100.0,"rgba_width":2,"rgba_height":1,"rgba_base64":"{rgba_b64}","char_boxes":[{{"x_min_pt":10.0,"y_min_pt":12.0,"x_max_pt":18.0,"y_max_pt":22.0,"file":"main.typ","char_idx":2,"text":"H"}}]}}],"outline":[{{"level":1,"numbers":"1","title":"Mock","file":"main.typ","line":1,"char_offset":0}}],"diagnostics":[]}}"#
    );
    let export_json = format!(r#"{{"pdf_base64":"{pdf_b64}","diagnostics":[]}}"#);

    let script = format!(
        "#!/bin/sh\ncat >/dev/null\nif [ \"$2\" = \"compile\" ]; then\n  printf '%s\\n' '{compile_json}'\nelif [ \"$2\" = \"export-pdf\" ]; then\n  printf '%s\\n' '{export_json}'\nelse\n  exit 1\nfi\n"
    );
    fs::write(&script_path, script).expect("write mock worker script");
    fs::set_permissions(&script_path, fs::Permissions::from_mode(0o755))
        .expect("chmod +x mock worker script");

    let engine =
        ExternalWorkerEngine::with_json_stdio("0.14.2".to_string(), script_path.clone(), None);
    let project = DiskProject::blank();

    let snapshot = engine.compile_project(&project, 1.5);
    assert!(snapshot.compiled_ok);
    assert_eq!(snapshot.pages.len(), 1);
    assert_eq!(snapshot.pages[0].char_boxes.len(), 1);
    assert_eq!(snapshot.pages[0].char_boxes[0].text, "H");
    assert_eq!(snapshot.outline.len(), 1);
    assert_eq!(snapshot.outline[0].title, "Mock");

    let pdf_bytes = engine
        .export_pdf_bytes(&project, &PdfExportConfig::default())
        .expect("export pdf via mock json worker");
    assert_eq!(pdf_bytes, b"%PDF-1.7\n");

    let _ = fs::remove_dir_all(&work_dir);
}

#[cfg(unix)]
#[test]
fn test_cli_worker_compiles_simple_typst_and_exports_pdf_end_to_end() {
    use std::os::unix::fs::PermissionsExt;

    let work_dir = std::env::temp_dir().join(format!("azo-mock-cli-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&work_dir).expect("create mock cli dir");
    let script_path = work_dir.join("typst");
    let sample_png = work_dir.join("sample.png");

    let img = image::RgbaImage::from_pixel(4, 2, image::Rgba([255, 255, 255, 255]));
    image::DynamicImage::ImageRgba8(img)
        .save_with_format(&sample_png, image::ImageFormat::Png)
        .expect("write sample png");

    let script = format!(
        r#"#!/bin/sh
cmd="$1"
shift
if [ "$cmd" = "query" ]; then
  printf '[{{"func":"heading","level":1,"outlined":true,"body":{{"func":"text","text":"Legacy Heading"}}}}]\n'
  exit 0
fi
if [ "$cmd" = "compile" ]; then
  fmt=""
  out=""
  for a in "$@"; do
    prev="$last"
    last="$a"
    if [ "$prev" = "--format" ]; then
      fmt="$a"
    fi
  done
  out="$last"
  if [ "$fmt" = "png" ]; then
    target=$(printf '%s' "$out" | sed 's/{{0p}}/01/')
    cp "{}" "$target"
    exit 0
  elif [ "$fmt" = "pdf" ]; then
    printf '%%PDF-1.5\n%%mock\n' > "$out"
    exit 0
  fi
fi
echo "error: unexpected invocation" >&2
exit 1
"#,
        sample_png.display()
    );

    fs::write(&script_path, script).expect("write mock cli script");
    fs::set_permissions(&script_path, fs::Permissions::from_mode(0o755))
        .expect("chmod +x mock cli script");

    let engine = ExternalWorkerEngine::new("0.14.2".to_string(), script_path, None);
    let mut project = DiskProject::blank();
    if let Some(buf) = project.buffers.get_mut(&PathBuf::from("main.typ")) {
        buf.text = Some("= Legacy Heading\nSimple typst check.".to_string());
        buf.dirty = true;
    }

    let snap = engine.compile_project(&project, 2.0);
    assert!(snap.compiled_ok, "diags: {:?}", snap.diagnostics);
    assert_eq!(snap.pages.len(), 1);
    assert_eq!(snap.pages[0].image.size, [4, 2]);
    assert_eq!(snap.outline.len(), 1);
    assert_eq!(snap.outline[0].title, "Legacy Heading");

    let pdf = engine
        .export_pdf_bytes(&project, &PdfExportConfig::default())
        .expect("export pdf via mock cli");
    assert!(pdf.starts_with(b"%PDF-1.5"));

    let _ = fs::remove_dir_all(&work_dir);
}
