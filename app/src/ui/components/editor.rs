//! Multiline text editor pane featuring syntax highlighting, gutter line numbers,
//! wrap glyphs, snippet insertion, collaborative cursor rendering, and context menu.

use std::path::PathBuf;

use egui::text::{CCursor, CCursorRange, LayoutJob};
use egui::{
    Align, Align2, Context, CornerRadius, FontFamily, FontId, Frame, Layout, Margin, Pos2, Rect,
    RichText, ScrollArea, Stroke, TextEdit, Ui, Vec2,
};

use crate::collab::awareness::PeerPresence;
use crate::collab::ui::paint_remote_cursors_in_editor;
use crate::core::diff::CharSelection;
use crate::core::file_kind::FileKind;
use crate::highlight::highlight_typst_code;
use crate::platform::menu::MenuAction;
use crate::project::DiskProject;
use crate::ui::components::menu_bar::EditorContextAction;
use crate::ui::theme::ThemePalette;

/// Output results and events triggered during the editor pane rendering frame.
#[derive(Debug, Default)]
pub struct EditorPaneOutput {
    pub text_changed: bool,
    pub edited_text_snapshot: Option<String>,
    pub observed_cursor: Option<CharSelection>,
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
    pub last_editor_selection: Option<CharSelection>,
    pub pending_editor_context_action: Option<EditorContextAction>,
    pub peers_in_active_file: &'a [PeerPresence],
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
                        .map(|b| b.dirty)
                        .unwrap_or(false);
                    let dirty_suffix = if dirty { " *" } else { "" };
                    let tab_label = format!("{} {}{}", icon, short_name, dirty_suffix);
                    let tab_text = RichText::new(tab_label).size(12.0).color(if is_active {
                        palette.text_primary
                    } else {
                        palette.text_muted
                    });

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
                            .on_hover_text(format!("Close tab {}", short_name))
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
                    if ui
                        .small_button("= Heading")
                        .on_hover_text("Append a section heading")
                        .clicked()
                    {
                        snippet_to_insert = Some("\n\n== New Section\n");
                    }
                    if ui
                        .small_button("*Bold*")
                        .on_hover_text("Insert bold markup")
                        .clicked()
                    {
                        snippet_to_insert = Some(" *bold text* ");
                    }
                    if ui
                        .small_button("_Italic_")
                        .on_hover_text("Insert italic markup")
                        .clicked()
                    {
                        snippet_to_insert = Some(" _emphasized_ ");
                    }
                    if ui
                        .small_button("$ Math $")
                        .on_hover_text("Insert a display math equation block")
                        .clicked()
                    {
                        snippet_to_insert = Some("\n$ f(x) = integral_0^x e^(-t^2) dif t $\n");
                    }
                    if ui
                        .small_button("+ Table")
                        .on_hover_text("Insert a formatted table snippet")
                        .clicked()
                    {
                        snippet_to_insert = Some(
                            "\n#table(\n  columns: (1fr, 1fr),\n  [*Metric*], [*Value*],\n  [Throughput], [60 FPS],\n)\n",
                        );
                    }
                    if ui
                        .small_button("+ Figure")
                        .on_hover_text("Insert figure with assets/diagram.svg")
                        .clicked()
                    {
                        snippet_to_insert = Some(
                            "\n#figure(\n  image(\"assets/diagram.svg\", width: 85%),\n  caption: [Architecture diagram.],\n)\n",
                        );
                    }
                } else if active_kind == FileKind::Svg {
                    let mode_text = if *params.inspect_asset_mode {
                        "Edit SVG XML"
                    } else {
                        "Asset Metadata"
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
                    if ui
                        .small_button("A+")
                        .on_hover_text("Increase editor font size")
                        .clicked()
                    {
                        *params.editor_font_size =
                            (*params.editor_font_size + 1.0).clamp(10.0, 22.0);
                    }
                    if ui
                        .small_button("A-")
                        .on_hover_text("Decrease editor font size")
                        .clicked()
                    {
                        *params.editor_font_size =
                            (*params.editor_font_size - 1.0).clamp(10.0, 22.0);
                    }
                    if ui
                        .selectable_label(*params.word_wrap, "↩ Wrap")
                        .on_hover_text(
                            "Toggle automatic line wrapping at editor panel width (also in File > Settings)",
                        )
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

    let Some(file) = params.project.active_buffer_mut() else {
        ui.centered_and_justified(|ui| {
            ui.label("Select a file from the Disk Project Tree.");
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
                ui.label(format!("Disk Path: {}", file.abs_path.display()));
                ui.label(format!("Asset Type: {}", file.kind.label()));
                ui.label(format!("File Size: {} bytes", file.size_bytes));
                ui.add_space(8.0);
                ui.label("Embed in any Typst file using:");
                ui.code(format!(
                    "#image(\"{}\", width: 80%)",
                    file.rel_path.to_string_lossy().replace('\\', "/")
                ));
                if file.text.is_some() {
                    ui.add_space(8.0);
                    if ui.button("Switch to Source Code Editor").clicked() {
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
    let gutter_fg = palette.text_muted;

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
                    let line_count = text_ref.lines().count().max(1);
                    let digits = line_count.to_string().len().max(3);
                    let gutter_w = if show_line_numbers {
                        (digits as f32 * (font_size * 0.62) + 18.0).max(40.0)
                    } else {
                        0.0
                    };

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

                                let mut edit_output = TextEdit::multiline(text_ref)
                                    .id_salt("typst_code_editor")
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

                                if let Some(sel) = restore_cursor {
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
                                } else if ctx.input(|i| i.pointer.secondary_pressed())
                                    && edit_output.response.hovered()
                                {
                                    if let Some(prev) = prev_selection {
                                        if prev.primary_char != prev.secondary_char {
                                            let restored = CCursorRange::two(
                                                CCursor::new(prev.secondary_char),
                                                CCursor::new(prev.primary_char),
                                            );
                                            edit_output
                                                .state
                                                .cursor
                                                .set_char_range(Some(restored));
                                            edit_output
                                                .state
                                                .clone()
                                                .store(ctx, edit_output.response.id);
                                        }
                                    }
                                }

                                if let Some(target_char) = scroll_to_char {
                                    edit_output.response.request_focus();
                                    let target_cursor = edit_output
                                        .galley
                                        .from_ccursor(CCursor::new(target_char));
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

                                let total_rows = edit_output.galley.rows.len();
                                let mut logical_line = 1usize;
                                let mut is_wrapped_continuation = false;
                                let gutter_font = FontId::new(font_size, FontFamily::Monospace);
                                let cont_font =
                                    FontId::new((font_size * 0.85).max(9.5), FontFamily::Monospace);
                                let wrap_badge_font = FontId::new(
                                    (font_size * 0.90).max(10.5),
                                    FontFamily::Monospace,
                                );

                                for (row_idx, row) in edit_output.galley.rows.iter().enumerate() {
                                    let row_top_y = edit_output.galley_pos.y + row.min_y();
                                    let row_center_y =
                                        edit_output.galley_pos.y + row.rect.center().y;

                                    if show_line_numbers {
                                        if !is_wrapped_continuation {
                                            ui.painter().text(
                                                Pos2::new(gutter_x_max - 8.0, row_top_y),
                                                Align2::RIGHT_TOP,
                                                logical_line.to_string(),
                                                gutter_font.clone(),
                                                gutter_fg,
                                            );
                                        } else {
                                            ui.painter().text(
                                                Pos2::new(gutter_x_max - 8.0, row_top_y),
                                                Align2::RIGHT_TOP,
                                                "↪",
                                                cont_font.clone(),
                                                gutter_fg.gamma_multiply(0.65),
                                            );
                                        }
                                    }

                                    let wrapped_to_next = word_wrap
                                        && !row.ends_with_newline
                                        && (row_idx + 1 < total_rows);
                                    if wrapped_to_next {
                                        let eol_x = (edit_output.galley_pos.x
                                            + row.rect.max.x
                                            + 4.0)
                                            .min(edit_output.response.rect.max.x - 12.0);
                                        ui.painter().text(
                                            Pos2::new(eol_x, row_center_y),
                                            Align2::LEFT_CENTER,
                                            "↩",
                                            wrap_badge_font.clone(),
                                            palette.syn_keyword,
                                        );
                                    }

                                    if row.ends_with_newline {
                                        logical_line += 1;
                                        is_wrapped_continuation = false;
                                    } else {
                                        is_wrapped_continuation = true;
                                    }
                                }

                                if show_line_numbers {
                                    ui.painter().line_segment(
                                        [
                                            Pos2::new(
                                                gutter_x_max,
                                                edit_output.response.rect.min.y - 8.0,
                                            ),
                                            Pos2::new(
                                                gutter_x_max,
                                                edit_output.response.rect.max.y + 8.0,
                                            ),
                                        ],
                                        Stroke::new(1.0_f32, border_col),
                                    );
                                }

                                let current_range = edit_output
                                    .state
                                    .cursor
                                    .char_range()
                                    .or_else(|| edit_output.cursor_range.map(|cr| cr.as_ccursor_range()));

                                let active_ccursor = current_range
                                    .map(|r| r.primary)
                                    .unwrap_or_else(|| CCursor::new(0));
                                let active_cursor_obj =
                                    edit_output.galley.from_ccursor(active_ccursor);
                                let cursor_local_rect =
                                    edit_output.galley.pos_from_cursor(&active_cursor_obj);
                                let line_min_y =
                                    edit_output.galley_pos.y + cursor_local_rect.min.y;
                                let line_max_y =
                                    edit_output.galley_pos.y + cursor_local_rect.max.y;
                                let full_line_rect = Rect::from_min_max(
                                    Pos2::new(edit_output.response.rect.min.x - 10.0, line_min_y),
                                    Pos2::new(edit_output.response.rect.max.x + 10.0, line_max_y),
                                );
                                ui.painter().set(
                                    active_line_shape_idx,
                                    egui::Shape::rect_filled(
                                        full_line_rect,
                                        CornerRadius::ZERO,
                                        palette.active_line_highlight,
                                    ),
                                );
                                let has_selection = current_range
                                    .map(|r| r.primary.index != r.secondary.index)
                                    .unwrap_or(false);
                                let cur_undo_state =
                                    (current_range.unwrap_or_default(), text_ref.clone());
                                let can_undo = edit_output.state.undoer().has_undo(&cur_undo_state);
                                let can_redo = edit_output.state.undoer().has_redo(&cur_undo_state);

                                edit_output.response.context_menu(|ui| {
                                    ui.set_min_width(220.0);
                                    ui.label(
                                        RichText::new(format!(
                                            "Editor — {}",
                                            active_path.display()
                                        ))
                                        .small()
                                        .strong()
                                        .weak(),
                                    );
                                    ui.separator();

                                    if ui
                                        .add_enabled(can_undo, egui::Button::new("Undo  (Ctrl+Z)"))
                                        .clicked()
                                    {
                                        context_action = Some(EditorContextAction::Undo);
                                        ui.close_menu();
                                    }
                                    if ui
                                        .add_enabled(can_redo, egui::Button::new("Redo  (Ctrl+Y)"))
                                        .clicked()
                                    {
                                        context_action = Some(EditorContextAction::Redo);
                                        ui.close_menu();
                                    }

                                    ui.separator();

                                    if ui
                                        .add_enabled(
                                            has_selection,
                                            egui::Button::new("Cut  (Ctrl+X)"),
                                        )
                                        .clicked()
                                    {
                                        context_action = Some(EditorContextAction::Cut);
                                        ui.close_menu();
                                    }
                                    if ui
                                        .add_enabled(
                                            has_selection,
                                            egui::Button::new("Copy  (Ctrl+C)"),
                                        )
                                        .clicked()
                                    {
                                        context_action = Some(EditorContextAction::Copy);
                                        ui.close_menu();
                                    }
                                    if ui.button("Paste  (Ctrl+V)").clicked() {
                                        context_action = Some(EditorContextAction::Paste);
                                        ui.close_menu();
                                    }
                                    if ui.button("Select All  (Ctrl+A)").clicked() {
                                        context_action = Some(EditorContextAction::SelectAll);
                                        ui.close_menu();
                                    }

                                    ui.separator();

                                    if ui.button("Bold  (*...*)").clicked() {
                                        context_action =
                                            Some(EditorContextAction::WrapOrInsert("*bold*", "*", "*"));
                                        ui.close_menu();
                                    }
                                    if ui.button("Italic  (_..._)").clicked() {
                                        context_action =
                                            Some(EditorContextAction::WrapOrInsert("_italic_", "_", "_"));
                                        ui.close_menu();
                                    }
                                    if ui.button("Inline Code  (`...`)").clicked() {
                                        context_action =
                                            Some(EditorContextAction::WrapOrInsert("`code`", "`", "`"));
                                        ui.close_menu();
                                    }
                                    if ui.button("Inline Math  ($...$)").clicked() {
                                        context_action =
                                            Some(EditorContextAction::WrapOrInsert("$x$", "$", "$"));
                                        ui.close_menu();
                                    }

                                    ui.menu_button("Insert Typst Element", |ui| {
                                        ui.set_min_width(210.0);
                                        if ui.button("Section Heading (== ...)").clicked() {
                                            context_action =
                                                Some(EditorContextAction::InsertSnippet("\n== New Section\n"));
                                            ui.close_menu();
                                        }
                                        if ui.button("Display Math Block ($ ... $)").clicked() {
                                            context_action = Some(EditorContextAction::InsertSnippet(
                                                "\n$ f(x) = integral_0^x e^(-t^2) dif t $\n",
                                            ));
                                            ui.close_menu();
                                        }
                                        if ui.button("Formatted Table (#table)").clicked() {
                                            context_action = Some(EditorContextAction::InsertSnippet(
                                                "\n#table(\n  columns: (1fr, 1fr),\n  [*Column A*], [*Column B*],\n  [Item 1], [Value 1],\n)\n",
                                            ));
                                            ui.close_menu();
                                        }
                                        if ui.button("Figure / Image (#figure)").clicked() {
                                            context_action = Some(EditorContextAction::InsertSnippet(
                                                "\n#figure(\n  image(\"assets/diagram.svg\", width: 80%),\n  caption: [Figure caption],\n)\n",
                                            ));
                                            ui.close_menu();
                                        }
                                        if ui.button("Code Block (```rust ... ```)").clicked() {
                                            context_action = Some(EditorContextAction::InsertSnippet(
                                                "\n```rust\nfn main() {\n    println!(\"Hello, Typst!\");\n}\n```\n",
                                            ));
                                            ui.close_menu();
                                        }
                                    });

                                    ui.separator();

                                    if ui.button("Save File  (Ctrl+S)").clicked() {
                                        context_action =
                                            Some(EditorContextAction::Menu(MenuAction::SaveFile));
                                        ui.close_menu();
                                    }
                                    if ui.button("Compile Document Now").clicked() {
                                        context_action =
                                            Some(EditorContextAction::Menu(MenuAction::CompileNow));
                                        ui.close_menu();
                                    }
                                    if ui.button("Export PDF...  (Ctrl+E)").clicked() {
                                        context_action =
                                            Some(EditorContextAction::Menu(MenuAction::ExportPdf));
                                        ui.close_menu();
                                    }
                                    if file_kind == FileKind::Typst
                                        && ui.button("Set as Main Entrypoint").clicked()
                                    {
                                        context_action =
                                            Some(EditorContextAction::SetActiveAsMain);
                                        ui.close_menu();
                                    }
                                });

                                if let Some(ref act) = context_action {
                                    let total_chars = text_ref.chars().count();
                                    let (start_char, end_char, cursor_char) =
                                        if let Some(r) = current_range {
                                            let a = r.primary.index.min(total_chars);
                                            let b = r.secondary.index.min(total_chars);
                                            (a.min(b), a.max(b), a)
                                        } else {
                                            (total_chars, total_chars, total_chars)
                                        };

                                    let char_to_byte = |s: &str, char_idx: usize| -> usize {
                                        s.char_indices()
                                            .nth(char_idx)
                                            .map(|(i, _)| i)
                                            .unwrap_or(s.len())
                                    };

                                    let ccursor_range = current_range.unwrap_or_default();

                                    match *act {
                                        EditorContextAction::Undo => {
                                            let mut undoer = edit_output.state.undoer();
                                            undoer.feed_state(
                                                ctx.input(|i| i.time),
                                                &(ccursor_range, text_ref.clone()),
                                            );
                                            if let Some((undo_range, undo_txt)) =
                                                undoer.undo(&(ccursor_range, text_ref.clone()))
                                            {
                                                *text_ref = undo_txt.clone();
                                                edit_output
                                                    .state
                                                    .cursor
                                                    .set_char_range(Some(*undo_range));
                                                edit_output.state.set_undoer(undoer);
                                                edit_output
                                                    .state
                                                    .clone()
                                                    .store(ctx, edit_output.response.id);
                                                output.text_changed = true;
                                                output.edited_text_snapshot = Some(text_ref.clone());
                                            }
                                            edit_output.response.request_focus();
                                        }
                                        EditorContextAction::Redo => {
                                            let mut undoer = edit_output.state.undoer();
                                            if let Some((redo_range, redo_txt)) =
                                                undoer.redo(&(ccursor_range, text_ref.clone()))
                                            {
                                                *text_ref = redo_txt.clone();
                                                edit_output
                                                    .state
                                                    .cursor
                                                    .set_char_range(Some(*redo_range));
                                                edit_output.state.set_undoer(undoer);
                                                edit_output
                                                    .state
                                                    .clone()
                                                    .store(ctx, edit_output.response.id);
                                                output.text_changed = true;
                                                output.edited_text_snapshot = Some(text_ref.clone());
                                            }
                                            edit_output.response.request_focus();
                                        }
                                        EditorContextAction::Copy => {
                                            if start_char < end_char {
                                                let start_b = char_to_byte(text_ref, start_char);
                                                let end_b = char_to_byte(text_ref, end_char);
                                                let selected_str =
                                                    text_ref[start_b..end_b].to_string();
                                                ctx.copy_text(selected_str.clone());
                                                if let Ok(mut cb) = arboard::Clipboard::new() {
                                                    let _ = cb.set_text(selected_str.clone());
                                                }
                                                *params.clipboard_fallback = selected_str;
                                            }
                                            edit_output.response.request_focus();
                                        }
                                        EditorContextAction::Cut => {
                                            if start_char < end_char {
                                                let mut undoer = edit_output.state.undoer();
                                                undoer.feed_state(
                                                    ctx.input(|i| i.time),
                                                    &(ccursor_range, text_ref.clone()),
                                                );
                                                let start_b = char_to_byte(text_ref, start_char);
                                                let end_b = char_to_byte(text_ref, end_char);
                                                let selected_str =
                                                    text_ref[start_b..end_b].to_string();
                                                ctx.copy_text(selected_str.clone());
                                                if let Ok(mut cb) = arboard::Clipboard::new() {
                                                    let _ = cb.set_text(selected_str.clone());
                                                }
                                                *params.clipboard_fallback = selected_str;

                                                text_ref.replace_range(start_b..end_b, "");
                                                let new_range =
                                                    CCursorRange::one(CCursor::new(start_char));
                                                edit_output
                                                    .state
                                                    .cursor
                                                    .set_char_range(Some(new_range));
                                                undoer.feed_state(
                                                    ctx.input(|i| i.time),
                                                    &(new_range, text_ref.clone()),
                                                );
                                                edit_output.state.set_undoer(undoer);
                                                edit_output
                                                    .state
                                                    .clone()
                                                    .store(ctx, edit_output.response.id);
                                                output.text_changed = true;
                                                output.edited_text_snapshot = Some(text_ref.clone());
                                            }
                                            edit_output.response.request_focus();
                                        }
                                        EditorContextAction::Paste => {
                                            let clip_text = arboard::Clipboard::new()
                                                .and_then(|mut cb| cb.get_text())
                                                .ok()
                                                .filter(|s| !s.is_empty())
                                                .unwrap_or_else(|| {
                                                    params.clipboard_fallback.clone()
                                                });
                                            if !clip_text.is_empty() {
                                                let mut undoer = edit_output.state.undoer();
                                                undoer.feed_state(
                                                    ctx.input(|i| i.time),
                                                    &(ccursor_range, text_ref.clone()),
                                                );
                                                let start_b = char_to_byte(text_ref, start_char);
                                                let end_b = char_to_byte(text_ref, end_char);
                                                text_ref.replace_range(start_b..end_b, &clip_text);
                                                let pasted_chars = clip_text.chars().count();
                                                let new_pos = start_char + pasted_chars;
                                                let new_range =
                                                    CCursorRange::one(CCursor::new(new_pos));
                                                edit_output
                                                    .state
                                                    .cursor
                                                    .set_char_range(Some(new_range));
                                                undoer.feed_state(
                                                    ctx.input(|i| i.time),
                                                    &(new_range, text_ref.clone()),
                                                );
                                                edit_output.state.set_undoer(undoer);
                                                edit_output
                                                    .state
                                                    .clone()
                                                    .store(ctx, edit_output.response.id);
                                                output.text_changed = true;
                                                output.edited_text_snapshot = Some(text_ref.clone());
                                            }
                                            edit_output.response.request_focus();
                                        }
                                        EditorContextAction::SelectAll => {
                                            let full_len = text_ref.chars().count();
                                            let all_range = CCursorRange::two(
                                                CCursor::new(0),
                                                CCursor::new(full_len),
                                            );
                                            edit_output
                                                .state
                                                .cursor
                                                .set_char_range(Some(all_range));
                                            edit_output
                                                .state
                                                .clone()
                                                .store(ctx, edit_output.response.id);
                                            edit_output.response.request_focus();
                                        }
                                        EditorContextAction::WrapOrInsert(
                                            default_snippet,
                                            prefix,
                                            suffix,
                                        ) => {
                                            let mut undoer = edit_output.state.undoer();
                                            undoer.feed_state(
                                                ctx.input(|i| i.time),
                                                &(ccursor_range, text_ref.clone()),
                                            );
                                            if start_char < end_char {
                                                let start_b = char_to_byte(text_ref, start_char);
                                                let end_b = char_to_byte(text_ref, end_char);
                                                let inner = text_ref[start_b..end_b].to_string();
                                                let wrapped =
                                                    format!("{}{}{}", prefix, inner, suffix);
                                                let wrapped_chars = wrapped.chars().count();
                                                text_ref.replace_range(start_b..end_b, &wrapped);
                                                let new_range = CCursorRange::two(
                                                    CCursor::new(start_char),
                                                    CCursor::new(start_char + wrapped_chars),
                                                );
                                                edit_output
                                                    .state
                                                    .cursor
                                                    .set_char_range(Some(new_range));
                                            } else {
                                                let ins_b = char_to_byte(text_ref, cursor_char);
                                                text_ref.insert_str(ins_b, default_snippet);
                                                let ins_chars = default_snippet.chars().count();
                                                let new_range = CCursorRange::one(CCursor::new(
                                                    cursor_char + ins_chars,
                                                ));
                                                edit_output
                                                    .state
                                                    .cursor
                                                    .set_char_range(Some(new_range));
                                            }
                                            edit_output.state.set_undoer(undoer);
                                            edit_output
                                                .state
                                                .clone()
                                                .store(ctx, edit_output.response.id);
                                            output.text_changed = true;
                                            output.edited_text_snapshot = Some(text_ref.clone());
                                            edit_output.response.request_focus();
                                        }
                                        EditorContextAction::InsertSnippet(snippet) => {
                                            let mut undoer = edit_output.state.undoer();
                                            undoer.feed_state(
                                                ctx.input(|i| i.time),
                                                &(ccursor_range, text_ref.clone()),
                                            );
                                            let start_b = char_to_byte(text_ref, start_char);
                                            let end_b = char_to_byte(text_ref, end_char);
                                            text_ref.replace_range(start_b..end_b, snippet);
                                            let ins_chars = snippet.chars().count();
                                            let new_range = CCursorRange::one(CCursor::new(
                                                start_char + ins_chars,
                                            ));
                                            edit_output
                                                .state
                                                .cursor
                                                .set_char_range(Some(new_range));
                                            edit_output.state.set_undoer(undoer);
                                            edit_output
                                                .state
                                                .clone()
                                                .store(ctx, edit_output.response.id);
                                            output.text_changed = true;
                                            output.edited_text_snapshot = Some(text_ref.clone());
                                            edit_output.response.request_focus();
                                        }
                                        EditorContextAction::Menu(_)
                                        | EditorContextAction::SetActiveAsMain => {}
                                    }
                                }

                                if let Some(range) = edit_output
                                    .state
                                    .cursor
                                    .char_range()
                                    .or_else(|| edit_output.cursor_range.map(|cr| cr.as_ccursor_range()))
                                {
                                    output.observed_cursor = Some(CharSelection {
                                        primary_char: range.primary.index,
                                        secondary_char: range.secondary.index,
                                    });
                                }

                                if !params.peers_in_active_file.is_empty() {
                                    let peer_refs: Vec<&PeerPresence> =
                                        params.peers_in_active_file.iter().collect();
                                    paint_remote_cursors_in_editor(ui, &edit_output, &peer_refs);
                                }

                                let ime_in_progress = ctx.input(|i| {
                                    i.events.iter().any(|ev| {
                                        matches!(ev, egui::Event::Ime(egui::ImeEvent::Preedit(_)))
                                    })
                                });

                                if edit_output.response.changed() && !ime_in_progress {
                                    output.text_changed = true;
                                    output.edited_text_snapshot = Some(text_ref.clone());
                                }
                            });
                    });
                });
        });

    output.context_action = context_action;
    output
}
