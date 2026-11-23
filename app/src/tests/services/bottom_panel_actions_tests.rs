//! Unit tests for bottom panel search, replace, and output orchestration helpers.

use egui::Context;

use super::*;
use crate::core::diff::CharSelection;
use crate::project::DiskProject;
use crate::ui::components::BottomPanelOutput;

#[test]
fn handle_bottom_panel_search_actions_replaces_text_and_clamps_selection() {
    let ctx = Context::default();
    let mut project = DiskProject::blank();
    if let Some(buf) = project.active_buffer_mut() {
        buf.text = Some("Hello 世界".to_string());
    }

    let mut inspect_asset_mode = true;
    let mut pending_cursor_restore = None;
    let mut pending_scroll_to_char = None;

    let output = BottomPanelOutput {
        replace_in_active_text: Some("Hi 世界!".to_string()),
        jump_to_selection: Some((3, 99)),
        ..Default::default()
    };

    let replaced = handle_bottom_panel_search_actions(
        &ctx,
        &output,
        &mut project,
        &mut inspect_asset_mode,
        &mut pending_cursor_restore,
        &mut pending_scroll_to_char,
    );

    assert_eq!(replaced.as_deref(), Some("Hi 世界!"));
    assert_eq!(
        project.active_buffer().and_then(|b| b.text.as_deref()),
        Some("Hi 世界!")
    );
    // "Hi 世界!" has 6 Unicode scalar chars; end_c 99 must clamp to 6.
    assert_eq!(
        pending_cursor_restore,
        Some(CharSelection {
            primary_char: 6,
            secondary_char: 3,
        })
    );
    assert_eq!(pending_scroll_to_char, Some(3));
    assert!(!inspect_asset_mode);
}

#[test]
fn inline_editor_menu_action_predicate_matches_only_seven_edit_tools() {
    assert!(is_inline_editor_menu_action(MenuAction::ToggleLineComment));
    assert!(is_inline_editor_menu_action(MenuAction::ToggleBlockComment));
    assert!(is_inline_editor_menu_action(MenuAction::IndentLines));
    assert!(is_inline_editor_menu_action(MenuAction::OutdentLines));
    assert!(is_inline_editor_menu_action(MenuAction::DuplicateLine));
    assert!(is_inline_editor_menu_action(MenuAction::MoveLineUp));
    assert!(is_inline_editor_menu_action(MenuAction::MoveLineDown));

    assert!(!is_inline_editor_menu_action(MenuAction::Find));
    assert!(!is_inline_editor_menu_action(MenuAction::FindAndReplace));
    assert!(!is_inline_editor_menu_action(MenuAction::SaveFile));
    assert!(!is_inline_editor_menu_action(MenuAction::CompileNow));
}

#[test]
fn append_snippet_to_active_buffer_appends_and_returns_snapshot() {
    let mut project = DiskProject::blank();
    if let Some(buf) = project.active_buffer_mut() {
        buf.text = Some("= Title".to_string());
    }
    let updated = append_snippet_to_active_buffer(&mut project, "\n== Section");
    assert_eq!(updated.as_deref(), Some("= Title\n== Section"));
    assert_eq!(
        project.active_buffer().and_then(|b| b.text.as_deref()),
        Some("= Title\n== Section")
    );
}
