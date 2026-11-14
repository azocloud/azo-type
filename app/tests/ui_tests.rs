//! End-to-end headless `egui_kittest` cross-panel UI integration tests for `AzoTypeApp`.

use std::collections::HashMap;

use azo_type::app::{AzoTypeApp, MenuAction, SettingsTab};
use azo_type::core::i18n::AppSavedState;
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
