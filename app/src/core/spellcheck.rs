//! Pure spellcheck/grammar domain types, embedded dictionary catalog, Hunspell encoding
//! transcoding, and supplementary pure-Rust typography/repetition checks.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::OnceLock;

use encoding_rs::{Encoding, UTF_8};

use crate::core::typst_prose::{normalize_language_code, span_touches_masked_syntax};

#[cfg(test)]
#[path = "../tests/core/spellcheck_tests.rs"]
mod tests;

const DICTIONARIES_JSON: &str = include_str!("../../assets/dictionaries.json");

/// Tier-1 language codes with first-party dictionary manifest entries.
pub const TIER1_LANGUAGES: [&str; 4] = ["en", "de", "fr", "es"];

/// Role of an individual downloadable file inside a Tier-1 language pack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DictionaryFileRole {
    /// Hunspell affix rules file (`.aff`).
    Aff,
    /// Hunspell dictionary word list (`.dic`).
    Dic,
    /// `nlprule` tokenizer binary.
    NlpruleTokenizer,
    /// `nlprule` grammar rules binary.
    NlpruleRules,
}

/// Compression encoding applied to a downloadable dictionary or grammar asset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetCompression {
    /// Uncompressed raw file.
    None,
    /// Gzip-compressed binary (`.gz`), decompressed to `filename` after SHA-256 verification.
    Gzip,
}

/// Single downloadable file metadata inside a Tier-1 language pack.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct DictionaryFileAsset {
    /// Semantic role within the language pipeline.
    pub role: DictionaryFileRole,
    /// Target filename stored on disk inside the language cache folder.
    pub filename: String,
    /// Wire compression format before extraction.
    pub compression: AssetCompression,
    /// Direct HTTPS download URL.
    pub url: String,
    /// Expected lowercase 64-character SHA-256 hex digest of the downloaded payload.
    pub sha256: String,
    /// Expected downloaded payload size in bytes.
    pub size_bytes: u64,
}

/// Downloadable pack definition for a Tier-1 supported language.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct Tier1LanguagePack {
    /// Full locale tag (e.g. `"en_US"`, `"de_DE"`).
    pub locale_tag: String,
    /// Human-readable native display label.
    pub display_name: String,
    /// Upstream dictionary snapshot version identifier.
    pub version: String,
    /// Whether this language ships offline `nlprule` grammar binaries.
    pub has_grammar_model: bool,
    /// Ordered list of files required by this language pack.
    pub files: Vec<DictionaryFileAsset>,
}

impl Tier1LanguagePack {
    /// Sum of all compressed download sizes in bytes for this language pack.
    pub fn total_size_bytes(&self) -> u64 {
        self.files.iter().map(|f| f.size_bytes).sum()
    }
}

/// Root embedded dictionary manifest loaded from `app/assets/dictionaries.json`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct DictionaryCatalog {
    /// Manifest format version (must be `1`).
    pub schema_version: u32,
    /// Map from primary ISO 639-1 language code (`"en"`, `"de"`, `"fr"`, `"es"`) to pack data.
    pub dictionaries: BTreeMap<String, Tier1LanguagePack>,
}

static DICTIONARY_CATALOG_CACHE: OnceLock<DictionaryCatalog> = OnceLock::new();

/// Returns the embedded Tier-1 dictionary catalog parsed from `app/assets/dictionaries.json`.
pub fn dictionary_catalog() -> &'static DictionaryCatalog {
    DICTIONARY_CATALOG_CACHE.get_or_init(|| {
        let catalog: DictionaryCatalog = serde_json::from_str(DICTIONARIES_JSON)
            .expect("embedded dictionaries.json must be valid JSON");
        assert_eq!(
            catalog.schema_version, 1,
            "unsupported dictionaries.json schema_version"
        );
        catalog
    })
}

/// Looks up a Tier-1 language pack by language or locale tag (e.g. `"de-DE"` -> `("de", pack)`).
pub fn find_tier1_pack(lang_tag: &str) -> Option<(&'static str, &'static Tier1LanguagePack)> {
    let primary = normalize_language_code(lang_tag)?;
    let catalog = dictionary_catalog();
    catalog
        .dictionaries
        .get_key_value(&primary)
        .map(|(k, v)| (k.as_str(), v))
}

/// Category of an offline language diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticCategory {
    /// Unknown or misspelled word reported by `spellbook`.
    Spelling,
    /// Grammatical error reported by `nlprule` or rule checks.
    Grammar,
    /// Spacing, punctuation, or typographic suggestion.
    Typography,
}

/// Single spelling, grammar, or typography diagnostic anchored to a source buffer span.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageCheckDiagnostic {
    /// Diagnostic classification determining editor underline styling.
    pub category: DiagnosticCategory,
    /// UTF-8 byte start offset in the source document.
    pub start_byte: usize,
    /// UTF-8 byte end offset in the source document.
    pub end_byte: usize,
    /// Unicode scalar (`char`) start offset for editor cursor mapping.
    pub start_char: usize,
    /// Unicode scalar (`char`) end offset for editor cursor mapping.
    pub end_char: usize,
    /// 1-based source line number.
    pub line: usize,
    /// 1-based source column (in Unicode scalar characters).
    pub column: usize,
    /// Substring from the source document highlighted by this diagnostic.
    pub excerpt: String,
    /// Human-readable explanation shown in tooltips and the bottom panel.
    pub message: String,
    /// Rule identifier (e.g. `"SPELLBOOK"`, `"GRAMMAR/..."`, `"TYPO/FR_SPACE"`).
    pub rule_id: String,
    /// Ordered replacement suggestions, if available.
    pub suggestions: Vec<String>,
}

/// User-installed Tier-2 Hunspell dictionary registered in settings.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct UserDictionaryEntry {
    /// Normalized ISO language/locale tag (e.g. `"pt_br"`, `"nl"`, `"it"`).
    pub lang_code: String,
    /// User-facing label shown in settings.
    pub label: String,
    /// Filesystem path to the `.aff` affix file.
    pub aff_path: PathBuf,
    /// Filesystem path to the `.dic` dictionary word list file.
    pub dic_path: PathBuf,
}

/// Resolves the `encoding_rs::Encoding` corresponding to a Hunspell `SET` declaration.
pub fn resolve_hunspell_encoding(raw_label: &str) -> Option<&'static Encoding> {
    let trimmed = raw_label.trim();
    if trimmed.is_empty() {
        return None;
    }
    let upper = trimmed.to_ascii_uppercase();
    if upper == "UTF-8" || upper == "UTF8" {
        return Some(UTF_8);
    }
    if let Some(rest) = upper.strip_prefix("ISO8859-") {
        let hyphenated = format!("iso-8859-{rest}");
        if let Some(enc) = Encoding::for_label(hyphenated.as_bytes()) {
            return Some(enc);
        }
    }
    if let Some(rest) = upper.strip_prefix("MICROSOFT-CP") {
        let win = format!("windows-{rest}");
        if let Some(enc) = Encoding::for_label(win.as_bytes()) {
            return Some(enc);
        }
    }
    if let Some(rest) = upper.strip_prefix("CP") {
        if rest.chars().all(|c| c.is_ascii_digit()) {
            let win = format!("windows-{rest}");
            if let Some(enc) = Encoding::for_label(win.as_bytes()) {
                return Some(enc);
            }
        }
    }
    Encoding::for_label(trimmed.as_bytes())
}

/// Extracts the declared `SET <encoding>` value from raw `.aff` header bytes.
pub fn detect_aff_set_encoding(aff_bytes: &[u8]) -> Option<String> {
    let prefix_len = aff_bytes.len().min(8192);
    let head = String::from_utf8_lossy(&aff_bytes[..prefix_len]);
    for line in head.lines().take(128) {
        let stripped = line.trim_start_matches('\u{feff}').trim();
        if stripped.is_empty() || stripped.starts_with('#') {
            continue;
        }
        let mut parts = stripped.split_whitespace();
        if let (Some(cmd), Some(enc)) = (parts.next(), parts.next()) {
            if cmd.eq_ignore_ascii_case("SET") {
                return Some(enc.to_string());
            }
        }
    }
    None
}

/// Transcodes raw Hunspell `.aff` and `.dic` bytes into valid UTF-8 strings for `spellbook`,
/// normalizing legacy `SET <encoding>` headers (e.g. `ISO8859-1`, `KOI8-R`) to `SET UTF-8`.
pub fn transcode_hunspell_to_utf8(
    aff_bytes: &[u8],
    dic_bytes: &[u8],
) -> Result<(String, String), String> {
    let declared = detect_aff_set_encoding(aff_bytes);
    let encoding = match declared.as_deref() {
        Some(label) => resolve_hunspell_encoding(label).ok_or_else(|| {
            format!("Unsupported Hunspell dictionary encoding in SET header: '{label}'.")
        })?,
        None => {
            if std::str::from_utf8(aff_bytes).is_ok() && std::str::from_utf8(dic_bytes).is_ok() {
                UTF_8
            } else {
                encoding_rs::WINDOWS_1252
            }
        }
    };

    let aff_text = decode_bytes_with_encoding(aff_bytes, encoding, "affix (.aff)")?;
    let dic_text = decode_bytes_with_encoding(dic_bytes, encoding, "dictionary (.dic)")?;
    let normalized_aff = normalize_aff_set_header(&aff_text);
    Ok((normalized_aff, dic_text))
}

fn decode_bytes_with_encoding(
    bytes: &[u8],
    encoding: &'static Encoding,
    label: &str,
) -> Result<String, String> {
    if encoding == UTF_8 {
        if let Ok(s) = std::str::from_utf8(bytes) {
            return Ok(s.strip_prefix('\u{feff}').unwrap_or(s).to_string());
        }
        // Some dictionaries mark `SET UTF-8` or omit SET while containing stray bytes;
        // surface a clean error only if completely undecodable, otherwise decode lossily?
        // Actually if encoding is UTF-8 and fails strict UTF-8, fail clearly or fallback?
        return Err(format!(
            "Failed decoding {label} file as UTF-8 (invalid byte sequence)."
        ));
    }
    let (cow, _actual_enc, _had_errors) = encoding.decode(bytes);
    let s = cow.as_ref();
    Ok(s.strip_prefix('\u{feff}').unwrap_or(s).to_string())
}

fn normalize_aff_set_header(aff: &str) -> String {
    let mut replaced = false;
    let mut out = String::with_capacity(aff.len() + 16);
    for segment in aff.split_inclusive('\n') {
        let line_no_nl = segment.strip_suffix('\n').unwrap_or(segment);
        let line_no_cr = line_no_nl.strip_suffix('\r').unwrap_or(line_no_nl);
        let has_nl = segment.ends_with('\n');
        let trimmed = line_no_cr.trim();
        let is_set = !replaced
            && !trimmed.starts_with('#')
            && trimmed
                .split_whitespace()
                .next()
                .is_some_and(|kw| kw.eq_ignore_ascii_case("SET"));
        if is_set {
            replaced = true;
            out.push_str("SET UTF-8");
            if has_nl {
                out.push('\n');
            }
        } else {
            out.push_str(segment);
        }
    }
    if !replaced {
        format!("SET UTF-8\n{out}")
    } else {
        out
    }
}

/// Runs pure-Rust supplementary checks (repeated adjacent words across languages, plus
/// French high-punctuation spacing rules when `lang == "fr"`).
pub fn check_supplementary_rules(
    lang: &str,
    paragraph: &str,
    base_byte: usize,
    prose_bytes: &[bool],
) -> Vec<LanguageCheckDiagnostic> {
    let mut diags = Vec::new();
    check_repeated_words(paragraph, base_byte, prose_bytes, &mut diags);
    if lang == "fr" {
        check_french_punctuation_spacing(paragraph, base_byte, prose_bytes, &mut diags);
    }
    diags
}

fn check_repeated_words(
    paragraph: &str,
    base_byte: usize,
    prose_bytes: &[bool],
    out: &mut Vec<LanguageCheckDiagnostic>,
) {
    let mut words: Vec<(usize, usize, &str)> = Vec::new();
    let mut word_start: Option<usize> = None;

    for (idx, ch) in paragraph.char_indices() {
        if ch.is_alphabetic() {
            if word_start.is_none() {
                word_start = Some(idx);
            }
        } else if let Some(start) = word_start.take() {
            words.push((start, idx, &paragraph[start..idx]));
        }
    }
    if let Some(start) = word_start {
        words.push((start, paragraph.len(), &paragraph[start..]));
    }

    for pair in words.windows(2) {
        let (s1, e1, w1) = pair[0];
        let (s2, e2, w2) = pair[1];
        let between = &paragraph[e1..s2];
        if !between.chars().all(|c| c.is_whitespace()) {
            continue;
        }
        if w1.chars().count() < 2 || !w1.eq_ignore_ascii_case(w2) {
            continue;
        }
        let abs_start = base_byte + s1;
        let abs_end = base_byte + e2;
        if span_touches_masked_syntax(prose_bytes, abs_start, abs_end) {
            continue;
        }
        out.push(LanguageCheckDiagnostic {
            category: DiagnosticCategory::Grammar,
            start_byte: abs_start,
            end_byte: abs_end,
            start_char: 0,
            end_char: 0,
            line: 1,
            column: 1,
            excerpt: paragraph[s1..e2].to_string(),
            message: format!("Repeated word: '{w2}'."),
            rule_id: "GRAMMAR/REPEATED_WORD".to_string(),
            suggestions: vec![w1.to_string()],
        });
    }
}

fn check_french_punctuation_spacing(
    paragraph: &str,
    base_byte: usize,
    prose_bytes: &[bool],
    out: &mut Vec<LanguageCheckDiagnostic>,
) {
    let chars: Vec<(usize, char)> = paragraph.char_indices().collect();
    for i in 1..chars.len() {
        let (byte_idx, ch) = chars[i];
        if !matches!(ch, ';' | '!' | '?' | ':') {
            continue;
        }
        let (prev_idx, prev_ch) = chars[i - 1];
        if !prev_ch.is_alphabetic() {
            continue;
        }
        // Allow URL schemes like `https:` or trailing colon before digits (`12:30` skipped by prev_ch check).
        if ch == ':' && paragraph[byte_idx..].starts_with("://") {
            continue;
        }
        let abs_start = base_byte + prev_idx;
        let abs_end = base_byte + byte_idx + ch.len_utf8();
        if span_touches_masked_syntax(prose_bytes, abs_start, abs_end) {
            continue;
        }
        let snippet = &paragraph[prev_idx..byte_idx + ch.len_utf8()];
        let suggestion = format!("{prev_ch}\u{00A0}{ch}");
        out.push(LanguageCheckDiagnostic {
            category: DiagnosticCategory::Typography,
            start_byte: abs_start,
            end_byte: abs_end,
            start_char: 0,
            end_char: 0,
            line: 1,
            column: 1,
            excerpt: snippet.to_string(),
            message: format!("En typographie française, un espace insécable précède « {ch} »."),
            rule_id: "TYPO/FR_NBSP_PUNCTUATION".to_string(),
            suggestions: vec![suggestion],
        });
    }
}
