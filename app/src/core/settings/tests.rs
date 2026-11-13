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
