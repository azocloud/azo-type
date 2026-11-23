//! End-to-end headless `egui_kittest` cross-panel UI integration tests for `AzoTypeApp`.

use std::collections::HashMap;
use std::path::PathBuf;

use azo_type::app::{AzoTypeApp, MenuAction, SettingsTab};
use azo_type::core::diff::CharSelection;
use azo_type::core::i18n::AppSavedState;
use azo_type::core::recent_projects::RecentProjects;
use azo_type::services::canonicalize_project_path;
use azo_type::services::persistence::RECENT_PROJECTS_STORAGE_KEY;
use azo_type::ui::components::BottomPanelTab;
use egui::{Event, Key, Modifiers};
use egui_kittest::{kittest::Queryable, Harness};

#[derive(Default)]
struct MemoryStorage(HashMap<String, String>);

impl eframe::Storage for MemoryStorage {
    fn get_string(&self, key: &str) -> Option<String> {
        self.0.get(key).cloned()
    }

    fn set_string(&mut self, key: &str, value: String) {
        self.0.insert(key.to_string(), value);
    }

    fn flush(&mut self) {}
}

fn english_storage() -> MemoryStorage {
    let mut storage = MemoryStorage::default();
    eframe::set_value(
        &mut storage,
        eframe::APP_KEY,
        &AppSavedState {
            language: "en".to_string(),
        },
    );
    storage
}

#[test]
fn full_app_renders_workspace_and_toggles_panels_headlessly() {
    let mut harness = Harness::new(|ctx| {
        let storage = english_storage();
        let mut app = AzoTypeApp::new_headless(ctx, Some(&storage), None, None);
        app.render_ui(ctx);
    });
    harness.step();
}

#[test]
fn full_app_opens_settings_and_new_file_and_export_dialogs_across_frames() {
    struct AppHolder(Option<AzoTypeApp>);

    let settings_tab_editor = "Editor & Filesystem";
    let cancel_btn = "Cancel";
    let export_cancel_btn = "Cancel";

    let mut harness = Harness::new_state(
        |ctx, holder: &mut AppHolder| {
            let app = holder.0.get_or_insert_with(|| {
                let storage = english_storage();
                AzoTypeApp::new_headless(ctx, Some(&storage), None, None)
            });
            app.render_ui(ctx);
        },
        AppHolder(None),
    );
    harness.run();

    // 1. Toggle Bottom Panel and verify console/autocorrect panels render inside full app.
    {
        let ctx = harness.ctx.clone();
        harness
            .state_mut()
            .0
            .as_mut()
            .unwrap()
            .dispatch_menu_action(&ctx, MenuAction::ToggleBottomPanel);
    }
    harness.run();
    assert!(harness.state().0.as_ref().unwrap().bottom_panel.visible);

    // 2. Open Settings embedded subwindow and switch to Editor tab.
    {
        let ctx = harness.ctx.clone();
        let app = harness.state_mut().0.as_mut().unwrap();
        app.dispatch_menu_action(&ctx, MenuAction::OpenSettings);
    }
    harness.run();
    harness.get_by_label(settings_tab_editor).click();
    harness.run();
    assert_eq!(
        harness.state().0.as_ref().unwrap().settings_tab,
        SettingsTab::EditorAndDisk
    );
    harness.state_mut().0.as_mut().unwrap().show_settings_window = false;
    harness.run();

    // 3. Open New File modal and dismiss via Cancel button.
    {
        let ctx = harness.ctx.clone();
        let app = harness.state_mut().0.as_mut().unwrap();
        app.dispatch_menu_action(&ctx, MenuAction::NewFile);
    }
    harness.run();
    assert!(harness.state().0.as_ref().unwrap().show_new_file_input);
    harness.get_by_label(cancel_btn).click();
    harness.run();
    assert!(!harness.state().0.as_ref().unwrap().show_new_file_input);

    // 4. Open Export PDF modal and dismiss via Cancel button.
    {
        let ctx = harness.ctx.clone();
        let app = harness.state_mut().0.as_mut().unwrap();
        app.dispatch_menu_action(&ctx, MenuAction::ExportPdf);
    }
    harness.run();
    assert!(harness.state().0.as_ref().unwrap().show_export_pdf_dialog);
    harness.get_by_label(export_cancel_btn).click();
    harness.run();
    assert!(!harness.state().0.as_ref().unwrap().show_export_pdf_dialog);

    // 5. Exercise language switching cleanly in an isolated integration test process.
    {
        let ctx = harness.ctx.clone();
        let app = harness.state_mut().0.as_mut().unwrap();
        app.set_language(&ctx, "de".to_string());
        assert_eq!(app.language, "de");
        app.set_language(&ctx, "en".to_string());
        assert_eq!(app.language, "en");
    }
}

fn temp_ui_project_dir(label: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("azo_type_ui_recent_{label}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("main.typ"), format!("= {label}\n")).unwrap();
    dir
}

#[test]
fn full_app_toggles_open_last_project_setting_and_opens_recent_project_via_menu() {
    struct AppHolder(Option<AzoTypeApp>);

    let dir_a = temp_ui_project_dir("alpha");
    let dir_b = temp_ui_project_dir("beta");
    let canon_a = canonicalize_project_path(&dir_a);
    let canon_b = canonicalize_project_path(&dir_b);

    let mut storage = english_storage();
    eframe::set_value(
        &mut storage,
        RECENT_PROJECTS_STORAGE_KEY,
        &RecentProjects::from_paths([canon_b.clone(), canon_a.clone()]),
    );

    let mut harness = Harness::new_state(
        |ctx, holder: &mut AppHolder| {
            let app = holder
                .0
                .get_or_insert_with(|| AzoTypeApp::new_headless(ctx, Some(&storage), None, None));
            app.render_ui(ctx);
        },
        AppHolder(None),
    );
    harness.run();

    // 1. Settings -> Editor & Filesystem -> toggle "Open last opened project on startup".
    {
        let ctx = harness.ctx.clone();
        let app = harness.state_mut().0.as_mut().unwrap();
        assert!(!app.settings.open_last_project_on_startup);
        app.dispatch_menu_action(&ctx, MenuAction::OpenSettings);
    }
    harness.run();
    harness.get_by_label("Editor & Filesystem").click();
    harness.run();
    harness
        .get_by_label("Open last opened project on startup")
        .click();
    harness.run();
    assert!(
        harness
            .state()
            .0
            .as_ref()
            .unwrap()
            .settings
            .open_last_project_on_startup
    );
    harness.state_mut().0.as_mut().unwrap().show_settings_window = false;
    harness.run();

    // 2. File -> Recently Opened -> click second entry (dir_a) and verify MRU + project switch,
    //    then clear recent projects via File -> Recently Opened -> Clear Recently Opened.
    #[cfg(not(target_os = "macos"))]
    {
        let target_label = RecentProjects::format_entry_label(&canon_a);
        harness.get_by_label("File").click();
        harness.run();
        harness.get_by_label("Recently Opened").hover();
        harness.run();
        harness.get_by_label(&target_label).click();
        harness.run();

        let app = harness.state().0.as_ref().unwrap();
        assert_eq!(app.project.root_dir, canon_a);
        assert_eq!(app.recent_projects.entries(), &[canon_a.clone(), canon_b]);

        // Save state and verify a new headless instance auto-opens canon_a on startup.
        let mut saved_storage = english_storage();
        eframe::App::save(harness.state_mut().0.as_mut().unwrap(), &mut saved_storage);
        let restarted = AzoTypeApp::new_headless(&harness.ctx, Some(&saved_storage), None, None);
        assert_eq!(restarted.project.root_dir, canon_a);

        harness.get_by_label("File").click();
        harness.run();
        harness.get_by_label("Recently Opened").hover();
        harness.run();
        harness.get_by_label("Clear Recently Opened").click();
        harness.run();
        assert!(harness
            .state()
            .0
            .as_ref()
            .unwrap()
            .recent_projects
            .is_empty());
    }

    let _ = std::fs::remove_dir_all(&dir_a);
    let _ = std::fs::remove_dir_all(&dir_b);
}

#[test]
fn full_app_edit_tools_menu_and_shortcuts_execute_once_without_double_toggle() {
    struct AppHolder(Option<AzoTypeApp>);

    let mut harness = Harness::new_state(
        |ctx, holder: &mut AppHolder| {
            let app = holder.0.get_or_insert_with(|| {
                let storage = english_storage();
                let mut app = AzoTypeApp::new_headless(ctx, Some(&storage), None, None);
                app.settings.auto_compile = false;
                if let Some(buf) = app.project.active_buffer_mut() {
                    buf.text = Some("Hello Typst\nSecond line\n".to_string());
                    buf.dirty = false;
                }
                app.pending_cursor_restore = Some(CharSelection::caret(0));
                app
            });
            app.render_ui(ctx);
        },
        AppHolder(None),
    );
    harness.run();

    // Focus editor and place caret on line 1.
    harness
        .get_by_role(egui::accesskit::Role::MultilineTextInput)
        .focus();
    harness
        .state_mut()
        .0
        .as_mut()
        .unwrap()
        .pending_cursor_restore = Some(CharSelection::caret(0));
    harness.run();

    // 1. Trigger ToggleLineComment via menu dispatch and run multiple frames to prove no double-toggle.
    {
        let ctx = harness.ctx.clone();
        let app = harness.state_mut().0.as_mut().unwrap();
        app.dispatch_menu_action(&ctx, MenuAction::ToggleLineComment);
    }
    harness.run();
    harness.run();
    {
        let app = harness.state().0.as_ref().unwrap();
        assert_eq!(
            app.project.active_buffer().and_then(|b| b.text.as_deref()),
            Some("// Hello Typst\nSecond line\n")
        );
        assert!(app.pending_editor_context_action.is_none());
        assert!(app.project.active_buffer().unwrap().dirty);
    }

    // 2. Trigger Ctrl+/ shortcut directly while editor has focus and verify single toggle back.
    harness.press_key_modifiers(Modifiers::COMMAND, Key::Slash);
    harness.run();
    harness.run();
    {
        let app = harness.state().0.as_ref().unwrap();
        assert_eq!(
            app.project.active_buffer().and_then(|b| b.text.as_deref()),
            Some("Hello Typst\nSecond line\n")
        );
        assert!(app.pending_editor_context_action.is_none());
    }

    // 3. Trigger DuplicateLine and IndentLines via menu dispatch.
    {
        let ctx = harness.ctx.clone();
        let app = harness.state_mut().0.as_mut().unwrap();
        app.pending_cursor_restore = Some(CharSelection::caret(0));
        app.dispatch_menu_action(&ctx, MenuAction::DuplicateLine);
    }
    harness.run();
    harness.run();
    assert_eq!(
        harness
            .state()
            .0
            .as_ref()
            .unwrap()
            .project
            .active_buffer()
            .and_then(|b| b.text.as_deref()),
        Some("Hello Typst\nHello Typst\nSecond line\n")
    );
}

#[test]
fn full_app_search_and_replace_bottom_panel_navigates_and_replaces_buffer() {
    struct AppHolder(Option<AzoTypeApp>);

    let mut harness = Harness::new_state(
        |ctx, holder: &mut AppHolder| {
            let app = holder.0.get_or_insert_with(|| {
                let storage = english_storage();
                let mut app = AzoTypeApp::new_headless(ctx, Some(&storage), None, None);
                app.settings.auto_compile = false;
                if let Some(buf) = app.project.active_buffer_mut() {
                    buf.text = Some("alpha one alpha two\n".to_string());
                    buf.dirty = false;
                }
                app
            });
            app.render_ui(ctx);
        },
        AppHolder(None),
    );
    harness.run();

    // Open Search & Replace tab via Ctrl+F shortcut.
    harness.press_key_modifiers(Modifiers::COMMAND, Key::F);
    harness.run();
    {
        let app = harness.state().0.as_ref().unwrap();
        assert!(app.bottom_panel.visible);
        assert_eq!(
            app.bottom_panel.active_tab,
            BottomPanelTab::SearchAndReplace
        );
    }

    // Type query into Search input and advance to the second match via Next button.
    harness.get_by_label("Find:").focus();
    harness.input_mut().events.push(Event::Text("alpha".into()));
    harness.run();
    harness.get_by_label("1 of 2");

    harness.get_by_label("Next").click();
    harness.run();
    {
        let app = harness.state().0.as_ref().unwrap();
        assert_eq!(app.bottom_panel.search.current_match_idx, 1);
        assert_eq!(
            app.last_editor_selection,
            Some(CharSelection {
                primary_char: 15,
                secondary_char: 10,
            })
        );
    }

    // Open Replace via MenuAction::FindAndReplace and click Replace All.
    {
        let ctx = harness.ctx.clone();
        let app = harness.state_mut().0.as_mut().unwrap();
        app.dispatch_menu_action(&ctx, MenuAction::FindAndReplace);
    }
    harness.run();
    harness.get_by_label("Replace:").focus();
    harness.input_mut().events.push(Event::Text("omega".into()));
    harness.run();

    harness.get_by_label("Replace All").click();
    harness.run();
    {
        let app = harness.state().0.as_ref().unwrap();
        assert_eq!(
            app.project.active_buffer().and_then(|b| b.text.as_deref()),
            Some("omega one omega two\n")
        );
        assert!(app.project.active_buffer().unwrap().dirty);
    }
}

#[test]
fn full_app_typst_intellisense_completes_label_reference_on_tab() {
    struct AppHolder(Option<AzoTypeApp>);

    let initial = "= Intro <sec-intro>\nSee ";
    let mut harness = Harness::new_state(
        |ctx, holder: &mut AppHolder| {
            let app = holder.0.get_or_insert_with(|| {
                let storage = english_storage();
                let mut app = AzoTypeApp::new_headless(ctx, Some(&storage), None, None);
                app.settings.auto_compile = false;
                if let Some(buf) = app.project.active_buffer_mut() {
                    buf.text = Some(initial.to_string());
                    buf.dirty = false;
                }
                app.pending_cursor_restore = Some(CharSelection::caret(initial.chars().count()));
                app
            });
            app.render_ui(ctx);
        },
        AppHolder(None),
    );
    harness.run();

    // Focus the editor with caret at the end of "See ".
    harness
        .get_by_role(egui::accesskit::Role::MultilineTextInput)
        .focus();
    harness
        .state_mut()
        .0
        .as_mut()
        .unwrap()
        .pending_cursor_restore = Some(CharSelection::caret(initial.chars().count()));
    harness.run();

    // Type "@sec-" to trigger Typst reference completion popup.
    harness.input_mut().events.push(Event::Text("@sec-".into()));
    harness.run_steps(2);

    assert!(harness.query_all_by_label("sec-intro").next().is_some());

    // Accept the completion via Tab and verify active buffer + dirty state update.
    harness.press_key(Key::Tab);
    harness.run_steps(2);
    let app = harness.state().0.as_ref().unwrap();
    let text = app
        .project
        .active_buffer()
        .and_then(|b| b.text.as_deref())
        .unwrap_or("");
    assert!(text.ends_with("See @sec-intro"));
    assert!(app.project.active_buffer().unwrap().dirty);
    assert!(app.editor_intellisense.active_completion.is_none());
}

#[test]
fn full_app_search_find_input_retains_focus_on_incremental_typing_and_enter() {
    struct AppHolder(Option<AzoTypeApp>);

    let initial = "alpha one alpha two\n";
    let mut harness = Harness::new_state(
        |ctx, holder: &mut AppHolder| {
            let app = holder.0.get_or_insert_with(|| {
                let storage = english_storage();
                let mut app = AzoTypeApp::new_headless(ctx, Some(&storage), None, None);
                app.settings.auto_compile = false;
                if let Some(buf) = app.project.active_buffer_mut() {
                    buf.text = Some(initial.to_string());
                    buf.dirty = false;
                }
                app
            });
            app.render_ui(ctx);
        },
        AppHolder(None),
    );
    harness.run();

    // Open Search & Replace via Ctrl+F, which focuses `Find:`.
    harness.press_key_modifiers(Modifiers::COMMAND, Key::F);
    harness.run();
    assert!(harness.get_by_label("Find:").is_focused());
    assert!(!harness
        .get_by_role(egui::accesskit::Role::MultilineTextInput)
        .is_focused());

    // Type "alpha" one character at a time across multiple frames.
    for (idx, ch) in ["a", "l", "p", "h", "a"].into_iter().enumerate() {
        harness.input_mut().events.push(Event::Text(ch.into()));
        harness.run();

        assert!(
            harness.get_by_label("Find:").is_focused(),
            "Find input lost focus after typing character #{idx} ('{ch}')"
        );
        assert!(
            !harness
                .get_by_role(egui::accesskit::Role::MultilineTextInput)
                .is_focused(),
            "Editor stole focus after typing character #{idx} ('{ch}')"
        );
        let app = harness.state().0.as_ref().unwrap();
        assert_eq!(
            app.project.active_buffer().and_then(|b| b.text.as_deref()),
            Some(initial)
        );
    }

    {
        let app = harness.state().0.as_ref().unwrap();
        assert_eq!(app.bottom_panel.search.query, "alpha");
        assert_eq!(app.bottom_panel.search.current_match_idx, 0);
        assert_eq!(app.bottom_panel.active_search_match_range(), Some((0, 5)));
        assert_eq!(
            app.last_editor_selection,
            Some(CharSelection {
                primary_char: 5,
                secondary_char: 0,
            })
        );
    }

    // Press Enter twice in Find:; focus must remain in Find: and no newline enters the editor.
    harness.press_key(Key::Enter);
    harness.run();
    assert!(harness.get_by_label("Find:").is_focused());
    assert!(!harness
        .get_by_role(egui::accesskit::Role::MultilineTextInput)
        .is_focused());
    {
        let app = harness.state().0.as_ref().unwrap();
        assert_eq!(app.bottom_panel.search.current_match_idx, 1);
        assert_eq!(app.bottom_panel.active_search_match_range(), Some((10, 15)));
        assert_eq!(
            app.last_editor_selection,
            Some(CharSelection {
                primary_char: 15,
                secondary_char: 10,
            })
        );
        assert_eq!(
            app.project.active_buffer().and_then(|b| b.text.as_deref()),
            Some(initial)
        );
    }

    harness.press_key(Key::Enter);
    harness.run();
    assert!(harness.get_by_label("Find:").is_focused());
    assert!(!harness
        .get_by_role(egui::accesskit::Role::MultilineTextInput)
        .is_focused());
    {
        let app = harness.state().0.as_ref().unwrap();
        assert_eq!(app.bottom_panel.search.current_match_idx, 0);
        assert_eq!(app.bottom_panel.active_search_match_range(), Some((0, 5)));
        assert_eq!(
            app.project.active_buffer().and_then(|b| b.text.as_deref()),
            Some(initial)
        );
    }

    // Shift+Enter moves backward while keeping Find: focused.
    harness.press_key_modifiers(Modifiers::SHIFT, Key::Enter);
    harness.run();
    assert!(harness.get_by_label("Find:").is_focused());
    assert_eq!(
        harness
            .state()
            .0
            .as_ref()
            .unwrap()
            .bottom_panel
            .search
            .current_match_idx,
        1
    );

    // Clicking Next / Prev / Replace does not steal focus to the editor.
    harness.get_by_label("Next").click();
    harness.run();
    assert!(!harness
        .get_by_role(egui::accesskit::Role::MultilineTextInput)
        .is_focused());
}
