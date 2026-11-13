//! Offline spellcheck (`spellbook`), grammar (`nlprule`), and language detection (`lingua`)
//! engine with paragraph-level hash caching and 1:1 Typst source offset mapping.

#[cfg(test)]
mod tests;

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use lingua::{Language, LanguageDetector, LanguageDetectorBuilder};
use nlprule::{Rules, Tokenizer};
use spellbook::Dictionary;

use crate::core::spellcheck::{
    check_supplementary_rules, find_tier1_pack, transcode_hunspell_to_utf8, DiagnosticCategory,
    LanguageCheckDiagnostic, UserDictionaryEntry,
};
use crate::core::typst_prose::{
    detect_typst_document_language, extract_typst_prose_mask, normalize_language_code,
    span_touches_masked_syntax, split_prose_paragraphs,
};
use crate::language_check::downloader::DictionaryManager;
pub use crate::language_check::tokenize::tokenize_prose_words;
use crate::language_check::tokenize::SourceOffsetMapper;

const MAX_PARAGRAPH_CACHE_ENTRIES: usize = 2048;
const MAX_SPELLING_SUGGESTIONS_PER_PARAGRAPH: usize = 120;

/// Origin of the resolved document checking language.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageSource {
    ManualOverride,
    TypstDirective,
    AutoDetectedLingua,
    DefaultFallback,
}

/// Complete outcome of checking a single document snapshot.
#[derive(Debug, Clone)]
pub struct DocumentCheckResult {
    pub effective_lang: String,
    pub lang_source: LanguageSource,
    pub missing_tier1_pack: Option<String>,
    pub diagnostics: Vec<LanguageCheckDiagnostic>,
    pub duration_ms: f64,
    pub warning: Option<String>,
}

#[derive(Debug, Clone)]
struct CachedDiag {
    category: DiagnosticCategory,
    rel_start: usize,
    rel_end: usize,
    excerpt: String,
    message: String,
    rule_id: String,
    suggestions: Vec<String>,
}

struct LoadedTier1Engine {
    dict: Dictionary,
    grammar: Option<(Tokenizer, Rules)>,
    grammar_load_warning: Option<String>,
}

struct LoadedTier2Engine {
    dict: Dictionary,
    aff_path: std::path::PathBuf,
    dic_path: std::path::PathBuf,
}

enum ActiveBackend<'a> {
    Tier1 {
        lang: &'a str,
        engine: &'a LoadedTier1Engine,
    },
    Tier2 {
        lang: &'a str,
        engine: &'a LoadedTier2Engine,
    },
}

/// Stateful offline checking engine running inside the background worker thread.
pub struct LanguageCheckEngine {
    detector: Option<LanguageDetector>,
    tier1_cache: HashMap<String, LoadedTier1Engine>,
    tier2_cache: HashMap<String, LoadedTier2Engine>,
    paragraph_cache: HashMap<(String, u64), Vec<CachedDiag>>,
}

impl Default for LanguageCheckEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageCheckEngine {
    pub fn new() -> Self {
        Self {
            detector: None,
            tier1_cache: HashMap::new(),
            tier2_cache: HashMap::new(),
            paragraph_cache: HashMap::new(),
        }
    }

    /// Drops cached dictionaries and paragraph results (called after downloading/removing packs).
    pub fn invalidate_dictionaries(&mut self) {
        self.tier1_cache.clear();
        self.tier2_cache.clear();
        self.paragraph_cache.clear();
    }

    fn ensure_detector(&mut self) -> &LanguageDetector {
        self.detector.get_or_insert_with(|| {
            LanguageDetectorBuilder::from_languages(&[
                Language::English,
                Language::German,
                Language::French,
                Language::Spanish,
            ])
            .build()
        })
    }

    /// Resolves which language tag should be used for `source` given settings and prose text.
    pub fn resolve_language(
        &mut self,
        source: &str,
        masked_prose: &str,
        preferred_setting: &str,
        fallback_ui_lang: &str,
        user_dictionaries: &[UserDictionaryEntry],
    ) -> (String, LanguageSource) {
        let pref = preferred_setting.trim();
        if !pref.is_empty() && !pref.eq_ignore_ascii_case("auto") {
            return (pref.to_ascii_lowercase(), LanguageSource::ManualOverride);
        }

        if let Some(directive_tag) = detect_typst_document_language(source) {
            if find_user_dict(user_dictionaries, &directive_tag).is_some()
                || find_tier1_pack(&directive_tag).is_some()
            {
                return (directive_tag, LanguageSource::TypstDirective);
            }
            if let Some(primary) = normalize_language_code(&directive_tag) {
                return (primary, LanguageSource::TypstDirective);
            }
        }

        let trimmed_prose = masked_prose.trim();
        if trimmed_prose.chars().filter(|c| c.is_alphabetic()).count() >= 12 {
            if let Some(detected) = self.ensure_detector().detect_language_of(trimmed_prose) {
                let code = match detected {
                    Language::English => "en",
                    Language::German => "de",
                    Language::French => "fr",
                    Language::Spanish => "es",
                };
                return (code.to_string(), LanguageSource::AutoDetectedLingua);
            }
        }

        let ui_primary = normalize_language_code(fallback_ui_lang)
            .filter(|c| find_tier1_pack(c).is_some())
            .unwrap_or_else(|| "en".to_string());
        (ui_primary, LanguageSource::DefaultFallback)
    }

    /// Runs offline spell and grammar checking across `source`.
    pub fn check_document(
        &mut self,
        source: &str,
        preferred_setting: &str,
        fallback_ui_lang: &str,
        grammar_enabled: bool,
        user_dictionaries: &[UserDictionaryEntry],
        ignored_words: &[String],
        dict_manager: &DictionaryManager,
    ) -> DocumentCheckResult {
        let started = std::time::Instant::now();
        let mask = extract_typst_prose_mask(source);
        let (resolved_lang, lang_source) = self.resolve_language(
            source,
            &mask.masked_text,
            preferred_setting,
            fallback_ui_lang,
            user_dictionaries,
        );

        let ignored_set: HashSet<&str> = ignored_words.iter().map(String::as_str).collect();

        // Check Tier-2 user dictionary first if explicitly matched (supports e.g. custom `pt_br` or `en_gb`).
        if let Some(user_entry) = find_user_dict(user_dictionaries, &resolved_lang).cloned() {
            if let Err(err) = self.ensure_tier2_loaded(&user_entry) {
                return DocumentCheckResult {
                    effective_lang: resolved_lang,
                    lang_source,
                    missing_tier1_pack: None,
                    diagnostics: Vec::new(),
                    duration_ms: started.elapsed().as_secs_f64() * 1000.0,
                    warning: Some(err),
                };
            }
            let lang_key = user_entry.lang_code.clone();
            let diags = self.run_paragraphs(
                source,
                &mask.masked_text,
                &mask.prose_bytes,
                &lang_key,
                false,
                &ignored_set,
                true,
            );
            return DocumentCheckResult {
                effective_lang: resolved_lang,
                lang_source,
                missing_tier1_pack: None,
                diagnostics: diags,
                duration_ms: started.elapsed().as_secs_f64() * 1000.0,
                warning: None,
            };
        }

        // Otherwise check Tier-1 manifest.
        let Some((tier1_code, _pack)) = find_tier1_pack(&resolved_lang) else {
            return DocumentCheckResult {
                effective_lang: resolved_lang.clone(),
                lang_source,
                missing_tier1_pack: None,
                diagnostics: Vec::new(),
                duration_ms: started.elapsed().as_secs_f64() * 1000.0,
                warning: Some(format!(
                    "No dictionary installed for language '{resolved_lang}'. Add a .dic/.aff dictionary in Settings."
                )),
            };
        };

        if dict_manager.installed_paths(tier1_code).is_none() {
            return DocumentCheckResult {
                effective_lang: tier1_code.to_string(),
                lang_source,
                missing_tier1_pack: Some(tier1_code.to_string()),
                diagnostics: Vec::new(),
                duration_ms: started.elapsed().as_secs_f64() * 1000.0,
                warning: None,
            };
        }

        if let Err(err) = self.ensure_tier1_loaded(tier1_code, dict_manager) {
            return DocumentCheckResult {
                effective_lang: tier1_code.to_string(),
                lang_source,
                missing_tier1_pack: None,
                diagnostics: Vec::new(),
                duration_ms: started.elapsed().as_secs_f64() * 1000.0,
                warning: Some(err),
            };
        }

        let warning = self
            .tier1_cache
            .get(tier1_code)
            .and_then(|e| e.grammar_load_warning.clone());

        let diags = self.run_paragraphs(
            source,
            &mask.masked_text,
            &mask.prose_bytes,
            tier1_code,
            grammar_enabled,
            &ignored_set,
            false,
        );

        DocumentCheckResult {
            effective_lang: tier1_code.to_string(),
            lang_source,
            missing_tier1_pack: None,
            diagnostics: diags,
            duration_ms: started.elapsed().as_secs_f64() * 1000.0,
            warning,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn run_paragraphs(
        &mut self,
        source: &str,
        masked_text: &str,
        prose_bytes: &[bool],
        lang_key: &str,
        grammar_enabled: bool,
        ignored_words: &HashSet<&str>,
        is_tier2: bool,
    ) -> Vec<LanguageCheckDiagnostic> {
        let paragraphs = split_prose_paragraphs(masked_text);
        if paragraphs.is_empty() {
            return Vec::new();
        }

        let mapper = SourceOffsetMapper::new(source);
        let cache_ns = format!(
            "{}:{}:g{}",
            if is_tier2 { "t2" } else { "t1" },
            lang_key,
            u8::from(grammar_enabled)
        );
        let mut results = Vec::new();

        for para in paragraphs {
            let key = (cache_ns.clone(), para.hash);
            let cached = if let Some(hit) = self.paragraph_cache.get(&key) {
                hit.clone()
            } else {
                let computed = if is_tier2 {
                    let Some(t2) = self.tier2_cache.get(lang_key) else {
                        continue;
                    };
                    check_paragraph_with_backend(
                        &para.text,
                        ActiveBackend::Tier2 {
                            lang: lang_key,
                            engine: t2,
                        },
                        false,
                    )
                } else {
                    let Some(t1) = self.tier1_cache.get(lang_key) else {
                        continue;
                    };
                    check_paragraph_with_backend(
                        &para.text,
                        ActiveBackend::Tier1 {
                            lang: lang_key,
                            engine: t1,
                        },
                        grammar_enabled,
                    )
                };
                if self.paragraph_cache.len() >= MAX_PARAGRAPH_CACHE_ENTRIES {
                    self.paragraph_cache.clear();
                }
                self.paragraph_cache.insert(key, computed.clone());
                computed
            };

            for item in cached {
                let abs_start = para.start_byte + item.rel_start;
                let abs_end = para.start_byte + item.rel_end;
                if abs_end > source.len() || abs_start >= abs_end {
                    continue;
                }
                if item.category == DiagnosticCategory::Spelling
                    && ignored_words.contains(item.excerpt.as_str())
                {
                    continue;
                }
                if item.rule_id.contains("WHITESPACE_RULE")
                    && span_touches_masked_syntax(prose_bytes, abs_start, abs_end)
                {
                    continue;
                }
                if item.rule_id == "GRAMMAR/REPEATED_WORD"
                    && span_touches_masked_syntax(prose_bytes, abs_start, abs_end)
                {
                    continue;
                }
                let (start_char, end_char, line, column) = mapper.map_range(abs_start, abs_end);
                results.push(LanguageCheckDiagnostic {
                    category: item.category,
                    start_byte: abs_start,
                    end_byte: abs_end,
                    start_char,
                    end_char,
                    line,
                    column,
                    excerpt: source[abs_start..abs_end].to_string(),
                    message: item.message,
                    rule_id: item.rule_id,
                    suggestions: item.suggestions,
                });
            }
        }

        results.sort_by_key(|d| (d.start_byte, d.end_byte));
        results.dedup_by(|a, b| {
            a.start_byte == b.start_byte && a.end_byte == b.end_byte && a.category == b.category
        });
        results
    }

    fn ensure_tier1_loaded(
        &mut self,
        lang: &str,
        dict_manager: &DictionaryManager,
    ) -> Result<(), String> {
        if self.tier1_cache.contains_key(lang) {
            return Ok(());
        }
        let paths = dict_manager
            .installed_paths(lang)
            .ok_or_else(|| format!("Language pack '{lang}' is not installed."))?;
        let dict = load_hunspell_dictionary(&paths.aff_path, &paths.dic_path)?;

        let (grammar, grammar_load_warning) = match (paths.tokenizer_path, paths.rules_path) {
            (Some(tok_p), Some(rules_p)) => match (Tokenizer::new(&tok_p), Rules::new(&rules_p)) {
                (Ok(tok), Ok(rules)) => (Some((tok, rules)), None),
                (Err(e), _) | (_, Err(e)) => (
                    None,
                    Some(format!(
                        "Loaded '{lang}' dictionary, but failed loading nlprule grammar model: {e}"
                    )),
                ),
            },
            _ => (None, None),
        };

        self.tier1_cache.insert(
            lang.to_string(),
            LoadedTier1Engine {
                dict,
                grammar,
                grammar_load_warning,
            },
        );
        Ok(())
    }

    fn ensure_tier2_loaded(&mut self, entry: &UserDictionaryEntry) -> Result<(), String> {
        if let Some(existing) = self.tier2_cache.get(&entry.lang_code) {
            if existing.aff_path == entry.aff_path && existing.dic_path == entry.dic_path {
                return Ok(());
            }
        }
        let dict = load_hunspell_dictionary(&entry.aff_path, &entry.dic_path)?;
        self.tier2_cache.insert(
            entry.lang_code.clone(),
            LoadedTier2Engine {
                dict,
                aff_path: entry.aff_path.clone(),
                dic_path: entry.dic_path.clone(),
            },
        );
        Ok(())
    }
}

fn load_hunspell_dictionary(aff_path: &Path, dic_path: &Path) -> Result<Dictionary, String> {
    let aff_bytes = fs::read(aff_path)
        .map_err(|e| format!("Failed reading affix file '{}': {e}", aff_path.display()))?;
    let dic_bytes = fs::read(dic_path).map_err(|e| {
        format!(
            "Failed reading dictionary file '{}': {e}",
            dic_path.display()
        )
    })?;
    let (aff_utf8, dic_utf8) = transcode_hunspell_to_utf8(&aff_bytes, &dic_bytes)?;
    Dictionary::new(&aff_utf8, &dic_utf8).map_err(|e| {
        format!(
            "Failed parsing Hunspell dictionary '{}': {e}",
            dic_path.display()
        )
    })
}

fn find_user_dict<'a>(
    entries: &'a [UserDictionaryEntry],
    lang_tag: &str,
) -> Option<&'a UserDictionaryEntry> {
    let norm = lang_tag.trim().replace('-', "_").to_ascii_lowercase();
    if let Some(exact) = entries.iter().find(|e| e.lang_code == norm) {
        return Some(exact);
    }
    let primary = normalize_language_code(&norm)?;
    entries
        .iter()
        .find(|e| normalize_language_code(&e.lang_code).as_deref() == Some(primary.as_str()))
}

fn check_paragraph_with_backend(
    paragraph: &str,
    backend: ActiveBackend<'_>,
    grammar_enabled: bool,
) -> Vec<CachedDiag> {
    let mut out = Vec::new();
    let (dict, grammar_opt, lang_code) = match backend {
        ActiveBackend::Tier1 { lang, engine } => (&engine.dict, engine.grammar.as_ref(), lang),
        ActiveBackend::Tier2 { lang, engine } => (&engine.dict, None, lang),
    };

    // 1. Spellchecking via `spellbook`
    let mut suggestion_budget = MAX_SPELLING_SUGGESTIONS_PER_PARAGRAPH;
    for (rel_start, rel_end, token) in tokenize_prose_words(paragraph) {
        let normalized = token.replace('\u{2019}', "'");
        if dict.check(token) || (normalized != token && dict.check(&normalized)) {
            continue;
        }
        let mut suggestions = Vec::new();
        if suggestion_budget > 0 {
            suggestion_budget -= 1;
            dict.suggest(&normalized, &mut suggestions);
            suggestions.truncate(5);
        }
        out.push(CachedDiag {
            category: DiagnosticCategory::Spelling,
            rel_start,
            rel_end,
            excerpt: token.to_string(),
            message: format!("Unknown word '{token}'."),
            rule_id: "SPELLBOOK".to_string(),
            suggestions,
        });
    }

    // 2. Grammar checking via `nlprule` (when enabled and loaded for Tier-1)
    if grammar_enabled {
        if let Some((tokenizer, rules)) = grammar_opt {
            for sug in rules.suggest(paragraph, tokenizer) {
                let source_id = sug.source();
                if source_id.contains("SPELLER") || source_id.contains("MORFOLOGIK") {
                    continue;
                }
                let span = sug.span().byte();
                if span.start >= span.end || span.end > paragraph.len() {
                    continue;
                }
                let excerpt = paragraph.get(span.clone()).unwrap_or_default().to_string();
                let replacements: Vec<String> = sug
                    .replacements()
                    .iter()
                    .take(5)
                    .map(|s| s.to_string())
                    .collect();
                out.push(CachedDiag {
                    category: DiagnosticCategory::Grammar,
                    rel_start: span.start,
                    rel_end: span.end,
                    excerpt,
                    message: sug.message().to_string(),
                    rule_id: source_id.to_string(),
                    suggestions: replacements,
                });
            }
        }

        // 3. Pure-Rust supplementary rules (repeated words + French punctuation spacing)
        let dummy_prose = vec![true; paragraph.len()];
        for diag in check_supplementary_rules(lang_code, paragraph, 0, &dummy_prose) {
            out.push(CachedDiag {
                category: diag.category,
                rel_start: diag.start_byte,
                rel_end: diag.end_byte,
                excerpt: diag.excerpt,
                message: diag.message,
                rule_id: diag.rule_id,
                suggestions: diag.suggestions,
            });
        }
    }

    out
}
