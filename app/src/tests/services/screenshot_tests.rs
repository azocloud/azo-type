//! Unit tests for headless screenshot capture lifecycle management.

use std::sync::Arc;

use egui::{ColorImage, Context, Event, RawInput, ViewportCommand};

use super::*;

#[test]
fn screenshot_lifecycle_noops_without_target_path() {
    let ctx = Context::default();
    let mut frames = 0;
    let mut requested = false;
    handle_screenshot_lifecycle(&ctx, None, &mut frames, &mut requested);
    assert_eq!(frames, 0);
    assert!(!requested);
}

#[test]
fn screenshot_lifecycle_requests_capture_at_threshold_and_writes_png() {
    let out_dir =
        std::env::temp_dir().join(format!("azo_type_screenshot_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out_dir);
    let out_file = out_dir.join("nested/shot.png");

    let ctx = Context::default();
    let mut frames = 119;
    let mut requested = false;

    let _ = ctx.run(RawInput::default(), |ctx| {
        handle_screenshot_lifecycle(ctx, Some(&out_file), &mut frames, &mut requested);
    });
    assert_eq!(frames, 120);
    assert!(requested);

    let img = Arc::new(ColorImage::new([2, 2], egui::Color32::RED));
    let raw = RawInput {
        events: vec![Event::Screenshot {
            viewport_id: egui::ViewportId::ROOT,
            user_data: egui::UserData::default(),
            image: img,
        }],
        ..Default::default()
    };

    let full_out = ctx.run(raw, |ctx| {
        handle_screenshot_lifecycle(ctx, Some(&out_file), &mut frames, &mut requested);
    });

    assert!(out_file.exists());
    assert!(full_out
        .viewport_output
        .values()
        .any(|v| v.commands.contains(&ViewportCommand::Close)));

    let _ = std::fs::remove_dir_all(&out_dir);
}
