//! Live document page canvas, zoom controls, interactive text selection, and compiler diagnostics banner.

use std::path::PathBuf;

use egui::{
    Align, Color32, CornerRadius, CursorIcon, Event, Frame, Key, Layout, Margin, PointerButton,
    Pos2, Rect, RichText, ScrollArea, Sense, Stroke, StrokeKind, Ui, Vec2,
};
use rust_i18n::t;

use crate::compiler::{DiagnosticItem, PreviewCharBox, RenderedPageTexture};
use crate::core::diff::CharSelection;
use crate::ui::components::preview_selection::{
    collect_selected_preview_text, compute_multifile_selection_jump, contains_pt, find_char_box_at,
    find_drag_char_box, find_target_page_for_pos, paint_char_box_selection, screen_to_page_pt,
    PreviewSelectionEndpoint, CLICK_HIT_THRESHOLD_PT, DRAG_START_THRESHOLD_PT,
};
use crate::ui::theme::ThemePalette;

pub use crate::ui::components::preview_selection::{PreviewSelectionState, PreviewSourceJump};

/// Output actions triggered by user interactions in the preview pane.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PreviewPaneOutput {
    pub open_diagnostic_file: Option<PathBuf>,
    pub source_jump: Option<PreviewSourceJump>,
    pub copied_text: Option<String>,
}

/// Parameters for rendering the live preview canvas and diagnostics banner.
pub struct PreviewPaneParams<'a> {
    pub dark_mode: bool,
    pub zoom: &'a mut f32,
    pub page_textures: &'a [RenderedPageTexture],
    pub diagnostics: &'a [DiagnosticItem],
    pub selection: &'a mut PreviewSelectionState,
}

/// Renders the preview viewport and diagnostics panel.
pub fn render_preview_pane(ui: &mut Ui, params: PreviewPaneParams<'_>) -> PreviewPaneOutput {
    let mut output = PreviewPaneOutput::default();
    let palette = ThemePalette::for_mode(params.dark_mode);
    let canvas_bg = palette.preview_workspace;
    let border_col = palette.panel_border;
    let prev_selection = params.selection.clone();

    render_preview_header(ui, &palette, params.zoom, params.page_textures.len());

    let has_diagnostics = !params.diagnostics.is_empty();
    let available_height = ui.available_height();
    let banner_height = if has_diagnostics {
        (available_height * 0.28).clamp(110.0, 195.0)
    } else {
        0.0
    };
    let preview_height = (available_height - banner_height).max(120.0);

    let highlight_color = Color32::from_rgba_unmultiplied(
        palette.selection_stroke.r(),
        palette.selection_stroke.g(),
        palette.selection_stroke.b(),
        72,
    );

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
                                    RichText::new(t!("preview.no_pages"))
                                        .size(14.0)
                                        .color(palette.text_muted),
                                );
                            } else {
                                let avail_w = (ui.available_width() - 36.0).max(200.0);
                                let mut page_rects = Vec::with_capacity(params.page_textures.len());

                                for (page_idx, page) in params.page_textures.iter().enumerate() {
                                    let base_scale = (avail_w / page.width_pt).clamp(0.45, 1.35);
                                    let display_w = page.width_pt * base_scale * *params.zoom;
                                    let display_h = page.height_pt * base_scale * *params.zoom;
                                    let page_size = Vec2::new(display_w, display_h);

                                    let (rect, response) =
                                        ui.allocate_exact_size(page_size, Sense::click_and_drag());
                                    page_rects.push(rect);

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

                                    if let Some((start, end)) = params.selection.ordered_range() {
                                        if page_idx >= start.page_idx
                                            && page_idx <= end.page_idx
                                            && !page.char_boxes.is_empty()
                                        {
                                            let from = if page_idx == start.page_idx {
                                                start.box_idx.min(page.char_boxes.len() - 1)
                                            } else {
                                                0
                                            };
                                            let to = if page_idx == end.page_idx {
                                                end.box_idx.min(page.char_boxes.len() - 1)
                                            } else {
                                                page.char_boxes.len() - 1
                                            };
                                            paint_char_box_selection(
                                                painter,
                                                &page.char_boxes[from..=to],
                                                rect,
                                                page.width_pt,
                                                page.height_pt,
                                                highlight_color,
                                            );
                                        }
                                    }

                                    painter.rect_stroke(
                                        rect,
                                        CornerRadius::ZERO,
                                        Stroke::new(1.0_f32, border_col),
                                        StrokeKind::Outside,
                                    );

                                    handle_page_pointer_interaction(
                                        ui,
                                        page_idx,
                                        page,
                                        rect,
                                        &response,
                                        params.selection,
                                        &mut output,
                                    );

                                    ui.add_space(6.0);
                                    let page_str = t!(
                                        "preview.page_x_of_y",
                                        current = page.page_number,
                                        total = params.page_textures.len()
                                    );
                                    ui.label(
                                        RichText::new(page_str)
                                            .size(11.0)
                                            .color(palette.text_muted),
                                    );
                                    ui.add_space(18.0);
                                }

                                update_active_drag(
                                    ui,
                                    params.page_textures,
                                    &page_rects,
                                    params.selection,
                                );
                            }
                        });
                    });
            });
    });

    if params.selection.anchor != prev_selection.anchor
        || params.selection.focus != prev_selection.focus
    {
        let pages_boxes: Vec<&[PreviewCharBox]> = params
            .page_textures
            .iter()
            .map(|p| p.char_boxes.as_slice())
            .collect();
        if let Some(jump) = compute_multifile_selection_jump(&pages_boxes, params.selection) {
            output.source_jump = Some(jump);
        }
    }

    if !params.selection.is_empty() {
        if ui.input(|i| i.key_pressed(Key::Escape)) {
            params.selection.clear();
        } else if ui.input(|i| i.events.iter().any(|e| matches!(e, Event::Copy))) {
            let pages_boxes: Vec<&[PreviewCharBox]> = params
                .page_textures
                .iter()
                .map(|p| p.char_boxes.as_slice())
                .collect();
            let text = collect_selected_preview_text(&pages_boxes, params.selection);
            if !text.is_empty() {
                ui.ctx().output_mut(|o| {
                    o.commands
                        .retain(|cmd| !matches!(cmd, egui::OutputCommand::CopyText(_)));
                });
                ui.ctx().copy_text(text.clone());
                output.copied_text = Some(text);
            }
        }
    }

    if has_diagnostics {
        render_diagnostics_banner(
            ui,
            &palette,
            params.diagnostics,
            banner_height,
            &mut output.open_diagnostic_file,
        );
    }

    output
}

fn render_preview_header(ui: &mut Ui, palette: &ThemePalette, zoom: &mut f32, page_count: usize) {
    Frame::new()
        .fill(palette.chrome_sidebar)
        .stroke(Stroke::new(1.0_f32, palette.panel_border))
        .corner_radius(CornerRadius::ZERO)
        .inner_margin(Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(t!("preview.title"))
                        .size(11.5)
                        .strong()
                        .color(palette.text_muted),
                );

                ui.separator();
                let pages_str = t!("preview.pages_count", count = page_count);
                ui.label(
                    RichText::new(pages_str)
                        .size(12.0)
                        .strong()
                        .color(palette.text_primary),
                );

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .small_button(t!("preview.fit_width"))
                        .on_hover_text(t!("preview.fit_width_tooltip"))
                        .clicked()
                    {
                        *zoom = 1.0;
                    }
                    let scale_str = t!("preview.scale", percent = format!("{:.0}", *zoom * 100.0));
                    ui.label(
                        RichText::new(scale_str)
                            .size(11.5)
                            .monospace()
                            .color(palette.text_muted),
                    );
                });
            });
        });
}

fn handle_page_pointer_interaction(
    ui: &Ui,
    page_idx: usize,
    page: &RenderedPageTexture,
    rect: Rect,
    response: &egui::Response,
    selection: &mut PreviewSelectionState,
    output: &mut PreviewPaneOutput,
) {
    if response.hovered() {
        if let Some(hover_pos) = response.hover_pos() {
            let (x_pt, y_pt) = screen_to_page_pt(hover_pos, rect, page.width_pt, page.height_pt);
            if page
                .char_boxes
                .iter()
                .any(|b| contains_pt(b, x_pt, y_pt, 1.5))
            {
                ui.ctx().set_cursor_icon(CursorIcon::Text);
            }
        }
    }

    if response.clicked() {
        selection.clear();
        if let Some(click_pos) = response.interact_pointer_pos() {
            let (x_pt, y_pt) = screen_to_page_pt(click_pos, rect, page.width_pt, page.height_pt);
            if let Some(box_idx) =
                find_char_box_at(&page.char_boxes, x_pt, y_pt, CLICK_HIT_THRESHOLD_PT)
            {
                let b = &page.char_boxes[box_idx];
                output.source_jump = Some(PreviewSourceJump {
                    file: b.file.clone(),
                    selection: CharSelection::caret(b.char_idx),
                    scroll_to_char: b.char_idx,
                });
            }
        }
    }

    if response.drag_started_by(PointerButton::Primary) {
        let start_pos = ui
            .input(|i| i.pointer.press_origin())
            .or_else(|| response.interact_pointer_pos());
        if let Some(pos) = start_pos {
            let (x_pt, y_pt) = screen_to_page_pt(pos, rect, page.width_pt, page.height_pt);
            if let Some(box_idx) =
                find_char_box_at(&page.char_boxes, x_pt, y_pt, DRAG_START_THRESHOLD_PT)
            {
                let ep = PreviewSelectionEndpoint { page_idx, box_idx };
                selection.anchor = Some(ep);
                selection.focus = Some(ep);
                selection.is_dragging = true;
            }
        }
    }
}

fn update_active_drag(
    ui: &Ui,
    page_textures: &[RenderedPageTexture],
    page_rects: &[Rect],
    selection: &mut PreviewSelectionState,
) {
    if !selection.is_dragging {
        return;
    }
    let primary_down = ui.input(|i| i.pointer.primary_down());
    if let Some(pointer_pos) = ui
        .ctx()
        .pointer_interact_pos()
        .or_else(|| ui.input(|i| i.pointer.latest_pos()))
    {
        if let Some((target_page_idx, target_rect)) =
            find_target_page_for_pos(page_rects, pointer_pos)
        {
            let target_page = &page_textures[target_page_idx];
            let (x_pt, y_pt) = screen_to_page_pt(
                pointer_pos,
                target_rect,
                target_page.width_pt,
                target_page.height_pt,
            );
            if let Some(box_idx) = find_drag_char_box(&target_page.char_boxes, x_pt, y_pt) {
                selection.focus = Some(PreviewSelectionEndpoint {
                    page_idx: target_page_idx,
                    box_idx,
                });
            }
        }
    }
    if !primary_down {
        selection.is_dragging = false;
    }
}

fn render_diagnostics_banner(
    ui: &mut Ui,
    palette: &ThemePalette,
    diagnostics: &[DiagnosticItem],
    banner_height: f32,
    open_diagnostic_file: &mut Option<PathBuf>,
) {
    let error_count = diagnostics.iter().filter(|d| d.is_error).count();
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
                    t!("preview.diagnostics_errors", count = error_count)
                } else {
                    t!("preview.diagnostics_warnings", count = diagnostics.len())
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
                    for diag in diagnostics {
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
                                .small_button(RichText::new(&loc).monospace().color(badge_color))
                                .on_hover_text(t!("preview.open_file_tooltip"))
                                .clicked()
                            {
                                *open_diagnostic_file = Some(diag.file.clone());
                            }
                            ui.label(
                                RichText::new(&diag.message)
                                    .strong()
                                    .size(12.0)
                                    .color(palette.text_primary),
                            );
                        });
                        for hint in &diag.hints {
                            let hint_str = t!("preview.hint", hint = hint);
                            ui.label(
                                RichText::new(hint_str)
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
