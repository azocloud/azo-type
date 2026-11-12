//! Wire protocol definitions, tickets, and abstract transport trait for P2P collaboration.

use std::fmt;
use std::net::SocketAddr;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use iroh::{EndpointAddr, EndpointId, RelayUrl, TransportAddr};
use rand::RngCore;
use serde::{Deserialize, Serialize};

pub const COLLAB_ALPN: &[u8] = b"azo-type/collab/v1";
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
