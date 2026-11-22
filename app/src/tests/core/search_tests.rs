//! Unit tests for char-indexed search and replace utilities.

use super::*;

#[test]
fn test_find_all_matches_empty_query_and_non_overlapping() {
    let opts = SearchOptions {
        case_sensitive: true,
    };
    assert_eq!(find_all_matches("hello", "", opts), Vec::new());
    assert_eq!(
        find_all_matches("aaaaa", "aa", opts),
        vec![
            SearchMatch {
                start_char: 0,
                end_char: 2,
            },
            SearchMatch {
                start_char: 2,
                end_char: 4,
            },
        ]
    );
    assert_eq!(
        find_all_matches("abab", "ab", opts),
        vec![
            SearchMatch {
                start_char: 0,
                end_char: 2,
            },
            SearchMatch {
                start_char: 2,
                end_char: 4,
            },
        ]
    );
}

#[test]
fn test_find_all_matches_unicode_emoji_and_case_folding() {
    let text = "🦀 İstanbul ẞeta 🚀";
    let insensitive = SearchOptions {
        case_sensitive: false,
    };
    let sensitive = SearchOptions {
        case_sensitive: true,
    };

    // 'İ' folds to 2 chars ("i\u{0307}") in lowercase, yet offsets after 'İ' must stay char-exact.
    assert_eq!(
        find_all_matches(text, "STANBUL", insensitive),
        vec![SearchMatch {
            start_char: 3,
            end_char: 10,
        }]
    );
    assert_eq!(
        find_all_matches(text, "ßETA", insensitive),
        vec![SearchMatch {
            start_char: 11,
            end_char: 15,
        }]
    );
    // Partial match inside 'İ' expansion ("i" alone) must not match.
    assert_eq!(find_all_matches("İ", "i", insensitive), Vec::new());
    // Full expansion match against 'İ' succeeds and reports [0, 1).
    assert_eq!(
        find_all_matches("İ", "i\u{0307}", insensitive),
        vec![SearchMatch {
            start_char: 0,
            end_char: 1,
        }]
    );
    // Case-sensitive skips mismatched case while preserving emoji char offsets.
    assert_eq!(find_all_matches(text, "istanbul", sensitive), Vec::new());
    assert_eq!(
        find_all_matches("🦀 Rust 🚀 rust", "rust", insensitive),
        vec![
            SearchMatch {
                start_char: 2,
                end_char: 6,
            },
            SearchMatch {
                start_char: 9,
                end_char: 13,
            },
        ]
    );
}

#[test]
fn test_find_next_match_index_forward_and_backward_wraparound() {
    assert_eq!(find_next_match_index(&[], 0, true), None);
    assert_eq!(find_next_match_index(&[], 0, false), None);

    let matches = vec![
        SearchMatch {
            start_char: 2,
            end_char: 5,
        },
        SearchMatch {
            start_char: 10,
            end_char: 13,
        },
        SearchMatch {
            start_char: 20,
            end_char: 23,
        },
    ];

    assert_eq!(find_next_match_index(&matches, 0, true), Some(0));
    assert_eq!(find_next_match_index(&matches, 2, true), Some(0));
    assert_eq!(find_next_match_index(&matches, 3, true), Some(1));
    assert_eq!(find_next_match_index(&matches, 20, true), Some(2));
    assert_eq!(find_next_match_index(&matches, 21, true), Some(0));

    assert_eq!(find_next_match_index(&matches, 25, false), Some(2));
    assert_eq!(find_next_match_index(&matches, 20, false), Some(1));
    assert_eq!(find_next_match_index(&matches, 11, false), Some(1));
    assert_eq!(find_next_match_index(&matches, 10, false), Some(0));
    assert_eq!(find_next_match_index(&matches, 2, false), Some(2));
    assert_eq!(find_next_match_index(&matches, 0, false), Some(2));
}

#[test]
fn test_replace_match_at_unicode() {
    let text = "🦀 foo 🚀";
    let (replaced, next_cursor) = replace_match_at(
        text,
        SearchMatch {
            start_char: 2,
            end_char: 5,
        },
        "bar✨",
    );
    assert_eq!(replaced, "🦀 bar✨ 🚀");
    assert_eq!(next_cursor, 6);
}

#[test]
fn test_replace_all_matches_one_pass_and_overlapping_replacement() {
    let opts = SearchOptions {
        case_sensitive: false,
    };

    assert_eq!(replace_all_matches("hello", "", "x", opts), None);
    assert_eq!(replace_all_matches("hello", "zzz", "x", opts), None);

    // Replacing "a" with "aa" must run in a single non-recursive pass.
    let (out, count) = replace_all_matches("Aba", "a", "aa", opts).unwrap();
    assert_eq!(out, "aabaa");
    assert_eq!(count, 2);

    let (unicode_out, unicode_count) =
        replace_all_matches("🦀 İ foo 🦀 İ", "i\u{0307}", "X", opts).unwrap();
    assert_eq!(unicode_out, "🦀 X foo 🦀 X");
    assert_eq!(unicode_count, 2);
}
