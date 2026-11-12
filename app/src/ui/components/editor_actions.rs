//! Context menu and edit action handlers (Undo, Redo, Cut, Copy, Paste, Format Snippets) for the text editor.

use std::path::Path;

use egui::text::{CCursor, CCursorRange};
use egui::text_edit::TextEditOutput;
use egui::{Context, RichText, Ui};

use crate::core::file_kind::FileKind;
use crate::platform::menu::MenuAction;
use crate::ui::components::editor::EditorPaneOutput;
use crate::ui::components::menu_bar::EditorContextAction;

/// Renders the right-click context menu within the code editor.
pub fn render_editor_context_menu(
    ui: &mut Ui,
    active_path: &Path,
    file_kind: FileKind,
    can_undo: bool,
    can_redo: bool,
    has_selection: bool,
    context_action: &mut Option<EditorContextAction>,
) {
    ui.set_min_width(220.0);
    ui.label(
        RichText::new(format!("Editor — {}", active_path.display()))
            .small()
            .strong()
            .weak(),
    );
    ui.separator();

    if ui
        .add_enabled(can_undo, egui::Button::new("Undo  (Ctrl+Z)"))
        .clicked()
    {
        *context_action = Some(EditorContextAction::Undo);
        ui.close_menu();
    }
    if ui
        .add_enabled(can_redo, egui::Button::new("Redo  (Ctrl+Y)"))
        .clicked()
    {
        *context_action = Some(EditorContextAction::Redo);
        ui.close_menu();
    }

    ui.separator();

    if ui
        .add_enabled(has_selection, egui::Button::new("Cut  (Ctrl+X)"))
        .clicked()
    {
        *context_action = Some(EditorContextAction::Cut);
        ui.close_menu();
    }
    if ui
        .add_enabled(has_selection, egui::Button::new("Copy  (Ctrl+C)"))
        .clicked()
    {
        *context_action = Some(EditorContextAction::Copy);
        ui.close_menu();
    }
    if ui.button("Paste  (Ctrl+V)").clicked() {
        *context_action = Some(EditorContextAction::Paste);
        ui.close_menu();
    }
    if ui.button("Select All  (Ctrl+A)").clicked() {
        *context_action = Some(EditorContextAction::SelectAll);
        ui.close_menu();
    }

    ui.separator();

    if ui.button("Bold  (*...*)").clicked() {
        *context_action = Some(EditorContextAction::WrapOrInsert("*bold*", "*", "*"));
        ui.close_menu();
    }
    if ui.button("Italic  (_..._)").clicked() {
        *context_action = Some(EditorContextAction::WrapOrInsert("_italic_", "_", "_"));
        ui.close_menu();
    }
    if ui.button("Inline Code  (`...`)").clicked() {
        *context_action = Some(EditorContextAction::WrapOrInsert("`code`", "`", "`"));
        ui.close_menu();
    }
    if ui.button("Inline Math  ($...$)").clicked() {
        *context_action = Some(EditorContextAction::WrapOrInsert("$x$", "$", "$"));
        ui.close_menu();
    }

    ui.menu_button("Insert Typst Element", |ui| {
        ui.set_min_width(210.0);
        if ui.button("Section Heading (== ...)").clicked() {
            *context_action = Some(EditorContextAction::InsertSnippet("\n== New Section\n"));
            ui.close_menu();
        }
        if ui.button("Display Math Block ($ ... $)").clicked() {
            *context_action = Some(EditorContextAction::InsertSnippet(
                "\n$ f(x) = integral_0^x e^(-t^2) dif t $\n",
            ));
            ui.close_menu();
        }
        if ui.button("Formatted Table (#table)").clicked() {
            *context_action = Some(EditorContextAction::InsertSnippet(
                "\n#table(\n  columns: (1fr, 1fr),\n  [*Column A*], [*Column B*],\n  [Item 1], [Value 1],\n)\n",
            ));
            ui.close_menu();
        }
        if ui.button("Figure / Image (#figure)").clicked() {
            *context_action = Some(EditorContextAction::InsertSnippet(
                "\n#figure(\n  image(\"assets/diagram.svg\", width: 80%),\n  caption: [Figure caption],\n)\n",
            ));
            ui.close_menu();
        }
        if ui.button("Code Block (```rust ... ```)").clicked() {
            *context_action = Some(EditorContextAction::InsertSnippet(
                "\n```rust\nfn main() {\n    println!(\"Hello, Typst!\");\n}\n```\n",
            ));
            ui.close_menu();
        }
    });

    ui.separator();

    if ui.button("Save File  (Ctrl+S)").clicked() {
        *context_action = Some(EditorContextAction::Menu(MenuAction::SaveFile));
        ui.close_menu();
    }
    if ui.button("Compile Document Now").clicked() {
        *context_action = Some(EditorContextAction::Menu(MenuAction::CompileNow));
        ui.close_menu();
    }
    if ui.button("Export PDF...  (Ctrl+E)").clicked() {
        *context_action = Some(EditorContextAction::Menu(MenuAction::ExportPdf));
        ui.close_menu();
    }
    if file_kind == FileKind::Typst && ui.button("Set as Main Entrypoint").clicked() {
        *context_action = Some(EditorContextAction::SetActiveAsMain);
        ui.close_menu();
    }
}

/// Applies an editor context or shortcut action to the text buffer and editor state.
pub fn apply_editor_context_action(
    ctx: &Context,
    act: &EditorContextAction,
    text_ref: &mut String,
    edit_output: &mut TextEditOutput,
    current_range: Option<CCursorRange>,
    clipboard_fallback: &mut String,
    output: &mut EditorPaneOutput,
) {
    let total_chars = text_ref.chars().count();
    let (start_char, end_char, cursor_char) = if let Some(r) = current_range {
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
            undoer.feed_state(ctx.input(|i| i.time), &(ccursor_range, text_ref.clone()));
            if let Some((undo_range, undo_txt)) = undoer.undo(&(ccursor_range, text_ref.clone())) {
                *text_ref = undo_txt.clone();
                edit_output.state.cursor.set_char_range(Some(*undo_range));
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
            if let Some((redo_range, redo_txt)) = undoer.redo(&(ccursor_range, text_ref.clone())) {
                *text_ref = redo_txt.clone();
                edit_output.state.cursor.set_char_range(Some(*redo_range));
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
                let selected_str = text_ref[start_b..end_b].to_string();
                ctx.copy_text(selected_str.clone());
                if let Ok(mut cb) = arboard::Clipboard::new() {
                    let _ = cb.set_text(selected_str.clone());
                }
                *clipboard_fallback = selected_str;
            }
            edit_output.response.request_focus();
        }
        EditorContextAction::Cut => {
            if start_char < end_char {
                let mut undoer = edit_output.state.undoer();
                undoer.feed_state(ctx.input(|i| i.time), &(ccursor_range, text_ref.clone()));
                let start_b = char_to_byte(text_ref, start_char);
                let end_b = char_to_byte(text_ref, end_char);
                let selected_str = text_ref[start_b..end_b].to_string();
                ctx.copy_text(selected_str.clone());
                if let Ok(mut cb) = arboard::Clipboard::new() {
                    let _ = cb.set_text(selected_str.clone());
                }
                *clipboard_fallback = selected_str;

                text_ref.replace_range(start_b..end_b, "");
                let new_range = CCursorRange::one(CCursor::new(start_char));
                edit_output.state.cursor.set_char_range(Some(new_range));
                undoer.feed_state(ctx.input(|i| i.time), &(new_range, text_ref.clone()));
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
                .unwrap_or_else(|| clipboard_fallback.clone());
            if !clip_text.is_empty() {
                let mut undoer = edit_output.state.undoer();
                undoer.feed_state(ctx.input(|i| i.time), &(ccursor_range, text_ref.clone()));
                let start_b = char_to_byte(text_ref, start_char);
                let end_b = char_to_byte(text_ref, end_char);
                text_ref.replace_range(start_b..end_b, &clip_text);
                let pasted_chars = clip_text.chars().count();
                let new_pos = start_char + pasted_chars;
                let new_range = CCursorRange::one(CCursor::new(new_pos));
                edit_output.state.cursor.set_char_range(Some(new_range));
                undoer.feed_state(ctx.input(|i| i.time), &(new_range, text_ref.clone()));
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
            let all_range = CCursorRange::two(CCursor::new(0), CCursor::new(full_len));
            edit_output.state.cursor.set_char_range(Some(all_range));
            edit_output
                .state
                .clone()
                .store(ctx, edit_output.response.id);
            edit_output.response.request_focus();
        }
        EditorContextAction::WrapOrInsert(default_snippet, prefix, suffix) => {
            let mut undoer = edit_output.state.undoer();
            undoer.feed_state(ctx.input(|i| i.time), &(ccursor_range, text_ref.clone()));
            if start_char < end_char {
                let start_b = char_to_byte(text_ref, start_char);
                let end_b = char_to_byte(text_ref, end_char);
                let inner = text_ref[start_b..end_b].to_string();
                let wrapped = format!("{}{}{}", prefix, inner, suffix);
                let wrapped_chars = wrapped.chars().count();
                text_ref.replace_range(start_b..end_b, &wrapped);
                let new_range = CCursorRange::two(
                    CCursor::new(start_char),
                    CCursor::new(start_char + wrapped_chars),
                );
                edit_output.state.cursor.set_char_range(Some(new_range));
            } else {
                let ins_b = char_to_byte(text_ref, cursor_char);
                text_ref.insert_str(ins_b, default_snippet);
                let ins_chars = default_snippet.chars().count();
                let new_range = CCursorRange::one(CCursor::new(cursor_char + ins_chars));
                edit_output.state.cursor.set_char_range(Some(new_range));
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
            undoer.feed_state(ctx.input(|i| i.time), &(ccursor_range, text_ref.clone()));
            let start_b = char_to_byte(text_ref, start_char);
            let end_b = char_to_byte(text_ref, end_char);
            text_ref.replace_range(start_b..end_b, snippet);
            let ins_chars = snippet.chars().count();
            let new_range = CCursorRange::one(CCursor::new(start_char + ins_chars));
            edit_output.state.cursor.set_char_range(Some(new_range));
            edit_output.state.set_undoer(undoer);
            edit_output
                .state
                .clone()
                .store(ctx, edit_output.response.id);
            output.text_changed = true;
            output.edited_text_snapshot = Some(text_ref.clone());
            edit_output.response.request_focus();
        }
        EditorContextAction::Menu(_) | EditorContextAction::SetActiveAsMain => {}
    }
}
