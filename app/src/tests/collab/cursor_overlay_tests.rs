//! Unit tests for painting remote collaboration cursors, selections, and name tags.

use egui::TextEdit;
use egui_kittest::Harness;

use super::*;

#[test]
fn paints_single_line_and_multiline_remote_peer_cursors_without_panicking() {
    let mut text = "Hello world\nSecond line here\nThird line".to_string();
    let p_single = PeerPresence {
        client_id: 1,
        endpoint_id: "p1".into(),
        name: "Alice".into(),
        color_rgb: [255, 100, 100],
        active_file_id: None,
        active_path: Some("main.typ".into()),
        cursor_char: Some(5),
        anchor_char: Some(0),
    };
    let p_multi = PeerPresence {
        client_id: 2,
        endpoint_id: "p2".into(),
        name: "Bob".into(),
        color_rgb: [100, 200, 255],
        active_file_id: None,
        active_path: Some("main.typ".into()),
        cursor_char: Some(25),
        anchor_char: Some(8),
    };
    let p_none = PeerPresence {
        client_id: 3,
        endpoint_id: "p3".into(),
        name: "NoCursor".into(),
        color_rgb: [120, 120, 120],
        active_file_id: None,
        active_path: Some("main.typ".into()),
        cursor_char: None,
        anchor_char: None,
    };

    let mut harness = Harness::new_ui(|ui| {
        let edit_out = TextEdit::multiline(&mut text).show(ui);
        paint_remote_cursors_in_editor(ui, &edit_out, &[]);
        paint_remote_cursors_in_editor(ui, &edit_out, &[&p_single, &p_multi, &p_none]);
    });
    harness.run();
}
