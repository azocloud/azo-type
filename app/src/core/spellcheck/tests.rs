//! Unit tests for the embedded dictionary manifest, Hunspell encoding transcoding, and pure rules.

use super::*;

#[test]
fn embedded_dictionary_catalog_contains_all_tier1_languages_with_valid_digests() {
    let catalog = dictionary_catalog();
    assert_eq!(catalog.schema_version, 1);

    for code in TIER1_LANGUAGES {
        let (resolved_key, pack) =
            find_tier1_pack(code).expect("tier-1 language must exist in dictionaries.json");
        assert_eq!(resolved_key, code);
        assert!(!pack.files.is_empty());
        assert!(pack.total_size_bytes() > 0);
        assert!(pack.files.iter().any(|f| f.role == DictionaryFileRole::Aff));
        assert!(pack.files.iter().any(|f| f.role == DictionaryFileRole::Dic));

        for file in &pack.files {
            assert!(file.url.starts_with("https://"));
            assert_eq!(file.sha256.len(), 64);
            assert!(file.sha256.chars().all(|c| c.is_ascii_hexdigit()));
        }
    }

    assert_eq!(find_tier1_pack("de-AT").map(|(k, _)| k), Some("de"));
    assert_eq!(find_tier1_pack("en_GB").map(|(k, _)| k), Some("en"));
}

#[test]
fn transcodes_iso8859_1_hunspell_pair_and_normalizes_set_header() {
    let aff_latin1 = b"# sample\nSET ISO8859-1\nTRY abc\n";
    // "café" and "Straße" in ISO-8859-1 (0xE9 = é, 0xDF = ß)
    let dic_latin1 = b"2\ncaf\xe9\nStra\xdfe\n";

    let (aff_utf8, dic_utf8) =
        transcode_hunspell_to_utf8(aff_latin1, dic_latin1).expect("latin1 transcode must succeed");

    assert!(aff_utf8.contains("SET UTF-8"));
    assert!(!aff_utf8.contains("ISO8859-1"));
    assert!(dic_utf8.contains("café"));
    assert!(dic_utf8.contains("Straße"));
}

#[test]
fn transcodes_koi8_r_hunspell_pair() {
    let aff_koi8 = b"SET KOI8-R\n";
    // "привет" in KOI8-R: п=0xD0 р=0xD2 и=0xC9 в=0xD7 е=0xC5 т=0xD4
    let dic_koi8 = b"1\n\xd0\xd2\xc9\xd7\xc5\xd4\n";

    let (aff_utf8, dic_utf8) =
        transcode_hunspell_to_utf8(aff_koi8, dic_koi8).expect("koi8-r transcode must succeed");

    assert!(aff_utf8.starts_with("SET UTF-8"));
    assert!(dic_utf8.contains("привет"));
}

#[test]
fn rejects_unsupported_encoding_header_cleanly() {
    let aff = b"SET UNKNOWN-CHARSET-999\n";
    let dic = b"1\nword\n";
    let err = transcode_hunspell_to_utf8(aff, dic).unwrap_err();
    assert!(err.contains("Unsupported Hunspell dictionary encoding"));
}

#[test]
fn detects_repeated_words_and_french_spacing() {
    let text = "Bonjour tout le le monde!";
    let prose = vec![true; text.len()];
    let diags = check_supplementary_rules("fr", text, 0, &prose);

    assert_eq!(diags.len(), 2);
    assert_eq!(diags[0].rule_id, "GRAMMAR/REPEATED_WORD");
    assert_eq!(diags[0].suggestions, vec!["le"]);
    assert_eq!(diags[1].rule_id, "TYPO/FR_NBSP_PUNCTUATION");
}
