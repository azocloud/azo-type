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
use typst_pdf::{PdfOptions, PdfStandard, PdfStandards, Timestamp};
use typst_render::RenderOptions;

use crate::project::DiskProject;
use crate::world::{EditorWorld, SharedWorldResources};

/// Selectable PDF version in the Export PDF dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdfVersionChoice {
    V1_4,
    V1_5,
    V1_6,
    V1_7,
    V2_0,
}

impl PdfVersionChoice {
    pub const ALL: &'static [PdfVersionChoice] = &[
        Self::V1_4,
        Self::V1_5,
        Self::V1_6,
        Self::V1_7,
        Self::V2_0,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::V1_4 => "PDF 1.4",
            Self::V1_5 => "PDF 1.5",
            Self::V1_6 => "PDF 1.6",
            Self::V1_7 => "PDF 1.7 (Default)",
            Self::V2_0 => "PDF 2.0",
        }
    }

    pub fn short_label(self) -> &'static str {
        match self {
            Self::V1_4 => "1.4",
            Self::V1_5 => "1.5",
            Self::V1_6 => "1.6",
            Self::V1_7 => "1.7",
            Self::V2_0 => "2.0",
        }
    }

    pub fn to_standard(self) -> PdfStandard {
        match self {
            Self::V1_4 => PdfStandard::V_1_4,
            Self::V1_5 => PdfStandard::V_1_5,
            Self::V1_6 => PdfStandard::V_1_6,
            Self::V1_7 => PdfStandard::V_1_7,
            Self::V2_0 => PdfStandard::V_2_0,
        }
    }
}

/// Selectable PDF archival (PDF/A) standard in the Export PDF dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdfArchivalStandard {
    None,
    A1b,
    A1a,
    A2b,
    A2u,
    A2a,
    A3b,
    A3u,
    A3a,
    A4,
    A4f,
    A4e,
}

impl PdfArchivalStandard {
    pub const ALL: &'static [PdfArchivalStandard] = &[
        Self::None,
        Self::A1b,
        Self::A1a,
        Self::A2b,
        Self::A2u,
        Self::A2a,
        Self::A3b,
        Self::A3u,
        Self::A3a,
        Self::A4,
        Self::A4f,
        Self::A4e,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::None => "None (Standard PDF)",
            Self::A1b => "PDF/A-1b (ISO 19005-1 Level B — Basic Archival)",
            Self::A1a => "PDF/A-1a (ISO 19005-1 Level A — Accessible Archival)",
            Self::A2b => "PDF/A-2b (ISO 19005-2 Level B — Basic Archival)",
            Self::A2u => "PDF/A-2u (ISO 19005-2 Level U — Unicode Archival)",
            Self::A2a => "PDF/A-2a (ISO 19005-2 Level A — Accessible Archival)",
            Self::A3b => "PDF/A-3b (ISO 19005-3 Level B — Embedded Files)",
            Self::A3u => "PDF/A-3u (ISO 19005-3 Level U — Unicode + Embedded)",
            Self::A3a => "PDF/A-3a (ISO 19005-3 Level A — Accessible + Embedded)",
            Self::A4 => "PDF/A-4 (ISO 19005-4 — Modern PDF 2.0 Archival)",
            Self::A4f => "PDF/A-4f (ISO 19005-4 — Archival with Embedded Files)",
            Self::A4e => "PDF/A-4e (ISO 19005-4 — Engineering Archival)",
        }
    }

    pub fn code_label(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::A1b => "PDF/A-1b",
            Self::A1a => "PDF/A-1a",
            Self::A2b => "PDF/A-2b",
            Self::A2u => "PDF/A-2u",
            Self::A2a => "PDF/A-2a",
            Self::A3b => "PDF/A-3b",
            Self::A3u => "PDF/A-3u",
            Self::A3a => "PDF/A-3a",
            Self::A4 => "PDF/A-4",
            Self::A4f => "PDF/A-4f",
            Self::A4e => "PDF/A-4e",
        }
    }

    pub fn to_standard(self) -> Option<PdfStandard> {
        match self {
            Self::None => None,
            Self::A1b => Some(PdfStandard::A_1b),
            Self::A1a => Some(PdfStandard::A_1a),
            Self::A2b => Some(PdfStandard::A_2b),
            Self::A2u => Some(PdfStandard::A_2u),
            Self::A2a => Some(PdfStandard::A_2a),
            Self::A3b => Some(PdfStandard::A_3b),
            Self::A3u => Some(PdfStandard::A_3u),
            Self::A3a => Some(PdfStandard::A_3a),
            Self::A4 => Some(PdfStandard::A_4),
            Self::A4f => Some(PdfStandard::A_4f),
            Self::A4e => Some(PdfStandard::A_4e),
        }
    }

    /// Returns the recommended PDF version for this archival standard.
    pub fn recommended_version(self) -> Option<PdfVersionChoice> {
        match self {
            Self::None => None,
            Self::A1b | Self::A1a => Some(PdfVersionChoice::V1_4),
            Self::A2b | Self::A2u | Self::A2a | Self::A3b | Self::A3u | Self::A3a => {
                Some(PdfVersionChoice::V1_7)
            }
            Self::A4 | Self::A4f | Self::A4e => Some(PdfVersionChoice::V2_0),
        }
    }
}

/// Configuration for PDF export chosen by the user in the Export PDF dialog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfExportConfig {
    pub version: PdfVersionChoice,
    pub archival_standard: PdfArchivalStandard,
    pub pdf_ua1: bool,
    pub tagged: bool,
}

impl Default for PdfExportConfig {
    fn default() -> Self {
        Self {
            version: PdfVersionChoice::V1_7,
            archival_standard: PdfArchivalStandard::None,
            pdf_ua1: false,
            tagged: true,
        }
    }
}

impl PdfExportConfig {
    pub fn build_standards(&self) -> Result<PdfStandards, String> {
        let mut list = vec![self.version.to_standard()];
        if let Some(a_std) = self.archival_standard.to_standard() {
            list.push(a_std);
        }
        if self.pdf_ua1 {
            list.push(PdfStandard::Ua_1);
        }
        PdfStandards::new(&list).map_err(|e| e.message().to_string())
    }

    pub fn summary_badge(&self) -> String {
        let mut parts = vec![format!("PDF {}", self.version.short_label())];
        if self.archival_standard != PdfArchivalStandard::None {
            parts.push(self.archival_standard.code_label().to_string());
        }
        if self.pdf_ua1 {
            parts.push("PDF/UA-1".to_string());
        }
        parts.join(" + ")
    }
}

#[derive(Debug, Clone)]
pub struct DiagnosticItem {
    pub is_error: bool,
    pub file: PathBuf,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub message: String,
    pub hints: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledOutlineItem {
    pub level: usize,
    pub numbers: Option<String>,
    pub title: String,
    pub file: PathBuf,
    pub line: usize,
    pub char_offset: usize,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merman_package_import_and_pdf_standards_export() {
        let temp_dir = std::env::temp_dir().join(format!(
            "typst_ide_merman_test_{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let test_typ_content = r#"// --- Pakete für Mermaid-Diagramme ---
#import "@preview/merman:0.5.0": mermaid

// --- 1. Natürliche Sprache ganz am Anfang setzen (Guide: Natural Language) ---
#set text(
  font: "Libertinus Serif",
  lang: "de",
  region: "DE",
  size: 11pt,
)

= Architektur-Übersicht

#mermaid("graph LR\n  A[EditorWorld] --> B[SystemPackages]\n  B --> C[Live Preview]", alt: "Architektur-Diagramm")
"#;
        std::fs::write(temp_dir.join("test.typ"), test_typ_content).unwrap();

        let project = DiskProject::open_folder(&temp_dir, false).unwrap();
        let resources = Arc::new(SharedWorldResources::new());

        let snapshot = compile_project(Arc::clone(&resources), &project, 2.0);
        let errors: Vec<_> = snapshot
            .diagnostics
            .iter()
            .filter(|d| d.is_error)
            .collect();
        assert!(
            errors.is_empty(),
            "Expected @preview/merman:0.5.0 compilation to succeed, got errors: {:?}",
            errors
        );
        assert!(
            !snapshot.pages.is_empty(),
            "Expected at least 1 rendered preview page"
        );

        // Test exporting with PDF 1.7 + PDF/A-3a (just like `typst compile --pdf-standard a-3a`)
        let cfg_a3a = PdfExportConfig {
            version: PdfVersionChoice::V1_7,
            archival_standard: PdfArchivalStandard::A3a,
            pdf_ua1: false,
            tagged: true,
        };
        let pdf_a3a = export_pdf_bytes_with_config(Arc::clone(&resources), &project, &cfg_a3a)
            .expect("PDF/A-3a export should succeed");
        assert!(pdf_a3a.starts_with(b"%PDF-1.7"));

        // Test exporting with PDF 1.7 + PDF/A-2a
        let cfg_a2a = PdfExportConfig {
            version: PdfVersionChoice::V1_7,
            archival_standard: PdfArchivalStandard::A2a,
            pdf_ua1: false,
            tagged: true,
        };
        let pdf_a2a = export_pdf_bytes_with_config(Arc::clone(&resources), &project, &cfg_a2a)
            .expect("PDF/A-2a export should succeed");
        assert!(pdf_a2a.starts_with(b"%PDF-1.7"));

        // Test exporting with PDF 1.4, 1.5, 1.6
        for ver in [
            PdfVersionChoice::V1_4,
            PdfVersionChoice::V1_5,
            PdfVersionChoice::V1_6,
        ] {
            let cfg = PdfExportConfig {
                version: ver,
                archival_standard: PdfArchivalStandard::None,
                pdf_ua1: false,
                tagged: true,
            };
            let bytes = export_pdf_bytes_with_config(Arc::clone(&resources), &project, &cfg)
                .expect("PDF version export should succeed");
            let expected_header = format!("%PDF-{}", ver.short_label());
            assert!(bytes.starts_with(expected_header.as_bytes()));
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
