//! Unit tests for UTF-16 text differencing, emoji offsets, and collision path formatting.

use super::*;

#[test]
fn test_compute_diff_insertions_at_start_mid_end() {
    let diff_start = compute_utf16_diff("world", "hello world").unwrap();
    assert_eq!(diff_start.utf16_index, 0);
    assert_eq!(diff_start.utf16_remove_len, 0);
    assert_eq!(diff_start.insert_text, "hello ");

    let diff_mid = compute_utf16_diff("ab", "aXb").unwrap();
    assert_eq!(diff_mid.utf16_index, 1);
    assert_eq!(diff_mid.utf16_remove_len, 0);
    assert_eq!(diff_mid.insert_text, "X");

    let diff_end = compute_utf16_diff("hello", "hello!").unwrap();
    assert_eq!(diff_end.utf16_index, 5);
    assert_eq!(diff_end.utf16_remove_len, 0);
    assert_eq!(diff_end.insert_text, "!");
}

#[test]
fn test_compute_diff_deletions() {
    let diff = compute_utf16_diff("hello world", "world").unwrap();
    assert_eq!(diff.utf16_index, 0);
    assert_eq!(diff.utf16_remove_len, 6);
    assert_eq!(diff.insert_text, "");
}

#[test]
fn test_compute_diff_identical_strings() {
    assert_eq!(compute_utf16_diff("exact match", "exact match"), None);
}

#[test]
fn test_astral_plane_emojis_utf16_conversions() {
    let text = "🦀 rust 🚀";
    assert_eq!(char_to_utf16_offset(text, 1), 2);
    assert_eq!(utf16_to_char_offset(text, 2), 1);
}

#[test]
fn test_normalize_rel_path_slashes() {
    assert_eq!(normalize_rel_path("\\foo\\bar.typ"), "foo/bar.typ");
    assert_eq!(normalize_rel_path("/nested/file.typ"), "nested/file.typ");
}

#[test]
fn test_format_collision_path_with_file_id() {
    let collision = format_collision_path("docs/report.typ", "abc12345-6789");
    assert_eq!(collision, "docs/report.conflict-abc123.typ");
}
