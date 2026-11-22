//! Unit tests for pure line, indentation, and comment editing transformations.

use super::*;

#[test]
fn test_toggle_line_comment_single_line_caret_roundtrip() {
    let text = "let x = 1\nlet y = 2";
    let (commented, sel1) = toggle_line_comment(text, CharSelection::caret(4), "//");
    assert_eq!(commented, "// let x = 1\nlet y = 2");
    assert_eq!(sel1, CharSelection::caret(7));

    let (uncommented, sel2) = toggle_line_comment(&commented, sel1, "//");
    assert_eq!(uncommented, text);
    assert_eq!(sel2, CharSelection::caret(4));
}

#[test]
fn test_toggle_line_comment_respects_trailing_col0_boundary() {
    let text = "alpha\nbeta\ngamma";
    // Selection [0, 6) covers "alpha\n" ending at column 0 of line 1 ("beta").
    // Line 1 must NOT be commented.
    let (out, sel) = toggle_line_comment(
        text,
        CharSelection {
            primary_char: 6,
            secondary_char: 0,
        },
        "//",
    );
    assert_eq!(out, "// alpha\nbeta\ngamma");
    assert_eq!(
        sel,
        CharSelection {
            primary_char: 9,
            secondary_char: 3,
        }
    );
}

#[test]
fn test_toggle_line_comment_uses_min_indent_and_skips_empty_lines() {
    let text = "    first\n\n  second";
    let sel = CharSelection {
        primary_char: 18,
        secondary_char: 0,
    };
    let (commented, _) = toggle_line_comment(text, sel, "//");
    assert_eq!(commented, "  //   first\n// \n  // second");

    let (uncommented, _) = toggle_line_comment(
        &commented,
        CharSelection {
            primary_char: 24,
            secondary_char: 0,
        },
        "//",
    );
    assert_eq!(uncommented, text);
}

#[test]
fn test_toggle_line_comment_uncomments_without_space_and_ignores_blank_lines() {
    let text = "  //alpha\n   \n  // beta";
    let (out, _) = toggle_line_comment(
        text,
        CharSelection {
            primary_char: 23,
            secondary_char: 0,
        },
        "//",
    );
    assert_eq!(out, "  alpha\n   \n  beta");
}

#[test]
fn test_toggle_line_comment_all_empty_lines_and_empty_prefix() {
    let (out, sel) = toggle_line_comment("", CharSelection::caret(0), "//");
    assert_eq!(out, "// ");
    assert_eq!(sel, CharSelection::caret(3));

    let (unchanged, sel_unchanged) = toggle_line_comment("abc", CharSelection::caret(1), "");
    assert_eq!(unchanged, "abc");
    assert_eq!(sel_unchanged, CharSelection::caret(1));
}

#[test]
fn test_toggle_line_comment_unicode_and_emoji_offsets() {
    let text = "🦀 hello\n🚀 world";
    // Select across both lines: '🦀' is 1 char, "🦀 hello\n🚀" is 10 chars.
    let (out, sel) = toggle_line_comment(
        text,
        CharSelection {
            primary_char: 10,
            secondary_char: 0,
        },
        "//",
    );
    assert_eq!(out, "// 🦀 hello\n// 🚀 world");
    assert_eq!(
        sel,
        CharSelection {
            primary_char: 16,
            secondary_char: 3,
        }
    );
}

#[test]
fn test_toggle_block_comment_selection_wrap_and_unwrap() {
    let text = "let a = 1 + 2";
    let sel = CharSelection {
        primary_char: 13,
        secondary_char: 8,
    };
    let (wrapped, sel1) = toggle_block_comment(text, sel);
    assert_eq!(wrapped, "let a = /* 1 + 2 */");
    assert_eq!(
        sel1,
        CharSelection {
            primary_char: 19,
            secondary_char: 8,
        }
    );

    let (unwrapped, sel2) = toggle_block_comment(&wrapped, sel1);
    assert_eq!(unwrapped, text);
    assert_eq!(sel2, sel);
}

#[test]
fn test_toggle_block_comment_trimmed_selection_and_backward_direction() {
    let text = "x /*🦀*/ y";
    let sel = CharSelection {
        primary_char: 1,
        secondary_char: 8,
    };
    let (unwrapped, new_sel) = toggle_block_comment(text, sel);
    assert_eq!(unwrapped, "x 🦀 y");
    assert_eq!(
        new_sel,
        CharSelection {
            primary_char: 1,
            secondary_char: 4,
        }
    );
}

#[test]
fn test_toggle_block_comment_caret_insert_and_enclosing_unwrap() {
    let (inserted, sel1) = toggle_block_comment("ab", CharSelection::caret(1));
    assert_eq!(inserted, "a/*  */b");
    assert_eq!(sel1, CharSelection::caret(4));

    let (unwrapped, sel2) = toggle_block_comment(&inserted, sel1);
    assert_eq!(unwrapped, "ab");
    assert_eq!(sel2, CharSelection::caret(1));
}

#[test]
fn test_toggle_block_comment_caret_between_blocks_and_nested_blocks() {
    let between = "/* a */ foo /* b */";
    let (out, sel) = toggle_block_comment(between, CharSelection::caret(9));
    assert_eq!(out, "/* a */ f/*  */oo /* b */");
    assert_eq!(sel, CharSelection::caret(12));

    let nested = "/* outer /* inner */ outer */";
    let (unwrapped_inner, sel_inner) = toggle_block_comment(nested, CharSelection::caret(14));
    assert_eq!(unwrapped_inner, "/* outer inner outer */");
    assert_eq!(sel_inner, CharSelection::caret(11));
}

#[test]
fn test_indent_and_outdent_lines() {
    let text = "  one\n\ttwo\nthree";
    let sel = CharSelection {
        primary_char: 16,
        secondary_char: 2,
    };
    let (indented, sel_in) = indent_lines(text, sel, 2);
    assert_eq!(indented, "    one\n  \ttwo\n  three");
    assert_eq!(
        sel_in,
        CharSelection {
            primary_char: 22,
            secondary_char: 4,
        }
    );

    let (outdented, sel_out) = outdent_lines(&indented, sel_in, 2);
    assert_eq!(outdented, "  one\n\ttwo\nthree");
    assert_eq!(sel_out, sel);

    // Outdenting a line starting with a tab removes exactly one tab.
    let (tab_removed, tab_sel) = outdent_lines("\t\titem", CharSelection::caret(2), 4);
    assert_eq!(tab_removed, "\titem");
    assert_eq!(tab_sel, CharSelection::caret(1));
}

#[test]
fn test_duplicate_lines_middle_last_and_trailing_boundary() {
    let text = "aa\nbb\ncc";
    let (dup_mid, sel_mid) = duplicate_lines(text, CharSelection::caret(4));
    assert_eq!(dup_mid, "aa\nbb\nbb\ncc");
    assert_eq!(sel_mid, CharSelection::caret(7));

    // Last line without trailing newline inserts newline separator.
    let (dup_last, sel_last) = duplicate_lines("aa\nbb", CharSelection::caret(4));
    assert_eq!(dup_last, "aa\nbb\nbb");
    assert_eq!(sel_last, CharSelection::caret(7));

    // Multi-line selection ending at col 0 of line 2 duplicates only lines 0..=1.
    let (dup_multi, sel_multi) = duplicate_lines(
        text,
        CharSelection {
            primary_char: 6,
            secondary_char: 0,
        },
    );
    assert_eq!(dup_multi, "aa\nbb\naa\nbb\ncc");
    assert_eq!(
        sel_multi,
        CharSelection {
            primary_char: 12,
            secondary_char: 6,
        }
    );
}

#[test]
fn test_move_lines_up_and_down_boundaries_and_crlf() {
    assert_eq!(
        move_lines("first\nsecond", CharSelection::caret(2), true),
        None
    );
    assert_eq!(
        move_lines("first\nsecond", CharSelection::caret(8), false),
        None
    );

    // Move last line without trailing newline up.
    let (up, up_sel) = move_lines("aa\nbb", CharSelection::caret(4), true).unwrap();
    assert_eq!(up, "bb\naa");
    assert_eq!(up_sel, CharSelection::caret(1));

    // Move first line down into last line without trailing newline.
    let (down, down_sel) = move_lines(&up, up_sel, false).unwrap();
    assert_eq!(down, "aa\nbb");
    assert_eq!(down_sel, CharSelection::caret(4));

    // Move multi-line block with CRLF and trailing boundary selection.
    let crlf = "0\r\n1\r\n2\r\n3";
    let sel = CharSelection {
        primary_char: 6,
        secondary_char: 3,
    };
    let (moved_down, sel_down) = move_lines(crlf, sel, false).unwrap();
    assert_eq!(moved_down, "0\r\n2\r\n1\r\n3");
    assert_eq!(
        sel_down,
        CharSelection {
            primary_char: 9,
            secondary_char: 6,
        }
    );
}
