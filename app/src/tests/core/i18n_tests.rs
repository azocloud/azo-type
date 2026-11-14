//! Unit tests for locale normalization, system language detection, and native display names.

use super::*;

#[test]
fn test_normalize_locale_tag() {
    assert_eq!(normalize_locale_tag("de-DE"), "de");
    assert_eq!(normalize_locale_tag("en_US"), "en");
    assert_eq!(normalize_locale_tag("DE"), "de");
    assert_eq!(normalize_locale_tag("en_US.UTF-8"), "en");
    assert_eq!(normalize_locale_tag(""), "en");
    assert_eq!(normalize_locale_tag("   "), "en");
}

#[test]
fn test_resolve_locale_match() {
    let available = ["en", "de"];
    assert_eq!(resolve_locale(Some("de-AT"), &available), "de");
    assert_eq!(resolve_locale(Some("en-GB"), &available), "en");
    assert_eq!(resolve_locale(Some("fr-FR"), &available), "en");
    assert_eq!(resolve_locale(None, &available), "en");
}

#[test]
fn test_locale_native_names() {
    assert_eq!(locale_native_name("en"), "English");
    assert_eq!(locale_native_name("de"), "Deutsch");
    assert_eq!(locale_native_name("unknown"), "English");
}

#[test]
fn test_translation_switching() {
    assert_eq!(
        rust_i18n::t!("settings.language_label", locale = "en"),
        "Language:"
    );
    assert_eq!(
        rust_i18n::t!("settings.language_label", locale = "de"),
        "Sprache:"
    );
}

#[test]
fn test_app_saved_state_serialization() {
    let state = AppSavedState {
        language: "de".to_string(),
    };
    let json = serde_json::to_string(&state).expect("serialize");
    let loaded: AppSavedState = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(loaded, state);
    assert_eq!(AppSavedState::default().language, "en");
}
