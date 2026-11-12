use std::collections::HashMap;
use std::fmt;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use iroh::endpoint::{presets, Connection, RecvStream, SendStream};
use iroh::{Endpoint, EndpointAddr, EndpointId, RelayMode, RelayUrl, TransportAddr};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, Mutex};

pub const COLLAB_ALPN: &[u8] = b"typst-studio/collab/v1";
pub const COLLAB_PROTOCOL_VERSION: u32 = 1;
pub const MAX_WIRE_FRAME_BYTES: usize = 8 * 1024 * 1024; // 8 MiB hard limit
pub const TICKET_PREFIX: &str = "typst-collab-v1:";

/// Connection route reported per peer (`Direct` UDP holepunched IP vs `Relay` server).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionRoute {
    Direct { addr: String, rtt_ms: u64 },
    Relay { url: String, rtt_ms: u64 },
    Unknown,
}

impl ConnectionRoute {
    pub fn badge_label(&self) -> String {
        match self {
            Self::Direct { rtt_ms, .. } => format!("Direct ({} ms)", rtt_ms),
            Self::Relay { rtt_ms, .. } => format!("Relay ({} ms)", rtt_ms),
            Self::Unknown => "Negotiating".to_string(),
        }
    }

    pub fn is_direct(&self) -> bool {
        matches!(self, Self::Direct { .. })
    }
}

/// Serializable representation of an `iroh::EndpointAddr`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerEndpointInfo {
    pub endpoint_id: String,
    pub ip_addrs: Vec<SocketAddr>,
    pub relay_urls: Vec<String>,
}

impl PeerEndpointInfo {
    pub fn from_endpoint_addr(addr: &EndpointAddr) -> Self {
        Self {
            endpoint_id: addr.id.to_string(),
            ip_addrs: addr.ip_addrs().copied().collect(),
            relay_urls: addr.relay_urls().map(|u| u.to_string()).collect(),
        }
    }

    pub fn to_endpoint_addr(&self) -> Result<EndpointAddr, String> {
        let id: EndpointId = self
            .endpoint_id
            .parse()
            .map_err(|e| format!("Invalid endpoint ID: {}", e))?;
        let addrs = self.ip_addrs.iter().copied().map(TransportAddr::Ip).chain(
            self.relay_urls
                .iter()
                .filter_map(|u| u.parse::<RelayUrl>().ok())
                .map(TransportAddr::Relay),
        );
        Ok(EndpointAddr::from_parts(id, addrs))
    }
}

/// Invite ticket containing the protocol version, 256-bit shared secret, and host address.
///
/// Treated like a password: `Debug` is redacted so the ticket secret never appears in logs.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionTicket {
    pub protocol_version: u32,
    pub session_secret: [u8; 32],
    pub host: PeerEndpointInfo,
}

impl fmt::Debug for SessionTicket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SessionTicket")
            .field("protocol_version", &self.protocol_version)
            .field("session_secret", &"<REDACTED>")
            .field("host_endpoint_id", &self.host.endpoint_id)
            .finish()
    }
}

impl SessionTicket {
    pub fn generate(host_addr: &EndpointAddr) -> Self {
        let mut secret = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut secret);
        Self {
            protocol_version: COLLAB_PROTOCOL_VERSION,
            session_secret: secret,
            host: PeerEndpointInfo::from_endpoint_addr(host_addr),
        }
    }

    pub fn encode_string(&self) -> Result<String, String> {
        let json = serde_json::to_vec(self).map_err(|e| e.to_string())?;
        Ok(format!("{}{}", TICKET_PREFIX, URL_SAFE_NO_PAD.encode(json)))
    }

    pub fn decode_str(input: &str) -> Result<Self, String> {
        let trimmed = input.trim();
        let payload = trimmed
            .strip_prefix(TICKET_PREFIX)
            .ok_or_else(|| "Invalid ticket prefix (expected 'typst-collab-v1:')".to_string())?;
        let raw = URL_SAFE_NO_PAD
            .decode(payload)
            .map_err(|_| "Invalid ticket Base64 encoding".to_string())?;
        let ticket: Self = serde_json::from_slice(&raw)
            .map_err(|_| "Malformed ticket payload structure".to_string())?;
        if ticket.protocol_version != COLLAB_PROTOCOL_VERSION {
            return Err(format!(
                "Incompatible ticket protocol version {} (expected {})",
                ticket.protocol_version, COLLAB_PROTOCOL_VERSION
            ));
        }
        Ok(ticket)
    }
}

/// Constant-time byte slice equality check for `session_secret`.
pub fn constant_time_eq(a: &[u8; 32], b: &[u8; 32]) -> bool {
    let mut diff = 0u8;
    for i in 0..32 {
        diff |= a[i] ^ b[i];
    }
    diff == 0
}

/// Protocol messages exchanged over the P2P transport.
#[derive(Clone, Serialize, Deserialize)]
pub enum WireMessage {
    HandshakeHello {
        protocol_version: u32,
        session_secret: [u8; 32],
        peer_name: String,
        peer_endpoint: PeerEndpointInfo,
    },
    HandshakeAck {
        protocol_version: u32,
        accepted: bool,
        reason: Option<String>,
        host_name: String,
        known_peers: Vec<PeerEndpointInfo>,
    },
    SyncStep1 {
        state_vector_v1: Vec<u8>,
    },
    SyncStep2 {
        update_v1: Vec<u8>,
    },
    CrdtUpdate {
        update_v1: Vec<u8>,
        origin_peer: String,
    },
    AwarenessUpdate {
        awareness_v1: Vec<u8>,
    },
    PeerListAnnounce {
        peers: Vec<PeerEndpointInfo>,
    },
    SessionEnded {
        reason: String,
    },
}

impl fmt::Debug for WireMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HandshakeHello {
                protocol_version,
                peer_name,
                ..
            } => f
                .debug_struct("HandshakeHello")
                .field("protocol_version", protocol_version)
                .field("peer_name", peer_name)
                .field("session_secret", &"<REDACTED>")
                .finish(),
            Self::HandshakeAck {
                protocol_version,
                accepted,
                reason,
                ..
            } => f
                .debug_struct("HandshakeAck")
                .field("protocol_version", protocol_version)
                .field("accepted", accepted)
                .field("reason", reason)
                .finish(),
            Self::SyncStep1 { state_vector_v1 } => f
                .debug_struct("SyncStep1")
                .field("bytes", &state_vector_v1.len())
                .finish(),
            Self::SyncStep2 { update_v1 } => f
                .debug_struct("SyncStep2")
                .field("bytes", &update_v1.len())
                .finish(),
            Self::CrdtUpdate {
                update_v1,
                origin_peer,
            } => f
                .debug_struct("CrdtUpdate")
                .field("bytes", &update_v1.len())
                .field("origin_peer", origin_peer)
                .finish(),
            Self::AwarenessUpdate { awareness_v1 } => f
                .debug_struct("AwarenessUpdate")
                .field("bytes", &awareness_v1.len())
                .finish(),
            Self::PeerListAnnounce { peers } => f
                .debug_struct("PeerListAnnounce")
                .field("count", &peers.len())
                .finish(),
            Self::SessionEnded { reason } => f
                .debug_struct("SessionEnded")
                .field("reason", reason)
                .finish(),
        }
    }
}

/// Events emitted by any `CollabTransport` implementation to the CRDT session actor.
#[derive(Debug, Clone)]
pub enum TransportEvent {
    PeerConnected {
        peer_id: String,
        peer_name: String,
        route: ConnectionRoute,
    },
    PeerRouteChanged {
        peer_id: String,
        route: ConnectionRoute,
    },
    MessageReceived {
        from_peer_id: String,
        message: WireMessage,
    },
    PeerDisconnected {
        peer_id: String,
        reason: String,
    },
    TransportError {
        message: String,
    },
}

/// Narrow transport trait decoupling the CRDT synchronization logic from `iroh`.
pub trait CollabTransport: Send + Sync {
    fn local_endpoint_id(&self) -> String;
    fn send_to_peer(&self, peer_id: &str, message: WireMessage);
    fn broadcast(&self, message: WireMessage);
    fn broadcast_except(&self, excluded_peer_id: &str, message: WireMessage);
    fn disconnect_peer(&self, peer_id: &str);
    fn shutdown(&self);
}

/// Inspects an active `iroh::endpoint::Connection` to determine if the selected path is Direct IP or Relay.
pub fn inspect_connection_route(conn: &Connection) -> ConnectionRoute {
    let paths = conn.paths();
    let selected = paths
        .iter()
        .find(|p| p.is_selected())
        .or_else(|| paths.iter().next());

    let Some(path) = selected else {
        return ConnectionRoute::Unknown;
    };

    let rtt_ms = path.rtt().as_millis() as u64;
    if path.is_ip() {
        ConnectionRoute::Direct {
            addr: format!("{:?}", path.remote_addr()),
            rtt_ms,
        }
    } else if path.is_relay() {
        ConnectionRoute::Relay {
            url: format!("{:?}", path.remote_addr()),
            rtt_ms,
        }
    } else {
        ConnectionRoute::Unknown
    }
}

pub async fn write_framed_message(send: &mut SendStream, msg: &WireMessage) -> Result<(), String> {
    let bytes = serde_json::to_vec(msg).map_err(|e| format!("Serialize error: {}", e))?;
    if bytes.len() > MAX_WIRE_FRAME_BYTES {
        return Err(format!(
            "Message size {} exceeds limit {}",
            bytes.len(),
            MAX_WIRE_FRAME_BYTES
        ));
    }
    let len_be = (bytes.len() as u32).to_be_bytes();
    send.write_all(&len_be)
        .await
        .map_err(|e| format!("Stream write length failed: {}", e))?;
    send.write_all(&bytes)
        .await
        .map_err(|e| format!("Stream write payload failed: {}", e))?;
    Ok(())
}

pub async fn read_framed_message(recv: &mut RecvStream) -> Result<Option<WireMessage>, String> {
    let mut len_buf = [0u8; 4];
    match recv.read_exact(&mut len_buf).await {
        Ok(()) => {}
        Err(iroh::endpoint::ReadExactError::FinishedEarly(0)) => return Ok(None),
        Err(e) => return Err(format!("Stream read length error: {}", e)),
    }

    let frame_len = u32::from_be_bytes(len_buf) as usize;
    if frame_len == 0 || frame_len > MAX_WIRE_FRAME_BYTES {
        return Err(format!(
            "Rejected untrusted frame of length {} bytes (max {})",
            frame_len, MAX_WIRE_FRAME_BYTES
        ));
    }

    let mut payload = vec![0u8; frame_len];
    recv.read_exact(&mut payload)
        .await
        .map_err(|e| format!("Stream read payload error: {}", e))?;

    let msg: WireMessage = serde_json::from_slice(&payload)
        .map_err(|e| format!("Malformed wire message JSON: {}", e))?;
    Ok(Some(msg))
}

struct PeerHandle {
    tx: mpsc::UnboundedSender<WireMessage>,
    conn: Connection,
    endpoint_info: PeerEndpointInfo,
}

/// `iroh` v1.4.1 implementation of `CollabTransport`.
pub struct IrohTransport {
    endpoint: Endpoint,
    local_id: String,
    peers: Arc<Mutex<HashMap<String, PeerHandle>>>,
}

impl IrohTransport {
    /// Starts a host P2P endpoint, generates a fresh `SessionTicket`, and spawns the accept loop.
    pub async fn bind_host(
        local_name: String,
        local_only: bool,
        event_tx: mpsc::UnboundedSender<TransportEvent>,
    ) -> Result<(Arc<Self>, SessionTicket), String> {
        let relay_mode = if local_only {
            RelayMode::Disabled
        } else {
            RelayMode::Default
        };

        let endpoint = Endpoint::builder(presets::N0)
            .alpns(vec![COLLAB_ALPN.to_vec()])
            .relay_mode(relay_mode)
            .bind()
            .await
            .map_err(|e| format!("Failed to bind iroh endpoint: {}", e))?;

        if !local_only {
            // Wait up to 2.5 seconds for home relay registration; proceed with direct IPs if offline
            let _ = tokio::time::timeout(Duration::from_millis(2500), endpoint.online()).await;
        }

        let addr = endpoint.addr();
        let ticket = SessionTicket::generate(&addr);
        let secret = ticket.session_secret;
        let local_id = endpoint.id().to_string();
        let peers: Arc<Mutex<HashMap<String, PeerHandle>>> = Arc::new(Mutex::new(HashMap::new()));

        let transport = Arc::new(Self {
            endpoint: endpoint.clone(),
            local_id,
            peers: Arc::clone(&peers),
        });

        // Spawn incoming connection accept loop
        let peers_accept = Arc::clone(&peers);
        let endpoint_accept = endpoint.clone();
        tokio::spawn(async move {
            while let Some(incoming) = endpoint_accept.accept().await {
                let peers_conn = Arc::clone(&peers_accept);
                let ev_tx = event_tx.clone();
                let host_name = local_name.clone();

                tokio::spawn(async move {
                    let connecting = match incoming.accept() {
                        Ok(c) => c,
                        Err(_) => return,
                    };
                    let conn = match connecting.await {
                        Ok(c) => c,
                        Err(_) => return,
                    };
                    Self::handle_incoming_peer_connection(
                        conn, secret, host_name, peers_conn, ev_tx,
                    )
                    .await;
                });
            }
        });

        Ok((transport, ticket))
    }

    /// Joins an existing host session using a `SessionTicket`.
    pub async fn connect_with_ticket(
        ticket: SessionTicket,
        local_name: String,
        local_only: bool,
        event_tx: mpsc::UnboundedSender<TransportEvent>,
    ) -> Result<Arc<Self>, String> {
        if ticket.protocol_version != COLLAB_PROTOCOL_VERSION {
            return Err(format!(
                "Incompatible protocol version {} (expected {})",
                ticket.protocol_version, COLLAB_PROTOCOL_VERSION
            ));
        }

        let relay_mode = if local_only {
            RelayMode::Disabled
        } else {
            RelayMode::Default
        };

        let endpoint = Endpoint::builder(presets::N0)
            .alpns(vec![COLLAB_ALPN.to_vec()])
            .relay_mode(relay_mode)
            .bind()
            .await
            .map_err(|e| format!("Failed to bind iroh endpoint: {}", e))?;

        if !local_only {
            let _ = tokio::time::timeout(Duration::from_millis(2500), endpoint.online()).await;
        }

        let local_id = endpoint.id().to_string();
        let local_info = PeerEndpointInfo::from_endpoint_addr(&endpoint.addr());
        let host_addr = ticket.host.to_endpoint_addr()?;

        let conn = tokio::time::timeout(
            Duration::from_secs(10),
            endpoint.connect(host_addr, COLLAB_ALPN),
        )
        .await
        .map_err(|_| "Connection timed out while dialing host".to_string())?
        .map_err(|e| format!("Failed to connect to host: {}", e))?;

        let (mut send, mut recv) = conn
            .open_bi()
            .await
            .map_err(|e| format!("Failed to open bi-stream: {}", e))?;

        // Send authenticated HandshakeHello
        let hello = WireMessage::HandshakeHello {
            protocol_version: COLLAB_PROTOCOL_VERSION,
            session_secret: ticket.session_secret,
            peer_name: local_name,
            peer_endpoint: local_info,
        };
        write_framed_message(&mut send, &hello).await?;

        // Wait for HandshakeAck
        let ack = tokio::time::timeout(Duration::from_secs(6), read_framed_message(&mut recv))
            .await
            .map_err(|_| "Handshake timed out".to_string())??
            .ok_or_else(|| "Host closed stream during handshake".to_string())?;

        let (host_name, _known_peers) = match ack {
            WireMessage::HandshakeAck {
                protocol_version,
                accepted,
                reason,
                host_name,
                known_peers,
            } => {
                if protocol_version != COLLAB_PROTOCOL_VERSION {
                    conn.close(1u8.into(), b"version mismatch");
                    return Err(format!(
                        "Host uses incompatible protocol version {} (expected {})",
                        protocol_version, COLLAB_PROTOCOL_VERSION
                    ));
                }
                if !accepted {
                    conn.close(1u8.into(), b"rejected");
                    return Err(reason.unwrap_or_else(|| "Handshake rejected by host".to_string()));
                }
                (host_name, known_peers)
            }
            _ => {
                conn.close(1u8.into(), b"unexpected handshake response");
                return Err("Unexpected handshake response from host".to_string());
            }
        };

        let host_peer_id = conn.remote_id().to_string();
        let route = inspect_connection_route(&conn);
        let peers: Arc<Mutex<HashMap<String, PeerHandle>>> = Arc::new(Mutex::new(HashMap::new()));

        let (peer_tx, peer_rx) = mpsc::unbounded_channel();
        {
            let mut map = peers.lock().await;
            map.insert(
                host_peer_id.clone(),
                PeerHandle {
                    tx: peer_tx,
                    conn: conn.clone(),
                    endpoint_info: ticket.host.clone(),
                },
            );
        }

        let _ = event_tx.send(TransportEvent::PeerConnected {
            peer_id: host_peer_id.clone(),
            peer_name: host_name,
            route,
        });

        Self::spawn_peer_io_loops(
            host_peer_id,
            conn,
            send,
            recv,
            peer_rx,
            Arc::clone(&peers),
            event_tx,
            false,
        );

        Ok(Arc::new(Self {
            endpoint,
            local_id,
            peers,
        }))
    }

    async fn handle_incoming_peer_connection(
        conn: Connection,
        expected_secret: [u8; 32],
        host_name: String,
        peers: Arc<Mutex<HashMap<String, PeerHandle>>>,
        event_tx: mpsc::UnboundedSender<TransportEvent>,
    ) {
        let Ok((mut send, mut recv)) = conn.accept_bi().await else {
            return;
        };

        let Ok(Ok(Some(first_msg))) =
            tokio::time::timeout(Duration::from_secs(6), read_framed_message(&mut recv)).await
        else {
            conn.close(1u8.into(), b"handshake timeout");
            return;
        };

        let WireMessage::HandshakeHello {
            protocol_version,
            session_secret,
            peer_name,
            peer_endpoint,
        } = first_msg
        else {
            conn.close(1u8.into(), b"expected HandshakeHello");
            return;
        };

        if protocol_version != COLLAB_PROTOCOL_VERSION {
            let _ = write_framed_message(
                &mut send,
                &WireMessage::HandshakeAck {
                    protocol_version: COLLAB_PROTOCOL_VERSION,
                    accepted: false,
                    reason: Some(format!(
                        "Protocol version mismatch: client={}, host={}",
                        protocol_version, COLLAB_PROTOCOL_VERSION
                    )),
                    host_name,
                    known_peers: Vec::new(),
                },
            )
            .await;
            let _ = send.finish();
            conn.close(1u8.into(), b"version mismatch");
            return;
        }

        if !constant_time_eq(&session_secret, &expected_secret) {
            let _ = write_framed_message(
                &mut send,
                &WireMessage::HandshakeAck {
                    protocol_version: COLLAB_PROTOCOL_VERSION,
                    accepted: false,
                    reason: Some("Invalid session secret in invite ticket".to_string()),
                    host_name,
                    known_peers: Vec::new(),
                },
            )
            .await;
            let _ = send.finish();
            conn.close(1u8.into(), b"invalid secret");
            let _ = event_tx.send(TransportEvent::TransportError {
                message: "Rejected connection attempt with invalid ticket secret".to_string(),
            });
            return;
        }

        let peer_id = conn.remote_id().to_string();
        let known_peers: Vec<PeerEndpointInfo> = {
            let map = peers.lock().await;
            map.values().map(|h| h.endpoint_info.clone()).collect()
        };

        if write_framed_message(
            &mut send,
            &WireMessage::HandshakeAck {
                protocol_version: COLLAB_PROTOCOL_VERSION,
                accepted: true,
                reason: None,
                host_name,
                known_peers,
            },
        )
        .await
        .is_err()
        {
            return;
        }

        let (peer_tx, peer_rx) = mpsc::unbounded_channel();
        {
            let mut map = peers.lock().await;
            map.insert(
                peer_id.clone(),
                PeerHandle {
                    tx: peer_tx,
                    conn: conn.clone(),
                    endpoint_info: peer_endpoint,
                },
            );
        }

        let route = inspect_connection_route(&conn);
        let _ = event_tx.send(TransportEvent::PeerConnected {
            peer_id: peer_id.clone(),
            peer_name,
            route,
        });

        Self::spawn_peer_io_loops(peer_id, conn, send, recv, peer_rx, peers, event_tx, true);
    }

    #[allow(clippy::too_many_arguments)]
    fn spawn_peer_io_loops(
        peer_id: String,
        conn: Connection,
        mut send: SendStream,
        mut recv: RecvStream,
        mut peer_rx: mpsc::UnboundedReceiver<WireMessage>,
        peers: Arc<Mutex<HashMap<String, PeerHandle>>>,
        event_tx: mpsc::UnboundedSender<TransportEvent>,
        is_host: bool,
    ) {
        // Writer + periodic path route monitor task
        let conn_writer = conn.clone();
        let peer_id_writer = peer_id.clone();
        let ev_tx_writer = event_tx.clone();
        tokio::spawn(async move {
            let mut route_ticker = tokio::time::interval(Duration::from_secs(3));
            let mut last_route = inspect_connection_route(&conn_writer);
            loop {
                tokio::select! {
                    maybe_msg = peer_rx.recv() => {
                        let Some(msg) = maybe_msg else {
                            let _ = send.finish();
                            break;
                        };
                        if write_framed_message(&mut send, &msg).await.is_err() {
                            break;
                        }
                    }
                    _ = route_ticker.tick() => {
                        let new_route = inspect_connection_route(&conn_writer);
                        if new_route != last_route {
                            last_route = new_route.clone();
                            let _ = ev_tx_writer.send(TransportEvent::PeerRouteChanged {
                                peer_id: peer_id_writer.clone(),
                                route: new_route,
                            });
                        }
                    }
                }
            }
        });

        // Reader task
        tokio::spawn(async move {
            let disconnect_reason = loop {
                match read_framed_message(&mut recv).await {
                    Ok(Some(msg)) => {
                        // If we are the host in a multi-peer session (>2 participants), forward
                        // CrdtUpdate and AwarenessUpdate to all other connected peers as well!
                        if is_host
                            && matches!(
                                msg,
                                WireMessage::CrdtUpdate { .. }
                                    | WireMessage::AwarenessUpdate { .. }
                            )
                        {
                            let map = peers.lock().await;
                            for (other_id, handle) in map.iter() {
                                if other_id != &peer_id {
                                    let _ = handle.tx.send(msg.clone());
                                }
                            }
                        }

                        let _ = event_tx.send(TransportEvent::MessageReceived {
                            from_peer_id: peer_id.clone(),
                            message: msg,
                        });
                    }
                    Ok(None) => break "Peer closed connection".to_string(),
                    Err(err) => {
                        conn.close(2u8.into(), b"protocol violation");
                        break format!("Disconnected peer due to protocol error: {}", err);
                    }
                }
            };

            {
                let mut map = peers.lock().await;
                map.remove(&peer_id);
            }
            let _ = event_tx.send(TransportEvent::PeerDisconnected {
                peer_id,
                reason: disconnect_reason,
            });
        });
    }
}

impl CollabTransport for IrohTransport {
    fn local_endpoint_id(&self) -> String {
        self.local_id.clone()
    }

    fn send_to_peer(&self, peer_id: &str, message: WireMessage) {
        let peers = Arc::clone(&self.peers);
        let target = peer_id.to_string();
        tokio::spawn(async move {
            let map = peers.lock().await;
            if let Some(handle) = map.get(&target) {
                let _ = handle.tx.send(message);
            }
        });
    }

    fn broadcast(&self, message: WireMessage) {
        let peers = Arc::clone(&self.peers);
        tokio::spawn(async move {
            let map = peers.lock().await;
            for handle in map.values() {
                let _ = handle.tx.send(message.clone());
            }
        });
    }

    fn broadcast_except(&self, excluded_peer_id: &str, message: WireMessage) {
        let peers = Arc::clone(&self.peers);
        let excluded = excluded_peer_id.to_string();
        tokio::spawn(async move {
            let map = peers.lock().await;
            for (id, handle) in map.iter() {
                if id != &excluded {
                    let _ = handle.tx.send(message.clone());
                }
            }
        });
    }

    fn disconnect_peer(&self, peer_id: &str) {
        let peers = Arc::clone(&self.peers);
        let target = peer_id.to_string();
        tokio::spawn(async move {
            let mut map = peers.lock().await;
            if let Some(handle) = map.remove(&target) {
                handle.conn.close(0u8.into(), b"disconnected");
            }
        });
    }

    fn shutdown(&self) {
        let peers = Arc::clone(&self.peers);
        let endpoint = self.endpoint.clone();
        tokio::spawn(async move {
            {
                let mut map = peers.lock().await;
                for (_, handle) in map.drain() {
                    let _ = handle.tx.send(WireMessage::SessionEnded {
                        reason: "Session ended".to_string(),
                    });
                    handle.conn.close(0u8.into(), b"session ended");
                }
            }
            endpoint.close().await;
        });
    }
}

#[cfg(test)]
mod tests;
