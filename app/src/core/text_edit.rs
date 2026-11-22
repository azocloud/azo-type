//! Pure line, indentation, and comment editing transformations.
//!
//! All operations operate strictly in-memory on `&str` and [`CharSelection`]
//! using Unicode scalar (`char`) indices, with zero UI or filesystem dependencies.

use crate::core::diff::CharSelection;

#[cfg(test)]
#[path = "../tests/core/text_edit_tests.rs"]
mod tests;

struct LineInfo<'a> {
    content: &'a str,
    newline: &'a str,
    content_start_char: usize,
    content_end_char: usize,
    line_end_char: usize,
    content_chars: usize,
    nl_chars: usize,
}

#[derive(Clone, Copy)]
enum CharEdit {
    Insert { pos: usize, len: usize },
    Delete { start: usize, end: usize },
}

fn parse_lines(text: &str) -> (Vec<LineInfo<'_>>, usize) {
    let mut lines = Vec::new();
    let mut char_pos = 0usize;

    for raw in text.split_inclusive('\n') {
        let (content, newline) = if let Some(c) = raw.strip_suffix("\r\n") {
            (c, "\r\n")
        } else if let Some(c) = raw.strip_suffix('\n') {
            (c, "\n")
        } else {
            (raw, "")
        };
        let content_chars = content.chars().count();
        let nl_chars = newline.chars().count();
        let content_end_char = char_pos + content_chars;
        let line_end_char = content_end_char + nl_chars;
        lines.push(LineInfo {
            content,
            newline,
            content_start_char: char_pos,
            content_end_char,
            line_end_char,
            content_chars,
            nl_chars,
        });
        char_pos = line_end_char;
    }

    if lines.is_empty() || text.ends_with('\n') {
        lines.push(LineInfo {
            content: "",
            newline: "",
            content_start_char: char_pos,
            content_end_char: char_pos,
            line_end_char: char_pos,
            content_chars: 0,
            nl_chars: 0,
        });
    }

    (lines, char_pos)
}

fn clamp_selection(sel: CharSelection, total_chars: usize) -> CharSelection {
    CharSelection {
        primary_char: sel.primary_char.min(total_chars),
        secondary_char: sel.secondary_char.min(total_chars),
    }
}

fn touched_line_range(lines: &[LineInfo<'_>], sel: CharSelection) -> (usize, usize) {
    let start = sel.primary_char.min(sel.secondary_char);
    let end = sel.primary_char.max(sel.secondary_char);

    if start == end {
        for (idx, line) in lines.iter().enumerate() {
            if line.newline.is_empty() || start < line.line_end_char {
                return (idx, idx);
            }
        }
        let last = lines.len().saturating_sub(1);
        return (last, last);
    }

    let mut range: Option<(usize, usize)> = None;
    for (idx, line) in lines.iter().enumerate() {
        if line.content_start_char < end && start < line.line_end_char {
            range = Some(match range {
                Some((first, _)) => (first, idx),
                None => (idx, idx),
            });
        }
    }
    range.unwrap_or_else(|| {
        let last = lines.len().saturating_sub(1);
        (last, last)
    })
}

fn map_pos_through_edits(pos: usize, edits: &[CharEdit]) -> usize {
    let mut delta: isize = 0;
    for &edit in edits {
        match edit {
            CharEdit::Insert { pos: edit_pos, len } if pos >= edit_pos => delta += len as isize,
            CharEdit::Delete { start, end } if pos >= end => delta -= (end - start) as isize,
            CharEdit::Delete { start, .. } if pos > start => delta -= (pos - start) as isize,
            _ => {}
        }
    }
    pos.saturating_add_signed(delta)
}

fn map_selection_through_edits(sel: CharSelection, edits: &[CharEdit]) -> CharSelection {
    CharSelection {
        primary_char: map_pos_through_edits(sel.primary_char, edits),
        secondary_char: map_pos_through_edits(sel.secondary_char, edits),
    }
}

fn char_to_byte_idx(s: &str, char_idx: usize) -> usize {
    s.char_indices().nth(char_idx).map_or(s.len(), |(b, _)| b)
}

fn leading_whitespace_chars(line: &str) -> usize {
    line.chars().take_while(|c| c.is_whitespace()).count()
}

fn push_lines(out: &mut String, slice: &[LineInfo<'_>]) {
    for line in slice {
        out.push_str(line.content);
        out.push_str(line.newline);
    }
}

/// Toggles a line comment prefix (such as `"//"`) across all lines overlapped by `sel`.
///
/// If all non-empty touched lines already begin with `prefix` after leading whitespace,
/// strips `prefix` and one optional following `' '` from each touched line. Otherwise,
/// inserts `format!("{prefix} ")` at the minimum leading-whitespace column across
/// non-empty touched lines (or column `0` if all touched lines are empty).
pub fn toggle_line_comment(
    text: &str,
    sel: CharSelection,
    prefix: &str,
) -> (String, CharSelection) {
    let (lines, total_chars) = parse_lines(text);
    let clamped = clamp_selection(sel, total_chars);
    if prefix.is_empty() {
        return (text.to_string(), clamped);
    }

    let (first, last) = touched_line_range(&lines, clamped);
    let prefix_chars = prefix.chars().count();
    let mut has_non_empty = false;
    let mut all_commented = true;
    let mut min_indent = usize::MAX;

    for line in &lines[first..=last] {
        if line.content.trim().is_empty() {
            continue;
        }
        has_non_empty = true;
        let ws_chars = leading_whitespace_chars(line.content);
        min_indent = min_indent.min(ws_chars);
        let ws_bytes = char_to_byte_idx(line.content, ws_chars);
        if !line.content[ws_bytes..].starts_with(prefix) {
            all_commented = false;
        }
    }

    let uncomment = has_non_empty && all_commented;
    let insert_col = if has_non_empty { min_indent } else { 0 };
    let insert_str = format!("{prefix} ");
    let insert_len = prefix_chars + 1;
    let mut out = String::with_capacity(text.len() + (last - first + 1) * insert_len);
    let mut edits = Vec::with_capacity(last - first + 1);

    for (idx, line) in lines.iter().enumerate() {
        if idx < first || idx > last {
            out.push_str(line.content);
            out.push_str(line.newline);
            continue;
        }
        if uncomment {
            let ws_chars = leading_whitespace_chars(line.content);
            let ws_bytes = char_to_byte_idx(line.content, ws_chars);
            if let Some(rest) = line.content[ws_bytes..].strip_prefix(prefix) {
                let (stripped, space_char) =
                    rest.strip_prefix(' ').map_or((rest, 0), |after| (after, 1));
                out.push_str(&line.content[..ws_bytes]);
                out.push_str(stripped);
                let del_start = line.content_start_char + ws_chars;
                edits.push(CharEdit::Delete {
                    start: del_start,
                    end: del_start + prefix_chars + space_char,
                });
            } else {
                out.push_str(line.content);
            }
        } else {
            let col = insert_col.min(line.content_chars);
            let byte_col = char_to_byte_idx(line.content, col);
            out.push_str(&line.content[..byte_col]);
            out.push_str(&insert_str);
            out.push_str(&line.content[byte_col..]);
            edits.push(CharEdit::Insert {
                pos: line.content_start_char + col,
                len: insert_len,
            });
        }
        out.push_str(line.newline);
    }

    (out, map_selection_through_edits(clamped, &edits))
}

fn find_enclosing_block_comment(chars: &[char], pos: usize) -> Option<(usize, usize)> {
    let mut stack = Vec::new();
    let mut i = 0usize;
    let mut best: Option<(usize, usize)> = None;

    while i + 1 < chars.len() {
        if chars[i] == '/' && chars[i + 1] == '*' {
            stack.push(i);
            i += 2;
        } else if chars[i] == '*' && chars[i + 1] == '/' {
            if let Some(open) = stack.pop() {
                let close = i + 2;
                if pos >= open + 2
                    && pos <= close - 2
                    && best.is_none_or(|(b_open, b_close)| close - open < b_close - b_open)
                {
                    best = Some((open, close));
                }
            }
            i += 2;
        } else {
            i += 1;
        }
    }
    best
}

fn unwrap_comment_range(
    chars: &[char],
    open: usize,
    close: usize,
    sel: CharSelection,
) -> (String, CharSelection) {
    let mut body_start = open + 2;
    let mut body_end = close - 2;
    if body_start < body_end && chars[body_start] == ' ' {
        body_start += 1;
    }
    if body_start < body_end && chars[body_end - 1] == ' ' {
        body_end -= 1;
    }
    let edits = [
        CharEdit::Delete {
            start: open,
            end: body_start,
        },
        CharEdit::Delete {
            start: body_end,
            end: close,
        },
    ];
    let mut out = String::with_capacity(chars.len());
    out.extend(&chars[..open]);
    out.extend(&chars[body_start..body_end]);
    out.extend(&chars[close..]);
    (out, map_selection_through_edits(sel, &edits))
}

/// Toggles a `/* ... */` block comment around the active selection or enclosing caret block.
///
/// - If a non-empty selection's trimmed range begins with `/*` and ends with `*/`, unwraps
///   those delimiters along with up to one padding space on each side.
/// - Else if the caret (`primary == secondary`) lies inside an enclosing `/* ... */` block,
///   unwraps that innermost enclosing block.
/// - Else wraps the selection in `/* ... */`, or inserts `/*  */` at the caret with the
///   caret positioned inside at `+3`.
pub fn toggle_block_comment(text: &str, sel: CharSelection) -> (String, CharSelection) {
    let chars: Vec<char> = text.chars().collect();
    let clamped = clamp_selection(sel, chars.len());
    let start = clamped.primary_char.min(clamped.secondary_char);
    let end = clamped.primary_char.max(clamped.secondary_char);

    if start < end {
        let lead_ws = chars[start..end]
            .iter()
            .take_while(|c| c.is_whitespace())
            .count();
        let trail_ws = chars[start..end]
            .iter()
            .rev()
            .take_while(|c| c.is_whitespace())
            .count();
        let inner_start = start + lead_ws;
        let inner_end = end.saturating_sub(trail_ws);

        if inner_end >= inner_start + 4
            && chars[inner_start] == '/'
            && chars[inner_start + 1] == '*'
            && chars[inner_end - 2] == '*'
            && chars[inner_end - 1] == '/'
        {
            return unwrap_comment_range(&chars, inner_start, inner_end, clamped);
        }

        let start_byte = char_to_byte_idx(text, start);
        let end_byte = char_to_byte_idx(text, end);
        let mut out = String::with_capacity(text.len() + 6);
        out.push_str(&text[..start_byte]);
        out.push_str("/* ");
        out.push_str(&text[start_byte..end_byte]);
        out.push_str(" */");
        out.push_str(&text[end_byte..]);

        let new_end = end + 6;
        let new_sel = if clamped.primary_char >= clamped.secondary_char {
            CharSelection {
                primary_char: new_end,
                secondary_char: start,
            }
        } else {
            CharSelection {
                primary_char: start,
                secondary_char: new_end,
            }
        };
        return (out, new_sel);
    }

    if let Some((open, close)) = find_enclosing_block_comment(&chars, start) {
        return unwrap_comment_range(&chars, open, close, clamped);
    }

    let start_byte = char_to_byte_idx(text, start);
    let mut out = String::with_capacity(text.len() + 6);
    out.push_str(&text[..start_byte]);
    out.push_str("/*  */");
    out.push_str(&text[start_byte..]);
    (out, CharSelection::caret(start + 3))
}

/// Indents all lines overlapped by `sel` by `spaces` space characters.
pub fn indent_lines(text: &str, sel: CharSelection, spaces: usize) -> (String, CharSelection) {
    let (lines, total_chars) = parse_lines(text);
    let clamped = clamp_selection(sel, total_chars);
    if spaces == 0 {
        return (text.to_string(), clamped);
    }

    let (first, last) = touched_line_range(&lines, clamped);
    let indent = " ".repeat(spaces);
    let mut out = String::with_capacity(text.len() + (last - first + 1) * spaces);
    let mut edits = Vec::with_capacity(last - first + 1);

    for (idx, line) in lines.iter().enumerate() {
        if idx >= first && idx <= last {
            out.push_str(&indent);
            edits.push(CharEdit::Insert {
                pos: line.content_start_char,
                len: spaces,
            });
        }
        out.push_str(line.content);
        out.push_str(line.newline);
    }

    (out, map_selection_through_edits(clamped, &edits))
}

/// Outdents all lines overlapped by `sel` by removing up to `spaces` leading spaces or one leading `\t`.
pub fn outdent_lines(text: &str, sel: CharSelection, spaces: usize) -> (String, CharSelection) {
    let (lines, total_chars) = parse_lines(text);
    let clamped = clamp_selection(sel, total_chars);
    if spaces == 0 {
        return (text.to_string(), clamped);
    }

    let (first, last) = touched_line_range(&lines, clamped);
    let mut out = String::with_capacity(text.len());
    let mut edits = Vec::with_capacity(last - first + 1);

    for (idx, line) in lines.iter().enumerate() {
        if idx >= first && idx <= last {
            let remove_chars = if line.content.starts_with('\t') {
                1
            } else {
                line.content
                    .chars()
                    .take_while(|&c| c == ' ')
                    .count()
                    .min(spaces)
            };
            let remove_bytes = char_to_byte_idx(line.content, remove_chars);
            out.push_str(&line.content[remove_bytes..]);
            if remove_chars > 0 {
                edits.push(CharEdit::Delete {
                    start: line.content_start_char,
                    end: line.content_start_char + remove_chars,
                });
            }
        } else {
            out.push_str(line.content);
        }
        out.push_str(line.newline);
    }

    (out, map_selection_through_edits(clamped, &edits))
}

/// Duplicates the lines overlapped by `sel` directly below and moves `sel` onto the duplicated copy.
pub fn duplicate_lines(text: &str, sel: CharSelection) -> (String, CharSelection) {
    let (lines, total_chars) = parse_lines(text);
    let clamped = clamp_selection(sel, total_chars);
    let (first, last) = touched_line_range(&lines, clamped);
    let default_nl = lines
        .iter()
        .find(|l| !l.newline.is_empty())
        .map_or("\n", |l| l.newline);

    let mut out = String::with_capacity(text.len() * 2);
    push_lines(&mut out, &lines[..=last]);
    let sep_nl = if lines[last].newline.is_empty() {
        out.push_str(default_nl);
        default_nl
    } else {
        ""
    };
    push_lines(&mut out, &lines[first..=last]);
    push_lines(&mut out, &lines[(last + 1)..]);

    let shift =
        (lines[last].line_end_char - lines[first].content_start_char) + sep_nl.chars().count();
    (
        out,
        CharSelection {
            primary_char: clamped.primary_char + shift,
            secondary_char: clamped.secondary_char + shift,
        },
    )
}

/// Swaps the touched line block one line up (`move_up = true`) or down (`move_up = false`).
///
/// Returns `None` if the touched lines are already at the top or bottom boundary.
pub fn move_lines(
    text: &str,
    sel: CharSelection,
    move_up: bool,
) -> Option<(String, CharSelection)> {
    let (lines, total_chars) = parse_lines(text);
    let clamped = clamp_selection(sel, total_chars);
    let (first, last) = touched_line_range(&lines, clamped);
    let block_end_old = lines[last].content_end_char;
    let mut out = String::with_capacity(text.len());

    if move_up {
        let above = lines.get(first.checked_sub(1)?)?;
        push_lines(&mut out, &lines[..first - 1]);
        push_lines(&mut out, &lines[first..last]);
        out.push_str(lines[last].content);
        out.push_str(above.newline);
        out.push_str(above.content);
        out.push_str(lines[last].newline);
        push_lines(&mut out, &lines[(last + 1)..]);

        let shift_back = above.content_chars + above.nl_chars;
        let new_block_end = block_end_old - shift_back + above.nl_chars;
        let map = |p: usize| {
            if p > block_end_old {
                new_block_end
            } else {
                p.saturating_sub(shift_back)
            }
        };
        return Some((
            out,
            CharSelection {
                primary_char: map(clamped.primary_char),
                secondary_char: map(clamped.secondary_char),
            },
        ));
    }

    let below = lines.get(last + 1)?;
    push_lines(&mut out, &lines[..first]);
    out.push_str(below.content);
    out.push_str(lines[last].newline);
    push_lines(&mut out, &lines[first..last]);
    out.push_str(lines[last].content);
    out.push_str(below.newline);
    push_lines(&mut out, &lines[(last + 2)..]);

    let shift_fwd = below.content_chars + lines[last].nl_chars;
    let new_block_end = block_end_old + shift_fwd + below.nl_chars;
    let map = |p: usize| {
        if p > block_end_old {
            new_block_end
        } else {
            p + shift_fwd
        }
    };
    Some((
        out,
        CharSelection {
            primary_char: map(clamped.primary_char),
            secondary_char: map(clamped.secondary_char),
        },
    ))
}
