//! Modal dialogs for creating new files and renaming existing files on disk.

use std::path::PathBuf;

use egui::{
    Align, CentralPanel, Color32, Context, CornerRadius, Frame, Key, Layout, Margin, RichText,
    TextEdit, Ui, ViewportBuilder, ViewportClass, ViewportId, Window,
};

use crate::ui::theme::{apply_theme, ThemePalette};

/// Renders the New File OS sub-window or embedded dialog.
///
/// Returns `Some(file_name)` when creation is confirmed by the user.
pub fn render_new_file_modal(
    ctx: &Context,
    open: &mut bool,
    file_name_buffer: &mut String,
    dark_mode: bool,
) -> Option<String> {
    if !*open {
        return None;
    }

    let mut is_open = *open;
    let mut close_clicked = false;
    let mut trigger_create = false;
    let window_bg = ThemePalette::for_mode(dark_mode).chrome_sidebar;

    let viewport_id = ViewportId::from_hash_of("azo_type_new_file_os_window");
    let builder = ViewportBuilder::default()
        .with_title("New File in Project")
        .with_inner_size([440.0, 160.0])
        .with_min_inner_size([360.0, 140.0]);

    ctx.show_viewport_immediate(viewport_id, builder, |ctx, class| {
        let mut render_body = |ui: &mut Ui| {
            ui.label(RichText::new("Create a new file inside the active project folder:").strong());
            ui.add_space(6.0);
            let resp = ui.add(
                TextEdit::singleline(file_name_buffer)
                    .desired_width(f32::INFINITY)
                    .hint_text("e.g. chapters/results.typ"),
            );
            let enter_pressed = resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
            if enter_pressed {
                trigger_create = true;
            }
            ui.add_space(10.0);
            ui.separator();
            ui.add_space(4.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let create_btn = egui::Button::new(
                    RichText::new("Create on Disk")
                        .strong()
                        .color(Color32::WHITE),
                )
                .fill(Color32::from_rgb(37, 99, 235))
                .corner_radius(CornerRadius::same(5));
                if ui.add(create_btn).clicked() {
                    trigger_create = true;
                }
                if ui.button("Cancel").clicked() {
                    close_clicked = true;
                }
            });
        };

        if class == ViewportClass::Embedded {
            Window::new("New File in Project")
                .open(&mut is_open)
                .resizable(false)
                .collapsible(false)
                .default_width(400.0)
                .show(ctx, |ui| render_body(ui));
        } else {
            apply_theme(ctx, dark_mode);
            if ctx.input(|i| i.viewport().close_requested()) {
                is_open = false;
            }
            CentralPanel::default()
                .frame(
                    Frame::new()
                        .fill(window_bg)
                        .inner_margin(Margin::symmetric(14, 12)),
                )
                .show(ctx, |ui| render_body(ui));
        }
    });

    *open = is_open && !close_clicked;
    if trigger_create {
        let res = file_name_buffer.trim().to_string();
        if !res.is_empty() {
            *open = false;
            return Some(res);
        }
    }
    None
}

/// Renders the Rename File OS sub-window or embedded dialog.
///
/// Returns `Some((target_file, new_name))` when renaming is confirmed.
pub fn render_rename_file_modal(
    ctx: &Context,
    target_file: &mut Option<PathBuf>,
    rename_input_buffer: &mut String,
    dark_mode: bool,
) -> Option<(PathBuf, String)> {
    let old_rel = target_file.as_ref()?.clone();

    let mut is_open = true;
    let mut close_clicked = false;
    let mut trigger_rename = false;
    let window_bg = ThemePalette::for_mode(dark_mode).chrome_sidebar;

    let viewport_id = ViewportId::from_hash_of("azo_type_rename_file_os_window");
    let builder = ViewportBuilder::default()
        .with_title("Rename File")
        .with_inner_size([440.0, 160.0])
        .with_min_inner_size([360.0, 140.0]);

    ctx.show_viewport_immediate(viewport_id, builder, |ctx, class| {
        let mut render_body = |ui: &mut Ui| {
            ui.label(RichText::new(format!("Rename '{}' to:", old_rel.display())).strong());
            ui.add_space(6.0);
            let resp =
                ui.add(TextEdit::singleline(rename_input_buffer).desired_width(f32::INFINITY));
            let enter_pressed = resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
            if enter_pressed {
                trigger_rename = true;
            }
            ui.add_space(10.0);
            ui.separator();
            ui.add_space(4.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let rename_btn = egui::Button::new(
                    RichText::new("Rename on Disk")
                        .strong()
                        .color(Color32::WHITE),
                )
                .fill(Color32::from_rgb(37, 99, 235))
                .corner_radius(CornerRadius::same(5));
                if ui.add(rename_btn).clicked() {
                    trigger_rename = true;
                }
                if ui.button("Cancel").clicked() {
                    close_clicked = true;
                }
            });
        };

        if class == ViewportClass::Embedded {
            Window::new("Rename File")
                .open(&mut is_open)
                .resizable(false)
                .collapsible(false)
                .default_width(400.0)
                .show(ctx, |ui| render_body(ui));
        } else {
            apply_theme(ctx, dark_mode);
            if ctx.input(|i| i.viewport().close_requested()) {
                is_open = false;
            }
            CentralPanel::default()
                .frame(
                    Frame::new()
                        .fill(window_bg)
                        .inner_margin(Margin::symmetric(14, 12)),
                )
                .show(ctx, |ui| render_body(ui));
        }
    });

    if !is_open || close_clicked {
        *target_file = None;
    }

    if trigger_rename {
        let new_name = rename_input_buffer.trim().to_string();
        if !new_name.is_empty() {
            *target_file = None;
            return Some((old_rel, new_name));
        }
    }

    None
}
