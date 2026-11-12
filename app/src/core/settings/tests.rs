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
    assert!(settings.dark_mode);
    assert!(settings.show_line_numbers);
    assert!(settings.syntax_highlighting);
    assert!(settings.auto_save_on_edit);
    assert!(!settings.show_hidden_files);
    assert!(settings.auto_compile);
    assert_eq!(settings.debounce_ms, 120);
    assert_eq!(settings.render_dpi_scale, 2.0);
    assert!(!settings.user_name.is_empty());
}
