use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use chrono::{Datelike, Local, Timelike};
use egui::{ColorImage, Context, TextureHandle, TextureOptions};
use typst::diag::{Severity, SourceDiagnostic};
use typst::foundations::{Datetime, NativeElement, StyleChain};
use typst::introspection::Introspector;
use typst::model::HeadingElem;
use typst::utils::Scalar;
use typst::{World, WorldExt};
use typst_layout::PagedDocument;
use typst_pdf::{PdfOptions, Timestamp};
use typst_render::RenderOptions;

use crate::project::DiskProject;
use crate::world::{EditorWorld, SharedWorldResources};

pub use crate::core::outline::CompiledOutlineItem;
pub use crate::core::pdf::{PdfArchivalStandard, PdfExportConfig, PdfVersionChoice};

#[derive(Debug, Clone)]
pub struct DiagnosticItem {
    pub is_error: bool,
    pub file: PathBuf,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub message: String,
    pub hints: Vec<String>,
}

#[derive(Clone)]
pub struct RenderedPageData {
    pub page_number: usize,
    pub width_pt: f32,
    pub height_pt: f32,
    pub image: Arc<ColorImage>,
}

pub struct RenderedPageTexture {
    pub page_number: usize,
    pub width_pt: f32,
    pub height_pt: f32,
    pub texture: TextureHandle,
}

pub struct CompileSnapshot {
    pub revision: u64,
    pub pixel_per_pt: f32,
    pub document: Option<Arc<PagedDocument>>,
    pub pages: Vec<RenderedPageData>,
    pub outline: Vec<CompiledOutlineItem>,
    pub diagnostics: Vec<DiagnosticItem>,
    pub duration_ms: f64,
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

    let (document, pages, outline) = match warned.output {
        Ok(doc) => {
            let doc_pages = doc.pages();
            let mut rendered_pages = Vec::with_capacity(doc_pages.len());
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
                rendered_pages.push(RenderedPageData {
                    page_number: idx + 1,
                    width_pt,
                    height_pt,
                    image: Arc::new(color_image),
                });
            }
            let compiled_outline = extract_compiled_outline(&world, &doc);
            (Some(Arc::new(doc)), rendered_pages, compiled_outline)
        }
        Err(errors) => {
            for err in &errors {
                diagnostics.push(extract_diagnostic(&world, err));
            }
            (None, Vec::new(), Vec::new())
        }
    };

    let duration_ms = start.elapsed().as_secs_f64() * 1000.0;

    CompileSnapshot {
        revision: project.revision,
        pixel_per_pt,
        document,
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

pub fn upload_page_textures(ctx: &Context, pages: &[RenderedPageData]) -> Vec<RenderedPageTexture> {
    pages
        .iter()
        .map(|p| {
            let texture = ctx.load_texture(
                format!("typst_page_{}", p.page_number),
                Arc::clone(&p.image),
                TextureOptions::LINEAR,
            );
            RenderedPageTexture {
                page_number: p.page_number,
                width_pt: p.width_pt,
                height_pt: p.height_pt,
                texture,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests;
