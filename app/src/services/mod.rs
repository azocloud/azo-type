//! Application background services and runners.
//!
//! - [`pdf_export`]: PDF compilation and asynchronous OS save-dialog workflow orchestration.
//! - [`persistence`]: Persistent settings, language, and zoom state via `eframe::Storage`.
//! - [`screenshot`]: Headless screenshot automation lifecycle runner.

pub mod pdf_export;
pub mod persistence;
pub mod screenshot;

pub use pdf_export::{run_pdf_export_flow, PdfExportFlowOutcome};
pub use persistence::{
    flush_if_changed, load_persisted_state, save_persisted_parts, save_persisted_state,
    PersistedAppState,
};
pub use screenshot::handle_screenshot_lifecycle;
