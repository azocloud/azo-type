//! Settings modal dialog (`File > Settings`) managing editor, appearance, and compilation preferences.

use std::path::Path;

use egui::{
    Align, CentralPanel, Context, CornerRadius, Frame, Layout, Margin, RichText, Slider, Stroke,
    Ui, ViewportBuilder, ViewportClass, ViewportId, Window,
};
use rust_i18n::t;

use crate::core::settings::{IdeSettings, SettingsTab};
use crate::ui::theme::{apply_theme, ThemePalette, DESIGN_SYSTEM_CSS_TOKENS};

/// Actions and side effects requested by user interaction within the Settings dialog.
#[derive(Debug, Default)]
pub struct SettingsModalOutput {
    pub open_folder_requested: bool,
    pub theme_changed: bool,
    pub rescan_needed: bool,
    pub dpi_changed: bool,
    pub language_changed: Option<String>,
}

/// Renders the Settings sub-window (embedded or native OS viewport).
pub fn render_settings_dialog(
    ctx: &Context,
    open: &mut bool,
    settings: &mut IdeSettings,
    language: &str,
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

    let viewport_id = ViewportId::from_hash_of("azo_type_settings_os_window");
    let builder = crate::platform::configure_viewport_builder(
        ViewportBuilder::default()
            .with_title(t!("settings.window_title"))
            .with_inner_size([540.0, 440.0])
            .with_min_inner_size([440.0, 340.0]),
    );

    ctx.show_viewport_immediate(viewport_id, builder, |ctx, class| {
        let mut render_body = |ui: &mut Ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(
                    active_tab,
                    SettingsTab::Appearance,
                    RichText::new(t!("settings.tab_appearance")).strong(),
                );
                ui.selectable_value(
                    active_tab,
                    SettingsTab::EditorAndDisk,
                    RichText::new(t!("settings.tab_editor")).strong(),
                );
                ui.selectable_value(
                    active_tab,
                    SettingsTab::CompilerAndPreview,
                    RichText::new(t!("settings.tab_compiler")).strong(),
                );
            });
            ui.separator();
            ui.add_space(6.0);

            match active_tab {
                SettingsTab::Appearance => {
                    ui.heading(t!("settings.heading_appearance"));
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label(t!("settings.language_label"));
                        let current_name = crate::core::locale_native_name(language);
                        egui::ComboBox::from_id_salt("settings_language_combo")
                            .selected_text(current_name)
                            .show_ui(ui, |ui| {
                                for loc in rust_i18n::available_locales!() {
                                    let name = crate::core::locale_native_name(loc);
                                    if ui.selectable_label(loc == language, name).clicked() {
                                        output.language_changed = Some(loc.to_string());
                                    }
                                }
                            });
                    });

                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label(t!("settings.theme_label"));
                        if ui
                            .radio_value(&mut settings.dark_mode, true, t!("settings.theme_dark"))
                            .changed()
                        {
                            output.theme_changed = true;
                        }
                        if ui
                            .radio_value(&mut settings.dark_mode, false, t!("settings.theme_light"))
                            .changed()
                        {
                            output.theme_changed = true;
                        }
                    });

                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label(t!("settings.font_size_label"));
                        ui.add(
                            Slider::new(&mut settings.editor_font_size, 10.0..=22.0).suffix(" pt"),
                        );
                    });

                    ui.add_space(8.0);
                    ui.checkbox(&mut settings.word_wrap, t!("settings.word_wrap_label"));
                    ui.checkbox(
                        &mut settings.show_line_numbers,
                        t!("settings.line_numbers_label"),
                    );
                    ui.checkbox(
                        &mut settings.syntax_highlighting,
                        t!("settings.syntax_highlighting_label"),
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
                                    RichText::new(t!("settings.design_system_tokens"))
                                        .small()
                                        .strong()
                                        .color(palette.text_primary),
                                );
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if ui
                                        .small_button(t!("settings.copy_css_tokens"))
                                        .on_hover_text(t!("settings.copy_css_tooltip"))
                                        .clicked()
                                    {
                                        ctx.copy_text(DESIGN_SYSTEM_CSS_TOKENS.to_string());
                                        if let Ok(mut cb) = arboard::Clipboard::new() {
                                            let _ =
                                                cb.set_text(DESIGN_SYSTEM_CSS_TOKENS.to_string());
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
                    ui.heading(t!("settings.heading_editor"));
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(format!(
                            "{}: {}",
                            t!("settings.project_folder_label"),
                            project_root.display()
                        ))
                        .monospace()
                        .small(),
                    );
                    ui.add_space(6.0);
                    ui.checkbox(&mut settings.word_wrap, t!("settings.word_wrap_label"));
                    ui.checkbox(
                        &mut settings.auto_save_on_edit,
                        t!("settings.auto_save_label"),
                    );
                    if ui
                        .checkbox(
                            &mut settings.show_hidden_files,
                            t!("settings.show_hidden_label"),
                        )
                        .changed()
                    {
                        output.rescan_needed = true;
                    }
                    ui.add_space(8.0);
                    if ui.button(t!("settings.choose_folder_btn")).clicked() {
                        output.open_folder_requested = true;
                    }
                }
                SettingsTab::CompilerAndPreview => {
                    ui.heading(t!("settings.heading_compiler"));
                    ui.add_space(8.0);
                    ui.checkbox(
                        &mut settings.auto_compile,
                        t!("settings.auto_compile_label"),
                    );
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.label(t!("settings.debounce_label"));
                        ui.add(Slider::new(&mut settings.debounce_ms, 50..=800).suffix(" ms"));
                    });
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label(t!("settings.dpi_scale_label"));
                        if ui
                            .selectable_value(
                                &mut settings.render_dpi_scale,
                                1.5,
                                t!("settings.dpi_fast"),
                            )
                            .changed()
                        {
                            output.dpi_changed = true;
                        }
                        if ui
                            .selectable_value(
                                &mut settings.render_dpi_scale,
                                2.0,
                                t!("settings.dpi_crisp"),
                            )
                            .changed()
                        {
                            output.dpi_changed = true;
                        }
                        if ui
                            .selectable_value(
                                &mut settings.render_dpi_scale,
                                3.0,
                                t!("settings.dpi_hidpi"),
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
                if ui.button(t!("settings.reset_defaults")).clicked() {
                    let prev_dark = settings.dark_mode;
                    *settings = IdeSettings::default();
                    if prev_dark != settings.dark_mode {
                        output.theme_changed = true;
                    }
                    output.dpi_changed = true;
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button(t!("settings.close_btn")).clicked() {
                        close_clicked = true;
                    }
                });
            });
        };

        if class == ViewportClass::Embedded {
            Window::new(t!("settings.window_title"))
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
