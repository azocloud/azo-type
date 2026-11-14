//! Unit and `egui_kittest` UI tests for the P2P collaboration window state and controls.

use std::path::PathBuf;

use egui_kittest::{kittest::Queryable, Harness};

use super::*;
use crate::collab::protocol::ConnectionRoute;
use crate::collab::session::{ConnectedPeerStatus, PeerLifecycleState};

#[test]
fn collab_ui_state_counts_peers_and_filters_by_normalized_path() {
    let mut state = CollabUiState {
        peers: vec![
            ConnectedPeerStatus {
                peer_id: "p1".into(),
                display_name: "Alice".into(),
                state: PeerLifecycleState::Connected,
                route: ConnectionRoute::Unknown,
            },
            ConnectedPeerStatus {
                peer_id: "p2".into(),
                display_name: "Bob".into(),
                state: PeerLifecycleState::Reconnecting { attempt: 1 },
                route: ConnectionRoute::Unknown,
            },
        ],
        remote_presences: vec![
            PeerPresence {
                client_id: 1,
                endpoint_id: "p1".into(),
                name: "Alice".into(),
                color_rgb: [255, 0, 0],
                active_file_id: None,
                active_path: Some("chapters/intro.typ".into()),
                cursor_char: Some(5),
                anchor_char: Some(5),
            },
            PeerPresence {
                client_id: 2,
                endpoint_id: "p2".into(),
                name: "Bob".into(),
                color_rgb: [0, 255, 0],
                active_file_id: None,
                active_path: Some("main.typ".into()),
                cursor_char: None,
                anchor_char: None,
            },
        ],
        ..Default::default()
    };

    assert_eq!(state.connected_peer_count(), 1);
    let in_intro = state.peers_in_file(Path::new("chapters\\intro.typ"));
    assert_eq!(in_intro.len(), 1);
    assert_eq!(in_intro[0].name, "Alice");

    state.on_session_started(true, Some("typst-collab-v1:secret".into()), "host-1".into());
    assert!(state.is_active);
    assert!(state.is_host);
    assert!(state.last_error.is_none());

    state.reset_session();
    assert!(!state.is_active);
    assert!(state.host_ticket.is_none());
    assert!(state.peers.is_empty());
    assert!(state.remote_presences.is_empty());
}

#[test]
fn collab_ui_state_pure_actions_validate_ticket_and_mask_preview() {
    let mut state = CollabUiState::default();
    assert_eq!(state.prepare_join_with_ticket("Alice", [1, 2, 3]), None);

    state.join_ticket_input = "  typst-collab-v1:abc123  ".into();
    state.local_only_mode = true;
    state.last_error = Some("old error".into());
    let join_action = state.prepare_join_with_ticket("Alice", [1, 2, 3]);
    assert_eq!(
        join_action,
        Some(CollabWindowAction::JoinWithTicket {
            ticket_str: "typst-collab-v1:abc123".into(),
            local_name: "Alice".into(),
            local_color: [1, 2, 3],
            local_only: true,
        })
    );
    assert!(state.last_error.is_none());

    let host_action = state.prepare_start_hosting("Alice", [4, 5, 6]);
    assert_eq!(
        host_action,
        CollabWindowAction::StartHosting {
            local_name: "Alice".into(),
            local_color: [4, 5, 6],
            local_only: true,
        }
    );

    assert_eq!(state.prepare_copy_ticket(), None);
    let long_ticket = format!("typst-collab-v1:{}", "x".repeat(60));
    state.host_ticket = Some(long_ticket.clone());
    assert!(state.masked_ticket_preview(&long_ticket).contains("•••"));
    state.toggle_reveal_ticket();
    assert!(state
        .masked_ticket_preview(&long_ticket)
        .starts_with("typst-collab-v1:xxx"));
    assert_eq!(
        state.prepare_copy_ticket(),
        Some(CollabWindowAction::CopyTicket(long_ticket))
    );

    state.last_error = Some("oops".into());
    state.dismiss_error();
    assert!(state.last_error.is_none());
}

#[test]
fn kittest_offline_collab_window_emits_host_and_join_actions_and_dismisses_error() {
    let mut state = CollabUiState {
        show_window: true,
        last_error: Some("Handshake failed".into()),
        status_banner: Some("Ready".into()),
        join_ticket_input: "typst-collab-v1:joinme".into(),
        ..Default::default()
    };
    let mut user_name = "Alice".to_string();
    let mut user_color = [56, 189, 248];
    let project_root = PathBuf::from("/tmp/demo");
    let mut emitted = Vec::new();

    {
        let mut harness = Harness::new_ui(|ui| {
            emitted.extend(render_collaboration_body(
                ui,
                true,
                &project_root,
                &mut user_name,
                &mut user_color,
                &mut state,
            ));
        });

        harness
            .get_by_label(t!("collab.dismiss_btn").as_ref())
            .click();
        harness.run();
        harness.get_by_label(t!("collab.lan_only").as_ref()).click();
        harness.run();
        harness
            .get_by_label(t!("collab.start_host_btn").as_ref())
            .click();
        harness.run();
        harness.get_by_label(t!("collab.join_btn").as_ref()).click();
        harness.run();
    }

    assert!(state.last_error.is_none());
    assert!(state.local_only_mode);
    assert_eq!(
        emitted,
        vec![
            CollabWindowAction::StartHosting {
                local_name: "Alice".into(),
                local_color: [56, 189, 248],
                local_only: true,
            },
            CollabWindowAction::JoinWithTicket {
                ticket_str: "typst-collab-v1:joinme".into(),
                local_name: "Alice".into(),
                local_color: [56, 189, 248],
                local_only: true,
            },
        ]
    );
}

#[test]
fn kittest_active_collab_window_renders_peers_and_emits_copy_and_leave() {
    let mut state = CollabUiState {
        show_window: true,
        is_active: true,
        is_host: true,
        local_endpoint_id: "0123456789abcdef0123456789abcdef".into(),
        host_ticket: Some("typst-collab-v1:secret-ticket-data".into()),
        peers: vec![
            ConnectedPeerStatus {
                peer_id: "peer-direct-123456".into(),
                display_name: "Bob".into(),
                state: PeerLifecycleState::Connected,
                route: ConnectionRoute::Direct {
                    addr: "127.0.0.1:4242".into(),
                    rtt_ms: 4,
                },
            },
            ConnectedPeerStatus {
                peer_id: "peer-relay-654321".into(),
                display_name: "Carol".into(),
                state: PeerLifecycleState::Disconnected,
                route: ConnectionRoute::Relay {
                    url: "https://relay.example".into(),
                    rtt_ms: 42,
                },
            },
        ],
        remote_presences: vec![PeerPresence {
            client_id: 10,
            endpoint_id: "peer-direct-123456".into(),
            name: "Bob".into(),
            color_rgb: [100, 200, 120],
            active_file_id: None,
            active_path: Some("main.typ".into()),
            cursor_char: Some(0),
            anchor_char: Some(0),
        }],
        ..Default::default()
    };
    let mut user_name = "HostAlice".to_string();
    let mut user_color = [200, 80, 80];
    let project_root = PathBuf::from("/tmp/demo");
    let mut emitted = Vec::new();

    {
        let mut harness = Harness::new_ui(|ui| {
            emitted.extend(render_collaboration_body(
                ui,
                false,
                &project_root,
                &mut user_name,
                &mut user_color,
                &mut state,
            ));
        });

        harness
            .get_by_label(t!("collab.reveal_ticket").as_ref())
            .click();
        harness.run();
        harness
            .get_by_label(t!("collab.copy_ticket_btn").as_ref())
            .click();
        harness.run();
        harness
            .get_by_label(t!("collab.end_session_btn").as_ref())
            .click();
        harness.run();
    }

    assert!(state.reveal_ticket);
    assert_eq!(
        emitted,
        vec![
            CollabWindowAction::CopyTicket("typst-collab-v1:secret-ticket-data".into()),
            CollabWindowAction::LeaveSession,
        ]
    );
}
