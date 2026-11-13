//! Unit tests for `eframe::Storage` settings persistence and reload behavior.

use std::collections::HashMap;

use eframe::Storage;

use super::*;
use crate::core::settings::ThemePreference;

#[derive(Default)]
struct MemoryStorage {
    entries: HashMap<String, String>,
    flush_count: usize,
}

impl eframe::Storage for MemoryStorage {
    fn get_string(&self, key: &str) -> Option<String> {
        self.entries.get(key).cloned()
    }

    fn set_string(&mut self, key: &str, value: String) {
        self.entries.insert(key.to_owned(), value);
    }

    fn flush(&mut self) {
        self.flush_count += 1;
    }
}

#[test]
fn test_settings_persist_and_reload_roundtrip_across_restart() {
    let mut storage = MemoryStorage::default();

    let custom = IdeSettings {
        theme_preference: ThemePreference::Light,
        dark_mode: false,
        editor_font_size: 18.0,
        word_wrap: false,
        show_line_numbers: false,
        syntax_highlighting: false,
        auto_save_on_edit: false,
        show_hidden_files: true,
        auto_compile: false,
        debounce_ms: 420,
        render_dpi_scale: 3.0,
        typst_version: "0.14.2".to_string(),
        preview_sync_selection: false,
        preview_pan_mode: true,
        user_name: "Tester".to_string(),
        user_color: [12, 34, 56],
    };

    let saved = PersistedAppState {
        language: "de".to_string(),
        settings: custom.clone(),
        zoom: PreviewZoom {
            fit_width: false,
            scale: 1.5,
            effective_scale: 1.5,
        },
    };
    save_persisted_state(&mut storage, &saved);

    let reloaded = load_persisted_state(Some(&storage), &["en", "de"]);
    assert_eq!(reloaded.language, "de");
    assert_eq!(reloaded.settings, custom);
    assert!(!reloaded.zoom.fit_width);
    assert!((reloaded.zoom.scale - 1.5).abs() < 1e-6);
}

#[test]
fn test_corrupt_storage_falls_back_gracefully_without_panicking() {
    let mut storage = MemoryStorage::default();
    storage.set_string(SETTINGS_STORAGE_KEY, "{not valid ron".to_string());
    storage.set_string(PREVIEW_ZOOM_STORAGE_KEY, "invalid".to_string());

    let loaded = load_persisted_state(Some(&storage), &["en", "de"]);
    assert_eq!(
        loaded.settings.debounce_ms,
        IdeSettings::default().debounce_ms
    );
    assert!(loaded.zoom.fit_width);
}
