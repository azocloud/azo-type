//! Unit tests for PDF export compilation preparation and failure diagnostics.

use std::sync::{mpsc, Arc};

use super::*;
use crate::world::SharedWorldResources;

#[test]
fn compile_pdf_for_export_succeeds_for_valid_typst_project() {
    let res = Arc::new(SharedWorldResources::new());
    let manager = EngineManager::new(res);
    let project = DiskProject::blank();
    let config = PdfExportConfig::default();

    let compiled =
        compile_pdf_for_export(&manager, "0.15.1", &project, &config).unwrap_or_else(|err| {
            panic!(
                "expected valid PDF compilation, got {:?}",
                err.export_pdf_error
            )
        });

    assert!(compiled.pdf_bytes.starts_with(b"%PDF-"));
    assert_eq!(compiled.default_pdf_name, "main.pdf");
    assert!(!compiled.console_logs.is_empty());
}

#[test]
fn run_pdf_export_flow_returns_error_outcome_for_invalid_typst() {
    let res = Arc::new(SharedWorldResources::new());
    let manager = EngineManager::new(res);
    let mut project = DiskProject::blank();
    let active = project.active_file.clone();
    project.buffers.get_mut(&active).unwrap().text = Some("#undefined_symbol".to_string());

    let ctx = Context::default();
    let (tx, _rx) = mpsc::channel::<(String, String)>();
    let outcome = run_pdf_export_flow(
        &ctx,
        &manager,
        "0.15.1",
        &project,
        &PdfExportConfig::default(),
        tx,
        |s, c| (s, c),
    );

    assert!(!outcome.exported);
    assert!(outcome.export_pdf_error.is_some());
    assert!(outcome.diagnostics.as_ref().is_some_and(|d| !d.is_empty()));
    assert!(outcome
        .console_logs
        .iter()
        .any(|line| line.contains("[PDF Export] Failed after")));
}
