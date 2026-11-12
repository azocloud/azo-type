//! Desktop menu bar rendered inside the application window on Linux and Windows.
//!
//! (On macOS, `muda` provides the native screen-top menu bar via `src/platform/menu.rs`).

use std::path::Path;

use egui::{Align, Context, CornerRadius, Frame, Layout, Margin, RichText, Stroke, TopBottomPanel};

use crate::core::file_kind::FileKind;
use crate::platform::menu::MenuAction;
use crate::ui::theme::ThemePalette;

/// Editor context actions triggered from the Edit menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorContextAction {
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    SelectAll,
    WrapOrInsert(&'static str, &'static str, &'static str),
    InsertSnippet(&'static str),
    Menu(MenuAction),
    SetActiveAsMain,
}

/// Output events and actions triggered by menu item selections.
#[derive(Debug, Default)]
pub struct MenuBarOutput {
    pub pending_action: Option<MenuAction>,
    pub snippet_to_insert: Option<&'static str>,
    pub editor_action: Option<EditorContextAction>,
    pub set_active_as_main: bool,
    pub language_changed: Option<String>,
}

/// Parameters required to render the desktop menu bar.
pub struct MenuBarParams<'a> {
    pub dark_mode: bool,
    pub show_sidebar: bool,
    pub word_wrap: bool,
    pub project_root: &'a Path,
    pub active_file: &'a Path,
    pub language: &'a str,
}

/// Renders the in-window menu bar for Windows and Linux.
#[cfg(not(target_os = "macos"))]
pub fn render_desktop_menu_bar(ctx: &Context, params: MenuBarParams<'_>) -> MenuBarOutput {
    let palette = ThemePalette::for_mode(params.dark_mode);
    let menu_bg = palette.chrome_sidebar;
    let border_col = palette.panel_border;

    let mut output = MenuBarOutput::default();

    TopBottomPanel::top("desktop_os_menu_bar")
        .frame(
            Frame::new()
                .fill(menu_bg)
                .corner_radius(CornerRadius::ZERO)
                .inner_margin(Margin::symmetric(10, 4))
                .stroke(Stroke::new(1.0_f32, border_col)),
        )
        .show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;

                // FILE MENU
                ui.menu_button(RichText::new("File").strong(), |ui| {
                    ui.set_min_width(220.0);
                    if ui.button("New File in Project...  (Ctrl+N)").clicked() {
                        output.pending_action = Some(MenuAction::NewFile);
                        ui.close_menu();
                    }
                    if ui.button("Open Project Folder...  (Ctrl+O)").clicked() {
                        output.pending_action = Some(MenuAction::OpenFolder);
                        ui.close_menu();
                    }
                    if ui.button("Rescan Folder on Disk  (F5)").clicked() {
                        output.pending_action = Some(MenuAction::RescanFolder);
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Save Active File  (Ctrl+S)").clicked() {
                        output.pending_action = Some(MenuAction::SaveFile);
                        ui.close_menu();
                    }
                    if ui.button("Save All Project Files").clicked() {
                        output.pending_action = Some(MenuAction::SaveAll);
                        ui.close_menu();
                    }
                    if ui.button("Export PDF...  (Ctrl+E)").clicked() {
                        output.pending_action = Some(MenuAction::ExportPdf);
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Settings...  (Ctrl+,)").clicked() {
                        output.pending_action = Some(MenuAction::OpenSettings);
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Quit").clicked() {
                        output.pending_action = Some(MenuAction::Quit);
                        ui.close_menu();
                    }
                });

                // EDIT MENU
                ui.menu_button(RichText::new("Edit").strong(), |ui| {
                    ui.set_min_width(210.0);
                    if ui.button("Undo  (Ctrl+Z)").clicked() {
                        output.editor_action = Some(EditorContextAction::Undo);
                        ui.close_menu();
                    }
                    if ui.button("Redo  (Ctrl+Y)").clicked() {
                        output.editor_action = Some(EditorContextAction::Redo);
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Cut  (Ctrl+X)").clicked() {
                        output.editor_action = Some(EditorContextAction::Cut);
                        ui.close_menu();
                    }
                    if ui.button("Copy  (Ctrl+C)").clicked() {
                        output.editor_action = Some(EditorContextAction::Copy);
                        ui.close_menu();
                    }
                    if ui.button("Paste  (Ctrl+V)").clicked() {
                        output.editor_action = Some(EditorContextAction::Paste);
                        ui.close_menu();
                    }
                    if ui.button("Select All  (Ctrl+A)").clicked() {
                        output.editor_action = Some(EditorContextAction::SelectAll);
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Insert Section Heading").clicked() {
                        output.snippet_to_insert = Some("\n\n== New Section\n");
                        ui.close_menu();
                    }
                    if ui.button("Insert Display Math ($ ... $)").clicked() {
                        output.snippet_to_insert =
                            Some("\n$ f(x) = integral_0^x e^(-t^2) dif t $\n");
                        ui.close_menu();
                    }
                    if ui.button("Insert Formatted Table").clicked() {
                        output.snippet_to_insert = Some(
                            "\n#table(\n  columns: (1fr, 1fr),\n  [*Column A*], [*Column B*],\n  [Item 1], [Value 1],\n)\n",
                        );
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Settings...  (Ctrl+,)").clicked() {
                        output.pending_action = Some(MenuAction::OpenSettings);
                        ui.close_menu();
                    }
                });

                // VIEW MENU
                ui.menu_button(RichText::new("View").strong(), |ui| {
                    ui.set_min_width(210.0);
                    let sidebar_txt = if params.show_sidebar {
                        "Hide Project File Tree  (Ctrl+B)"
                    } else {
                        "Show Project File Tree  (Ctrl+B)"
                    };
                    if ui.button(sidebar_txt).clicked() {
                        output.pending_action = Some(MenuAction::ToggleSidebar);
                        ui.close_menu();
                    }
                    let wrap_txt = if params.word_wrap {
                        "Disable Automatic Line Wrapping"
                    } else {
                        "Enable Automatic Line Wrapping"
                    };
                    if ui.button(wrap_txt).clicked() {
                        output.pending_action = Some(MenuAction::ToggleWordWrap);
                        ui.close_menu();
                    }
                    let theme_txt = if params.dark_mode {
                        "Switch to Light Theme"
                    } else {
                        "Switch to Dark Theme"
                    };
                    if ui.button(theme_txt).clicked() {
                        output.pending_action = Some(MenuAction::ToggleTheme);
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Zoom In Preview (+)").clicked() {
                        output.pending_action = Some(MenuAction::ZoomIn);
                        ui.close_menu();
                    }
                    if ui.button("Zoom Out Preview (-)").clicked() {
                        output.pending_action = Some(MenuAction::ZoomOut);
                        ui.close_menu();
                    }
                    if ui.button("Reset Preview Zoom (100%)").clicked() {
                        output.pending_action = Some(MenuAction::ZoomReset);
                        ui.close_menu();
                    }
                });

                // PROJECT MENU
                ui.menu_button(RichText::new("Project").strong(), |ui| {
                    ui.set_min_width(220.0);
                    if ui.button("Collaboration (P2P)...").clicked() {
                        output.pending_action = Some(MenuAction::OpenCollab);
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Compile Document Now").clicked() {
                        output.pending_action = Some(MenuAction::CompileNow);
                        ui.close_menu();
                    }
                    if ui.button("Export PDF...  (Ctrl+E)").clicked() {
                        output.pending_action = Some(MenuAction::ExportPdf);
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Set Active File as Main Entrypoint").clicked() {
                        if FileKind::from_path(params.active_file) == FileKind::Typst {
                            output.set_active_as_main = true;
                            output.pending_action = Some(MenuAction::CompileNow);
                        }
                        ui.close_menu();
                    }
                });

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let current_name = crate::core::locale_native_name(params.language);
                    egui::ComboBox::from_id_salt("menu_bar_language_selector")
                        .selected_text(format!("🌐 {}", current_name))
                        .show_ui(ui, |ui| {
                            for loc in rust_i18n::available_locales!() {
                                let name = crate::core::locale_native_name(loc);
                                if ui.selectable_label(loc == params.language, name).clicked() {
                                    output.language_changed = Some(loc.to_string());
                                }
                            }
                        });

                    ui.label(
                        RichText::new(format!("Folder: {}", params.project_root.display()))
                            .small()
                            .monospace()
                            .weak(),
                    );
                });
            });
        });

    output
}
