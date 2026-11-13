//! Application background services and runners.
//!
//! - [`pdf_export`]: PDF compilation and asynchronous OS save-dialog workflow orchestration.
//! - [`persistence`]: Persistent settings, language, and zoom state via `eframe::Storage`.
//! - [`project_actions`]: Project file creation, renaming, and deletion workflows with collab sync.
//! - [`screenshot`]: Headless screenshot automation lifecycle runner.

pub mod language_actions;
pub mod pdf_export;
pub mod persistence;
pub mod project_actions;
pub mod screenshot;

pub use pdf_export::{run_pdf_export_flow, PdfExportFlowOutcome};
pub use persistence::{
    flush_if_changed, load_persisted_state, save_persisted_parts, save_persisted_state,
    PersistedAppState,
};
pub use project_actions::{
    create_project_file, delete_project_file, open_picked_folder, poll_collab_session_events,
    rename_project_file, save_active_or_prompt, save_all_project_files,
    spawn_project_folder_picker,
};
pub use screenshot::handle_screenshot_lifecycle;
