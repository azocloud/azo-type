//! Unit tests for Hunspell dictionary companion file resolution.

use std::fs;

use uuid::Uuid;

use super::resolve_dictionary_pair;

#[test]
fn resolves_companion_aff_when_dic_is_picked() {
    let dir = std::env::temp_dir().join(format!("azo-dict-pair-{}", Uuid::new_v4()));
    fs::create_dir_all(&dir).expect("create temp dir");
    let aff = dir.join("pt_BR.aff");
    let dic = dir.join("pt_BR.dic");
    fs::write(&aff, b"SET UTF-8\n").unwrap();
    fs::write(&dic, b"1\nola\n").unwrap();

    let entry = resolve_dictionary_pair(&dic).expect("should find companion .aff");
    assert_eq!(entry.lang_code, "pt_br");
    assert_eq!(entry.label, "pt_BR");
    assert_eq!(entry.aff_path, aff);
    assert_eq!(entry.dic_path, dic);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn rejects_missing_companion_file_with_descriptive_error() {
    let dir = std::env::temp_dir().join(format!("azo-dict-miss-{}", Uuid::new_v4()));
    fs::create_dir_all(&dir).expect("create temp dir");
    let aff = dir.join("nl_NL.aff");
    fs::write(&aff, b"SET UTF-8\n").unwrap();

    let err = resolve_dictionary_pair(&aff).unwrap_err();
    assert!(err.contains("Companion .dic file not found"));

    let _ = fs::remove_dir_all(&dir);
}
