//! Headless `egui_kittest` tests for the Spell & Grammar Settings tab.

use std::path::PathBuf;

use egui_kittest::{kittest::Queryable, Harness};
use rust_i18n::t;

use super::*;
use crate::core::spellcheck::UserDictionaryEntry;

struct SpellHarnessState {
    settings: IdeSettings,
    dict_manager: DictionaryManager,
    palette: ThemePalette,
    check_now: bool,
    cleared: bool,
    add_user_dict: bool,
    removed_idx: Option<usize>,
}

#[test]
fn spell_tab_toggles_flags_and_clears_ignored_words() {
    let tmp = std::env::temp_dir().join("azo_type_test_settings_spell");
    let dict_manager = DictionaryManager::with_dir(tmp);
    dict_manager.set_error("simulated offline error".to_string());

    let state = SpellHarnessState {
        settings: IdeSettings {
            spell_check_enabled: true,
            auto_spell_check: true,
            grammar_check_enabled: true,
            ignored_words: vec!["foo".to_string(), "bar".to_string()],
            user_dictionaries: vec![UserDictionaryEntry {
                lang_code: "nl".to_string(),
                label: "Dutch".to_string(),
                aff_path: PathBuf::from("/tmp/nl.aff"),
                dic_path: PathBuf::from("/tmp/nl.dic"),
            }],
            ..Default::default()
        },
        dict_manager,
        palette: ThemePalette::for_mode(false),
        check_now: false,
        cleared: false,
        add_user_dict: false,
        removed_idx: None,
    };

    let enable_label = t!("settings.spell_enable_label", locale = "en").to_string();
    let clear_ignored_label = t!("settings.spell_clear_ignored_btn", locale = "en").to_string();
    let add_dict_label = t!("settings.spell_add_dict_btn", locale = "en").to_string();
    let remove_btn_label = t!("settings.typst_remove_btn", locale = "en").to_string();

    let mut harness = Harness::new_ui_state(
        |ui, state: &mut SpellHarnessState| {
            let mut output = SettingsModalOutput::default();
            render_spell_and_grammar_tab(
                ui,
                &mut state.settings,
                &state.dict_manager,
                &state.palette,
                &mut output,
            );
            state.check_now |= output.check_language_now;
            state.cleared |= output.clear_ignored_words;
            state.add_user_dict |= output.add_user_dictionary_requested;
            if output.remove_user_dictionary_idx.is_some() {
                state.removed_idx = output.remove_user_dictionary_idx;
            }
        },
        state,
    );

    harness.get_by_label(&clear_ignored_label).click();
    harness.run();
    assert!(harness.state().cleared);

    harness.get_by_label(&add_dict_label).click();
    harness.run();
    assert!(harness.state().add_user_dict);

    harness.get_by_label(&remove_btn_label).click();
    harness.run();
    assert_eq!(harness.state().removed_idx, Some(0));

    harness.get_by_label(&enable_label).click();
    harness.run();
    assert!(!harness.state().settings.spell_check_enabled);
    assert!(harness.state().check_now);
}
