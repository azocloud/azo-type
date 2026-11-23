//! Built-in Typst standard library symbol catalog and keyword documentation.
//!
//! Lazily extracts global and math scope bindings, parameter signatures, and
//! embedded Typst documentation from `typst::Library::default()`.

use std::collections::HashMap;
use std::sync::OnceLock;

use typst::foundations::{Func, Scope, Type, Value};
use typst::{Library, LibraryExt};

use super::{matches_prefix, CompletionItem, CompletionKind};

/// Internal metadata for a built-in Typst standard library item.
#[derive(Debug, Clone)]
pub(super) struct BuiltinEntry {
    pub name: String,
    pub detail: String,
    pub documentation: String,
    pub kind: CompletionKind,
    pub params: Vec<String>,
    pub in_global: bool,
    pub in_math: bool,
}

/// Cached built-in Typst standard library catalog.
#[derive(Debug, Clone)]
pub(super) struct BuiltinCatalog {
    pub entries: Vec<BuiltinEntry>,
    pub by_name: HashMap<String, usize>,
    library: Library,
}

static BUILTIN_CATALOG: OnceLock<BuiltinCatalog> = OnceLock::new();

/// Core Typst language keywords with brief hover/completion descriptions.
pub(super) const TYPST_KEYWORDS: &[(&str, &str)] = &[
    ("let", "Binds a variable or function (`#let name = value`)."),
    (
        "set",
        "Sets default style properties on an element (`#set text(...)`).",
    ),
    (
        "show",
        "Applies a styling or transformation rule (`#show heading: ...`).",
    ),
    (
        "import",
        "Imports bindings from another file or package (`#import \"file.typ\": *`).",
    ),
    (
        "include",
        "Evaluates and inserts the content of another Typst file (`#include \"ch.typ\"`).",
    ),
    (
        "if",
        "Conditional branching expression (`#if cond { ... } else { ... }`).",
    ),
    (
        "else",
        "Fallback branch for a conditional (`#if ... { ... } else { ... }`).",
    ),
    (
        "for",
        "Iterates over an array, dictionary, or string (`#for item in items { ... }`).",
    ),
    (
        "while",
        "Repeats a block while a condition holds (`#while cond { ... }`).",
    ),
    (
        "return",
        "Returns a value early from a function (`#return value`).",
    ),
    (
        "context",
        "Accesses contextual layout or style state (`#context counter(...).get()`).",
    ),
];

pub(super) fn builtin_catalog() -> &'static BuiltinCatalog {
    BUILTIN_CATALOG.get_or_init(|| {
        let library = Library::default();
        let mut entries = Vec::new();
        let mut by_name = HashMap::new();

        collect_scope_entries(
            library.global.scope(),
            true,
            false,
            &mut entries,
            &mut by_name,
        );
        collect_scope_entries(
            library.math.scope(),
            false,
            true,
            &mut entries,
            &mut by_name,
        );

        BuiltinCatalog {
            entries,
            by_name,
            library,
        }
    })
}

impl BuiltinCatalog {
    /// Resolves a dotted member path like `calc.sin` or `sym.arrow` or `arrow.r`.
    pub(super) fn lookup_dotted(&self, dotted: &str, prefer_math: bool) -> Option<BuiltinEntry> {
        let (root_name, rest) = dotted.split_once('.')?;
        let mut val = self.resolve_root_value(root_name, prefer_math)?;
        let segments: Vec<&str> = rest.split('.').collect();
        for (idx, seg) in segments.iter().enumerate() {
            if seg.is_empty() {
                return None;
            }
            match val {
                Value::Module(m) => {
                    val = m.scope().get(seg)?.read().clone();
                }
                Value::Type(ty) => {
                    val = ty.scope().get(seg)?.read().clone();
                }
                Value::Func(f) => {
                    val = f
                        .scope()
                        .and_then(|s| s.get(seg))
                        .map(|b| b.read().clone())
                        .or_else(|| func_method_value(&f, seg))?;
                }
                Value::Symbol(sym) => {
                    let tail = &segments[idx..];
                    if tail.iter().all(|s| sym.modifiers().any(|m| m == *s)) {
                        return Some(BuiltinEntry {
                            name: dotted.to_string(),
                            detail: format!("symbol variant of `{root_name}`"),
                            documentation: format!(
                                "Modified symbol `{dotted}` (base `{}`)",
                                sym.get()
                            ),
                            kind: CompletionKind::Variable,
                            params: Vec::new(),
                            in_global: true,
                            in_math: true,
                        });
                    }
                    return None;
                }
                _ => return None,
            }
        }
        let (kind, detail, documentation, params) = describe_typst_value(&val);
        Some(BuiltinEntry {
            name: dotted.to_string(),
            detail,
            documentation,
            kind,
            params,
            in_global: true,
            in_math: prefer_math,
        })
    }

    /// Collects member completions for a dotted prefix such as `calc.` or `sym.arr`.
    pub(super) fn complete_dotted(
        &self,
        container_path: &str,
        member_prefix: &str,
        prefer_math: bool,
    ) -> Vec<CompletionItem> {
        let mut parts = container_path.split('.');
        let Some(root_name) = parts.next() else {
            return Vec::new();
        };
        let Some(mut val) = self.resolve_root_value(root_name, prefer_math) else {
            return Vec::new();
        };
        for seg in parts {
            val = match val {
                Value::Module(m) => match m.scope().get(seg) {
                    Some(b) => b.read().clone(),
                    None => return Vec::new(),
                },
                Value::Type(ty) => match ty.scope().get(seg) {
                    Some(b) => b.read().clone(),
                    None => return Vec::new(),
                },
                Value::Func(f) => {
                    if let Some(b) = f.scope().and_then(|s| s.get(seg)) {
                        b.read().clone()
                    } else if let Some(method_val) = func_method_value(&f, seg) {
                        method_val
                    } else {
                        return Vec::new();
                    }
                }
                _ => return Vec::new(),
            };
        }

        let prefix_lower = member_prefix.to_ascii_lowercase();
        let mut out = Vec::new();
        match val {
            Value::Module(m) => push_scope_items(m.scope(), &prefix_lower, &mut out),
            Value::Type(ty) => push_scope_items(ty.scope(), &prefix_lower, &mut out),
            Value::Func(f) => {
                if let Some(scope) = f.scope() {
                    push_scope_items(scope, &prefix_lower, &mut out);
                }
                push_func_method_items(&f, &prefix_lower, &mut out);
            }
            Value::Symbol(sym) => {
                for modifier in sym.modifiers() {
                    if matches_prefix(modifier, &prefix_lower) {
                        out.push(CompletionItem {
                            label: modifier.to_string(),
                            insert_text: modifier.to_string(),
                            detail: "symbol modifier".to_string(),
                            documentation: format!("Modifier `.{modifier}` on `{container_path}`"),
                            kind: CompletionKind::Variable,
                            source_hint: Some("typst".to_string()),
                        });
                    }
                }
            }
            _ => {}
        }
        out
    }

    fn resolve_root_value(&self, name: &str, prefer_math: bool) -> Option<Value> {
        if prefer_math {
            if let Some(b) = self.library.math.scope().get(name) {
                return Some(b.read().clone());
            }
        }
        self.library
            .global
            .scope()
            .get(name)
            .or_else(|| self.library.math.scope().get(name))
            .map(|b| b.read().clone())
    }
}

fn func_method_value(func: &Func, seg: &str) -> Option<Value> {
    if seg == "where" && func.to_element().is_none() {
        return None;
    }
    Type::of::<Func>()
        .scope()
        .get(seg)
        .map(|b| b.read().clone())
}

fn push_func_method_items(func: &Func, prefix_lower: &str, out: &mut Vec<CompletionItem>) {
    for (name, binding) in Type::of::<Func>().scope().iter() {
        let name_str = name.as_str();
        if name_str == "where" && func.to_element().is_none() {
            continue;
        }
        if matches_prefix(name_str, prefix_lower) && !out.iter().any(|item| item.label == name_str)
        {
            let (kind, detail, documentation, _) = describe_typst_value(binding.read());
            out.push(CompletionItem {
                label: name_str.to_string(),
                insert_text: name_str.to_string(),
                detail,
                documentation,
                kind,
                source_hint: Some("typst".to_string()),
            });
        }
    }
}

fn push_scope_items(scope: &Scope, prefix_lower: &str, out: &mut Vec<CompletionItem>) {
    for (name, binding) in scope.iter() {
        let name_str = name.as_str();
        if matches_prefix(name_str, prefix_lower) {
            let (kind, detail, documentation, _) = describe_typst_value(binding.read());
            out.push(CompletionItem {
                label: name_str.to_string(),
                insert_text: name_str.to_string(),
                detail,
                documentation,
                kind,
                source_hint: Some("typst".to_string()),
            });
        }
    }
}

fn collect_scope_entries(
    scope: &Scope,
    in_global: bool,
    in_math: bool,
    entries: &mut Vec<BuiltinEntry>,
    by_name: &mut HashMap<String, usize>,
) {
    for (name, binding) in scope.iter() {
        let key = name.to_string();
        if let Some(&idx) = by_name.get(&key) {
            entries[idx].in_global |= in_global;
            entries[idx].in_math |= in_math;
            continue;
        }
        let (kind, detail, documentation, params) = describe_typst_value(binding.read());
        let idx = entries.len();
        by_name.insert(key.clone(), idx);
        entries.push(BuiltinEntry {
            name: key,
            detail,
            documentation,
            kind,
            params,
            in_global,
            in_math,
        });
    }
}

fn describe_typst_value(val: &Value) -> (CompletionKind, String, String, Vec<String>) {
    match val {
        Value::Func(func) => {
            let detail = format_func_params(func);
            let docs = sanitize_typst_docs(func.docs().unwrap_or(""));
            let params = func
                .params()
                .filter_map(|p| p.name().map(str::to_string))
                .collect();
            (CompletionKind::Function, detail, docs, params)
        }
        Value::Type(ty) => {
            let (detail, params) = match ty.constructor() {
                Ok(ctor) => (
                    format_func_params(&ctor),
                    ctor.params()
                        .filter_map(|p| p.name().map(str::to_string))
                        .collect(),
                ),
                Err(_) => ("type".to_string(), Vec::new()),
            };
            (
                CompletionKind::Variable,
                detail,
                sanitize_typst_docs(ty.docs()),
                params,
            )
        }
        Value::Symbol(sym) => {
            let glyph = sym.get();
            let mods: Vec<&str> = sym.modifiers().take(6).collect();
            let doc = if mods.is_empty() {
                format!("Symbol `{glyph}`")
            } else {
                format!("Symbol `{glyph}` — variants: {}", mods.join(", "))
            };
            (
                CompletionKind::Variable,
                format!("symbol ({glyph})"),
                doc,
                Vec::new(),
            )
        }
        Value::Module(module) => {
            let count = module.scope().iter().count();
            (
                CompletionKind::Variable,
                "module".to_string(),
                format!("Built-in module with {count} exported definitions."),
                Vec::new(),
            )
        }
        other => (
            CompletionKind::Variable,
            other.ty().short_name().to_string(),
            format!("Built-in `{}` value.", other.ty().long_name()),
            Vec::new(),
        ),
    }
}

fn format_func_params(func: &Func) -> String {
    let mut parts = Vec::new();
    let mut named_count = 0usize;
    let mut truncated = false;

    for param in func.params() {
        if param.variadic() {
            parts.push(format!("..{}", param.name().unwrap_or("args")));
        } else if param.positional() {
            let name = param.name().unwrap_or("_");
            if param.required() {
                parts.push(name.to_string());
            } else {
                parts.push(format!("[{name}]"));
            }
        } else if param.named() {
            if named_count < 2 && parts.len() < 4 {
                parts.push(format!("{}: ..", param.name().unwrap_or("_")));
                named_count += 1;
            } else {
                truncated = true;
            }
        }
    }

    if truncated {
        parts.push("..".to_string());
    }
    format!("({})", parts.join(", "))
}

/// Converts raw Typst stdlib documentation markup into clean, readable plain text.
pub(super) fn sanitize_typst_docs(raw: &str) -> String {
    let mut lines_out: Vec<String> = Vec::new();
    let mut in_code_block = false;
    let mut prev_blank = true;

    for raw_line in raw.lines() {
        let trimmed = raw_line.trim();
        if let Some(after_ticks) = trimmed.strip_prefix("```") {
            in_code_block = !in_code_block;
            if in_code_block && after_ticks.trim().eq_ignore_ascii_case("example") {
                let has_hdr = lines_out
                    .iter()
                    .rev()
                    .find(|l| !l.is_empty())
                    .is_some_and(|l| l.eq_ignore_ascii_case("example:"));
                if !has_hdr {
                    if !prev_blank && !lines_out.is_empty() {
                        lines_out.push(String::new());
                    }
                    lines_out.push("Example:".to_string());
                    prev_blank = false;
                }
            } else if !in_code_block && !prev_blank && !lines_out.is_empty() {
                lines_out.push(String::new());
                prev_blank = true;
            }
            continue;
        }

        if in_code_block {
            let code_line = raw_line.trim_end();
            if code_line.is_empty() && !prev_blank {
                lines_out.push(String::new());
                prev_blank = true;
            } else if !code_line.is_empty() {
                lines_out.push(format!("  {code_line}"));
                prev_blank = false;
            }
            continue;
        }

        if trimmed.is_empty() {
            if !prev_blank {
                lines_out.push(String::new());
                prev_blank = true;
            }
            continue;
        }

        let cleaned = clean_typst_doc_heading(trimmed)
            .unwrap_or_else(|| clean_typst_doc_inline(raw_line.trim_end()));
        if !cleaned.trim().is_empty() {
            lines_out.push(cleaned);
            prev_blank = false;
        }
    }

    while lines_out.last().is_some_and(String::is_empty) {
        lines_out.pop();
    }
    lines_out.join("\n")
}

fn clean_typst_doc_heading(trimmed: &str) -> Option<String> {
    let eq_count = trimmed.bytes().take_while(|&b| b == b'=').count();
    let rest = (eq_count > 0)
        .then(|| trimmed[eq_count..].strip_prefix(' '))
        .flatten()?
        .trim();
    let without_label = rest
        .rfind(" <")
        .filter(|_| rest.ends_with('>'))
        .map_or(rest, |idx| rest[..idx].trim_end());
    let title = clean_typst_doc_inline(without_label);
    (!title.is_empty()).then(|| {
        if title.ends_with([':', '.', '?', '!']) {
            title
        } else {
            format!("{title}:")
        }
    })
}

fn clean_typst_doc_inline(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut i = 0usize;
    let bytes = line.as_bytes();

    while i < bytes.len() {
        if bytes[i] == b'`' {
            if let Some(rel_end) = line[i + 1..].find('`') {
                let inner = &line[i + 1..i + 1 + rel_end];
                out.push('`');
                out.push_str(unwrap_typst_code_braces(inner).unwrap_or(inner));
                out.push('`');
                i += rel_end + 2;
                continue;
            }
        }
        if bytes[i] == b'[' {
            if let Some((label, next_i)) = parse_doc_dollar_link(line, i) {
                out.push_str(&clean_typst_doc_inline(label));
                i = next_i;
                continue;
            }
        }
        if bytes[i] == b'@'
            && !line[..i]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            if let Some((replacement, next_i)) = parse_typst_doc_ref(line, i) {
                out.push_str(&replacement);
                i = next_i;
                continue;
            }
        }
        let ch = line[i..].chars().next().unwrap_or('\0');
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

fn unwrap_typst_code_braces(inner: &str) -> Option<&str> {
    let body = inner.strip_prefix('{')?.strip_suffix('}')?;
    if body.is_empty() || body.contains([':', ';', '\n']) {
        return None;
    }
    let mut depth = 1i32;
    for ch in body.chars() {
        depth += i32::from(ch == '{') - i32::from(ch == '}');
        if depth <= 0 {
            return None;
        }
    }
    Some(body)
}

fn parse_doc_dollar_link(line: &str, start: usize) -> Option<(&str, usize)> {
    let mut depth = 0usize;
    let close_b = line[start..].char_indices().find_map(|(rel, ch)| {
        depth = match ch {
            '[' => depth + 1,
            ']' => depth.saturating_sub(1),
            _ => depth,
        };
        (ch == ']' && depth == 0).then_some(start + rel)
    })?;
    let after_b = line[close_b + 1..].strip_prefix("($")?;
    let close_paren = after_b.find(')')?;
    Some((&line[start + 1..close_b], close_b + 3 + close_paren + 1))
}

fn parse_typst_doc_ref(line: &str, at_idx: usize) -> Option<(String, usize)> {
    let rest = &line[at_idx + 1..];
    if !rest.chars().next()?.is_ascii_alphabetic() {
        return None;
    }
    let path_end = rest
        .char_indices()
        .take_while(|&(_, c)| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | ':'))
        .map(|(rel, c)| rel + c.len_utf8())
        .last()?;
    let target = rest[..path_end].trim_end_matches(['.', ':']);
    let len = (!target.is_empty()).then_some(target.len())?;
    let after = &rest[len..];
    if let Some(after_open) = after.strip_prefix('[') {
        if let Some(rel_close) = after_open.find(']') {
            let cleaned = clean_typst_doc_inline(&after_open[..rel_close]);
            return Some((cleaned, at_idx + 1 + len + 1 + rel_close + 1));
        }
    }
    Some((target.to_string(), at_idx + 1 + len))
}
