//! Pure Typst IntelliSense engine for reference completions, macro/stdlib documentation,
//! file path suggestions, hover tooltips, and diagnostic hints.
//!
//! Operates strictly in-memory over `&str` buffers, `&[(&Path, &str)]` project sources,
//! and `typst::Library::default()` with zero filesystem or GUI dependencies.

use std::path::{Path, PathBuf};

use typst::syntax::{LinkedNode, SyntaxKind};

use crate::core::diff::normalize_rel_path;

mod complete;
mod hover;
mod index;
mod stdlib;

#[cfg(test)]
#[path = "../tests/core/intellisense_tests.rs"]
mod tests;

/// Semantic category of an IntelliSense completion item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompletionKind {
    /// Cross-reference label (`<label>`) or BibTeX citation key (`@key`).
    Reference,
    /// User-defined `#let` macro or helper function.
    Macro,
    /// Built-in Typst standard library function or element.
    Function,
    /// Typst language keyword (`let`, `set`, `show`, etc.).
    Keyword,
    /// Relative project file path inside `#import`, `#include`, `image`, or `bibliography`.
    FilePath,
    /// User-defined `#let` variable or standard library constant, symbol, or type.
    Variable,
    /// Named parameter of an enclosing function call or set rule.
    Parameter,
}

/// A single candidate returned by Typst IntelliSense completion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionItem {
    /// Display label shown in the completion popup.
    pub label: String,
    /// Text inserted into the editor replacing `replace_start_char..replace_end_char`.
    pub insert_text: String,
    /// Compact parameter signature, target kind, or type summary.
    pub detail: String,
    /// Documentation text or surrounding source context snippet.
    pub documentation: String,
    /// Semantic kind of this completion item.
    pub kind: CompletionKind,
    /// Optional origin hint such as `"main.typ:12"` or `"refs.bib:4"`.
    pub source_hint: Option<String>,
}

/// Active completion session context around the editor cursor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionRequestContext {
    /// Inclusive Unicode scalar (`char`) start index to replace when accepting an item.
    pub replace_start_char: usize,
    /// Exclusive Unicode scalar (`char`) end index to replace when accepting an item.
    pub replace_end_char: usize,
    /// Typed prefix string used for filtering and ranking.
    pub prefix: String,
    /// Filtered and ranked completion candidates.
    pub items: Vec<CompletionItem>,
}

/// Hover tooltip metadata for a symbol, reference, or diagnostic under the cursor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HoverInfo {
    /// Inclusive Unicode scalar (`char`) start offset of the hovered token or span.
    pub start_char: usize,
    /// Exclusive Unicode scalar (`char`) end offset of the hovered token or span.
    pub end_char: usize,
    /// Primary heading or symbol name displayed in the tooltip.
    pub title: String,
    /// Signature, kind, or file location subtitle.
    pub detail: String,
    /// Detailed documentation, context snippet, or diagnostic message.
    pub documentation: String,
    /// `true` when this hover represents a compiler diagnostic or unresolved reference.
    pub is_diagnostic: bool,
}

/// A Typst diagnostic or semantic hint spanning a character range in the active buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypstDiagnosticHint {
    /// Inclusive Unicode scalar (`char`) start offset in the active document.
    pub start_char: usize,
    /// Exclusive Unicode scalar (`char`) end offset in the active document.
    pub end_char: usize,
    /// Human-readable diagnostic or hint message.
    pub message: String,
    /// `true` for errors (including unresolved references), `false` for warnings/hints.
    pub is_error: bool,
}

/// An indexed `<label>` or BibTeX citation target across the project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelDefinition {
    /// Identifier without surrounding `<...>` or `@` (e.g., `"sec-intro"` or `"knuth84"`).
    pub label: String,
    /// Project-relative file path where the target is defined.
    pub file: PathBuf,
    /// 1-based line number of the definition.
    pub line: usize,
    /// Inclusive Unicode scalar (`char`) start offset in `file`.
    pub start_char: usize,
    /// Exclusive Unicode scalar (`char`) end offset in `file`.
    pub end_char: usize,
    /// Human-readable target kind (e.g., `"Heading"`, `"Figure"`, `"Equation"`, `"Article"`).
    pub kind_label: String,
    /// Clean trimmed context snippet around the definition.
    pub context_snippet: String,
}

/// An indexed user-defined `#let` variable or function across the project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserSymbolDefinition {
    /// Bound identifier name.
    pub name: String,
    /// Formatted parameter list if this binding is a function/closure (e.g., `"(title, body)"`).
    pub params: Option<String>,
    /// Project-relative file path where the binding is defined.
    pub file: PathBuf,
    /// 1-based line number of the binding.
    pub line: usize,
    /// Extracted leading `///` or `//` comment block documentation.
    pub documentation: String,
    /// `true` if this binding defines a function or closure.
    pub is_function: bool,
}

/// Computes context-aware Typst completion candidates at `cursor_char` in `active_text`.
pub fn compute_completions(
    active_text: &str,
    cursor_char: usize,
    project_sources: &[(&Path, &str)],
    project_files: &[PathBuf],
) -> Option<CompletionRequestContext> {
    complete::resolve_completions(active_text, cursor_char, project_sources, project_files)
}

/// Computes hover tooltip metadata at `hover_char` in `active_text`.
///
/// Compiler diagnostics covering `hover_char` take priority, followed by unresolved or
/// resolved `@ref` targets, `<label>` definitions, user `#let` symbols, and Typst built-ins.
pub fn compute_hover_info(
    active_text: &str,
    hover_char: usize,
    project_sources: &[(&Path, &str)],
    compiler_diagnostics: &[(usize, usize, String, bool)],
) -> Option<HoverInfo> {
    hover::resolve_hover_info(
        active_text,
        hover_char,
        project_sources,
        compiler_diagnostics,
    )
}

/// Computes semantic diagnostic hints for unresolved `@ref` usages plus compiler diagnostics.
pub fn compute_typst_hints(
    active_text: &str,
    project_sources: &[(&Path, &str)],
    compiler_diagnostics: &[(usize, usize, String, bool)],
) -> Vec<TypstDiagnosticHint> {
    hover::resolve_typst_hints(active_text, project_sources, compiler_diagnostics)
}

pub(super) fn find_ancestor_kind<'a>(
    node: &LinkedNode<'a>,
    kind: SyntaxKind,
) -> Option<LinkedNode<'a>> {
    let mut cur = Some(node.clone());
    while let Some(n) = cur {
        if n.kind() == kind {
            return Some(n);
        }
        cur = n.parent().cloned();
    }
    None
}

pub(super) fn matches_prefix(candidate: &str, prefix_lower: &str) -> bool {
    prefix_lower.is_empty() || candidate.to_ascii_lowercase().starts_with(prefix_lower)
}

pub(super) fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}

pub(super) fn is_ref_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | ':' | '-')
}

pub(super) fn char_to_byte_offset(text: &str, char_idx: usize) -> usize {
    text.char_indices()
        .nth(char_idx)
        .map(|(b, _)| b)
        .unwrap_or(text.len())
}

pub(super) fn line_number_at_byte(text: &str, byte_offset: usize) -> usize {
    let mut b = byte_offset.min(text.len());
    while b > 0 && !text.is_char_boundary(b) {
        b -= 1;
    }
    text[..b].bytes().filter(|&ch| ch == b'\n').count() + 1
}

pub(super) fn normalize_display_path(path: &Path) -> String {
    normalize_rel_path(&path.to_string_lossy().replace('\\', "/"))
}
