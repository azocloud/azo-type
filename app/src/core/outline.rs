//! Pure document outline extraction, AST traversal, and hierarchical numbering.
//!
//! Extracts headings from Typst source files (including recursive `#include` statements
//! and `#heading(level: ...)` calls), BibTeX entries, and Markdown headings into a
//! unified hierarchical outline tree.

use std::path::{Path, PathBuf};

use typst::syntax::ast::AstNode;
use typst::syntax::{ast, LinkedNode, SyntaxKind, SyntaxNode};

use crate::core::diff::normalize_rel_path;
use crate::core::file_kind::FileKind;

#[cfg(test)]
mod tests;

/// Trait abstracting source file resolution for outline extraction.
///
/// This decouples pure outline parsing from filesystem I/O and disk projects.
pub trait SourceResolver {
    /// Reads the textual content of a relative source path.
    fn read_source_text(&self, path: &Path) -> Result<String, String>;

    /// Returns whether the specified file is the primary compile entrypoint.
    fn is_main_entrypoint(&self, path: &Path) -> bool;
}

/// Outline item extracted from compiled Typst document introspection.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CompiledOutlineItem {
    /// Heading nesting depth (1-based).
    pub level: usize,
    /// Optional compiled section numbering string.
    pub numbers: Option<String>,
    /// Plain-text heading title.
    pub title: String,
    /// Relative path of the source file containing this heading.
    pub file: PathBuf,
    /// 1-based line number in the source file.
    pub line: usize,
    /// Character offset from the start of the source file.
    pub char_offset: usize,
}

/// A single heading or navigable entry extracted from an open document or project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentOutlineItem {
    /// Heading nesting depth (1-based).
    pub level: usize,
    /// Hierarchical section number label (e.g. `"1.2.1"`).
    pub number_label: String,
    /// Plain-text heading title.
    pub title: String,
    /// Relative path of the source file containing this entry.
    pub file: PathBuf,
    /// 1-based line number in the source file.
    pub line_number: usize,
    /// Character offset from the start of the source file.
    pub char_offset: usize,
}

/// Tree node representing a document heading with nested child sections.
#[derive(Debug, Clone)]
pub struct OutlineTreeNode {
    /// The heading entry data for this node.
    pub item: DocumentOutlineItem,
    /// Whether the editor cursor is currently inside this section.
    pub is_current: bool,
    /// Nested sub-section nodes.
    pub children: Vec<OutlineTreeNode>,
}

/// Recursively extracts human-readable plain text from a Typst syntax node.
pub fn extract_typst_markup_plain_text(node: &SyntaxNode) -> String {
    fn visit(n: &SyntaxNode, out: &mut String) {
        match n.kind() {
            SyntaxKind::LineComment
            | SyntaxKind::BlockComment
            | SyntaxKind::Label
            | SyntaxKind::HeadingMarker => {}
            SyntaxKind::Text => out.push_str(n.leaf_text().as_str()),
            SyntaxKind::Space | SyntaxKind::Parbreak | SyntaxKind::Linebreak => {
                if !out.ends_with(' ') && !out.is_empty() {
                    out.push(' ');
                }
            }
            SyntaxKind::SmartQuote
            | SyntaxKind::Shorthand
            | SyntaxKind::Escape
            | SyntaxKind::MathText
            | SyntaxKind::MathIdent
            | SyntaxKind::Int
            | SyntaxKind::Float
            | SyntaxKind::Numeric => out.push_str(n.leaf_text().as_str()),
            SyntaxKind::Str => {
                out.push_str(n.leaf_text().trim_matches('"'));
            }
            _ => {
                for child in n.children() {
                    visit(child, out);
                }
            }
        }
    }
    let mut raw = String::new();
    visit(node, &mut raw);
    raw.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Traverses a Typst AST to extract heading items, handling `#include` statements recursively.
pub fn extract_typst_ast_outline(
    text: &str,
    current_file: &Path,
    source_resolver: Option<&dyn SourceResolver>,
    visited: &mut Vec<PathBuf>,
) -> Vec<DocumentOutlineItem> {
    let root = typst::syntax::parse(text);
    let linked = LinkedNode::new(&root);
    let mut items = Vec::new();

    fn walk(
        node: &LinkedNode<'_>,
        text: &str,
        current_file: &Path,
        source_resolver: Option<&dyn SourceResolver>,
        visited: &mut Vec<PathBuf>,
        items: &mut Vec<DocumentOutlineItem>,
    ) {
        match node.kind() {
            SyntaxKind::Heading => {
                if let Some(heading) = node.cast::<ast::Heading>() {
                    let level = heading.depth().get();
                    let title = extract_typst_markup_plain_text(heading.body().to_untyped());
                    if !title.is_empty() {
                        let byte_offset = node.offset().min(text.len());
                        let char_offset = text[..byte_offset].chars().count();
                        let line_number =
                            text[..byte_offset].bytes().filter(|&b| b == b'\n').count() + 1;
                        items.push(DocumentOutlineItem {
                            level,
                            number_label: String::new(),
                            title,
                            file: current_file.to_path_buf(),
                            line_number,
                            char_offset,
                        });
                    }
                }
                return;
            }
            SyntaxKind::FuncCall => {
                if let Some(call) = node.cast::<ast::FuncCall>() {
                    let callee_txt = call.callee().to_untyped().full_text();
                    if callee_txt == "heading" || callee_txt.ends_with(".heading") {
                        let mut level = 1usize;
                        let mut body_title = String::new();
                        for arg in call.args().items() {
                            match arg {
                                ast::Arg::Named(named) => {
                                    if named.name().as_str() == "level" {
                                        if let ast::Expr::Int(i) = named.expr() {
                                            level = (i.get() as usize).max(1);
                                        }
                                    }
                                }
                                ast::Arg::Pos(expr) => {
                                    let candidate = match expr {
                                        ast::Expr::ContentBlock(cb) => {
                                            extract_typst_markup_plain_text(cb.body().to_untyped())
                                        }
                                        ast::Expr::Str(s) => s.get().to_string(),
                                        other => {
                                            extract_typst_markup_plain_text(other.to_untyped())
                                        }
                                    };
                                    if !candidate.is_empty() {
                                        body_title = candidate;
                                    }
                                }
                                _ => {}
                            }
                        }
                        if !body_title.is_empty() {
                            let byte_offset = node.offset().min(text.len());
                            let char_offset = text[..byte_offset].chars().count();
                            let line_number =
                                text[..byte_offset].bytes().filter(|&b| b == b'\n').count() + 1;
                            items.push(DocumentOutlineItem {
                                level,
                                number_label: String::new(),
                                title: body_title,
                                file: current_file.to_path_buf(),
                                line_number,
                                char_offset,
                            });
                            return;
                        }
                    }
                }
            }
            SyntaxKind::ModuleInclude => {
                if let (Some(inc), Some(resolver)) =
                    (node.cast::<ast::ModuleInclude>(), source_resolver)
                {
                    if let ast::Expr::Str(path_lit) = inc.source() {
                        let raw_rel = path_lit.get();
                        let resolved = if let Some(stripped) = raw_rel.strip_prefix('/') {
                            PathBuf::from(stripped)
                        } else {
                            current_file
                                .parent()
                                .map(|p| p.join(raw_rel.as_str()))
                                .unwrap_or_else(|| PathBuf::from(raw_rel.as_str()))
                        };
                        let norm_rel =
                            PathBuf::from(normalize_rel_path(&resolved.to_string_lossy()));
                        if !visited.contains(&norm_rel) {
                            visited.push(norm_rel.clone());
                            if let Ok(inc_text) = resolver.read_source_text(&norm_rel) {
                                let sub_items = extract_typst_ast_outline(
                                    &inc_text,
                                    &norm_rel,
                                    Some(resolver),
                                    visited,
                                );
                                items.extend(sub_items);
                            }
                        }
                    }
                }
                return;
            }
            SyntaxKind::ShowRule | SyntaxKind::SetRule => {
                // Do not treat styling rules like `show heading.where(level: 1): ...` as document headings
                return;
            }
            _ => {}
        }

        for child in node.children() {
            walk(&child, text, current_file, source_resolver, visited, items);
        }
    }

    walk(
        &linked,
        text,
        current_file,
        source_resolver,
        visited,
        &mut items,
    );
    items
}

/// Assigns hierarchical numeric labels (e.g., "1", "1.1", "1.1.2") to outline items in place.
pub fn assign_hierarchical_numbers(items: &mut [DocumentOutlineItem]) {
    let mut counters: Vec<usize> = Vec::new();
    for item in items.iter_mut() {
        let lvl = item.level.clamp(1, 6);
        if counters.len() < lvl {
            counters.resize(lvl, 0);
        } else {
            counters.truncate(lvl);
        }
        counters[lvl - 1] += 1;
        for c in &mut counters[..lvl] {
            if *c == 0 {
                *c = 1;
            }
        }
        if item.number_label.is_empty() {
            item.number_label = counters
                .iter()
                .map(|c| c.to_string())
                .collect::<Vec<_>>()
                .join(".");
        }
    }
}

/// Converts a linear slice of outline items into a nested tree structure.
pub fn build_outline_tree(
    items: &[DocumentOutlineItem],
    active_idx: Option<usize>,
) -> Vec<OutlineTreeNode> {
    fn build_slice(
        slice: &[(usize, &DocumentOutlineItem)],
        active_idx: Option<usize>,
    ) -> Vec<OutlineTreeNode> {
        let mut nodes = Vec::new();
        let mut i = 0;
        while i < slice.len() {
            let (idx, curr) = slice[i];
            let mut j = i + 1;
            while j < slice.len() && slice[j].1.level > curr.level {
                j += 1;
            }
            let children = build_slice(&slice[i + 1..j], active_idx);
            nodes.push(OutlineTreeNode {
                item: curr.clone(),
                is_current: active_idx == Some(idx),
                children,
            });
            i = j;
        }
        nodes
    }

    let indexed: Vec<(usize, &DocumentOutlineItem)> = items.iter().enumerate().collect();
    build_slice(&indexed, active_idx)
}

/// Extracts document outline items based on file kind, blending live AST headings with
/// compiled introspection data when available.
pub fn extract_document_outline(
    text: &str,
    kind: FileKind,
    current_file: &Path,
    source_resolver: Option<&dyn SourceResolver>,
    compiled_outline: &[CompiledOutlineItem],
) -> Vec<DocumentOutlineItem> {
    let mut items = match kind {
        FileKind::Typst => {
            let mut visited = vec![current_file.to_path_buf()];
            let mut ast_items =
                extract_typst_ast_outline(text, current_file, source_resolver, &mut visited);

            let is_main_entrypoint = source_resolver
                .map(|r| r.is_main_entrypoint(current_file))
                .unwrap_or(false);
            if is_main_entrypoint && !compiled_outline.is_empty() {
                let mut merged = Vec::with_capacity(compiled_outline.len().max(ast_items.len()));
                let mut used_ast = vec![false; ast_items.len()];
                for comp in compiled_outline {
                    if let Some((ast_idx, ast_match)) =
                        ast_items.iter().enumerate().find(|(i, a)| {
                            !used_ast[*i]
                                && a.file == comp.file
                                && (a.title == comp.title || a.line_number == comp.line)
                        })
                    {
                        used_ast[ast_idx] = true;
                        let mut item = ast_match.clone();
                        item.level = comp.level;
                        if let Some(ref num) = comp.numbers {
                            item.number_label = num.clone();
                        }
                        merged.push(item);
                    } else {
                        merged.push(DocumentOutlineItem {
                            level: comp.level,
                            number_label: comp.numbers.clone().unwrap_or_default(),
                            title: comp.title.clone(),
                            file: comp.file.clone(),
                            line_number: comp.line,
                            char_offset: comp.char_offset,
                        });
                    }
                }
                for (idx, ast_item) in ast_items.into_iter().enumerate() {
                    if !used_ast[idx] {
                        let insert_pos = merged
                            .iter()
                            .position(|m| {
                                m.file == ast_item.file && m.line_number > ast_item.line_number
                            })
                            .unwrap_or(merged.len());
                        merged.insert(insert_pos, ast_item);
                    }
                }
                ast_items = merged;
            }

            ast_items
        }
        FileKind::Bib => {
            let mut bib_items = Vec::new();
            let mut char_cursor = 0usize;
            for (idx, raw_line) in text.split('\n').enumerate() {
                let line_number = idx + 1;
                let line_char_len = raw_line.chars().count();
                let line = raw_line.trim_end_matches('\r');
                let trimmed = line.trim_start();
                let leading_ws = line.chars().count().saturating_sub(trimmed.chars().count());
                if trimmed.starts_with('@') {
                    if let Some(brace_idx) = trimmed.find('{') {
                        let entry_type = &trimmed[1..brace_idx];
                        let after_brace = &trimmed[brace_idx + 1..];
                        let key = after_brace.split(',').next().unwrap_or("").trim();
                        if !key.is_empty() {
                            bib_items.push(DocumentOutlineItem {
                                level: 1,
                                number_label: (bib_items.len() + 1).to_string(),
                                title: format!("@{} ({})", key, entry_type),
                                file: current_file.to_path_buf(),
                                line_number,
                                char_offset: char_cursor + leading_ws,
                            });
                        }
                    }
                }
                char_cursor += line_char_len + 1;
            }
            bib_items
        }
        _ => {
            let mut md_items = Vec::new();
            let mut char_cursor = 0usize;
            for (idx, raw_line) in text.split('\n').enumerate() {
                let line_number = idx + 1;
                let line_char_len = raw_line.chars().count();
                let line = raw_line.trim_end_matches('\r');
                let trimmed = line.trim_start();
                let leading_ws = line.chars().count().saturating_sub(trimmed.chars().count());
                let hash_count = trimmed.chars().take_while(|&c| c == '#').count();
                if hash_count >= 1 {
                    let after_hash = &trimmed[hash_count..];
                    if after_hash.starts_with(' ') {
                        let title = after_hash.trim().to_string();
                        if !title.is_empty() {
                            md_items.push(DocumentOutlineItem {
                                level: hash_count,
                                number_label: String::new(),
                                title,
                                file: current_file.to_path_buf(),
                                line_number,
                                char_offset: char_cursor + leading_ws,
                            });
                        }
                    }
                }
                char_cursor += line_char_len + 1;
            }
            md_items
        }
    };

    assign_hierarchical_numbers(&mut items);
    items
}
