use azo_type::compiler::{PdfArchivalStandard, PdfExportConfig, PdfVersionChoice};

#[test]
fn validates_default_pdf_configuration() {
    let cfg = PdfExportConfig::default();
    assert_eq!(cfg.version, PdfVersionChoice::V1_7);
    assert_eq!(cfg.archival_standard, PdfArchivalStandard::None);
    assert!(!cfg.pdf_ua1);
    assert!(cfg.tagged);
    assert!(cfg.build_standards().is_ok());
    assert_eq!(cfg.summary_badge(), "PDF 1.7");
}

#[test]
fn builds_valid_pdf_standards_for_matching_versions() {
    // PDF 1.7 with PDF/A-3a
    let cfg_a3a = PdfExportConfig {
        version: PdfVersionChoice::V1_7,
        archival_standard: PdfArchivalStandard::A3a,
        pdf_ua1: false,
        tagged: true,
    };
    assert!(cfg_a3a.build_standards().is_ok());
    assert_eq!(cfg_a3a.summary_badge(), "PDF 1.7 + PDF/A-3a");

    // PDF 1.7 with PDF/A-2b
    let cfg_a2b = PdfExportConfig {
        version: PdfVersionChoice::V1_7,
        archival_standard: PdfArchivalStandard::A2b,
        pdf_ua1: false,
        tagged: false,
    };
    assert!(cfg_a2b.build_standards().is_ok());

    // PDF 1.4 with PDF/A-1b
    let cfg_a1b = PdfExportConfig {
        version: PdfVersionChoice::V1_4,
        archival_standard: PdfArchivalStandard::A1b,
        pdf_ua1: false,
        tagged: false,
    };
    assert!(cfg_a1b.build_standards().is_ok());

    // PDF 2.0 with PDF/A-4
    let cfg_a4 = PdfExportConfig {
        version: PdfVersionChoice::V2_0,
        archival_standard: PdfArchivalStandard::A4,
        pdf_ua1: false,
        tagged: false,
    };
    assert!(cfg_a4.build_standards().is_ok());
}

#[test]
fn rejects_invalid_standard_and_version_combinations_with_descriptive_error() {
    // PDF 1.7 cannot be combined with PDF/A-1 (which mandates PDF 1.4)
    let bad_cfg = PdfExportConfig {
        version: PdfVersionChoice::V1_7,
        archival_standard: PdfArchivalStandard::A1b,
        pdf_ua1: false,
        tagged: false,
    };
    let res = bad_cfg.build_standards();
    assert!(res.is_err());
    let err_msg = res.err().unwrap();
    assert!(!err_msg.is_empty());
}

#[test]
fn provides_accurate_recommended_version_for_each_archival_standard() {
    assert_eq!(PdfArchivalStandard::None.recommended_version(), None);
    assert_eq!(
        PdfArchivalStandard::A1b.recommended_version(),
        Some(PdfVersionChoice::V1_4)
    );
    assert_eq!(
        PdfArchivalStandard::A1a.recommended_version(),
        Some(PdfVersionChoice::V1_4)
    );
    assert_eq!(
        PdfArchivalStandard::A2b.recommended_version(),
        Some(PdfVersionChoice::V1_7)
    );
    assert_eq!(
        PdfArchivalStandard::A2a.recommended_version(),
        Some(PdfVersionChoice::V1_7)
    );
    assert_eq!(
        PdfArchivalStandard::A3b.recommended_version(),
        Some(PdfVersionChoice::V1_7)
    );
    assert_eq!(
        PdfArchivalStandard::A3a.recommended_version(),
        Some(PdfVersionChoice::V1_7)
    );
    assert_eq!(
        PdfArchivalStandard::A4.recommended_version(),
        Some(PdfVersionChoice::V2_0)
    );
    assert_eq!(
        PdfArchivalStandard::A4f.recommended_version(),
        Some(PdfVersionChoice::V2_0)
    );
}

#[test]
fn enforces_tagged_pdf_when_pdf_ua1_is_enabled() {
    let cfg = PdfExportConfig {
        version: PdfVersionChoice::V1_7,
        archival_standard: PdfArchivalStandard::None,
        pdf_ua1: true,
        tagged: true,
    };
    assert!(cfg.build_standards().is_ok());
    assert_eq!(cfg.summary_badge(), "PDF 1.7 + PDF/UA-1");
}
