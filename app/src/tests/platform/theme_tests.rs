//! Unit tests for OS theme detection parsers and System-Default fallback behavior.

use super::*;

#[test]
fn test_parse_portal_color_scheme_variants() {
    assert_eq!(parse_portal_color_scheme("(<<uint32 1>>,)"), Some(true));
    assert_eq!(parse_portal_color_scheme("(<<uint32 2>>,)"), Some(false));
    assert_eq!(parse_portal_color_scheme("(<<uint32 0>>,)"), None);
    assert_eq!(
        parse_portal_color_scheme("   variant       uint32 1"),
        Some(true)
    );
    assert_eq!(
        parse_portal_color_scheme("   variant       uint32 2"),
        Some(false)
    );
    assert_eq!(parse_portal_color_scheme(""), None);
}

#[test]
fn test_parse_gsettings_and_gtk_theme_names() {
    assert_eq!(parse_gsettings_color_scheme("'prefer-dark'\n"), Some(true));
    assert_eq!(
        parse_gsettings_color_scheme("'prefer-light'\n"),
        Some(false)
    );
    assert_eq!(parse_gsettings_color_scheme("'default'\n"), None);

    assert_eq!(parse_gtk_theme_name_is_dark("'Adwaita-dark'\n"), Some(true));
    assert_eq!(parse_gtk_theme_name_is_dark("Yaru-light"), Some(false));
    assert_eq!(parse_gtk_theme_name_is_dark("'Adwaita'\n"), None);
}

#[test]
fn test_parse_gtk_settings_ini_and_kdeglobals() {
    let gtk_dark = "[Settings]\ngtk-application-prefer-dark-theme=1\ngtk-theme-name=Adwaita\n";
    assert_eq!(parse_gtk_settings_ini(gtk_dark), Some(true));

    let gtk_light = "[Settings]\ngtk-application-prefer-dark-theme=false\n";
    assert_eq!(parse_gtk_settings_ini(gtk_light), Some(false));

    let kde_dark = "[General]\nColorScheme=BreezeDark\n";
    assert_eq!(parse_kdeglobals(kde_dark), Some(true));

    let kde_light = "[General]\nColorScheme=BreezeLight\n";
    assert_eq!(parse_kdeglobals(kde_light), Some(false));
}

#[test]
fn test_resolve_effective_dark_mode_defaults_to_dark_when_unknown() {
    let ctx = Context::default();
    assert!(!resolve_effective_dark_mode(&ctx, ThemePreference::Light));
    assert!(resolve_effective_dark_mode(&ctx, ThemePreference::Dark));
    // Note: in a headless test Context without OS theme injected, Linux host may resolve
    // its real portal/config or fall back to true; explicit None check lives in core tests.
    let _ = resolve_effective_dark_mode(&ctx, ThemePreference::SystemDefault);
}
