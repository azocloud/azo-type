//! Secondary OS sub-windows and modal dialogs.
//!
//! - [`export_pdf`]: PDF export options, versions, and PDF/A standards.
//! - [`file_modals`]: New file creation and file renaming.
//! - [`settings`]: Global IDE user preferences (Appearance, Editor, Compiler).

pub mod export_pdf;
pub mod file_modals;
pub mod settings;
pub mod settings_spell;

pub use export_pdf::render_export_pdf_dialog;
pub use file_modals::{render_new_file_modal, render_rename_file_modal};
pub use settings::{render_settings_dialog, SettingsModalOutput};
