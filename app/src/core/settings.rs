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

/// User color theme selection supporting explicit modes and OS tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum ThemePreference {
    /// Follow the operating system's light/dark appearance (defaults to Dark if unavailable).
    #[default]
    SystemDefault,
    /// Light theme palette.
    Light,
    /// Dark theme palette.
    Dark,
}

impl ThemePreference {
    /// Ordered options displayed in the Settings theme selector dropdown.
    pub const ALL: [Self; 3] = [Self::Light, Self::Dark, Self::SystemDefault];

    /// Resolves whether dark mode should be active given an optional OS dark-mode probe.
    ///
    /// When `Self::SystemDefault` is active and OS detection returns `None`, defaults to `true` (Dark).
    pub fn resolve_dark_mode(self, system_is_dark: Option<bool>) -> bool {
        match self {
            Self::Light => false,
            Self::Dark => true,
            Self::SystemDefault => system_is_dark.unwrap_or(true),
        }
    }
}

/// Global settings model persisted across restarts via `eframe` storage.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct IdeSettings {
    /// User-selected theme mode (`Light`, `Dark`, or `SystemDefault`).
    pub theme_preference: ThemePreference,
    /// Resolved dark theme flag used by rendering (`true` for dark, `false` for light).
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
    /// Whether selecting text in the preview automatically jumps the editor to the source text.
    pub preview_sync_selection: bool,
    /// Whether cursor dragging pans the preview canvas instead of selecting preview text.
    pub preview_pan_mode: bool,
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
            theme_preference: ThemePreference::SystemDefault,
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
            preview_sync_selection: true,
            preview_pan_mode: false,
            user_name: default_user,
            user_color: [56, 189, 248],
        }
    }
}

impl IdeSettings {
    /// Clamps numeric fields to safe ranges after loading persisted data.
    pub fn sanitized(mut self) -> Self {
        let defaults = Self::default();
        self.editor_font_size = if self.editor_font_size.is_finite() {
            self.editor_font_size.clamp(10.0, 22.0)
        } else {
            defaults.editor_font_size
        };
        self.debounce_ms = self.debounce_ms.clamp(50, 800);
        self.render_dpi_scale = if self.render_dpi_scale.is_finite() {
            self.render_dpi_scale.clamp(1.0, 4.0)
        } else {
            defaults.render_dpi_scale
        };
        if self.user_name.trim().is_empty() {
            self.user_name = defaults.user_name;
        }
        self
    }
}

/// Predefined zoom percentage presets shown in the preview zoom selector.
pub const PREVIEW_ZOOM_PRESETS: &[f32] = &[0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0];

/// Minimum allowed preview zoom scale factor (`25%`).
pub const MIN_PREVIEW_ZOOM: f32 = 0.25;

/// Maximum allowed preview zoom scale factor (`400%`).
pub const MAX_PREVIEW_ZOOM: f32 = 4.0;

/// Step increment used for menu zoom in/out actions.
const ZOOM_STEP: f32 = 0.15;

/// Preview zoom state supporting both Fit Width and fixed zoom scales.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct PreviewZoom {
    /// Whether pages automatically scale to fit the preview viewport width.
    pub fit_width: bool,
    /// Explicit scale factor (`1.0` = 100%) used when `fit_width` is `false`.
    pub scale: f32,
    /// Most recently computed effective scale factor (tracks dynamic Fit Width scale).
    pub effective_scale: f32,
}

impl Default for PreviewZoom {
    fn default() -> Self {
        Self {
            fit_width: true,
            scale: 1.0,
            effective_scale: 1.0,
        }
    }
}

impl PreviewZoom {
    /// Enables dynamic Fit Width zoom mode.
    pub fn set_fit_width(&mut self) {
        self.fit_width = true;
    }

    /// Switches to fixed-percentage zoom mode clamped to `[MIN_PREVIEW_ZOOM, MAX_PREVIEW_ZOOM]`.
    pub fn set_scale(&mut self, scale: f32) {
        let clamped = scale.clamp(MIN_PREVIEW_ZOOM, MAX_PREVIEW_ZOOM);
        self.fit_width = false;
        self.scale = clamped;
        self.effective_scale = clamped;
    }

    /// Applies a multiplicative wheel/pinch zoom delta (`!= 1.0`), exiting Fit Width if active.
    pub fn apply_zoom_delta(&mut self, zoom_delta: f32) {
        if (zoom_delta - 1.0).abs() <= 0.001 {
            return;
        }
        let base = if self.fit_width {
            self.effective_scale
        } else {
            self.scale
        };
        self.set_scale(base * zoom_delta);
    }

    /// Steps zoom inward by a fixed increment, exiting Fit Width from the active effective scale.
    pub fn step_in(&mut self) {
        let base = if self.fit_width {
            self.effective_scale
        } else {
            self.scale
        };
        self.set_scale(base + ZOOM_STEP);
    }

    /// Steps zoom outward by a fixed increment, exiting Fit Width from the active effective scale.
    pub fn step_out(&mut self) {
        let base = if self.fit_width {
            self.effective_scale
        } else {
            self.scale
        };
        self.set_scale(base - ZOOM_STEP);
    }

    /// Resets zoom to an explicit `100%` scale.
    pub fn reset_100(&mut self) {
        self.set_scale(1.0);
    }

    /// Computes the display scale for the given viewport width and widest page width (in points).
    pub fn effective_scale_for_viewport(&self, avail_width_px: f32, max_page_width_pt: f32) -> f32 {
        if self.fit_width && max_page_width_pt > 0.0 && avail_width_px > 0.0 {
            (avail_width_px / max_page_width_pt).clamp(MIN_PREVIEW_ZOOM, MAX_PREVIEW_ZOOM)
        } else {
            self.scale.clamp(MIN_PREVIEW_ZOOM, MAX_PREVIEW_ZOOM)
        }
    }

    /// Returns the rounded zoom percentage suitable for toolbar display.
    pub fn display_percent(&self) -> u32 {
        (self.effective_scale * 100.0).round().max(1.0) as u32
    }
}
