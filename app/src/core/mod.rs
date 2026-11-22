//! Core domain models, algorithms, and data types.
//!
//! This module contains pure logic with zero GUI and zero operating-system dependencies:
//! - [`diff`]: UTF-16 code unit offset calculations, text differencing, and path collisions.
//! - [`engine`]: Hardcoded Typst release catalog, target matching, and pure-Rust SHA-256 verification.
//! - [`file_kind`]: Semantic file type classification.
//! - [`i18n`]: Cross-platform locale normalization, language detection, and native display names.
//! - [`intellisense`]: Pure Typst IntelliSense engine for completions, hover tooltips, and hints.
//! - [`outline`]: Document outline extraction, AST traversal, and hierarchical numbering.
//! - [`pdf`]: PDF versions, archival (PDF/A) standards, and export configurations.
//! - [`recent_projects`]: Recently opened project folder MRU history and display formatting.
//! - [`search`]: Pure char-indexed search and replace utilities.
//! - [`settings`]: User preferences model and default configuration.
//! - [`spellcheck`]: Dictionary manifest, Hunspell transcoding, and language diagnostic types.
//! - [`text_edit`]: Pure line, indentation, and comment editing transformations.
//! - [`typst_prose`]: Typst AST prose masking, `#set text(lang: ...)` detection, and byte/char spans.

#![warn(missing_docs)]

pub mod diff;
pub mod engine;
pub mod file_kind;
pub mod i18n;
pub mod intellisense;
pub mod outline;
pub mod pdf;
pub mod recent_projects;
pub mod search;
pub mod settings;
pub mod spellcheck;
pub mod text_edit;
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
pub use intellisense::{
    compute_completions, compute_hover_info, compute_typst_hints, CompletionItem, CompletionKind,
    CompletionRequestContext, HoverInfo, LabelDefinition, TypstDiagnosticHint,
    UserSymbolDefinition,
};
pub use outline::{
    assign_hierarchical_numbers, build_outline_tree, extract_document_outline,
    extract_typst_ast_outline, extract_typst_markup_plain_text, CompiledOutlineItem,
    DocumentOutlineItem, OutlineTreeNode, SourceResolver,
};
pub use pdf::{PdfArchivalStandard, PdfExportConfig, PdfVersionChoice};
pub use recent_projects::{RecentProjects, MAX_RECENT_PROJECTS};
pub use search::{
    find_all_matches, find_next_match_index, replace_all_matches, replace_match_at, SearchMatch,
    SearchOptions,
};
pub use settings::{
    IdeSettings, PreviewZoom, SettingsTab, ThemePreference, MAX_PREVIEW_ZOOM, MIN_PREVIEW_ZOOM,
    PREVIEW_ZOOM_PRESETS,
};
pub use spellcheck::{
    dictionary_catalog, find_tier1_pack, transcode_hunspell_to_utf8, AssetCompression,
    DiagnosticCategory, DictionaryCatalog, DictionaryFileAsset, DictionaryFileRole,
    LanguageCheckDiagnostic, Tier1LanguagePack, UserDictionaryEntry, TIER1_LANGUAGES,
};
pub use text_edit::{
    duplicate_lines, indent_lines, move_lines, outdent_lines, toggle_block_comment,
    toggle_line_comment,
};
pub use typst_prose::{
    byte_span_to_char_range, detect_typst_document_language, extract_typst_prose_mask,
    normalize_language_code, split_prose_paragraphs, ProseParagraph, TypstProseMask,
};
