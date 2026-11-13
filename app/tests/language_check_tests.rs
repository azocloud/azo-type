use std::fs;

use uuid::Uuid;

use azo_type::core::settings::IdeSettings;
use azo_type::core::spellcheck::{
    dictionary_catalog, transcode_hunspell_to_utf8, DiagnosticCategory, UserDictionaryEntry,
};
use azo_type::core::typst_prose::{extract_typst_prose_mask, split_prose_paragraphs};
use azo_type::language_check::{DictionaryManager, LanguageCheckEngine, LanguageSource};
use azo_type::project::DiskProject;
use azo_type::services::language_actions::apply_language_suggestion;

#[test]
fn settings_disable_spell_and_grammar_check_by_default() {
    let settings = IdeSettings::default();
    assert!(!settings.spell_check_enabled);
    assert!(!settings.auto_spell_check);
    assert_eq!(settings.spell_check_language, "auto");
    assert_eq!(dictionary_catalog().schema_version, 1);
}

#[test]
fn end_to_end_typst_prose_masking_tier2_transcode_and_suggestion_apply() {
    let dir = std::env::temp_dir().join(format!("azo-lang-e2e-{}", Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();
    let aff = dir.join("zz.aff");
    let dic = dir.join("zz.dic");

    // Legacy ISO8859-1 dictionary where "résumé" and "important" are valid.
    fs::write(&aff, b"SET ISO8859-1\nTRY erainsotl\n").unwrap();
    fs::write(&dic, b"3\nr\xe9sum\xe9\nimportant\nThis\n").unwrap();

    let (norm_aff, norm_dic) =
        transcode_hunspell_to_utf8(&fs::read(&aff).unwrap(), &fs::read(&dic).unwrap()).unwrap();
    assert!(norm_aff.starts_with("SET UTF-8\n"));
    assert!(norm_dic.contains("résumé"));

    let doc = "#set text(lang: \"zz\")\n= #emph[résumé]\nThis is *very* #emph[importnt] $x + y$.\n";
    let prose = extract_typst_prose_mask(doc);
    assert_eq!(prose.masked_text.len(), doc.len());
    assert!(!prose.masked_text.contains("$x + y$"));
    assert!(prose.masked_text.contains("importnt"));
    assert_eq!(split_prose_paragraphs(&prose.masked_text).len(), 1);

    let entry = UserDictionaryEntry {
        lang_code: "zz".to_string(),
        label: "Custom ZZ".to_string(),
        aff_path: aff,
        dic_path: dic,
    };

    let mgr = DictionaryManager::with_dir(dir.join("cache"));
    let mut engine = LanguageCheckEngine::new();
    let ignored = vec!["is".to_string(), "very".to_string()];
    let res = engine.check_document(doc, "auto", "en", true, &[entry], &ignored, &mgr);

    assert_eq!(res.effective_lang, "zz");
    assert_eq!(res.lang_source, LanguageSource::TypstDirective);
    assert!(res.missing_tier1_pack.is_none());
    assert_eq!(res.diagnostics.len(), 1);

    let diag = &res.diagnostics[0];
    assert_eq!(diag.category, DiagnosticCategory::Spelling);
    assert_eq!(&doc[diag.start_byte..diag.end_byte], "importnt");
    assert!(diag.suggestions.iter().any(|s| s == "important"));

    let mut project = DiskProject::default();
    let active = project.active_file.clone();
    project.buffers.get_mut(&active).unwrap().text = Some(doc.to_string());
    let (updated, _caret) = apply_language_suggestion(
        &mut project,
        diag.start_byte,
        diag.end_byte,
        "important",
        false,
    )
    .unwrap();
    assert!(updated.contains("#emph[important]"));

    let _ = fs::remove_dir_all(&dir);
}
