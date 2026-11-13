//! Preview page coordinate mapping, glyph hit-testing, drag selection, and clipboard text extraction.

use std::path::PathBuf;

use egui::{Color32, CornerRadius, Painter, Pos2, Rect};

use crate::compiler::PreviewCharBox;
use crate::core::diff::CharSelection;

/// Hit-test radius in points when clicking on a preview page to jump to source.
pub const CLICK_HIT_THRESHOLD_PT: f32 = 24.0;

/// Hit-test radius in points when initiating a drag selection on a preview page.
pub const DRAG_START_THRESHOLD_PT: f32 = 36.0;

/// Endpoint of a text selection inside the rendered preview pages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreviewSelectionEndpoint {
    pub page_idx: usize,
    pub box_idx: usize,
}

/// Active click-and-drag text selection state across preview pages.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PreviewSelectionState {
    pub anchor: Option<PreviewSelectionEndpoint>,
    pub focus: Option<PreviewSelectionEndpoint>,
    pub is_dragging: bool,
}

impl PreviewSelectionState {
    /// Returns `true` when no preview text selection is currently active.
    pub fn is_empty(&self) -> bool {
        self.anchor.is_none() || self.focus.is_none()
    }

    /// Clears the active preview selection and resets drag state.
    pub fn clear(&mut self) {
        self.anchor = None;
        self.focus = None;
        self.is_dragging = false;
    }

    /// Returns `(start, end)` ordered by `(page_idx, box_idx)` in reading order.
    pub fn ordered_range(&self) -> Option<(PreviewSelectionEndpoint, PreviewSelectionEndpoint)> {
        let anchor = self.anchor?;
        let focus = self.focus?;
        if (anchor.page_idx, anchor.box_idx) <= (focus.page_idx, focus.box_idx) {
            Some((anchor, focus))
        } else {
            Some((focus, anchor))
        }
    }
}

/// Navigation and selection synchronization request emitted from the preview pane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewSourceJump {
    pub file: PathBuf,
    pub selection: CharSelection,
    pub scroll_to_char: usize,
}

/// Converts a screen position inside a rendered `page_rect` into page point (`pt`) coordinates.
pub fn screen_to_page_pt(
    pos: Pos2,
    page_rect: Rect,
    page_width_pt: f32,
    page_height_pt: f32,
) -> (f32, f32) {
    let x_pt = ((pos.x - page_rect.min.x) / page_rect.width().max(1e-3)) * page_width_pt;
    let y_pt = ((pos.y - page_rect.min.y) / page_rect.height().max(1e-3)) * page_height_pt;
    (x_pt, y_pt)
}

/// Converts page point (`pt`) bounds into a screen-space `egui::Rect` inside `page_rect`.
pub fn pt_bounds_to_screen_rect(
    x_min_pt: f32,
    y_min_pt: f32,
    x_max_pt: f32,
    y_max_pt: f32,
    page_rect: Rect,
    page_width_pt: f32,
    page_height_pt: f32,
) -> Rect {
    let sx = page_rect.width() / page_width_pt.max(1e-3);
    let sy = page_rect.height() / page_height_pt.max(1e-3);
    Rect::from_min_max(
        Pos2::new(
            page_rect.min.x + x_min_pt * sx,
            page_rect.min.y + y_min_pt * sy,
        ),
        Pos2::new(
            page_rect.min.x + x_max_pt * sx,
            page_rect.min.y + y_max_pt * sy,
        ),
    )
}

/// Converts a `PreviewCharBox` from page point (`pt`) coordinates into a screen-space `egui::Rect`.
pub fn page_pt_rect_to_screen(
    char_box: &PreviewCharBox,
    page_rect: Rect,
    page_width_pt: f32,
    page_height_pt: f32,
) -> Rect {
    pt_bounds_to_screen_rect(
        char_box.x_min_pt,
        char_box.y_min_pt,
        char_box.x_max_pt,
        char_box.y_max_pt,
        page_rect,
        page_width_pt,
        page_height_pt,
    )
}

/// Returns `true` if `(x_pt, y_pt)` lies inside `char_box` expanded by `pad_pt`.
pub fn contains_pt(char_box: &PreviewCharBox, x_pt: f32, y_pt: f32, pad_pt: f32) -> bool {
    x_pt >= char_box.x_min_pt - pad_pt
        && x_pt <= char_box.x_max_pt + pad_pt
        && y_pt >= char_box.y_min_pt - pad_pt
        && y_pt <= char_box.y_max_pt + pad_pt
}

fn rect_distance_sq(char_box: &PreviewCharBox, x_pt: f32, y_pt: f32, y_weight: f32) -> f32 {
    let dx = if x_pt < char_box.x_min_pt {
        char_box.x_min_pt - x_pt
    } else if x_pt > char_box.x_max_pt {
        x_pt - char_box.x_max_pt
    } else {
        0.0
    };
    let dy = if y_pt < char_box.y_min_pt {
        char_box.y_min_pt - y_pt
    } else if y_pt > char_box.y_max_pt {
        y_pt - char_box.y_max_pt
    } else {
        0.0
    };
    let wy = dy * y_weight;
    dx * dx + wy * wy
}

fn center_distance_sq(char_box: &PreviewCharBox, x_pt: f32, y_pt: f32) -> f32 {
    let cx = 0.5 * (char_box.x_min_pt + char_box.x_max_pt);
    let cy = 0.5 * (char_box.y_min_pt + char_box.y_max_pt);
    let dx = x_pt - cx;
    let dy = y_pt - cy;
    dx * dx + dy * dy
}

/// Finds the glyph box directly under `(x_pt, y_pt)`, or the nearest within `max_dist_pt`.
pub fn find_char_box_at(
    char_boxes: &[PreviewCharBox],
    x_pt: f32,
    y_pt: f32,
    max_dist_pt: f32,
) -> Option<usize> {
    let mut best_exact: Option<(usize, f32)> = None;
    for (idx, b) in char_boxes.iter().enumerate() {
        if contains_pt(b, x_pt, y_pt, 0.5) {
            let cdist = center_distance_sq(b, x_pt, y_pt);
            if best_exact.is_none_or(|(_, d)| cdist < d) {
                best_exact = Some((idx, cdist));
            }
        }
    }
    if let Some((idx, _)) = best_exact {
        return Some(idx);
    }

    let limit_sq = max_dist_pt * max_dist_pt;
    let mut best_near: Option<(usize, f32)> = None;
    for (idx, b) in char_boxes.iter().enumerate() {
        let dist_sq = rect_distance_sq(b, x_pt, y_pt, 1.0);
        if dist_sq <= limit_sq && best_near.is_none_or(|(_, d)| dist_sq < d) {
            best_near = Some((idx, dist_sq));
        }
    }
    best_near.map(|(idx, _)| idx)
}

/// Finds the best glyph box for an active drag endpoint, preferring glyphs on the same line band.
pub fn find_drag_char_box(char_boxes: &[PreviewCharBox], x_pt: f32, y_pt: f32) -> Option<usize> {
    if char_boxes.is_empty() {
        return None;
    }
    if let Some(exact) = find_char_box_at(char_boxes, x_pt, y_pt, 0.0) {
        return Some(exact);
    }

    let mut best_band: Option<(usize, f32)> = None;
    for (idx, b) in char_boxes.iter().enumerate() {
        if y_pt >= b.y_min_pt - 2.0 && y_pt <= b.y_max_pt + 2.0 {
            let dx = if x_pt < b.x_min_pt {
                b.x_min_pt - x_pt
            } else if x_pt > b.x_max_pt {
                x_pt - b.x_max_pt
            } else {
                0.0
            };
            if best_band.is_none_or(|(_, d)| dx < d) {
                best_band = Some((idx, dx));
            }
        }
    }
    if let Some((idx, _)) = best_band {
        return Some(idx);
    }

    let mut best_any: Option<(usize, f32)> = None;
    for (idx, b) in char_boxes.iter().enumerate() {
        let dist_sq = rect_distance_sq(b, x_pt, y_pt, 2.0);
        if best_any.is_none_or(|(_, d)| dist_sq < d) {
            best_any = Some((idx, dist_sq));
        }
    }
    best_any.map(|(idx, _)| idx)
}

/// Computes an editor `PreviewSourceJump` anchored to `anchor_box.file` across selected boxes.
pub fn compute_multifile_selection_jump(
    pages_boxes: &[&[PreviewCharBox]],
    selection: &PreviewSelectionState,
) -> Option<PreviewSourceJump> {
    let anchor = selection.anchor?;
    let focus = selection.focus?;
    let anchor_box = pages_boxes.get(anchor.page_idx)?.get(anchor.box_idx)?;
    let anchor_file = &anchor_box.file;
    let (start, end) = selection.ordered_range()?;

    let mut min_char = usize::MAX;
    let mut max_char = 0usize;
    let mut found_any = false;

    for p_idx in start.page_idx..=end.page_idx {
        let Some(boxes) = pages_boxes.get(p_idx) else {
            continue;
        };
        if boxes.is_empty() {
            continue;
        }
        let from = if p_idx == start.page_idx {
            start.box_idx.min(boxes.len() - 1)
        } else {
            0
        };
        let to = if p_idx == end.page_idx {
            end.box_idx.min(boxes.len() - 1)
        } else {
            boxes.len() - 1
        };
        for b in &boxes[from..=to] {
            if b.file == *anchor_file {
                min_char = min_char.min(b.char_idx);
                max_char = max_char.max(b.char_idx);
                found_any = true;
            }
        }
    }

    if !found_any {
        return None;
    }

    let forward = (anchor.page_idx, anchor.box_idx) <= (focus.page_idx, focus.box_idx);
    Some(PreviewSourceJump {
        file: anchor_file.clone(),
        selection: CharSelection {
            secondary_char: min_char,
            primary_char: max_char + 1,
        },
        scroll_to_char: if forward { max_char + 1 } else { min_char },
    })
}

/// Concatenates the plain text of all selected `PreviewCharBox` items for clipboard copying.
pub fn collect_selected_preview_text(
    pages_boxes: &[&[PreviewCharBox]],
    selection: &PreviewSelectionState,
) -> String {
    let Some((start, end)) = selection.ordered_range() else {
        return String::new();
    };
    let mut out = String::new();
    let mut prev_box: Option<&PreviewCharBox> = None;

    for p_idx in start.page_idx..=end.page_idx {
        let Some(boxes) = pages_boxes.get(p_idx) else {
            continue;
        };
        if boxes.is_empty() {
            continue;
        }
        let from = if p_idx == start.page_idx {
            start.box_idx.min(boxes.len() - 1)
        } else {
            0
        };
        let to = if p_idx == end.page_idx {
            end.box_idx.min(boxes.len() - 1)
        } else {
            boxes.len() - 1
        };
        if p_idx > start.page_idx && !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
            prev_box = None;
        }
        for b in &boxes[from..=to] {
            if b.text.is_empty() {
                continue;
            }
            if let Some(prev) = prev_box {
                let prev_h = (prev.y_max_pt - prev.y_min_pt).max(6.0);
                let curr_h = (b.y_max_pt - b.y_min_pt).max(6.0);
                let line_h = prev_h.max(curr_h);
                let prev_cy = 0.5 * (prev.y_min_pt + prev.y_max_pt);
                let curr_cy = 0.5 * (b.y_min_pt + b.y_max_pt);
                let is_new_line = b.y_min_pt >= prev.y_max_pt - prev_h * 0.3
                    || ((curr_cy - prev_cy) > line_h * 0.55 && b.x_min_pt < prev.x_max_pt);

                if is_new_line {
                    while out.ends_with(' ') {
                        out.pop();
                    }
                    if !out.is_empty() && !out.ends_with('\n') {
                        out.push('\n');
                    }
                } else if b.x_min_pt - prev.x_max_pt > line_h * 0.2
                    && !out.ends_with(char::is_whitespace)
                    && !b.text.starts_with(char::is_whitespace)
                {
                    out.push(' ');
                }
            }
            out.push_str(&b.text);
            prev_box = Some(b);
        }
    }
    out
}

/// Finds the rendered page rectangle containing `pos`, or the vertically nearest page.
pub(crate) fn find_target_page_for_pos(page_rects: &[Rect], pos: Pos2) -> Option<(usize, Rect)> {
    if page_rects.is_empty() {
        return None;
    }
    for (idx, rect) in page_rects.iter().enumerate() {
        if rect.contains(pos) {
            return Some((idx, *rect));
        }
    }
    page_rects
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            let dy_a = if pos.y < a.min.y {
                a.min.y - pos.y
            } else if pos.y > a.max.y {
                pos.y - a.max.y
            } else {
                0.0
            };
            let dy_b = if pos.y < b.min.y {
                b.min.y - pos.y
            } else if pos.y > b.max.y {
                pos.y - b.max.y
            } else {
                0.0
            };
            dy_a.total_cmp(&dy_b)
        })
        .map(|(idx, rect)| (idx, *rect))
}

/// Paints semi-transparent selection highlight rectangles over `boxes` on a preview page.
pub(crate) fn paint_char_box_selection(
    painter: &Painter,
    boxes: &[PreviewCharBox],
    page_rect: Rect,
    width_pt: f32,
    height_pt: f32,
    color: Color32,
) {
    let mut current: Option<(f32, f32, f32, f32)> = None;

    for b in boxes {
        if b.x_max_pt <= b.x_min_pt && b.y_max_pt <= b.y_min_pt {
            continue;
        }
        match current {
            Some((cx0, cy0, cx1, cy1))
                if (b.y_min_pt - cy0).abs() <= 2.5
                    && (b.y_max_pt - cy1).abs() <= 2.5
                    && b.x_min_pt <= cx1 + 4.0
                    && b.x_max_pt >= cx0 - 2.0 =>
            {
                current = Some((
                    cx0.min(b.x_min_pt),
                    cy0.min(b.y_min_pt),
                    cx1.max(b.x_max_pt),
                    cy1.max(b.y_max_pt),
                ));
            }
            Some((cx0, cy0, cx1, cy1)) => {
                let rect =
                    pt_bounds_to_screen_rect(cx0, cy0, cx1, cy1, page_rect, width_pt, height_pt);
                painter.rect_filled(rect, CornerRadius::ZERO, color);
                current = Some((b.x_min_pt, b.y_min_pt, b.x_max_pt, b.y_max_pt));
            }
            None => {
                current = Some((b.x_min_pt, b.y_min_pt, b.x_max_pt, b.y_max_pt));
            }
        }
    }

    if let Some((cx0, cy0, cx1, cy1)) = current {
        let rect = pt_bounds_to_screen_rect(cx0, cy0, cx1, cy1, page_rect, width_pt, height_pt);
        painter.rect_filled(rect, CornerRadius::ZERO, color);
    }
}

#[cfg(test)]
mod tests;
