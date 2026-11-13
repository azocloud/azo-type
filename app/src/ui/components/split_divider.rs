//! Resizable split dividers between the editor/preview panes and sidebar panels.

use egui::{Color32, Context, CornerRadius, CursorIcon, Pos2, Rect, Sense, Stroke, Ui, Vec2};

use crate::ui::theme::ThemePalette;

/// Computed pane rectangles and divider axis coordinate for a two-pane split container.
pub struct SplitGeometry {
    pub first_rect: Rect,
    pub second_rect: Rect,
    pub divider_center: f32,
    pub avail_span: f32,
}

/// Computes left/right pane rectangles for a vertical split divider inside `total_rect`.
pub fn compute_vertical_split(
    total_rect: Rect,
    spacing: f32,
    min_pane_w: f32,
    ratio: f32,
) -> SplitGeometry {
    let full_h = total_rect.height();
    let avail_span = (total_rect.width() - spacing).max(200.0);
    let min_w = min_pane_w.min(avail_span * 0.4);
    let max_w = (avail_span - min_w).max(min_w);
    let first_w = (avail_span * ratio).clamp(min_w, max_w);
    SplitGeometry {
        first_rect: Rect::from_min_size(total_rect.min, Vec2::new(first_w, full_h)),
        divider_center: total_rect.min.x + first_w + spacing * 0.5,
        second_rect: Rect::from_min_size(
            Pos2::new(total_rect.min.x + first_w + spacing, total_rect.min.y),
            Vec2::new((avail_span - first_w).max(min_w), full_h),
        ),
        avail_span,
    }
}

/// Computes top/bottom pane rectangles for a horizontal split divider inside `total_rect`.
pub fn compute_horizontal_split(
    total_rect: Rect,
    spacing: f32,
    min_pane_h: f32,
    ratio: f32,
) -> SplitGeometry {
    let full_w = total_rect.width();
    let avail_span = (total_rect.height() - spacing).max(160.0);
    let min_h = min_pane_h.min(avail_span * 0.4);
    let max_h = (avail_span - min_h).max(min_h);
    let first_h = (avail_span * ratio).clamp(min_h, max_h);
    SplitGeometry {
        first_rect: Rect::from_min_size(total_rect.min, Vec2::new(full_w, first_h)),
        divider_center: total_rect.min.y + first_h + spacing * 0.5,
        second_rect: Rect::from_min_size(
            Pos2::new(total_rect.min.x, total_rect.min.y + first_h + spacing),
            Vec2::new(full_w, (avail_span - first_h).max(min_h)),
        ),
        avail_span,
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
pub fn render_vertical_split_divider(
    ctx: &Context,
    ui: &mut Ui,
    palette: &ThemePalette,
    total_rect: Rect,
    avail_w: f32,
    div_x: f32,
    split_ratio: &mut f32,
) {
    let full_h = total_rect.height();
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
            let new_ratio = (pointer_pos.x - total_rect.min.x) / avail_w.max(1.0);
            *split_ratio = new_ratio.clamp(0.15, 0.85);
            ctx.request_repaint();
        }
    }
    if div_resp.double_clicked() {
        *split_ratio = 0.5;
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

/// Renders the interactive horizontal split divider and updates `split_ratio` on drag or double-click.
pub fn render_horizontal_split_divider(
    ctx: &Context,
    ui: &mut Ui,
    palette: &ThemePalette,
    total_rect: Rect,
    avail_h: f32,
    div_y: f32,
    split_ratio: &mut f32,
) {
    let full_w = total_rect.width();
    let divider_hit_rect = Rect::from_center_size(
        Pos2::new(total_rect.center().x, div_y),
        Vec2::new(full_w, 10.0),
    );
    let div_resp = ui
        .interact(
            divider_hit_rect,
            ui.id().with("sidebar_tree_outline_split_divider"),
            Sense::click_and_drag(),
        )
        .on_hover_text("Drag to resize Project Files and Outline panels (double-click to reset)");

    if div_resp.dragged() {
        if let Some(pointer_pos) = ctx.pointer_interact_pos() {
            let new_ratio = (pointer_pos.y - total_rect.min.y) / avail_h.max(1.0);
            *split_ratio = new_ratio.clamp(0.15, 0.85);
            ctx.request_repaint();
        }
    }
    if div_resp.double_clicked() {
        *split_ratio = 0.42;
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
