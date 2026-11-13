//! PDF compilation and asynchronous OS save-dialog workflow orchestration.

use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::time::Instant;

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
    pub console_logs: Vec<String>,
}

/// Runs PDF compilation via `engine_manager` for `requested_version` and opens the save dialog on success.
pub fn run_pdf_export_flow<E: Send + 'static>(
    ctx: &Context,
    engine_manager: &EngineManager,
    requested_version: &str,
    project: &DiskProject,
    config: &PdfExportConfig,
    tx: Sender<E>,
    make_event: impl FnOnce(String, String) -> E + Send + 'static,
) -> PdfExportFlowOutcome {
    let start = Instant::now();
    let (engine, fallback_notice) = engine_manager.resolve_engine(requested_version);
    let engine_meta = engine.metadata();
    let engine_kind = if engine_meta.is_builtin {
        "Built-in"
    } else {
        "Managed Binary"
    };
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
    let mut console_logs = vec![format!(
        "[PDF Export] Started | Entry: {} | Engine: Typst {} ({}) | Standard: {} | Tagged: {}",
        entry_file.display(),
        engine_meta.version,
        engine_kind,
        badge,
        config.tagged,
    )];
    if let Some(ref fb) = fallback_notice {
        console_logs.push(format!("[PDF Export] Fallback notice: {fb}"));
    }

    match engine.export_pdf_bytes(project, config) {
        Ok(pdf_bytes) => {
            let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
            let byte_len = pdf_bytes.len();
            let size_str = format!("{:.1}", byte_len as f64 / 1024.0);
            console_logs.push(format!(
                "[PDF Export] Compiled PDF in {:.1} ms ({size_str} KB, {byte_len} bytes) — awaiting save destination...",
                elapsed_ms
            ));
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
                    let (status_msg, console_msg) = match res {
                        Ok(p) => {
                            let path_str = p.display().to_string();
                            let status = rust_i18n::t!(
                                "app.status_pdf_exported",
                                badge = badge.as_str(),
                                path = path_str.as_str(),
                                size = size_str.as_str()
                            )
                            .to_string();
                            let console = format!(
                                "[PDF Export] Saved '{path_str}' ({size_str} KB, {byte_len} bytes, {badge})"
                            );
                            (status, console)
                        }
                        Err(e) => (
                            format!("PDF write failed: {e}"),
                            format!("[PDF Export] Save cancelled or failed: {e}"),
                        ),
                    };
                    let _ = tx.send(make_event(status_msg, console_msg));
                },
                move || ctx_clone.request_repaint(),
            );

            PdfExportFlowOutcome {
                exported: true,
                export_pdf_error: None,
                diagnostics: None,
                status_notice,
                fallback_notice,
                console_logs,
            }
        }
        Err(diagnostics) => {
            let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
            console_logs.push(format!(
                "[PDF Export] Failed after {:.1} ms ({} diagnostic(s)):",
                elapsed_ms,
                diagnostics.len()
            ));
            for d in &diagnostics {
                let kind = if d.is_error { "ERROR" } else { "WARN" };
                let loc = match (d.line, d.column) {
                    (Some(l), Some(c)) => format!("{}:{}:{}", d.file.display(), l, c),
                    (Some(l), None) => format!("{}:{}", d.file.display(), l),
                    _ => d.file.display().to_string(),
                };
                console_logs.push(format!("[PDF Export]   [{kind}] {loc}: {}", d.message));
            }
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
                console_logs,
            }
        }
    }
}
