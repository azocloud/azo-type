//! Live document page canvas, zoom controls, and compiler diagnostics banner.

use std::path::PathBuf;

use egui::{
    Align, Color32, CornerRadius, Frame, Layout, Margin, Pos2, Rect, RichText, ScrollArea, Sense,
    Stroke, StrokeKind, Ui, Vec2,
};

use crate::compiler::{DiagnosticItem, RenderedPageTexture};
use crate::ui::theme::ThemePalette;

/// Parameters for rendering the live preview canvas and diagnostics banner.
pub struct PreviewPaneParams<'a> {
    pub dark_mode: bool,
    pub zoom: &'a mut f32,
    pub page_textures: &'a [RenderedPageTexture],
    pub diagnostics: &'a [DiagnosticItem],
}

/// Renders the preview viewport and diagnostics panel. Returns `Some(file_to_open)` if clicked.
pub fn render_preview_pane(ui: &mut Ui, params: PreviewPaneParams<'_>) -> Option<PathBuf> {
    let palette = ThemePalette::for_mode(params.dark_mode);
    let canvas_bg = palette.preview_workspace;
    let border_col = palette.panel_border;

    Frame::new()
        .fill(palette.chrome_sidebar)
        .stroke(Stroke::new(1.0_f32, border_col))
        .corner_radius(CornerRadius::ZERO)
        .inner_margin(Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("LIVE DOCUMENT PREVIEW")
                        .size(11.5)
                        .strong()
                        .color(palette.text_muted),
                );

                ui.separator();
                ui.label(
                    RichText::new(format!("{} Page(s)", params.page_textures.len()))
                        .size(12.0)
                        .strong()
                        .color(palette.text_primary),
                );

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .small_button("Fit Width")
                        .on_hover_text("Reset preview zoom to 100%")
                        .clicked()
                    {
                        *params.zoom = 1.0;
                    }
                    ui.label(
                        RichText::new(format!("Scale: {:.0}%", *params.zoom * 100.0))
                            .size(11.5)
                            .monospace()
                            .color(palette.text_muted),
                    );
                });
            });
        });

    let has_diagnostics = !params.diagnostics.is_empty();
    let available_height = ui.available_height();
    let banner_height = if has_diagnostics {
        (available_height * 0.28).clamp(110.0, 195.0)
    } else {
        0.0
    };
    let preview_height = (available_height - banner_height).max(120.0);

    ui.allocate_ui(Vec2::new(ui.available_width(), preview_height), |ui| {
        Frame::new()
            .fill(canvas_bg)
            .stroke(Stroke::new(1.0_f32, border_col))
            .corner_radius(CornerRadius::ZERO)
            .show(ui, |ui| {
                if ui.ui_contains_pointer() {
                    let zoom_delta = ui.input(|i| i.zoom_delta());
                    if (zoom_delta - 1.0).abs() > 0.001 {
                        *params.zoom = (*params.zoom * zoom_delta).clamp(0.4, 2.5);
                    }
                }

                ScrollArea::both()
                    .id_salt("typst_preview_scroll_area")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.vertical_centered(|ui| {
                            ui.add_space(18.0);

                            if params.page_textures.is_empty() {
                                ui.add_space(40.0);
                                ui.label(
                                    RichText::new("No rendered pages available.")
                                        .size(14.0)
                                        .color(palette.text_muted),
                                );
                            } else {
                                let avail_w = (ui.available_width() - 36.0).max(200.0);

                                for page in params.page_textures {
                                    let base_scale = (avail_w / page.width_pt).clamp(0.45, 1.35);
                                    let display_w = page.width_pt * base_scale * *params.zoom;
                                    let display_h = page.height_pt * base_scale * *params.zoom;
                                    let page_size = Vec2::new(display_w, display_h);

                                    let (rect, _response) =
                                        ui.allocate_exact_size(page_size, Sense::hover());

                                    let painter = ui.painter();
                                    let shadow_rect =
                                        rect.translate(Vec2::new(0.0, 2.0)).expand(1.0);
                                    painter.rect_filled(
                                        shadow_rect,
                                        CornerRadius::ZERO,
                                        Color32::from_black_alpha(if params.dark_mode {
                                            45
                                        } else {
                                            22
                                        }),
                                    );

                                    painter.rect_filled(rect, CornerRadius::ZERO, Color32::WHITE);
                                    painter.image(
                                        page.texture.id(),
                                        rect,
                                        Rect::from_min_max(
                                            Pos2::new(0.0, 0.0),
                                            Pos2::new(1.0, 1.0),
                                        ),
                                        Color32::WHITE,
                                    );
                                    painter.rect_stroke(
                                        rect,
                                        CornerRadius::ZERO,
                                        Stroke::new(1.0_f32, border_col),
                                        StrokeKind::Outside,
                                    );

                                    ui.add_space(6.0);
                                    ui.label(
                                        RichText::new(format!(
                                            "Page {} of {}",
                                            page.page_number,
                                            params.page_textures.len()
                                        ))
                                        .size(11.0)
                                        .color(palette.text_muted),
                                    );
                                    ui.add_space(18.0);
                                }
                            }
                        });
                    });
            });
    });

    let mut file_to_open: Option<PathBuf> = None;

    if has_diagnostics {
        let error_count = params.diagnostics.iter().filter(|d| d.is_error).count();
        let banner_bg = palette.chrome_sidebar;
        let banner_border = if error_count > 0 {
            palette.syn_number
        } else {
            palette.syn_operator
        };

        Frame::new()
            .fill(banner_bg)
            .stroke(Stroke::new(1.0_f32, banner_border))
            .corner_radius(CornerRadius::ZERO)
            .inner_margin(Margin::symmetric(10, 8))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let title = if error_count > 0 {
                        format!("[!] Typst Compiler Diagnostics ({} Error(s))", error_count)
                    } else {
                        format!("[!] Typst Compiler Warnings ({})", params.diagnostics.len())
                    };
                    ui.label(
                        RichText::new(title)
                            .strong()
                            .color(banner_border)
                            .size(12.5),
                    );
                });
                ui.separator();

                ScrollArea::vertical()
                    .id_salt("diagnostics_banner_scroll")
                    .max_height(banner_height - 38.0)
                    .show(ui, |ui| {
                        for diag in params.diagnostics {
                            ui.horizontal_wrapped(|ui| {
                                let badge_color = if diag.is_error {
                                    palette.syn_number
                                } else {
                                    palette.syn_operator
                                };
                                let loc = match (diag.line, diag.column) {
                                    (Some(l), Some(c)) => {
                                        format!("{}:{}:{}", diag.file.display(), l, c)
                                    }
                                    (Some(l), None) => format!("{}:{}", diag.file.display(), l),
                                    _ => diag.file.display().to_string(),
                                };
                                if ui
                                    .small_button(
                                        RichText::new(&loc).monospace().color(badge_color),
                                    )
                                    .on_hover_text("Click to open file in editor")
                                    .clicked()
                                {
                                    file_to_open = Some(diag.file.clone());
                                }
                                ui.label(
                                    RichText::new(&diag.message)
                                        .strong()
                                        .size(12.0)
                                        .color(palette.text_primary),
                                );
                            });
                            for hint in &diag.hints {
                                ui.label(
                                    RichText::new(format!("  Hint: {}", hint))
                                        .italics()
                                        .size(11.5)
                                        .color(palette.text_muted),
                                );
                            }
                            ui.add_space(3.0);
                        }
                    });
            });
    }

    file_to_open
}
