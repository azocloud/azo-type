//! Version-agnostic compiler trait, plain Rust boundary types, and external worker DTOs.

#[cfg(test)]
mod tests;

use std::path::PathBuf;
use std::sync::Arc;

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use egui::{ColorImage, Context, TextureHandle, TextureOptions};

use crate::core::outline::CompiledOutlineItem;
use crate::core::pdf::PdfExportConfig;
use crate::project::DiskProject;

/// Compiler warning or error message mapped to a source location.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DiagnosticItem {
    pub is_error: bool,
    pub file: PathBuf,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub message: String,
    pub hints: Vec<String>,
}

/// Glyph-level bounding box in Typst page point coordinates mapped to a project source char offset.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PreviewCharBox {
    pub x_min_pt: f32,
    pub y_min_pt: f32,
    pub x_max_pt: f32,
    pub y_max_pt: f32,
    pub file: PathBuf,
    pub char_idx: usize,
    pub text: String,
}

/// Version-agnostic rasterized page image and interactive glyph source map.
#[derive(Clone)]
pub struct RenderedPageData {
    pub page_number: usize,
    pub width_pt: f32,
    pub height_pt: f32,
    pub image: Arc<ColorImage>,
    pub char_boxes: Arc<Vec<PreviewCharBox>>,
}

/// Uploaded GPU texture handle and glyph map ready for rendering in the preview pane.
pub struct RenderedPageTexture {
    pub page_number: usize,
    pub width_pt: f32,
    pub height_pt: f32,
    pub texture: TextureHandle,
    pub char_boxes: Arc<Vec<PreviewCharBox>>,
}

/// Version-agnostic result produced by compiling a Typst project.
pub struct CompileSnapshot {
    pub revision: u64,
    pub pixel_per_pt: f32,
    pub compiled_ok: bool,
    pub pages: Vec<RenderedPageData>,
    pub outline: Vec<CompiledOutlineItem>,
    pub diagnostics: Vec<DiagnosticItem>,
    pub duration_ms: f64,
}

/// Identity and provenance metadata reported by an active [`TypstEngine`] implementation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineMetadata {
    pub version: String,
    pub is_builtin: bool,
    pub display_label: String,
}

/// Version-agnostic interface decoupling document compilation and PDF export from bundled Typst crates.
pub trait TypstEngine: Send + Sync {
    /// Returns engine version and provenance metadata.
    fn metadata(&self) -> EngineMetadata;

    /// Compiles and rasterizes the project at `pixel_per_pt` into a version-agnostic snapshot.
    fn compile_project(&self, project: &DiskProject, pixel_per_pt: f32) -> CompileSnapshot;

    /// Compiles and exports the project as PDF bytes configured by `config`.
    fn export_pdf_bytes(
        &self,
        project: &DiskProject,
        config: &PdfExportConfig,
    ) -> Result<Vec<u8>, Vec<DiagnosticItem>>;
}

/// Initialization handshake sent to JSON-stdio external Typst engine workers.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EngineInitHandshake {
    pub expected_version: String,
    pub package_cache_dir: Option<PathBuf>,
    pub include_system_fonts: bool,
}

/// Serialization boundary DTO requesting page compilation from an external engine worker.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EngineCompileRequest {
    pub revision: u64,
    pub pixel_per_pt: f32,
    pub root_dir: PathBuf,
    pub entry_file: PathBuf,
    pub in_memory_files: Vec<(PathBuf, Vec<u8>)>,
    pub package_cache_dir: Option<PathBuf>,
}

/// Serialized page image + glyph source boxes returned by an external worker engine.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RenderedPageDto {
    pub page_number: usize,
    pub width_pt: f32,
    pub height_pt: f32,
    pub rgba_width: usize,
    pub rgba_height: usize,
    pub rgba_base64: String,
    pub char_boxes: Vec<PreviewCharBox>,
}

/// Serialization boundary DTO returned after compilation by an external engine worker.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EngineCompileResponse {
    pub compiled_ok: bool,
    pub pages: Vec<RenderedPageDto>,
    pub outline: Vec<CompiledOutlineItem>,
    pub diagnostics: Vec<DiagnosticItem>,
}

/// Serialization boundary DTO requesting PDF generation from an external engine worker.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EngineExportPdfRequest {
    pub root_dir: PathBuf,
    pub entry_file: PathBuf,
    pub in_memory_files: Vec<(PathBuf, Vec<u8>)>,
    pub config: PdfExportConfig,
    pub package_cache_dir: Option<PathBuf>,
}

/// Serialization boundary DTO containing exported PDF bytes or structured diagnostics.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EngineExportPdfResponse {
    pub pdf_base64: Option<String>,
    pub diagnostics: Vec<DiagnosticItem>,
}

/// Encodes a [`RenderedPageData`] into a wire-serializable [`RenderedPageDto`].
pub fn encode_rendered_page_dto(page: &RenderedPageData) -> RenderedPageDto {
    let [rgba_width, rgba_height] = page.image.size;
    let raw_bytes: Vec<u8> = page
        .image
        .pixels
        .iter()
        .flat_map(|c| c.to_array())
        .collect();
    RenderedPageDto {
        page_number: page.page_number,
        width_pt: page.width_pt,
        height_pt: page.height_pt,
        rgba_width,
        rgba_height,
        rgba_base64: BASE64.encode(raw_bytes),
        char_boxes: page.char_boxes.as_ref().clone(),
    }
}

/// Decodes a [`RenderedPageDto`] back into a version-agnostic [`RenderedPageData`].
pub fn decode_rendered_page_dto(dto: RenderedPageDto) -> Result<RenderedPageData, String> {
    let raw = BASE64
        .decode(dto.rgba_base64.as_bytes())
        .map_err(|e| format!("Invalid page RGBA base64 payload: {e}"))?;
    let expected_len = dto
        .rgba_width
        .checked_mul(dto.rgba_height)
        .and_then(|px| px.checked_mul(4))
        .ok_or_else(|| "Rendered page dimensions overflowed".to_string())?;
    if raw.len() != expected_len {
        return Err(format!(
            "Page {} RGBA byte length mismatch: expected {}, got {}",
            dto.page_number,
            expected_len,
            raw.len()
        ));
    }

    let image = ColorImage::from_rgba_unmultiplied([dto.rgba_width, dto.rgba_height], &raw);
    Ok(RenderedPageData {
        page_number: dto.page_number,
        width_pt: dto.width_pt,
        height_pt: dto.height_pt,
        image: Arc::new(image),
        char_boxes: Arc::new(dto.char_boxes),
    })
}

/// Uploads rendered page RGBA images into GPU textures for the preview canvas.
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
                char_boxes: Arc::clone(&p.char_boxes),
            }
        })
        .collect()
}
