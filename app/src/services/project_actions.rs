//! Project file creation, renaming, and deletion workflows synchronized with collaboration state.

use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;

use egui::Context;

use crate::collab::doc::CharSelection;
use crate::collab::session::{CollabSessionHandle, SessionToUiEvent, UiToSessionCmd};
use crate::collab::ui::CollabUiState;
use crate::platform::dialogs::{spawn_open_folder_dialog, spawn_save_file_dialog};
use crate::project::DiskProject;

/// Creates `new_file_str` on disk (or in memory) and notifies peers when P2P collab is active.
///
/// Returns `(succeeded, status_notice)`.
pub fn create_project_file(
    project: &mut DiskProject,
    collab_active: bool,
    collab_session: &CollabSessionHandle,
    new_file_str: &str,
) -> (bool, String) {
    match project.create_file_on_disk(new_file_str) {
        Ok(rel) => {
            let initial_text = project.read_source_text(&rel).unwrap_or_default();
            if collab_active {
                collab_session.send(UiToSessionCmd::LocalFileCreated {
                    rel_path: rel.clone(),
                    initial_text,
                });
            }
            let disk_p = project.root_dir.join(&rel).display().to_string();
            (
                true,
                rust_i18n::t!("app.status_created_file", path = disk_p).to_string(),
            )
        }
        Err(e) => (
            false,
            rust_i18n::t!("app.status_create_failed", error = e.to_string()).to_string(),
        ),
    }
}

/// Renames `old_rel` to `new_str` on disk and notifies peers when P2P collab is active.
pub fn rename_project_file(
    project: &mut DiskProject,
    collab_active: bool,
    collab_session: &CollabSessionHandle,
    old_rel: &Path,
    new_str: &str,
) -> (bool, String) {
    match project.rename_file_on_disk(old_rel, new_str) {
        Ok(new_rel) => {
            if collab_active {
                collab_session.send(UiToSessionCmd::LocalFileRenamed {
                    old_rel_path: old_rel.to_path_buf(),
                    new_rel_path: new_rel.clone(),
                });
            }
            let old_s = old_rel.display().to_string();
            let new_s = new_rel.display().to_string();
            (
                true,
                rust_i18n::t!("app.status_renamed_file", old = old_s, new = new_s).to_string(),
            )
        }
        Err(e) => (
            false,
            rust_i18n::t!("app.status_rename_failed", error = e.to_string()).to_string(),
        ),
    }
}

/// Deletes `rel_to_delete` on disk and notifies peers when P2P collab is active.
pub fn delete_project_file(
    project: &mut DiskProject,
    collab_active: bool,
    collab_session: &CollabSessionHandle,
    rel_to_delete: &Path,
) -> (bool, String) {
    match project.delete_file_on_disk(rel_to_delete) {
        Ok(()) => {
            if collab_active {
                collab_session.send(UiToSessionCmd::LocalFileDeleted {
                    rel_path: rel_to_delete.to_path_buf(),
                });
            }
            let del_s = rel_to_delete.display().to_string();
            (
                true,
                rust_i18n::t!("app.status_deleted_file", path = del_s).to_string(),
            )
        }
        Err(e) => (
            false,
            rust_i18n::t!("app.status_delete_failed", error = e.to_string()).to_string(),
        ),
    }
}

/// Saves the active project file to disk (or opens an async Save-As dialog for blank projects).
///
/// Returns `Some((saved_now, status_notice))` when synchronous disk saving occurred.
pub fn save_active_or_prompt<E: Send + 'static>(
    ctx: &Context,
    project: &mut DiskProject,
    tx: Sender<E>,
    on_saved_as: impl FnOnce(PathBuf) -> E + Send + 'static,
    on_error: impl FnOnce(String) -> E + Send + 'static,
) -> Option<(bool, String)> {
    if !project.has_disk_backing() {
        let start = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let f = &project.active_file;
        let name = f.file_name().and_then(|n| n.to_str()).unwrap_or("main.typ");
        let data = project.read_file_bytes(f).unwrap_or_default();
        let ctx_c = ctx.clone();
        spawn_save_file_dialog(
            &start,
            name,
            "Typst File (*.typ)",
            "typ",
            data,
            move |res| {
                let ev = match res {
                    Ok(p) => on_saved_as(p),
                    Err(e) => on_error(
                        rust_i18n::t!("app.status_save_error", error = e.to_string()).to_string(),
                    ),
                };
                let _ = tx.send(ev);
            },
            move || ctx_c.request_repaint(),
        );
        return None;
    }

    Some(match project.save_active_file() {
        Ok(p) => (
            true,
            rust_i18n::t!("app.status_saved_file", path = p.display().to_string()).to_string(),
        ),
        Err(e) => (
            false,
            rust_i18n::t!("app.status_save_error", error = e.to_string()).to_string(),
        ),
    })
}

/// Saves all modified project buffers to disk when the project has disk backing.
pub fn save_all_project_files(project: &mut DiskProject) -> (bool, String) {
    if !project.has_disk_backing() {
        return (
            false,
            rust_i18n::t!("app.status_unsaved_warning").to_string(),
        );
    }
    match project.save_all_files() {
        Ok(n) => {
            let p = project.root_dir.display().to_string();
            (
                true,
                rust_i18n::t!("app.status_saved_all", count = n, path = p).to_string(),
            )
        }
        Err(e) => (
            false,
            rust_i18n::t!("app.status_save_error", error = e.to_string()).to_string(),
        ),
    }
}

/// Replaces `project` with the folder at `path`, leaving any active collab session.
///
/// Returns `(opened_ok, status_notice)`.
pub fn open_picked_folder(
    path: &Path,
    show_hidden_files: bool,
    collab_active: bool,
    collab_session: &CollabSessionHandle,
    project: &mut DiskProject,
) -> (bool, String) {
    if collab_active {
        collab_session.send(UiToSessionCmd::LeaveSession);
    }
    match DiskProject::open_folder(path, show_hidden_files) {
        Ok(proj) => {
            let name = proj.project_name.clone();
            let root_str = proj.root_dir.display().to_string();
            *project = proj;
            (
                true,
                rust_i18n::t!(
                    "app.status_opened_folder",
                    name = name.as_str(),
                    path = root_str
                )
                .to_string(),
            )
        }
        Err(err) => (
            false,
            rust_i18n::t!("app.status_cannot_open_folder", error = err.to_string()).to_string(),
        ),
    }
}

/// Drains pending P2P collaboration actor events onto UI and project state.
///
/// Returns `(need_compile, optional_status_notice)`.
pub fn poll_collab_session_events(
    collab_session: &CollabSessionHandle,
    collab_ui: &mut CollabUiState,
    project: &mut DiskProject,
    pending_cursor_restore: &mut Option<CharSelection>,
) -> (bool, Option<String>) {
    let mut need_compile = false;
    let mut status_notice = None;
    while let Some(ev) = collab_session.try_recv() {
        match ev {
            SessionToUiEvent::SessionStarted {
                is_host,
                ticket_str,
                local_endpoint_id,
                files,
            } => {
                collab_ui.on_session_started(is_host, ticket_str, local_endpoint_id);
                if !files.is_empty() {
                    project.sync_from_crdt_files(&files);
                    need_compile = true;
                }
            }
            SessionToUiEvent::SessionStopped { reason } => {
                collab_ui.reset_session();
                collab_ui.is_host = false;
                collab_ui.status_banner = Some(reason.clone());
                status_notice = Some(reason);
            }
            SessionToUiEvent::PeerListChanged { peers } => collab_ui.peers = peers,
            SessionToUiEvent::RemoteFilesUpdated {
                files,
                disk_updated_paths: _,
                preserved_selection,
            } => {
                project.sync_from_crdt_files(&files);
                if let Some(sel) = preserved_selection {
                    *pending_cursor_restore = Some(sel);
                }
                need_compile = true;
            }
            SessionToUiEvent::RemotePresenceUpdated { presences } => {
                collab_ui.remote_presences = presences;
            }
            SessionToUiEvent::StatusNotice { message } => {
                collab_ui.status_banner = Some(message.clone());
                status_notice = Some(message);
            }
            SessionToUiEvent::CollabError { message } => {
                collab_ui.last_error = Some(message.clone());
                status_notice = Some(format!("Collab error: {message}"));
            }
        }
    }
    (need_compile, status_notice)
}

/// Spawns an OS folder picker rooted at `project.root_dir` (or cwd for blank projects).
pub fn spawn_project_folder_picker<E: Send + 'static>(
    ctx: &Context,
    project: &DiskProject,
    io_tx: Sender<E>,
    make_event: fn(PathBuf) -> E,
) {
    let ctx_clone = ctx.clone();
    let start = if project.has_disk_backing() {
        project.root_dir.clone()
    } else {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    };
    spawn_open_folder_dialog(
        &start,
        move |p| {
            let _ = io_tx.send(make_event(p));
        },
        move || ctx_clone.request_repaint(),
    );
}
