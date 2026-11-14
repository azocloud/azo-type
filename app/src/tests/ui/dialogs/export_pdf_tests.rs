//! Pure unit and headless `egui_kittest` tests for the Export PDF dialog.

use std::path::Path;

use egui_kittest::{kittest::Queryable, Harness};
use rust_i18n::t;

use super::*;

struct ExportHarnessState {
    config: PdfExportConfig,
    export_error: Option<String>,
    exported: bool,
    cancelled: bool,
    fallback_notice: Option<&'static str>,
    dark_mode: bool,
}

#[test]
fn apply_archival_standard_adjusts_incompatible_version_and_clears_error() {
    let mut config = PdfExportConfig {
        version: PdfVersionChoice::V2_0,
        ..Default::default()
    };
    let mut err = Some("previous error".to_string());

    apply_archival_standard_choice(&mut config, &mut err, PdfArchivalStandard::A2b);

    assert!(err.is_none());
    assert_eq!(config.archival_standard, PdfArchivalStandard::A2b);
    assert_eq!(config.version, PdfVersionChoice::V1_7);
    assert!(config.build_standards().is_ok());
}

#[test]
fn apply_pdf_ua1_enforces_tagged_and_compatible_version() {
    let mut config = PdfExportConfig {
        version: PdfVersionChoice::V2_0,
        tagged: false,
        pdf_ua1: true,
        ..Default::default()
    };
    let mut err = Some("bad state".to_string());

    apply_pdf_ua1_toggle(&mut config, &mut err);

    assert!(err.is_none());
    assert!(config.tagged);
    assert_eq!(config.version, PdfVersionChoice::V1_7);
    assert!(config.build_standards().is_ok());
}

#[test]
fn standard_requires_tagged_detects_ua1_and_level_a_standards() {
    let mut config = PdfExportConfig::default();
    assert!(!standard_requires_tagged(&config));

    config.archival_standard = PdfArchivalStandard::A2b;
    assert!(!standard_requires_tagged(&config));

    config.archival_standard = PdfArchivalStandard::A2a;
    assert!(standard_requires_tagged(&config));

    config.archival_standard = PdfArchivalStandard::None;
    config.pdf_ua1 = true;
    assert!(standard_requires_tagged(&config));
}

#[test]
fn render_export_pdf_dialog_returns_false_when_closed() {
    let mut open = false;
    let mut config = PdfExportConfig::default();
    let mut export_error = None;
    let ctx = egui::Context::default();

    let result = render_export_pdf_dialog(
        &ctx,
        &mut open,
        &mut config,
        &mut export_error,
        Path::new("main.typ"),
        "0.15.1",
        None,
        true,
    );

    assert!(!result);
    assert!(!open);
}

#[test]
fn export_pdf_body_renders_valid_state_and_emits_export_action() {
    let state = ExportHarnessState {
        config: PdfExportConfig::default(),
        export_error: Some("disk full".to_string()),
        exported: false,
        cancelled: false,
        fallback_notice: Some("Using built-in engine fallback"),
        dark_mode: false,
    };

    let export_label = t!("export_pdf.export_btn", locale = "en").to_string();

    let mut harness = Harness::new_ui_state(
        |ui, state: &mut ExportHarnessState| {
            let (exp, cancel) = render_export_pdf_body(
                ui,
                &mut state.config,
                &mut state.export_error,
                Path::new("chapters/main.typ"),
                "0.15.1 (Built-in)",
                state.fallback_notice,
                state.dark_mode,
            );
            state.exported |= exp;
            state.cancelled |= cancel;
        },
        state,
    );

    harness.get_by_label(&export_label).click();
    harness.run();

    assert!(harness.state().exported);
    assert!(!harness.state().cancelled);
}

#[test]
fn export_pdf_body_allows_switching_to_compatible_version_and_resetting() {
    let state = ExportHarnessState {
        config: PdfExportConfig {
            archival_standard: PdfArchivalStandard::A1b,
            version: PdfVersionChoice::V2_0,
            tagged: false,
            pdf_ua1: false,
        },
        export_error: None,
        exported: false,
        cancelled: false,
        fallback_notice: None,
        dark_mode: true,
    };

    let switch_label = t!(
        "export_pdf.switch_compatible",
        locale = "en",
        version = PdfVersionChoice::V1_4.label()
    )
    .to_string();
    let reset_label = t!("export_pdf.reset_defaults", locale = "en").to_string();
    let cancel_label = t!("export_pdf.cancel_btn", locale = "en").to_string();

    let mut harness = Harness::new_ui_state(
        |ui, state: &mut ExportHarnessState| {
            let (exp, cancel) = render_export_pdf_body(
                ui,
                &mut state.config,
                &mut state.export_error,
                Path::new("main.typ"),
                "0.15.1",
                state.fallback_notice,
                state.dark_mode,
            );
            state.exported |= exp;
            state.cancelled |= cancel;
        },
        state,
    );

    harness.get_by_label(&switch_label).click();
    harness.run();
    assert_eq!(harness.state().config.version, PdfVersionChoice::V1_4);
    assert!(harness.state().config.build_standards().is_ok());

    harness.get_by_label(&reset_label).click();
    harness.run();
    assert_eq!(
        harness.state().config.archival_standard,
        PdfArchivalStandard::None
    );
    assert_eq!(harness.state().config.version, PdfVersionChoice::V1_7);

    harness.get_by_label(&cancel_label).click();
    harness.run();
    assert!(harness.state().cancelled);
}
