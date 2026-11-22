//! Context-aware completion candidate generation for `@` references, `#`/math symbols,
//! function parameters, keywords, and project file paths.

use std::path::{Path, PathBuf};

use typst::syntax::ast::{self, AstNode};
use typst::syntax::{LinkedNode, Side, SyntaxKind, SyntaxMode};

use super::index::collect_project_index;
use super::stdlib::{builtin_catalog, TYPST_KEYWORDS};
use super::{
    char_to_byte_offset, find_ancestor_kind, is_ident_char, is_ref_char, matches_prefix,
    normalize_display_path, CompletionItem, CompletionKind, CompletionRequestContext,
    LabelDefinition, UserSymbolDefinition,
};

pub(super) fn resolve_completions(
    active_text: &str,
    cursor_char: usize,
    project_sources: &[(&Path, &str)],
    project_files: &[PathBuf],
) -> Option<CompletionRequestContext> {
    let cursor_byte = char_to_byte_offset(active_text, cursor_char);
    let clamped_cursor_char = active_text[..cursor_byte].chars().count();
    let before = &active_text[..cursor_byte];

    if let Some((start_byte, typed_path, func_hint)) = detect_file_path_context(before) {
        let replace_start_char = active_text[..start_byte].chars().count();
        let items = build_file_path_items(typed_path, func_hint, project_files);
        return Some(CompletionRequestContext {
            replace_start_char,
            replace_end_char: clamped_cursor_char,
            prefix: typed_path.to_string(),
            items,
        });
    }

    let root = typst::syntax::parse(active_text);
    let linked = LinkedNode::new(&root);
    let cursor_leaf = cursor_byte
        .checked_sub(1)
        .and_then(|b| linked.leaf_at(b, Side::After));

    if is_non_trigger_leaf(cursor_leaf.as_ref()) {
        return None;
    }

    if let Some((start_byte, prefix)) = detect_reference_prefix(before) {
        let replace_start_char = active_text[..start_byte].chars().count();
        let (labels, _) = collect_project_index(active_text, project_sources);
        let items = filter_and_sort_references(&labels, prefix);
        return Some(CompletionRequestContext {
            replace_start_char,
            replace_end_char: clamped_cursor_char,
            prefix: prefix.to_string(),
            items,
        });
    }

    detect_code_or_math_completions(
        active_text,
        before,
        cursor_byte,
        clamped_cursor_char,
        cursor_leaf.as_ref(),
        project_sources,
    )
}

fn detect_code_or_math_completions(
    active_text: &str,
    before: &str,
    cursor_byte: usize,
    clamped_cursor_char: usize,
    cursor_leaf: Option<&LinkedNode<'_>>,
    project_sources: &[(&Path, &str)],
) -> Option<CompletionRequestContext> {
    let ident_len = before
        .char_indices()
        .rev()
        .take_while(|&(_, c)| is_ident_char(c))
        .map(|(idx, _)| cursor_byte - idx)
        .last()
        .unwrap_or(0);
    let prefix_start = cursor_byte - ident_len;
    let prefix = &before[prefix_start..];
    let after_hash = prefix_start > 0
        && before[..prefix_start].ends_with('#')
        && !before[..prefix_start].ends_with("\\#");

    let mode = cursor_leaf.and_then(|l| l.mode_after());
    let in_code = mode == Some(SyntaxMode::Code);
    let in_math = mode == Some(SyntaxMode::Math);

    if (in_code || in_math) && before[..prefix_start].ends_with('.') {
        let dot_before = &before[..prefix_start - 1];
        let path_len = dot_before
            .char_indices()
            .rev()
            .take_while(|&(_, c)| is_ident_char(c) || c == '.')
            .map(|(idx, _)| dot_before.len() - idx)
            .last()
            .unwrap_or(0);
        if path_len > 0 {
            let container = &dot_before[dot_before.len() - path_len..];
            let mut items = builtin_catalog().complete_dotted(container, prefix, in_math);
            sort_completion_items(&mut items, prefix);
            if !items.is_empty() {
                return Some(CompletionRequestContext {
                    replace_start_char: active_text[..prefix_start].chars().count(),
                    replace_end_char: clamped_cursor_char,
                    prefix: prefix.to_string(),
                    items,
                });
            }
        }
    }

    if !after_hash && !((in_code || in_math) && !prefix.is_empty()) {
        if in_code {
            if let Some(callee) = enclosing_call_name(cursor_leaf) {
                let (_, user_syms) = collect_project_index(active_text, project_sources);
                let items = collect_param_items(&callee, "", &user_syms);
                if !items.is_empty() {
                    return Some(CompletionRequestContext {
                        replace_start_char: clamped_cursor_char,
                        replace_end_char: clamped_cursor_char,
                        prefix: String::new(),
                        items,
                    });
                }
            }
        }
        return None;
    }

    let replace_start_char = active_text[..prefix_start].chars().count();
    let (_, user_syms) = collect_project_index(active_text, project_sources);
    let mut items = Vec::new();

    if in_code {
        if let Some(callee) = enclosing_call_name(cursor_leaf) {
            items.extend(collect_param_items(&callee, prefix, &user_syms));
        }
    }

    let prefix_lower = prefix.to_ascii_lowercase();
    for sym in &user_syms {
        if matches_prefix(&sym.name, &prefix_lower) {
            items.push(CompletionItem {
                label: sym.name.clone(),
                insert_text: sym.name.clone(),
                detail: sym.params.clone().unwrap_or_else(|| "variable".to_string()),
                documentation: sym.documentation.clone(),
                kind: if sym.is_function {
                    CompletionKind::Macro
                } else {
                    CompletionKind::Variable
                },
                source_hint: Some(format!(
                    "{}:{}",
                    normalize_display_path(&sym.file),
                    sym.line
                )),
            });
        }
    }

    for entry in &builtin_catalog().entries {
        let in_scope = if in_math && !after_hash {
            entry.in_math
        } else {
            entry.in_global
        };
        if in_scope && matches_prefix(&entry.name, &prefix_lower) {
            items.push(CompletionItem {
                label: entry.name.clone(),
                insert_text: entry.name.clone(),
                detail: entry.detail.clone(),
                documentation: entry.documentation.clone(),
                kind: entry.kind,
                source_hint: Some("typst".to_string()),
            });
        }
    }

    if after_hash || in_code {
        for (kw, doc) in TYPST_KEYWORDS {
            if matches_prefix(kw, &prefix_lower) {
                items.push(CompletionItem {
                    label: (*kw).to_string(),
                    insert_text: (*kw).to_string(),
                    detail: "keyword".to_string(),
                    documentation: (*doc).to_string(),
                    kind: CompletionKind::Keyword,
                    source_hint: None,
                });
            }
        }
    }

    sort_completion_items(&mut items, prefix);
    Some(CompletionRequestContext {
        replace_start_char,
        replace_end_char: clamped_cursor_char,
        prefix: prefix.to_string(),
        items,
    })
}

fn collect_param_items(
    callee: &str,
    prefix: &str,
    user_syms: &[UserSymbolDefinition],
) -> Vec<CompletionItem> {
    let prefix_lower = prefix.to_ascii_lowercase();
    let mut out = Vec::new();
    let catalog = builtin_catalog();
    if let Some(&idx) = catalog.by_name.get(callee) {
        for param in &catalog.entries[idx].params {
            if matches_prefix(param, &prefix_lower) {
                out.push(CompletionItem {
                    label: param.clone(),
                    insert_text: format!("{param}: "),
                    detail: format!("parameter of `{callee}`"),
                    documentation: String::new(),
                    kind: CompletionKind::Parameter,
                    source_hint: Some("typst".to_string()),
                });
            }
        }
    } else if let Some(sym) = user_syms.iter().find(|s| s.name == callee && s.is_function) {
        if let Some(params_raw) = &sym.params {
            for raw in params_raw.trim_matches(&['(', ')'][..]).split(',') {
                let p = raw
                    .split(':')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .trim_start_matches("..");
                if !p.is_empty() && matches_prefix(p, &prefix_lower) {
                    out.push(CompletionItem {
                        label: p.to_string(),
                        insert_text: format!("{p}: "),
                        detail: format!("parameter of `{callee}`"),
                        documentation: String::new(),
                        kind: CompletionKind::Parameter,
                        source_hint: Some(format!(
                            "{}:{}",
                            normalize_display_path(&sym.file),
                            sym.line
                        )),
                    });
                }
            }
        }
    }
    out
}

fn enclosing_call_name(leaf: Option<&LinkedNode<'_>>) -> Option<String> {
    let mut cur = leaf.cloned();
    while let Some(node) = cur {
        if node.kind() == SyntaxKind::FuncCall {
            if let Some(call) = node.cast::<ast::FuncCall>() {
                return Some(call.callee().to_untyped().full_text().to_string());
            }
        } else if node.kind() == SyntaxKind::SetRule {
            if let Some(rule) = node.cast::<ast::SetRule>() {
                return Some(rule.target().to_untyped().full_text().to_string());
            }
        }
        cur = node.parent().cloned();
    }
    None
}

fn detect_file_path_context(before: &str) -> Option<(usize, &str, &'static str)> {
    let line_start = before.rfind('\n').map(|i| i + 1).unwrap_or(0);
    let line = &before[line_start..];
    let mut quote_positions = Vec::new();
    let mut escaped = false;
    for (idx, ch) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
        } else if ch == '"' {
            quote_positions.push(idx);
        }
    }
    if quote_positions.len() % 2 == 0 {
        return None;
    }
    let rel_quote = *quote_positions.last()?;
    let abs_quote = line_start + rel_quote;
    let typed = &before[abs_quote + 1..];
    let prefix = line[..rel_quote].trim_end();

    let hint = if ends_with_keyword(prefix, "import") {
        "import"
    } else if ends_with_keyword(prefix, "include") {
        "include"
    } else {
        let inner = prefix.strip_suffix('(').map(str::trim_end)?;
        if ends_with_keyword(inner, "image") {
            "image"
        } else if ends_with_keyword(inner, "bibliography") {
            "bibliography"
        } else {
            return None;
        }
    };
    Some((abs_quote + 1, typed, hint))
}

fn ends_with_keyword(text: &str, kw: &str) -> bool {
    let Some(rest) = text.strip_suffix(kw) else {
        return false;
    };
    rest.chars().next_back().is_none_or(|c| !is_ident_char(c))
}

fn build_file_path_items(
    typed: &str,
    func_hint: &str,
    project_files: &[PathBuf],
) -> Vec<CompletionItem> {
    let typed_lower = typed.to_ascii_lowercase();
    let mut scored = Vec::new();
    for path in project_files {
        let norm = normalize_display_path(path);
        let norm_lower = norm.to_ascii_lowercase();
        let file_name_lower = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let match_tier = if typed_lower.is_empty() || norm_lower.starts_with(&typed_lower) {
            0u8
        } else if file_name_lower.starts_with(&typed_lower) {
            1u8
        } else if norm_lower.contains(&typed_lower) {
            2u8
        } else {
            continue;
        };
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let ext_priority = match func_hint {
            "import" | "include" => u8::from(ext != "typ"),
            "bibliography" => u8::from(!matches!(ext.as_str(), "bib" | "yml" | "yaml")),
            "image" => u8::from(!matches!(
                ext.as_str(),
                "png" | "jpg" | "jpeg" | "svg" | "webp" | "pdf" | "gif"
            )),
            _ => 0,
        };
        scored.push((
            (ext_priority, match_tier, norm.clone()),
            CompletionItem {
                label: norm.clone(),
                insert_text: norm,
                detail: if ext.is_empty() {
                    "file".into()
                } else {
                    format!(".{ext} file")
                },
                documentation: String::new(),
                kind: CompletionKind::FilePath,
                source_hint: None,
            },
        ));
    }
    scored.sort_by(|a, b| a.0.cmp(&b.0));
    scored.into_iter().map(|(_, item)| item).collect()
}

fn detect_reference_prefix(before: &str) -> Option<(usize, &str)> {
    for (idx, ch) in before.char_indices().rev() {
        if is_ref_char(ch) {
            continue;
        }
        if ch == '@' {
            let prev = before[..idx].chars().next_back();
            if prev.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_') {
                return None;
            }
            return Some((idx + 1, &before[idx + 1..]));
        }
        return None;
    }
    None
}

fn filter_and_sort_references(labels: &[LabelDefinition], prefix: &str) -> Vec<CompletionItem> {
    let prefix_lower = prefix.to_ascii_lowercase();
    let mut items: Vec<CompletionItem> = labels
        .iter()
        .filter(|def| matches_prefix(&def.label, &prefix_lower))
        .map(|def| CompletionItem {
            label: def.label.clone(),
            insert_text: def.label.clone(),
            detail: def.kind_label.clone(),
            documentation: def.context_snippet.clone(),
            kind: CompletionKind::Reference,
            source_hint: Some(format!(
                "{}:{}",
                normalize_display_path(&def.file),
                def.line
            )),
        })
        .collect();
    sort_completion_items(&mut items, prefix);
    items
}

fn sort_completion_items(items: &mut [CompletionItem], prefix: &str) {
    items.sort_by(|a, b| {
        let a_exact = !prefix.is_empty() && a.label.starts_with(prefix);
        let b_exact = !prefix.is_empty() && b.label.starts_with(prefix);
        let rank = |k: CompletionKind| match k {
            CompletionKind::Parameter => 0u8,
            CompletionKind::Macro | CompletionKind::Reference => 1,
            CompletionKind::Function => 2,
            CompletionKind::Variable => 3,
            CompletionKind::Keyword => 4,
            CompletionKind::FilePath => 5,
        };
        (!a_exact, rank(a.kind), a.label.len(), &a.label).cmp(&(
            !b_exact,
            rank(b.kind),
            b.label.len(),
            &b.label,
        ))
    });
}

fn is_non_trigger_leaf(leaf: Option<&LinkedNode<'_>>) -> bool {
    let Some(node) = leaf else { return false };
    matches!(
        node.kind(),
        SyntaxKind::LineComment | SyntaxKind::BlockComment | SyntaxKind::Str
    ) || find_ancestor_kind(node, SyntaxKind::Raw).is_some()
}
