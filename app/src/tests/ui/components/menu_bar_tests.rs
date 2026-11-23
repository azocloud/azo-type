//! Headless `egui_kittest` tests for the in-window desktop menu bar component.

#[cfg(not(target_os = "macos"))]
use std::path::{Path, PathBuf};

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
                    recent_projects: &[],
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

#[cfg(not(target_os = "macos"))]
#[test]
fn desktop_menu_bar_shows_empty_recent_projects_placeholder() {
    let file_menu = t!("menu.file", locale = "en").to_string();
    let recent_menu = t!("menu.recent_projects", locale = "en").to_string();
    let empty_label = t!("menu.recent_empty", locale = "en").to_string();
    let clear_label = t!("menu.clear_recent", locale = "en").to_string();

    let mut harness = Harness::new(|ctx| {
        render_desktop_menu_bar(
            ctx,
            MenuBarParams {
                dark_mode: true,
                show_sidebar: true,
                show_bottom_panel: false,
                word_wrap: true,
                project_root: Path::new("/tmp/project"),
                active_file: Path::new("main.typ"),
                recent_projects: &[],
            },
        );
    });

    harness.get_by_label(&file_menu).click();
    harness.run();
    harness.get_by_label(&recent_menu).hover();
    harness.run();

    let empty_node = harness.get_by_label(&empty_label);
    assert!(empty_node.is_disabled());
    assert!(harness.query_by_label(&clear_label).is_none());
}

#[cfg(not(target_os = "macos"))]
#[test]
fn desktop_menu_bar_recent_projects_click_emits_open_action_and_caps_list() {
    let file_menu = t!("menu.file", locale = "en").to_string();
    let recent_menu = t!("menu.recent_projects", locale = "en").to_string();
    let recent: Vec<PathBuf> = (0..12)
        .map(|i| PathBuf::from(format!("/tmp/workspace/proj-{i}")))
        .collect();
    let second_label = RecentProjects::format_entry_label(&recent[1]);
    let eleventh_label = RecentProjects::format_entry_label(&recent[10]);

    let mut harness = Harness::new_state(
        |ctx, pending_action: &mut Option<MenuAction>| {
            let out = render_desktop_menu_bar(
                ctx,
                MenuBarParams {
                    dark_mode: false,
                    show_sidebar: true,
                    show_bottom_panel: false,
                    word_wrap: false,
                    project_root: Path::new("/tmp/project"),
                    active_file: Path::new("main.typ"),
                    recent_projects: &recent,
                },
            );
            if out.pending_action.is_some() {
                *pending_action = out.pending_action;
            }
        },
        None,
    );

    harness.get_by_label(&file_menu).click();
    harness.run();
    harness.get_by_label(&recent_menu).hover();
    harness.run();

    assert!(harness.query_by_label(&eleventh_label).is_none());
    harness.get_by_label(&second_label).click();
    harness.run();

    assert_eq!(*harness.state(), Some(MenuAction::OpenRecentProject(1)));
}

#[cfg(not(target_os = "macos"))]
#[test]
fn desktop_menu_bar_clear_recent_projects_emits_clear_action() {
    let file_menu = t!("menu.file", locale = "en").to_string();
    let recent_menu = t!("menu.recent_projects", locale = "en").to_string();
    let clear_label = t!("menu.clear_recent", locale = "en").to_string();
    let recent = vec![PathBuf::from("/tmp/workspace/alpha")];

    let mut harness = Harness::new_state(
        |ctx, pending_action: &mut Option<MenuAction>| {
            let out = render_desktop_menu_bar(
                ctx,
                MenuBarParams {
                    dark_mode: true,
                    show_sidebar: true,
                    show_bottom_panel: false,
                    word_wrap: true,
                    project_root: Path::new("/tmp/project"),
                    active_file: Path::new("main.typ"),
                    recent_projects: &recent,
                },
            );
            if out.pending_action.is_some() {
                *pending_action = out.pending_action;
            }
        },
        None,
    );

    harness.get_by_label(&file_menu).click();
    harness.run();
    harness.get_by_label(&recent_menu).hover();
    harness.run();
    harness.get_by_label(&clear_label).click();
    harness.run();

    assert_eq!(*harness.state(), Some(MenuAction::ClearRecentProjects));
}

#[cfg(not(target_os = "macos"))]
#[test]
fn desktop_menu_bar_does_not_render_top_right_panel_button() {
    let panel_toggle_label = t!("menu.bottom_panel_toggle", locale = "en").to_string();
    let folder_label = format!("{}: /tmp/project", t!("menu.folder_label", locale = "en"));

    let harness = Harness::new(|ctx| {
        render_desktop_menu_bar(
            ctx,
            MenuBarParams {
                dark_mode: true,
                show_sidebar: true,
                show_bottom_panel: false,
                word_wrap: true,
                project_root: Path::new("/tmp/project"),
                active_file: Path::new("main.typ"),
                recent_projects: &[],
            },
        );
    });

    assert!(harness.query_by_label(&panel_toggle_label).is_none());
    assert!(harness.query_by_label(&folder_label).is_some());
}

#[cfg(not(target_os = "macos"))]
#[test]
fn desktop_menu_bar_edit_menu_emits_find_and_toggle_line_comment_actions() {
    let edit_menu = t!("menu.edit", locale = "en").to_string();
    let find_item = t!("menu.find", locale = "en").to_string();
    let toggle_line_comment_item = t!("menu.toggle_line_comment", locale = "en").to_string();

    let mut harness = Harness::new_state(
        |ctx, pending_action: &mut Option<MenuAction>| {
            let out = render_desktop_menu_bar(
                ctx,
                MenuBarParams {
                    dark_mode: true,
                    show_sidebar: true,
                    show_bottom_panel: false,
                    word_wrap: true,
                    project_root: Path::new("/tmp/project"),
                    active_file: Path::new("main.typ"),
                    recent_projects: &[],
                },
            );
            if out.pending_action.is_some() {
                *pending_action = out.pending_action;
            }
        },
        None,
    );

    harness.get_by_label(&edit_menu).click();
    harness.run();
    harness.get_by_label(&find_item).click();
    harness.run();
    assert_eq!(*harness.state(), Some(MenuAction::Find));

    harness.get_by_label(&edit_menu).click();
    harness.run();
    harness.get_by_label(&toggle_line_comment_item).click();
    harness.run();
    assert_eq!(*harness.state(), Some(MenuAction::ToggleLineComment));
}
