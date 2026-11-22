//! Multiline text editor pane featuring syntax highlighting, gutter line numbers, wrap glyphs, and remote peer cursors.

use std::path::PathBuf;

use egui::text::{CCursor, CCursorRange, LayoutJob};
use egui::{
    Align, Context, CornerRadius, FontFamily, FontId, Frame, Layout, Margin, RichText, ScrollArea,
    Stroke, TextEdit, Ui, Vec2,
};
use rust_i18n::t;

use crate::collab::awareness::PeerPresence;
use crate::collab::ui::paint_remote_cursors_in_editor;
use crate::core::diff::CharSelection;
use crate::core::file_kind::FileKind;
use crate::core::spellcheck::LanguageCheckDiagnostic;
use crate::engine::DiagnosticItem;
use crate::highlight::highlight_typst_code;
use crate::project::DiskProject;
use crate::ui::components::editor_actions::{
    apply_editor_context_action, render_editor_context_menu,
};
use crate::ui::components::editor_diagnostics::paint_language_diagnostics;
pub use crate::ui::components::editor_gutter::compute_gutter_width;
use crate::ui::components::editor_gutter::{
    paint_editor_gutter_and_active_line, paint_unfocused_search_highlight,
};
use crate::ui::components::editor_intellisense::{
    handle_intellisense_keys_before_text_edit, update_and_render_intellisense,
    EditorIntellisenseState, IntellisenseUpdateParams,
};
use crate::ui::components::menu_bar::EditorContextAction;
use crate::ui::theme::ThemePalette;

/// Output results and events triggered during the editor pane rendering frame.
#[derive(Debug, Default)]
pub struct EditorPaneOutput {
    pub text_changed: bool,
    pub edited_text_snapshot: Option<String>,
    pub observed_cursor: Option<CharSelection>,
    pub user_interacted: bool,
    pub context_action: Option<EditorContextAction>,
    pub select_tab: Option<PathBuf>,
    pub close_tab: Option<PathBuf>,
    pub auto_save_requested: bool,
}

/// Parameters for rendering the editor pane.
pub struct EditorPaneParams<'a> {
    pub dark_mode: bool,
    pub editor_font_size: &'a mut f32,
    pub word_wrap: &'a mut bool,
    pub show_line_numbers: bool,
    pub syntax_highlighting: bool,
    pub inspect_asset_mode: &'a mut bool,
    pub clipboard_fallback: &'a mut String,
    pub project: &'a mut DiskProject,
    pub pending_cursor_restore: Option<CharSelection>,
    pub pending_scroll_to_char: Option<usize>,
    pub pending_focus_editor: bool,
    pub search_highlight_range: Option<(usize, usize)>,
    pub last_editor_selection: Option<CharSelection>,
    pub pending_editor_context_action: Option<EditorContextAction>,
    pub peers_in_active_file: &'a [PeerPresence],
    pub language_diagnostics: &'a [LanguageCheckDiagnostic],
    pub compiler_diagnostics: &'a [DiagnosticItem],
    pub intellisense: &'a mut EditorIntellisenseState,
}

/// Quick Typst snippet definitions: `(label_key, tooltip_key, snippet_text)`.
const TYPST_TOOLBAR_SNIPPETS: &[(&str, &str, &str)] = &[
    ("editor.heading_btn", "editor.heading_tooltip", "\n\n== New Section\n"),
    ("editor.bold_btn", "editor.bold_tooltip", " *bold text* "),
    ("editor.italic_btn", "editor.italic_tooltip", " _emphasized_ "),
    (
        "editor.math_btn",
        "editor.math_tooltip",
        "\n$ f(x) = integral_0^x e^(-t^2) dif t $\n",
    ),
    (
        "editor.table_btn",
        "editor.table_tooltip",
        "\n#table(\n  columns: (1fr, 1fr),\n  [*Metric*], [*Value*],\n  [Throughput], [60 FPS],\n)\n",
    ),
    (
        "editor.figure_btn",
        "editor.figure_tooltip",
        "\n#figure(\n  image(\"assets/diagram.svg\", width: 85%),\n  caption: [Architecture diagram.],\n)\n",
    ),
];

/// Adjusts the editor font size by `delta` and clamps to `[10.0, 22.0]`.
pub fn adjust_editor_font_size(current: f32, delta: f32) -> f32 {
    (current + delta).clamp(10.0, 22.0)
}

/// Renders the code editor or asset inspection pane.
pub fn render_editor_pane(
    ctx: &Context,
    ui: &mut Ui,
    params: EditorPaneParams<'_>,
) -> EditorPaneOutput {
    let mut output = EditorPaneOutput::default();
    let dark_mode = params.dark_mode;
    let palette = ThemePalette::for_mode(dark_mode);
    let border_col = palette.panel_border;
    let active_path = params.project.active_file.clone();
    let active_kind = FileKind::from_path(&active_path);

    // 1. Docked Chrome Header for Open File Tabs & Quick Formatting Toolbar
    Frame::new()
        .fill(palette.chrome_sidebar)
        .stroke(Stroke::new(1.0_f32, border_col))
        .corner_radius(CornerRadius::ZERO)
        .inner_margin(Margin::symmetric(10, 6))
        .show(ui, |ui| {
            // Open File Tabs Bar
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let tabs = params.project.open_tabs.clone();

                for tab_path in &tabs {
                    let is_active = *tab_path == active_path;
                    let short_name = tab_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("file");
                    let icon = FileKind::from_path(tab_path).icon();
                    let dirty = params
                        .project
                        .buffers
                        .get(tab_path)
                        .is_some_and(|b| b.dirty);
                    let tab_label =
                        format!("{} {}{}", icon, short_name, if dirty { " *" } else { "" });
                    let tab_color = if is_active {
                        palette.text_primary
                    } else {
                        palette.text_muted
                    };
                    let tab_text = RichText::new(tab_label).size(12.0).color(tab_color);

                    if ui
                        .selectable_label(is_active, tab_text)
                        .on_hover_text(params.project.root_dir.join(tab_path).display().to_string())
                        .clicked()
                    {
                        output.select_tab = Some(tab_path.clone());
                    }
                    if tabs.len() > 1
                        && ui
                            .small_button(RichText::new("×").color(palette.text_muted))
                            .on_hover_text(t!("editor.close_tab_tooltip", name = short_name))
                            .clicked()
                    {
                        output.close_tab = Some(tab_path.clone());
                    }
                    ui.separator();
                }
            });

            ui.separator();

            // Quick Formatting & Snippet Toolbar
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 5.0;
                let mut snippet_to_insert: Option<&'static str> = None;

                if active_kind == FileKind::Typst {
                    for &(label_key, tooltip_key, snippet) in TYPST_TOOLBAR_SNIPPETS {
                        if ui
                            .small_button(t!(label_key))
                            .on_hover_text(t!(tooltip_key))
                            .clicked()
                        {
                            snippet_to_insert = Some(snippet);
                        }
                    }
                } else if active_kind == FileKind::Svg {
                    let mode_text = if *params.inspect_asset_mode {
                        t!("editor.edit_svg")
                    } else {
                        t!("editor.asset_metadata")
                    };
                    if ui.small_button(mode_text).clicked() {
                        *params.inspect_asset_mode = !*params.inspect_asset_mode;
                    }
                }

                if let Some(snippet) = snippet_to_insert {
                    if let Some(file) = params.project.active_buffer_mut() {
                        if let Some(ref mut text) = file.text {
                            text.push_str(snippet);
                            output.edited_text_snapshot = Some(text.clone());
                            output.text_changed = true;
                        }
                    }
                }

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    for (label, tip_key, delta) in [
                        ("A+", "editor.increase_font", 1.0_f32),
                        ("A-", "editor.decrease_font", -1.0_f32),
                    ] {
                        if ui.small_button(label).on_hover_text(t!(tip_key)).clicked() {
                            *params.editor_font_size =
                                adjust_editor_font_size(*params.editor_font_size, delta);
                        }
                    }
                    if ui
                        .selectable_label(*params.word_wrap, t!("editor.wrap_toggle"))
                        .on_hover_text(t!("editor.wrap_tooltip"))
                        .clicked()
                    {
                        *params.word_wrap = !*params.word_wrap;
                    }
                });
            });
        });

    // 2. Editor Area or Asset Inspector
    let font_size = *params.editor_font_size;
    let word_wrap = *params.word_wrap;
    let show_line_numbers = params.show_line_numbers;
    let use_syntax_highlight = params.syntax_highlighting;
    let inspect_asset_mode = *params.inspect_asset_mode;
    let restore_cursor = params.pending_cursor_restore;
    let scroll_to_char = params.pending_scroll_to_char;
    params.intellisense.refresh_project_cache(params.project);

    let Some(file) = params.project.active_buffer_mut() else {
        ui.centered_and_justified(|ui| {
            ui.label(t!("editor.no_file_selected"));
        });
        return output;
    };

    if file.text.is_none() || inspect_asset_mode {
        let mut exit_inspect = false;
        Frame::new()
            .fill(palette.editor_canvas)
            .inner_margin(Margin::same(16))
            .corner_radius(CornerRadius::ZERO)
            .stroke(Stroke::new(1.0_f32, border_col))
            .show(ui, |ui| {
                ui.heading(format!("{} {}", file.kind.icon(), file.rel_path.display()));
                ui.add_space(6.0);
                ui.label(t!(
                    "editor.disk_path",
                    path = file.abs_path.display().to_string()
                ));
                ui.label(t!("editor.asset_type", r#type = file.kind.label()));
                ui.label(t!("editor.file_size", bytes = file.size_bytes));
                ui.add_space(8.0);
                ui.label(t!("editor.embed_hint"));
                let rel_norm = file.rel_path.to_string_lossy().replace('\\', "/");
                ui.code(format!("#image(\"{rel_norm}\", width: 80%)"));
                if file.text.is_some() {
                    ui.add_space(8.0);
                    if ui.button(t!("editor.switch_to_editor")).clicked() {
                        exit_inspect = true;
                    }
                }
            });
        if exit_inspect {
            *params.inspect_asset_mode = false;
        }
        return output;
    }

    let is_typst = file.kind == FileKind::Typst && use_syntax_highlight;
    let file_kind = file.kind;
    let mut context_action: Option<EditorContextAction> = params.pending_editor_context_action;
    let prev_selection = params.last_editor_selection;

    let editor_bg = palette.editor_canvas;

    Frame::new()
        .fill(editor_bg)
        .stroke(Stroke::new(1.0_f32, border_col))
        .corner_radius(CornerRadius::ZERO)
        .show(ui, |ui| {
            ScrollArea::new([!word_wrap, true])
                .id_salt("code_editor_scroll_area")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let text_ref = file.text.as_mut().unwrap();
                    let gutter_w = compute_gutter_width(
                        text_ref.lines().count(),
                        font_size,
                        show_line_numbers,
                    );

                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;

                        let gutter_x_min = ui.cursor().min.x;
                        if show_line_numbers {
                            ui.add_space(gutter_w);
                        }
                        let gutter_x_max = gutter_x_min + gutter_w;

                        Frame::new()
                            .fill(editor_bg)
                            .inner_margin(Margin::symmetric(10, 8))
                            .show(ui, |ui| {
                                let avail_editor_size = ui.available_size();
                                let target_wrap_width = if word_wrap {
                                    (avail_editor_size.x - 22.0).max(80.0)
                                } else {
                                    f32::INFINITY
                                };

                                let mut layouter = |ui: &Ui, string: &str, _wrap_width: f32| {
                                    let mut job = if is_typst {
                                        highlight_typst_code(string, dark_mode, font_size)
                                    } else {
                                        LayoutJob::simple(
                                            string.to_owned(),
                                            FontId::new(font_size, FontFamily::Monospace),
                                            palette.syn_default,
                                            target_wrap_width,
                                        )
                                    };
                                    job.wrap.max_width = target_wrap_width;
                                    ui.fonts(|f| f.layout_job(job))
                                };

                                let active_line_shape_idx = ui.painter().add(egui::Shape::Noop);
                                let search_highlight_shape_idx =
                                    ui.painter().add(egui::Shape::Noop);
                                let editor_id = ui.make_persistent_id(egui::Id::new((
                                    "typst_code_editor",
                                    &active_path,
                                )));
                                let esc_pressed = ctx.input(|i| i.key_pressed(egui::Key::Escape));
                                let editor_has_focus = ctx.memory(|m| m.has_focus(editor_id))
                                    || (params.intellisense.editor_had_focus && esc_pressed);
                                let (isense_consumed, accepted_completion) =
                                    if file_kind == FileKind::Typst {
                                        handle_intellisense_keys_before_text_edit(
                                            ctx,
                                            editor_has_focus,
                                            params.intellisense,
                                        )
                                    } else {
                                        (false, None)
                                    };
                                if isense_consumed && !ctx.memory(|m| m.has_focus(editor_id)) {
                                    ctx.memory_mut(|m| m.request_focus(editor_id));
                                }
                                if editor_has_focus && !isense_consumed && context_action.is_none()
                                {
                                    context_action = crate::platform::poll_editor_shortcuts(ctx)
                                        .map(EditorContextAction::Menu);
                                }

                                let mut edit_output = TextEdit::multiline(text_ref)
                                    .id(editor_id)
                                    .code_editor()
                                    .desired_width(if word_wrap {
                                        avail_editor_size.x
                                    } else {
                                        f32::INFINITY
                                    })
                                    .min_size(avail_editor_size)
                                    .desired_rows(34)
                                    .frame(false)
                                    .font(FontId::new(font_size, FontFamily::Monospace))
                                    .layouter(&mut layouter)
                                    .show(ui);

                                let restored_sel = restore_cursor.or_else(|| {
                                    (ctx.input(|i| i.pointer.secondary_pressed())
                                        && edit_output.response.hovered())
                                    .then_some(prev_selection)
                                    .flatten()
                                    .filter(|p| p.primary_char != p.secondary_char)
                                });
                                if let Some(sel) = restored_sel {
                                    edit_output.state.cursor.set_char_range(Some(
                                        CCursorRange::two(
                                            CCursor::new(sel.secondary_char),
                                            CCursor::new(sel.primary_char),
                                        ),
                                    ));
                                    edit_output
                                        .state
                                        .clone()
                                        .store(ctx, edit_output.response.id);
                                }

                                if let Some(target_char) = scroll_to_char {
                                    if params.pending_focus_editor {
                                        edit_output.response.request_focus();
                                    }
                                    let target_cursor =
                                        edit_output.galley.from_ccursor(CCursor::new(target_char));
                                    let local_rect =
                                        edit_output.galley.pos_from_cursor(&target_cursor);
                                    let screen_rect =
                                        local_rect.translate(edit_output.galley_pos.to_vec2());
                                    ui.scroll_to_rect(
                                        screen_rect.expand2(Vec2::new(0.0, 56.0)),
                                        Some(Align::Center),
                                    );
                                    ctx.request_repaint();
                                }

                                let current_range =
                                    edit_output.state.cursor.char_range().or_else(|| {
                                        edit_output.cursor_range.map(|cr| cr.as_ccursor_range())
                                    });
                                let active_ccursor = current_range
                                    .map(|r| r.primary)
                                    .unwrap_or_else(|| CCursor::new(0));
                                paint_editor_gutter_and_active_line(
                                    ui,
                                    &edit_output,
                                    active_line_shape_idx,
                                    active_ccursor,
                                    gutter_x_max,
                                    font_size,
                                    show_line_numbers,
                                    word_wrap,
                                    &palette,
                                );
                                if !edit_output.response.has_focus() {
                                    if let Some(hl_range) = params.search_highlight_range {
                                        paint_unfocused_search_highlight(
                                            ui,
                                            &edit_output,
                                            search_highlight_shape_idx,
                                            hl_range,
                                            &palette,
                                        );
                                    }
                                }
                                let has_selection = current_range
                                    .is_some_and(|r| r.primary.index != r.secondary.index);
                                let cur_undo_state =
                                    (current_range.unwrap_or_default(), text_ref.clone());
                                let can_undo = edit_output.state.undoer().has_undo(&cur_undo_state);
                                let can_redo = edit_output.state.undoer().has_redo(&cur_undo_state);

                                edit_output.response.context_menu(|ui| {
                                    render_editor_context_menu(
                                        ui,
                                        &active_path,
                                        file_kind,
                                        can_undo,
                                        can_redo,
                                        has_selection,
                                        &mut context_action,
                                    );
                                });

                                if let Some(ref act) = context_action {
                                    apply_editor_context_action(
                                        ctx,
                                        act,
                                        text_ref,
                                        &mut edit_output,
                                        current_range,
                                        params.clipboard_fallback,
                                        &mut output,
                                    );
                                }

                                let ime_in_progress = ctx.input(|i| {
                                    i.events.iter().any(|ev| {
                                        matches!(ev, egui::Event::Ime(egui::ImeEvent::Preedit(_)))
                                    })
                                });

                                if !params.peers_in_active_file.is_empty() {
                                    let peer_refs: Vec<&PeerPresence> =
                                        params.peers_in_active_file.iter().collect();
                                    paint_remote_cursors_in_editor(ui, &edit_output, &peer_refs);
                                }
                                if file_kind == FileKind::Typst {
                                    if !params.language_diagnostics.is_empty() {
                                        paint_language_diagnostics(
                                            ui,
                                            &edit_output,
                                            params.language_diagnostics,
                                            params.dark_mode,
                                            text_ref.chars().count(),
                                        );
                                    }
                                    update_and_render_intellisense(
                                        ctx,
                                        ui,
                                        IntellisenseUpdateParams {
                                            active_file: &active_path,
                                            compiler_diagnostics: params.compiler_diagnostics,
                                            dark_mode: params.dark_mode,
                                            ime_in_progress,
                                            accepted_completion,
                                        },
                                        text_ref,
                                        &mut edit_output,
                                        params.intellisense,
                                        &mut output,
                                    );
                                }

                                if let Some(range) =
                                    edit_output.state.cursor.char_range().or_else(|| {
                                        edit_output.cursor_range.map(|cr| cr.as_ccursor_range())
                                    })
                                {
                                    output.observed_cursor = Some(CharSelection {
                                        primary_char: range.primary.index,
                                        secondary_char: range.secondary.index,
                                    });
                                }

                                if edit_output.response.changed() && !ime_in_progress {
                                    output.text_changed = true;
                                    output.edited_text_snapshot = Some(text_ref.clone());
                                }

                                let cursor_moved_by_user = output.observed_cursor.is_some()
                                    && output.observed_cursor != prev_selection;
                                output.user_interacted = restore_cursor.is_none()
                                    && scroll_to_char.is_none()
                                    && (edit_output.response.clicked()
                                        || edit_output.response.dragged()
                                        || output.text_changed
                                        || cursor_moved_by_user);
                            });
                    });
                });
        });

    output.context_action = context_action;
    output
}

#[cfg(test)]
#[path = "../../tests/ui/components/editor_tests.rs"]
mod tests;
