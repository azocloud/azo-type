//! Unit tests for peer awareness state encoding, decoding, and synchronization.

use super::*;

#[test]
fn test_awareness_sync_roundtrip() {
    let doc_a = Doc::with_client_id(10);
    let doc_b = Doc::with_client_id(20);

    let mut aw_a = CollabAwareness::new(doc_a);
    let mut aw_b = CollabAwareness::new(doc_b);

    let presence_a = PeerPresence {
        client_id: 10,
        endpoint_id: "peer-a-endpoint".to_string(),
        name: "Alice".to_string(),
        color_rgb: [56, 189, 248],
        active_file_id: Some("uuid-1".to_string()),
        active_path: Some("main.typ".to_string()),
        cursor_char: Some(42),
        anchor_char: Some(30),
    };

    let bytes = aw_a.set_local_presence(&presence_a).unwrap();
    aw_b.apply_remote_awareness_bytes(&bytes).unwrap();

    let remotes_on_b = aw_b.remote_peers();
    assert_eq!(remotes_on_b.len(), 1);
    assert_eq!(remotes_on_b[0], presence_a);
}
