//! Pure unit and headless `egui_kittest` tests for the Settings dialog.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use egui_kittest::{kittest::Queryable, Harness};
use rust_i18n::t;

use super::*;
use crate::core::settings::ThemePreference;
use crate::world::SharedWorldResources;

struct SettingsHarnessState {
    settings: IdeSettings,
    active_tab: SettingsTab,
    project_root: PathBuf,
    engine_manager: EngineManager,
    dict_manager: DictionaryManager,
    open_folder_triggered: bool,
    reset_engine_seen: bool,
}

fn make_managers() -> (EngineManager, DictionaryManager) {
    let res = Arc::new(SharedWorldResources::new());
    (EngineManager::new(res), DictionaryManager::new())
}

#[test]
fn closed_settings_dialog_returns_default_output() {
    let ctx = egui::Context::default();
    let mut open = false;
    let mut settings = IdeSettings::default();
    let mut active_tab = SettingsTab::Appearance;
    let (engine_manager, dict_manager) = make_managers();

    let output = render_settings_dialog(
        &ctx,
        &mut open,
        &mut settings,
        "en",
        &mut active_tab,
        Path::new("."),
        &engine_manager,
        &dict_manager,
    );

    assert!(!output.theme_changed);
    assert!(!output.dpi_changed);
    assert!(!output.engine_changed);
    assert!(output.language_changed.is_none());
}

#[test]
fn apply_theme_preference_updates_dark_mode_and_sets_flag() {
    let ctx = egui::Context::default();
    let mut settings = IdeSettings {
        dark_mode: false,
        theme_preference: ThemePreference::Light,
        ..Default::default()
    };
    let mut output = SettingsModalOutput::default();

    apply_theme_preference(&ctx, &mut settings, ThemePreference::Dark, &mut output);

    assert_eq!(settings.theme_preference, ThemePreference::Dark);
    assert!(settings.dark_mode);
    assert!(output.theme_changed);
}

#[test]
fn reset_settings_to_defaults_restores_defaults_and_marks_refresh_flags() {
    let ctx = egui::Context::default();
    let mut settings = IdeSettings {
        editor_font_size: 20.0,
        render_dpi_scale: 3.0,
        word_wrap: false,
        ..Default::default()
    };
    let mut output = SettingsModalOutput::default();

    reset_settings_to_defaults(&ctx, &mut settings, &mut output);

    assert_eq!(
        settings.editor_font_size,
        IdeSettings::default().editor_font_size
    );
    assert_eq!(
        settings.render_dpi_scale,
        IdeSettings::default().render_dpi_scale
    );
    assert!(output.dpi_changed);
    assert!(output.engine_changed);
}

#[test]
fn settings_body_switches_tabs_and_triggers_actions() {
    let (engine_manager, dict_manager) = make_managers();
    let state = SettingsHarnessState {
        settings: IdeSettings {
            editor_font_size: 18.5,
            ..Default::default()
        },
        active_tab: SettingsTab::Appearance,
        project_root: PathBuf::from("/tmp/example-project"),
        engine_manager,
        dict_manager,
        open_folder_triggered: false,
        reset_engine_seen: false,
    };

    let tab_editor = t!("settings.tab_editor", locale = "en").to_string();
    let tab_compiler = t!("settings.tab_compiler", locale = "en").to_string();
    let tab_spell = t!("settings.tab_spell_grammar", locale = "en").to_string();
    let open_folder_btn = t!("settings.choose_folder_btn", locale = "en").to_string();
    let reset_btn = t!("settings.reset_defaults", locale = "en").to_string();

    let mut harness = Harness::new_ui_state(
        |ui, state: &mut SettingsHarnessState| {
            let ctx = ui.ctx().clone();
            let out = render_settings_body(
                &ctx,
                ui,
                &mut state.settings,
                "en",
                &mut state.active_tab,
                &state.project_root,
                &state.engine_manager,
                &state.dict_manager,
            );
            state.open_folder_triggered |= out.open_folder_requested;
            state.reset_engine_seen |= out.engine_changed;
        },
        state,
    );

    harness.get_by_label(&tab_editor).click();
    harness.run();
    assert_eq!(harness.state().active_tab, SettingsTab::EditorAndDisk);

    harness.get_by_label(&open_folder_btn).click();
    harness.run();
    assert!(harness.state().open_folder_triggered);

    harness.get_by_label(&tab_compiler).click();
    harness.run();
    assert_eq!(harness.state().active_tab, SettingsTab::CompilerAndPreview);

    harness.get_by_label(&tab_spell).click();
    harness.run();
    assert_eq!(harness.state().active_tab, SettingsTab::SpellAndGrammar);

    harness.get_by_label(&reset_btn).click();
    harness.run();
    assert!(harness.state().reset_engine_seen);
    assert_eq!(
        harness.state().settings.editor_font_size,
        IdeSettings::default().editor_font_size
    );
}

#[test]
fn download_progress_bar_renders_all_stages() {
    for stage in [
        DownloadStage::Connecting,
        DownloadStage::Downloading,
        DownloadStage::VerifyingAndExtracting,
    ] {
        let prog = DownloadProgress {
            version: "0.15.1".to_string(),
            downloaded_bytes: 512,
            total_bytes: Some(1024),
            started_at: Instant::now(),
            stage,
        };
        let mut harness = Harness::new_ui(|ui| {
            render_download_progress(ui, &prog);
        });
        harness.step();
    }
}
