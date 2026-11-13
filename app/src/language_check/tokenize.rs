//! Word tokenization and UTF-8 byte-to-char/line-column source offset mapping.

/// Tokenizes prose into alphabetic word spans `(start_byte, end_byte, slice)` suitable for Hunspell.
pub fn tokenize_prose_words(text: &str) -> Vec<(usize, usize, &str)> {
    let mut words = Vec::new();
    let mut start: Option<usize> = None;

    for (idx, ch) in text.char_indices() {
        let is_word_char = ch.is_alphanumeric() || matches!(ch, '\'' | '\u{2019}' | '-');
        if is_word_char {
            if start.is_none() {
                start = Some(idx);
            }
        } else if let Some(s) = start.take() {
            push_filtered_word(text, s, idx, &mut words);
        }
    }
    if let Some(s) = start {
        push_filtered_word(text, s, text.len(), &mut words);
    }
    words
}

fn push_filtered_word<'a>(
    text: &'a str,
    raw_start: usize,
    raw_end: usize,
    out: &mut Vec<(usize, usize, &'a str)>,
) {
    let mut slice = &text[raw_start..raw_end];
    let mut start = raw_start;
    let mut end = raw_end;

    while let Some(first) = slice.chars().next() {
        if matches!(first, '\'' | '\u{2019}' | '-') {
            let step = first.len_utf8();
            start += step;
            slice = &slice[step..];
        } else {
            break;
        }
    }
    while let Some(last) = slice.chars().next_back() {
        if matches!(last, '\'' | '\u{2019}' | '-') {
            let step = last.len_utf8();
            end -= step;
            slice = &slice[..slice.len() - step];
        } else {
            break;
        }
    }

    if slice.chars().count() < 2 {
        return;
    }
    // Skip numbers/versions/identifiers containing digits (e.g. `2026`, `v0`, `UTF-8`, `PDF/A`).
    if slice.chars().any(|c| c.is_ascii_digit()) {
        return;
    }
    if !slice
        .chars()
        .all(|c| c.is_alphabetic() || matches!(c, '\'' | '\u{2019}' | '-'))
    {
        return;
    }
    out.push((start, end, slice));
}

/// Maps UTF-8 byte offsets to Unicode scalar offsets and 1-based `(line, column)`.
pub(crate) struct SourceOffsetMapper {
    byte_to_char: Vec<usize>,
    line_starts: Vec<usize>,
}

impl SourceOffsetMapper {
    pub(crate) fn new(source: &str) -> Self {
        let mut byte_to_char = vec![0usize; source.len() + 1];
        let mut line_starts = vec![0usize];
        let mut char_count = 0usize;

        for (byte_idx, ch) in source.char_indices() {
            let next_byte = byte_idx + ch.len_utf8();
            for slot in &mut byte_to_char[byte_idx..next_byte] {
                *slot = char_count;
            }
            char_count += 1;
            if ch == '\n' {
                line_starts.push(next_byte);
            }
        }
        byte_to_char[source.len()] = char_count;

        Self {
            byte_to_char,
            line_starts,
        }
    }

    pub(crate) fn map_range(
        &self,
        start_byte: usize,
        end_byte: usize,
    ) -> (usize, usize, usize, usize) {
        let max_b = self.byte_to_char.len().saturating_sub(1);
        let sb = start_byte.min(max_b);
        let eb = end_byte.min(max_b);
        let start_char = self.byte_to_char[sb];
        let end_char = self.byte_to_char[eb];

        let line_idx = match self.line_starts.binary_search(&sb) {
            Ok(idx) => idx,
            Err(ins) => ins.saturating_sub(1),
        };
        let line_start_byte = self.line_starts[line_idx];
        let col_chars = start_char.saturating_sub(self.byte_to_char[line_start_byte]);

        (start_char, end_char, line_idx + 1, col_chars + 1)
    }
}
