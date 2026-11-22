//! Pure char-indexed search and replace utilities.
//!
//! Operates strictly in-memory on `&str` using Unicode scalar (`char`) indices
//! for seamless integration with editor selections and CRDT offsets.

#[cfg(test)]
#[path = "../tests/core/search_tests.rs"]
mod tests;

/// A single char-indexed match span `[start_char, end_char)` within a document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchMatch {
    /// Inclusive starting index in Unicode scalar `char` units.
    pub start_char: usize,
    /// Exclusive ending index in Unicode scalar `char` units.
    pub end_char: usize,
}

/// Configuration options controlling search matching behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SearchOptions {
    /// Whether character matching distinguishes uppercase from lowercase.
    pub case_sensitive: bool,
}

/// Finds all non-overlapping char-indexed occurrences of `query` in `text`.
///
/// Returns an empty vector when `query` is empty. Handles Unicode case folding
/// (including multi-character expansions such as `'İ'` -> `"i\u{0307}"`) while
/// preserving exact original `char` offsets.
pub fn find_all_matches(text: &str, query: &str, opts: SearchOptions) -> Vec<SearchMatch> {
    if query.is_empty() {
        return Vec::new();
    }

    if opts.case_sensitive {
        return find_case_sensitive(text, query);
    }

    find_case_insensitive(text, query)
}

fn find_case_sensitive(text: &str, query: &str) -> Vec<SearchMatch> {
    let byte_matches: Vec<(usize, usize)> = text
        .match_indices(query)
        .map(|(start, slice)| (start, start + slice.len()))
        .collect();
    if byte_matches.is_empty() {
        return Vec::new();
    }

    let mut out = Vec::with_capacity(byte_matches.len());
    let mut match_idx = 0usize;
    let mut current_start_char = 0usize;
    let mut has_open_start = false;

    for (char_idx, (byte_idx, _)) in text
        .char_indices()
        .chain(std::iter::once((text.len(), '\0')))
        .enumerate()
    {
        if has_open_start && byte_idx == byte_matches[match_idx].1 {
            out.push(SearchMatch {
                start_char: current_start_char,
                end_char: char_idx,
            });
            has_open_start = false;
            match_idx += 1;
            if match_idx == byte_matches.len() {
                break;
            }
        }
        if !has_open_start && byte_idx == byte_matches[match_idx].0 {
            current_start_char = char_idx;
            has_open_start = true;
        }
    }

    out
}

fn find_case_insensitive(text: &str, query: &str) -> Vec<SearchMatch> {
    let folded_query: String = query.chars().flat_map(char::to_lowercase).collect();
    if folded_query.is_empty() {
        return Vec::new();
    }

    let mut folded_text = String::with_capacity(text.len());
    let mut boundary_map: Vec<Option<usize>> = Vec::with_capacity(text.len() + 1);
    let mut orig_char_count = 0usize;

    for (orig_idx, ch) in text.chars().enumerate() {
        orig_char_count = orig_idx + 1;
        let start_len = folded_text.len();
        for lc in ch.to_lowercase() {
            folded_text.push(lc);
        }
        let added = folded_text.len() - start_len;
        boundary_map.push(Some(orig_idx));
        boundary_map.resize(boundary_map.len() + added - 1, None);
    }
    boundary_map.push(Some(orig_char_count));

    let mut out = Vec::new();
    let mut pos = 0usize;
    while let Some(rel) = folded_text[pos..].find(&folded_query) {
        let abs_start = pos + rel;
        let abs_end = abs_start + folded_query.len();
        if let (Some(start_char), Some(end_char)) = (boundary_map[abs_start], boundary_map[abs_end])
        {
            out.push(SearchMatch {
                start_char,
                end_char,
            });
            pos = abs_end;
        } else {
            let step = folded_text[abs_start..]
                .chars()
                .next()
                .map_or(1, char::len_utf8);
            pos = abs_start + step;
        }
    }

    out
}

/// Returns the index in `matches` of the next match relative to `cursor_char`.
///
/// Wraps around cleanly in both directions (`forward = true` or `false`), and
/// returns `None` when `matches` is empty.
pub fn find_next_match_index(
    matches: &[SearchMatch],
    cursor_char: usize,
    forward: bool,
) -> Option<usize> {
    if matches.is_empty() {
        return None;
    }

    if forward {
        matches
            .iter()
            .position(|m| m.start_char >= cursor_char)
            .or(Some(0))
    } else {
        matches
            .iter()
            .rposition(|m| m.start_char < cursor_char)
            .or_else(|| Some(matches.len() - 1))
    }
}

/// Replaces the char range `[m.start_char, m.end_char)` in `text` with `replacement`.
///
/// Returns `(new_text, m.start_char + replacement.chars().count())`, clamping out-of-bounds
/// offsets to the document length.
pub fn replace_match_at(text: &str, m: SearchMatch, replacement: &str) -> (String, usize) {
    let mut start_byte = text.len();
    let mut end_byte = text.len();
    let mut actual_start_char = None;
    let clamped_end_target = m.end_char.max(m.start_char);
    let mut total_chars = 0usize;

    for (char_idx, (byte_idx, _)) in text
        .char_indices()
        .chain(std::iter::once((text.len(), '\0')))
        .enumerate()
    {
        total_chars = char_idx;
        if actual_start_char.is_none() && char_idx == m.start_char {
            start_byte = byte_idx;
            actual_start_char = Some(char_idx);
        }
        if char_idx == clamped_end_target {
            end_byte = byte_idx;
            break;
        }
    }

    let start_char = actual_start_char.unwrap_or(total_chars);
    let mut out = String::with_capacity(text.len() - (end_byte - start_byte) + replacement.len());
    out.push_str(&text[..start_byte]);
    out.push_str(replacement);
    out.push_str(&text[end_byte..]);

    (out, start_char + replacement.chars().count())
}

/// Replaces all non-overlapping matches of `query` in `text` with `replacement` in one pass.
///
/// Returns `Some((new_text, match_count))` if at least one match was found, or `None`
/// when `query` is empty or yields zero matches.
pub fn replace_all_matches(
    text: &str,
    query: &str,
    replacement: &str,
    opts: SearchOptions,
) -> Option<(String, usize)> {
    let matches = find_all_matches(text, query, opts);
    if matches.is_empty() {
        return None;
    }

    let byte_spans = char_matches_to_byte_spans(text, &matches);
    let mut out = String::with_capacity(text.len());
    let mut cursor = 0usize;
    for (start_byte, end_byte) in byte_spans {
        out.push_str(&text[cursor..start_byte]);
        out.push_str(replacement);
        cursor = end_byte;
    }
    out.push_str(&text[cursor..]);

    Some((out, matches.len()))
}

fn char_matches_to_byte_spans(text: &str, matches: &[SearchMatch]) -> Vec<(usize, usize)> {
    let mut spans = Vec::with_capacity(matches.len());
    let mut idx = 0usize;
    let mut current_start_byte = 0usize;
    let mut has_open = false;

    for (char_idx, (byte_idx, _)) in text
        .char_indices()
        .chain(std::iter::once((text.len(), '\0')))
        .enumerate()
    {
        if has_open && char_idx == matches[idx].end_char {
            spans.push((current_start_byte, byte_idx));
            has_open = false;
            idx += 1;
            if idx == matches.len() {
                break;
            }
        }
        if !has_open && char_idx == matches[idx].start_char {
            current_start_byte = byte_idx;
            has_open = true;
        }
    }

    spans
}
