//! Application icon loader, PNG image decoding, and desktop viewport icon configuration.

use std::sync::Arc;
use std::sync::OnceLock;

/// Application ID used for Wayland compositors, desktop window grouping, and application matching.
pub const APP_ID: &str = "azo-type";

/// Decodes and returns the cached high-resolution application window icon.
///
/// The icon is embedded at compile time from `app/icons/icon.png` and decoded once
/// into an [`egui::IconData`] structure suitable for window managers, taskbars, and Alt+Tab switchers.
#[must_use]
pub fn app_window_icon() -> Arc<egui::IconData> {
    static ICON: OnceLock<Arc<egui::IconData>> = OnceLock::new();
    ICON.get_or_init(|| {
        let icon_bytes = include_bytes!("../../icons/icon.png");
        let icon_data = load_icon_from_png_bytes(icon_bytes)
            .expect("Embedded application icon PNG must be a valid image");
        Arc::new(icon_data)
    })
    .clone()
}

/// Helper function to configure an [`egui::ViewportBuilder`] with the application ID and window icon.
///
/// Applies both `.with_app_id(APP_ID)` and `.with_icon(app_window_icon())` to ensure
/// consistent icon rendering on Wayland, X11, Windows, and macOS sub-windows.
#[must_use]
pub fn configure_viewport_builder(builder: egui::ViewportBuilder) -> egui::ViewportBuilder {
    builder.with_app_id(APP_ID).with_icon(app_window_icon())
}

/// Decodes raw PNG image bytes into an [`egui::IconData`].
///
/// Returns an error if the byte slice is not a valid PNG image or cannot be decoded.
pub fn load_icon_from_png_bytes(bytes: &[u8]) -> Result<egui::IconData, String> {
    let img = image::load_from_memory(bytes)
        .map_err(|err| format!("Failed to decode icon PNG: {err}"))?;
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    Ok(egui::IconData {
        rgba: rgba.into_raw(),
        width,
        height,
    })
}

#[cfg(test)]
mod tests;
