//! Unit tests for preview coordinate mapping, hit-testing, multi-file selection, and text extraction.

use std::path::PathBuf;

use egui::{Pos2, Rect};

use super::*;

fn mk_box(
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    file: &str,
    char_idx: usize,
    text: &str,
) -> PreviewCharBox {
    PreviewCharBox {
        x_min_pt: x0,
        y_min_pt: y0,
        x_max_pt: x1,
        y_max_pt: y1,
        file: PathBuf::from(file),
        char_idx,
        text: text.to_string(),
    }
}

#[test]
fn test_screen_and_page_pt_coordinate_roundtrip() {
    let page_rect = Rect::from_min_max(Pos2::new(100.0, 50.0), Pos2::new(700.0, 850.0));
    let width_pt = 300.0;
    let height_pt = 400.0;

    let char_box = mk_box(30.0, 40.0, 45.0, 52.0, "main.typ", 10, "A");
    let screen_rect = page_pt_rect_to_screen(&char_box, page_rect, width_pt, height_pt);

    // 2x zoom: (30,40)->(160,130), (45,52)->(190,154)
    assert!((screen_rect.min.x - 160.0).abs() < 1e-3);
    assert!((screen_rect.min.y - 130.0).abs() < 1e-3);
    assert!((screen_rect.max.x - 190.0).abs() < 1e-3);
    assert!((screen_rect.max.y - 154.0).abs() < 1e-3);

    let (pt_x, pt_y) = screen_to_page_pt(screen_rect.center(), page_rect, width_pt, height_pt);
    assert!((pt_x - 37.5).abs() < 1e-3);
    assert!((pt_y - 46.0).abs() < 1e-3);
}

#[test]
fn test_find_char_box_at_exact_nearest_and_threshold() {
    let boxes = vec![
        mk_box(50.0, 100.0, 58.0, 112.0, "main.typ", 0, "H"),
        mk_box(58.0, 100.0, 65.0, 112.0, "main.typ", 1, "i"),
    ];

    assert_eq!(find_char_box_at(&boxes, 54.0, 106.0, 20.0), Some(0));
    assert_eq!(find_char_box_at(&boxes, 62.0, 106.0, 20.0), Some(1));
    // Slightly right of 'i' within 20pt threshold hits nearest ('i')
    assert_eq!(find_char_box_at(&boxes, 72.0, 106.0, 20.0), Some(1));
    // Far outside threshold returns None
    assert_eq!(find_char_box_at(&boxes, 200.0, 200.0, 20.0), None);
}

#[test]
fn test_find_drag_char_box_prefers_same_horizontal_line_band() {
    let boxes = vec![
        mk_box(50.0, 100.0, 60.0, 112.0, "main.typ", 0, "A"),
        mk_box(60.0, 100.0, 70.0, 112.0, "main.typ", 1, "B"),
        // Next line slightly below with wider text extending to x=200
        mk_box(50.0, 116.0, 200.0, 128.0, "main.typ", 3, "LongLineTwo"),
    ];

    // Pointer in right margin of line 1 (x=190, y=106) should pick end of line 1 ('B'), not line 2
    assert_eq!(find_drag_char_box(&boxes, 190.0, 106.0), Some(1));
}

#[test]
fn test_multifile_selection_anchors_to_start_file_without_overflow() {
    let page_boxes = vec![
        mk_box(50.0, 100.0, 58.0, 112.0, "main.typ", 10, "H"),
        mk_box(58.0, 100.0, 66.0, 112.0, "main.typ", 11, "i"),
        // Included file starts right after on the same page with lower char indices (0, 1, 2)
        mk_box(50.0, 130.0, 58.0, 142.0, "chapters/ch1.typ", 0, "C"),
        mk_box(58.0, 130.0, 66.0, 142.0, "chapters/ch1.typ", 1, "h"),
        mk_box(66.0, 130.0, 74.0, 142.0, "chapters/ch1.typ", 2, "1"),
    ];

    // Forward drag from main.typ ('H') into chapters/ch1.typ ('1')
    let sel_forward = PreviewSelectionState {
        anchor: Some(PreviewSelectionEndpoint {
            page_idx: 0,
            box_idx: 0,
        }),
        focus: Some(PreviewSelectionEndpoint {
            page_idx: 0,
            box_idx: 4,
        }),
        is_dragging: false,
    };
    let jump_fwd = compute_multifile_selection_jump(&[&page_boxes], &sel_forward).unwrap();
    assert_eq!(jump_fwd.file, PathBuf::from("main.typ"));
    assert_eq!(
        jump_fwd.selection,
        CharSelection {
            secondary_char: 10,
            primary_char: 12,
        }
    );
    assert_eq!(jump_fwd.scroll_to_char, 12);

    // Reverse drag starting in chapters/ch1.typ ('1') upward into main.typ ('H')
    let sel_reverse = PreviewSelectionState {
        anchor: Some(PreviewSelectionEndpoint {
            page_idx: 0,
            box_idx: 4,
        }),
        focus: Some(PreviewSelectionEndpoint {
            page_idx: 0,
            box_idx: 0,
        }),
        is_dragging: false,
    };
    let jump_rev = compute_multifile_selection_jump(&[&page_boxes], &sel_reverse).unwrap();
    assert_eq!(jump_rev.file, PathBuf::from("chapters/ch1.typ"));
    assert_eq!(
        jump_rev.selection,
        CharSelection {
            secondary_char: 0,
            primary_char: 3,
        }
    );
    assert_eq!(jump_rev.scroll_to_char, 0);
}

#[test]
fn test_collect_selected_preview_text_handles_lines_gaps_and_pages() {
    let page0 = vec![
        mk_box(50.0, 100.0, 58.0, 112.0, "main.typ", 0, "A"),
        // Gap > 0.2 * 12pt -> inserts space
        mk_box(66.0, 100.0, 74.0, 112.0, "main.typ", 2, "B"),
        // New line
        mk_box(50.0, 120.0, 58.0, 132.0, "main.typ", 4, "C"),
    ];
    let page1 = vec![mk_box(50.0, 100.0, 58.0, 112.0, "main.typ", 6, "D")];

    let sel = PreviewSelectionState {
        anchor: Some(PreviewSelectionEndpoint {
            page_idx: 0,
            box_idx: 0,
        }),
        focus: Some(PreviewSelectionEndpoint {
            page_idx: 1,
            box_idx: 0,
        }),
        is_dragging: false,
    };

    let text = collect_selected_preview_text(&[&page0, &page1], &sel);
    assert_eq!(text, "A B\nC\nD");
}
