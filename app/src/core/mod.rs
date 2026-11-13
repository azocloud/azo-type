//! Core domain models, algorithms, and data types.
//!
//! This module contains pure logic with zero GUI and zero operating-system dependencies:
//! - [`diff`]: UTF-16 code unit offset calculations, text differencing, and path collisions.
//! - [`engine`]: Hardcoded Typst release catalog, target matching, and pure-Rust SHA-256 verification.
//! - [`file_kind`]: Semantic file type classification.
//! - [`i18n`]: Cross-platform locale normalization, language detection, and native display names.
//! - [`outline`]: Document outline extraction, AST traversal, and hierarchical numbering.
//! - [`pdf`]: PDF versions, archival (PDF/A) standards, and export configurations.
//! - [`settings`]: User preferences model and default configuration.
//! - [`spellcheck`]: Dictionary manifest, Hunspell transcoding, and language diagnostic types.
//! - [`typst_prose`]: Typst AST prose masking, `#set text(lang: ...)` detection, and byte/char spans.

#![warn(missing_docs)]

pub mod diff;
pub mod engine;
pub mod file_kind;
pub mod i18n;
pub mod outline;
pub mod pdf;
pub mod settings;
pub mod spellcheck;
pub mod typst_prose;

pub use diff::{
    char_to_utf16_offset, compute_utf16_diff, format_collision_path, normalize_rel_path,
    utf16_to_char_offset, CharSelection, Utf16TextDiff,
};
pub use engine::{
    all_selectable_versions, available_typst_releases, find_asset_for_target, find_host_asset,
    find_release_for_version, host_target_triple, is_builtin_version, sha256_hex,
    verify_sha256_hex, EngineArchiveKind, EngineReleaseAsset, TypstEngineRelease,
    BUILTIN_TYPST_VERSION,
};
pub use file_kind::FileKind;
pub use i18n::{
    detect_system_locale, locale_native_name, normalize_locale_tag, resolve_locale, AppSavedState,
};
pub use outline::{
    assign_hierarchical_numbers, build_outline_tree, extract_document_outline,
    extract_typst_ast_outline, extract_typst_markup_plain_text, CompiledOutlineItem,
    DocumentOutlineItem, OutlineTreeNode, SourceResolver,
};
pub use pdf::{PdfArchivalStandard, PdfExportConfig, PdfVersionChoice};
pub use settings::{
    IdeSettings, PreviewZoom, SettingsTab, ThemePreference, MAX_PREVIEW_ZOOM, MIN_PREVIEW_ZOOM,
    PREVIEW_ZOOM_PRESETS,
};
pub use spellcheck::{
    dictionary_catalog, find_tier1_pack, transcode_hunspell_to_utf8, AssetCompression,
    DiagnosticCategory, DictionaryCatalog, DictionaryFileAsset, DictionaryFileRole,
    LanguageCheckDiagnostic, Tier1LanguagePack, UserDictionaryEntry, TIER1_LANGUAGES,
};
pub use typst_prose::{
    byte_span_to_char_range, detect_typst_document_language, extract_typst_prose_mask,
    normalize_language_code, split_prose_paragraphs, ProseParagraph, TypstProseMask,
};
