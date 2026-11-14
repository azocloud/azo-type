//! Unit tests for split divider pane geometry calculations.

use egui::{Pos2, Rect, Vec2};

use super::{compute_horizontal_split, compute_vertical_split};

#[test]
fn test_compute_horizontal_split_allocates_stacked_panes_and_divider() {
    let total = Rect::from_min_size(Pos2::new(10.0, 40.0), Vec2::new(250.0, 608.0));
    let split = compute_horizontal_split(total, 8.0, 110.0, 110.0, 0.5);

    assert!((split.avail_span - 600.0).abs() < 0.01);
    assert!((split.first_rect.height() - 300.0).abs() < 0.01);
    assert!((split.second_rect.height() - 300.0).abs() < 0.01);
    assert!((split.first_rect.width() - 250.0).abs() < 0.01);
    assert!((split.second_rect.width() - 250.0).abs() < 0.01);
    assert!((split.divider_center - (40.0 + 300.0 + 4.0)).abs() < 0.01);
    assert!((split.second_rect.top() - (40.0 + 300.0 + 8.0)).abs() < 0.01);
}

#[test]
fn test_compute_horizontal_split_clamps_extreme_ratios_to_min_pane_height() {
    let total = Rect::from_min_size(Pos2::ZERO, Vec2::new(220.0, 508.0));

    let too_small = compute_horizontal_split(total, 8.0, 110.0, 110.0, 0.01);
    assert!((too_small.first_rect.height() - 110.0).abs() < 0.01);
    assert!((too_small.second_rect.height() - 390.0).abs() < 0.01);

    let too_large = compute_horizontal_split(total, 8.0, 110.0, 110.0, 0.99);
    assert!((too_large.first_rect.height() - 390.0).abs() < 0.01);
    assert!((too_large.second_rect.height() - 110.0).abs() < 0.01);
}

#[test]
fn test_compute_vertical_split_allocates_side_by_side_panes_and_enforces_min_preview_width() {
    let total = Rect::from_min_size(Pos2::new(200.0, 30.0), Vec2::new(1004.0, 700.0));
    let split = compute_vertical_split(total, 4.0, 220.0, 340.0, 0.5);

    assert!((split.avail_span - 1000.0).abs() < 0.01);
    assert!((split.first_rect.width() - 500.0).abs() < 0.01);
    assert!((split.second_rect.width() - 500.0).abs() < 0.01);
    assert!((split.divider_center - (200.0 + 500.0 + 2.0)).abs() < 0.01);
    assert!((split.second_rect.left() - (200.0 + 500.0 + 4.0)).abs() < 0.01);

    // Dragging editor all the way right preserves minimum 340px preview width.
    let max_editor = compute_vertical_split(total, 4.0, 220.0, 340.0, 0.95);
    assert!((max_editor.second_rect.width() - 340.0).abs() < 0.01);
    assert!((max_editor.first_rect.width() - 660.0).abs() < 0.01);
}

#[test]
fn render_vertical_and_horizontal_split_dividers_draw_without_panicking() {
    use super::{render_horizontal_split_divider, render_vertical_split_divider};
    use crate::ui::theme::ThemePalette;

    let mut harness_v = egui_kittest::Harness::new_ui_state(
        |ui, v_ratio: &mut f32| {
            let ctx = ui.ctx().clone();
            let palette = ThemePalette::for_mode(true);
            let rect = ui.available_rect_before_wrap();
            let v_split = compute_vertical_split(rect, 4.0, 80.0, 80.0, *v_ratio);
            render_vertical_split_divider(&ctx, ui, &palette, rect, &v_split, v_ratio);
        },
        0.5_f32,
    );
    harness_v.run();

    let mut harness_h = egui_kittest::Harness::new_ui_state(
        |ui, h_ratio: &mut f32| {
            let ctx = ui.ctx().clone();
            let palette = ThemePalette::for_mode(false);
            let rect = ui.available_rect_before_wrap();
            let h_split = compute_horizontal_split(rect, 6.0, 60.0, 60.0, *h_ratio);
            render_horizontal_split_divider(
                &ctx, ui, &palette, "h_div", "Resize", rect, &h_split, 0.5, h_ratio,
            );
        },
        0.5_f32,
    );
    harness_h.run();
}
