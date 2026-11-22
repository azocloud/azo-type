//! Application background services and runners.
//!
//! - [`bottom_panel_actions`]: Bottom panel search, replace, and output wiring helpers.
//! - [`pdf_export`]: PDF compilation and asynchronous OS save-dialog workflow orchestration.
//! - [`persistence`]: Persistent settings, language, and zoom state via `eframe::Storage`.
//! - [`project_actions`]: Project file creation, renaming, and deletion workflows with collab sync.
//! - [`screenshot`]: Headless screenshot automation lifecycle runner.

pub mod bottom_panel_actions;
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
    canonicalize_project_path, create_project_file, delete_project_file,
    dispatch_collab_window_actions, format_outline_nav_notice, open_folder_and_record,
    open_picked_folder, open_sidebar_file, poll_collab_session_events, rename_project_file,
    rescan_project_folder, resolve_startup_project, save_active_or_prompt,
    save_active_to_path_notice, save_all_project_files, send_collab_undo_redo,
    spawn_project_folder_picker, sync_collab_cursor,
};
pub use screenshot::handle_screenshot_lifecycle;
