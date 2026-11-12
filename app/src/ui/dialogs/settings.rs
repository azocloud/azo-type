//! Settings modal dialog (`File > Settings`) managing editor, appearance, and compilation preferences.

use std::path::Path;

use egui::{
    Align, CentralPanel, Context, CornerRadius, Frame, Layout, Margin, RichText, Slider, Stroke,
    Ui, ViewportBuilder, ViewportClass, ViewportId, Window,
};

use crate::core::settings::{IdeSettings, SettingsTab};
use crate::ui::theme::{apply_theme, ThemePalette, DESIGN_SYSTEM_CSS_TOKENS};

/// Actions and side effects requested by user interaction within the Settings dialog.
#[derive(Debug, Default)]
pub struct SettingsModalOutput {
    pub open_folder_requested: bool,
    pub theme_changed: bool,
    pub rescan_needed: bool,
    pub dpi_changed: bool,
}

/// Renders the Settings sub-window (embedded or native OS viewport).
pub fn render_settings_dialog(
    ctx: &Context,
    open: &mut bool,
    settings: &mut IdeSettings,
    active_tab: &mut SettingsTab,
    project_root: &Path,
) -> SettingsModalOutput {
    if !*open {
        return SettingsModalOutput::default();
    }

    let mut is_open = *open;
    let mut close_clicked = false;
    let mut output = SettingsModalOutput::default();

    let dark_mode = settings.dark_mode;
    let palette = ThemePalette::for_mode(dark_mode);
    let window_bg = palette.chrome_sidebar;

    let viewport_id = ViewportId::from_hash_of("typst_studio_settings_os_window");
    let builder = ViewportBuilder::default()
        .with_title("Settings — Typst Studio")
        .with_inner_size([540.0, 440.0])
        .with_min_inner_size([440.0, 340.0]);

    ctx.show_viewport_immediate(viewport_id, builder, |ctx, class| {
        let mut render_body = |ui: &mut Ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(
                    active_tab,
                    SettingsTab::Appearance,
                    RichText::new("Appearance").strong(),
                );
                ui.selectable_value(
                    active_tab,
                    SettingsTab::EditorAndDisk,
                    RichText::new("Editor & Filesystem").strong(),
                );
                ui.selectable_value(
                    active_tab,
                    SettingsTab::CompilerAndPreview,
                    RichText::new("Compiler & Preview").strong(),
                );
            });
            ui.separator();
            ui.add_space(6.0);

            match active_tab {
                SettingsTab::Appearance => {
                    ui.heading("Theme & Visual Appearance");
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label("Color Theme:");
                        if ui
                            .radio_value(&mut settings.dark_mode, true, "Dark Mode (#1C1B22)")
                            .changed()
                        {
                            output.theme_changed = true;
                        }
                        if ui
                            .radio_value(&mut settings.dark_mode, false, "Light Mode (#FFFFFF)")
                            .changed()
                        {
                            output.theme_changed = true;
                        }
                    });

                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label("Editor Font Size:");
                        ui.add(
                            Slider::new(&mut settings.editor_font_size, 10.0..=22.0).suffix(" pt"),
                        );
                    });

                    ui.add_space(8.0);
                    ui.checkbox(
                        &mut settings.word_wrap,
                        "Wrap long lines automatically to editor panel width (with end-of-line ↩ indicator)",
                    );
                    ui.checkbox(
                        &mut settings.show_line_numbers,
                        "Show line numbers gutter in code editor",
                    );
                    ui.checkbox(
                        &mut settings.syntax_highlighting,
                        "Enable real-time Typst syntax highlighting",
                    );

                    ui.add_space(8.0);
                    Frame::new()
                        .fill(palette.editor_canvas)
                        .stroke(Stroke::new(1.0_f32, palette.panel_border))
                        .corner_radius(CornerRadius::same(5))
                        .inner_margin(Margin::symmetric(10, 8))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new("Design System Tokens")
                                        .small()
                                        .strong()
                                        .color(palette.text_primary),
                                );
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if ui
                                        .small_button("Copy CSS Tokens")
                                        .on_hover_text(
                                            "Copy standardized :root and [data-theme=\"dark\"] CSS variables",
                                        )
                                        .clicked()
                                    {
                                        ctx.copy_text(DESIGN_SYSTEM_CSS_TOKENS.to_string());
                                        if let Ok(mut cb) = arboard::Clipboard::new() {
                                            let _ = cb.set_text(DESIGN_SYSTEM_CSS_TOKENS.to_string());
                                        }
                                    }
                                });
                            });
                            ui.add_space(4.0);
                            ui.horizontal_wrapped(|ui| {
                                let swatches = [
                                    ("Canvas", palette.editor_canvas),
                                    ("Chrome", palette.chrome_sidebar),
                                    ("Workspace", palette.preview_workspace),
                                    ("Line", palette.active_line_highlight),
                                    ("Keyword", palette.syn_keyword),
                                    ("String", palette.syn_string),
                                    ("Prop/Fn", palette.syn_property_or_func),
                                    ("Number", palette.syn_number),
                                ];
                                for (name, col) in swatches {
                                    Frame::new()
                                        .fill(col)
                                        .stroke(Stroke::new(1.0_f32, palette.panel_border))
                                        .corner_radius(CornerRadius::same(4))
                                        .inner_margin(Margin::symmetric(6, 3))
                                        .show(ui, |ui| {
                                            ui.label(
                                                RichText::new(name)
                                                    .size(10.5)
                                                    .monospace()
                                                    .color(palette.text_primary),
                                            );
                                        });
                                }
                            });
                        });
                }
                SettingsTab::EditorAndDisk => {
                    ui.heading("Filesystem & Project Behavior");
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(format!(
                            "Active Project Directory: {}",
                            project_root.display()
                        ))
                        .monospace()
                        .small(),
                    );
                    ui.add_space(6.0);
                    ui.checkbox(
                        &mut settings.word_wrap,
                        "Wrap long lines automatically to editor panel width (with end-of-line ↩ indicator)",
                    );
                    ui.checkbox(
                        &mut settings.auto_save_on_edit,
                        "Automatically save modified buffers to disk on edit",
                    );
                    if ui
                        .checkbox(
                            &mut settings.show_hidden_files,
                            "Show hidden dotfiles in project tree",
                        )
                        .changed()
                    {
                        output.rescan_needed = true;
                    }
                    ui.add_space(8.0);
                    if ui.button("Open Another Project Folder...").clicked() {
                        output.open_folder_requested = true;
                    }
                }
                SettingsTab::CompilerAndPreview => {
                    ui.heading("Typst Compiler & Live Preview");
                    ui.add_space(8.0);
                    ui.checkbox(
                        &mut settings.auto_compile,
                        "Automatically compile Typst document while typing",
                    );
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.label("Compile Debounce Delay:");
                        ui.add(
                            Slider::new(&mut settings.debounce_ms, 50..=800).suffix(" ms"),
                        );
                    });
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label("Preview Rasterization DPI:");
                        if ui
                            .selectable_value(
                                &mut settings.render_dpi_scale,
                                1.5,
                                "1.5x Fast",
                            )
                            .changed()
                        {
                            output.dpi_changed = true;
                        }
                        if ui
                            .selectable_value(
                                &mut settings.render_dpi_scale,
                                2.0,
                                "2.0x Crisp (Default)",
                            )
                            .changed()
                        {
                            output.dpi_changed = true;
                        }
                        if ui
                            .selectable_value(
                                &mut settings.render_dpi_scale,
                                3.0,
                                "3.0x Ultra HiDPI",
                            )
                            .changed()
                        {
                            output.dpi_changed = true;
                        }
                    });
                }
            }

            ui.add_space(14.0);
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Reset to Defaults").clicked() {
                    let prev_dark = settings.dark_mode;
                    *settings = IdeSettings::default();
                    if prev_dark != settings.dark_mode {
                        output.theme_changed = true;
                    }
                    output.dpi_changed = true;
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("Close").clicked() {
                        close_clicked = true;
                    }
                });
            });
        };

        if class == ViewportClass::Embedded {
            Window::new("Settings — Typst Studio")
                .open(&mut is_open)
                .resizable(true)
                .collapsible(false)
                .default_width(480.0)
                .default_height(360.0)
                .show(ctx, |ui| render_body(ui));
        } else {
            apply_theme(ctx, dark_mode);
            if ctx.input(|i| i.viewport().close_requested()) {
                is_open = false;
            }
            CentralPanel::default()
                .frame(
                    Frame::new()
                        .fill(window_bg)
                        .inner_margin(Margin::symmetric(14, 12)),
                )
                .show(ctx, |ui| render_body(ui));
        }
    });

    *open = is_open && !close_clicked;
    output
}
