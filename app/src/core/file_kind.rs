//! File type identification and classification based on file extensions.

use std::path::Path;

#[cfg(test)]
mod tests;

/// Semantic classification of project files for syntax highlighting, editing, and preview.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileKind {
    Typst,
    Bib,
    Svg,
    Image,
    Text,
    Binary,
}

impl FileKind {
    /// Infers the file kind from the extension of the given path.
    pub fn from_path(path: &Path) -> Self {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        match ext.as_str() {
            "typ" => Self::Typst,
            "bib" | "yml" | "yaml" => Self::Bib,
            "svg" => Self::Svg,
            "png" | "jpg" | "jpeg" | "gif" | "webp" => Self::Image,
            "txt" | "md" | "csv" | "json" | "toml" => Self::Text,
            _ => Self::Binary,
        }
    }

    /// Whether this file type can be opened in the textual code editor.
    pub fn is_editable_text(self) -> bool {
        matches!(self, Self::Typst | Self::Bib | Self::Svg | Self::Text)
    }

    /// Short glyph/icon token representing this file type in the UI file tree.
    pub fn icon(self) -> &'static str {
        match self {
            Self::Typst => "[T]",
            Self::Bib => "[B]",
            Self::Svg => "[S]",
            Self::Image => "[I]",
            Self::Text => "[F]",
            Self::Binary => "[#]",
        }
    }

    /// Human-readable label for this file kind.
    pub fn label(self) -> &'static str {
        match self {
            Self::Typst => "Typst Source (.typ)",
            Self::Bib => "Bibliography (.bib)",
            Self::Svg => "Vector Graphic (.svg)",
            Self::Image => "Raster Image",
            Self::Text => "Text Document",
            Self::Binary => "Binary Asset",
        }
    }
}
