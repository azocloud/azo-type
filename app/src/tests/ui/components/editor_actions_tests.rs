//! Pure unit and headless `egui_kittest` tests for editor buffer actions and shortcuts.

use egui::TextEdit;
use egui_kittest::Harness;

use super::*;

#[test]
fn char_to_byte_index_handles_multibyte_and_out_of_bounds() {
    let s = "héllo 🌍!";
    assert_eq!(char_to_byte_index(s, 0), 0);
    assert_eq!(char_to_byte_index(s, 1), 1);
    assert_eq!(char_to_byte_index(s, 2), 3); // 'é' is 2 bytes
    assert_eq!(char_to_byte_index(s, 999), s.len());
}

#[test]
fn normalize_char_selection_clamps_and_orders_endpoints() {
    let range = CCursorRange::two(CCursor::new(50), CCursor::new(3));
    assert_eq!(
        normalize_char_selection(10, Some(range)),
        (3, 10, range.primary.index.min(10))
    );
    assert_eq!(normalize_char_selection(10, None), (10, 10, 10));
}

#[test]
fn apply_cut_and_paste_text_preserve_utf8_boundaries() {
    let mut text = "alpha βeta gamma".to_string();
    let removed = apply_cut_text(&mut text, 6, 10);
    assert_eq!(removed.as_deref(), Some("βeta"));
    assert_eq!(text, "alpha  gamma");

    assert_eq!(apply_cut_text(&mut text, 4, 4), None);

    let new_pos = apply_paste_text(&mut text, 6, 7, "δelta ").unwrap();
    assert_eq!(text, "alpha δelta gamma");
    assert_eq!(new_pos, 12);
    assert_eq!(apply_paste_text(&mut text, 0, 0, ""), None);
}

#[test]
fn apply_wrap_or_insert_wraps_selection_and_inserts_default_when_collapsed() {
    let mut text = "hello world".to_string();
    let (s, e) = apply_wrap_or_insert_text(&mut text, 6, 11, 11, "*bold*", "*", "*");
    assert_eq!(text, "hello *world*");
    assert_eq!((s, e), (6, 13));

    let (s2, e2) = apply_wrap_or_insert_text(&mut text, 5, 5, 5, " _em_", "_", "_");
    assert_eq!(text, "hello _em_ *world*");
    assert_eq!((s2, e2), (10, 10));
}

#[test]
fn apply_insert_snippet_text_replaces_selection_and_advances_cursor() {
    let mut text = "before middle after".to_string();
    let new_pos = apply_insert_snippet_text(&mut text, 7, 13, "$x^2$");
    assert_eq!(text, "before $x^2$ after");
    assert_eq!(new_pos, 12);
}

#[test]
fn apply_editor_context_action_executes_wrap_select_all_snippet_and_undo() {
    struct ActionState {
        text: String,
        clipboard_fallback: String,
        queued_actions: Vec<(EditorContextAction, Option<CCursorRange>)>,
        last_output: EditorPaneOutput,
    }

    let state = ActionState {
        text: "hello world".to_string(),
        clipboard_fallback: String::new(),
        queued_actions: vec![
            (
                EditorContextAction::WrapOrInsert("*bold*", "*", "*"),
                Some(CCursorRange::two(CCursor::new(6), CCursor::new(11))),
            ),
            (EditorContextAction::SelectAll, None),
            (EditorContextAction::InsertSnippet("= Title"), None),
        ],
        last_output: EditorPaneOutput::default(),
    };

    let mut harness = Harness::new_ui_state(
        |ui, state: &mut ActionState| {
            let mut edit_output = TextEdit::multiline(&mut state.text).show(ui);
            let ctx = ui.ctx().clone();
            if !state.queued_actions.is_empty() {
                let (act, explicit_range) = state.queued_actions.remove(0);
                let range = explicit_range.or_else(|| edit_output.state.cursor.char_range());
                apply_editor_context_action(
                    &ctx,
                    &act,
                    &mut state.text,
                    &mut edit_output,
                    range,
                    &mut state.clipboard_fallback,
                    &mut state.last_output,
                );
            }
        },
        state,
    );

    harness.run();
    assert_eq!(harness.state().text, "= Title");
    assert!(harness.state().last_output.text_changed);
}
