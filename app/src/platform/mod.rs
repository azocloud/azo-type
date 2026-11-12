//! Platform-specific integrations and OS services.
//!
//! - [`menu`]: macOS muda native menu bar and cross-platform [`MenuAction`] definitions.
//! - [`dialogs`]: Platform-native file open and save dialogs.
//! - [`icon`]: Application window icon loader, decoding, and viewport configuration.

pub mod dialogs;
pub mod icon;
pub mod menu;

pub use dialogs::{block_on_or_spin, spawn_open_folder_dialog, spawn_save_file_dialog};
pub use icon::{app_window_icon, configure_viewport_builder, load_icon_from_png_bytes, APP_ID};
pub use menu::{MacOsMenuBar, MenuAction};
