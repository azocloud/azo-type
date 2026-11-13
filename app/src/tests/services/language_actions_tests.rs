//! Unit tests for spell/grammar suggestion application and Tier-2 dictionary registration.

use std::path::PathBuf;

use super::*;

#[test]
fn applies_language_suggestion_preserving_utf8_boundaries() {
    let mut project = DiskProject::default();
    let active = project.active_file.clone();
    project.buffers.get_mut(&active).unwrap().text = Some("Grüß mispeled Wört.".to_string());

    let start = "Grüß ".len();
    let end = start + "mispeled".len();
    let (updated, caret) =
        apply_language_suggestion(&mut project, start, end, "misspelled", false).unwrap();

    assert_eq!(updated, "Grüß misspelled Wört.");
    assert_eq!(caret, "Grüß misspelled".chars().count());
    assert!(project.buffers.get(&active).unwrap().dirty);

    // Out-of-bounds or mid-codepoint offsets (byte 3 splits 'ü') must safely return None.
    assert!(apply_language_suggestion(&mut project, 0, 3, "x", false).is_none());
    assert!(apply_language_suggestion(&mut project, 0, 999, "x", false).is_none());
}

#[test]
fn registers_and_replaces_custom_dictionary_by_lang_code() {
    let mut settings = IdeSettings::default();
    register_user_dictionary(
        &mut settings,
        UserDictionaryEntry {
            lang_code: "pt".to_string(),
            label: "pt_PT".to_string(),
            aff_path: PathBuf::from("/tmp/a.aff"),
            dic_path: PathBuf::from("/tmp/a.dic"),
        },
    );
    register_user_dictionary(
        &mut settings,
        UserDictionaryEntry {
            lang_code: "pt".to_string(),
            label: "pt_BR".to_string(),
            aff_path: PathBuf::from("/tmp/b.aff"),
            dic_path: PathBuf::from("/tmp/b.dic"),
        },
    );
    assert_eq!(settings.user_dictionaries.len(), 1);
    assert_eq!(settings.user_dictionaries[0].label, "pt_BR");
}
