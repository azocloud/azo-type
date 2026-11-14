//! Bottom status bar indicating compilation status, error counts, P2P peers, and active buffer stats.

use std::path::Path;

use egui::{
    Align, Color32, Context, CornerRadius, Frame, Layout, Margin, RichText, Stroke, TopBottomPanel,
};
use rust_i18n::t;

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
    pub active_typst_version: &'a str,
    pub downloading_typst_version: Option<&'a str>,
    pub typst_fallback_active: bool,
    pub collab_active: bool,
    pub collab_is_host: bool,
    pub collab_peer_count: usize,
    pub status_notice: &'a str,
    pub active_buffer: Option<StatusBarBufferInfo<'a>>,
}

/// User actions emitted by interactive items in the status bar.
#[derive(Debug, Default)]
pub struct StatusBarOutput {
    pub open_compiler_settings: bool,
}

/// Renders the bottom status bar panel across the application window.
pub fn render_status_bar(ctx: &Context, params: StatusBarParams<'_>) -> StatusBarOutput {
    let mut output = StatusBarOutput::default();
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
                        (palette.syn_operator, t!("status.compiling").to_string())
                    } else if params.error_count > 0 {
                        (
                            palette.syn_number,
                            t!("status.errors_count", count = params.error_count).to_string(),
                        )
                    } else {
                        (
                            Color32::from_rgb(0x44, 0x8C, 0x6B),
                            t!(
                                "status.compiled_duration",
                                duration = format!("{:.1}", params.compile_duration_ms)
                            )
                            .to_string(),
                        )
                    };

                ui.label(RichText::new(status_text).color(dot_color).strong().small());

                if params.warn_count > 0 {
                    ui.separator();
                    ui.label(
                        RichText::new(
                            t!("status.warnings_count", count = params.warn_count).to_string(),
                        )
                        .color(palette.syn_operator)
                        .small(),
                    );
                }

                ui.separator();
                ui.label(
                    RichText::new(
                        t!(
                            "status.main_file",
                            path = params.main_file.display().to_string()
                        )
                        .to_string(),
                    )
                    .color(palette.text_primary)
                    .small(),
                );

                ui.separator();
                ui.label(
                    RichText::new(t!("status.page_count", count = params.page_count).to_string())
                        .color(palette.text_primary)
                        .small(),
                );

                ui.separator();
                let engine_text = if let Some(dl_ver) = params.downloading_typst_version {
                    t!("status.typst_downloading", version = dl_ver).to_string()
                } else {
                    t!(
                        "status.typst_version",
                        version = params.active_typst_version
                    )
                    .to_string()
                };
                let engine_color =
                    if params.downloading_typst_version.is_some() || params.typst_fallback_active {
                        palette.syn_operator
                    } else {
                        palette.text_primary
                    };
                if ui
                    .button(RichText::new(engine_text).small().color(engine_color))
                    .on_hover_text(t!("status.typst_version_tooltip"))
                    .clicked()
                {
                    output.open_compiler_settings = true;
                }

                if params.collab_active {
                    ui.separator();
                    let role = if params.collab_is_host {
                        t!("status.role_host")
                    } else {
                        t!("status.role_peer")
                    };
                    ui.label(
                        RichText::new(
                            t!(
                                "status.collab_status",
                                role = role,
                                count = params.collab_peer_count
                            )
                            .to_string(),
                        )
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
                        let saved_state = if active.dirty {
                            t!("status.state_modified")
                        } else {
                            t!("status.state_saved")
                        };
                        ui.label(
                            RichText::new(
                                t!(
                                    "status.buffer_stats",
                                    path = active.rel_path.display().to_string(),
                                    state = saved_state,
                                    lines = active.line_count,
                                    bytes = active.size_bytes
                                )
                                .to_string(),
                            )
                            .small()
                            .monospace()
                            .color(palette.text_muted),
                        );
                    }
                });
            });
        });
    output
}

#[cfg(test)]
#[path = "../../tests/ui/components/status_bar_tests.rs"]
mod tests;
