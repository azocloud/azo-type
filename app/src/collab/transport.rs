use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use iroh::endpoint::{presets, Connection, RecvStream, SendStream};
use iroh::{Endpoint, RelayMode};
use tokio::sync::{mpsc, Mutex};

pub use crate::collab::protocol::*;

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
