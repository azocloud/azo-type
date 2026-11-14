//! Export PDF sub-window dialog for selecting PDF versions and PDF/A standards.

use std::path::Path;

use egui::{
    Align, CentralPanel, Color32, Context, CornerRadius, Frame, Layout, Margin, RichText,
    ScrollArea, Stroke, Ui, ViewportBuilder, ViewportClass, ViewportId, Window,
};
use rust_i18n::t;

use crate::core::pdf::{PdfArchivalStandard, PdfExportConfig, PdfVersionChoice};
use crate::ui::theme::{apply_theme, ThemePalette};

/// Selects a PDF version in `config` and clears any previous export error.
pub fn apply_pdf_version_choice(
    config: &mut PdfExportConfig,
    export_error: &mut Option<String>,
    version: PdfVersionChoice,
) {
    config.version = version;
    *export_error = None;
}

/// Selects a PDF archival standard, clears errors, and adjusts incompatible PDF versions automatically.
pub fn apply_archival_standard_choice(
    config: &mut PdfExportConfig,
    export_error: &mut Option<String>,
    standard: PdfArchivalStandard,
) {
    config.archival_standard = standard;
    *export_error = None;
    if config.build_standards().is_err() {
        if let Some(rec) = standard.recommended_version() {
            config.version = rec;
        }
    }
}

/// Applies a change to `pdf_ua1`, enforcing tagged PDF and compatible PDF version when enabled.
pub fn apply_pdf_ua1_toggle(config: &mut PdfExportConfig, export_error: &mut Option<String>) {
    *export_error = None;
    if config.pdf_ua1 {
        config.tagged = true;
        if config.build_standards().is_err() {
            config.version = PdfVersionChoice::V1_7;
        }
    }
}

/// Returns `true` when the selected archival standard or PDF/UA-1 requires tagged PDF output.
pub fn standard_requires_tagged(config: &PdfExportConfig) -> bool {
    config.pdf_ua1
        || matches!(
            config.archival_standard,
            PdfArchivalStandard::A1a | PdfArchivalStandard::A2a | PdfArchivalStandard::A3a
        )
}

/// Renders the Export PDF dialog either as an immediate-mode embedded window or a native OS sub-window.
///
/// Returns `true` if the user confirmed the export action.
#[allow(clippy::too_many_arguments)]
pub fn render_export_pdf_dialog(
    ctx: &Context,
    open: &mut bool,
    config: &mut PdfExportConfig,
    export_error: &mut Option<String>,
    entry_file: &Path,
    active_engine_label: &str,
    fallback_notice: Option<&str>,
    dark_mode: bool,
) -> bool {
    if !*open {
        return false;
    }

    let window_bg = ThemePalette::for_mode(dark_mode).chrome_sidebar;
    let mut is_open = *open;
    let mut close_clicked = false;
    let mut trigger_export = false;

    let viewport_id = ViewportId::from_hash_of("azo_type_export_pdf_os_window");
    let builder = crate::platform::configure_viewport_builder(
        ViewportBuilder::default()
            .with_title(t!("export_pdf.window_title"))
            .with_inner_size([560.0, 480.0])
            .with_min_inner_size([460.0, 380.0]),
    );

    ctx.show_viewport_immediate(viewport_id, builder, |ctx, class| {
        if class == ViewportClass::Embedded {
            Window::new(t!("export_pdf.window_title"))
                .open(&mut is_open)
                .resizable(true)
                .collapsible(false)
                .default_width(520.0)
                .show(ctx, |ui| {
                    let (exp, cls) = render_export_pdf_body(
                        ui,
                        config,
                        export_error,
                        entry_file,
                        active_engine_label,
                        fallback_notice,
                        dark_mode,
                    );
                    trigger_export |= exp;
                    close_clicked |= cls;
                });
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
                .show(ctx, |ui| {
                    let (exp, cls) = render_export_pdf_body(
                        ui,
                        config,
                        export_error,
                        entry_file,
                        active_engine_label,
                        fallback_notice,
                        dark_mode,
                    );
                    trigger_export |= exp;
                    close_clicked |= cls;
                });
        }
    });

    *open = is_open && !close_clicked;
    trigger_export
}

/// Renders the inner body of the Export PDF dialog and returns `(trigger_export, close_clicked)`.
pub fn render_export_pdf_body(
    ui: &mut Ui,
    config: &mut PdfExportConfig,
    export_error: &mut Option<String>,
    entry_file: &Path,
    active_engine_label: &str,
    fallback_notice: Option<&str>,
    dark_mode: bool,
) -> (bool, bool) {
    let palette = ThemePalette::for_mode(dark_mode);
    let card_bg = palette.editor_canvas;
    let border_col = palette.panel_border;
    let mut trigger_export = false;
    let mut close_clicked = false;

    ScrollArea::vertical()
        .id_salt("export_pdf_os_window_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(t!("export_pdf.target_document")).strong());
                ui.code(entry_file.display().to_string());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(
                        RichText::new(config.summary_badge())
                            .strong()
                            .color(Color32::from_rgb(56, 189, 248)),
                    );
                });
            });

            ui.add_space(6.0);

            // Section 1: PDF Version Selection (1.4, 1.5, 1.6, 1.7, 2.0)
            Frame::new()
                .fill(card_bg)
                .stroke(Stroke::new(1.0_f32, border_col))
                .corner_radius(CornerRadius::same(6))
                .inner_margin(Margin::symmetric(12, 10))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(t!("export_pdf.section_version"))
                            .strong()
                            .size(13.0),
                    );
                    ui.label(RichText::new(t!("export_pdf.version_desc")).small().weak());
                    ui.add_space(6.0);
                    ui.horizontal_wrapped(|ui| {
                        for &ver in PdfVersionChoice::ALL {
                            let selected = config.version == ver;
                            if ui
                                .selectable_label(
                                    selected,
                                    RichText::new(format!("PDF {}", ver.short_label())).strong(),
                                )
                                .on_hover_text(ver.label())
                                .clicked()
                            {
                                apply_pdf_version_choice(config, export_error, ver);
                            }
                        }
                    });
                });

            ui.add_space(8.0);

            // Section 2: PDF Standard(s) Selection (PDF/A-2a, PDF/A-3a, etc.)
            Frame::new()
                .fill(card_bg)
                .stroke(Stroke::new(1.0_f32, border_col))
                .corner_radius(CornerRadius::same(6))
                .inner_margin(Margin::symmetric(12, 10))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(t!("export_pdf.section_standard"))
                            .strong()
                            .size(13.0),
                    );
                    ui.label(RichText::new(t!("export_pdf.standard_desc")).small().weak());
                    ui.add_space(6.0);

                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(t!("export_pdf.quick_presets")).small());
                        let presets = [
                            PdfArchivalStandard::None,
                            PdfArchivalStandard::A2b,
                            PdfArchivalStandard::A2a,
                            PdfArchivalStandard::A3b,
                            PdfArchivalStandard::A3a,
                            PdfArchivalStandard::A1b,
                            PdfArchivalStandard::A1a,
                        ];
                        for std_choice in presets {
                            let is_sel = config.archival_standard == std_choice;
                            if ui
                                .selectable_label(is_sel, std_choice.code_label())
                                .on_hover_text(std_choice.label())
                                .clicked()
                            {
                                apply_archival_standard_choice(config, export_error, std_choice);
                            }
                        }
                    });

                    ui.add_space(6.0);

                    egui::ComboBox::from_id_salt("pdf_archival_standard_combo")
                        .width(460.0)
                        .selected_text(config.archival_standard.label())
                        .show_ui(ui, |ui| {
                            for &std_item in PdfArchivalStandard::ALL {
                                let is_selected = config.archival_standard == std_item;
                                if ui.selectable_label(is_selected, std_item.label()).clicked() {
                                    apply_archival_standard_choice(config, export_error, std_item);
                                }
                            }
                        });

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);

                    if ui
                        .checkbox(&mut config.pdf_ua1, t!("export_pdf.pdf_ua1_label"))
                        .changed()
                    {
                        apply_pdf_ua1_toggle(config, export_error);
                    }

                    let requires_tagged = standard_requires_tagged(config);
                    if requires_tagged {
                        config.tagged = true;
                    }
                    ui.add_enabled(
                        !requires_tagged,
                        egui::Checkbox::new(&mut config.tagged, t!("export_pdf.tagged_pdf_label")),
                    );
                });

            ui.add_space(8.0);

            match config.build_standards() {
                Ok(_) => {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(
                            RichText::new(t!("export_pdf.valid_config"))
                                .color(Color32::from_rgb(52, 211, 153))
                                .strong()
                                .small(),
                        );
                        ui.separator();
                        ui.label(
                            RichText::new(format!("Typst {active_engine_label}"))
                                .weak()
                                .small(),
                        );
                    });
                    if let Some(notice) = fallback_notice {
                        ui.label(
                            RichText::new(notice)
                                .color(Color32::from_rgb(245, 158, 11))
                                .small(),
                        );
                    }
                }
                Err(validation_err) => {
                    Frame::new()
                        .fill(if dark_mode {
                            Color32::from_rgb(45, 24, 18)
                        } else {
                            Color32::from_rgb(254, 242, 242)
                        })
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(248, 113, 113)))
                        .corner_radius(CornerRadius::same(5))
                        .inner_margin(Margin::symmetric(10, 6))
                        .show(ui, |ui| {
                            let err_text = t!(
                                "export_pdf.incompatible_config",
                                error = validation_err.to_string()
                            );
                            ui.label(
                                RichText::new(err_text)
                                    .color(Color32::from_rgb(248, 113, 113))
                                    .strong()
                                    .small(),
                            );
                            if let Some(rec) = config.archival_standard.recommended_version() {
                                let btn_label =
                                    t!("export_pdf.switch_compatible", version = rec.label());
                                if ui.small_button(btn_label).clicked() {
                                    apply_pdf_version_choice(config, export_error, rec);
                                }
                            }
                        });
                }
            }

            if let Some(ref err_msg) = *export_error {
                ui.add_space(4.0);
                let export_err_text = t!("export_pdf.export_error", error = err_msg);
                ui.label(
                    RichText::new(export_err_text)
                        .color(Color32::from_rgb(248, 113, 113))
                        .strong()
                        .small(),
                );
            }

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(4.0);

            let is_valid = config.build_standards().is_ok();
            ui.horizontal(|ui| {
                if ui.button(t!("export_pdf.reset_defaults")).clicked() {
                    *config = PdfExportConfig::default();
                    *export_error = None;
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let export_btn = egui::Button::new(
                        RichText::new(t!("export_pdf.export_btn"))
                            .strong()
                            .color(Color32::WHITE),
                    )
                    .fill(if is_valid {
                        Color32::from_rgb(37, 99, 235)
                    } else {
                        Color32::from_rgb(100, 116, 139)
                    })
                    .corner_radius(CornerRadius::same(5));

                    if ui.add_enabled(is_valid, export_btn).clicked() {
                        trigger_export = true;
                    }
                    if ui.button(t!("export_pdf.cancel_btn")).clicked() {
                        close_clicked = true;
                    }
                });
            });
        });

    (trigger_export, close_clicked)
}

#[cfg(test)]
#[path = "../../tests/ui/dialogs/export_pdf_tests.rs"]
mod tests;
