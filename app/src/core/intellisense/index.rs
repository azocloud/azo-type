//! Pure in-memory project indexer for Typst `<label>` definitions, `#let` bindings,
//! doc comments, and BibTeX citation entries.

use std::collections::HashSet;
use std::path::Path;

use typst::syntax::ast::{self, AstNode};
use typst::syntax::{LinkedNode, SyntaxKind};

use crate::core::typst_prose::byte_span_to_char_range;

use super::{line_number_at_byte, LabelDefinition, UserSymbolDefinition};

/// Indexes `<label>` definitions, `#let` bindings, and `.bib` entries across `active_text`
/// and `project_sources` without touching the filesystem.
pub(super) fn collect_project_index(
    active_text: &str,
    project_sources: &[(&Path, &str)],
) -> (Vec<LabelDefinition>, Vec<UserSymbolDefinition>) {
    let mut labels = Vec::new();
    let mut symbols = Vec::new();
    let mut seen_labels = HashSet::new();
    let mut seen_symbols = HashSet::new();

    let active_in_sources = project_sources.iter().any(|(_, src)| *src == active_text);
    if !active_in_sources {
        index_typ_source(
            Path::new("current.typ"),
            active_text,
            &mut labels,
            &mut symbols,
            &mut seen_labels,
            &mut seen_symbols,
        );
    }

    for (path, text) in project_sources {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if ext == "bib" {
            index_bib_source(path, text, &mut labels, &mut seen_labels);
        } else if ext == "typ" || ext.is_empty() {
            index_typ_source(
                path,
                text,
                &mut labels,
                &mut symbols,
                &mut seen_labels,
                &mut seen_symbols,
            );
        }
    }

    (labels, symbols)
}

fn index_typ_source(
    path: &Path,
    text: &str,
    labels: &mut Vec<LabelDefinition>,
    symbols: &mut Vec<UserSymbolDefinition>,
    seen_labels: &mut HashSet<String>,
    seen_symbols: &mut HashSet<String>,
) {
    let root = typst::syntax::parse(text);
    let linked = LinkedNode::new(&root);
    let lines: Vec<&str> = text.lines().collect();

    fn walk(
        node: &LinkedNode<'_>,
        path: &Path,
        text: &str,
        lines: &[&str],
        labels: &mut Vec<LabelDefinition>,
        symbols: &mut Vec<UserSymbolDefinition>,
        seen_labels: &mut HashSet<String>,
        seen_symbols: &mut HashSet<String>,
    ) {
        match node.kind() {
            SyntaxKind::Label => {
                let name = node
                    .cast::<ast::Label>()
                    .map(|l| l.get().to_string())
                    .unwrap_or_default();
                if !name.is_empty() && seen_labels.insert(name.clone()) {
                    let line = line_number_at_byte(text, node.offset());
                    let (start_char, end_char) =
                        byte_span_to_char_range(text, node.offset(), node.offset() + node.len());
                    labels.push(LabelDefinition {
                        label: name,
                        file: path.to_path_buf(),
                        line,
                        start_char,
                        end_char,
                        kind_label: infer_label_kind(node),
                        context_snippet: extract_context_snippet(lines, line),
                    });
                }
            }
            SyntaxKind::LetBinding => {
                if let Some(binding) = node.cast::<ast::LetBinding>() {
                    let line = line_number_at_byte(text, node.offset());
                    let docs = extract_leading_doc_comment(lines, line);
                    let closure_params = match binding.init() {
                        Some(ast::Expr::Closure(c)) => {
                            Some(c.params().to_untyped().full_text().to_string())
                        }
                        _ => None,
                    };
                    let is_fn = matches!(binding.kind(), ast::LetBindingKind::Closure(_))
                        || closure_params.is_some();
                    for ident in binding.kind().bindings() {
                        let name = ident.as_str().to_string();
                        if !name.is_empty() && name != "_" && seen_symbols.insert(name.clone()) {
                            symbols.push(UserSymbolDefinition {
                                name,
                                params: closure_params.clone(),
                                file: path.to_path_buf(),
                                line,
                                documentation: docs.clone(),
                                is_function: is_fn,
                            });
                        }
                    }
                }
            }
            _ => {}
        }
        for child in node.children() {
            walk(
                &child,
                path,
                text,
                lines,
                labels,
                symbols,
                seen_labels,
                seen_symbols,
            );
        }
    }

    walk(
        &linked,
        path,
        text,
        &lines,
        labels,
        symbols,
        seen_labels,
        seen_symbols,
    );
}

fn infer_label_kind(label_node: &LinkedNode<'_>) -> String {
    let target = label_node
        .prev_sibling()
        .or_else(|| label_node.parent().cloned());
    if let Some(prev) = target {
        match prev.kind() {
            SyntaxKind::Heading => return "Heading".to_string(),
            SyntaxKind::Equation => return "Equation".to_string(),
            SyntaxKind::FuncCall => {
                if let Some(call) = prev.cast::<ast::FuncCall>() {
                    let callee = call.callee().to_untyped().full_text();
                    let short = callee.rsplit('.').next().unwrap_or(callee.as_str());
                    return match short {
                        "figure" => "Figure",
                        "table" => "Table",
                        "heading" => "Heading",
                        "equation" => "Equation",
                        _ => "Element",
                    }
                    .to_string();
                }
            }
            _ => {}
        }
    }
    "Label".to_string()
}

fn extract_context_snippet(lines: &[&str], line_1based: usize) -> String {
    if lines.is_empty() {
        return String::new();
    }
    let idx = line_1based.saturating_sub(1).min(lines.len() - 1);
    let start = idx.saturating_sub(1);
    let end = (idx + 1).min(lines.len() - 1);
    lines[start..=end]
        .iter()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn extract_leading_doc_comment(lines: &[&str], line_1based: usize) -> String {
    let mut idx = line_1based.saturating_sub(1);
    let mut collected = Vec::new();
    while idx > 0 {
        idx -= 1;
        let trimmed = lines.get(idx).map(|s| s.trim()).unwrap_or("");
        if let Some(rest) = trimmed.strip_prefix("///") {
            collected.push(rest.strip_prefix(' ').unwrap_or(rest));
        } else if let Some(rest) = trimmed.strip_prefix("//") {
            collected.push(rest.strip_prefix(' ').unwrap_or(rest));
        } else {
            break;
        }
    }
    collected.reverse();
    collected.join("\n").trim().to_string()
}

fn index_bib_source(
    path: &Path,
    text: &str,
    labels: &mut Vec<LabelDefinition>,
    seen_labels: &mut HashSet<String>,
) {
    let bytes = text.as_bytes();
    let mut pos = 0usize;
    while let Some(rel_at) = text[pos..].find('@') {
        let at_byte = pos + rel_at;
        let line_start = at_byte == 0 || bytes[at_byte - 1] == b'\n';
        let after_at = &text[at_byte + 1..];
        let Some(open_rel) = after_at.find('{') else {
            break;
        };
        let entry_type = after_at[..open_rel].trim();
        if !line_start && !text[pos..at_byte].trim().is_empty() {
            pos = at_byte + 1;
            continue;
        }
        if entry_type.is_empty()
            || !entry_type.chars().all(|c| c.is_ascii_alphanumeric())
            || matches!(
                entry_type.to_ascii_lowercase().as_str(),
                "comment" | "preamble" | "string"
            )
        {
            pos = at_byte + 1;
            continue;
        }
        let body_start = at_byte + 1 + open_rel + 1;
        let body_end = find_matching_brace(text, body_start);
        let body = &text[body_start..body_end];
        if let Some((key_raw, rest)) = body.split_once(',') {
            let key = key_raw.trim();
            if !key.is_empty() && seen_labels.insert(key.to_string()) {
                let line = line_number_at_byte(text, at_byte);
                let key_byte = body_start + body.find(key).unwrap_or(0);
                let (start_char, end_char) =
                    byte_span_to_char_range(text, key_byte, key_byte + key.len());
                labels.push(LabelDefinition {
                    label: key.to_string(),
                    file: path.to_path_buf(),
                    line,
                    start_char,
                    end_char,
                    kind_label: capitalize_first(&entry_type.to_ascii_lowercase()),
                    context_snippet: format_bib_snippet(entry_type, rest),
                });
            }
        }
        pos = body_end.max(at_byte + 1);
    }
}

fn find_matching_brace(text: &str, start_after_open: usize) -> usize {
    let mut depth = 1usize;
    let mut in_quotes = false;
    let mut escaped = false;
    for (rel, ch) in text[start_after_open..].char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '"' if depth == 1 => in_quotes = !in_quotes,
            '{' if !in_quotes => depth += 1,
            '}' if !in_quotes => {
                depth -= 1;
                if depth == 0 {
                    return start_after_open + rel;
                }
            }
            _ => {}
        }
    }
    text.len()
}

fn format_bib_snippet(entry_type: &str, body: &str) -> String {
    let title = extract_bib_field(body, "title");
    let author = extract_bib_field(body, "author");
    let year = extract_bib_field(body, "year");
    let mut parts = vec![format!("@{}", entry_type.to_ascii_lowercase())];
    if let Some(t) = title {
        parts.push(t);
    }
    if let Some(a) = author {
        parts.push(a);
    }
    if let Some(y) = year {
        parts.push(format!("({y})"));
    }
    parts.join(" — ")
}

fn extract_bib_field(body: &str, field_name: &str) -> Option<String> {
    for raw_line in body.lines() {
        let trimmed = raw_line.trim();
        let Some((lhs, rhs)) = trimmed.split_once('=') else {
            continue;
        };
        if lhs.trim().eq_ignore_ascii_case(field_name) {
            let cleaned = rhs
                .trim()
                .trim_end_matches(',')
                .trim()
                .trim_matches(&['{', '}', '"'][..])
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            if !cleaned.is_empty() {
                return Some(cleaned);
            }
        }
    }
    None
}

fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
        None => String::new(),
    }
}
