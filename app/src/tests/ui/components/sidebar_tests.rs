//! Headless `egui_kittest` tests for the collapsible project sidebar and outline.

use std::path::PathBuf;

use egui_kittest::{kittest::Queryable, Harness};
use rust_i18n::t;

use super::*;
use crate::core::outline::CompiledOutlineItem;

#[test]
fn sidebar_returns_empty_when_hidden() {
    let project = DiskProject::blank();
    let mut ratio = 0.5;
    let ctx = egui::Context::default();
    let out = render_sidebar(
        &ctx,
        SidebarParams {
            dark_mode: false,
            show_sidebar: false,
            sidebar_split_ratio: &mut ratio,
            project: &project,
            remote_presences: &[],
            compiled_outline: &[],
            last_editor_selection: None,
        },
    );
    assert!(!out.rescan_tree);
    assert!(!out.show_new_file_modal);
    assert!(out.clicked_path.is_none());
}

#[test]
fn sidebar_renders_tree_outline_and_emits_toolbar_and_navigation_actions() {
    struct SidebarState {
        project: DiskProject,
        split_ratio: f32,
        compiled_outline: Vec<CompiledOutlineItem>,
        remote_presences: Vec<PeerPresence>,
        rescan_tree: bool,
        show_new_file_modal: bool,
        navigated: Option<String>,
    }

    let project = DiskProject::blank();
    let compiled_outline = vec![CompiledOutlineItem {
        level: 1,
        numbers: Some("1".to_string()),
        title: "Introduction".to_string(),
        file: PathBuf::from("main.typ"),
        line: 1,
        char_offset: 0,
    }];
    let remote_presences = vec![PeerPresence {
        client_id: 42,
        endpoint_id: "peer-abc".to_string(),
        name: "Ada".to_string(),
        color_rgb: [64, 160, 255],
        active_file_id: Some("file-1".to_string()),
        active_path: Some("main.typ".to_string()),
        cursor_char: Some(0),
        anchor_char: None,
    }];

    let refresh_label = t!("sidebar.refresh_button", locale = "en").to_string();
    let new_file_label = t!("sidebar.new_file_button", locale = "en").to_string();

    let mut harness = Harness::new_state(
        |ctx, state: &mut SidebarState| {
            let out = render_sidebar(
                ctx,
                SidebarParams {
                    dark_mode: true,
                    show_sidebar: true,
                    sidebar_split_ratio: &mut state.split_ratio,
                    project: &state.project,
                    remote_presences: &state.remote_presences,
                    compiled_outline: &state.compiled_outline,
                    last_editor_selection: Some(CharSelection {
                        primary_char: 0,
                        secondary_char: 0,
                    }),
                },
            );
            state.rescan_tree |= out.rescan_tree;
            state.show_new_file_modal |= out.show_new_file_modal;
            if let Some(item) = out.navigate_to_item {
                state.navigated = Some(item.title);
            }
        },
        SidebarState {
            project,
            split_ratio: 0.5,
            compiled_outline,
            remote_presences,
            rescan_tree: false,
            show_new_file_modal: false,
            navigated: None,
        },
    );

    harness.get_by_label(&new_file_label).click();
    harness.run();
    assert!(harness.state().show_new_file_modal);

    harness.get_by_label(&refresh_label).click();
    harness.run();
    assert!(harness.state().rescan_tree);
}
