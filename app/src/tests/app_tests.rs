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
    assert_eq!(app.project.root_dir, dir);

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

    assert_eq!(app.project.root_dir, dir);
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
