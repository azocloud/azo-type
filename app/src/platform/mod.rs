//! Platform-specific integrations and OS services.
//!
//! - [`menu`]: macOS muda native menu bar and cross-platform [`MenuAction`] definitions.
//! - [`dialogs`]: Platform-native file open and save dialogs.
//! - [`icon`]: Application window icon loader, decoding, and viewport configuration.
//! - [`theme`]: Cross-platform OS dark/light appearance detection.

pub mod dialogs;
pub mod icon;
pub mod menu;
pub mod theme;

pub use dialogs::{block_on_or_spin, spawn_open_folder_dialog, spawn_save_file_dialog};
pub use icon::{
    app_window_icon, configure_viewport_builder, ensure_linux_desktop_integration,
    load_icon_from_png_bytes, APP_ID,
};
pub use menu::{MacOsMenuBar, MenuAction};
pub use theme::{detect_system_dark_mode, resolve_effective_dark_mode};
