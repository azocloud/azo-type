//! IDE user preferences, configuration defaults, and settings dialog state.

#[cfg(test)]
mod tests;

/// Active tab within the Settings sub-window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsTab {
    /// Visual theme and typography settings.
    Appearance,
    /// Text editor behavior and disk synchronization settings.
    EditorAndDisk,
    /// Typst compiler and preview rendering settings.
    CompilerAndPreview,
}

/// Global settings model persisted for user preferences.
#[derive(Debug, Clone, PartialEq)]
pub struct IdeSettings {
    /// Whether dark theme mode is active.
    pub dark_mode: bool,
    /// Font size (in points) used in the code editor.
    pub editor_font_size: f32,
    /// Whether long lines wrap in the code editor.
    pub word_wrap: bool,
    /// Whether line numbers are rendered in the editor gutter.
    pub show_line_numbers: bool,
    /// Whether Typst syntax highlighting is enabled.
    pub syntax_highlighting: bool,
    /// Whether edits automatically save to disk.
    pub auto_save_on_edit: bool,
    /// Whether dotfiles and hidden directories are shown in the file tree.
    pub show_hidden_files: bool,
    /// Whether edits automatically trigger incremental compilation.
    pub auto_compile: bool,
    /// Debounce interval (in milliseconds) before auto-compilation runs.
    pub debounce_ms: u64,
    /// Rasterization DPI scale factor for preview rendering.
    pub render_dpi_scale: f32,
    /// Display name advertised to peers during collaborative editing.
    pub user_name: String,
    /// RGB badge color advertised to peers during collaborative editing.
    pub user_color: [u8; 3],
}

impl Default for IdeSettings {
    fn default() -> Self {
        let default_user = std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .unwrap_or_else(|_| "Author".to_string());
        Self {
            dark_mode: true,
            editor_font_size: 13.5,
            word_wrap: true,
            show_line_numbers: true,
            syntax_highlighting: true,
            auto_save_on_edit: true,
            show_hidden_files: false,
            auto_compile: true,
            debounce_ms: 120,
            render_dpi_scale: 2.0,
            user_name: default_user,
            user_color: [56, 189, 248],
        }
    }
}
