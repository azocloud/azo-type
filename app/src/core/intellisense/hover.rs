//! Hover tooltip and diagnostic hint resolution for Typst references, symbols, and diagnostics.

use std::collections::HashSet;
use std::path::Path;

use typst::syntax::ast;
use typst::syntax::{LinkedNode, Side, SyntaxKind};

use crate::core::typst_prose::byte_span_to_char_range;

use super::index::collect_project_index;
use super::stdlib::{builtin_catalog, TYPST_KEYWORDS};
use super::{
    char_to_byte_offset, find_ancestor_kind, normalize_display_path, HoverInfo, TypstDiagnosticHint,
};

pub(super) fn resolve_hover_info(
    active_text: &str,
    hover_char: usize,
    project_sources: &[(&Path, &str)],
    compiler_diagnostics: &[(usize, usize, String, bool)],
) -> Option<HoverInfo> {
    if let Some(diag) = find_diagnostic_at(compiler_diagnostics, hover_char) {
        return Some(diag);
    }

    let hover_byte = char_to_byte_offset(active_text, hover_char);
    if hover_byte >= active_text.len() {
        return None;
    }

    let root = typst::syntax::parse(active_text);
    let linked = LinkedNode::new(&root);
    let leaf = linked.leaf_at(hover_byte, Side::After)?;

    if let Some(ref_node) = find_ancestor_kind(&leaf, SyntaxKind::Ref) {
        let target = ref_node
            .cast::<ast::Ref>()
            .map(|r| r.target())
            .unwrap_or("");
        if !target.is_empty() {
            let span_node = ref_node
                .children()
                .find(|c| c.kind() == SyntaxKind::RefMarker)
                .unwrap_or(ref_node.clone());
            let (start_char, end_char) = byte_span_to_char_range(
                active_text,
                span_node.offset(),
                span_node.offset() + span_node.len(),
            );
            let (labels, _) = collect_project_index(active_text, project_sources);
            if let Some(def) = labels.into_iter().find(|l| l.label == target) {
                return Some(HoverInfo {
                    start_char,
                    end_char,
                    title: format!("@{}", def.label),
                    detail: format!(
                        "{} ({}:{})",
                        def.kind_label,
                        normalize_display_path(&def.file),
                        def.line
                    ),
                    documentation: def.context_snippet,
                    is_diagnostic: false,
                });
            }
            return Some(HoverInfo {
                start_char,
                end_char,
                title: format!("Unresolved reference: @{target}"),
                detail: "Diagnostic".to_string(),
                documentation: format!("No `<{target}>` label or BibTeX key found in project."),
                is_diagnostic: true,
            });
        }
    }

    if leaf.kind() == SyntaxKind::Label {
        let name = leaf
            .cast::<ast::Label>()
            .map(|l| l.get())
            .unwrap_or_else(|| leaf.get().leaf_text().trim_matches(&['<', '>'][..]));
        let (labels, _) = collect_project_index(active_text, project_sources);
        if let Some(def) = labels.into_iter().find(|l| l.label == name) {
            let (start_char, end_char) =
                byte_span_to_char_range(active_text, leaf.offset(), leaf.offset() + leaf.len());
            return Some(HoverInfo {
                start_char,
                end_char,
                title: format!("<{}>", def.label),
                detail: format!(
                    "{} ({}:{})",
                    def.kind_label,
                    normalize_display_path(&def.file),
                    def.line
                ),
                documentation: def.context_snippet,
                is_diagnostic: false,
            });
        }
    }

    hover_symbol_or_keyword(active_text, &leaf, project_sources)
}

pub(super) fn resolve_typst_hints(
    active_text: &str,
    project_sources: &[(&Path, &str)],
    compiler_diagnostics: &[(usize, usize, String, bool)],
) -> Vec<TypstDiagnosticHint> {
    let (labels, _) = collect_project_index(active_text, project_sources);
    let known: HashSet<&str> = labels.iter().map(|l| l.label.as_str()).collect();
    let mut hints = Vec::new();

    let root = typst::syntax::parse(active_text);
    let linked = LinkedNode::new(&root);
    collect_unresolved_refs(&linked, active_text, &known, &mut hints);

    let total_chars = active_text.chars().count();
    for (start_char, end_char, message, is_error) in compiler_diagnostics {
        let start = (*start_char).min(total_chars);
        let end = (*end_char).max(start).min(total_chars);
        hints.push(TypstDiagnosticHint {
            start_char: start,
            end_char: end,
            message: message.clone(),
            is_error: *is_error,
        });
    }

    hints.sort_by_key(|h| (h.start_char, h.end_char));
    hints
}

fn collect_unresolved_refs(
    node: &LinkedNode<'_>,
    active_text: &str,
    known: &HashSet<&str>,
    out: &mut Vec<TypstDiagnosticHint>,
) {
    if node.kind() == SyntaxKind::Ref {
        if let Some(r) = node.cast::<ast::Ref>() {
            let target = r.target();
            if !target.is_empty() && !known.contains(target) {
                let marker = node
                    .children()
                    .find(|c| c.kind() == SyntaxKind::RefMarker)
                    .unwrap_or_else(|| node.clone());
                let (start_char, end_char) = byte_span_to_char_range(
                    active_text,
                    marker.offset(),
                    marker.offset() + marker.len(),
                );
                out.push(TypstDiagnosticHint {
                    start_char,
                    end_char,
                    message: format!("Unresolved reference: @{target}"),
                    is_error: true,
                });
            }
        }
        return;
    }
    for child in node.children() {
        collect_unresolved_refs(&child, active_text, known, out);
    }
}

fn find_diagnostic_at(
    compiler_diagnostics: &[(usize, usize, String, bool)],
    hover_char: usize,
) -> Option<HoverInfo> {
    compiler_diagnostics
        .iter()
        .filter(|(start, end, _, _)| {
            hover_char >= *start && (hover_char < *end || (*start == *end && hover_char == *start))
        })
        .min_by_key(|(start, end, _, is_err)| (!*is_err, end.saturating_sub(*start)))
        .map(|(start, end, msg, is_err)| HoverInfo {
            start_char: *start,
            end_char: *end,
            title: if *is_err { "Error" } else { "Warning" }.to_string(),
            detail: String::new(),
            documentation: msg.clone(),
            is_diagnostic: true,
        })
}

fn hover_symbol_or_keyword(
    active_text: &str,
    leaf: &LinkedNode<'_>,
    project_sources: &[(&Path, &str)],
) -> Option<HoverInfo> {
    let target_leaf = if leaf.kind() == SyntaxKind::Hash {
        leaf.next_sibling_with_trivia()
            .and_then(|n| n.leftmost_leaf())
            .unwrap_or_else(|| leaf.clone())
    } else {
        leaf.clone()
    };

    let (start_char, end_char) = byte_span_to_char_range(
        active_text,
        target_leaf.offset(),
        target_leaf.offset() + target_leaf.len(),
    );
    let text_tok = target_leaf.get().leaf_text().as_str();

    if matches!(
        target_leaf.kind(),
        SyntaxKind::Ident | SyntaxKind::MathIdent
    ) {
        let catalog = builtin_catalog();
        if let Some(access_node) = outermost_field_access(&target_leaf) {
            let full = access_node.get().full_text();
            let prefer_math = target_leaf.kind() == SyntaxKind::MathIdent;
            if let Some(entry) = catalog.lookup_dotted(full.as_str(), prefer_math) {
                return Some(HoverInfo {
                    start_char,
                    end_char,
                    title: entry.name,
                    detail: entry.detail,
                    documentation: entry.documentation,
                    is_diagnostic: false,
                });
            }
        }

        let (_, user_syms) = collect_project_index(active_text, project_sources);
        if let Some(sym) = user_syms.into_iter().find(|s| s.name == text_tok) {
            let sig = sym.params.clone().unwrap_or_else(|| {
                if sym.is_function {
                    "()".into()
                } else {
                    "variable".into()
                }
            });
            return Some(HoverInfo {
                start_char,
                end_char,
                title: sym.name,
                detail: format!("{sig} ({}:{})", normalize_display_path(&sym.file), sym.line),
                documentation: sym.documentation,
                is_diagnostic: false,
            });
        }

        if let Some(&idx) = catalog.by_name.get(text_tok) {
            let entry = &catalog.entries[idx];
            return Some(HoverInfo {
                start_char,
                end_char,
                title: entry.name.clone(),
                detail: entry.detail.clone(),
                documentation: entry.documentation.clone(),
                is_diagnostic: false,
            });
        }
    }

    if let Some((_, kw_doc)) = TYPST_KEYWORDS.iter().find(|(kw, _)| *kw == text_tok) {
        return Some(HoverInfo {
            start_char,
            end_char,
            title: text_tok.to_string(),
            detail: "keyword".to_string(),
            documentation: (*kw_doc).to_string(),
            is_diagnostic: false,
        });
    }

    None
}

fn outermost_field_access<'a>(leaf: &LinkedNode<'a>) -> Option<LinkedNode<'a>> {
    let mut best = None;
    let mut cur = leaf.parent().cloned();
    while let Some(node) = cur {
        if matches!(
            node.kind(),
            SyntaxKind::FieldAccess | SyntaxKind::MathFieldAccess
        ) {
            best = Some(node.clone());
            cur = node.parent().cloned();
        } else {
            break;
        }
    }
    best
}
