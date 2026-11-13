//! Unit tests for split divider pane geometry calculations.

use egui::{Pos2, Rect, Vec2};

use super::{compute_horizontal_split, compute_vertical_split};

#[test]
fn test_compute_horizontal_split_allocates_stacked_panes_and_divider() {
    let total = Rect::from_min_size(Pos2::new(10.0, 40.0), Vec2::new(250.0, 608.0));
    let split = compute_horizontal_split(total, 8.0, 110.0, 0.5);

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

    let too_small = compute_horizontal_split(total, 8.0, 110.0, 0.01);
    assert!((too_small.first_rect.height() - 110.0).abs() < 0.01);
    assert!((too_small.second_rect.height() - 390.0).abs() < 0.01);

    let too_large = compute_horizontal_split(total, 8.0, 110.0, 0.99);
    assert!((too_large.first_rect.height() - 390.0).abs() < 0.01);
    assert!((too_large.second_rect.height() - 110.0).abs() < 0.01);
}

#[test]
fn test_compute_vertical_split_allocates_side_by_side_panes() {
    let total = Rect::from_min_size(Pos2::new(200.0, 30.0), Vec2::new(1004.0, 700.0));
    let split = compute_vertical_split(total, 4.0, 180.0, 0.5);

    assert!((split.avail_span - 1000.0).abs() < 0.01);
    assert!((split.first_rect.width() - 500.0).abs() < 0.01);
    assert!((split.second_rect.width() - 500.0).abs() < 0.01);
    assert!((split.divider_center - (200.0 + 500.0 + 2.0)).abs() < 0.01);
    assert!((split.second_rect.left() - (200.0 + 500.0 + 4.0)).abs() < 0.01);
}
