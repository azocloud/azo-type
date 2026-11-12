//! Unit tests for end-to-end multi-file P2P session synchronization over Iroh.

use std::time::Instant;

use super::*;

#[test]
fn test_two_sessions_sync_multi_file_project_over_iroh() {
    let temp_base =
        std::env::temp_dir().join(format!("typst_collab_test_{}", uuid::Uuid::new_v4()));
    let host_dir = temp_base.join("host_proj");
    let join_dir = temp_base.join("join_proj");
    std::fs::create_dir_all(&host_dir).unwrap();
    std::fs::create_dir_all(&join_dir).unwrap();

    std::fs::write(
        host_dir.join("main.typ"),
        "= Shared Document\n\nInitial text.\n",
    )
    .unwrap();
    std::fs::write(
        host_dir.join("second.typ"),
        "== Second File\n\nContent B.\n",
    )
    .unwrap();

    let host_handle = CollabSessionHandle::spawn(None);
    let join_handle = CollabSessionHandle::spawn(None);

    host_handle.send(UiToSessionCmd::StartHosting {
        project_root: host_dir.clone(),
        local_name: "AliceHost".to_string(),
        local_color: [56, 189, 248],
        local_only: true,
    });

    let start = Instant::now();
    let mut ticket_str = None;
    while start.elapsed() < Duration::from_secs(6) {
        if let Some(SessionToUiEvent::SessionStarted {
            ticket_str: Some(t),
            ..
        }) = host_handle.try_recv()
        {
            ticket_str = Some(t);
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let ticket = ticket_str.expect("Host should emit invite ticket");

    join_handle.send(UiToSessionCmd::JoinWithTicket {
        project_root: join_dir.clone(),
        ticket_str: ticket,
        local_name: "BobPeer".to_string(),
        local_color: [244, 114, 182],
        local_only: true,
    });

    // Wait until Bob receives initial files from Alice and writes them to join_dir
    let sync_start = Instant::now();
    let mut bob_synced = false;
    while sync_start.elapsed() < Duration::from_secs(6) {
        if let Some(SessionToUiEvent::RemoteFilesUpdated { files, .. }) = join_handle.try_recv() {
            if files.len() >= 2 {
                bob_synced = true;
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(bob_synced, "Bob should receive initial files from host");
    assert!(join_dir.join("main.typ").is_file());
    assert!(join_dir.join("second.typ").is_file());

    // Now Bob edits second.typ while Alice edits main.typ (2 people in 2 different files!)
    join_handle.send(UiToSessionCmd::LocalTextEdited {
        rel_path: PathBuf::from("second.typ"),
        new_text: "== Second File\n\nEdited by Bob in real time! 🦀\n".to_string(),
    });
    host_handle.send(UiToSessionCmd::LocalTextEdited {
        rel_path: PathBuf::from("main.typ"),
        new_text: "= Shared Document\n\nEdited by Alice in main.typ! ÄÖÜ\n".to_string(),
    });

    let edit_wait = Instant::now();
    let mut host_got_second = false;
    let mut join_got_main = false;
    while edit_wait.elapsed() < Duration::from_secs(6) && (!host_got_second || !join_got_main) {
        if let Some(SessionToUiEvent::RemoteFilesUpdated { files, .. }) = host_handle.try_recv() {
            if files
                .iter()
                .any(|f| f.resolved_path == "second.typ" && f.content.contains("Edited by Bob"))
            {
                host_got_second = true;
            }
        }
        if let Some(SessionToUiEvent::RemoteFilesUpdated { files, .. }) = join_handle.try_recv() {
            if files
                .iter()
                .any(|f| f.resolved_path == "main.typ" && f.content.contains("Edited by Alice"))
            {
                join_got_main = true;
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }

    assert!(
        host_got_second,
        "Host should receive Bob's edit to second.typ"
    );
    assert!(join_got_main, "Bob should receive Alice's edit to main.typ");

    host_handle.send(UiToSessionCmd::LeaveSession);
    join_handle.send(UiToSessionCmd::LeaveSession);
    let _ = std::fs::remove_dir_all(&temp_base);
}
