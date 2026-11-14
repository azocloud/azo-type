//! In-editor remote peer cursor, selection highlight, and name-tag overlay painting.

use egui::text::CCursor;
use egui::text_edit::TextEditOutput;
use egui::{Color32, CornerRadius, FontFamily, FontId, Pos2, Rect, Stroke, Ui, Vec2};

use crate::collab::awareness::PeerPresence;

/// Paints remote peer cursors, selections, and floating name tags directly onto the `egui::TextEdit` canvas.
pub fn paint_remote_cursors_in_editor(
    ui: &Ui,
    output: &TextEditOutput,
    remote_peers_in_file: &[&PeerPresence],
) {
    if remote_peers_in_file.is_empty() {
        return;
    }

    let painter = ui.painter();
    let galley = &output.galley;
    let galley_pos = output.galley_pos;

    for peer in remote_peers_in_file {
        let Some(caret_char) = peer.cursor_char else {
            continue;
        };

        let peer_color = Color32::from_rgb(peer.color_rgb[0], peer.color_rgb[1], peer.color_rgb[2]);

        // 1. Draw translucent selection highlight if anchor != caret
        if let Some(anchor_char) = peer.anchor_char {
            if anchor_char != caret_char {
                let start_c = anchor_char.min(caret_char);
                let end_c = anchor_char.max(caret_char);
                let r_start = galley
                    .pos_from_cursor(&galley.from_ccursor(CCursor::new(start_c)))
                    .translate(galley_pos.to_vec2());
                let r_end = galley
                    .pos_from_cursor(&galley.from_ccursor(CCursor::new(end_c)))
                    .translate(galley_pos.to_vec2());

                let sel_fill = Color32::from_rgba_unmultiplied(
                    peer.color_rgb[0],
                    peer.color_rgb[1],
                    peer.color_rgb[2],
                    55,
                );

                let sel_rect = if (r_start.min.y - r_end.min.y).abs() < 2.0 {
                    Rect::from_min_max(
                        Pos2::new(r_start.min.x, r_start.min.y),
                        Pos2::new(r_end.min.x.max(r_start.min.x + 2.0), r_end.max.y),
                    )
                } else {
                    Rect::from_min_max(
                        Pos2::new(r_start.min.x.min(r_end.min.x), r_start.min.y),
                        Pos2::new(r_start.max.x.max(r_end.max.x + 40.0), r_end.max.y),
                    )
                };
                painter.rect_filled(sel_rect, CornerRadius::same(2), sel_fill);
            }
        }

        // 2. Draw vertical colored caret bar
        let caret_local = galley.pos_from_cursor(&galley.from_ccursor(CCursor::new(caret_char)));
        let caret_rect = caret_local.translate(galley_pos.to_vec2());
        let top = Pos2::new(caret_rect.min.x, caret_rect.min.y);
        let bottom = Pos2::new(caret_rect.min.x, caret_rect.max.y);
        painter.line_segment([top, bottom], Stroke::new(2.0_f32, peer_color));

        // 3. Draw peer name tag pill right above the caret
        let tag_font = FontId::new(10.0, FontFamily::Proportional);
        let tag_galley = painter.layout_no_wrap(peer.name.clone(), tag_font, Color32::BLACK);
        let tag_size = tag_galley.size() + Vec2::new(8.0, 3.0);
        let tag_min = Pos2::new(top.x, (top.y - tag_size.y).max(galley_pos.y));
        let tag_rect = Rect::from_min_size(tag_min, tag_size);

        painter.rect_filled(tag_rect, CornerRadius::same(3), peer_color);
        painter.galley(
            tag_rect.min + Vec2::new(4.0, 1.5),
            tag_galley,
            Color32::BLACK,
        );
    }
}

#[cfg(test)]
#[path = "../tests/collab/cursor_overlay_tests.rs"]
mod tests;
