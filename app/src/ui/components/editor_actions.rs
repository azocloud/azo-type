//! Context menu and edit action handlers (Undo, Redo, Cut, Copy, Paste, Format Snippets) for the text editor.

use std::path::Path;

use egui::text::{CCursor, CCursorRange};
use egui::text_edit::TextEditOutput;
use egui::{Context, RichText, Ui};
use rust_i18n::t;

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
        RichText::new(t!(
            "editor.context_title",
            path = active_path.display().to_string()
        ))
        .small()
        .strong()
        .weak(),
    );
    ui.separator();

    if ui
        .add_enabled(can_undo, egui::Button::new(t!("menu.undo")))
        .clicked()
    {
        *context_action = Some(EditorContextAction::Undo);
        ui.close_menu();
    }
    if ui
        .add_enabled(can_redo, egui::Button::new(t!("menu.redo")))
        .clicked()
    {
        *context_action = Some(EditorContextAction::Redo);
        ui.close_menu();
    }

    ui.separator();

    if ui
        .add_enabled(has_selection, egui::Button::new(t!("menu.cut")))
        .clicked()
    {
        *context_action = Some(EditorContextAction::Cut);
        ui.close_menu();
    }
    if ui
        .add_enabled(has_selection, egui::Button::new(t!("menu.copy")))
        .clicked()
    {
        *context_action = Some(EditorContextAction::Copy);
        ui.close_menu();
    }
    if ui.button(t!("menu.paste")).clicked() {
        *context_action = Some(EditorContextAction::Paste);
        ui.close_menu();
    }
    if ui.button(t!("menu.select_all")).clicked() {
        *context_action = Some(EditorContextAction::SelectAll);
        ui.close_menu();
    }

    ui.separator();

    if ui.button(t!("editor.bold_menu")).clicked() {
        *context_action = Some(EditorContextAction::WrapOrInsert("*bold*", "*", "*"));
        ui.close_menu();
    }
    if ui.button(t!("editor.italic_menu")).clicked() {
        *context_action = Some(EditorContextAction::WrapOrInsert("_italic_", "_", "_"));
        ui.close_menu();
    }
    if ui.button(t!("editor.inline_code")).clicked() {
        *context_action = Some(EditorContextAction::WrapOrInsert("`code`", "`", "`"));
        ui.close_menu();
    }
    if ui.button(t!("editor.inline_math")).clicked() {
        *context_action = Some(EditorContextAction::WrapOrInsert("$x$", "$", "$"));
        ui.close_menu();
    }

    ui.menu_button(t!("editor.insert_typst_element"), |ui| {
        ui.set_min_width(210.0);
        if ui.button(t!("editor.insert_heading_menu")).clicked() {
            *context_action = Some(EditorContextAction::InsertSnippet("\n== New Section\n"));
            ui.close_menu();
        }
        if ui.button(t!("editor.insert_math_menu")).clicked() {
            *context_action = Some(EditorContextAction::InsertSnippet(
                "\n$ f(x) = integral_0^x e^(-t^2) dif t $\n",
            ));
            ui.close_menu();
        }
        if ui.button(t!("editor.insert_table_menu")).clicked() {
            *context_action = Some(EditorContextAction::InsertSnippet(
                "\n#table(\n  columns: (1fr, 1fr),\n  [*Column A*], [*Column B*],\n  [Item 1], [Value 1],\n)\n",
            ));
            ui.close_menu();
        }
        if ui.button(t!("editor.insert_figure_menu")).clicked() {
            *context_action = Some(EditorContextAction::InsertSnippet(
                "\n#figure(\n  image(\"assets/diagram.svg\", width: 80%),\n  caption: [Figure caption],\n)\n",
            ));
            ui.close_menu();
        }
        if ui.button(t!("editor.insert_code_menu")).clicked() {
            *context_action = Some(EditorContextAction::InsertSnippet(
                "\n```rust\nfn main() {\n    println!(\"Hello, Typst!\");\n}\n```\n",
            ));
            ui.close_menu();
        }
    });

    ui.separator();

    if ui.button(t!("menu.save_file")).clicked() {
        *context_action = Some(EditorContextAction::Menu(MenuAction::SaveFile));
        ui.close_menu();
    }
    if ui.button(t!("menu.compile_now")).clicked() {
        *context_action = Some(EditorContextAction::Menu(MenuAction::CompileNow));
        ui.close_menu();
    }
    if ui.button(t!("menu.export_pdf")).clicked() {
        *context_action = Some(EditorContextAction::Menu(MenuAction::ExportPdf));
        ui.close_menu();
    }
    if file_kind == FileKind::Typst && ui.button(t!("menu.set_main")).clicked() {
        *context_action = Some(EditorContextAction::SetActiveAsMain);
        ui.close_menu();
    }
}

/// Converts a character index into a UTF-8 byte offset within `s` (clamped to `s.len()`).
pub fn char_to_byte_index(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map(|(i, _)| i)
        .unwrap_or(s.len())
}

/// Normalizes an optional `CCursorRange` into clamped `(start_char, end_char, cursor_char)`.
pub fn normalize_char_selection(
    total_chars: usize,
    current_range: Option<CCursorRange>,
) -> (usize, usize, usize) {
    if let Some(r) = current_range {
        let a = r.primary.index.min(total_chars);
        let b = r.secondary.index.min(total_chars);
        (a.min(b), a.max(b), a)
    } else {
        (total_chars, total_chars, total_chars)
    }
}

/// Cuts `[start_char, end_char)` out of `text`, returning the removed substring if non-empty.
pub fn apply_cut_text(text: &mut String, start_char: usize, end_char: usize) -> Option<String> {
    if start_char >= end_char {
        return None;
    }
    let start_b = char_to_byte_index(text, start_char);
    let end_b = char_to_byte_index(text, end_char);
    let removed = text[start_b..end_b].to_string();
    text.replace_range(start_b..end_b, "");
    Some(removed)
}

/// Replaces `[start_char, end_char)` in `text` with `clip_text` and returns the new cursor char index.
pub fn apply_paste_text(
    text: &mut String,
    start_char: usize,
    end_char: usize,
    clip_text: &str,
) -> Option<usize> {
    if clip_text.is_empty() {
        return None;
    }
    let start_b = char_to_byte_index(text, start_char);
    let end_b = char_to_byte_index(text, end_char);
    text.replace_range(start_b..end_b, clip_text);
    Some(start_char + clip_text.chars().count())
}

/// Wraps the selected range with `prefix`/`suffix` or inserts `default_snippet` at `cursor_char`,
/// returning the resulting `(primary_char, secondary_char)` selection range.
pub fn apply_wrap_or_insert_text(
    text: &mut String,
    start_char: usize,
    end_char: usize,
    cursor_char: usize,
    default_snippet: &str,
    prefix: &str,
    suffix: &str,
) -> (usize, usize) {
    if start_char < end_char {
        let start_b = char_to_byte_index(text, start_char);
        let end_b = char_to_byte_index(text, end_char);
        let inner = text[start_b..end_b].to_string();
        let wrapped = format!("{prefix}{inner}{suffix}");
        let wrapped_chars = wrapped.chars().count();
        text.replace_range(start_b..end_b, &wrapped);
        (start_char, start_char + wrapped_chars)
    } else {
        let ins_b = char_to_byte_index(text, cursor_char);
        text.insert_str(ins_b, default_snippet);
        let end = cursor_char + default_snippet.chars().count();
        (end, end)
    }
}

/// Replaces `[start_char, end_char)` with `snippet` and returns the new cursor char index.
pub fn apply_insert_snippet_text(
    text: &mut String,
    start_char: usize,
    end_char: usize,
    snippet: &str,
) -> usize {
    let start_b = char_to_byte_index(text, start_char);
    let end_b = char_to_byte_index(text, end_char);
    text.replace_range(start_b..end_b, snippet);
    start_char + snippet.chars().count()
}

fn commit_text_mutation(
    ctx: &Context,
    text_ref: &str,
    edit_output: &mut TextEditOutput,
    mut undoer: egui::util::undoer::Undoer<(CCursorRange, String)>,
    new_range: CCursorRange,
    feed_after: bool,
    output: &mut EditorPaneOutput,
) {
    edit_output.state.cursor.set_char_range(Some(new_range));
    if feed_after {
        undoer.feed_state(ctx.input(|i| i.time), &(new_range, text_ref.to_owned()));
    }
    edit_output.state.set_undoer(undoer);
    edit_output
        .state
        .clone()
        .store(ctx, edit_output.response.id);
    output.text_changed = true;
    output.edited_text_snapshot = Some(text_ref.to_owned());
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
    let (start_char, end_char, cursor_char) = normalize_char_selection(total_chars, current_range);
    let ccursor_range = current_range.unwrap_or_default();

    match *act {
        EditorContextAction::Undo => {
            let mut undoer = edit_output.state.undoer();
            undoer.feed_state(ctx.input(|i| i.time), &(ccursor_range, text_ref.clone()));
            if let Some((undo_range, undo_txt)) = undoer.undo(&(ccursor_range, text_ref.clone())) {
                *text_ref = undo_txt.clone();
                let next_range = *undo_range;
                commit_text_mutation(
                    ctx,
                    text_ref,
                    edit_output,
                    undoer,
                    next_range,
                    false,
                    output,
                );
            }
            edit_output.response.request_focus();
        }
        EditorContextAction::Redo => {
            let mut undoer = edit_output.state.undoer();
            if let Some((redo_range, redo_txt)) = undoer.redo(&(ccursor_range, text_ref.clone())) {
                *text_ref = redo_txt.clone();
                let next_range = *redo_range;
                commit_text_mutation(
                    ctx,
                    text_ref,
                    edit_output,
                    undoer,
                    next_range,
                    false,
                    output,
                );
            }
            edit_output.response.request_focus();
        }
        EditorContextAction::Copy => {
            if start_char < end_char {
                let start_b = char_to_byte_index(text_ref, start_char);
                let end_b = char_to_byte_index(text_ref, end_char);
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
            let mut undoer = edit_output.state.undoer();
            undoer.feed_state(ctx.input(|i| i.time), &(ccursor_range, text_ref.clone()));
            if let Some(selected_str) = apply_cut_text(text_ref, start_char, end_char) {
                ctx.copy_text(selected_str.clone());
                if let Ok(mut cb) = arboard::Clipboard::new() {
                    let _ = cb.set_text(selected_str.clone());
                }
                *clipboard_fallback = selected_str;
                let new_range = CCursorRange::one(CCursor::new(start_char));
                commit_text_mutation(ctx, text_ref, edit_output, undoer, new_range, true, output);
            }
            edit_output.response.request_focus();
        }
        EditorContextAction::Paste => {
            let clip_text = arboard::Clipboard::new()
                .and_then(|mut cb| cb.get_text())
                .ok()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| clipboard_fallback.clone());
            let mut undoer = edit_output.state.undoer();
            undoer.feed_state(ctx.input(|i| i.time), &(ccursor_range, text_ref.clone()));
            if let Some(new_pos) = apply_paste_text(text_ref, start_char, end_char, &clip_text) {
                let new_range = CCursorRange::one(CCursor::new(new_pos));
                commit_text_mutation(ctx, text_ref, edit_output, undoer, new_range, true, output);
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
            let (sel_start, sel_end) = apply_wrap_or_insert_text(
                text_ref,
                start_char,
                end_char,
                cursor_char,
                default_snippet,
                prefix,
                suffix,
            );
            let new_range = if sel_start != sel_end {
                CCursorRange::two(CCursor::new(sel_start), CCursor::new(sel_end))
            } else {
                CCursorRange::one(CCursor::new(sel_end))
            };
            commit_text_mutation(ctx, text_ref, edit_output, undoer, new_range, false, output);
            edit_output.response.request_focus();
        }
        EditorContextAction::InsertSnippet(snippet) => {
            let mut undoer = edit_output.state.undoer();
            undoer.feed_state(ctx.input(|i| i.time), &(ccursor_range, text_ref.clone()));
            let new_pos = apply_insert_snippet_text(text_ref, start_char, end_char, snippet);
            let new_range = CCursorRange::one(CCursor::new(new_pos));
            commit_text_mutation(ctx, text_ref, edit_output, undoer, new_range, false, output);
            edit_output.response.request_focus();
        }
        EditorContextAction::Menu(_) | EditorContextAction::SetActiveAsMain => {}
    }
}

#[cfg(test)]
#[path = "../../tests/ui/components/editor_actions_tests.rs"]
mod tests;
