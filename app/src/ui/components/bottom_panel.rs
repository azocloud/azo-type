//! Collapsible multi-tab bottom workspace panel rendered beneath the editor and preview panes.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use egui::{
    Align, Color32, CornerRadius, CursorIcon, Frame, Label, Layout, Margin, RichText, ScrollArea,
    Sense, Stroke, TextWrapMode, Ui, Vec2,
};
use rust_i18n::t;

use crate::core::diff::normalize_rel_path;
use crate::engine::DiagnosticItem;
use crate::project::DiskProject;
use crate::ui::theme::ThemePalette;

const MAX_CONSOLE_LOG_LINES: usize = 200;
const COLLAPSED_SUMMARY_MAX_CHARS: usize = 120;

/// Available tabs inside the collapsible content bottom panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BottomPanelTab {
    #[default]
    WarningsAndErrors,
    Console,
    AutoCorrection,
}

impl BottomPanelTab {
    /// Ordered list of tabs displayed in the bottom panel header bar.
    pub const ALL: &'static [Self] =
        &[Self::WarningsAndErrors, Self::Console, Self::AutoCorrection];

    /// Localized title for each tab.
    pub fn label(self, diagnostics_count: usize) -> String {
        match self {
            Self::WarningsAndErrors => t!(
                "bottom_panel.tab_warnings_errors",
                count = diagnostics_count
            )
            .to_string(),
            Self::Console => t!("bottom_panel.tab_console").to_string(),
            Self::AutoCorrection => t!("bottom_panel.tab_autocorrect").to_string(),
        }
    }
}

/// Persistent UI state for the collapsible bottom panel.
#[derive(Debug, Clone)]
pub struct BottomPanelState {
    pub visible: bool,
    pub split_ratio: f32,
    pub active_tab: BottomPanelTab,
    pub console_logs: Vec<String>,
    pub expanded_diagnostics: HashSet<u64>,
}

impl Default for BottomPanelState {
    fn default() -> Self {
        Self {
            visible: false,
            split_ratio: 0.72,
            active_tab: BottomPanelTab::default(),
            console_logs: Vec::new(),
            expanded_diagnostics: HashSet::new(),
        }
    }
}

impl BottomPanelState {
    /// Toggles whether the bottom panel is visible in the content area.
    pub fn toggle_visible(&mut self) {
        self.visible = !self.visible;
    }

    /// Toggles expand/collapse state for a single diagnostic card by fingerprint.
    pub fn toggle_diagnostic_expanded(&mut self, fingerprint: u64) {
        if !self.expanded_diagnostics.remove(&fingerprint) {
            self.expanded_diagnostics.insert(fingerprint);
        }
    }

    /// Returns whether the given diagnostic fingerprint is currently expanded.
    pub fn is_diagnostic_expanded(&self, fingerprint: u64) -> bool {
        self.expanded_diagnostics.contains(&fingerprint)
    }

    /// Appends a timestamped log message to the Console tab while keeping memory bounded.
    pub fn push_console_log(&mut self, msg: impl Into<String>) {
        let raw = msg.into();
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return;
        }
        if self
            .console_logs
            .last()
            .is_some_and(|last| last.ends_with(trimmed))
        {
            return;
        }
        let stamp = chrono::Local::now().format("%H:%M:%S");
        self.console_logs.push(format!("[{stamp}] {trimmed}"));
        if self.console_logs.len() > MAX_CONSOLE_LOG_LINES {
            let overflow = self.console_logs.len() - MAX_CONSOLE_LOG_LINES;
            self.console_logs.drain(0..overflow);
        }
    }

    /// Clears all buffered lines in the Console tab.
    pub fn clear_console(&mut self) {
        self.console_logs.clear();
    }
}

/// Parameters required to render the bottom panel.
pub struct BottomPanelParams<'a> {
    pub dark_mode: bool,
    pub state: &'a mut BottomPanelState,
    pub diagnostics: &'a [DiagnosticItem],
    pub active_file: &'a Path,
    pub language: &'a str,
}

/// User actions emitted by interactions inside the bottom panel.
#[derive(Debug, Default)]
pub struct BottomPanelOutput {
    pub close_requested: bool,
    pub open_diagnostic: Option<DiagnosticItem>,
}

/// Returns diagnostics ordered with all errors first, followed by all warnings.
pub fn sort_diagnostics_errors_first(diagnostics: &[DiagnosticItem]) -> Vec<&DiagnosticItem> {
    let mut ordered = Vec::with_capacity(diagnostics.len());
    ordered.extend(diagnostics.iter().filter(|d| d.is_error));
    ordered.extend(diagnostics.iter().filter(|d| !d.is_error));
    ordered
}

/// Computes a stable FNV-1a fingerprint for a diagnostic item to track expand/collapse state.
pub fn diagnostic_fingerprint(diag: &DiagnosticItem) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    let mut feed = |bytes: &[u8]| {
        for &b in bytes {
            hash ^= u64::from(b);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash ^= 0xff;
        hash = hash.wrapping_mul(0x100000001b3);
    };
    feed(&[u8::from(diag.is_error)]);
    feed(diag.file.to_string_lossy().as_bytes());
    feed(&diag.line.unwrap_or(0).to_le_bytes());
    feed(&diag.column.unwrap_or(0).to_le_bytes());
    feed(diag.message.as_bytes());
    hash
}

/// Produces a single-line capped summary of a diagnostic message and reports whether text was shortened.
pub fn truncate_diagnostic_summary(message: &str, max_chars: usize) -> (String, bool) {
    let normalized: String = message.split_whitespace().collect::<Vec<_>>().join(" ");
    let total_chars = normalized.chars().count();
    let had_multiline = message.contains('\n');
    if total_chars <= max_chars {
        return (normalized, had_multiline);
    }
    let keep = max_chars.saturating_sub(1);
    let prefix: String = normalized.chars().take(keep).collect();
    (format!("{prefix}…"), true)
}

/// Renders the multi-tab bottom panel inside the allocated content rect.
pub fn render_bottom_panel(ui: &mut Ui, params: BottomPanelParams<'_>) -> BottomPanelOutput {
    let mut output = BottomPanelOutput::default();
    let palette = ThemePalette::for_mode(params.dark_mode);
    let diag_count = params.diagnostics.len();

    Frame::new()
        .fill(palette.chrome_sidebar)
        .stroke(Stroke::new(1.0_f32, palette.panel_border))
        .corner_radius(CornerRadius::ZERO)
        .show(ui, |ui| {
            // Tab strip header
            Frame::new()
                .fill(palette.chrome_sidebar)
                .inner_margin(Margin::symmetric(10, 5))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        for &tab in BottomPanelTab::ALL {
                            let selected = params.state.active_tab == tab;
                            let text = RichText::new(tab.label(diag_count)).size(11.5).strong();
                            if ui.selectable_label(selected, text).clicked() {
                                params.state.active_tab = tab;
                            }
                        }

                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui
                                .small_button("✕")
                                .on_hover_text(t!("bottom_panel.close_tooltip"))
                                .clicked()
                            {
                                output.close_requested = true;
                            }
                            if params.state.active_tab == BottomPanelTab::Console
                                && !params.state.console_logs.is_empty()
                                && ui.small_button(t!("bottom_panel.clear_console")).clicked()
                            {
                                params.state.clear_console();
                            }
                        });
                    });
                });

            ui.separator();

            // Active tab body
            Frame::new()
                .fill(palette.editor_canvas)
                .inner_margin(Margin::symmetric(10, 6))
                .show(ui, |ui| {
                    ScrollArea::vertical()
                        .id_salt(("bottom_panel_body", params.state.active_tab as u8))
                        .auto_shrink([false, false])
                        .stick_to_bottom(params.state.active_tab == BottomPanelTab::Console)
                        .show(ui, |ui| match params.state.active_tab {
                            BottomPanelTab::WarningsAndErrors => {
                                render_diagnostics_tab(
                                    ui,
                                    params.dark_mode,
                                    &palette,
                                    params.state,
                                    params.diagnostics,
                                    &mut output.open_diagnostic,
                                );
                            }
                            BottomPanelTab::Console => {
                                render_console_tab(
                                    ui,
                                    params.dark_mode,
                                    &palette,
                                    &params.state.console_logs,
                                );
                            }
                            BottomPanelTab::AutoCorrection => {
                                render_autocorrect_tab(
                                    ui,
                                    &palette,
                                    params.active_file,
                                    params.language,
                                );
                            }
                        });
                });
        });

    output
}

fn render_diagnostics_tab(
    ui: &mut Ui,
    dark_mode: bool,
    palette: &ThemePalette,
    state: &mut BottomPanelState,
    diagnostics: &[DiagnosticItem],
    open_diagnostic: &mut Option<DiagnosticItem>,
) {
    if diagnostics.is_empty() {
        ui.label(
            RichText::new(t!("bottom_panel.no_diagnostics"))
                .small()
                .color(palette.text_muted),
        );
        return;
    }

    let ordered = sort_diagnostics_errors_first(diagnostics);
    for (idx, diag) in ordered.into_iter().enumerate() {
        let fp = diagnostic_fingerprint(diag);
        let expanded = state.is_diagnostic_expanded(fp);
        let (card_bg, card_border, accent_col, badge_fill, badge_txt) = if diag.is_error {
            (
                if dark_mode {
                    Color32::from_rgb(48, 22, 26)
                } else {
                    Color32::from_rgb(254, 242, 242)
                },
                Color32::from_rgb(220, 38, 38),
                Color32::from_rgb(239, 68, 68),
                Color32::from_rgb(220, 38, 38),
                Color32::WHITE,
            )
        } else {
            (
                if dark_mode {
                    Color32::from_rgb(46, 36, 16)
                } else {
                    Color32::from_rgb(255, 251, 235)
                },
                Color32::from_rgb(217, 119, 6),
                Color32::from_rgb(245, 158, 11),
                Color32::from_rgb(217, 119, 6),
                Color32::WHITE,
            )
        };

        let loc = match (diag.line, diag.column) {
            (Some(l), Some(c)) => format!("{}:{}:{}", diag.file.display(), l, c),
            (Some(l), None) => format!("{}:{}", diag.file.display(), l),
            _ => diag.file.display().to_string(),
        };

        let frame_resp = Frame::new()
            .fill(card_bg)
            .stroke(Stroke::new(1.0_f32, card_border.gamma_multiply(0.7)))
            .corner_radius(CornerRadius::same(4))
            .inner_margin(Margin {
                left: 12,
                right: 8,
                top: 5,
                bottom: 5,
            })
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    let badge_label = if diag.is_error {
                        t!("bottom_panel.error_badge")
                    } else {
                        t!("bottom_panel.warn_badge")
                    };
                    Frame::new()
                        .fill(badge_fill)
                        .corner_radius(CornerRadius::same(3))
                        .inner_margin(Margin::symmetric(5, 1))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(badge_label)
                                    .monospace()
                                    .size(10.0)
                                    .strong()
                                    .color(badge_txt),
                            );
                        });

                    if ui.link(RichText::new(&loc).monospace().small()).clicked() {
                        *open_diagnostic = Some(diag.clone());
                    }

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let arrow = if expanded { "▾" } else { "▸" };
                        let tip = if expanded {
                            t!("bottom_panel.collapse_tooltip")
                        } else {
                            t!("bottom_panel.expand_tooltip")
                        };
                        if ui.small_button(arrow).on_hover_text(tip).clicked() {
                            state.toggle_diagnostic_expanded(fp);
                        }
                    });
                });

                ui.push_id(("diag_body", idx, fp), |ui| {
                    let body_resp = if expanded {
                        ui.add(
                            Label::new(
                                RichText::new(&diag.message)
                                    .small()
                                    .color(palette.text_primary),
                            )
                            .wrap()
                            .sense(Sense::click()),
                        )
                    } else {
                        let (summary, _shortened) =
                            truncate_diagnostic_summary(&diag.message, COLLAPSED_SUMMARY_MAX_CHARS);
                        let hint_suffix = if diag.hints.is_empty() {
                            String::new()
                        } else {
                            format!(" (+{})", diag.hints.len())
                        };
                        ui.add(
                            Label::new(
                                RichText::new(format!("{summary}{hint_suffix}"))
                                    .small()
                                    .color(palette.text_primary),
                            )
                            .wrap_mode(TextWrapMode::Truncate)
                            .sense(Sense::click()),
                        )
                    };
                    let tip = if expanded {
                        t!("bottom_panel.collapse_tooltip")
                    } else {
                        t!("bottom_panel.expand_tooltip")
                    };
                    if body_resp
                        .on_hover_cursor(CursorIcon::PointingHand)
                        .on_hover_text(tip)
                        .clicked()
                    {
                        state.toggle_diagnostic_expanded(fp);
                    }
                });

                if expanded {
                    for hint in &diag.hints {
                        ui.label(
                            RichText::new(format!("hint: {hint}"))
                                .small()
                                .italics()
                                .color(palette.text_muted),
                        );
                    }
                }
            });

        // Paint distinct left severity accent strip along the card edge.
        let r = frame_resp.response.rect;
        let strip = egui::Rect::from_min_size(r.min, Vec2::new(4.0, r.height()));
        ui.painter()
            .rect_filled(strip, CornerRadius::same(2), accent_col);

        ui.add_space(4.0);
    }
}

fn render_console_tab(ui: &mut Ui, dark_mode: bool, palette: &ThemePalette, logs: &[String]) {
    if logs.is_empty() {
        ui.label(
            RichText::new(t!("bottom_panel.no_console_output"))
                .small()
                .color(palette.text_muted),
        );
        return;
    }

    for line in logs {
        let color = if line.contains("Failed")
            || line.contains("[ERROR]")
            || line.contains("failed")
        {
            Color32::from_rgb(248, 113, 113)
        } else if line.contains("Fallback") || line.contains("[WARN]") || line.contains("cancelled")
        {
            Color32::from_rgb(251, 191, 36)
        } else if line.contains("[PDF Export]") {
            if dark_mode {
                Color32::from_rgb(56, 189, 248)
            } else {
                Color32::from_rgb(3, 105, 161)
            }
        } else {
            palette.text_primary
        };
        ui.label(RichText::new(line).monospace().small().color(color));
    }
}

fn render_autocorrect_tab(ui: &mut Ui, palette: &ThemePalette, active_file: &Path, language: &str) {
    ui.label(
        RichText::new(t!("bottom_panel.autocorrect_heading"))
            .small()
            .strong(),
    );
    ui.add_space(4.0);
    ui.label(
        RichText::new(t!(
            "bottom_panel.autocorrect_active_doc",
            file = active_file.display().to_string(),
            locale = language
        ))
        .small()
        .monospace()
        .color(palette.text_muted),
    );
    ui.add_space(4.0);
    ui.label(
        RichText::new(t!("bottom_panel.autocorrect_placeholder"))
            .small()
            .color(palette.text_muted),
    );
}

/// Computes the character offset (`CCursor` index) corresponding to a diagnostic's line and column.
pub fn diagnostic_char_offset(project: &DiskProject, diag: &DiagnosticItem) -> usize {
    diag.line
        .and_then(|line_num| {
            let norm = normalize_rel_path(&diag.file.to_string_lossy());
            let buf = project.buffers.get(&PathBuf::from(norm))?;
            let txt = buf.text.as_deref()?;
            let mut offset = 0usize;
            for (idx, line) in txt.split_inclusive('\n').enumerate() {
                if idx + 1 >= line_num {
                    let col_chars = diag.column.unwrap_or(1).saturating_sub(1);
                    return Some(offset + col_chars.min(line.chars().count()));
                }
                offset += line.chars().count();
            }
            Some(offset)
        })
        .unwrap_or(0)
}

#[cfg(test)]
mod tests;
