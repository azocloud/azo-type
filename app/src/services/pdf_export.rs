//! PDF compilation and asynchronous OS save-dialog workflow orchestration.

use std::path::PathBuf;
use std::sync::mpsc::Sender;

use egui::Context;

use crate::core::pdf::PdfExportConfig;
use crate::engine::{DiagnosticItem, EngineManager};
use crate::platform::dialogs::spawn_save_file_dialog;
use crate::project::DiskProject;

/// Outcome of triggering a PDF export from the modal dialog.
pub struct PdfExportFlowOutcome {
    pub exported: bool,
    pub export_pdf_error: Option<String>,
    pub diagnostics: Option<Vec<DiagnosticItem>>,
    pub status_notice: String,
    pub fallback_notice: Option<String>,
}

/// Runs PDF compilation via `engine_manager` for `requested_version` and opens the save dialog on success.
pub fn run_pdf_export_flow<E: Send + 'static>(
    ctx: &Context,
    engine_manager: &EngineManager,
    requested_version: &str,
    project: &DiskProject,
    config: &PdfExportConfig,
    tx: Sender<E>,
    make_status_event: impl FnOnce(String) -> E + Send + 'static,
) -> PdfExportFlowOutcome {
    let (engine, fallback_notice) = engine_manager.resolve_engine(requested_version);
    let entry_file = project.effective_compile_entrypoint();
    let stem = entry_file
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("document");
    let default_pdf_name = format!("{stem}.pdf");
    let start_dir = if project.has_disk_backing() {
        project.root_dir.clone()
    } else {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    };
    let badge = config.summary_badge();

    match engine.export_pdf_bytes(project, config) {
        Ok(pdf_bytes) => {
            let size_str = format!("{:.1}", pdf_bytes.len() as f64 / 1024.0);
            let status_notice = rust_i18n::t!(
                "app.status_pdf_exporting",
                badge = badge.as_str(),
                size = size_str.as_str()
            )
            .to_string();
            let ctx_clone = ctx.clone();

            spawn_save_file_dialog(
                &start_dir,
                &default_pdf_name,
                "PDF Document",
                "pdf",
                pdf_bytes,
                move |res| {
                    let msg = match res {
                        Ok(p) => rust_i18n::t!(
                            "app.status_pdf_exported",
                            badge = badge.as_str(),
                            path = p.display().to_string(),
                            size = size_str.as_str()
                        )
                        .to_string(),
                        Err(e) => format!("PDF write failed: {e}"),
                    };
                    let _ = tx.send(make_status_event(msg));
                },
                move || ctx_clone.request_repaint(),
            );

            PdfExportFlowOutcome {
                exported: true,
                export_pdf_error: None,
                diagnostics: None,
                status_notice,
                fallback_notice,
            }
        }
        Err(diagnostics) => {
            let first_error = diagnostics
                .first()
                .map(|d| d.message.clone())
                .unwrap_or_else(|| rust_i18n::t!("app.status_pdf_export_failed").to_string());
            let status_notice =
                rust_i18n::t!("app.status_cannot_export_pdf", error = first_error.as_str())
                    .to_string();

            PdfExportFlowOutcome {
                exported: false,
                export_pdf_error: Some(first_error),
                diagnostics: Some(diagnostics),
                status_notice,
                fallback_notice,
            }
        }
    }
}
