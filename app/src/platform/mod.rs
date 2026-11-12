//! Platform-specific integrations and OS services.
//!
//! - [`menu`]: macOS muda native menu bar and cross-platform [`MenuAction`] definitions.
//! - [`dialogs`]: Platform-native file open and save dialogs.

pub mod dialogs;
pub mod menu;

pub use dialogs::{block_on_or_spin, spawn_open_folder_dialog, spawn_save_file_dialog};
pub use menu::{MacOsMenuBar, MenuAction};
