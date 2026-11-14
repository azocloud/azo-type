//! Desktop menu bar rendered inside the application window on Linux and Windows.
//!
//! (On macOS, `muda` provides the native screen-top menu bar via `src/platform/menu.rs`).

use std::path::Path;

use egui::{Align, Context, CornerRadius, Frame, Layout, Margin, RichText, Stroke, TopBottomPanel};
use rust_i18n::t;

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
}

/// Parameters required to render the desktop menu bar.
pub struct MenuBarParams<'a> {
    pub dark_mode: bool,
    pub show_sidebar: bool,
    pub show_bottom_panel: bool,
    pub word_wrap: bool,
    pub project_root: &'a Path,
    pub active_file: &'a Path,
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
                ui.menu_button(RichText::new(t!("menu.file")).strong(), |ui| {
                    ui.set_min_width(220.0);
                    if ui.button(t!("menu.file_new")).clicked() {
                        output.pending_action = Some(MenuAction::NewFile);
                        ui.close_menu();
                    }
                    if ui.button(t!("menu.file_open")).clicked() {
                        output.pending_action = Some(MenuAction::OpenFolder);
                        ui.close_menu();
                    }
                    if ui.button(t!("menu.rescan_folder")).clicked() {
                        output.pending_action = Some(MenuAction::RescanFolder);
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button(t!("menu.save_file")).clicked() {
                        output.pending_action = Some(MenuAction::SaveFile);
                        ui.close_menu();
                    }
                    if ui.button(t!("menu.save_all")).clicked() {
                        output.pending_action = Some(MenuAction::SaveAll);
                        ui.close_menu();
                    }
                    if ui.button(t!("menu.export_pdf")).clicked() {
                        output.pending_action = Some(MenuAction::ExportPdf);
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button(t!("menu.settings")).clicked() {
                        output.pending_action = Some(MenuAction::OpenSettings);
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button(t!("menu.quit")).clicked() {
                        output.pending_action = Some(MenuAction::Quit);
                        ui.close_menu();
                    }
                });

                // EDIT MENU
                ui.menu_button(RichText::new(t!("menu.edit")).strong(), |ui| {
                    ui.set_min_width(210.0);
                    if ui.button(t!("menu.undo")).clicked() {
                        output.editor_action = Some(EditorContextAction::Undo);
                        ui.close_menu();
                    }
                    if ui.button(t!("menu.redo")).clicked() {
                        output.editor_action = Some(EditorContextAction::Redo);
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button(t!("menu.cut")).clicked() {
                        output.editor_action = Some(EditorContextAction::Cut);
                        ui.close_menu();
                    }
                    if ui.button(t!("menu.copy")).clicked() {
                        output.editor_action = Some(EditorContextAction::Copy);
                        ui.close_menu();
                    }
                    if ui.button(t!("menu.paste")).clicked() {
                        output.editor_action = Some(EditorContextAction::Paste);
                        ui.close_menu();
                    }
                    if ui.button(t!("menu.select_all")).clicked() {
                        output.editor_action = Some(EditorContextAction::SelectAll);
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button(t!("menu.insert_heading")).clicked() {
                        output.snippet_to_insert = Some("\n\n== New Section\n");
                        ui.close_menu();
                    }
                    if ui.button(t!("menu.insert_math")).clicked() {
                        output.snippet_to_insert =
                            Some("\n$ f(x) = integral_0^x e^(-t^2) dif t $\n");
                        ui.close_menu();
                    }
                    if ui.button(t!("menu.insert_table")).clicked() {
                        output.snippet_to_insert = Some(
                            "\n#table(\n  columns: (1fr, 1fr),\n  [*Column A*], [*Column B*],\n  [Item 1], [Value 1],\n)\n",
                        );
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button(t!("menu.settings")).clicked() {
                        output.pending_action = Some(MenuAction::OpenSettings);
                        ui.close_menu();
                    }
                });

                // VIEW MENU
                ui.menu_button(RichText::new(t!("menu.view")).strong(), |ui| {
                    ui.set_min_width(210.0);
                    let sidebar_txt = if params.show_sidebar {
                        t!("menu.hide_sidebar")
                    } else {
                        t!("menu.show_sidebar")
                    };
                    if ui.button(sidebar_txt).clicked() {
                        output.pending_action = Some(MenuAction::ToggleSidebar);
                        ui.close_menu();
                    }
                    let bottom_panel_txt = if params.show_bottom_panel {
                        t!("menu.hide_bottom_panel")
                    } else {
                        t!("menu.show_bottom_panel")
                    };
                    if ui.button(bottom_panel_txt).clicked() {
                        output.pending_action = Some(MenuAction::ToggleBottomPanel);
                        ui.close_menu();
                    }
                    let wrap_txt = if params.word_wrap {
                        t!("menu.disable_word_wrap")
                    } else {
                        t!("menu.enable_word_wrap")
                    };
                    if ui.button(wrap_txt).clicked() {
                        output.pending_action = Some(MenuAction::ToggleWordWrap);
                        ui.close_menu();
                    }
                    let theme_txt = if params.dark_mode {
                        t!("menu.theme_light")
                    } else {
                        t!("menu.theme_dark")
                    };
                    if ui.button(theme_txt).clicked() {
                        output.pending_action = Some(MenuAction::ToggleTheme);
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button(t!("menu.zoom_in")).clicked() {
                        output.pending_action = Some(MenuAction::ZoomIn);
                        ui.close_menu();
                    }
                    if ui.button(t!("menu.zoom_out")).clicked() {
                        output.pending_action = Some(MenuAction::ZoomOut);
                        ui.close_menu();
                    }
                    if ui.button(t!("menu.zoom_reset")).clicked() {
                        output.pending_action = Some(MenuAction::ZoomReset);
                        ui.close_menu();
                    }
                });

                // PROJECT MENU
                ui.menu_button(RichText::new(t!("menu.project")).strong(), |ui| {
                    ui.set_min_width(220.0);
                    if ui.button(t!("menu.collab")).clicked() {
                        output.pending_action = Some(MenuAction::OpenCollab);
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button(t!("menu.compile_now")).clicked() {
                        output.pending_action = Some(MenuAction::CompileNow);
                        ui.close_menu();
                    }
                    if ui.button(t!("menu.export_pdf")).clicked() {
                        output.pending_action = Some(MenuAction::ExportPdf);
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button(t!("menu.set_main")).clicked() {
                        if FileKind::from_path(params.active_file) == FileKind::Typst {
                            output.set_active_as_main = true;
                            output.pending_action = Some(MenuAction::CompileNow);
                        }
                        ui.close_menu();
                    }
                });

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .selectable_label(
                            params.show_bottom_panel,
                            RichText::new(t!("menu.bottom_panel_toggle")).small().strong(),
                        )
                        .on_hover_text(t!("menu.bottom_panel_tooltip"))
                        .clicked()
                    {
                        output.pending_action = Some(MenuAction::ToggleBottomPanel);
                    }
                    ui.separator();
                    ui.label(
                        RichText::new(format!(
                            "{}: {}",
                            t!("menu.folder_label"),
                            params.project_root.display()
                        ))
                        .small()
                        .monospace()
                        .weak(),
                    );
                });
            });
        });

    output
}

#[cfg(test)]
#[path = "../../tests/ui/components/menu_bar_tests.rs"]
mod tests;
