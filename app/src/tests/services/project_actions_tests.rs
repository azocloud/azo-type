//! Unit tests for project file actions, folder switching, and collaboration event routing.

use std::path::PathBuf;
use std::sync::mpsc;

use super::*;
use crate::collab::awareness::PeerPresence;
use crate::collab::doc::CollabFileEntry;
use crate::collab::protocol::ConnectionRoute;
use crate::collab::session::{CollabSessionHandle, ConnectedPeerStatus, PeerLifecycleState};

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
