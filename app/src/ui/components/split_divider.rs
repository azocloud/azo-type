//! Resizable split dividers between the editor/preview panes and sidebar panels.

use egui::{Color32, Context, CornerRadius, CursorIcon, Pos2, Rect, Sense, Stroke, Ui, Vec2};

use crate::ui::theme::ThemePalette;

/// Computed pane rectangles and divider axis coordinate for a two-pane split container.
pub struct SplitGeometry {
    pub first_rect: Rect,
    pub second_rect: Rect,
    pub divider_center: f32,
    pub avail_span: f32,
    pub min_ratio: f32,
    pub max_ratio: f32,
}

fn clamp_split_span(
    avail_span: f32,
    min_first: f32,
    min_second: f32,
    ratio: f32,
) -> (f32, f32, f32) {
    let min_sum = (min_first + min_second).max(1.0);
    let scale = if min_sum > avail_span {
        avail_span / min_sum
    } else {
        1.0
    };
    let min_f = min_first * scale;
    let max_f = (avail_span - min_second * scale).max(min_f);
    let first_len = (avail_span * ratio).clamp(min_f, max_f);
    let min_ratio = (min_f / avail_span).clamp(0.05, 0.9);
    let max_ratio = (max_f / avail_span).clamp(min_ratio, 0.95);
    (first_len, min_ratio, max_ratio)
}

/// Computes left/right pane rectangles for a vertical split divider inside `total_rect`.
pub fn compute_vertical_split(
    total_rect: Rect,
    spacing: f32,
    min_left_w: f32,
    min_right_w: f32,
    ratio: f32,
) -> SplitGeometry {
    let full_h = total_rect.height();
    let avail_span = (total_rect.width() - spacing).max(200.0);
    let (first_w, min_ratio, max_ratio) =
        clamp_split_span(avail_span, min_left_w, min_right_w, ratio);
    SplitGeometry {
        first_rect: Rect::from_min_size(total_rect.min, Vec2::new(first_w, full_h)),
        divider_center: total_rect.min.x + first_w + spacing * 0.5,
        second_rect: Rect::from_min_size(
            Pos2::new(total_rect.min.x + first_w + spacing, total_rect.min.y),
            Vec2::new((avail_span - first_w).max(0.0), full_h),
        ),
        avail_span,
        min_ratio,
        max_ratio,
    }
}

/// Computes top/bottom pane rectangles for a horizontal split divider inside `total_rect`.
pub fn compute_horizontal_split(
    total_rect: Rect,
    spacing: f32,
    min_top_h: f32,
    min_bottom_h: f32,
    ratio: f32,
) -> SplitGeometry {
    let full_w = total_rect.width();
    let avail_span = (total_rect.height() - spacing).max(160.0);
    let (first_h, min_ratio, max_ratio) =
        clamp_split_span(avail_span, min_top_h, min_bottom_h, ratio);
    SplitGeometry {
        first_rect: Rect::from_min_size(total_rect.min, Vec2::new(full_w, first_h)),
        divider_center: total_rect.min.y + first_h + spacing * 0.5,
        second_rect: Rect::from_min_size(
            Pos2::new(total_rect.min.x, total_rect.min.y + first_h + spacing),
            Vec2::new(full_w, (avail_span - first_h).max(0.0)),
        ),
        avail_span,
        min_ratio,
        max_ratio,
    }
}

fn divider_colors(palette: &ThemePalette, active: bool) -> (Color32, f32, Color32) {
    let line_col = if active {
        palette.selection_stroke
    } else {
        palette.panel_border
    };
    let line_width = if active { 2.0_f32 } else { 1.0_f32 };
    let handle_col = if active {
        palette.selection_stroke
    } else {
        palette.text_muted.gamma_multiply(0.45)
    };
    (line_col, line_width, handle_col)
}

/// Renders the interactive vertical split divider and updates `split_ratio` on drag or double-click.
#[allow(clippy::too_many_arguments)]
pub fn render_vertical_split_divider(
    ctx: &Context,
    ui: &mut Ui,
    palette: &ThemePalette,
    total_rect: Rect,
    split: &SplitGeometry,
    split_ratio: &mut f32,
) {
    let full_h = total_rect.height();
    let div_x = split.divider_center;
    let divider_hit_rect = Rect::from_center_size(
        Pos2::new(div_x, total_rect.center().y),
        Vec2::new(10.0, full_h),
    );
    let div_resp = ui
        .interact(
            divider_hit_rect,
            ui.id().with("editor_preview_split_divider"),
            Sense::click_and_drag(),
        )
        .on_hover_text(
            "Drag to resize Code Editor and Live Preview panels (double-click to reset 50/50)",
        );

    if div_resp.dragged() {
        if let Some(pointer_pos) = ctx.pointer_interact_pos() {
            let new_ratio = (pointer_pos.x - total_rect.min.x) / split.avail_span.max(1.0);
            *split_ratio = new_ratio.clamp(split.min_ratio, split.max_ratio);
            ctx.request_repaint();
        }
    }
    if div_resp.double_clicked() {
        *split_ratio = (0.5_f32).clamp(split.min_ratio, split.max_ratio);
    }

    let div_active = div_resp.hovered() || div_resp.dragged();
    if div_active {
        ctx.set_cursor_icon(CursorIcon::ResizeHorizontal);
    }

    let (line_col, line_width, handle_col) = divider_colors(palette, div_active);

    ui.painter().line_segment(
        [
            Pos2::new(div_x, total_rect.min.y),
            Pos2::new(div_x, total_rect.max.y),
        ],
        Stroke::new(line_width, line_col),
    );

    let handle_rect = Rect::from_center_size(
        Pos2::new(div_x, total_rect.center().y),
        Vec2::new(4.0, 28.0),
    );
    ui.painter()
        .rect_filled(handle_rect, CornerRadius::same(2), handle_col);

    ui.advance_cursor_after_rect(total_rect);
}

/// Renders an interactive horizontal split divider and updates `split_ratio` on drag or double-click.
#[allow(clippy::too_many_arguments)]
pub fn render_horizontal_split_divider(
    ctx: &Context,
    ui: &mut Ui,
    palette: &ThemePalette,
    id_salt: &str,
    hover_text: &str,
    total_rect: Rect,
    split: &SplitGeometry,
    default_ratio: f32,
    split_ratio: &mut f32,
) {
    let full_w = total_rect.width();
    let div_y = split.divider_center;
    let divider_hit_rect = Rect::from_center_size(
        Pos2::new(total_rect.center().x, div_y),
        Vec2::new(full_w, 10.0),
    );
    let div_resp = ui
        .interact(
            divider_hit_rect,
            ui.id().with(id_salt),
            Sense::click_and_drag(),
        )
        .on_hover_text(hover_text);

    if div_resp.dragged() {
        if let Some(pointer_pos) = ctx.pointer_interact_pos() {
            let new_ratio = (pointer_pos.y - total_rect.min.y) / split.avail_span.max(1.0);
            *split_ratio = new_ratio.clamp(split.min_ratio, split.max_ratio);
            ctx.request_repaint();
        }
    }
    if div_resp.double_clicked() {
        *split_ratio = default_ratio.clamp(split.min_ratio, split.max_ratio);
    }

    let div_active = div_resp.hovered() || div_resp.dragged();
    if div_active {
        ctx.set_cursor_icon(CursorIcon::ResizeVertical);
    }

    let (line_col, line_width, handle_col) = divider_colors(palette, div_active);

    ui.painter().line_segment(
        [
            Pos2::new(total_rect.min.x, div_y),
            Pos2::new(total_rect.max.x, div_y),
        ],
        Stroke::new(line_width, line_col),
    );

    let handle_rect = Rect::from_center_size(
        Pos2::new(total_rect.center().x, div_y),
        Vec2::new(28.0, 4.0),
    );
    ui.painter()
        .rect_filled(handle_rect, CornerRadius::same(2), handle_col);

    ui.advance_cursor_after_rect(total_rect);
}

#[cfg(test)]
mod tests;
