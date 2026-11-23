use std::sync::Mutex;

use azo_type::core::{
    detect_system_locale, locale_native_name, normalize_locale_tag, resolve_locale, AppSavedState,
};
use rust_i18n::t;

rust_i18n::i18n!("locales", fallback = "en");

static LOCALE_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn test_available_locales_contains_english_and_german() {
    let locales = rust_i18n::available_locales!();
    assert!(
        locales.contains(&"en"),
        "available locales must contain 'en'"
    );
    assert!(
        locales.contains(&"de"),
        "available locales must contain 'de'"
    );
}

#[test]
fn test_runtime_locale_switching_updates_ui_strings() {
    let _guard = LOCALE_LOCK.lock().unwrap();

    // English lookups
    rust_i18n::set_locale("en");
    assert_eq!(t!("app.title"), "Azo Type");
    assert_eq!(t!("menu.file"), "File");
    assert_eq!(t!("menu.settings"), "Settings...  (Ctrl+,)");
    assert_eq!(t!("settings.window_title"), "Settings");
    assert_eq!(t!("settings.language_label"), "Language:");
    assert_eq!(t!("export_pdf.window_title"), "Export PDF");
    assert_eq!(t!("file_modals.new_file_title"), "New File in Project");
    assert_eq!(t!("sidebar.disk_tree_title"), "DISK PROJECT TREE");

    // German lookups
    rust_i18n::set_locale("de");
    assert_eq!(t!("app.title"), "Azo Type");
    assert_eq!(t!("menu.file"), "Datei");
    assert_eq!(t!("menu.settings"), "Einstellungen...  (Strg+,)");
    assert_eq!(t!("settings.window_title"), "Einstellungen");
    assert_eq!(t!("settings.language_label"), "Sprache:");
    assert_eq!(t!("export_pdf.window_title"), "PDF exportieren");
    assert_eq!(t!("file_modals.new_file_title"), "Neue Datei im Projekt");
    assert_eq!(t!("sidebar.disk_tree_title"), "FESTPLATTEN-DATEIBAUM");

    // Reset back to English
    rust_i18n::set_locale("en");
}

#[test]
fn test_unsupported_locale_falls_back_to_english() {
    let _guard = LOCALE_LOCK.lock().unwrap();

    // Switching to an unsupported locale should fallback to "en"
    rust_i18n::set_locale("zh");
    assert_eq!(t!("menu.file"), "File");
    assert_eq!(t!("settings.language_label"), "Language:");

    rust_i18n::set_locale("en");
}

#[test]
fn test_translation_interpolation_variables() {
    let _guard = LOCALE_LOCK.lock().unwrap();

    rust_i18n::set_locale("en");
    let en_status = t!("app.status_saved_file", path = "/tmp/test.typ");
    assert_eq!(en_status, "Saved to disk: /tmp/test.typ");

    let en_pages = t!("status.page_count", count = 5);
    assert_eq!(en_pages, "Pages: 5");

    let en_scale = t!("preview.scale", percent = "120");
    assert_eq!(en_scale, "Scale: 120%");

    rust_i18n::set_locale("de");
    let de_status = t!("app.status_saved_file", path = "/tmp/test.typ");
    assert_eq!(de_status, "Auf Festplatte gespeichert: /tmp/test.typ");

    let de_pages = t!("status.page_count", count = 5);
    assert_eq!(de_pages, "Seiten: 5");

    let de_scale = t!("preview.scale", percent = "120");
    assert_eq!(de_scale, "Skalierung: 120%");

    rust_i18n::set_locale("en");
}

#[test]
fn test_app_saved_state_persistence_roundtrip() {
    let state = AppSavedState {
        language: "de".to_string(),
    };
    let serialized = serde_json::to_string(&state).expect("serialize AppSavedState");
    assert!(serialized.contains("\"language\":\"de\""));

    let deserialized: AppSavedState =
        serde_json::from_str(&serialized).expect("deserialize AppSavedState");
    assert_eq!(deserialized, state);
    assert_eq!(deserialized.language, "de");
}

#[test]
fn test_locale_normalization_and_resolution() {
    let available = ["en", "de"];

    assert_eq!(normalize_locale_tag("en-US"), "en");
    assert_eq!(normalize_locale_tag("de_DE.UTF-8"), "de");
    assert_eq!(normalize_locale_tag("FR-CA"), "fr");

    assert_eq!(resolve_locale(Some("de-CH"), &available), "de");
    assert_eq!(resolve_locale(Some("en-AU"), &available), "en");
    assert_eq!(resolve_locale(Some("es-ES"), &available), "en");
    assert_eq!(resolve_locale(None, &available), "en");

    let detected = detect_system_locale(&available);
    assert!(available.contains(&detected.as_str()));
}

#[test]
fn test_native_language_names() {
    assert_eq!(locale_native_name("en"), "English");
    assert_eq!(locale_native_name("de"), "Deutsch");
    assert_eq!(locale_native_name("fr"), "Français");
    assert_eq!(locale_native_name("unknown"), "English");
}

#[test]
fn test_edit_tools_search_and_intellisense_keys_in_en_and_de() {
    let _guard = LOCALE_LOCK.lock().unwrap();

    rust_i18n::set_locale("en");
    assert_eq!(t!("menu.find"), "Find...  (Ctrl+F)");
    assert_eq!(t!("menu.find_replace"), "Find & Replace...  (Ctrl+H)");
    assert_eq!(
        t!("menu.toggle_line_comment"),
        "Toggle Line Comment  (Ctrl+/)"
    );
    assert_eq!(
        t!("menu.toggle_block_comment"),
        "Toggle Block Comment  (Ctrl+Shift+/)"
    );
    assert_eq!(t!("menu.indent_lines"), "Indent Lines  (Ctrl+])");
    assert_eq!(t!("menu.outdent_lines"), "Outdent Lines  (Ctrl+[)");
    assert_eq!(t!("menu.duplicate_line"), "Duplicate Line  (Ctrl+D)");
    assert_eq!(t!("menu.move_line_up"), "Move Line Up  (Alt+Up)");
    assert_eq!(t!("menu.move_line_down"), "Move Line Down  (Alt+Down)");

    assert_eq!(t!("bottom_panel.tab_search_replace"), "Search & Replace");
    assert_eq!(t!("bottom_panel.search_label"), "Find:");
    assert_eq!(
        t!("bottom_panel.search_placeholder"),
        "Search in active file..."
    );
    assert_eq!(t!("bottom_panel.replace_label"), "Replace:");
    assert_eq!(
        t!("bottom_panel.replace_placeholder"),
        "Replacement text..."
    );
    assert_eq!(t!("bottom_panel.match_case"), "Match Case");
    assert_eq!(t!("bottom_panel.prev_match_btn"), "Prev");
    assert_eq!(t!("bottom_panel.next_match_btn"), "Next");
    assert_eq!(t!("bottom_panel.replace_btn"), "Replace");
    assert_eq!(t!("bottom_panel.replace_all_btn"), "Replace All");
    assert_eq!(
        t!("bottom_panel.matches_count", current = 2, total = 5),
        "2 of 5"
    );
    assert_eq!(t!("bottom_panel.no_matches"), "No matches");
    assert_eq!(
        t!("bottom_panel.replaced_all_status", count = 3),
        "Replaced 3 occurrence(s)"
    );

    assert_eq!(t!("intellisense.suggestions_header"), "Typst IntelliSense");
    assert_eq!(t!("intellisense.no_docs"), "No documentation available.");
    assert_eq!(
        t!("intellisense.hint_accept"),
        "Tab / Enter to accept  •  Esc to close"
    );
    assert_eq!(t!("intellisense.diagnostic_badge"), "HINT");
    assert_eq!(
        t!("intellisense.unresolved_ref", name = "sec-intro"),
        "Unresolved reference '@sec-intro'"
    );

    rust_i18n::set_locale("de");
    assert_eq!(t!("menu.find"), "Suchen...  (Strg+F)");
    assert_eq!(t!("menu.find_replace"), "Suchen & Ersetzen...  (Strg+H)");
    assert_eq!(
        t!("menu.toggle_line_comment"),
        "Zeilenkommentar umschalten  (Strg+/)"
    );
    assert_eq!(
        t!("menu.toggle_block_comment"),
        "Blockkommentar umschalten  (Strg+Umschalt+/)"
    );
    assert_eq!(t!("menu.indent_lines"), "Zeilen einrücken  (Strg+])");
    assert_eq!(t!("menu.outdent_lines"), "Zeilen ausrücken  (Strg+[)");
    assert_eq!(t!("menu.duplicate_line"), "Zeile duplizieren  (Strg+D)");
    assert_eq!(
        t!("menu.move_line_up"),
        "Zeile nach oben verschieben  (Alt+Hoch)"
    );
    assert_eq!(
        t!("menu.move_line_down"),
        "Zeile nach unten verschieben  (Alt+Runter)"
    );

    assert_eq!(t!("bottom_panel.tab_search_replace"), "Suchen & Ersetzen");
    assert_eq!(t!("bottom_panel.search_label"), "Suchen:");
    assert_eq!(
        t!("bottom_panel.search_placeholder"),
        "In aktiver Datei suchen..."
    );
    assert_eq!(t!("bottom_panel.replace_label"), "Ersetzen:");
    assert_eq!(t!("bottom_panel.replace_placeholder"), "Ersatztext...");
    assert_eq!(t!("bottom_panel.match_case"), "Groß-/Kleinschreibung");
    assert_eq!(t!("bottom_panel.prev_match_btn"), "Zurück");
    assert_eq!(t!("bottom_panel.next_match_btn"), "Weiter");
    assert_eq!(t!("bottom_panel.replace_btn"), "Ersetzen");
    assert_eq!(t!("bottom_panel.replace_all_btn"), "Alle ersetzen");
    assert_eq!(
        t!("bottom_panel.matches_count", current = 2, total = 5),
        "2 von 5"
    );
    assert_eq!(t!("bottom_panel.no_matches"), "Keine Treffer");
    assert_eq!(
        t!("bottom_panel.replaced_all_status", count = 3),
        "3 Vorkommen ersetzt"
    );

    assert_eq!(t!("intellisense.suggestions_header"), "Typst-IntelliSense");
    assert_eq!(t!("intellisense.no_docs"), "Keine Dokumentation verfügbar.");
    assert_eq!(
        t!("intellisense.hint_accept"),
        "Tab / Eingabe zum Übernehmen  •  Esc zum Schließen"
    );
    assert_eq!(t!("intellisense.diagnostic_badge"), "HINWEIS");
    assert_eq!(
        t!("intellisense.unresolved_ref", name = "sec-intro"),
        "Unaufgelöste Referenz '@sec-intro'"
    );

    rust_i18n::set_locale("en");
}
