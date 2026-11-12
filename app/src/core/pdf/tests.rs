//! Unit tests for PDF export version choices and archival standard validation.

use super::*;

#[test]
fn test_default_pdf_config() {
    let cfg = PdfExportConfig::default();
    assert_eq!(cfg.version, PdfVersionChoice::V1_7);
    assert_eq!(cfg.archival_standard, PdfArchivalStandard::None);
    assert!(!cfg.pdf_ua1);
    assert!(cfg.tagged);
    assert!(cfg.build_standards().is_ok());
    assert_eq!(cfg.summary_badge(), "PDF 1.7");
}

#[test]
fn test_pdf_a3a_with_v1_7() {
    let cfg = PdfExportConfig {
        version: PdfVersionChoice::V1_7,
        archival_standard: PdfArchivalStandard::A3a,
        pdf_ua1: false,
        tagged: true,
    };
    assert!(cfg.build_standards().is_ok());
    assert_eq!(cfg.summary_badge(), "PDF 1.7 + PDF/A-3a");
}

#[test]
fn test_incompatible_version_and_standard_fails() {
    let cfg = PdfExportConfig {
        version: PdfVersionChoice::V1_7,
        archival_standard: PdfArchivalStandard::A1b,
        pdf_ua1: false,
        tagged: true,
    };
    assert!(cfg.build_standards().is_err());
}
