use std::sync::Arc;

use egui::{ColorImage, Context, TextureHandle, TextureOptions};
use typst::diag::{Severity, SourceDiagnostic};
use typst::model::Document;
use typst::World;
use typst_pdf::PdfOptions;

use crate::vfs::{normalize_vpath, ProjectWorkspace};
use crate::world::{EditorWorld, SharedWorldResources};

#[derive(Debug, Clone)]
pub struct DiagnosticItem {
    pub is_error: bool,
    pub file: String,
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
    pub document: Option<Arc<Document>>,
    pub pages: Vec<RenderedPageData>,
    pub diagnostics: Vec<DiagnosticItem>,
    pub duration_ms: f64,
}

pub fn compile_workspace(
    resources: Arc<SharedWorldResources>,
    workspace: &ProjectWorkspace,
    pixel_per_pt: f32,
) -> CompileSnapshot {
    #[cfg(not(target_arch = "wasm32"))]
    let start = std::time::Instant::now();

    let world = EditorWorld::new(resources, workspace.clone());
    let warned = typst::compile(&world);

    let mut diagnostics = Vec::new();
    for warning in &warned.warnings {
        diagnostics.push(extract_diagnostic(&world, warning));
    }

    let (document, pages) = match warned.output {
        Ok(doc) => {
            let mut rendered_pages = Vec::with_capacity(doc.pages.len());
            let scale = pixel_per_pt.clamp(1.0, 4.0);
            for (idx, page) in doc.pages.iter().enumerate() {
                let size = page.frame.size();
                let width_pt = size.x.to_pt() as f32;
                let height_pt = size.y.to_pt() as f32;
                let pixmap = typst_render::render(page, scale);
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
            (Some(Arc::new(doc)), rendered_pages)
        }
        Err(errors) => {
            for err in &errors {
                diagnostics.push(extract_diagnostic(&world, err));
            }
            (None, Vec::new())
        }
    };

    #[cfg(not(target_arch = "wasm32"))]
    let duration_ms = start.elapsed().as_secs_f64() * 1000.0;
    #[cfg(target_arch = "wasm32")]
    let duration_ms = 4.2;

    CompileSnapshot {
        revision: workspace.revision,
        pixel_per_pt,
        document,
        pages,
        diagnostics,
        duration_ms,
    }
}

pub fn export_pdf_bytes(
    resources: Arc<SharedWorldResources>,
    workspace: &ProjectWorkspace,
) -> Result<Vec<u8>, Vec<DiagnosticItem>> {
    let world = EditorWorld::new(resources, workspace.clone());
    let warned = typst::compile(&world);

    match warned.output {
        Ok(doc) => {
            let options = PdfOptions::default();
            typst_pdf::pdf(&doc, &options).map_err(|errs| {
                errs.iter()
                    .map(|e| extract_diagnostic(&world, e))
                    .collect()
            })
        }
        Err(errs) => Err(errs
            .iter()
            .map(|e| extract_diagnostic(&world, e))
            .collect()),
    }
}

fn extract_diagnostic(world: &EditorWorld, diag: &SourceDiagnostic) -> DiagnosticItem {
    let is_error = matches!(diag.severity, Severity::Error);
    let mut file = world
        .main()
        .vpath()
        .as_rootless_path()
        .to_string_lossy()
        .to_string();
    let mut line = None;
    let mut column = None;

    let mut target_span = diag.span;
    if target_span.id().is_none() {
        for traced in &diag.trace {
            if traced.span.id().is_some() {
                target_span = traced.span;
                break;
            }
        }
    }

    if let Some(id) = target_span.id() {
        file = normalize_vpath(&id.vpath().as_rootless_path().to_string_lossy());
        if let Ok(source) = world.source(id) {
            if let Some(range) = source.range(target_span) {
                line = source.byte_to_line(range.start).map(|l| l + 1);
                column = source.byte_to_column(range.start).map(|c| c + 1);
            }
        }
    }

    let hints = diag.hints.iter().map(|h| h.to_string()).collect();

    DiagnosticItem {
        is_error,
        file,
        line,
        column,
        message: diag.message.to_string(),
        hints,
    }
}

pub fn upload_page_textures(
    ctx: &Context,
    pages: &[RenderedPageData],
) -> Vec<RenderedPageTexture> {
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
