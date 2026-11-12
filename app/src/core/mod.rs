//! Core domain models, algorithms, and data types.
//!
//! This module contains pure logic with zero GUI and zero operating-system dependencies:
//! - [`diff`]: UTF-16 code unit offset calculations, text differencing, and path collisions.
//! - [`file_kind`]: Semantic file type classification.
//! - [`outline`]: Document outline extraction, AST traversal, and hierarchical numbering.
//! - [`pdf`]: PDF versions, archival (PDF/A) standards, and export configurations.
//! - [`settings`]: User preferences model and default configuration.

pub mod diff;
pub mod file_kind;
pub mod outline;
pub mod pdf;
pub mod settings;

pub use diff::{
    char_to_utf16_offset, compute_utf16_diff, format_collision_path, normalize_rel_path,
    utf16_to_char_offset, CharSelection, Utf16TextDiff,
};
pub use file_kind::FileKind;
pub use outline::{
    assign_hierarchical_numbers, build_outline_tree, extract_document_outline,
    extract_typst_ast_outline, extract_typst_markup_plain_text, CompiledOutlineItem,
    DocumentOutlineItem, OutlineTreeNode, SourceResolver,
};
pub use pdf::{PdfArchivalStandard, PdfExportConfig, PdfVersionChoice};
pub use settings::{IdeSettings, SettingsTab};
