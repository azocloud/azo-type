//! Unit tests for P2P collaboration wire protocol types, ticket validation, and secret redaction.

use iroh::{EndpointAddr, EndpointId, RelayUrl};

use super::*;

const VALID_HEX_ID: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

#[test]
fn connection_route_labels_and_direct_flag() {
    let direct = ConnectionRoute::Direct {
        addr: "127.0.0.1:4242".to_string(),
        rtt_ms: 12,
    };
    assert!(direct.is_direct());
    assert_eq!(direct.badge_label(), "Direct (12 ms)");

    let relay = ConnectionRoute::Relay {
        url: "https://relay.example".to_string(),
        rtt_ms: 48,
    };
    assert!(!relay.is_direct());
    assert_eq!(relay.badge_label(), "Relay (48 ms)");

    let unknown = ConnectionRoute::Unknown;
    assert!(!unknown.is_direct());
    assert_eq!(unknown.badge_label(), "Negotiating");
}

#[test]
fn peer_endpoint_info_roundtrips_and_rejects_invalid_id() {
    let id: EndpointId = VALID_HEX_ID.parse().expect("valid endpoint id");
    let relay: RelayUrl = "https://relay.example.com"
        .parse()
        .expect("valid relay url");
    let addr = EndpointAddr::new(id)
        .with_ip_addr("127.0.0.1:7777".parse().unwrap())
        .with_relay_url(relay);

    let info = PeerEndpointInfo::from_endpoint_addr(&addr);
    assert_eq!(info.endpoint_id, VALID_HEX_ID);
    assert_eq!(info.ip_addrs.len(), 1);
    assert_eq!(info.relay_urls.len(), 1);

    let reconstructed = info.to_endpoint_addr().expect("reconstruct endpoint addr");
    assert_eq!(reconstructed.id, id);

    let bad = PeerEndpointInfo {
        endpoint_id: "not-a-valid-endpoint-id".to_string(),
        ip_addrs: Vec::new(),
        relay_urls: Vec::new(),
    };
    assert!(bad.to_endpoint_addr().is_err());
}

#[test]
fn session_ticket_generate_and_decode_validation_errors() {
    let id: EndpointId = VALID_HEX_ID.parse().unwrap();
    let ticket = SessionTicket::generate(&EndpointAddr::new(id));
    let encoded = ticket.encode_string().unwrap();
    let decoded = SessionTicket::decode_str(&format!("  {encoded}  ")).unwrap();
    assert_eq!(decoded, ticket);

    assert!(SessionTicket::decode_str("wrong-prefix:abc").is_err());
    assert!(SessionTicket::decode_str(&format!("{TICKET_PREFIX}!!!not-base64!!!")).is_err());

    let bad_json_b64 = URL_SAFE_NO_PAD.encode(b"{not-json");
    assert!(SessionTicket::decode_str(&format!("{TICKET_PREFIX}{bad_json_b64}")).is_err());

    let mut wrong_ver = ticket.clone();
    wrong_ver.protocol_version = COLLAB_PROTOCOL_VERSION + 99;
    let wrong_ver_str = wrong_ver.encode_string().unwrap();
    let err = SessionTicket::decode_str(&wrong_ver_str).unwrap_err();
    assert!(err.contains("Incompatible ticket protocol version"));
}

#[test]
fn constant_time_eq_and_wire_message_debug_redaction() {
    let a = [7u8; 32];
    let mut b = [7u8; 32];
    assert!(constant_time_eq(&a, &b));
    b[31] = 8;
    assert!(!constant_time_eq(&a, &b));

    let ep = PeerEndpointInfo {
        endpoint_id: VALID_HEX_ID.to_string(),
        ip_addrs: Vec::new(),
        relay_urls: Vec::new(),
    };
    let hello = WireMessage::HandshakeHello {
        protocol_version: COLLAB_PROTOCOL_VERSION,
        session_secret: [99u8; 32],
        peer_name: "Alice".to_string(),
        peer_endpoint: ep.clone(),
    };
    let dbg = format!("{hello:?}");
    assert!(dbg.contains("<REDACTED>"));
    assert!(!dbg.contains("99"));

    let messages = [
        WireMessage::HandshakeAck {
            protocol_version: 1,
            accepted: true,
            reason: None,
            host_name: "Host".to_string(),
            known_peers: vec![ep.clone()],
        },
        WireMessage::SyncStep1 {
            state_vector_v1: vec![1, 2, 3],
        },
        WireMessage::SyncStep2 {
            update_v1: vec![4, 5],
        },
        WireMessage::CrdtUpdate {
            update_v1: vec![6],
            origin_peer: "peer".to_string(),
        },
        WireMessage::AwarenessUpdate {
            awareness_v1: vec![7, 8],
        },
        WireMessage::PeerListAnnounce { peers: vec![ep] },
        WireMessage::SessionEnded {
            reason: "done".to_string(),
        },
    ];
    for msg in &messages {
        assert!(!format!("{msg:?}").is_empty());
    }
}
