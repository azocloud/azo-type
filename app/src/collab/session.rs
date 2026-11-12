use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc as std_mpsc;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;

use crate::collab::awareness::{CollabAwareness, PeerPresence};
use crate::collab::doc::{normalize_rel_path, CharSelection, CollabFileEntry, CollabProjectDoc};
use crate::collab::persistence::CollabPersistence;
use crate::collab::transport::{
    CollabTransport, ConnectionRoute, IrohTransport, SessionTicket, TransportEvent, WireMessage,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeerLifecycleState {
    Connecting,
    Connected,
    Reconnecting { attempt: u32 },
    Disconnected,
}

impl PeerLifecycleState {
    pub fn label(&self) -> String {
        match self {
            Self::Connecting => "Connecting...".to_string(),
            Self::Connected => "Connected".to_string(),
            Self::Reconnecting { attempt } => format!("Reconnecting (#{})", attempt),
            Self::Disconnected => "Disconnected".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConnectedPeerStatus {
    pub peer_id: String,
    pub display_name: String,
    pub state: PeerLifecycleState,
    pub route: ConnectionRoute,
}

/// Commands sent from the `egui` UI thread to the background Collaboration Actor.
#[derive(Debug, Clone)]
pub enum UiToSessionCmd {
    StartHosting {
        project_root: PathBuf,
        local_name: String,
        local_color: [u8; 3],
        local_only: bool,
    },
    JoinWithTicket {
        project_root: PathBuf,
        ticket_str: String,
        local_name: String,
        local_color: [u8; 3],
        local_only: bool,
    },
    LocalTextEdited {
        rel_path: PathBuf,
        new_text: String,
    },
    LocalFileCreated {
        rel_path: PathBuf,
        initial_text: String,
    },
    LocalFileRenamed {
        old_rel_path: PathBuf,
        new_rel_path: PathBuf,
    },
    LocalFileDeleted {
        rel_path: PathBuf,
    },
    LocalCursorUpdated {
        active_rel_path: Option<PathBuf>,
        selection: Option<CharSelection>,
    },
    LocalUndo {
        rel_path: PathBuf,
    },
    LocalRedo {
        rel_path: PathBuf,
    },
    UpdateIdentity {
        local_name: String,
        local_color: [u8; 3],
    },
    LeaveSession,
}

/// Events sent from the background Collaboration Actor to the `egui` UI thread.
#[derive(Debug, Clone)]
pub enum SessionToUiEvent {
    SessionStarted {
        is_host: bool,
        ticket_str: Option<String>,
        local_endpoint_id: String,
        files: Vec<CollabFileEntry>,
    },
    SessionStopped {
        reason: String,
    },
    PeerListChanged {
        peers: Vec<ConnectedPeerStatus>,
    },
    RemoteFilesUpdated {
        files: Vec<CollabFileEntry>,
        disk_updated_paths: Vec<PathBuf>,
        preserved_selection: Option<CharSelection>,
    },
    RemotePresenceUpdated {
        presences: Vec<PeerPresence>,
    },
    StatusNotice {
        message: String,
    },
    CollabError {
        message: String,
    },
}

/// Non-blocking handle held by the `egui` UI thread to communicate with the background
/// Collaboration Actor thread.
pub struct CollabSessionHandle {
    cmd_tx: mpsc::UnboundedSender<UiToSessionCmd>,
    ui_rx: std_mpsc::Receiver<SessionToUiEvent>,
}

impl CollabSessionHandle {
    pub fn spawn(egui_ctx: Option<egui::Context>) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let (ui_tx, ui_rx) = std_mpsc::channel();

        std::thread::Builder::new()
            .name("typst-collab-actor".to_string())
            .spawn(move || {
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(2)
                    .enable_all()
                    .build()
                    .expect("Failed to build tokio runtime for collaboration actor");

                rt.block_on(async move {
                    let mut actor = SessionActor::new(ui_tx, egui_ctx);
                    actor.run(cmd_rx).await;
                });
            })
            .expect("Failed to spawn collaboration actor thread");

        Self { cmd_tx, ui_rx }
    }

    pub fn send(&self, cmd: UiToSessionCmd) {
        let _ = self.cmd_tx.send(cmd);
    }

    pub fn try_recv(&self) -> Option<SessionToUiEvent> {
        self.ui_rx.try_recv().ok()
    }
}

struct ActiveSessionState {
    is_host: bool,
    project_root: PathBuf,
    doc: CollabProjectDoc,
    awareness: CollabAwareness,
    persistence: CollabPersistence,
    transport: Arc<dyn CollabTransport>,
    ticket: Option<SessionTicket>,
    local_only: bool,
    peers: BTreeMap<String, ConnectedPeerStatus>,
    dirty_for_snapshot: bool,
}

struct SessionActor {
    ui_tx: std_mpsc::Sender<SessionToUiEvent>,
    egui_ctx: Option<egui::Context>,
    local_name: String,
    local_color: [u8; 3],
    active_rel_path: Option<PathBuf>,
    current_selection: Option<CharSelection>,
    active: Option<ActiveSessionState>,
    transport_rx: Option<mpsc::UnboundedReceiver<TransportEvent>>,
}

impl SessionActor {
    fn new(ui_tx: std_mpsc::Sender<SessionToUiEvent>, egui_ctx: Option<egui::Context>) -> Self {
        Self {
            ui_tx,
            egui_ctx,
            local_name: "Typst Author".to_string(),
            local_color: [56, 189, 248],
            active_rel_path: Some(PathBuf::from("main.typ")),
            current_selection: None,
            active: None,
            transport_rx: None,
        }
    }

    fn emit_to_ui(&self, event: SessionToUiEvent) {
        let _ = self.ui_tx.send(event);
        if let Some(ref ctx) = self.egui_ctx {
            ctx.request_repaint();
        }
    }

    async fn run(&mut self, mut cmd_rx: mpsc::UnboundedReceiver<UiToSessionCmd>) {
        let mut snapshot_ticker = tokio::time::interval(Duration::from_secs(4));

        loop {
            tokio::select! {
                maybe_cmd = cmd_rx.recv() => {
                    let Some(cmd) = maybe_cmd else {
                        self.shutdown_active("Application closing".to_string());
                        break;
                    };
                    self.handle_ui_cmd(cmd).await;
                }
                maybe_net = async {
                    if let Some(ref mut rx) = self.transport_rx {
                        rx.recv().await
                    } else {
                        std::future::pending().await
                    }
                } => {
                    if let Some(net_ev) = maybe_net {
                        self.handle_transport_event(net_ev).await;
                    }
                }
                _ = snapshot_ticker.tick() => {
                    if let Some(ref mut state) = self.active {
                        if state.dirty_for_snapshot {
                            let _ = state.persistence.save_snapshot(&state.doc);
                            state.dirty_for_snapshot = false;
                        }
                    }
                }
            }
        }
    }

    async fn handle_ui_cmd(&mut self, cmd: UiToSessionCmd) {
        match cmd {
            UiToSessionCmd::StartHosting {
                project_root,
                local_name,
                local_color,
                local_only,
            } => {
                self.shutdown_active("Starting new hosted session".to_string());
                self.local_name = local_name.clone();
                self.local_color = local_color;

                let mut doc = CollabProjectDoc::new();
                let persistence = CollabPersistence::new(&project_root);
                if let Err(e) = persistence.load_and_reconcile_from_disk(&mut doc, true) {
                    self.emit_to_ui(SessionToUiEvent::CollabError {
                        message: format!("Failed to initialize CRDT from disk: {}", e),
                    });
                    return;
                }
                let _ = doc.drain_local_updates();

                let (ev_tx, ev_rx) = mpsc::unbounded_channel();
                match IrohTransport::bind_host(local_name, local_only, ev_tx).await {
                    Ok((transport, ticket)) => {
                        let ticket_str = ticket.encode_string().ok();
                        let local_endpoint_id = transport.local_endpoint_id();
                        let awareness = CollabAwareness::new(doc.doc().clone());
                        let files = doc.list_files();

                        self.transport_rx = Some(ev_rx);
                        self.active = Some(ActiveSessionState {
                            is_host: true,
                            project_root,
                            doc,
                            awareness,
                            persistence,
                            transport,
                            ticket: Some(ticket),
                            local_only,
                            peers: BTreeMap::new(),
                            dirty_for_snapshot: false,
                        });
                        self.broadcast_local_awareness();

                        self.emit_to_ui(SessionToUiEvent::SessionStarted {
                            is_host: true,
                            ticket_str,
                            local_endpoint_id,
                            files,
                        });
                    }
                    Err(err) => {
                        self.emit_to_ui(SessionToUiEvent::CollabError { message: err });
                    }
                }
            }

            UiToSessionCmd::JoinWithTicket {
                project_root,
                ticket_str,
                local_name,
                local_color,
                local_only,
            } => {
                self.shutdown_active("Joining session".to_string());
                self.local_name = local_name.clone();
                self.local_color = local_color;

                let ticket = match SessionTicket::decode_str(&ticket_str) {
                    Ok(t) => t,
                    Err(err) => {
                        self.emit_to_ui(SessionToUiEvent::CollabError { message: err });
                        return;
                    }
                };

                let mut doc = CollabProjectDoc::new();
                let persistence = CollabPersistence::new(&project_root);
                // Load any prior CRDT snapshot for offline merge, but let the host's files populate first
                let _ = persistence.load_and_reconcile_from_disk(&mut doc, false);
                let _ = doc.drain_local_updates();

                let (ev_tx, ev_rx) = mpsc::unbounded_channel();
                match IrohTransport::connect_with_ticket(
                    ticket.clone(),
                    local_name,
                    local_only,
                    ev_tx,
                )
                .await
                {
                    Ok(transport) => {
                        let local_endpoint_id = transport.local_endpoint_id();
                        let awareness = CollabAwareness::new(doc.doc().clone());
                        let files = doc.list_files();

                        self.transport_rx = Some(ev_rx);
                        self.active = Some(ActiveSessionState {
                            is_host: false,
                            project_root,
                            doc,
                            awareness,
                            persistence,
                            transport,
                            ticket: Some(ticket),
                            local_only,
                            peers: BTreeMap::new(),
                            dirty_for_snapshot: false,
                        });
                        self.broadcast_local_awareness();

                        self.emit_to_ui(SessionToUiEvent::SessionStarted {
                            is_host: false,
                            ticket_str: None,
                            local_endpoint_id,
                            files,
                        });
                    }
                    Err(err) => {
                        self.emit_to_ui(SessionToUiEvent::CollabError { message: err });
                    }
                }
            }

            UiToSessionCmd::LocalTextEdited { rel_path, new_text } => {
                let Some(ref mut state) = self.active else {
                    return;
                };
                let fid = match state.doc.find_file_id_by_path(&rel_path) {
                    Some(id) => id,
                    None => state
                        .doc
                        .create_file(None, &rel_path.to_string_lossy(), &new_text),
                };
                if state.doc.apply_local_text_edit(&fid, &new_text) {
                    state.dirty_for_snapshot = true;
                    Self::flush_outgoing_doc_updates(state);
                }
            }

            UiToSessionCmd::LocalFileCreated {
                rel_path,
                initial_text,
            } => {
                let Some(ref mut state) = self.active else {
                    return;
                };
                let norm = normalize_rel_path(&rel_path.to_string_lossy());
                if state.doc.find_file_id_by_path(Path::new(&norm)).is_none() {
                    state.doc.create_file(None, &norm, &initial_text);
                    state.dirty_for_snapshot = true;
                    Self::flush_outgoing_doc_updates(state);
                }
            }

            UiToSessionCmd::LocalFileRenamed {
                old_rel_path,
                new_rel_path,
            } => {
                let Some(ref mut state) = self.active else {
                    return;
                };
                if let Some(fid) = state.doc.find_file_id_by_path(&old_rel_path) {
                    if state.doc.rename_file(&fid, &new_rel_path.to_string_lossy()) {
                        state.dirty_for_snapshot = true;
                        Self::flush_outgoing_doc_updates(state);
                    }
                }
            }

            UiToSessionCmd::LocalFileDeleted { rel_path } => {
                let Some(ref mut state) = self.active else {
                    return;
                };
                if let Some(fid) = state.doc.find_file_id_by_path(&rel_path) {
                    if state.doc.delete_file(&fid) {
                        state.dirty_for_snapshot = true;
                        Self::flush_outgoing_doc_updates(state);
                    }
                }
            }

            UiToSessionCmd::LocalCursorUpdated {
                active_rel_path,
                selection,
            } => {
                self.active_rel_path = active_rel_path;
                self.current_selection = selection;
                self.broadcast_local_awareness();
            }

            UiToSessionCmd::LocalUndo { rel_path } => {
                let mut emit_files = None;
                if let Some(ref mut state) = self.active {
                    if let Some(fid) = state.doc.find_file_id_by_path(&rel_path) {
                        if state.doc.undo_file(&fid) {
                            state.dirty_for_snapshot = true;
                            Self::flush_outgoing_doc_updates(state);
                            let files = state.doc.list_files();
                            let disk_updated = state
                                .persistence
                                .flush_crdt_files_to_disk(&files)
                                .unwrap_or_default();
                            emit_files = Some((files, disk_updated));
                        }
                    }
                }
                if let Some((files, disk_updated_paths)) = emit_files {
                    self.emit_to_ui(SessionToUiEvent::RemoteFilesUpdated {
                        files,
                        disk_updated_paths,
                        preserved_selection: self.current_selection,
                    });
                }
            }

            UiToSessionCmd::LocalRedo { rel_path } => {
                let mut emit_files = None;
                if let Some(ref mut state) = self.active {
                    if let Some(fid) = state.doc.find_file_id_by_path(&rel_path) {
                        if state.doc.redo_file(&fid) {
                            state.dirty_for_snapshot = true;
                            Self::flush_outgoing_doc_updates(state);
                            let files = state.doc.list_files();
                            let disk_updated = state
                                .persistence
                                .flush_crdt_files_to_disk(&files)
                                .unwrap_or_default();
                            emit_files = Some((files, disk_updated));
                        }
                    }
                }
                if let Some((files, disk_updated_paths)) = emit_files {
                    self.emit_to_ui(SessionToUiEvent::RemoteFilesUpdated {
                        files,
                        disk_updated_paths,
                        preserved_selection: self.current_selection,
                    });
                }
            }

            UiToSessionCmd::UpdateIdentity {
                local_name,
                local_color,
            } => {
                self.local_name = local_name;
                self.local_color = local_color;
                self.broadcast_local_awareness();
            }

            UiToSessionCmd::LeaveSession => {
                self.shutdown_active("Left collaboration session".to_string());
            }
        }
    }

    async fn handle_transport_event(&mut self, event: TransportEvent) {
        match event {
            TransportEvent::PeerConnected {
                peer_id,
                peer_name,
                route,
            } => {
                let mut peers_list = Vec::new();
                if let Some(ref mut state) = self.active {
                    state.peers.insert(
                        peer_id.clone(),
                        ConnectedPeerStatus {
                            peer_id: peer_id.clone(),
                            display_name: peer_name,
                            state: PeerLifecycleState::Connected,
                            route,
                        },
                    );
                    peers_list = state.peers.values().cloned().collect();

                    // Initiate Yrs SyncStep1: send our StateVector and full current state diff
                    let sv = state.doc.encode_state_vector_v1();
                    state.transport.send_to_peer(
                        &peer_id,
                        WireMessage::SyncStep1 {
                            state_vector_v1: sv,
                        },
                    );
                }
                self.broadcast_local_awareness();
                self.emit_to_ui(SessionToUiEvent::PeerListChanged { peers: peers_list });
            }

            TransportEvent::PeerRouteChanged { peer_id, route } => {
                if let Some(ref mut state) = self.active {
                    if let Some(peer) = state.peers.get_mut(&peer_id) {
                        peer.route = route;
                    }
                    let peers = state.peers.values().cloned().collect();
                    self.emit_to_ui(SessionToUiEvent::PeerListChanged { peers });
                }
            }

            TransportEvent::MessageReceived {
                from_peer_id,
                message,
            } => {
                self.handle_wire_message(&from_peer_id, message);
            }

            TransportEvent::PeerDisconnected { peer_id, reason } => {
                let mut should_try_reconnect = None;
                let mut peers_snapshot = Vec::new();
                let mut presences_snapshot = Vec::new();

                if let Some(ref mut state) = self.active {
                    if let Some(p) = state.peers.get_mut(&peer_id) {
                        p.state = PeerLifecycleState::Disconnected;
                    }
                    state.awareness.remove_peer_by_endpoint(&peer_id);
                    peers_snapshot = state.peers.values().cloned().collect();
                    presences_snapshot = state.awareness.remote_peers();

                    if !state.is_host {
                        if let Some(ref ticket) = state.ticket {
                            should_try_reconnect = Some((
                                ticket.clone(),
                                state.local_only,
                                state.project_root.clone(),
                            ));
                        }
                    }
                }

                self.emit_to_ui(SessionToUiEvent::PeerListChanged {
                    peers: peers_snapshot,
                });
                self.emit_to_ui(SessionToUiEvent::RemotePresenceUpdated {
                    presences: presences_snapshot,
                });
                self.emit_to_ui(SessionToUiEvent::StatusNotice {
                    message: format!("Peer disconnected: {}", reason),
                });

                // Automatic reconnect with exponential backoff if we are a client and lost connection
                if let Some((ticket, local_only, _root)) = should_try_reconnect {
                    self.try_auto_reconnect(ticket, local_only).await;
                }
            }

            TransportEvent::TransportError { message } => {
                self.emit_to_ui(SessionToUiEvent::CollabError { message });
            }
        }
    }

    fn handle_wire_message(&mut self, from_peer_id: &str, message: WireMessage) {
        match message {
            WireMessage::SyncStep1 { state_vector_v1 } => {
                if let Some(ref mut state) = self.active {
                    match state.doc.encode_diff_v1(&state_vector_v1) {
                        Ok(diff_update) => {
                            state.transport.send_to_peer(
                                from_peer_id,
                                WireMessage::SyncStep2 {
                                    update_v1: diff_update,
                                },
                            );
                        }
                        Err(err) => {
                            state.transport.disconnect_peer(from_peer_id);
                            self.emit_to_ui(SessionToUiEvent::CollabError {
                                message: format!(
                                    "Disconnected peer with invalid SyncStep1: {}",
                                    err
                                ),
                            });
                        }
                    }
                }
            }

            WireMessage::SyncStep2 { update_v1 } | WireMessage::CrdtUpdate { update_v1, .. } => {
                self.apply_incoming_crdt_update(from_peer_id, &update_v1);
            }

            WireMessage::AwarenessUpdate { awareness_v1 } => {
                if let Some(ref mut state) = self.active {
                    if state
                        .awareness
                        .apply_remote_awareness_bytes(&awareness_v1)
                        .is_ok()
                    {
                        let presences = state.awareness.remote_peers();
                        self.emit_to_ui(SessionToUiEvent::RemotePresenceUpdated { presences });
                    }
                }
            }

            WireMessage::SessionEnded { reason } => {
                self.shutdown_active(format!("Host ended session: {}", reason));
            }

            _ => {}
        }
    }

    fn apply_incoming_crdt_update(&mut self, from_peer_id: &str, update_bytes: &[u8]) {
        let mut emit_event = None;
        let mut disconnect_bad_peer = None;

        if let Some(ref mut state) = self.active {
            let active_fid = self
                .active_rel_path
                .as_ref()
                .and_then(|p| state.doc.find_file_id_by_path(p));

            match state.doc.apply_remote_update_preserving_cursor(
                update_bytes,
                active_fid.as_deref(),
                self.current_selection,
            ) {
                Ok(new_sel) => {
                    self.current_selection = new_sel;
                    state.dirty_for_snapshot = true;
                    let files = state.doc.list_files();
                    let disk_updated_paths = state
                        .persistence
                        .flush_crdt_files_to_disk(&files)
                        .unwrap_or_default();
                    emit_event = Some(SessionToUiEvent::RemoteFilesUpdated {
                        files,
                        disk_updated_paths,
                        preserved_selection: new_sel,
                    });
                }
                Err(err) => {
                    state.transport.disconnect_peer(from_peer_id);
                    disconnect_bad_peer = Some(err);
                }
            }
        }

        if let Some(ev) = emit_event {
            self.emit_to_ui(ev);
        }
        if let Some(err) = disconnect_bad_peer {
            self.emit_to_ui(SessionToUiEvent::CollabError {
                message: format!("Disconnected peer sending malformed update: {}", err),
            });
        }
    }

    async fn try_auto_reconnect(&mut self, ticket: SessionTicket, local_only: bool) {
        for attempt in 1..=3 {
            let backoff = Duration::from_millis(600 * (1 << (attempt - 1)));
            tokio::time::sleep(backoff).await;

            // Only reconnect if we are still in an active client session
            if self.active.as_ref().map(|s| s.is_host).unwrap_or(true) {
                return;
            }

            self.emit_to_ui(SessionToUiEvent::StatusNotice {
                message: format!("Reconnecting to session host (attempt {}/3)...", attempt),
            });

            let (ev_tx, ev_rx) = mpsc::unbounded_channel();
            if let Ok(new_transport) = IrohTransport::connect_with_ticket(
                ticket.clone(),
                self.local_name.clone(),
                local_only,
                ev_tx,
            )
            .await
            {
                if let Some(ref mut state) = self.active {
                    state.transport = new_transport;
                }
                self.transport_rx = Some(ev_rx);
                self.broadcast_local_awareness();
                self.emit_to_ui(SessionToUiEvent::StatusNotice {
                    message: "Reconnected to P2P session and merged offline edits.".to_string(),
                });
                return;
            }
        }
    }

    fn flush_outgoing_doc_updates(state: &ActiveSessionState) {
        let local_id = state.transport.local_endpoint_id();
        for update_v1 in state.doc.drain_local_updates() {
            state.transport.broadcast(WireMessage::CrdtUpdate {
                update_v1,
                origin_peer: local_id.clone(),
            });
        }
    }

    fn broadcast_local_awareness(&mut self) {
        let Some(ref mut state) = self.active else {
            return;
        };

        let active_path_str = self
            .active_rel_path
            .as_ref()
            .map(|p| normalize_rel_path(&p.to_string_lossy()));
        let active_file_id = self
            .active_rel_path
            .as_ref()
            .and_then(|p| state.doc.find_file_id_by_path(p));

        let presence = PeerPresence {
            client_id: state.awareness.client_id(),
            endpoint_id: state.transport.local_endpoint_id(),
            name: self.local_name.clone(),
            color_rgb: self.local_color,
            active_file_id,
            active_path: active_path_str,
            cursor_char: self.current_selection.map(|s| s.primary_char),
            anchor_char: self.current_selection.map(|s| s.secondary_char),
        };

        if let Ok(awareness_v1) = state.awareness.set_local_presence(&presence) {
            state
                .transport
                .broadcast(WireMessage::AwarenessUpdate { awareness_v1 });
        }
    }

    fn shutdown_active(&mut self, reason: String) {
        if let Some(state) = self.active.take() {
            let _ = state.persistence.save_snapshot(&state.doc);
            let files = state.doc.list_files();
            let _ = state.persistence.flush_crdt_files_to_disk(&files);
            state.transport.shutdown();
            self.transport_rx = None;
            self.emit_to_ui(SessionToUiEvent::SessionStopped { reason });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

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
            if let Some(SessionToUiEvent::RemoteFilesUpdated { files, .. }) = join_handle.try_recv()
            {
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
            if let Some(SessionToUiEvent::RemoteFilesUpdated { files, .. }) = host_handle.try_recv()
            {
                if files
                    .iter()
                    .any(|f| f.resolved_path == "second.typ" && f.content.contains("Edited by Bob"))
                {
                    host_got_second = true;
                }
            }
            if let Some(SessionToUiEvent::RemoteFilesUpdated { files, .. }) = join_handle.try_recv()
            {
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
}
