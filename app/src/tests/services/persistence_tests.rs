//! Unit tests for `eframe::Storage` settings persistence and reload behavior.

use std::collections::HashMap;
use std::path::PathBuf;

use eframe::Storage;

use super::*;
use crate::core::recent_projects::{RecentProjects, MAX_RECENT_PROJECTS};
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
        open_last_project_on_startup: true,
        auto_compile: false,
        debounce_ms: 420,
        render_dpi_scale: 3.0,
        typst_version: "0.14.2".to_string(),
        preview_sync_selection: false,
        preview_pan_mode: true,
        user_name: "Tester".to_string(),
        user_color: [12, 34, 56],
        spell_check_enabled: true,
        auto_spell_check: true,
        grammar_check_enabled: false,
        spell_check_debounce_ms: 500,
        spell_check_language: "de".to_string(),
        user_dictionaries: Vec::new(),
        ignored_words: vec!["Azo".to_string()],
    };

    let recent_projects = RecentProjects::from_paths([
        PathBuf::from("/tmp/project-a"),
        PathBuf::from("/tmp/project-b"),
    ]);

    let saved = PersistedAppState {
        language: "de".to_string(),
        settings: custom.clone(),
        zoom: PreviewZoom {
            fit_width: false,
            scale: 1.5,
            effective_scale: 1.5,
        },
        recent_projects: recent_projects.clone(),
    };
    save_persisted_state(&mut storage, &saved);

    let reloaded = load_persisted_state(Some(&storage), &["en", "de"]);
    assert_eq!(reloaded.language, "de");
    assert_eq!(reloaded.settings, custom);
    assert!(reloaded.settings.open_last_project_on_startup);
    assert!(!reloaded.zoom.fit_width);
    assert!((reloaded.zoom.scale - 1.5).abs() < 1e-6);
    assert_eq!(reloaded.recent_projects, recent_projects);
}

#[test]
fn test_corrupt_storage_falls_back_gracefully_without_panicking() {
    let mut storage = MemoryStorage::default();
    storage.set_string(SETTINGS_STORAGE_KEY, "{not valid ron".to_string());
    storage.set_string(PREVIEW_ZOOM_STORAGE_KEY, "invalid".to_string());
    storage.set_string(RECENT_PROJECTS_STORAGE_KEY, "corrupt".to_string());

    let loaded = load_persisted_state(Some(&storage), &["en", "de"]);
    assert_eq!(
        loaded.settings.debounce_ms,
        IdeSettings::default().debounce_ms
    );
    assert!(!loaded.settings.open_last_project_on_startup);
    assert!(loaded.zoom.fit_width);
    assert!(loaded.recent_projects.is_empty());
}

#[test]
fn test_load_persisted_state_normalizes_and_caps_recent_projects() {
    let mut storage = MemoryStorage::default();
    let entries: Vec<String> = (0..14)
        .map(|i| {
            if i == 1 {
                "\"/tmp/proj-0/\"".to_string()
            } else {
                format!("\"/tmp/proj-{i}\"")
            }
        })
        .collect();
    let ron_payload = format!("(entries:[{}])", entries.join(","));
    storage.set_string(RECENT_PROJECTS_STORAGE_KEY, ron_payload);

    let loaded = load_persisted_state(Some(&storage), &["en"]);
    assert_eq!(loaded.recent_projects.entries().len(), MAX_RECENT_PROJECTS);
    assert_eq!(
        loaded.recent_projects.most_recent(),
        Some(std::path::Path::new("/tmp/proj-0"))
    );
    assert!(loaded
        .recent_projects
        .entries()
        .contains(&PathBuf::from("/tmp/proj-10")));
    assert!(!loaded
        .recent_projects
        .entries()
        .contains(&PathBuf::from("/tmp/proj-11")));
}

#[test]
fn test_save_persisted_parts_roundtrips_recent_projects_and_startup_flag() {
    let mut storage = MemoryStorage::default();
    let settings = IdeSettings {
        open_last_project_on_startup: true,
        ..IdeSettings::default()
    };
    let recent = RecentProjects::from_paths([PathBuf::from("/workspace/alpha")]);
    save_persisted_parts(
        &mut storage,
        "en",
        &settings,
        PreviewZoom::default(),
        &recent,
    );

    let reloaded = load_persisted_state(Some(&storage), &["en"]);
    assert!(reloaded.settings.open_last_project_on_startup);
    assert_eq!(reloaded.recent_projects, recent);
}
