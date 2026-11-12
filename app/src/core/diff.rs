//! Pure text differencing and UTF-16 code-unit conversion utilities.
//!
//! Provides conversion between Unicode scalar (`char`) counts used by editor widgets
//! and UTF-16 code units used by CRDT text models (such as `yrs`). Also provides
//! path normalization and deterministic collision path resolution.

use std::path::Path;

#[cfg(test)]
mod tests;

/// Minimal text edit operation expressed in `yrs` UTF-16 code-unit offsets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Utf16TextDiff {
    pub utf16_index: u32,
    pub utf16_remove_len: u32,
    pub insert_text: String,
}

/// Cursor and selection range expressed in Unicode scalar `char` indices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CharSelection {
    pub primary_char: usize,
    pub secondary_char: usize,
}

impl CharSelection {
    pub fn caret(char_idx: usize) -> Self {
        Self {
            primary_char: char_idx,
            secondary_char: char_idx,
        }
    }
}

/// Converts a Unicode scalar `char` index into a UTF-16 code unit offset.
pub fn char_to_utf16_offset(s: &str, char_idx: usize) -> u32 {
    s.chars().take(char_idx).map(|c| c.len_utf16() as u32).sum()
}

/// Converts a UTF-16 code unit offset back into a Unicode scalar `char` index.
pub fn utf16_to_char_offset(s: &str, utf16_offset: u32) -> usize {
    let mut accumulated_utf16 = 0u32;
    let mut char_count = 0usize;
    for ch in s.chars() {
        if accumulated_utf16 >= utf16_offset {
            break;
        }
        accumulated_utf16 += ch.len_utf16() as u32;
        char_count += 1;
    }
    char_count
}

/// Computes the minimal single-span difference between `old_text` and `new_text` using
/// common prefix and common suffix over Unicode scalar values (`char`), and returns the
/// corresponding UTF-16 offset, removal length, and inserted substring.
pub fn compute_utf16_diff(old_text: &str, new_text: &str) -> Option<Utf16TextDiff> {
    if old_text == new_text {
        return None;
    }

    let old_chars: Vec<char> = old_text.chars().collect();
    let new_chars: Vec<char> = new_text.chars().collect();

    let mut prefix_chars = 0usize;
    let min_len = old_chars.len().min(new_chars.len());
    while prefix_chars < min_len && old_chars[prefix_chars] == new_chars[prefix_chars] {
        prefix_chars += 1;
    }

    let mut old_suffix_end = old_chars.len();
    let mut new_suffix_end = new_chars.len();
    while old_suffix_end > prefix_chars
        && new_suffix_end > prefix_chars
        && old_chars[old_suffix_end - 1] == new_chars[new_suffix_end - 1]
    {
        old_suffix_end -= 1;
        new_suffix_end -= 1;
    }

    let utf16_index: u32 = old_chars[..prefix_chars]
        .iter()
        .map(|c| c.len_utf16() as u32)
        .sum();
    let utf16_remove_len: u32 = old_chars[prefix_chars..old_suffix_end]
        .iter()
        .map(|c| c.len_utf16() as u32)
        .sum();
    let insert_text: String = new_chars[prefix_chars..new_suffix_end].iter().collect();

    Some(Utf16TextDiff {
        utf16_index,
        utf16_remove_len,
        insert_text,
    })
}

/// Normalizes a relative path to forward slashes without leading slashes.
pub fn normalize_rel_path(path: &str) -> String {
    path.replace('\\', "/")
        .trim_start_matches('/')
        .trim()
        .to_string()
}

/// Deterministic collision suffix helper: when two `file_id`s claim the same relative path
/// concurrently, the lexicographically smallest `file_id` keeps the original path and any
/// subsequent `file_id` receives a deterministic `(conflict-<short_id>)` suffix before the extension.
pub fn format_collision_path(raw_path: &str, file_id: &str) -> String {
    let short_id: String = file_id.chars().take(6).collect();
    let path = Path::new(raw_path);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
    let ext = path.extension().and_then(|e| e.to_str());
    let parent = path
        .parent()
        .and_then(|p| p.to_str())
        .filter(|p| !p.is_empty());

    let file_name = match ext {
        Some(e) => format!("{}.conflict-{}.{}", stem, short_id, e),
        None => format!("{}.conflict-{}", stem, short_id),
    };

    match parent {
        Some(p) => format!("{}/{}", p.replace('\\', "/"), file_name),
        None => file_name,
    }
}
