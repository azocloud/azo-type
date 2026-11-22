//! Built-in Typst standard library symbol catalog and keyword documentation.
//!
//! Lazily extracts global and math scope bindings, parameter signatures, and
//! embedded Typst documentation from `typst::Library::default()`.

use std::collections::HashMap;
use std::sync::OnceLock;

use typst::foundations::{Func, Scope, Value};
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
                    val = f.scope()?.get(seg)?.read().clone();
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
                Value::Func(f) => match f.scope().and_then(|s| s.get(seg)) {
                    Some(b) => b.read().clone(),
                    None => return Vec::new(),
                },
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
            let docs = func.docs().unwrap_or("").trim().to_string();
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
                ty.docs().trim().to_string(),
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
