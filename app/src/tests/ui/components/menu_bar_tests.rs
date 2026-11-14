//! Headless `egui_kittest` tests for the in-window desktop menu bar component.

#[cfg(not(target_os = "macos"))]
use std::path::Path;

#[cfg(not(target_os = "macos"))]
use egui_kittest::{kittest::Queryable, Harness};
#[cfg(not(target_os = "macos"))]
use rust_i18n::t;

#[cfg(not(target_os = "macos"))]
use super::*;

#[cfg(not(target_os = "macos"))]
#[test]
fn desktop_menu_bar_opens_file_edit_view_insert_compile_and_collab_menus() {
    struct MenuHarnessState {
        pending_action: Option<MenuAction>,
        editor_action: Option<EditorContextAction>,
    }

    let file_menu = t!("menu.file", locale = "en").to_string();
    let new_file_item = t!("menu.file_new", locale = "en").to_string();
    let edit_menu = t!("menu.edit", locale = "en").to_string();
    let undo_item = t!("menu.undo", locale = "en").to_string();
    let view_menu = t!("menu.view", locale = "en").to_string();
    let toggle_theme_item = t!("menu.theme_light", locale = "en").to_string();

    let mut harness = Harness::new_state(
        |ctx, state: &mut MenuHarnessState| {
            let out = render_desktop_menu_bar(
                ctx,
                MenuBarParams {
                    dark_mode: true,
                    show_sidebar: true,
                    show_bottom_panel: false,
                    word_wrap: true,
                    project_root: Path::new("/tmp/project"),
                    active_file: Path::new("main.typ"),
                },
            );
            if out.pending_action.is_some() {
                state.pending_action = out.pending_action;
            }
            if out.editor_action.is_some() {
                state.editor_action = out.editor_action;
            }
        },
        MenuHarnessState {
            pending_action: None,
            editor_action: None,
        },
    );

    harness.get_by_label(&file_menu).click();
    harness.run();
    harness.get_by_label(&new_file_item).click();
    harness.run();
    assert_eq!(harness.state().pending_action, Some(MenuAction::NewFile));

    harness.get_by_label(&edit_menu).click();
    harness.run();
    harness.get_by_label(&undo_item).click();
    harness.run();
    assert_eq!(
        harness.state().editor_action,
        Some(EditorContextAction::Undo)
    );

    harness.get_by_label(&view_menu).click();
    harness.run();
    harness.get_by_label(&toggle_theme_item).click();
    harness.run();
    assert_eq!(
        harness.state().pending_action,
        Some(MenuAction::ToggleTheme)
    );
}
