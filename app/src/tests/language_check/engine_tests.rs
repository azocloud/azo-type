//! Unit tests for offline spell/grammar engine tokenization, language detection, and caching.

use std::fs;

use uuid::Uuid;

use super::*;

#[test]
fn tokenizes_prose_words_preserving_apostrophes_and_skipping_numbers() {
    let text = "Typst v2026 doesn't fail on UTF-8 or 'quoted' words.";
    let tokens: Vec<&str> = tokenize_prose_words(text)
        .into_iter()
        .map(|(_, _, w)| w)
        .collect();
    assert_eq!(
        tokens,
        vec!["Typst", "doesn't", "fail", "on", "or", "quoted", "words"]
    );
}

#[test]
fn detects_language_with_precedence_override_then_typst_then_lingua() {
    let mut engine = LanguageCheckEngine::new();
    let src_de = "#set text(lang: \"de\")\nHello world in English.";

    let (lang, src_kind) =
        engine.resolve_language(src_de, "Hello world in English.", "auto", "en", &[]);
    assert_eq!(lang, "de");
    assert_eq!(src_kind, LanguageSource::TypstDirective);

    let (lang_ov, ov_kind) =
        engine.resolve_language(src_de, "Hello world in English.", "fr", "en", &[]);
    assert_eq!(lang_ov, "fr");
    assert_eq!(ov_kind, LanguageSource::ManualOverride);

    let src_no_dir = "Bonjour tout le monde, ceci est une phrase en français.";
    let (lang_auto, auto_kind) = engine.resolve_language(src_no_dir, src_no_dir, "auto", "en", &[]);
    assert_eq!(lang_auto, "fr");
    assert_eq!(auto_kind, LanguageSource::AutoDetectedLingua);
}

#[test]
fn checks_tier2_iso8859_1_dictionary_and_maps_offsets_accurately() {
    let dir = std::env::temp_dir().join(format!("azo-lang-t2-{}", Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();
    let aff = dir.join("xx.aff");
    let dic = dir.join("xx.dic");

    // ISO8859-1 dictionary containing "Titel", "café", and "bonjour"
    fs::write(&aff, b"SET ISO8859-1\n").unwrap();
    fs::write(&dic, b"3\nTitel\ncaf\xe9\nbonjour\n").unwrap();

    let entry = UserDictionaryEntry {
        lang_code: "xx".to_string(),
        label: "Custom XX".to_string(),
        aff_path: aff,
        dic_path: dic,
    };

    let mut engine = LanguageCheckEngine::new();
    let mgr = DictionaryManager::with_dir(dir.join("cache"));
    let doc = "#set text(lang: \"xx\")\n= Titel\nbonjour café mistakke\n";

    let res = engine.check_document(doc, "auto", "en", true, &[entry], &[], &mgr);
    assert_eq!(res.effective_lang, "xx");
    assert!(res.missing_tier1_pack.is_none());
    assert_eq!(res.diagnostics.len(), 1);
    let d = &res.diagnostics[0];
    assert_eq!(d.category, DiagnosticCategory::Spelling);
    assert_eq!(d.excerpt, "mistakke");
    assert_eq!(&doc[d.start_byte..d.end_byte], "mistakke");
    assert_eq!(d.line, 3);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn prompts_for_missing_tier1_pack_when_not_downloaded() {
    let dir = std::env::temp_dir().join(format!("azo-lang-missing-{}", Uuid::new_v4()));
    let mgr = DictionaryManager::with_dir(dir.clone());
    let mut engine = LanguageCheckEngine::new();

    let doc = "#set text(lang: \"de\")\nDas ist ein Test.";
    let res = engine.check_document(doc, "auto", "en", true, &[], &[], &mgr);
    assert_eq!(res.effective_lang, "de");
    assert_eq!(res.missing_tier1_pack.as_deref(), Some("de"));
    assert!(res.diagnostics.is_empty());

    let _ = fs::remove_dir_all(&dir);
}
