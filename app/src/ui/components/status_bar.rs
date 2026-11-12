//! Bottom status bar indicating compilation status, error counts, P2P peers, and active buffer stats.

use std::path::Path;

use egui::{
    Align, Color32, Context, CornerRadius, Frame, Layout, Margin, RichText, Stroke, TopBottomPanel,
};

use crate::ui::theme::ThemePalette;

/// Summary statistics of the active open buffer for display on the right side of the status bar.
pub struct StatusBarBufferInfo<'a> {
    pub rel_path: &'a Path,
    pub dirty: bool,
    pub line_count: usize,
    pub size_bytes: u64,
}

/// Parameters for rendering the status bar.
pub struct StatusBarParams<'a> {
    pub dark_mode: bool,
    pub compile_in_progress: bool,
    pub pending_compile: bool,
    pub error_count: usize,
    pub warn_count: usize,
    pub compile_duration_ms: f64,
    pub main_file: &'a Path,
    pub page_count: usize,
    pub collab_active: bool,
    pub collab_is_host: bool,
    pub collab_peer_count: usize,
    pub status_notice: &'a str,
    pub active_buffer: Option<StatusBarBufferInfo<'a>>,
}

/// Renders the bottom status bar panel across the application window.
pub fn render_status_bar(ctx: &Context, params: StatusBarParams<'_>) {
    let palette = ThemePalette::for_mode(params.dark_mode);
    let bar_bg = palette.chrome_sidebar;
    let border_col = palette.panel_border;

    TopBottomPanel::bottom("bottom_status_bar")
        .frame(
            Frame::new()
                .fill(bar_bg)
                .corner_radius(CornerRadius::ZERO)
                .inner_margin(Margin::symmetric(12, 5))
                .stroke(Stroke::new(1.0_f32, border_col)),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                let (dot_color, status_text) =
                    if params.compile_in_progress || params.pending_compile {
                        (palette.syn_operator, "Compiling...".to_string())
                    } else if params.error_count > 0 {
                        (
                            palette.syn_number,
                            format!("{} Error(s)", params.error_count),
                        )
                    } else {
                        (
                            Color32::from_rgb(0x44, 0x8C, 0x6B),
                            format!("Compiled ({:.1} ms)", params.compile_duration_ms),
                        )
                    };

                ui.label(RichText::new(status_text).color(dot_color).strong().small());

                if params.warn_count > 0 {
                    ui.separator();
                    ui.label(
                        RichText::new(format!("{} Warning(s)", params.warn_count))
                            .color(palette.syn_operator)
                            .small(),
                    );
                }

                ui.separator();
                ui.label(
                    RichText::new(format!("Main: {}", params.main_file.display()))
                        .color(palette.text_primary)
                        .small(),
                );

                ui.separator();
                ui.label(
                    RichText::new(format!("Pages: {}", params.page_count))
                        .color(palette.text_primary)
                        .small(),
                );

                if params.collab_active {
                    ui.separator();
                    let role = if params.collab_is_host {
                        "Host"
                    } else {
                        "Peer"
                    };
                    ui.label(
                        RichText::new(format!(
                            "P2P {}: {} peer(s)",
                            role, params.collab_peer_count
                        ))
                        .small()
                        .strong()
                        .color(palette.syn_keyword),
                    );
                }

                ui.separator();
                ui.label(
                    RichText::new(params.status_notice)
                        .small()
                        .color(palette.text_muted),
                );

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if let Some(active) = params.active_buffer {
                        let saved_state = if active.dirty { "Modified" } else { "Saved" };
                        ui.label(
                            RichText::new(format!(
                                "{} ({})  |  {} lines  |  {} bytes",
                                active.rel_path.display(),
                                saved_state,
                                active.line_count,
                                active.size_bytes
                            ))
                            .small()
                            .monospace()
                            .color(palette.text_muted),
                        );
                    }
                });
            });
        });
}
