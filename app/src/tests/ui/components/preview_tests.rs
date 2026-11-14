//! Headless `egui_kittest` tests for the live Typst preview pane.

use std::path::PathBuf;
use std::sync::Arc;

use egui::ColorImage;
use egui_kittest::{kittest::Queryable, Harness};
use rust_i18n::t;

use super::*;
use crate::engine::{DiagnosticItem, PreviewCharBox};

#[test]
fn preview_pane_renders_pages_diagnostics_and_toggles_toolbar_modes() {
    struct PreviewState {
        zoom: PreviewZoom,
        sync_selection: bool,
        pan_mode: bool,
        selection: PreviewSelectionState,
        pages: Option<Vec<RenderedPageTexture>>,
        diagnostics: Vec<DiagnosticItem>,
    }

    let diagnostics = vec![DiagnosticItem {
        is_error: true,
        file: PathBuf::from("main.typ"),
        line: Some(3),
        column: Some(1),
        message: "unknown variable".to_string(),
        hints: vec!["Check spelling".to_string()],
    }];

    let pan_label = t!("preview.pan_mode_short", locale = "en").to_string();
    let sync_label = t!("preview.sync_selection_short", locale = "en").to_string();

    let mut harness = Harness::new_ui_state(
        |ui, state: &mut PreviewState| {
            let pages = state.pages.get_or_insert_with(|| {
                let tex = ui.ctx().load_texture(
                    "test_preview_page",
                    ColorImage::example(),
                    egui::TextureOptions::LINEAR,
                );
                vec![RenderedPageTexture {
                    page_number: 1,
                    width_pt: 200.0,
                    height_pt: 280.0,
                    texture: tex,
                    char_boxes: Arc::new(Vec::<PreviewCharBox>::new()),
                }]
            });
            render_preview_pane(
                ui,
                PreviewPaneParams {
                    dark_mode: true,
                    zoom: &mut state.zoom,
                    sync_selection: &mut state.sync_selection,
                    pan_mode: &mut state.pan_mode,
                    page_textures: pages,
                    diagnostics: &state.diagnostics,
                    selection: &mut state.selection,
                },
            );
        },
        PreviewState {
            zoom: PreviewZoom::default(),
            sync_selection: true,
            pan_mode: false,
            selection: PreviewSelectionState::default(),
            pages: None,
            diagnostics,
        },
    );

    harness.get_by_label(&pan_label).click();
    harness.run();
    assert!(harness.state().pan_mode);

    harness.get_by_label(&sync_label).click();
    harness.run();
    assert!(!harness.state().sync_selection);
}

#[test]
fn preview_pane_renders_empty_state_without_panicking() {
    struct EmptyPreviewState {
        zoom: PreviewZoom,
        sync_selection: bool,
        pan_mode: bool,
        selection: PreviewSelectionState,
    }

    let mut harness = Harness::new_ui_state(
        |ui, s: &mut EmptyPreviewState| {
            render_preview_pane(
                ui,
                PreviewPaneParams {
                    dark_mode: false,
                    zoom: &mut s.zoom,
                    sync_selection: &mut s.sync_selection,
                    pan_mode: &mut s.pan_mode,
                    page_textures: &[],
                    diagnostics: &[],
                    selection: &mut s.selection,
                },
            );
        },
        EmptyPreviewState {
            zoom: PreviewZoom::default(),
            sync_selection: false,
            pan_mode: false,
            selection: PreviewSelectionState::default(),
        },
    );
    harness.run();
}
