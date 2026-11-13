//! Persistent application preferences, language, and preview zoom backed by `eframe::Storage`.

#[cfg(test)]
#[path = "../tests/services/persistence_tests.rs"]
mod tests;

use crate::core::i18n::{detect_system_locale, AppSavedState};
use crate::core::settings::{IdeSettings, PreviewZoom};

/// Storage key used to persist [`IdeSettings`] inside `eframe`'s native store.
pub const SETTINGS_STORAGE_KEY: &str = "ide_settings";

/// Storage key used to persist [`PreviewZoom`] inside `eframe`'s native store.
pub const PREVIEW_ZOOM_STORAGE_KEY: &str = "preview_zoom";

/// Snapshot of user-modifiable persistent state tracked for immediate flush on change.
#[derive(Debug, Clone, PartialEq)]
pub struct PersistedAppState {
    pub language: String,
    pub settings: IdeSettings,
    pub zoom: PreviewZoom,
}

/// Loads persisted language, IDE settings, and preview zoom from `eframe::Storage`.
///
/// Missing keys fall back to OS-detected locale and default settings cleanly.
pub fn load_persisted_state(
    storage: Option<&dyn eframe::Storage>,
    available_locales: &[&str],
) -> PersistedAppState {
    let language = storage
        .and_then(|s| eframe::get_value::<AppSavedState>(s, eframe::APP_KEY))
        .map(|st| st.language)
        .unwrap_or_else(|| detect_system_locale(available_locales));

    let settings = storage
        .and_then(|s| eframe::get_value::<IdeSettings>(s, SETTINGS_STORAGE_KEY))
        .unwrap_or_default()
        .sanitized();

    let zoom = storage
        .and_then(|s| eframe::get_value::<PreviewZoom>(s, PREVIEW_ZOOM_STORAGE_KEY))
        .unwrap_or_default();

    PersistedAppState {
        language,
        settings,
        zoom,
    }
}

/// Writes the current language, IDE settings, and preview zoom into `eframe::Storage`.
pub fn save_persisted_state(storage: &mut dyn eframe::Storage, state: &PersistedAppState) {
    save_persisted_parts(storage, &state.language, &state.settings, state.zoom);
}

/// Writes individual state parts into `eframe::Storage`.
pub fn save_persisted_parts(
    storage: &mut dyn eframe::Storage,
    language: &str,
    settings: &IdeSettings,
    zoom: PreviewZoom,
) {
    eframe::set_value(
        storage,
        eframe::APP_KEY,
        &AppSavedState {
            language: language.to_string(),
        },
    );
    eframe::set_value(storage, SETTINGS_STORAGE_KEY, settings);
    eframe::set_value(storage, PREVIEW_ZOOM_STORAGE_KEY, &zoom);
}

/// Flushes updated preferences to disk immediately when any persisted field changes.
pub fn flush_if_changed(
    frame: &mut eframe::Frame,
    last_saved: &mut PersistedAppState,
    language: &str,
    settings: &IdeSettings,
    zoom: PreviewZoom,
) {
    let zoom_changed = last_saved.zoom.fit_width != zoom.fit_width
        || (last_saved.zoom.scale - zoom.scale).abs() > 1e-4;
    if last_saved.language == language && last_saved.settings == *settings && !zoom_changed {
        return;
    }

    let next = PersistedAppState {
        language: language.to_string(),
        settings: settings.clone(),
        zoom,
    };
    if let Some(storage) = frame.storage_mut() {
        save_persisted_state(storage, &next);
        storage.flush();
    }
    *last_saved = next;
}
