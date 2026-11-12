//! Unit tests for the Typst compiler, package fetching, and PDF export pipeline.

use super::*;

#[test]
fn test_merman_package_import_and_pdf_standards_export() {
    let temp_dir =
        std::env::temp_dir().join(format!("typst_ide_merman_test_{}", uuid::Uuid::new_v4()));
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
