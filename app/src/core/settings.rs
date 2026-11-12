//! IDE user preferences, configuration defaults, and settings dialog state.

#[cfg(test)]
mod tests;

/// Active tab within the Settings sub-window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsTab {
    Appearance,
    EditorAndDisk,
    CompilerAndPreview,
}

/// Global settings model persisted for user preferences.
#[derive(Debug, Clone, PartialEq)]
pub struct IdeSettings {
    pub dark_mode: bool,
    pub editor_font_size: f32,
    pub word_wrap: bool,
    pub show_line_numbers: bool,
    pub syntax_highlighting: bool,
    pub auto_save_on_edit: bool,
    pub show_hidden_files: bool,
    pub auto_compile: bool,
    pub debounce_ms: u64,
    pub render_dpi_scale: f32,
    pub user_name: String,
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
