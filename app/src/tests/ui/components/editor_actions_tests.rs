//! Pure unit and headless `egui_kittest` tests for editor buffer actions and shortcuts.

use std::path::Path;

use egui::TextEdit;
use egui_kittest::{kittest::Queryable, Harness};
use rust_i18n::t;

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

struct EditorHarnessState {
    text: String,
    clipboard_fallback: String,
    pending_action: Option<(EditorContextAction, Option<CCursorRange>)>,
    last_cursor: Option<CCursorRange>,
    last_output: EditorPaneOutput,
}

fn create_editor_harness(initial_text: &str) -> Harness<'_, EditorHarnessState> {
    let state = EditorHarnessState {
        text: initial_text.to_string(),
        clipboard_fallback: String::new(),
        pending_action: None,
        last_cursor: None,
        last_output: EditorPaneOutput::default(),
    };
    Harness::new_ui_state(
        |ui, state: &mut EditorHarnessState| {
            let mut edit_output = TextEdit::multiline(&mut state.text).show(ui);
            let ctx = ui.ctx().clone();
            if let Some((act, explicit_range)) = state.pending_action.take() {
                state.last_output = EditorPaneOutput::default();
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
            state.last_cursor = edit_output.state.cursor.char_range();
        },
        state,
    )
}

fn dispatch_action(
    harness: &mut Harness<'_, EditorHarnessState>,
    action: EditorContextAction,
    range: Option<CCursorRange>,
) {
    harness.state_mut().pending_action = Some((action, range));
    harness.run();
}

#[test]
fn render_editor_context_menu_emits_toggle_comment_actions() {
    struct MenuState {
        action: Option<EditorContextAction>,
    }

    let line_label = t!("menu.toggle_line_comment", locale = "en").to_string();
    let block_label = t!("menu.toggle_block_comment", locale = "en").to_string();

    let mut harness = Harness::new_ui_state(
        |ui, state: &mut MenuState| {
            render_editor_context_menu(
                ui,
                Path::new("main.typ"),
                FileKind::Typst,
                true,
                false,
                true,
                &mut state.action,
            );
        },
        MenuState { action: None },
    );
    harness.run();

    harness.get_by_label(&line_label).click();
    harness.run();
    assert_eq!(
        harness.state().action,
        Some(EditorContextAction::Menu(MenuAction::ToggleLineComment))
    );

    harness.state_mut().action = None;
    harness.get_by_label(&block_label).click();
    harness.run();
    assert_eq!(
        harness.state().action,
        Some(EditorContextAction::Menu(MenuAction::ToggleBlockComment))
    );
}

#[test]
fn menu_toggle_line_and_block_comment_mutate_selection_and_undo() {
    let mut harness = create_editor_harness("alpha\nbeta");
    harness.run();

    dispatch_action(
        &mut harness,
        EditorContextAction::Menu(MenuAction::ToggleLineComment),
        Some(CCursorRange::two(CCursor::new(1), CCursor::new(8))),
    );
    assert_eq!(harness.state().text, "// alpha\n// beta");
    assert!(harness.state().last_output.text_changed);
    assert_eq!(
        harness.state().last_cursor,
        Some(CCursorRange::two(CCursor::new(4), CCursor::new(14)))
    );

    dispatch_action(&mut harness, EditorContextAction::Undo, None);
    assert_eq!(harness.state().text, "alpha\nbeta");
    assert!(harness.state().last_output.text_changed);

    dispatch_action(&mut harness, EditorContextAction::Redo, None);
    assert_eq!(harness.state().text, "// alpha\n// beta");

    dispatch_action(
        &mut harness,
        EditorContextAction::Menu(MenuAction::ToggleLineComment),
        None,
    );
    assert_eq!(harness.state().text, "alpha\nbeta");
    assert_eq!(
        harness.state().last_cursor,
        Some(CCursorRange::two(CCursor::new(1), CCursor::new(8)))
    );

    let mut block_harness = create_editor_harness("hello world");
    block_harness.run();

    dispatch_action(
        &mut block_harness,
        EditorContextAction::Menu(MenuAction::ToggleBlockComment),
        Some(CCursorRange::two(CCursor::new(6), CCursor::new(11))),
    );
    assert_eq!(block_harness.state().text, "hello /* world */");
    assert_eq!(
        block_harness.state().last_cursor,
        Some(CCursorRange::two(CCursor::new(6), CCursor::new(17)))
    );

    dispatch_action(&mut block_harness, EditorContextAction::Undo, None);
    assert_eq!(block_harness.state().text, "hello world");

    dispatch_action(
        &mut block_harness,
        EditorContextAction::Menu(MenuAction::ToggleBlockComment),
        Some(CCursorRange::one(CCursor::new(5))),
    );
    assert_eq!(block_harness.state().text, "hello/*  */ world");
    assert_eq!(
        block_harness.state().last_cursor,
        Some(CCursorRange::one(CCursor::new(8)))
    );
}

#[test]
fn menu_indent_outdent_duplicate_and_move_lines_mutate_selection_and_undo() {
    let mut harness = create_editor_harness("one\ntwo");
    harness.run();

    dispatch_action(
        &mut harness,
        EditorContextAction::Menu(MenuAction::IndentLines),
        Some(CCursorRange::two(CCursor::new(1), CCursor::new(6))),
    );
    assert_eq!(harness.state().text, "  one\n  two");
    assert_eq!(
        harness.state().last_cursor,
        Some(CCursorRange::two(CCursor::new(3), CCursor::new(10)))
    );

    dispatch_action(&mut harness, EditorContextAction::Undo, None);
    assert_eq!(harness.state().text, "one\ntwo");

    dispatch_action(&mut harness, EditorContextAction::Redo, None);
    assert_eq!(harness.state().text, "  one\n  two");

    dispatch_action(
        &mut harness,
        EditorContextAction::Menu(MenuAction::OutdentLines),
        None,
    );
    assert_eq!(harness.state().text, "one\ntwo");
    assert_eq!(
        harness.state().last_cursor,
        Some(CCursorRange::two(CCursor::new(1), CCursor::new(6)))
    );

    dispatch_action(
        &mut harness,
        EditorContextAction::Menu(MenuAction::OutdentLines),
        None,
    );
    assert_eq!(harness.state().text, "one\ntwo");
    assert!(!harness.state().last_output.text_changed);

    let mut dup_harness = create_editor_harness("first\nsecond");
    dup_harness.run();
    dispatch_action(
        &mut dup_harness,
        EditorContextAction::Menu(MenuAction::DuplicateLine),
        Some(CCursorRange::one(CCursor::new(2))),
    );
    assert_eq!(dup_harness.state().text, "first\nfirst\nsecond");
    assert_eq!(
        dup_harness.state().last_cursor,
        Some(CCursorRange::one(CCursor::new(8)))
    );
    dispatch_action(&mut dup_harness, EditorContextAction::Undo, None);
    assert_eq!(dup_harness.state().text, "first\nsecond");

    let mut move_harness = create_editor_harness("top\nmid\nbot");
    move_harness.run();
    dispatch_action(
        &mut move_harness,
        EditorContextAction::Menu(MenuAction::MoveLineUp),
        Some(CCursorRange::one(CCursor::new(5))),
    );
    assert_eq!(move_harness.state().text, "mid\ntop\nbot");
    assert_eq!(
        move_harness.state().last_cursor,
        Some(CCursorRange::one(CCursor::new(1)))
    );

    dispatch_action(
        &mut move_harness,
        EditorContextAction::Menu(MenuAction::MoveLineUp),
        None,
    );
    assert_eq!(move_harness.state().text, "mid\ntop\nbot");
    assert!(!move_harness.state().last_output.text_changed);

    dispatch_action(&mut move_harness, EditorContextAction::Undo, None);
    assert_eq!(move_harness.state().text, "top\nmid\nbot");

    dispatch_action(
        &mut move_harness,
        EditorContextAction::Menu(MenuAction::MoveLineDown),
        Some(CCursorRange::one(CCursor::new(5))),
    );
    assert_eq!(move_harness.state().text, "top\nbot\nmid");
    assert_eq!(
        move_harness.state().last_cursor,
        Some(CCursorRange::one(CCursor::new(9)))
    );

    dispatch_action(
        &mut move_harness,
        EditorContextAction::Menu(MenuAction::MoveLineDown),
        None,
    );
    assert_eq!(move_harness.state().text, "top\nbot\nmid");
    assert!(!move_harness.state().last_output.text_changed);
}
