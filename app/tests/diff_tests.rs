use azo_type::collab::doc::{
    char_to_utf16_offset, compute_utf16_diff, format_collision_path, normalize_rel_path,
    utf16_to_char_offset,
};

#[test]
fn returns_none_when_strings_are_identical() {
    assert_eq!(compute_utf16_diff("hello world", "hello world"), None);
    assert_eq!(compute_utf16_diff("", ""), None);
}

#[test]
fn computes_diff_for_insertions_at_start_middle_and_end() {
    // Insertion at end
    let diff_end = compute_utf16_diff("abc", "abcdef").unwrap();
    assert_eq!(diff_end.utf16_index, 3);
    assert_eq!(diff_end.utf16_remove_len, 0);
    assert_eq!(diff_end.insert_text, "def");

    // Insertion at start
    let diff_start = compute_utf16_diff("def", "abcdef").unwrap();
    assert_eq!(diff_start.utf16_index, 0);
    assert_eq!(diff_start.utf16_remove_len, 0);
    assert_eq!(diff_start.insert_text, "abc");

    // Insertion in middle
    let diff_mid = compute_utf16_diff("ac", "abc").unwrap();
    assert_eq!(diff_mid.utf16_index, 1);
    assert_eq!(diff_mid.utf16_remove_len, 0);
    assert_eq!(diff_mid.insert_text, "b");
}

#[test]
fn computes_diff_for_deletions() {
    let diff_del = compute_utf16_diff("abcdef", "abef").unwrap();
    assert_eq!(diff_del.utf16_index, 2);
    assert_eq!(diff_del.utf16_remove_len, 2); // removed "cd"
    assert_eq!(diff_del.insert_text, "");
}

#[test]
fn accurately_converts_utf16_and_char_offsets_across_astral_plane_emojis() {
    // String containing 1-byte ASCII, 2-byte Umlauts, 4-byte astral plane emojis (2 UTF-16 code units)
    let s = "A ä 🦀 B 👨‍👩‍👧‍👦 C";

    let char_count = s.chars().count();
    for i in 0..=char_count {
        let utf16 = char_to_utf16_offset(s, i);
        let back_char = utf16_to_char_offset(s, utf16);
        assert_eq!(
            back_char, i,
            "Failed roundtrip at char index {} (utf16={})",
            i, utf16
        );
    }
}

#[test]
fn formats_deterministic_collision_paths_with_file_id_hash() {
    let collision = format_collision_path("notes.typ", "12345678-abcd-ef01-2345-6789abcdef01");
    assert_eq!(collision, "notes.conflict-123456.typ");

    let nested_collision =
        format_collision_path("chapters/intro.typ", "abcdef99-1111-2222-3333-444455556666");
    assert_eq!(nested_collision, "chapters/intro.conflict-abcdef.typ");

    let no_ext_collision = format_collision_path("LICENSE", "99887766-0000-0000-0000-000000000000");
    assert_eq!(no_ext_collision, "LICENSE.conflict-998877");
}

#[test]
fn normalizes_windows_and_unix_relative_paths() {
    assert_eq!(
        normalize_rel_path("chapters\\sub\\intro.typ"),
        "chapters/sub/intro.typ"
    );
    assert_eq!(normalize_rel_path("/root/file.typ"), "root/file.typ");
    assert_eq!(normalize_rel_path("\\\\win\\path.typ"), "win/path.typ");
    assert_eq!(normalize_rel_path("   clean/path.typ   "), "clean/path.typ");
}
