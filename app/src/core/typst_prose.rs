//! Pure Typst AST prose extraction, `#set text(lang: ...)` detection, and byte-preserving masking.
//!
//! Produces a 1:1 UTF-8 byte-length shadow buffer where Typst markup delimiters, code expressions,
//! math equations, raw blocks, comments, labels, and references are masked with ASCII spaces
//! (preserving `\n`), while prose and `[...]` content blocks remain at their exact source offsets.

use typst::syntax::{ast, LinkedNode, SyntaxKind};

#[cfg(test)]
#[path = "../tests/core/typst_prose_tests.rs"]
mod tests;

/// Result of extracting prose from a Typst source file with 1:1 UTF-8 byte alignment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypstProseMask {
    /// Shadow string of the exact same UTF-8 byte length as the original source text.
    pub masked_text: String,
    /// Per-byte flag indicating whether the byte came directly from prose (`true`) or was masked (`false`).
    pub prose_bytes: Vec<bool>,
}

/// A contiguous non-empty prose paragraph inside a masked Typst document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProseParagraph {
    /// UTF-8 byte offset of the paragraph start within the full source document.
    pub start_byte: usize,
    /// UTF-8 byte offset of the paragraph end within the full source document.
    pub end_byte: usize,
    /// Paragraph substring (matching `masked_text[start_byte..end_byte]`).
    pub text: String,
    /// Stable 64-bit FNV-1a hash of `text` for incremental paragraph caching.
    pub hash: u64,
}

/// Normalizes a language/locale tag (e.g. `"de-DE"`, `"en_US"`, `"FR"`) into a lowercase
/// primary ISO 639-1 code (`"de"`, `"en"`, `"fr"`), or a lowercase underscore tag if longer.
pub fn normalize_language_code(tag: &str) -> Option<String> {
    let trimmed = tag.trim();
    if trimmed.is_empty() {
        return None;
    }
    let primary = trimmed.split(['-', '_']).next().unwrap_or(trimmed).trim();
    if primary.is_empty() {
        return None;
    }
    Some(primary.to_ascii_lowercase())
}

/// Normalizes a full locale/language identifier into lowercase underscore format (e.g. `"pt-BR"` -> `"pt_br"`).
pub fn normalize_full_language_tag(tag: &str) -> Option<String> {
    let trimmed = tag.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.replace('-', "_").to_ascii_lowercase())
}

/// Inspects the Typst AST for `#set text(lang: "...")` and returns the normalized language code.
pub fn detect_typst_document_language(source: &str) -> Option<String> {
    let root = typst::syntax::parse(source);
    let linked = LinkedNode::new(&root);
    find_set_text_lang(&linked)
}

fn find_set_text_lang(node: &LinkedNode) -> Option<String> {
    if node.kind() == SyntaxKind::SetRule {
        if let Some(set_rule) = node.cast::<ast::SetRule>() {
            if let ast::Expr::Ident(ident) = set_rule.target() {
                if ident.as_str() == "text" {
                    for arg in set_rule.args().items() {
                        if let ast::Arg::Named(named) = arg {
                            if named.name().as_str() == "lang" {
                                if let ast::Expr::Str(s) = named.expr() {
                                    if let Some(code) = normalize_full_language_tag(&s.get()) {
                                        return Some(code);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    for child in node.children() {
        if let Some(found) = find_set_text_lang(&child) {
            return Some(found);
        }
    }
    None
}

/// Builds a 1:1 byte-aligned whitespace-masked shadow buffer from Typst source text.
pub fn extract_typst_prose_mask(source: &str) -> TypstProseMask {
    let root = typst::syntax::parse(source);
    let linked = LinkedNode::new(&root);
    let mut out = Vec::with_capacity(source.len());
    let mut prose_bytes = Vec::with_capacity(source.len());

    visit_prose_node(&linked, source, true, &mut out, &mut prose_bytes);

    debug_assert_eq!(out.len(), source.len());
    debug_assert_eq!(prose_bytes.len(), source.len());

    let masked_text = String::from_utf8(out).expect("masked shadow buffer must remain valid UTF-8");
    TypstProseMask {
        masked_text,
        prose_bytes,
    }
}

fn visit_prose_node(
    node: &LinkedNode,
    source: &str,
    in_prose: bool,
    out: &mut Vec<u8>,
    prose_bytes: &mut Vec<bool>,
) {
    let kind = node.kind();

    // Non-prose subtrees are masked wholesale so syntax/code/math never leak into prose checks.
    if matches!(
        kind,
        SyntaxKind::Equation
            | SyntaxKind::Raw
            | SyntaxKind::LineComment
            | SyntaxKind::BlockComment
            | SyntaxKind::Label
            | SyntaxKind::Ref
            | SyntaxKind::RefMarker
            | SyntaxKind::Link
    ) {
        if let Some(slice) = source.get(node.range()) {
            push_masked_slice(slice, out, prose_bytes);
        }
        return;
    }

    // Content blocks `[...]` switch back into prose mode for their inner Markup,
    // allowing `#emph[important]` or `#footnote[...]` to be checked cleanly.
    let child_in_prose = match kind {
        SyntaxKind::Markup
        | SyntaxKind::ContentBlock
        | SyntaxKind::Heading
        | SyntaxKind::Strong
        | SyntaxKind::Emph
        | SyntaxKind::ListItem
        | SyntaxKind::EnumItem
        | SyntaxKind::TermItem => true,
        SyntaxKind::CodeBlock
        | SyntaxKind::FuncCall
        | SyntaxKind::SetRule
        | SyntaxKind::ShowRule
        | SyntaxKind::LetBinding
        | SyntaxKind::ModuleImport
        | SyntaxKind::ModuleInclude
        | SyntaxKind::Conditional
        | SyntaxKind::ForLoop
        | SyntaxKind::WhileLoop => false,
        _ => in_prose,
    };

    let mut has_children = false;
    for child in node.children() {
        has_children = true;
        visit_prose_node(&child, source, child_in_prose, out, prose_bytes);
    }

    if has_children {
        return;
    }

    let Some(slice) = source.get(node.range()) else {
        return;
    };

    let keep_leaf = in_prose
        && matches!(
            kind,
            SyntaxKind::Text | SyntaxKind::Space | SyntaxKind::Parbreak | SyntaxKind::SmartQuote
        );

    if keep_leaf {
        out.extend_from_slice(slice.as_bytes());
        prose_bytes.resize(prose_bytes.len() + slice.len(), true);
    } else {
        push_masked_slice(slice, out, prose_bytes);
    }
}

fn push_masked_slice(slice: &str, out: &mut Vec<u8>, prose_bytes: &mut Vec<bool>) {
    for &b in slice.as_bytes() {
        if b == b'\n' {
            out.push(b'\n');
        } else {
            out.push(b' ');
        }
        prose_bytes.push(false);
    }
}

/// Splits a masked Typst shadow buffer into paragraph slices separated by blank lines.
///
/// Paragraphs containing no alphabetic characters (e.g. masked-only lines) are skipped.
pub fn split_prose_paragraphs(masked_text: &str) -> Vec<ProseParagraph> {
    let mut paragraphs = Vec::new();
    let bytes = masked_text.as_bytes();
    let len = bytes.len();
    let mut pos = 0usize;
    let mut para_start: Option<usize> = None;
    let mut para_end: usize = 0;

    while pos < len {
        let line_start = pos;
        let mut line_end = pos;
        while line_end < len && bytes[line_end] != b'\n' {
            line_end += 1;
        }
        let line_is_blank = masked_text[line_start..line_end].trim().is_empty();

        if line_is_blank {
            if let Some(start) = para_start.take() {
                push_paragraph_if_prose(masked_text, start, para_end, &mut paragraphs);
            }
        } else {
            if para_start.is_none() {
                para_start = Some(line_start);
            }
            para_end = line_end;
        }

        pos = if line_end < len { line_end + 1 } else { len };
    }

    if let Some(start) = para_start {
        push_paragraph_if_prose(masked_text, start, para_end, &mut paragraphs);
    }

    paragraphs
}

fn push_paragraph_if_prose(
    masked_text: &str,
    start_byte: usize,
    end_byte: usize,
    out: &mut Vec<ProseParagraph>,
) {
    let slice = &masked_text[start_byte..end_byte];
    if !slice.chars().any(|c| c.is_alphabetic()) {
        return;
    }
    out.push(ProseParagraph {
        start_byte,
        end_byte,
        text: slice.to_string(),
        hash: fnv1a_64(slice.as_bytes()),
    });
}

/// Returns `true` if the given byte range (expanded by 1 byte on each side) touches masked Typst syntax.
pub fn span_touches_masked_syntax(
    prose_bytes: &[bool],
    start_byte: usize,
    end_byte: usize,
) -> bool {
    let lo = start_byte.saturating_sub(1);
    let hi = (end_byte + 1).min(prose_bytes.len());
    if lo >= hi {
        return false;
    }
    prose_bytes[lo..hi].iter().any(|&is_prose| !is_prose)
}

/// Converts a UTF-8 byte range inside `text` to a Unicode scalar (`char`) range `(start_char, end_char)`.
pub fn byte_span_to_char_range(text: &str, start_byte: usize, end_byte: usize) -> (usize, usize) {
    let clamped_start = clamp_to_char_boundary(text, start_byte);
    let clamped_end = clamp_to_char_boundary(text, end_byte.max(clamped_start));
    let start_char = text[..clamped_start].chars().count();
    let span_chars = text[clamped_start..clamped_end].chars().count();
    (start_char, start_char + span_chars)
}

/// Clamps `byte_idx` down to the nearest valid UTF-8 character boundary within `text`.
pub fn clamp_to_char_boundary(text: &str, byte_idx: usize) -> usize {
    let mut idx = byte_idx.min(text.len());
    while idx > 0 && !text.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

fn fnv1a_64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in bytes {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}
