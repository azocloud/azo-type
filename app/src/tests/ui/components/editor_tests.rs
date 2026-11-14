//! Pure unit and headless `egui_kittest` tests for the editor pane component.

use std::path::PathBuf;

use egui_kittest::{kittest::Queryable, Harness};
use rust_i18n::t;

use super::*;
use crate::core::spellcheck::DiagnosticCategory;
use crate::project::buffer::OpenBuffer;

#[test]
fn adjust_editor_font_size_clamps_bounds() {
    assert_eq!(adjust_editor_font_size(14.0, 1.0), 15.0);
    assert_eq!(adjust_editor_font_size(22.0, 3.0), 22.0);
    assert_eq!(adjust_editor_font_size(10.0, -5.0), 10.0);
}

#[test]
fn compute_gutter_width_respects_visibility_and_scales_with_digits() {
    assert_eq!(compute_gutter_width(500, 14.0, false), 0.0);
    let w_small = compute_gutter_width(10, 14.0, true);
    let w_large = compute_gutter_width(10_000, 14.0, true);
    assert!(w_small >= 40.0);
    assert!(w_large > w_small);
}

#[test]
fn editor_pane_renders_typst_toolbar_actions_and_tabs() {
    struct EditorState {
        project: DiskProject,
        font_size: f32,
        word_wrap: bool,
        inspect_asset_mode: bool,
        clipboard_fallback: String,
        selected_tab: Option<PathBuf>,
        closed_tab: Option<PathBuf>,
        text_changed: bool,
    }

    let mut project = DiskProject::blank();
    let second_path = PathBuf::from("chapter.typ");
    project.buffers.insert(
        second_path.clone(),
        OpenBuffer {
            rel_path: second_path.clone(),
            abs_path: PathBuf::from("/tmp/chapter.typ"),
            kind: FileKind::Typst,
            text: Some("= Chapter 1\nBody text.".to_string()),
            size_bytes: 22,
            dirty: true,
        },
    );
    project.open_tabs.push(second_path.clone());

    let state = EditorState {
        project,
        font_size: 14.0,
        word_wrap: true,
        inspect_asset_mode: false,
        clipboard_fallback: String::new(),
        selected_tab: None,
        closed_tab: None,
        text_changed: false,
    };

    let diag = LanguageCheckDiagnostic {
        category: DiagnosticCategory::Spelling,
        start_byte: 2,
        end_byte: 9,
        start_char: 2,
        end_char: 9,
        line: 1,
        column: 3,
        excerpt: "Hello".to_string(),
        message: "Unknown word".to_string(),
        rule_id: "SPELLBOOK".to_string(),
        suggestions: vec!["Hello".to_string()],
    };

    let heading_label = t!("editor.heading_btn", locale = "en").to_string();
    let wrap_label = t!("editor.wrap_toggle", locale = "en").to_string();

    let mut harness = Harness::new_ui_state(
        |ui, state: &mut EditorState| {
            let ctx = ui.ctx().clone();
            let out = render_editor_pane(
                &ctx,
                ui,
                EditorPaneParams {
                    dark_mode: true,
                    editor_font_size: &mut state.font_size,
                    word_wrap: &mut state.word_wrap,
                    show_line_numbers: true,
                    syntax_highlighting: true,
                    inspect_asset_mode: &mut state.inspect_asset_mode,
                    clipboard_fallback: &mut state.clipboard_fallback,
                    project: &mut state.project,
                    pending_cursor_restore: None,
                    pending_scroll_to_char: None,
                    last_editor_selection: None,
                    pending_editor_context_action: None,
                    peers_in_active_file: &[],
                    language_diagnostics: std::slice::from_ref(&diag),
                },
            );
            state.text_changed |= out.text_changed;
            if out.select_tab.is_some() {
                state.selected_tab = out.select_tab;
            }
            if out.close_tab.is_some() {
                state.closed_tab = out.close_tab;
            }
        },
        state,
    );

    harness.get_by_label("A+").click();
    harness.run();
    assert_eq!(harness.state().font_size, 15.0);

    harness.get_by_label("A-").click();
    harness.run();
    assert_eq!(harness.state().font_size, 14.0);

    harness.get_by_label(&wrap_label).click();
    harness.run();
    assert!(!harness.state().word_wrap);

    harness.get_by_label(&heading_label).click();
    harness.run();
    assert!(harness.state().text_changed);
}

#[test]
fn editor_pane_renders_svg_and_asset_inspector_mode() {
    struct InspectorState {
        project: DiskProject,
        font_size: f32,
        word_wrap: bool,
        inspect_asset_mode: bool,
        clipboard_fallback: String,
    }

    let mut project = DiskProject::blank();
    let svg_path = PathBuf::from("diagram.svg");
    project.buffers.insert(
        svg_path.clone(),
        OpenBuffer {
            rel_path: svg_path.clone(),
            abs_path: PathBuf::from("/tmp/diagram.svg"),
            kind: FileKind::Svg,
            text: Some("<svg></svg>".to_string()),
            size_bytes: 11,
            dirty: false,
        },
    );
    project.open_tabs.push(svg_path.clone());
    project.active_file = svg_path;

    let state = InspectorState {
        project,
        font_size: 14.0,
        word_wrap: true,
        inspect_asset_mode: false,
        clipboard_fallback: String::new(),
    };

    let inspect_btn = t!("editor.asset_metadata", locale = "en").to_string();
    let back_to_editor_btn = t!("editor.switch_to_editor", locale = "en").to_string();

    let mut harness = Harness::new_ui_state(
        |ui, state: &mut InspectorState| {
            let ctx = ui.ctx().clone();
            render_editor_pane(
                &ctx,
                ui,
                EditorPaneParams {
                    dark_mode: false,
                    editor_font_size: &mut state.font_size,
                    word_wrap: &mut state.word_wrap,
                    show_line_numbers: false,
                    syntax_highlighting: false,
                    inspect_asset_mode: &mut state.inspect_asset_mode,
                    clipboard_fallback: &mut state.clipboard_fallback,
                    project: &mut state.project,
                    pending_cursor_restore: None,
                    pending_scroll_to_char: None,
                    last_editor_selection: None,
                    pending_editor_context_action: None,
                    peers_in_active_file: &[],
                    language_diagnostics: &[],
                },
            );
        },
        state,
    );

    harness.get_by_label(&inspect_btn).click();
    harness.run();
    assert!(harness.state().inspect_asset_mode);

    harness.get_by_label(&back_to_editor_btn).click();
    harness.run();
    assert!(!harness.state().inspect_asset_mode);
}

#[test]
fn editor_pane_handles_empty_buffer_without_panicking() {
    struct EmptyState {
        project: DiskProject,
        font_size: f32,
        word_wrap: bool,
        inspect_asset_mode: bool,
        clipboard_fallback: String,
    }

    let mut project = DiskProject::blank();
    project.buffers.clear();
    project.open_tabs.clear();

    let mut harness = Harness::new_ui_state(
        |ui, state: &mut EmptyState| {
            let ctx = ui.ctx().clone();
            render_editor_pane(
                &ctx,
                ui,
                EditorPaneParams {
                    dark_mode: false,
                    editor_font_size: &mut state.font_size,
                    word_wrap: &mut state.word_wrap,
                    show_line_numbers: true,
                    syntax_highlighting: true,
                    inspect_asset_mode: &mut state.inspect_asset_mode,
                    clipboard_fallback: &mut state.clipboard_fallback,
                    project: &mut state.project,
                    pending_cursor_restore: None,
                    pending_scroll_to_char: None,
                    last_editor_selection: None,
                    pending_editor_context_action: None,
                    peers_in_active_file: &[],
                    language_diagnostics: &[],
                },
            );
        },
        EmptyState {
            project,
            font_size: 14.0,
            word_wrap: true,
            inspect_asset_mode: false,
            clipboard_fallback: String::new(),
        },
    );

    harness.run();
}
