//! Incremental Typst document compiler, rasterizer, PDF exporter, and diagnostic collector.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use chrono::{Datelike, Local, Timelike};
use egui::ColorImage;
use typst::diag::{Severity, SourceDiagnostic};
use typst::foundations::{Datetime, NativeElement, StyleChain};
use typst::introspection::Introspector;
use typst::layout::{Abs, Frame, FrameItem, Point, Transform};
use typst::model::HeadingElem;
use typst::syntax::{FileId, VirtualRoot};
use typst::text::TextItem;
use typst::utils::Scalar;
use typst::{World, WorldExt};
use typst_layout::PagedDocument;
use typst_pdf::{PdfOptions, Timestamp};
use typst_render::RenderOptions;

use crate::core::diff::normalize_rel_path;
use crate::core::engine::BUILTIN_TYPST_VERSION;
use crate::engine::types::{EngineMetadata, TypstEngine};
use crate::project::DiskProject;
use crate::world::{EditorWorld, SharedWorldResources};

pub use crate::core::outline::CompiledOutlineItem;
pub use crate::core::pdf::{PdfArchivalStandard, PdfExportConfig, PdfVersionChoice};
pub use crate::engine::types::{
    upload_page_textures, CompileSnapshot, DiagnosticItem, PreviewCharBox, RenderedPageData,
    RenderedPageTexture,
};

/// Built-in Typst engine backed by the statically linked `typst` 0.15.1 compiler crates.
pub struct NativeTypstEngine {
    resources: Arc<SharedWorldResources>,
}

impl NativeTypstEngine {
    /// Creates a new built-in Typst 0.15.1 engine sharing font/package resources.
    pub fn new(resources: Arc<SharedWorldResources>) -> Self {
        Self { resources }
    }
}

impl TypstEngine for NativeTypstEngine {
    fn metadata(&self) -> EngineMetadata {
        EngineMetadata {
            version: BUILTIN_TYPST_VERSION.to_string(),
            is_builtin: true,
            display_label: format!("{BUILTIN_TYPST_VERSION} (Built-in)"),
        }
    }

    fn compile_project(&self, project: &DiskProject, pixel_per_pt: f32) -> CompileSnapshot {
        compile_project(Arc::clone(&self.resources), project, pixel_per_pt)
    }

    fn export_pdf_bytes(
        &self,
        project: &DiskProject,
        config: &PdfExportConfig,
    ) -> Result<Vec<u8>, Vec<DiagnosticItem>> {
        export_pdf_bytes_with_config(Arc::clone(&self.resources), project, config)
    }
}

pub fn compile_project(
    resources: Arc<SharedWorldResources>,
    project: &DiskProject,
    pixel_per_pt: f32,
) -> CompileSnapshot {
    let start = Instant::now();

    let world = EditorWorld::new(resources, project.clone());
    let warned = typst::compile::<PagedDocument>(&world);

    let mut diagnostics = Vec::new();
    for warning in &warned.warnings {
        diagnostics.push(extract_diagnostic(&world, warning));
    }

    let (compiled_ok, pages, outline) = match warned.output {
        Ok(doc) => {
            let doc_pages = doc.pages();
            let mut rendered_pages = Vec::with_capacity(doc_pages.len());
            let mut file_cache: HashMap<FileId, Option<(PathBuf, Vec<usize>)>> = HashMap::new();
            let scale = pixel_per_pt.clamp(1.0, 4.0);
            let render_options = RenderOptions {
                pixel_per_pt: Scalar::new(scale as f64),
                render_bleed: false,
            };
            for (idx, page) in doc_pages.iter().enumerate() {
                let size = page.frame.size();
                let width_pt = size.x.to_pt() as f32;
                let height_pt = size.y.to_pt() as f32;
                let pixmap = typst_render::render(page, &render_options);
                let color_image = ColorImage::from_rgba_premultiplied(
                    [pixmap.width() as usize, pixmap.height() as usize],
                    pixmap.data(),
                );
                let char_boxes = Arc::new(extract_page_char_boxes(page, &world, &mut file_cache));
                rendered_pages.push(RenderedPageData {
                    page_number: idx + 1,
                    width_pt,
                    height_pt,
                    image: Arc::new(color_image),
                    char_boxes,
                });
            }
            let compiled_outline = extract_compiled_outline(&world, &doc);
            (true, rendered_pages, compiled_outline)
        }
        Err(errors) => {
            for err in &errors {
                diagnostics.push(extract_diagnostic(&world, err));
            }
            (false, Vec::new(), Vec::new())
        }
    };

    let duration_ms = start.elapsed().as_secs_f64() * 1000.0;

    CompileSnapshot {
        revision: project.revision,
        pixel_per_pt,
        compiled_ok,
        pages,
        outline,
        diagnostics,
        duration_ms,
    }
}

pub fn export_pdf_bytes_with_config(
    resources: Arc<SharedWorldResources>,
    project: &DiskProject,
    config: &PdfExportConfig,
) -> Result<Vec<u8>, Vec<DiagnosticItem>> {
    let world = EditorWorld::new(resources, project.clone());
    let standards = config.build_standards().map_err(|msg| {
        vec![DiagnosticItem {
            is_error: true,
            file: world.display_path_for_id(world.main()),
            line: None,
            column: None,
            message: msg,
            hints: vec![
                "Select a PDF version compatible with the chosen PDF standard (e.g. PDF 1.4 for PDF/A-1, PDF 1.7 for PDF/A-2 and PDF/A-3, or PDF 2.0 for PDF/A-4).".to_string(),
            ],
        }]
    })?;

    let warned = typst::compile::<PagedDocument>(&world);

    match warned.output {
        Ok(doc) => {
            let options = PdfOptions {
                timestamp: current_pdf_timestamp(),
                standards,
                tagged: config.tagged
                    || config.pdf_ua1
                    || matches!(
                        config.archival_standard,
                        PdfArchivalStandard::A1a
                            | PdfArchivalStandard::A2a
                            | PdfArchivalStandard::A3a
                    ),
                ..PdfOptions::default()
            };
            typst_pdf::pdf(&doc, &options)
                .map_err(|errs| errs.iter().map(|e| extract_diagnostic(&world, e)).collect())
        }
        Err(errs) => Err(errs.iter().map(|e| extract_diagnostic(&world, e)).collect()),
    }
}

fn current_pdf_timestamp() -> Option<Timestamp> {
    let now = Local::now();
    let dt = Datetime::from_ymd_hms(
        now.year(),
        now.month().try_into().ok()?,
        now.day().try_into().ok()?,
        now.hour().try_into().ok()?,
        now.minute().try_into().ok()?,
        now.second().try_into().ok()?,
    )?;
    let minute_offset = now.offset().local_minus_utc() / 60;
    Timestamp::new_local(dt, minute_offset).or_else(|| Some(Timestamp::new_utc(dt)))
}

fn extract_compiled_outline(world: &EditorWorld, doc: &PagedDocument) -> Vec<CompiledOutlineItem> {
    let mut items = Vec::new();
    let elems = doc.introspector().query(&HeadingElem::ELEM.select());
    let default_file = world.display_path_for_id(world.main());

    for elem in elems.iter() {
        let Some(heading) = elem.to_packed::<HeadingElem>() else {
            continue;
        };
        if !heading.outlined.get(StyleChain::default()) {
            continue;
        }
        let level = heading.resolve_level(StyleChain::default()).get();
        let numbers = heading.numbers.as_ref().map(|n| n.to_string());
        let title = heading.body.plain_text().trim().to_string();
        if title.is_empty() {
            continue;
        }

        let span = heading.span();
        let mut file = default_file.clone();
        let mut line = 1usize;
        let mut char_offset = 0usize;

        if let Some(id) = span.id() {
            #[allow(deprecated)]
            let is_project_file = id.package().is_none();
            if is_project_file {
                file = world.display_path_for_id(id);
                if let Ok(source) = world.source(id) {
                    if let Some(range) = world.range(span) {
                        let byte_start = range.start.min(source.text().len());
                        line = source
                            .lines()
                            .byte_to_line(byte_start)
                            .map(|l| l + 1)
                            .unwrap_or(1);
                        char_offset = source.text()[..byte_start].chars().count();
                    }
                }
            }
        }

        items.push(CompiledOutlineItem {
            level,
            numbers,
            title,
            file,
            line,
            char_offset,
        });
    }

    items
}

fn extract_diagnostic(world: &EditorWorld, diag: &SourceDiagnostic) -> DiagnosticItem {
    let is_error = matches!(diag.severity, Severity::Error);
    let mut file = world.display_path_for_id(world.main());
    let mut line = None;
    let mut column = None;

    let mut target_span = diag.span;
    if target_span.id().is_none() {
        for traced in &diag.trace {
            if traced.span.id().is_some() {
                target_span = traced.span.into();
                break;
            }
        }
    }

    if let Some(id) = target_span.id() {
        file = world.display_path_for_id(id);
        if let Ok(source) = world.source(id) {
            if let Some(range) = world.range(target_span) {
                line = source.lines().byte_to_line(range.start).map(|l| l + 1);
                column = source.lines().byte_to_column(range.start).map(|c| c + 1);
            }
        }
    }

    let hints = diag.hints.iter().map(|h| h.v.to_string()).collect();

    DiagnosticItem {
        is_error,
        file,
        line,
        column,
        message: diag.message.to_string(),
        hints,
    }
}

/// Builds an O(1) UTF-8 byte-offset to `char` index lookup table of length `text.len() + 1`.
pub(crate) fn build_byte_to_char_table(text: &str) -> Vec<usize> {
    let mut table = vec![0usize; text.len() + 1];
    let mut char_idx = 0usize;
    for (byte_idx, ch) in text.char_indices() {
        let end = byte_idx + ch.len_utf8();
        for slot in &mut table[byte_idx..end] {
            *slot = char_idx;
        }
        char_idx += 1;
    }
    table[text.len()] = char_idx;
    table
}

fn extract_page_char_boxes(
    page: &typst_layout::Page,
    world: &EditorWorld,
    file_cache: &mut HashMap<FileId, Option<(PathBuf, Vec<usize>)>>,
) -> Vec<PreviewCharBox> {
    let mut out = Vec::new();
    collect_frame_char_boxes(
        &page.frame,
        Transform::identity(),
        world,
        file_cache,
        &mut out,
    );
    out
}

fn collect_frame_char_boxes(
    frame: &Frame,
    transform: Transform,
    world: &EditorWorld,
    file_cache: &mut HashMap<FileId, Option<(PathBuf, Vec<usize>)>>,
    out: &mut Vec<PreviewCharBox>,
) {
    for (pos, item) in frame.items() {
        match item {
            FrameItem::Group(group) => {
                let child_transform = transform
                    .pre_concat(Transform::translate(pos.x, pos.y))
                    .pre_concat(group.transform);
                collect_frame_char_boxes(&group.frame, child_transform, world, file_cache, out);
            }
            FrameItem::Text(text) => {
                let text_transform = transform.pre_concat(Transform::translate(pos.x, pos.y));
                collect_text_char_boxes(text, text_transform, world, file_cache, out);
            }
            _ => {}
        }
    }
}

fn collect_text_char_boxes(
    text: &TextItem,
    text_transform: Transform,
    world: &EditorWorld,
    file_cache: &mut HashMap<FileId, Option<(PathBuf, Vec<usize>)>>,
    out: &mut Vec<PreviewCharBox>,
) {
    let metrics = text.font.metrics();
    let ascender = metrics.ascender.at(text.size);
    let descender = metrics.descender.at(text.size);
    let above = if ascender > Abs::zero() {
        ascender
    } else {
        text.size * 0.8
    };
    let below = if descender < Abs::zero() {
        -descender
    } else {
        text.size * 0.2
    };

    let mut cursor_x = Abs::zero();
    let mut cursor_y = Abs::zero();
    let mut last_range: Option<std::ops::Range<usize>> = None;

    for glyph in &text.glyphs {
        let gx = cursor_x + glyph.x_offset.at(text.size);
        let gy = cursor_y + glyph.y_offset.at(text.size);
        let x_adv = glyph.x_advance.at(text.size);
        cursor_x += x_adv;
        cursor_y += glyph.y_advance.at(text.size);

        let (span, span_offset) = glyph.span;
        let Some(id) = span.id() else {
            continue;
        };
        if !matches!(id.root(), VirtualRoot::Project) {
            continue;
        }

        let cached = file_cache.entry(id).or_insert_with(|| {
            let source = world.source(id).ok()?;
            let raw_path = world.display_path_for_id(id);
            let norm = normalize_rel_path(&raw_path.to_string_lossy());
            let file = PathBuf::from(norm);
            let byte_to_char = build_byte_to_char_table(source.text());
            Some((file, byte_to_char))
        });
        let Some((file, byte_to_char)) = cached.as_ref() else {
            continue;
        };

        let Some(range) = world.range(span) else {
            continue;
        };
        let max_byte = byte_to_char.len().saturating_sub(1);
        let byte_offset = (range.start + span_offset as usize).min(max_byte);
        let char_idx = byte_to_char[byte_offset];

        let baseline_y = -gy;
        let y0 = baseline_y - above;
        let y1 = baseline_y + below;
        let x0 = gx;
        let x1 = gx + x_adv;

        let corners = [
            Point::new(x0, y0).transform(text_transform),
            Point::new(x1, y0).transform(text_transform),
            Point::new(x0, y1).transform(text_transform),
            Point::new(x1, y1).transform(text_transform),
        ];

        let mut x_min_pt = f32::INFINITY;
        let mut y_min_pt = f32::INFINITY;
        let mut x_max_pt = f32::NEG_INFINITY;
        let mut y_max_pt = f32::NEG_INFINITY;
        for c in corners {
            let cx = c.x.to_pt() as f32;
            let cy = c.y.to_pt() as f32;
            x_min_pt = x_min_pt.min(cx);
            y_min_pt = y_min_pt.min(cy);
            x_max_pt = x_max_pt.max(cx);
            y_max_pt = y_max_pt.max(cy);
        }

        let g_range = glyph.range();
        let glyph_text = if last_range.as_ref() == Some(&g_range) {
            String::new()
        } else {
            text.text.get(g_range.clone()).unwrap_or("").to_string()
        };
        last_range = Some(g_range);

        out.push(PreviewCharBox {
            x_min_pt,
            y_min_pt,
            x_max_pt,
            y_max_pt,
            file: file.clone(),
            char_idx,
            text: glyph_text,
        });
    }
}

#[cfg(test)]
mod tests;
