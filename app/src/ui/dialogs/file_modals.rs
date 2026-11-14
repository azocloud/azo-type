//! Modal dialogs for creating new files and renaming existing files on disk.

use std::path::{Path, PathBuf};

use egui::{
    Align, CentralPanel, Color32, Context, CornerRadius, Frame, Key, Layout, Margin, RichText,
    TextEdit, Ui, ViewportBuilder, ViewportClass, ViewportId, Window,
};
use rust_i18n::t;

use crate::ui::theme::{apply_theme, ThemePalette};

/// Validates and confirms a new file name, closing the modal when non-empty.
pub fn try_confirm_new_file(file_name_buffer: &str, open: &mut bool) -> Option<String> {
    let res = file_name_buffer.trim().to_string();
    if res.is_empty() {
        return None;
    }
    *open = false;
    Some(res)
}

/// Validates and confirms a rename target, clearing `target_file` when non-empty.
pub fn try_confirm_rename_file(
    target_file: &mut Option<PathBuf>,
    rename_input_buffer: &str,
) -> Option<(PathBuf, String)> {
    let new_name = rename_input_buffer.trim().to_string();
    if new_name.is_empty() {
        return None;
    }
    let old_rel = target_file.take()?;
    Some((old_rel, new_name))
}

/// Renders the New File modal body and returns `(trigger_create, close_clicked)`.
pub fn render_new_file_body(ui: &mut Ui, file_name_buffer: &mut String) -> (bool, bool) {
    let mut trigger_create = false;
    let mut close_clicked = false;
    let prompt_resp = ui.label(RichText::new(t!("file_modals.new_file_prompt")).strong());
    ui.add_space(6.0);
    let resp = ui
        .add(
            TextEdit::singleline(file_name_buffer)
                .desired_width(f32::INFINITY)
                .hint_text(t!("file_modals.new_file_hint")),
        )
        .labelled_by(prompt_resp.id);
    if resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
        trigger_create = true;
    }
    ui.add_space(10.0);
    ui.separator();
    ui.add_space(4.0);
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        let create_btn = egui::Button::new(
            RichText::new(t!("file_modals.create_btn"))
                .strong()
                .color(Color32::WHITE),
        )
        .fill(Color32::from_rgb(37, 99, 235))
        .corner_radius(CornerRadius::same(5));
        if ui.add(create_btn).clicked() {
            trigger_create = true;
        }
        if ui.button(t!("file_modals.cancel_btn")).clicked() {
            close_clicked = true;
        }
    });
    (trigger_create, close_clicked)
}

/// Renders the Rename File modal body and returns `(trigger_rename, close_clicked)`.
pub fn render_rename_file_body(
    ui: &mut Ui,
    old_rel: &Path,
    rename_input_buffer: &mut String,
) -> (bool, bool) {
    let mut trigger_rename = false;
    let mut close_clicked = false;
    let prompt = t!(
        "file_modals.rename_file_prompt",
        name = old_rel.display().to_string()
    );
    let prompt_resp = ui.label(RichText::new(prompt).strong());
    ui.add_space(6.0);
    let resp = ui
        .add(TextEdit::singleline(rename_input_buffer).desired_width(f32::INFINITY))
        .labelled_by(prompt_resp.id);
    if resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
        trigger_rename = true;
    }
    ui.add_space(10.0);
    ui.separator();
    ui.add_space(4.0);
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        let rename_btn = egui::Button::new(
            RichText::new(t!("file_modals.rename_btn"))
                .strong()
                .color(Color32::WHITE),
        )
        .fill(Color32::from_rgb(37, 99, 235))
        .corner_radius(CornerRadius::same(5));
        if ui.add(rename_btn).clicked() {
            trigger_rename = true;
        }
        if ui.button(t!("file_modals.cancel_btn")).clicked() {
            close_clicked = true;
        }
    });
    (trigger_rename, close_clicked)
}

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
    let builder = crate::platform::configure_viewport_builder(
        ViewportBuilder::default()
            .with_title(t!("file_modals.new_file_title"))
            .with_inner_size([440.0, 160.0])
            .with_min_inner_size([360.0, 140.0]),
    );

    ctx.show_viewport_immediate(viewport_id, builder, |ctx, class| {
        if class == ViewportClass::Embedded {
            Window::new(t!("file_modals.new_file_title"))
                .open(&mut is_open)
                .resizable(false)
                .collapsible(false)
                .default_width(400.0)
                .show(ctx, |ui| {
                    let (t_create, c_clicked) = render_new_file_body(ui, file_name_buffer);
                    trigger_create |= t_create;
                    close_clicked |= c_clicked;
                });
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
                .show(ctx, |ui| {
                    let (t_create, c_clicked) = render_new_file_body(ui, file_name_buffer);
                    trigger_create |= t_create;
                    close_clicked |= c_clicked;
                });
        }
    });

    *open = is_open && !close_clicked;
    if trigger_create {
        return try_confirm_new_file(file_name_buffer, open);
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
    let builder = crate::platform::configure_viewport_builder(
        ViewportBuilder::default()
            .with_title(t!("file_modals.rename_file_title"))
            .with_inner_size([440.0, 160.0])
            .with_min_inner_size([360.0, 140.0]),
    );

    ctx.show_viewport_immediate(viewport_id, builder, |ctx, class| {
        if class == ViewportClass::Embedded {
            Window::new(t!("file_modals.rename_file_title"))
                .open(&mut is_open)
                .resizable(false)
                .collapsible(false)
                .default_width(400.0)
                .show(ctx, |ui| {
                    let (t_ren, c_clicked) =
                        render_rename_file_body(ui, &old_rel, rename_input_buffer);
                    trigger_rename |= t_ren;
                    close_clicked |= c_clicked;
                });
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
                .show(ctx, |ui| {
                    let (t_ren, c_clicked) =
                        render_rename_file_body(ui, &old_rel, rename_input_buffer);
                    trigger_rename |= t_ren;
                    close_clicked |= c_clicked;
                });
        }
    });

    if !is_open || close_clicked {
        *target_file = None;
    }

    if trigger_rename {
        return try_confirm_rename_file(target_file, rename_input_buffer);
    }

    None
}

#[cfg(test)]
#[path = "../../tests/ui/dialogs/file_modals_tests.rs"]
mod tests;
