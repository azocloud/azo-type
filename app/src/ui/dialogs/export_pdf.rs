//! Export PDF sub-window dialog for selecting PDF versions and PDF/A standards.

use std::path::Path;

use egui::{
    Align, CentralPanel, Color32, Context, CornerRadius, Frame, Layout, Margin, RichText,
    ScrollArea, Stroke, Ui, ViewportBuilder, ViewportClass, ViewportId, Window,
};

use crate::core::pdf::{PdfArchivalStandard, PdfExportConfig, PdfVersionChoice};
use crate::ui::theme::{apply_theme, ThemePalette};

/// Renders the Export PDF dialog either as an immediate-mode embedded window or a native OS sub-window.
///
/// Returns `true` if the user confirmed the export action.
pub fn render_export_pdf_dialog(
    ctx: &Context,
    open: &mut bool,
    config: &mut PdfExportConfig,
    export_error: &mut Option<String>,
    entry_file: &Path,
    dark_mode: bool,
) -> bool {
    if !*open {
        return false;
    }

    let palette = ThemePalette::for_mode(dark_mode);
    let card_bg = palette.editor_canvas;
    let window_bg = palette.chrome_sidebar;
    let border_col = palette.panel_border;

    let mut is_open = *open;
    let mut close_clicked = false;
    let mut trigger_export = false;

    let viewport_id = ViewportId::from_hash_of("azo_type_export_pdf_os_window");
    let builder = ViewportBuilder::default()
        .with_title("Export PDF")
        .with_inner_size([560.0, 480.0])
        .with_min_inner_size([460.0, 380.0]);

    ctx.show_viewport_immediate(viewport_id, builder, |ctx, class| {
        let mut render_body = |ui: &mut Ui| {
            ScrollArea::vertical()
                .id_salt("export_pdf_os_window_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Target Document:").strong());
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
                            ui.label(RichText::new("1. PDF Version").strong().size(13.0));
                            ui.label(
                                RichText::new(
                                    "Choose the target PDF specification version for the exported document:",
                                )
                                .small()
                                .weak(),
                            );
                            ui.add_space(6.0);
                            ui.horizontal_wrapped(|ui| {
                                for &ver in PdfVersionChoice::ALL {
                                    let selected = config.version == ver;
                                    if ui
                                        .selectable_label(
                                            selected,
                                            RichText::new(format!("PDF {}", ver.short_label()))
                                                .strong(),
                                        )
                                        .on_hover_text(ver.label())
                                        .clicked()
                                    {
                                        config.version = ver;
                                        *export_error = None;
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
                                RichText::new("2. PDF Archival & Conformance Standard (PDF/A)")
                                    .strong()
                                    .size(13.0),
                            );
                            ui.label(
                                RichText::new(
                                    "Select an ISO 19005 archival standard (such as PDF/A-2a, PDF/A-2b, or PDF/A-3a):",
                                )
                                .small()
                                .weak(),
                            );
                            ui.add_space(6.0);

                            // Quick Preset Buttons for common standards
                            ui.horizontal_wrapped(|ui| {
                                ui.label(RichText::new("Quick Presets:").small());
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
                                        config.archival_standard = std_choice;
                                        *export_error = None;
                                        if config.build_standards().is_err() {
                                            if let Some(rec) = std_choice.recommended_version() {
                                                config.version = rec;
                                            }
                                        }
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
                                        if ui
                                            .selectable_label(is_selected, std_item.label())
                                            .clicked()
                                        {
                                            config.archival_standard = std_item;
                                            *export_error = None;
                                            if config.build_standards().is_err() {
                                                if let Some(rec) = std_item.recommended_version() {
                                                    config.version = rec;
                                                }
                                            }
                                        }
                                    }
                                });

                            ui.add_space(8.0);
                            ui.separator();
                            ui.add_space(4.0);

                            if ui
                                .checkbox(
                                    &mut config.pdf_ua1,
                                    "Also enforce PDF/UA-1 (Universal Accessibility — ISO 14289-1)",
                                )
                                .changed()
                            {
                                *export_error = None;
                                if config.pdf_ua1 {
                                    config.tagged = true;
                                    if config.build_standards().is_err() {
                                        config.version = PdfVersionChoice::V1_7;
                                    }
                                }
                            }

                            let requires_tagged = config.pdf_ua1
                                || matches!(
                                    config.archival_standard,
                                    PdfArchivalStandard::A1a
                                        | PdfArchivalStandard::A2a
                                        | PdfArchivalStandard::A3a
                                );
                            if requires_tagged {
                                config.tagged = true;
                            }
                            ui.add_enabled(
                                !requires_tagged,
                                egui::Checkbox::new(
                                    &mut config.tagged,
                                    "Generate Tagged PDF (Document Accessibility Structure Tree)",
                                ),
                            );
                        });

                    ui.add_space(8.0);

                    // Validation status & CLI preview
                    match config.build_standards() {
                        Ok(_) => {
                            let mut std_flags = vec![config.version.short_label().to_string()];
                            if config.archival_standard != PdfArchivalStandard::None {
                                std_flags.push(
                                    config
                                        .archival_standard
                                        .code_label()
                                        .trim_start_matches("PDF/")
                                        .to_ascii_lowercase(),
                                );
                            }
                            if config.pdf_ua1 {
                                std_flags.push("ua-1".to_string());
                            }
                            ui.horizontal_wrapped(|ui| {
                                ui.label(
                                    RichText::new("✓ Valid Standard Configuration")
                                        .color(Color32::from_rgb(52, 211, 153))
                                        .strong()
                                        .small(),
                                );
                                ui.separator();
                                ui.code(format!(
                                    "typst compile --pdf-standard {} {}",
                                    std_flags.join(","),
                                    entry_file.display()
                                ));
                            });
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
                                    ui.label(
                                        RichText::new(format!(
                                            "Incompatible Version & Standard: {}",
                                            validation_err
                                        ))
                                        .color(Color32::from_rgb(248, 113, 113))
                                        .strong()
                                        .small(),
                                    );
                                    if let Some(rec) = config.archival_standard.recommended_version()
                                    {
                                        if ui
                                            .small_button(format!(
                                                "Switch to compatible {}",
                                                rec.label()
                                            ))
                                            .clicked()
                                        {
                                            config.version = rec;
                                            *export_error = None;
                                        }
                                    }
                                });
                        }
                    }

                    if let Some(ref err_msg) = *export_error {
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(format!("Export Error: {}", err_msg))
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
                        if ui.button("Reset Defaults").clicked() {
                            *config = PdfExportConfig::default();
                            *export_error = None;
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let export_btn = egui::Button::new(
                                RichText::new("Export PDF to Disk...")
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
                            if ui.button("Cancel").clicked() {
                                close_clicked = true;
                            }
                        });
                    });
                });
        };

        if class == ViewportClass::Embedded {
            Window::new("Export PDF")
                .open(&mut is_open)
                .resizable(true)
                .collapsible(false)
                .default_width(520.0)
                .show(ctx, |ui| {
                    render_body(ui);
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
                    render_body(ui);
                });
        }
    });

    *open = is_open && !close_clicked;
    trigger_export
}
