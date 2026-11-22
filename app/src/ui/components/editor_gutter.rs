//! Line-number gutter width computation, line-number/wrap-glyph painting, and active line highlight.

use egui::text::CCursor;
use egui::widgets::text_edit::TextEditOutput;
use egui::{Align2, CornerRadius, FontFamily, FontId, Pos2, Rect, Shape, Stroke, Ui};

use crate::ui::theme::ThemePalette;

/// Computes the editor line-number gutter width in points (`0.0` when disabled).
pub fn compute_gutter_width(line_count: usize, font_size: f32, show_line_numbers: bool) -> f32 {
    if !show_line_numbers {
        return 0.0;
    }
    let digits = line_count.max(1).to_string().len().max(3);
    (digits as f32 * (font_size * 0.62) + 18.0).max(40.0)
}

/// Paints gutter line numbers, soft-wrap continuation glyphs, gutter border, and active line highlight.
#[allow(clippy::too_many_arguments)]
pub fn paint_editor_gutter_and_active_line(
    ui: &Ui,
    edit_output: &TextEditOutput,
    active_line_shape_idx: egui::layers::ShapeIdx,
    active_ccursor: CCursor,
    gutter_x_max: f32,
    font_size: f32,
    show_line_numbers: bool,
    word_wrap: bool,
    palette: &ThemePalette,
) {
    let total_rows = edit_output.galley.rows.len();
    let mut logical_line = 1usize;
    let mut is_wrapped_continuation = false;
    let gutter_font = FontId::new(font_size, FontFamily::Monospace);
    let cont_font = FontId::new((font_size * 0.85).max(9.5), FontFamily::Monospace);
    let wrap_badge_font = FontId::new((font_size * 0.90).max(10.5), FontFamily::Monospace);
    let gutter_fg = palette.text_muted;

    for (row_idx, row) in edit_output.galley.rows.iter().enumerate() {
        let row_top_y = edit_output.galley_pos.y + row.min_y();
        let row_center_y = edit_output.galley_pos.y + row.rect.center().y;

        if show_line_numbers {
            let (label, font, col) = if !is_wrapped_continuation {
                (logical_line.to_string(), gutter_font.clone(), gutter_fg)
            } else {
                (
                    "↪".to_string(),
                    cont_font.clone(),
                    gutter_fg.gamma_multiply(0.65),
                )
            };
            ui.painter().text(
                Pos2::new(gutter_x_max - 8.0, row_top_y),
                Align2::RIGHT_TOP,
                label,
                font,
                col,
            );
        }

        if word_wrap && !row.ends_with_newline && (row_idx + 1 < total_rows) {
            let eol_x = (edit_output.galley_pos.x + row.rect.max.x + 4.0)
                .min(edit_output.response.rect.max.x - 12.0);
            ui.painter().text(
                Pos2::new(eol_x, row_center_y),
                Align2::LEFT_CENTER,
                "↩",
                wrap_badge_font.clone(),
                palette.syn_keyword,
            );
        }

        if row.ends_with_newline {
            logical_line += 1;
            is_wrapped_continuation = false;
        } else {
            is_wrapped_continuation = true;
        }
    }

    if show_line_numbers {
        let rect = edit_output.response.rect;
        ui.painter().line_segment(
            [
                Pos2::new(gutter_x_max, rect.min.y - 8.0),
                Pos2::new(gutter_x_max, rect.max.y + 8.0),
            ],
            Stroke::new(1.0_f32, palette.panel_border),
        );
    }

    let cursor_local_rect = edit_output
        .galley
        .pos_from_cursor(&edit_output.galley.from_ccursor(active_ccursor));
    let full_line_rect = Rect::from_min_max(
        Pos2::new(
            edit_output.response.rect.min.x - 10.0,
            edit_output.galley_pos.y + cursor_local_rect.min.y,
        ),
        Pos2::new(
            edit_output.response.rect.max.x + 10.0,
            edit_output.galley_pos.y + cursor_local_rect.max.y,
        ),
    );
    ui.painter().set(
        active_line_shape_idx,
        Shape::rect_filled(
            full_line_rect,
            CornerRadius::ZERO,
            palette.active_line_highlight,
        ),
    );
}
