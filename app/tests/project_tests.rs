use std::fs;
use std::path::{Path, PathBuf};
use typst_ide::project::{DiskProject, FileKind, OpenBuffer};

#[test]
fn opens_existing_folder_and_collects_files() {
    let temp_dir = std::env::temp_dir().join(format!("typst_test_proj_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(temp_dir.join("sub")).unwrap();
    fs::write(temp_dir.join("main.typ"), "= Main").unwrap();
    fs::write(temp_dir.join("sub/extra.typ"), "== Extra").unwrap();
    fs::write(temp_dir.join("refs.bib"), "@article{key,}").unwrap();

    let project = DiskProject::open_folder(&temp_dir, false).expect("open folder should succeed");
    assert_eq!(project.main_file, PathBuf::from("main.typ"));
    assert_eq!(project.active_file, PathBuf::from("main.typ"));

    let typ_files = project.collect_typ_files();
    assert_eq!(typ_files.len(), 2);
    assert!(typ_files.contains(&PathBuf::from("main.typ")));
    assert!(typ_files.contains(&PathBuf::from("sub/extra.typ")));

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn fails_gracefully_when_opening_nonexistent_directory() {
    let fake_dir = Path::new("/nonexistent/directory/for/typst/testing/12345");
    let result = DiskProject::open_folder(fake_dir, false);
    assert!(result.is_err());
    let err_msg = result.err().unwrap();
    assert!(err_msg.contains("is not a directory"));
}

#[test]
fn identifies_file_kinds_correctly_from_extensions() {
    assert_eq!(FileKind::from_path(Path::new("doc.typ")), FileKind::Typst);
    assert_eq!(FileKind::from_path(Path::new("doc.TYP")), FileKind::Typst);
    assert_eq!(FileKind::from_path(Path::new("refs.bib")), FileKind::Bib);
    assert_eq!(FileKind::from_path(Path::new("config.yaml")), FileKind::Bib);
    assert_eq!(FileKind::from_path(Path::new("config.yml")), FileKind::Bib);
    assert_eq!(FileKind::from_path(Path::new("icon.svg")), FileKind::Svg);
    assert_eq!(FileKind::from_path(Path::new("photo.png")), FileKind::Image);
    assert_eq!(FileKind::from_path(Path::new("photo.jpg")), FileKind::Image);
    assert_eq!(FileKind::from_path(Path::new("notes.md")), FileKind::Text);
    assert_eq!(FileKind::from_path(Path::new("data.csv")), FileKind::Text);
    assert_eq!(FileKind::from_path(Path::new("data.json")), FileKind::Text);
    assert_eq!(FileKind::from_path(Path::new("app.toml")), FileKind::Text);
    assert_eq!(
        FileKind::from_path(Path::new("binary.bin")),
        FileKind::Binary
    );
    assert_eq!(FileKind::from_path(Path::new("no_ext")), FileKind::Binary);

    assert!(FileKind::Typst.is_editable_text());
    assert!(FileKind::Bib.is_editable_text());
    assert!(FileKind::Svg.is_editable_text());
    assert!(FileKind::Text.is_editable_text());
    assert!(!FileKind::Image.is_editable_text());
    assert!(!FileKind::Binary.is_editable_text());

    assert_eq!(FileKind::Typst.icon(), "[T]");
    assert_eq!(FileKind::Bib.icon(), "[B]");
    assert_eq!(FileKind::Svg.icon(), "[S]");
}

#[test]
fn creates_nested_file_on_disk_and_populates_typst_template() {
    let temp_dir = std::env::temp_dir().join(format!("typst_test_create_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&temp_dir).unwrap();

    let mut project = DiskProject::open_folder(&temp_dir, false).unwrap();
    let created_rel = project
        .create_file_on_disk("chapters/conclusion.typ")
        .expect("create file should succeed");

    assert_eq!(created_rel, PathBuf::from("chapters/conclusion.typ"));
    let abs_file = temp_dir.join("chapters/conclusion.typ");
    assert!(abs_file.is_file());

    let content = fs::read_to_string(&abs_file).unwrap();
    assert!(content.contains("== chapters/conclusion.typ"));
    assert_eq!(project.active_file, created_rel);
    assert!(project
        .open_tabs
        .contains(&PathBuf::from("chapters/conclusion.typ")));

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn prevents_empty_filename_creation_and_renaming() {
    let temp_dir = std::env::temp_dir().join(format!("typst_test_empty_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&temp_dir).unwrap();

    let mut project = DiskProject::open_folder(&temp_dir, false).unwrap();
    assert!(project.create_file_on_disk("   ").is_err());
    assert!(project.create_file_on_disk("///").is_err());

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn renames_file_on_disk_and_updates_open_tabs_and_buffers() {
    let temp_dir = std::env::temp_dir().join(format!("typst_test_rename_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&temp_dir).unwrap();
    fs::write(temp_dir.join("old.typ"), "= Old File\n").unwrap();

    let mut project = DiskProject::open_folder(&temp_dir, false).unwrap();
    project.open_file(Path::new("old.typ")).unwrap();
    assert_eq!(project.active_file, PathBuf::from("old.typ"));

    let new_rel = project
        .rename_file_on_disk(Path::new("old.typ"), "docs/new.typ")
        .expect("rename should succeed");

    assert_eq!(new_rel, PathBuf::from("docs/new.typ"));
    assert!(!temp_dir.join("old.typ").exists());
    assert!(temp_dir.join("docs/new.typ").is_file());
    assert_eq!(project.active_file, PathBuf::from("docs/new.typ"));
    assert!(project.open_tabs.contains(&PathBuf::from("docs/new.typ")));
    assert!(!project.open_tabs.contains(&PathBuf::from("old.typ")));

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn deletes_file_from_disk_and_cleans_up_buffers() {
    let temp_dir = std::env::temp_dir().join(format!("typst_test_del_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&temp_dir).unwrap();
    fs::write(temp_dir.join("main.typ"), "= Main").unwrap();
    fs::write(temp_dir.join("temp.typ"), "= Temp").unwrap();

    let mut project = DiskProject::open_folder(&temp_dir, false).unwrap();
    project.open_file(Path::new("temp.typ")).unwrap();
    assert_eq!(project.active_file, PathBuf::from("temp.typ"));

    project
        .delete_file_on_disk(Path::new("temp.typ"))
        .expect("delete should succeed");

    assert!(!temp_dir.join("temp.typ").exists());
    assert!(!project.buffers.contains_key(Path::new("temp.typ")));
    assert!(!project.open_tabs.contains(&PathBuf::from("temp.typ")));
    assert_eq!(project.active_file, PathBuf::from("main.typ"));

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn determines_effective_compile_entrypoint_for_standalone_and_imported_files() {
    let temp_dir = std::env::temp_dir().join(format!("typst_test_entry_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&temp_dir).unwrap();
    fs::write(
        temp_dir.join("main.typ"),
        "#include \"chapters/intro.typ\"\n",
    )
    .unwrap();
    fs::create_dir_all(temp_dir.join("chapters")).unwrap();
    fs::write(temp_dir.join("chapters/intro.typ"), "= Intro\n").unwrap();
    fs::write(temp_dir.join("standalone.typ"), "= Standalone\n").unwrap();

    let mut project = DiskProject::open_folder(&temp_dir, false).unwrap();
    assert_eq!(project.main_file, PathBuf::from("main.typ"));

    // Case 1: active file is main.typ -> entrypoint is main.typ
    project.active_file = PathBuf::from("main.typ");
    assert_eq!(
        project.effective_compile_entrypoint(),
        PathBuf::from("main.typ")
    );

    // Case 2: active file is chapters/intro.typ, which is referenced in main.typ -> entrypoint is still main.typ
    project.open_file(Path::new("chapters/intro.typ")).unwrap();
    assert_eq!(
        project.effective_compile_entrypoint(),
        PathBuf::from("main.typ")
    );

    // Case 3: active file is standalone.typ, NOT referenced in main.typ -> compiles standalone.typ directly
    project.open_file(Path::new("standalone.typ")).unwrap();
    assert_eq!(
        project.effective_compile_entrypoint(),
        PathBuf::from("standalone.typ")
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn saves_modified_buffer_to_disk_and_clears_dirty_flag() {
    let temp_dir = std::env::temp_dir().join(format!("typst_test_save_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&temp_dir).unwrap();
    fs::write(temp_dir.join("main.typ"), "Original Text\n").unwrap();

    let mut project = DiskProject::open_folder(&temp_dir, false).unwrap();
    {
        let buf = project.active_buffer_mut().unwrap();
        buf.text = Some("Edited Text\n".to_string());
    }
    project.mark_active_modified(false);
    assert!(project.active_buffer().unwrap().dirty);

    let saved_path = project.save_active_file().expect("save should succeed");
    assert_eq!(saved_path, temp_dir.join("main.typ"));
    assert!(!project.active_buffer().unwrap().dirty);
    assert_eq!(
        fs::read_to_string(temp_dir.join("main.typ")).unwrap(),
        "Edited Text\n"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn saves_all_dirty_buffers_in_batch() {
    let temp_dir =
        std::env::temp_dir().join(format!("typst_test_saveall_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&temp_dir).unwrap();
    fs::write(temp_dir.join("file1.typ"), "F1\n").unwrap();
    fs::write(temp_dir.join("file2.typ"), "F2\n").unwrap();

    let mut project = DiskProject::open_folder(&temp_dir, false).unwrap();
    project.open_file(Path::new("file1.typ")).unwrap();
    project.open_file(Path::new("file2.typ")).unwrap();

    project
        .buffers
        .get_mut(Path::new("file1.typ"))
        .unwrap()
        .text = Some("F1 modified\n".into());
    project
        .buffers
        .get_mut(Path::new("file1.typ"))
        .unwrap()
        .dirty = true;
    project
        .buffers
        .get_mut(Path::new("file2.typ"))
        .unwrap()
        .text = Some("F2 modified\n".into());
    project
        .buffers
        .get_mut(Path::new("file2.typ"))
        .unwrap()
        .dirty = true;

    let saved_count = project.save_all_files().unwrap();
    assert_eq!(saved_count, 2);
    assert!(!project.buffers.get(Path::new("file1.typ")).unwrap().dirty);
    assert!(!project.buffers.get(Path::new("file2.typ")).unwrap().dirty);

    assert_eq!(
        fs::read_to_string(temp_dir.join("file1.typ")).unwrap(),
        "F1 modified\n"
    );
    assert_eq!(
        fs::read_to_string(temp_dir.join("file2.typ")).unwrap(),
        "F2 modified\n"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn loads_buffer_and_counts_lines_accurately() {
    let temp_dir = std::env::temp_dir().join(format!("typst_test_lines_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&temp_dir).unwrap();
    fs::write(temp_dir.join("test.txt"), "Line 1\nLine 2\nLine 3\n").unwrap();

    let buf = OpenBuffer::load_from_disk(&temp_dir, Path::new("test.txt")).unwrap();
    assert_eq!(buf.line_count(), 3);
    assert_eq!(buf.kind, FileKind::Text);
    assert_eq!(buf.size_bytes, 21);

    let _ = fs::remove_dir_all(&temp_dir);
}
