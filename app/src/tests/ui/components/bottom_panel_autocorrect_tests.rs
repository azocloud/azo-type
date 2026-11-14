//! Headless `egui_kittest` tests for the bottom panel Auto-Correction tab.

use std::path::Path;

use egui_kittest::{kittest::Queryable, Harness};
use rust_i18n::t;

use super::*;
use crate::core::spellcheck::LanguageCheckDiagnostic;

#[test]
fn autocorrect_tab_emits_jump_suggestion_ignore_check_and_settings_actions() {
    struct AutoState {
        spell_check_enabled: bool,
        auto_spell_check: bool,
        language_check: LanguageCheckHost,
        check_now: bool,
        open_spell_settings: bool,
        jump_to_char: Option<usize>,
        apply_suggestion: Option<(usize, usize, String)>,
        ignore_word: Option<String>,
    }

    let mut lc = LanguageCheckHost::new(None);
    lc.effective_lang = "en_US".to_string();
    lc.last_duration_ms = 5.4;
    lc.diagnostics = vec![
        LanguageCheckDiagnostic {
            category: DiagnosticCategory::Spelling,
            start_byte: 0,
            end_byte: 3,
            start_char: 0,
            end_char: 3,
            line: 1,
            column: 1,
            excerpt: "teh".to_string(),
            message: "Unknown word".to_string(),
            rule_id: "SPELLBOOK".to_string(),
            suggestions: vec!["the".to_string()],
        },
        LanguageCheckDiagnostic {
            category: DiagnosticCategory::Grammar,
            start_byte: 4,
            end_byte: 10,
            start_char: 4,
            end_char: 10,
            line: 1,
            column: 5,
            excerpt: "effect".to_string(),
            message: "Check article".to_string(),
            rule_id: "GRAMMAR/ART".to_string(),
            suggestions: vec![],
        },
    ];

    let check_now_label = t!("bottom_panel.autocorrect_check_now", locale = "en").to_string();
    let settings_label = t!("bottom_panel.autocorrect_settings_btn", locale = "en").to_string();
    let ignore_label = t!("bottom_panel.autocorrect_ignore_btn", locale = "en").to_string();

    let mut harness = Harness::new_ui_state(
        |ui, state: &mut AutoState| {
            let palette = ThemePalette::for_mode(false);
            let mut output = BottomPanelOutput::default();
            render_autocorrect_tab(
                ui,
                false,
                &palette,
                Path::new("main.typ"),
                true,
                &mut state.spell_check_enabled,
                &mut state.auto_spell_check,
                &state.language_check,
                &mut output,
            );
            state.check_now |= output.check_language_now;
            state.open_spell_settings |= output.open_spell_settings;
            if output.jump_to_char.is_some() {
                state.jump_to_char = output.jump_to_char;
            }
            if output.apply_suggestion.is_some() {
                state.apply_suggestion = output.apply_suggestion;
            }
            if output.ignore_word.is_some() {
                state.ignore_word = output.ignore_word;
            }
        },
        AutoState {
            spell_check_enabled: true,
            auto_spell_check: true,
            language_check: lc,
            check_now: false,
            open_spell_settings: false,
            jump_to_char: None,
            apply_suggestion: None,
            ignore_word: None,
        },
    );

    harness.get_by_label("L1:1").click();
    harness.run();
    assert_eq!(harness.state().jump_to_char, Some(0));

    harness.get_by_label("→ the").click();
    harness.run();
    assert_eq!(
        harness.state().apply_suggestion,
        Some((0, 3, "the".to_string()))
    );

    harness.get_by_label(&ignore_label).click();
    harness.run();
    assert_eq!(harness.state().ignore_word.as_deref(), Some("teh"));

    harness.get_by_label(&check_now_label).click();
    harness.run();
    assert!(harness.state().check_now);

    harness.get_by_label(&settings_label).click();
    harness.run();
    assert!(harness.state().open_spell_settings);
}

#[test]
fn autocorrect_tab_handles_disabled_and_missing_pack_states() {
    let mut lc = LanguageCheckHost::new(None);
    lc.missing_tier1_pack = Some("fr_FR".to_string());
    let dl_label = t!(
        "bottom_panel.autocorrect_download_pack_btn",
        locale = "en",
        lang = "fr_FR"
    )
    .to_string();

    struct MissingState {
        enabled: bool,
        auto: bool,
        lc: LanguageCheckHost,
        dl_lang: Option<String>,
    }

    let mut harness = Harness::new_ui_state(
        |ui, s: &mut MissingState| {
            let palette = ThemePalette::for_mode(true);
            let mut out = BottomPanelOutput::default();
            render_autocorrect_tab(
                ui,
                true,
                &palette,
                Path::new("main.typ"),
                true,
                &mut s.enabled,
                &mut s.auto,
                &s.lc,
                &mut out,
            );
            if out.download_dictionary_lang.is_some() {
                s.dl_lang = out.download_dictionary_lang;
            }
        },
        MissingState {
            enabled: true,
            auto: false,
            lc,
            dl_lang: None,
        },
    );

    harness.get_by_label(&dl_label).click();
    harness.run();
    assert_eq!(harness.state().dl_lang.as_deref(), Some("fr_FR"));
}
