use super::*;

#[test]
fn test_ticket_encode_decode_and_redacted_debug() {
    let ep_info = PeerEndpointInfo {
        endpoint_id: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
        ip_addrs: vec!["127.0.0.1:12345".parse().unwrap()],
        relay_urls: Vec::new(),
    };
    let ticket = SessionTicket {
        protocol_version: COLLAB_PROTOCOL_VERSION,
        session_secret: [42u8; 32],
        host: ep_info,
    };

    // Ensure Debug never leaks the secret bytes
    let debug_str = format!("{:?}", ticket);
    assert!(debug_str.contains("<REDACTED>"));
    assert!(!debug_str.contains("42"));

    let encoded = ticket.encode_string().unwrap();
    assert!(encoded.starts_with(TICKET_PREFIX));
    let decoded = SessionTicket::decode_str(&encoded).unwrap();
    assert_eq!(decoded, ticket);
}

#[tokio::test]
async fn test_iroh_two_endpoints_in_same_process_handshake_and_sync() {
    let (host_ev_tx, mut host_ev_rx) = mpsc::unbounded_channel();
    let (client_ev_tx, mut client_ev_rx) = mpsc::unbounded_channel();

    let (host_transport, ticket) =
        IrohTransport::bind_host("HostAlice".to_string(), true, host_ev_tx)
            .await
            .expect("bind host");

    // 1. Verify that a client with a tampered secret is rejected
    let mut bad_ticket = ticket.clone();
    bad_ticket.session_secret[0] ^= 0xFF;
    let bad_res = IrohTransport::connect_with_ticket(
        bad_ticket,
        "Intruder".to_string(),
        true,
        client_ev_tx.clone(),
    )
    .await;
    assert!(bad_res.is_err());

    // 2. Connect with the valid ticket
    let client_transport =
        IrohTransport::connect_with_ticket(ticket, "ClientBob".to_string(), true, client_ev_tx)
            .await
            .expect("connect client");

    // Wait for PeerConnected on both sides
    let host_ev = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(ev @ TransportEvent::PeerConnected { .. }) = host_ev_rx.recv().await {
                return ev;
            }
        }
    })
    .await
    .expect("host PeerConnected");

    let client_ev = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(ev @ TransportEvent::PeerConnected { .. }) = client_ev_rx.recv().await {
                return ev;
            }
        }
    })
    .await
    .expect("client PeerConnected");

    assert!(matches!(host_ev, TransportEvent::PeerConnected { .. }));
    assert!(matches!(client_ev, TransportEvent::PeerConnected { .. }));

    // Exchange a SyncStep1 message from client to host
    client_transport.broadcast(WireMessage::SyncStep1 {
        state_vector_v1: vec![1, 2, 3, 4],
    });

    let msg_ev = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(ev @ TransportEvent::MessageReceived { .. }) = host_ev_rx.recv().await {
                return ev;
            }
        }
    })
    .await
    .expect("host MessageReceived");

    match msg_ev {
        TransportEvent::MessageReceived {
            message: WireMessage::SyncStep1 { state_vector_v1 },
            ..
        } => {
            assert_eq!(state_vector_v1, vec![1, 2, 3, 4]);
        }
        other => panic!("Unexpected event: {:?}", other),
    }

    client_transport.shutdown();
    host_transport.shutdown();
}
