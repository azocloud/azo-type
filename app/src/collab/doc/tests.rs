use super::*;

#[test]
fn test_two_docs_converge_with_swapped_update_order() {
    let mut peer_a = CollabProjectDoc::with_client_id(Some(101));
    let mut peer_b = CollabProjectDoc::with_client_id(Some(202));

    // Initial shared file created on Peer A and synced to Peer B
    let fid = peer_a.create_file(
        Some("11111111-1111-1111-1111-111111111111".to_string()),
        "main.typ",
        "= Title\n\nHello world!\n",
    );
    for u in peer_a.drain_local_updates() {
        peer_b.apply_remote_update_v1(&u).unwrap();
    }
    assert_eq!(peer_a.file_content(&fid), peer_b.file_content(&fid));

    // Concurrent edits on both peers
    peer_a.apply_local_text_edit(&fid, "= Title (Edited by A)\n\nHello world!\n");
    peer_b.apply_local_text_edit(&fid, "= Title\n\nHello collaborative world from B!\n");

    let updates_from_a = peer_a.drain_local_updates();
    let updates_from_b = peer_b.drain_local_updates();

    // Clone into a third replica to test applying (B then A) vs (A then B)
    let mut peer_c = CollabProjectDoc::with_client_id(Some(303));
    peer_c
        .apply_remote_update_v1(&peer_a.encode_full_snapshot_v1())
        .unwrap();

    for u in &updates_from_b {
        peer_a.apply_remote_update_v1(u).unwrap();
        peer_c.apply_remote_update_v1(u).unwrap();
    }
    for u in &updates_from_a {
        peer_b.apply_remote_update_v1(u).unwrap();
    }

    assert_eq!(peer_a.file_content(&fid), peer_b.file_content(&fid));
    assert_eq!(peer_a.file_content(&fid), peer_c.file_content(&fid));
}

#[test]
fn test_unicode_umlauts_emoji_and_multi_codepoint_offsets() {
    let mut doc = CollabProjectDoc::with_client_id(Some(1));
    let initial = "Grüße 🦀 aus München 👨‍👩‍👧‍👦 Ende";
    let fid = doc.create_file(None, "unicode.typ", initial);

    // Verify roundtrip between char index and UTF-16 offset for every char boundary
    let char_count = initial.chars().count();
    for c_idx in 0..=char_count {
        let utf16 = char_to_utf16_offset(initial, c_idx);
        let back = utf16_to_char_offset(initial, utf16);
        assert_eq!(back, c_idx, "Mismatch at char index {}", c_idx);
    }

    // Edit right after the crab emoji 🦀 and before the family emoji 👨‍👩‍👧‍👦
    let updated = "Grüße 🦀 & ÄÖÜß aus München 👨‍👩‍👧‍👦 Ende!";
    assert!(doc.apply_local_text_edit(&fid, updated));
    assert_eq!(doc.file_content(&fid).as_deref(), Some(updated));
}

#[test]
fn test_sticky_cursor_preservation_on_remote_edit() {
    let mut peer_a = CollabProjectDoc::with_client_id(Some(10));
    let mut peer_b = CollabProjectDoc::with_client_id(Some(20));

    let initial = "Line 1\nTarget Word\nLine 3";
    let fid = peer_a.create_file(
        Some("22222222-2222-2222-2222-222222222222".to_string()),
        "chapters/intro.typ",
        initial,
    );
    for u in peer_a.drain_local_updates() {
        peer_b.apply_remote_update_v1(&u).unwrap();
    }

    // Peer B has its cursor placed right before 'W' in "Target Word" (char index 14)
    let cursor_before = CharSelection::caret(14);
    assert_eq!(
        peer_b
            .file_content(&fid)
            .unwrap()
            .chars()
            .nth(cursor_before.primary_char),
        Some('W')
    );

    // Peer A inserts "Inserted Header 🦀\n" at the very top of the file
    let inserted_prefix = "Inserted Header 🦀\n";
    peer_a.apply_local_text_edit(&fid, &format!("{}{}", inserted_prefix, initial));
    let updates = peer_a.drain_local_updates();

    let mut cursor_after = Some(cursor_before);
    for u in updates {
        cursor_after = peer_b
            .apply_remote_update_preserving_cursor(&u, Some(&fid), cursor_after)
            .unwrap();
    }

    let resolved_idx = cursor_after.unwrap().primary_char;
    let final_text = peer_b.file_content(&fid).unwrap();
    assert_eq!(final_text.chars().nth(resolved_idx), Some('W'));
    assert_eq!(
        resolved_idx,
        cursor_before.primary_char + inserted_prefix.chars().count()
    );
}

#[test]
fn test_concurrent_file_rename_and_path_collision_resolution() {
    let mut peer_a = CollabProjectDoc::with_client_id(Some(1));
    let mut peer_b = CollabProjectDoc::with_client_id(Some(2));

    // Both peers concurrently create a file named "notes.typ" with different UUIDs
    let id_a = peer_a.create_file(
        Some("aaaa0000-0000-0000-0000-000000000001".to_string()),
        "notes.typ",
        "Notes from A",
    );
    let id_b = peer_b.create_file(
        Some("bbbb0000-0000-0000-0000-000000000002".to_string()),
        "notes.typ",
        "Notes from B",
    );

    let up_a = peer_a.drain_local_updates();
    let up_b = peer_b.drain_local_updates();

    for u in up_a {
        peer_b.apply_remote_update_v1(&u).unwrap();
    }
    for u in up_b {
        peer_a.apply_remote_update_v1(&u).unwrap();
    }

    let files_a = peer_a.list_files();
    let files_b = peer_b.list_files();
    assert_eq!(files_a, files_b);
    assert_eq!(files_a.len(), 2);
    assert_eq!(files_a[0].resolved_path, "notes.conflict-bbbb00.typ");
    assert_eq!(files_a[1].resolved_path, "notes.typ");

    // Now Peer A renames its file while Peer B edits it concurrently
    peer_a.rename_file(&id_a, "chapters/notes-a.typ");
    peer_b.apply_local_text_edit(&id_a, "Notes from A (updated by B during rename!)");

    let up_a2 = peer_a.drain_local_updates();
    let up_b2 = peer_b.drain_local_updates();
    for u in up_a2 {
        peer_b.apply_remote_update_v1(&u).unwrap();
    }
    for u in up_b2 {
        peer_a.apply_remote_update_v1(&u).unwrap();
    }

    assert_eq!(peer_a.list_files(), peer_b.list_files());
    assert_eq!(
        peer_a.file_content(&id_a).as_deref(),
        Some("Notes from A (updated by B during rename!)")
    );
    let _ = id_b;
}

#[test]
fn test_local_undo_only_reverts_own_changes() {
    let mut peer_a = CollabProjectDoc::with_client_id(Some(11));
    let mut peer_b = CollabProjectDoc::with_client_id(Some(22));

    let fid = peer_a.create_file(
        Some("33333333-3333-3333-3333-333333333333".to_string()),
        "main.typ",
        "Base",
    );
    for u in peer_a.drain_local_updates() {
        peer_b.apply_remote_update_v1(&u).unwrap();
    }

    // Peer A makes a local edit
    peer_a.apply_local_text_edit(&fid, "Base + LocalA");
    for u in peer_a.drain_local_updates() {
        peer_b.apply_remote_update_v1(&u).unwrap();
    }

    // Peer B appends a remote edit at the end
    peer_b.apply_local_text_edit(&fid, "Base + LocalA + RemoteB");
    for u in peer_b.drain_local_updates() {
        peer_a.apply_remote_update_v1(&u).unwrap();
    }
    assert_eq!(
        peer_a.file_content(&fid).as_deref(),
        Some("Base + LocalA + RemoteB")
    );

    // Peer A hits Undo: only " + LocalA" should be undone, preserving " + RemoteB"!
    assert!(peer_a.undo_file(&fid));
    assert_eq!(peer_a.file_content(&fid).as_deref(), Some("Base + RemoteB"));
}
