//! Unit tests for project file actions, folder switching, and collaboration event routing.

use std::path::PathBuf;
use std::sync::mpsc;

use super::*;
use crate::collab::awareness::PeerPresence;
use crate::collab::doc::CollabFileEntry;
use crate::collab::protocol::ConnectionRoute;
use crate::collab::session::{CollabSessionHandle, ConnectedPeerStatus, PeerLifecycleState};
use crate::core::recent_projects::RecentProjects;

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "azo_type_project_actions_{name}_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn create_rename_delete_save_and_open_folder_update_disk_project() {
    let dir = temp_dir("crud");
    std::fs::write(dir.join("main.typ"), "= Hello\n").unwrap();

    let mut project = DiskProject::open_folder(&dir, false).unwrap();
    let collab = CollabSessionHandle::spawn(None);

    let (ok_create, _) = create_project_file(&mut project, false, &collab, "notes.typ");
    assert!(ok_create);
    assert!(dir.join("notes.typ").exists());

    let (ok_rename, _) = rename_project_file(
        &mut project,
        false,
        &collab,
        Path::new("notes.typ"),
        "chapter1.typ",
    );
    assert!(ok_rename);
    assert!(!dir.join("notes.typ").exists());
    assert!(dir.join("chapter1.typ").exists());

    if let Some(buf) = project.active_buffer_mut() {
        buf.text = Some("= Updated\n".to_string());
        buf.dirty = true;
    }
    let ctx = Context::default();
    let (tx, _rx) = mpsc::channel::<()>();
    let saved = save_active_or_prompt(&ctx, &mut project, tx, |_| (), |_| ());
    assert!(saved.is_some_and(|(ok, _)| ok));

    let (saved_all, _) = save_all_project_files(&mut project);
    assert!(saved_all);

    let (ok_del, _) = delete_project_file(&mut project, false, &collab, Path::new("chapter1.typ"));
    assert!(ok_del);
    assert!(!dir.join("chapter1.typ").exists());

    let (reopened, _) = open_picked_folder(&dir, false, false, &collab, &mut project);
    assert!(reopened);

    let missing_dir = dir.join("does_not_exist");
    let (failed_open, _) = open_picked_folder(&missing_dir, false, false, &collab, &mut project);
    assert!(!failed_open);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn save_all_rejects_blank_project_without_disk_backing() {
    let mut blank = DiskProject::blank();
    let (ok, notice) = save_all_project_files(&mut blank);
    assert!(!ok);
    assert!(!notice.is_empty());
}

#[test]
fn apply_collab_session_event_updates_ui_and_project_state() {
    let mut collab_ui = CollabUiState::default();
    let mut project = DiskProject::blank();
    let mut pending_cursor = None;

    let entry = CollabFileEntry {
        file_id: "f1".to_string(),
        raw_path: "main.typ".to_string(),
        resolved_path: "main.typ".to_string(),
        content: "= Synced\n".to_string(),
    };

    let (compile1, _) = apply_collab_session_event(
        SessionToUiEvent::SessionStarted {
            is_host: true,
            ticket_str: Some("ticket-123".to_string()),
            local_endpoint_id: "ep-1".to_string(),
            files: vec![entry.clone()],
        },
        &mut collab_ui,
        &mut project,
        &mut pending_cursor,
    );
    assert!(compile1);
    assert!(collab_ui.is_active);
    assert!(collab_ui.is_host);

    let (compile_peers, _) = apply_collab_session_event(
        SessionToUiEvent::PeerListChanged {
            peers: vec![ConnectedPeerStatus {
                peer_id: "p1".to_string(),
                display_name: "Ada".to_string(),
                state: PeerLifecycleState::Connected,
                route: ConnectionRoute::Direct {
                    addr: "127.0.0.1:4433".to_string(),
                    rtt_ms: 8,
                },
            }],
        },
        &mut collab_ui,
        &mut project,
        &mut pending_cursor,
    );
    assert!(!compile_peers);
    assert_eq!(collab_ui.peers.len(), 1);

    let (compile_remote, _) = apply_collab_session_event(
        SessionToUiEvent::RemoteFilesUpdated {
            files: vec![entry],
            disk_updated_paths: vec![],
            preserved_selection: Some(CharSelection::caret(4)),
        },
        &mut collab_ui,
        &mut project,
        &mut pending_cursor,
    );
    assert!(compile_remote);
    assert_eq!(pending_cursor, Some(CharSelection::caret(4)));

    let (_, _) = apply_collab_session_event(
        SessionToUiEvent::RemotePresenceUpdated {
            presences: vec![PeerPresence {
                client_id: 2,
                endpoint_id: "p1".to_string(),
                name: "Ada".to_string(),
                color_rgb: [10, 20, 30],
                active_file_id: Some("f1".to_string()),
                active_path: Some("main.typ".to_string()),
                cursor_char: Some(2),
                anchor_char: None,
            }],
        },
        &mut collab_ui,
        &mut project,
        &mut pending_cursor,
    );
    assert_eq!(collab_ui.remote_presences.len(), 1);

    let (_, notice) = apply_collab_session_event(
        SessionToUiEvent::StatusNotice {
            message: "Peer joined".to_string(),
        },
        &mut collab_ui,
        &mut project,
        &mut pending_cursor,
    );
    assert_eq!(notice.as_deref(), Some("Peer joined"));

    let (_, err_notice) = apply_collab_session_event(
        SessionToUiEvent::CollabError {
            message: "timeout".to_string(),
        },
        &mut collab_ui,
        &mut project,
        &mut pending_cursor,
    );
    assert_eq!(collab_ui.last_error.as_deref(), Some("timeout"));
    assert!(err_notice.is_some_and(|n| n.contains("timeout")));

    let (compile_stop, stop_notice) = apply_collab_session_event(
        SessionToUiEvent::SessionStopped {
            reason: "Closed".to_string(),
        },
        &mut collab_ui,
        &mut project,
        &mut pending_cursor,
    );
    assert!(!compile_stop);
    assert!(!collab_ui.is_active);
    assert_eq!(stop_notice.as_deref(), Some("Closed"));
}

#[test]
fn dispatch_collab_window_actions_handles_copy_and_session_cmds() {
    let ctx = Context::default();
    let collab = CollabSessionHandle::spawn(None);
    dispatch_collab_window_actions(
        &ctx,
        Path::new("."),
        &collab,
        vec![
            CollabWindowAction::CopyTicket("abc".to_string()),
            CollabWindowAction::UpdateIdentity {
                local_name: "Grace".to_string(),
                local_color: [1, 2, 3],
            },
            CollabWindowAction::LeaveSession,
        ],
    );
}

#[test]
fn canonicalize_project_path_resolves_real_dir_and_falls_back_for_missing() {
    let dir = temp_dir("canon");
    let nested = dir.join("sub").join("..").join("sub");
    std::fs::create_dir_all(&nested).unwrap();

    let canonical = canonicalize_project_path(&nested);
    assert_eq!(canonical, std::fs::canonicalize(&nested).unwrap());

    let missing = dir.join("does_not_exist");
    assert_eq!(canonicalize_project_path(&missing), missing);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn strip_windows_verbatim_prefix_handles_drive_and_unc_paths() {
    assert_eq!(
        strip_windows_verbatim_prefix(PathBuf::from(r"\\?\C:\Users\Ada\doc")),
        PathBuf::from(r"C:\Users\Ada\doc")
    );
    assert_eq!(
        strip_windows_verbatim_prefix(PathBuf::from(r"\\?\UNC\server\share\proj")),
        PathBuf::from(r"\\server\share\proj")
    );
    assert_eq!(
        strip_windows_verbatim_prefix(PathBuf::from("/tmp/project")),
        PathBuf::from("/tmp/project")
    );
}

#[test]
fn resolve_startup_project_prioritizes_initial_folder_over_recent() {
    let cli_dir = temp_dir("startup_cli");
    let last_dir = temp_dir("startup_last");
    std::fs::write(cli_dir.join("main.typ"), "= CLI\n").unwrap();
    std::fs::write(last_dir.join("main.typ"), "= Last\n").unwrap();

    let mut recent = RecentProjects::from_paths([canonicalize_project_path(&last_dir)]);
    let project = resolve_startup_project(Some(cli_dir.clone()), true, &mut recent, false);

    assert!(project.has_disk_backing());
    assert_eq!(project.root_dir, cli_dir);
    assert_eq!(
        recent.most_recent(),
        Some(canonicalize_project_path(&cli_dir).as_path())
    );

    let _ = std::fs::remove_dir_all(&cli_dir);
    let _ = std::fs::remove_dir_all(&last_dir);
}

#[test]
fn resolve_startup_project_respects_open_last_project_flag_and_missing_fallback() {
    let last_dir = temp_dir("startup_flag");
    std::fs::write(last_dir.join("main.typ"), "= Last\n").unwrap();

    let mut recent = RecentProjects::from_paths([canonicalize_project_path(&last_dir)]);

    // Disabled -> blank project
    let blank = resolve_startup_project(None, false, &mut recent, false);
    assert!(!blank.has_disk_backing());

    // Enabled -> opens most recent project
    let opened = resolve_startup_project(None, true, &mut recent, false);
    assert!(opened.has_disk_backing());
    assert_eq!(opened.root_dir, canonicalize_project_path(&last_dir));

    // Missing directory -> falls back to blank without panicking
    let _ = std::fs::remove_dir_all(&last_dir);
    let fallback = resolve_startup_project(None, true, &mut recent, false);
    assert!(!fallback.has_disk_backing());
}

#[test]
fn open_folder_and_record_updates_recent_on_success_and_prunes_missing_on_failure() {
    let dir_a = temp_dir("record_a");
    let dir_b = temp_dir("record_b");
    std::fs::write(dir_a.join("main.typ"), "= A\n").unwrap();
    std::fs::write(dir_b.join("main.typ"), "= B\n").unwrap();

    let mut project = DiskProject::blank();
    let collab = CollabSessionHandle::spawn(None);
    let mut recent = RecentProjects::new();

    let (ok_a, _) =
        open_folder_and_record(&dir_a, false, false, &collab, &mut project, &mut recent);
    assert!(ok_a);
    assert_eq!(
        recent.most_recent(),
        Some(canonicalize_project_path(&dir_a).as_path())
    );

    let (ok_b, _) =
        open_folder_and_record(&dir_b, false, false, &collab, &mut project, &mut recent);
    assert!(ok_b);
    assert_eq!(
        recent.most_recent(),
        Some(canonicalize_project_path(&dir_b).as_path())
    );

    // Delete dir_a on disk and try opening it via recent -> should fail and prune dir_a
    let canon_a = canonicalize_project_path(&dir_a);
    std::fs::remove_dir_all(&dir_a).unwrap();
    let (ok_missing, notice) =
        open_folder_and_record(&canon_a, false, false, &collab, &mut project, &mut recent);
    assert!(!ok_missing);
    assert!(!notice.is_empty());
    assert!(!recent.entries().contains(&canon_a));
    assert_eq!(
        recent.most_recent(),
        Some(canonicalize_project_path(&dir_b).as_path())
    );

    let _ = std::fs::remove_dir_all(&dir_b);
}
