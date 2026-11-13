//! Unit tests for default IDE user preferences and settings initialization.

use super::*;

#[test]
fn test_word_wrap_enabled_by_default() {
    let settings = IdeSettings::default();
    assert!(
        settings.word_wrap,
        "Automatic line wrapping must be on by default"
    );
}

#[test]
fn test_default_settings_values() {
    let settings = IdeSettings::default();
    assert_eq!(settings.theme_preference, ThemePreference::SystemDefault);
    assert!(settings.dark_mode);
    assert!(settings.show_line_numbers);
    assert!(settings.syntax_highlighting);
    assert!(settings.auto_save_on_edit);
    assert!(!settings.show_hidden_files);
    assert!(settings.auto_compile);
    assert_eq!(settings.debounce_ms, 120);
    assert_eq!(settings.render_dpi_scale, 2.0);
    assert!(settings.preview_sync_selection);
    assert!(!settings.preview_pan_mode);
    assert!(!settings.user_name.is_empty());
    assert!(!settings.spell_check_enabled);
    assert!(!settings.auto_spell_check);
    assert!(settings.grammar_check_enabled);
    assert_eq!(settings.spell_check_debounce_ms, 450);
    assert_eq!(settings.spell_check_language, "auto");
    assert!(settings.user_dictionaries.is_empty());
    assert!(settings.ignored_words.is_empty());
}

#[test]
fn test_preview_zoom_fit_width_and_explicit_presets() {
    let mut zoom = PreviewZoom::default();
    assert!(zoom.fit_width);
    assert_eq!(zoom.scale, 1.0);

    // 900px viewport / 600pt page => 1.5x scale (not artificially capped at 1.35)
    let fit_scale = zoom.effective_scale_for_viewport(900.0, 600.0);
    assert!((fit_scale - 1.5).abs() < 1e-4);
    zoom.effective_scale = fit_scale;
    assert_eq!(zoom.display_percent(), 150);

    // Wheel zoom from Fit Width transitions smoothly into fixed zoom from effective_scale.
    zoom.apply_zoom_delta(1.1);
    assert!(!zoom.fit_width);
    assert!((zoom.scale - 1.65).abs() < 1e-4);

    // Switching back to Fit Width preserves the mode cleanly.
    zoom.set_fit_width();
    assert!(zoom.fit_width);

    // Explicit percent presets clamp cleanly and disable Fit Width.
    zoom.set_scale(2.0);
    assert!(!zoom.fit_width);
    assert_eq!(zoom.display_percent(), 200);
    assert!((zoom.effective_scale_for_viewport(400.0, 600.0) - 2.0).abs() < 1e-4);

    zoom.set_scale(10.0);
    assert_eq!(zoom.scale, MAX_PREVIEW_ZOOM);
    zoom.set_scale(0.05);
    assert_eq!(zoom.scale, MIN_PREVIEW_ZOOM);
}

#[test]
fn test_preview_zoom_step_and_reset() {
    let mut zoom = PreviewZoom {
        fit_width: true,
        scale: 1.0,
        effective_scale: 0.8,
    };
    zoom.step_in();
    assert!(!zoom.fit_width);
    assert!((zoom.scale - 0.95).abs() < 1e-4);

    zoom.step_out();
    assert!((zoom.scale - 0.80).abs() < 1e-4);

    zoom.set_fit_width();
    zoom.reset_100();
    assert!(!zoom.fit_width);
    assert_eq!(zoom.scale, 1.0);
}

#[test]
fn test_theme_preference_resolution_and_dark_fallback() {
    assert!(!ThemePreference::Light.resolve_dark_mode(Some(true)));
    assert!(!ThemePreference::Light.resolve_dark_mode(None));

    assert!(ThemePreference::Dark.resolve_dark_mode(Some(false)));
    assert!(ThemePreference::Dark.resolve_dark_mode(None));

    assert!(ThemePreference::SystemDefault.resolve_dark_mode(Some(true)));
    assert!(!ThemePreference::SystemDefault.resolve_dark_mode(Some(false)));
    assert!(
        ThemePreference::SystemDefault.resolve_dark_mode(None),
        "Unsupported or failed OS detection must default to Dark"
    );
}

#[test]
fn test_ide_settings_serde_roundtrip_and_partial_defaults() {
    let custom = IdeSettings {
        theme_preference: ThemePreference::Light,
        dark_mode: false,
        editor_font_size: 16.5,
        word_wrap: false,
        show_line_numbers: false,
        syntax_highlighting: false,
        auto_save_on_edit: false,
        show_hidden_files: true,
        auto_compile: false,
        debounce_ms: 350,
        render_dpi_scale: 3.0,
        typst_version: "0.14.2".to_string(),
        preview_sync_selection: false,
        preview_pan_mode: true,
        user_name: "Alice".to_string(),
        user_color: [10, 20, 30],
        spell_check_enabled: true,
        auto_spell_check: true,
        grammar_check_enabled: false,
        spell_check_debounce_ms: 600,
        spell_check_language: "de".to_string(),
        user_dictionaries: Vec::new(),
        ignored_words: vec!["Typst".to_string()],
    };

    let json = serde_json::to_string(&custom).expect("serialize IdeSettings");
    let loaded: IdeSettings = serde_json::from_str(&json).expect("deserialize IdeSettings");
    assert_eq!(loaded, custom);

    // Older or partial configs missing newly added fields fill in defaults cleanly.
    let partial: IdeSettings = serde_json::from_str(r#"{"editor_font_size":99.0,"debounce_ms":5}"#)
        .expect("deserialize partial IdeSettings");
    let sanitized = partial.sanitized();
    assert_eq!(sanitized.editor_font_size, 22.0);
    assert_eq!(sanitized.debounce_ms, 50);
    assert_eq!(sanitized.typst_version, BUILTIN_TYPST_VERSION);
    assert_eq!(sanitized.theme_preference, ThemePreference::SystemDefault);
    assert!(sanitized.preview_sync_selection);
    assert!(!sanitized.preview_pan_mode);
}
