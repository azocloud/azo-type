//! Resizable vertical split divider between the code editor and live preview panes.

use egui::{Context, CornerRadius, CursorIcon, Pos2, Rect, Sense, Stroke, Ui, Vec2};

use crate::ui::theme::ThemePalette;

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

    let line_col = if div_active {
        palette.selection_stroke
    } else {
        palette.panel_border
    };
    let line_width = if div_active { 2.0_f32 } else { 1.0_f32 };

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
    ui.painter().rect_filled(
        handle_rect,
        CornerRadius::same(2),
        if div_active {
            palette.selection_stroke
        } else {
            palette.text_muted.gamma_multiply(0.45)
        },
    );

    ui.advance_cursor_after_rect(total_rect);
}
