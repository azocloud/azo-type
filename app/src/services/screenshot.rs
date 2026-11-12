//! Headless screenshot capture automation for tests and CI documentation.

use std::path::Path;
use std::sync::Arc;

use egui::Context;

/// Manages automated frame rendering and screenshot saving when `--screenshot <path>` is supplied on CLI.
pub fn handle_screenshot_lifecycle(
    ctx: &Context,
    target_path: Option<&Path>,
    frame_count: &mut u32,
    screenshot_requested: &mut bool,
) {
    let Some(target_path) = target_path else {
        return;
    };

    ctx.request_repaint();
    *frame_count = frame_count.saturating_add(1);

    let mut captured_image: Option<Arc<egui::ColorImage>> = None;
    ctx.input(|i| {
        for event in &i.raw.events {
            if let egui::Event::Screenshot { image, .. } = event {
                captured_image = Some(Arc::clone(image));
            }
        }
    });

    if let Some(img) = captured_image {
        if let Some(parent) = target_path.parent() {
            if !parent.as_os_str().is_empty() {
                let _ = std::fs::create_dir_all(parent);
            }
        }
        let width = img.width() as u32;
        let height = img.height() as u32;
        let raw_rgba = img.as_raw();
        if let Err(err) = image::save_buffer(
            target_path,
            raw_rgba,
            width,
            height,
            image::ColorType::Rgba8,
        ) {
            eprintln!("Failed to save screenshot to {:?}: {}", target_path, err);
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        return;
    }

    if *frame_count >= 120 && !*screenshot_requested {
        *screenshot_requested = true;
        ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
    }
}
