//! Isolated workspace staging, PNG page decoding, and CLI stderr/query parsing helpers.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use egui::ColorImage;
use serde_json::Value;
use uuid::Uuid;

use crate::core::diff::normalize_rel_path;
use crate::core::outline::CompiledOutlineItem;
use crate::engine::types::{DiagnosticItem, RenderedPageData};
use crate::project::DiskProject;

/// RAII temporary directory automatically deleted on drop.
pub(crate) struct TempDirGuard {
    path: PathBuf,
}

impl TempDirGuard {
    pub(crate) fn new(prefix: &str) -> std::io::Result<Self> {
        let path = std::env::temp_dir().join(format!("{prefix}-{}", Uuid::new_v4()));
        fs::create_dir_all(&path)?;
        Ok(Self { path })
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// Holds either the real on-disk project root or an isolated staged copy with unsaved buffers overlaid.
pub(crate) struct StagedWorkspace {
    _temp_guard: Option<TempDirGuard>,
    pub(crate) root_dir: PathBuf,
    pub(crate) entry_rel: PathBuf,
    pub(crate) entry_abs: PathBuf,
}

impl StagedWorkspace {
    pub(crate) fn prepare(project: &DiskProject) -> Result<Self, String> {
        let entry_rel = normalize_rel_pathbuf(&project.effective_compile_entrypoint());
        let has_dirty_buffers = project.buffers.values().any(|b| b.dirty);

        if project.has_disk_backing() && !has_dirty_buffers {
            let root_dir = project.root_dir.clone();
            let entry_abs = root_dir.join(&entry_rel);
            return Ok(Self {
                _temp_guard: None,
                root_dir,
                entry_rel,
                entry_abs,
            });
        }

        let guard = TempDirGuard::new("azo-type-stage")
            .map_err(|e| format!("Unable to create staging dir: {e}"))?;
        let root_dir = guard.path().to_path_buf();

        if project.has_disk_backing() {
            copy_dir_tree(&project.root_dir, &root_dir, project.show_hidden_files)
                .map_err(|e| format!("Unable to copy project files for preview: {e}"))?;
        }

        for (rel_path, buf) in &project.buffers {
            if let Some(ref text) = buf.text {
                let target = root_dir.join(normalize_rel_pathbuf(rel_path));
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent)
                        .map_err(|e| format!("Unable to create staging parent dir: {e}"))?;
                }
                fs::write(&target, text.as_bytes()).map_err(|e| {
                    format!("Unable to write staged buffer '{}': {e}", target.display())
                })?;
            }
        }

        let entry_abs = root_dir.join(&entry_rel);
        Ok(Self {
            _temp_guard: Some(guard),
            root_dir,
            entry_rel,
            entry_abs,
        })
    }
}

pub(crate) fn normalize_rel_pathbuf(path: &Path) -> PathBuf {
    PathBuf::from(normalize_rel_path(&path.to_string_lossy()))
}

fn copy_dir_tree(src: &Path, dst: &Path, include_hidden: bool) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if (!include_hidden && name_str.starts_with('.'))
            || matches!(name_str.as_ref(), "target" | ".git" | ".typst-collab")
        {
            continue;
        }
        let file_type = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(&name);
        if file_type.is_dir() {
            copy_dir_tree(&src_path, &dst_path, include_hidden)?;
        } else if file_type.is_file() {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

pub(crate) fn decode_rendered_png_pages(
    out_dir: &Path,
    scale: f32,
) -> Result<Vec<RenderedPageData>, String> {
    let mut png_files: Vec<(usize, PathBuf)> = Vec::new();
    let entries = fs::read_dir(out_dir)
        .map_err(|e| format!("Failed reading rendered page output directory: {e}"))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("png") {
            let page_num = parse_page_number_from_filename(&path).unwrap_or(usize::MAX);
            png_files.push((page_num, path));
        }
    }
    png_files.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));

    let mut pages = Vec::with_capacity(png_files.len());
    for (idx, (_parsed_num, path)) in png_files.into_iter().enumerate() {
        let bytes = fs::read(&path)
            .map_err(|e| format!("Failed reading rendered PNG '{}': {e}", path.display()))?;
        let dyn_img = image::load_from_memory(&bytes)
            .map_err(|e| format!("Failed decoding PNG '{}': {e}", path.display()))?;
        let rgba = dyn_img.to_rgba8();
        let w = rgba.width() as usize;
        let h = rgba.height() as usize;
        let color_image = ColorImage::from_rgba_unmultiplied([w, h], rgba.as_raw());
        pages.push(RenderedPageData {
            page_number: idx + 1,
            width_pt: w as f32 / scale,
            height_pt: h as f32 / scale,
            image: Arc::new(color_image),
            char_boxes: Arc::new(Vec::new()),
        });
    }
    Ok(pages)
}

fn parse_page_number_from_filename(path: &Path) -> Option<usize> {
    let stem = path.file_stem()?.to_str()?;
    let digits = stem.strip_prefix("page-").unwrap_or(stem);
    digits.parse::<usize>().ok()
}

/// Parses JSON output produced by `typst query <input> heading` into [`CompiledOutlineItem`]s.
pub(crate) fn parse_cli_outline_json(
    raw_json: &str,
    default_file: &Path,
) -> Result<Vec<CompiledOutlineItem>, String> {
    let val: Value =
        serde_json::from_str(raw_json).map_err(|e| format!("Invalid typst query JSON: {e}"))?;
    let Some(arr) = val.as_array() else {
        return Ok(Vec::new());
    };

    let mut items = Vec::new();
    for item in arr {
        if item.get("outlined").and_then(|v| v.as_bool()) == Some(false) {
            continue;
        }
        let level = item
            .get("level")
            .or_else(|| item.get("depth"))
            .and_then(|v| v.as_u64())
            .unwrap_or(1)
            .max(1) as usize;
        let mut title = String::new();
        if let Some(body) = item.get("body") {
            collect_json_plain_text(body, &mut title);
        }
        let trimmed = title.trim().to_string();
        if trimmed.is_empty() {
            continue;
        }
        items.push(CompiledOutlineItem {
            level,
            numbers: None,
            title: trimmed,
            file: default_file.to_path_buf(),
            line: 1,
            char_offset: 0,
        });
    }
    Ok(items)
}

fn collect_json_plain_text(value: &Value, out: &mut String) {
    match value {
        Value::String(s) => out.push_str(s),
        Value::Array(arr) => {
            for item in arr {
                collect_json_plain_text(item, out);
            }
        }
        Value::Object(map) => {
            if let Some(Value::String(text)) = map.get("text") {
                out.push_str(text);
            } else if map.get("func").and_then(|f| f.as_str()) == Some("space") {
                out.push(' ');
            }
            if let Some(body) = map.get("body") {
                collect_json_plain_text(body, out);
            }
            if let Some(children) = map.get("children") {
                collect_json_plain_text(children, out);
            }
        }
        _ => {}
    }
}

/// Parses structured or unstructured stderr diagnostics emitted by the `typst` CLI.
pub(crate) fn parse_cli_diagnostics(
    stderr: &str,
    root_dir: &Path,
    default_file: &Path,
    command_failed: bool,
) -> Vec<DiagnosticItem> {
    let clean = strip_ansi_escapes(stderr);
    let mut items: Vec<DiagnosticItem> = Vec::new();
    let mut current: Option<DiagnosticItem> = None;

    for raw_line in clean.lines() {
        let line = raw_line.trim_end();
        if let Some(msg) = line.strip_prefix("error:") {
            if let Some(prev) = current.take() {
                items.push(prev);
            }
            current = Some(DiagnosticItem {
                is_error: true,
                file: default_file.to_path_buf(),
                line: None,
                column: None,
                message: msg.trim().to_string(),
                hints: Vec::new(),
            });
            continue;
        }
        if let Some(msg) = line.strip_prefix("warning:") {
            if let Some(prev) = current.take() {
                items.push(prev);
            }
            current = Some(DiagnosticItem {
                is_error: false,
                file: default_file.to_path_buf(),
                line: None,
                column: None,
                message: msg.trim().to_string(),
                hints: Vec::new(),
            });
            continue;
        }
        if let Some(diag) = current.as_mut() {
            let trimmed = line.trim_start();
            if let Some(loc) = trimmed.strip_prefix("┌─ ") {
                if let Some((path, ln, col)) = parse_diagnostic_location(loc.trim(), root_dir) {
                    diag.file = path;
                    diag.line = Some(ln);
                    diag.column = Some(col);
                }
            } else if let Some(hint) = trimmed
                .strip_prefix("help:")
                .or_else(|| trimmed.strip_prefix("hint:"))
            {
                let hint_text = hint.trim();
                if !hint_text.is_empty() {
                    diag.hints.push(hint_text.to_string());
                }
            }
        }
    }

    if let Some(prev) = current.take() {
        items.push(prev);
    }

    if items.is_empty() && command_failed {
        let msg = clean.trim();
        items.push(DiagnosticItem {
            is_error: true,
            file: default_file.to_path_buf(),
            line: None,
            column: None,
            message: if msg.is_empty() {
                "External Typst compiler failed without stderr output.".to_string()
            } else {
                msg.to_string()
            },
            hints: Vec::new(),
        });
    }

    items
}

fn parse_diagnostic_location(loc: &str, root_dir: &Path) -> Option<(PathBuf, usize, usize)> {
    let mut parts = loc.rsplitn(3, ':');
    let col = parts.next()?.trim().parse::<usize>().ok()?;
    let ln = parts.next()?.trim().parse::<usize>().ok()?;
    let raw_path = parts.next()?.trim();
    let p = Path::new(raw_path);
    let rel = p.strip_prefix(root_dir).unwrap_or(p);
    Some((normalize_rel_pathbuf(rel), ln, col))
}

fn strip_ansi_escapes(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' && chars.peek() == Some(&'[') {
            chars.next();
            for next in chars.by_ref() {
                if ('@'..='~').contains(&next) {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}
