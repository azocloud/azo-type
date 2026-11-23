//! Unit tests for `AzoTypeApp` orchestration, menu dispatch, snapshot application, and channel polling.

use std::collections::HashMap;
use std::path::PathBuf;

use egui::{Event, Key, Modifiers, RawInput};

use super::*;
use crate::compiler::{DiagnosticItem, PreviewCharBox};

#[derive(Default)]
struct MemoryStorage {
    map: HashMap<String, String>,
}

impl eframe::Storage for MemoryStorage {
    fn get_string(&self, key: &str) -> Option<String> {
        self.map.get(key).cloned()
    }

    fn set_string(&mut self, key: &str, value: String) {
        self.map.insert(key.to_string(), value);
    }

    fn flush(&mut self) {}
}

fn english_storage() -> MemoryStorage {
    let mut storage = MemoryStorage::default();
    // Pin locale to English so parallel `egui_kittest` unit tests in `--lib`
    // never race with host OS locale detection mutating global `rust_i18n`.
    eframe::set_value(
        &mut storage,
        eframe::APP_KEY,
        &crate::core::i18n::AppSavedState {
            language: "en".to_string(),
        },
    );
    storage
}

#[test]
fn new_headless_opens_disk_folder_and_saves_state_to_storage() {
    let dir =
        std::env::temp_dir().join(format!("azo_type_app_headless_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("main.typ"), "= Title\nBody.\n").unwrap();

    let ctx = Context::default();
    let mut storage = english_storage();
    let mut app = AzoTypeApp::new_headless(&ctx, Some(&storage), Some(dir.clone()), None);

    assert!(app.project.has_disk_backing());
    let expected_root = crate::services::canonicalize_project_path(&dir);
    assert_eq!(app.project.root_dir, expected_root);

    eframe::App::save(&mut app, &mut storage);
    assert!(storage
        .map
        .contains_key(crate::services::persistence::SETTINGS_STORAGE_KEY));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn dispatch_menu_action_updates_ui_toggles_and_zoom_and_theme() {
    let ctx = Context::default();
    let storage = english_storage();
    let mut app = AzoTypeApp::new_headless(&ctx, Some(&storage), None, None);

    assert!(app.show_sidebar);
    app.dispatch_menu_action(&ctx, MenuAction::ToggleSidebar);
    assert!(!app.show_sidebar);

    assert!(!app.bottom_panel.visible);
    app.dispatch_menu_action(&ctx, MenuAction::ToggleBottomPanel);
    assert!(app.bottom_panel.visible);

    let prev_wrap = app.settings.word_wrap;
    app.dispatch_menu_action(&ctx, MenuAction::ToggleWordWrap);
    assert_ne!(app.settings.word_wrap, prev_wrap);

    let prev_dark = app.settings.dark_mode;
    app.dispatch_menu_action(&ctx, MenuAction::ToggleTheme);
    assert_ne!(app.settings.dark_mode, prev_dark);

    app.dispatch_menu_action(&ctx, MenuAction::ZoomIn);
    let zoomed = app.zoom.scale;
    assert!(zoomed > 1.0 || !app.zoom.fit_width);
    app.dispatch_menu_action(&ctx, MenuAction::ZoomOut);
    app.dispatch_menu_action(&ctx, MenuAction::ZoomReset);
    assert!(!app.zoom.fit_width);
    assert!((app.zoom.scale - 1.0).abs() < 0.001);

    app.dispatch_menu_action(&ctx, MenuAction::NewFile);
    assert!(app.show_new_file_input);
    app.dispatch_menu_action(&ctx, MenuAction::OpenSettings);
    assert!(app.show_settings_window);
    app.dispatch_menu_action(&ctx, MenuAction::OpenCollab);
    assert!(app.collab_ui.show_window);
    app.dispatch_menu_action(&ctx, MenuAction::ExportPdf);
    assert!(app.show_export_pdf_dialog);
    app.dispatch_menu_action(&ctx, MenuAction::SaveAll);
    app.dispatch_menu_action(&ctx, MenuAction::RescanFolder);
    app.dispatch_menu_action(&ctx, MenuAction::Quit);
}

#[test]
fn apply_compile_snapshot_updates_pages_diagnostics_and_console() {
    let ctx = Context::default();
    let storage = english_storage();
    let mut app = AzoTypeApp::new_headless(&ctx, Some(&storage), None, None);
    app.compile_in_progress = true;

    let img = Arc::new(egui::ColorImage::new([4, 4], egui::Color32::WHITE));
    let snapshot = CompileSnapshot {
        revision: 7,
        pixel_per_pt: 2.0,
        compiled_ok: true,
        pages: vec![RenderedPageData {
            page_number: 1,
            width_pt: 200.0,
            height_pt: 300.0,
            image: img,
            char_boxes: Arc::new(Vec::<PreviewCharBox>::new()),
        }],
        outline: vec![CompiledOutlineItem {
            level: 1,
            numbers: Some("1".to_string()),
            title: "Intro".to_string(),
            file: PathBuf::from("main.typ"),
            line: 1,
            char_offset: 0,
        }],
        diagnostics: vec![DiagnosticItem {
            is_error: false,
            file: PathBuf::from("main.typ"),
            line: Some(1),
            column: Some(1),
            message: "minor warning".to_string(),
            hints: vec![],
        }],
        duration_ms: 19.0,
    };

    app.apply_compile_snapshot(&ctx, snapshot);

    assert!(!app.compile_in_progress);
    assert_eq!(app.last_compiled_revision, 7);
    assert_eq!(app.page_textures.len(), 1);
    assert_eq!(app.compiled_outline.len(), 1);
    assert_eq!(app.diagnostics.len(), 1);
    assert!(app
        .bottom_panel
        .console_logs
        .iter()
        .any(|l| l.contains("[compile] 19 ms")));
}

#[test]
fn poll_channels_processes_io_events_and_navigation_clamps_cursor() {
    let dir = std::env::temp_dir().join(format!("azo_type_app_poll_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("main.typ"), "= Heading\nLine two.\n").unwrap();

    let ctx = Context::default();
    let storage = english_storage();
    let mut app = AzoTypeApp::new_headless(&ctx, Some(&storage), None, None);

    app.io_tx
        .send(IoEvent::StatusMessage("custom notice".to_string()))
        .unwrap();
    app.io_tx
        .send(IoEvent::PdfExportFinished {
            status: "pdf ready".to_string(),
            console: "[PDF Export] Done".to_string(),
        })
        .unwrap();
    app.io_tx.send(IoEvent::FolderPicked(dir.clone())).unwrap();
    app.io_tx
        .send(IoEvent::DictionaryPicked(Ok(UserDictionaryEntry {
            lang_code: "fr".to_string(),
            label: "French".to_string(),
            aff_path: dir.join("fr.aff"),
            dic_path: dir.join("fr.dic"),
        })))
        .unwrap();

    app.poll_channels(&ctx);

    let expected_root = crate::services::canonicalize_project_path(&dir);
    assert_eq!(app.project.root_dir, expected_root);
    assert!(app
        .bottom_panel
        .console_logs
        .iter()
        .any(|l| l.contains("[PDF Export] Done")));
    assert_eq!(app.settings.user_dictionaries.len(), 1);

    app.on_active_text_modified(&ctx, Some("= Edited\n".to_string()));
    assert!(app.pending_compile);

    app.jump_to_diagnostic(
        &ctx,
        DiagnosticItem {
            is_error: true,
            file: PathBuf::from("main.typ"),
            line: Some(2),
            column: Some(999),
            message: "err".to_string(),
            hints: vec![],
        },
    );
    assert!(app.pending_cursor_restore.is_some());
    assert!(app.pending_scroll_to_char.is_some());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn handle_keyboard_shortcuts_toggles_sidebar_on_ctrl_b() {
    let ctx = Context::default();
    let storage = english_storage();
    let mut app = AzoTypeApp::new_headless(&ctx, Some(&storage), None, None);
    let was_visible = app.show_sidebar;

    let raw = RawInput {
        events: vec![Event::Key {
            key: Key::B,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::COMMAND,
        }],
        ..Default::default()
    };
    let _ = ctx.run(raw, |ctx| {
        app.handle_keyboard_shortcuts(ctx);
    });

    assert_ne!(app.show_sidebar, was_visible);
}

fn temp_project_dir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "azo_type_app_recent_{label}_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("main.typ"), format!("= {label}\n")).unwrap();
    dir
}

#[test]
fn new_headless_opens_last_project_on_startup_when_enabled() {
    let dir_last = temp_project_dir("startup_last");
    let dir_cli = temp_project_dir("startup_cli");
    let canon_last = crate::services::canonicalize_project_path(&dir_last);
    let canon_cli = crate::services::canonicalize_project_path(&dir_cli);

    let ctx = Context::default();
    let mut storage = english_storage();
    let settings = IdeSettings {
        open_last_project_on_startup: true,
        ..IdeSettings::default()
    };
    let recent = RecentProjects::from_paths([canon_last.clone()]);
    eframe::set_value(
        &mut storage,
        crate::services::persistence::SETTINGS_STORAGE_KEY,
        &settings,
    );
    eframe::set_value(
        &mut storage,
        crate::services::persistence::RECENT_PROJECTS_STORAGE_KEY,
        &recent,
    );

    let app = AzoTypeApp::new_headless(&ctx, Some(&storage), None, None);
    assert!(app.project.has_disk_backing());
    assert_eq!(app.project.root_dir, canon_last);
    assert_eq!(
        app.recent_projects.most_recent(),
        Some(canon_last.as_path())
    );

    // Explicit CLI folder takes precedence and moves to front of MRU.
    let app_cli = AzoTypeApp::new_headless(&ctx, Some(&storage), Some(dir_cli.clone()), None);
    assert!(app_cli.project.has_disk_backing());
    assert_eq!(app_cli.project.root_dir, canon_cli);
    assert_eq!(app_cli.recent_projects.entries(), &[canon_cli, canon_last]);

    let _ = std::fs::remove_dir_all(&dir_last);
    let _ = std::fs::remove_dir_all(&dir_cli);
}

#[test]
fn new_headless_falls_back_to_blank_when_startup_auto_open_disabled_or_missing() {
    let dir_last = temp_project_dir("startup_disabled");
    let canon_last = crate::services::canonicalize_project_path(&dir_last);

    let ctx = Context::default();
    let mut storage = english_storage();
    eframe::set_value(
        &mut storage,
        crate::services::persistence::RECENT_PROJECTS_STORAGE_KEY,
        &RecentProjects::from_paths([canon_last.clone()]),
    );

    // Disabled by default -> blank project, while preserving recent history.
    let app_disabled = AzoTypeApp::new_headless(&ctx, Some(&storage), None, None);
    assert!(!app_disabled.project.has_disk_backing());
    assert_eq!(
        app_disabled.recent_projects.entries(),
        std::slice::from_ref(&canon_last)
    );

    // Enabled but last folder missing -> falls back to blank cleanly.
    std::fs::remove_dir_all(&dir_last).unwrap();
    eframe::set_value(
        &mut storage,
        crate::services::persistence::SETTINGS_STORAGE_KEY,
        &IdeSettings {
            open_last_project_on_startup: true,
            ..IdeSettings::default()
        },
    );
    let app_missing = AzoTypeApp::new_headless(&ctx, Some(&storage), None, None);
    assert!(!app_missing.project.has_disk_backing());
}

#[test]
fn folder_picked_and_open_recent_project_update_mru_and_clear_resets_list() {
    let dir_a = temp_project_dir("mru_a");
    let dir_b = temp_project_dir("mru_b");
    let canon_a = crate::services::canonicalize_project_path(&dir_a);
    let canon_b = crate::services::canonicalize_project_path(&dir_b);

    let ctx = Context::default();
    let mut storage = english_storage();
    let mut app = AzoTypeApp::new_headless(&ctx, Some(&storage), None, None);
    assert!(app.recent_projects.is_empty());

    app.io_tx
        .send(IoEvent::FolderPicked(dir_a.clone()))
        .unwrap();
    app.poll_channels(&ctx);
    assert_eq!(app.project.root_dir, canon_a);
    assert_eq!(
        app.recent_projects.entries(),
        std::slice::from_ref(&canon_a)
    );

    app.io_tx
        .send(IoEvent::FolderPicked(dir_b.clone()))
        .unwrap();
    app.poll_channels(&ctx);
    assert_eq!(app.project.root_dir, canon_b);
    assert_eq!(
        app.recent_projects.entries(),
        &[canon_b.clone(), canon_a.clone()]
    );

    // Selecting index 1 (dir_a) moves it to front of MRU.
    app.dispatch_menu_action(&ctx, MenuAction::OpenRecentProject(1));
    assert_eq!(app.project.root_dir, canon_a);
    assert_eq!(
        app.recent_projects.entries(),
        &[canon_a.clone(), canon_b.clone()]
    );

    // Out-of-bounds index is a safe no-op.
    app.dispatch_menu_action(&ctx, MenuAction::OpenRecentProject(99));
    assert_eq!(app.project.root_dir, canon_a);

    // Removing dir_b from disk and selecting it prunes missing entry while keeping dir_a open.
    std::fs::remove_dir_all(&dir_b).unwrap();
    app.dispatch_menu_action(&ctx, MenuAction::OpenRecentProject(1));
    assert_eq!(app.project.root_dir, canon_a);
    assert_eq!(
        app.recent_projects.entries(),
        std::slice::from_ref(&canon_a)
    );
    assert!(!app.status_notice.is_empty());

    // Persistence saves recent projects; ClearRecentProjects empties list.
    eframe::App::save(&mut app, &mut storage);
    let persisted = crate::services::load_persisted_state(Some(&storage), &["en"]);
    assert_eq!(persisted.recent_projects.entries(), &[canon_a]);

    app.dispatch_menu_action(&ctx, MenuAction::ClearRecentProjects);
    assert!(app.recent_projects.is_empty());

    let _ = std::fs::remove_dir_all(&dir_a);
}

#[test]
fn dispatch_menu_action_find_replace_and_edit_tools_sets_expected_state() {
    let ctx = Context::default();
    let storage = english_storage();
    let mut app = AzoTypeApp::new_headless(&ctx, Some(&storage), None, None);

    app.dispatch_menu_action(&ctx, MenuAction::Find);
    assert!(app.bottom_panel.visible);
    assert_eq!(
        app.bottom_panel.active_tab,
        crate::ui::components::BottomPanelTab::SearchAndReplace
    );
    assert!(app.bottom_panel.search.focus_search_requested);
    assert!(!app.bottom_panel.search.focus_replace_requested);

    app.dispatch_menu_action(&ctx, MenuAction::FindAndReplace);
    assert!(app.bottom_panel.visible);
    assert_eq!(
        app.bottom_panel.active_tab,
        crate::ui::components::BottomPanelTab::SearchAndReplace
    );
    assert!(!app.bottom_panel.search.focus_search_requested);
    assert!(app.bottom_panel.search.focus_replace_requested);

    for act in [
        MenuAction::ToggleLineComment,
        MenuAction::ToggleBlockComment,
        MenuAction::IndentLines,
        MenuAction::OutdentLines,
        MenuAction::DuplicateLine,
        MenuAction::MoveLineUp,
        MenuAction::MoveLineDown,
    ] {
        app.dispatch_menu_action(&ctx, act);
        assert_eq!(
            app.pending_editor_context_action,
            Some(EditorContextAction::Menu(act))
        );
    }
}

#[test]
fn bottom_panel_output_search_replace_updates_buffer_selection_and_dirty_state() {
    let ctx = Context::default();
    let storage = english_storage();
    let mut app = AzoTypeApp::new_headless(&ctx, Some(&storage), None, None);
    if let Some(buf) = app.project.active_buffer_mut() {
        buf.text = Some("= Heading\nalpha beta alpha\n".to_string());
        buf.dirty = false;
    }
    app.bottom_panel.visible = true;
    app.inspect_asset_mode = true;
    app.pending_compile = false;

    let output = crate::ui::components::BottomPanelOutput {
        replace_in_active_text: Some("= Heading\ngamma beta alpha\n".to_string()),
        jump_to_selection: Some((21, 999)),
        ..Default::default()
    };

    let (open_diag, edited_text) = crate::services::bottom_panel_actions::apply_bottom_panel_output(
        &ctx,
        &output,
        &mut app.bottom_panel.visible,
        &mut app.settings,
        &mut app.language_check,
        &mut app.project,
        &app.language,
        &mut app.inspect_asset_mode,
        &mut app.pending_cursor_restore,
        &mut app.pending_scroll_to_char,
        &mut app.show_settings_window,
        &mut app.settings_tab,
    );
    assert!(open_diag.is_none());
    let edited = edited_text.expect("search/replace must emit updated snapshot");
    app.on_active_text_modified(&ctx, Some(edited));

    let buf = app.project.active_buffer().expect("active buffer");
    assert_eq!(buf.text.as_deref(), Some("= Heading\ngamma beta alpha\n"));
    assert!(buf.dirty);
    assert!(app.pending_compile);
    assert!(!app.inspect_asset_mode);
    // Updated text has 27 chars; end 999 clamps to 27.
    assert_eq!(
        app.pending_cursor_restore,
        Some(CharSelection {
            primary_char: 27,
            secondary_char: 21,
        })
    );
    assert_eq!(app.pending_scroll_to_char, Some(21));

    let close_out = crate::ui::components::BottomPanelOutput {
        close_requested: true,
        ..Default::default()
    };
    let _ = crate::services::bottom_panel_actions::apply_bottom_panel_output(
        &ctx,
        &close_out,
        &mut app.bottom_panel.visible,
        &mut app.settings,
        &mut app.language_check,
        &mut app.project,
        &app.language,
        &mut app.inspect_asset_mode,
        &mut app.pending_cursor_restore,
        &mut app.pending_scroll_to_char,
        &mut app.show_settings_window,
        &mut app.settings_tab,
    );
    assert!(!app.bottom_panel.visible);
}
