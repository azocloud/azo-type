//! Unit tests for spell/grammar suggestion application and Tier-2 dictionary registration.

use std::path::PathBuf;

use super::*;

#[test]
fn applies_language_suggestion_preserving_utf8_boundaries() {
    let mut project = DiskProject::default();
    let active = project.active_file.clone();
    project.buffers.get_mut(&active).unwrap().text = Some("Grüß mispeled Wört.".to_string());

    let start = "Grüß ".len();
    let end = start + "mispeled".len();
    let (updated, caret) =
        apply_language_suggestion(&mut project, start, end, "misspelled", false).unwrap();

    assert_eq!(updated, "Grüß misspelled Wört.");
    assert_eq!(caret, "Grüß misspelled".chars().count());
    assert!(project.buffers.get(&active).unwrap().dirty);

    // Out-of-bounds or mid-codepoint offsets (byte 3 splits 'ü') must safely return None.
    assert!(apply_language_suggestion(&mut project, 0, 3, "x", false).is_none());
    assert!(apply_language_suggestion(&mut project, 0, 999, "x", false).is_none());
}

#[test]
fn registers_and_replaces_custom_dictionary_by_lang_code() {
    let mut settings = IdeSettings::default();
    register_user_dictionary(
        &mut settings,
        UserDictionaryEntry {
            lang_code: "pt".to_string(),
            label: "pt_PT".to_string(),
            aff_path: PathBuf::from("/tmp/a.aff"),
            dic_path: PathBuf::from("/tmp/a.dic"),
        },
    );
    register_user_dictionary(
        &mut settings,
        UserDictionaryEntry {
            lang_code: "pt".to_string(),
            label: "pt_BR".to_string(),
            aff_path: PathBuf::from("/tmp/b.aff"),
            dic_path: PathBuf::from("/tmp/b.dic"),
        },
    );
    assert_eq!(settings.user_dictionaries.len(), 1);
    assert_eq!(settings.user_dictionaries[0].label, "pt_BR");
}

#[test]
fn apply_picked_dictionary_result_updates_settings_and_triggers_recheck() {
    let ctx = egui::Context::default();
    let mut settings = IdeSettings {
        spell_check_enabled: true,
        ..Default::default()
    };
    let mut lc = LanguageCheckHost::new(Some(ctx));
    let project = DiskProject::blank();

    let entry = UserDictionaryEntry {
        lang_code: "it".to_string(),
        label: "Italian".to_string(),
        aff_path: PathBuf::from("/tmp/it.aff"),
        dic_path: PathBuf::from("/tmp/it.dic"),
    };
    let status_notice =
        apply_picked_dictionary_result(Ok(entry), &mut settings, &mut lc, &project, "en");
    assert_eq!(settings.user_dictionaries.len(), 1);
    assert!(status_notice.contains("Italian"));

    let err_notice = apply_picked_dictionary_result(
        Err("cancelled".to_string()),
        &mut settings,
        &mut lc,
        &project,
        "en",
    );
    assert!(err_notice.contains("cancelled"));
}

#[test]
fn handle_settings_and_bottom_panel_language_actions_update_state() {
    let ctx = egui::Context::default();
    let mut settings = IdeSettings {
        spell_check_enabled: true,
        ignored_words: vec!["foo".to_string()],
        user_dictionaries: vec![UserDictionaryEntry {
            lang_code: "it".to_string(),
            label: "Italian".to_string(),
            aff_path: PathBuf::from("/tmp/it.aff"),
            dic_path: PathBuf::from("/tmp/it.dic"),
        }],
        ..Default::default()
    };
    let mut lc = LanguageCheckHost::new(Some(ctx.clone()));
    let mut project = DiskProject::blank();
    let active = project.active_file.clone();
    project.buffers.get_mut(&active).unwrap().text = Some("teh world".to_string());
    let (tx, _rx) = std::sync::mpsc::channel::<()>();
    let mut status_notice = String::new();

    let settings_out = SettingsModalOutput {
        remove_user_dictionary_idx: Some(0),
        clear_ignored_words: true,
        check_language_now: true,
        ..Default::default()
    };
    handle_settings_language_actions(
        &ctx,
        &settings_out,
        &mut settings,
        &mut lc,
        &project,
        "en",
        &tx,
        |_| (),
        &mut status_notice,
    );
    assert!(settings.user_dictionaries.is_empty());
    assert!(settings.ignored_words.is_empty());

    let mut pending_cursor = None;
    let mut pending_scroll = None;
    let mut show_settings = false;
    let mut tab = SettingsTab::Appearance;
    let panel_out = BottomPanelOutput {
        open_spell_settings: true,
        ignore_word: Some("customword".to_string()),
        apply_suggestion: Some((0, 3, "the".to_string())),
        ..Default::default()
    };

    let snapshot = handle_bottom_panel_language_actions(
        &ctx,
        &panel_out,
        &mut settings,
        &mut lc,
        &mut project,
        "en",
        &mut pending_cursor,
        &mut pending_scroll,
        &mut show_settings,
        &mut tab,
    );
    assert!(show_settings);
    assert_eq!(tab, SettingsTab::SpellAndGrammar);
    assert!(settings.ignored_words.contains(&"customword".to_string()));
    assert_eq!(snapshot.as_deref(), Some("the world"));
    assert!(pending_cursor.is_some());
    assert!(pending_scroll.is_some());
}
