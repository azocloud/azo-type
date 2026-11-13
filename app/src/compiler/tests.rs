//! Unit tests for the Typst compiler, package fetching, and PDF export pipeline.

use super::*;

#[test]
fn test_merman_package_import_and_pdf_standards_export() {
    let temp_dir =
        std::env::temp_dir().join(format!("azo_type_merman_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let test_typ_content = r#"// --- Pakete für Mermaid-Diagramme ---
#import "@preview/merman:0.5.0": mermaid

// --- 1. Natürliche Sprache ganz am Anfang setzen (Guide: Natural Language) ---
#set text(
  font: "Libertinus Serif",
  lang: "de",
  region: "DE",
  size: 11pt,
)

= Architektur-Übersicht

#mermaid("graph LR\n  A[EditorWorld] --> B[SystemPackages]\n  B --> C[Live Preview]", alt: "Architektur-Diagramm")
"#;
    std::fs::write(temp_dir.join("test.typ"), test_typ_content).unwrap();

    let project = DiskProject::open_folder(&temp_dir, false).unwrap();
    let resources = Arc::new(SharedWorldResources::new());

    let snapshot = compile_project(Arc::clone(&resources), &project, 2.0);
    let errors: Vec<_> = snapshot.diagnostics.iter().filter(|d| d.is_error).collect();
    assert!(
        errors.is_empty(),
        "Expected @preview/merman:0.5.0 compilation to succeed, got errors: {:?}",
        errors
    );
    assert!(
        !snapshot.pages.is_empty(),
        "Expected at least 1 rendered preview page"
    );
    assert!(
        snapshot.pages[0]
            .char_boxes
            .iter()
            .any(|b| b.file == std::path::Path::new("test.typ")),
        "Expected PreviewCharBox entries from test.typ"
    );
    assert!(
        !snapshot.pages[0]
            .char_boxes
            .iter()
            .any(|b| b.file.to_string_lossy().starts_with('@')),
        "Expected @preview package spans to be ignored in char_boxes"
    );

    // Test exporting with PDF 1.7 + PDF/A-3a (just like `typst compile --pdf-standard a-3a`)
    let cfg_a3a = PdfExportConfig {
        version: PdfVersionChoice::V1_7,
        archival_standard: PdfArchivalStandard::A3a,
        pdf_ua1: false,
        tagged: true,
    };
    let pdf_a3a = export_pdf_bytes_with_config(Arc::clone(&resources), &project, &cfg_a3a)
        .expect("PDF/A-3a export should succeed");
    assert!(pdf_a3a.starts_with(b"%PDF-1.7"));

    // Test exporting with PDF 1.7 + PDF/A-2a
    let cfg_a2a = PdfExportConfig {
        version: PdfVersionChoice::V1_7,
        archival_standard: PdfArchivalStandard::A2a,
        pdf_ua1: false,
        tagged: true,
    };
    let pdf_a2a = export_pdf_bytes_with_config(Arc::clone(&resources), &project, &cfg_a2a)
        .expect("PDF/A-2a export should succeed");
    assert!(pdf_a2a.starts_with(b"%PDF-1.7"));

    // Test exporting with PDF 1.4, 1.5, 1.6
    for ver in [
        PdfVersionChoice::V1_4,
        PdfVersionChoice::V1_5,
        PdfVersionChoice::V1_6,
    ] {
        let cfg = PdfExportConfig {
            version: ver,
            archival_standard: PdfArchivalStandard::None,
            pdf_ua1: false,
            tagged: true,
        };
        let bytes = export_pdf_bytes_with_config(Arc::clone(&resources), &project, &cfg)
            .expect("PDF version export should succeed");
        let expected_header = format!("%PDF-{}", ver.short_label());
        assert!(bytes.starts_with(expected_header.as_bytes()));
    }

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_build_byte_to_char_table_with_multibyte_and_astral_chars() {
    let empty = build_byte_to_char_table("");
    assert_eq!(empty, vec![0]);

    // 'a' (1B, char 0), 'ä' (2B, char 1), '🚀' (4B, char 2), 'z' (1B, char 3)
    let text = "aä🚀z";
    let table = build_byte_to_char_table(text);
    assert_eq!(table.len(), text.len() + 1);
    assert_eq!(table[0], 0); // 'a'
    assert_eq!(table[1], 1); // 'ä' byte 0
    assert_eq!(table[2], 1); // 'ä' byte 1
    assert_eq!(table[3], 2); // '🚀' byte 0
    assert_eq!(table[4], 2); // '🚀' byte 1
    assert_eq!(table[5], 2); // '🚀' byte 2
    assert_eq!(table[6], 2); // '🚀' byte 3
    assert_eq!(table[7], 3); // 'z'
    assert_eq!(table[8], 4); // EOF
}

#[test]
fn test_compile_project_extracts_multifile_and_transformed_char_boxes() {
    let temp_dir =
        std::env::temp_dir().join(format!("azo_type_charbox_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(temp_dir.join("chapters")).unwrap();

    let main_src = "= Main 🚀 Title\n\nIntroWord\n\n#rotate(90deg)[RotatedText]\n\n#include \"chapters/ch1.typ\"\n";
    let ch1_src = "== Chapter One\n\nAlphaBetaGamma\n";

    std::fs::write(temp_dir.join("main.typ"), main_src).unwrap();
    std::fs::write(temp_dir.join("chapters/ch1.typ"), ch1_src).unwrap();

    let project = DiskProject::open_folder(&temp_dir, false).unwrap();
    let resources = Arc::new(SharedWorldResources::new());
    let snapshot = compile_project(resources, &project, 1.5);

    assert!(
        snapshot.diagnostics.iter().all(|d| !d.is_error),
        "Unexpected compile errors: {:?}",
        snapshot.diagnostics
    );
    assert_eq!(snapshot.pages.len(), 1);

    let page = &snapshot.pages[0];
    assert!(
        !page.char_boxes.is_empty(),
        "Expected extracted PreviewCharBox entries"
    );

    // Every box must have valid ordered coordinates within reasonable page range and no @preview path
    for b in page.char_boxes.iter() {
        assert!(b.x_min_pt <= b.x_max_pt, "Invalid X bounds: {:?}", b);
        assert!(b.y_min_pt <= b.y_max_pt, "Invalid Y bounds: {:?}", b);
        assert!(!b.file.to_string_lossy().starts_with('@'));
    }

    // Verify main.typ word 'IntroWord' maps to exact char index after the astral emoji '🚀'
    let main_chars: Vec<char> = main_src.chars().collect();
    let intro_expected_idx = main_src[..main_src.find("IntroWord").unwrap()]
        .chars()
        .count();
    let intro_box = page
        .char_boxes
        .iter()
        .find(|b| b.file == std::path::Path::new("main.typ") && b.char_idx == intro_expected_idx)
        .expect("Expected PreviewCharBox for start of 'IntroWord' in main.typ");
    assert_eq!(main_chars[intro_box.char_idx], 'I');
    assert_eq!(intro_box.text, "I");

    // Verify rotated text is present and maps to 'RotatedText'
    let rot_expected_idx = main_src[..main_src.find("RotatedText").unwrap()]
        .chars()
        .count();
    let rot_box = page
        .char_boxes
        .iter()
        .find(|b| b.file == std::path::Path::new("main.typ") && b.char_idx == rot_expected_idx)
        .expect("Expected PreviewCharBox for rotated text");
    assert_eq!(main_chars[rot_box.char_idx], 'R');

    // Verify included file chapters/ch1.typ has normalized path and exact char index
    let ch1_chars: Vec<char> = ch1_src.chars().collect();
    let alpha_expected_idx = ch1_src[..ch1_src.find("AlphaBetaGamma").unwrap()]
        .chars()
        .count();
    let alpha_box = page
        .char_boxes
        .iter()
        .find(|b| {
            b.file == std::path::Path::new("chapters/ch1.typ") && b.char_idx == alpha_expected_idx
        })
        .expect("Expected PreviewCharBox for 'AlphaBetaGamma' in chapters/ch1.typ");
    assert_eq!(ch1_chars[alpha_box.char_idx], 'A');
    assert_eq!(alpha_box.text, "A");

    let _ = std::fs::remove_dir_all(&temp_dir);
}
