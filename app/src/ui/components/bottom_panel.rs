//! Collapsible multi-tab bottom workspace panel rendered beneath the editor and preview panes.

use std::path::{Path, PathBuf};

use egui::{Align, Color32, CornerRadius, Frame, Layout, Margin, RichText, ScrollArea, Stroke, Ui};
use rust_i18n::t;

use crate::core::diff::normalize_rel_path;
use crate::engine::DiagnosticItem;
use crate::project::DiskProject;
use crate::ui::theme::ThemePalette;

const MAX_CONSOLE_LOG_LINES: usize = 200;

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
}

impl Default for BottomPanelState {
    fn default() -> Self {
        Self {
            visible: false,
            split_ratio: 0.72,
            active_tab: BottomPanelTab::default(),
            console_logs: Vec::new(),
        }
    }
}

impl BottomPanelState {
    /// Toggles whether the bottom panel is visible in the content area.
    pub fn toggle_visible(&mut self) {
        self.visible = !self.visible;
    }

    /// Appends a log message to the Console tab while keeping memory bounded.
    pub fn push_console_log(&mut self, msg: impl Into<String>) {
        let entry = msg.into();
        if entry.trim().is_empty() {
            return;
        }
        if self.console_logs.last().is_some_and(|last| last == &entry) {
            return;
        }
        self.console_logs.push(entry);
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
                        .show(ui, |ui| match params.state.active_tab {
                            BottomPanelTab::WarningsAndErrors => {
                                render_diagnostics_tab(
                                    ui,
                                    &palette,
                                    params.diagnostics,
                                    &mut output.open_diagnostic,
                                );
                            }
                            BottomPanelTab::Console => {
                                render_console_tab(ui, &palette, &params.state.console_logs);
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
    palette: &ThemePalette,
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

    for diag in diagnostics {
        let severity_color = if diag.is_error {
            Color32::from_rgb(248, 113, 113)
        } else {
            Color32::from_rgb(251, 191, 36)
        };
        let badge = if diag.is_error { "ERR" } else { "WARN" };
        let loc = match (diag.line, diag.column) {
            (Some(l), Some(c)) => format!("{}:{}:{}", diag.file.display(), l, c),
            (Some(l), None) => format!("{}:{}", diag.file.display(), l),
            _ => diag.file.display().to_string(),
        };

        ui.horizontal_wrapped(|ui| {
            ui.label(
                RichText::new(badge)
                    .monospace()
                    .small()
                    .strong()
                    .color(severity_color),
            );
            if ui.link(RichText::new(&loc).monospace().small()).clicked() {
                *open_diagnostic = Some(diag.clone());
            }
            ui.label(RichText::new(&diag.message).small());
        });
        for hint in &diag.hints {
            ui.label(
                RichText::new(format!("  hint: {hint}"))
                    .small()
                    .italics()
                    .color(palette.text_muted),
            );
        }
    }
}

fn render_console_tab(ui: &mut Ui, palette: &ThemePalette, logs: &[String]) {
    if logs.is_empty() {
        ui.label(
            RichText::new(t!("bottom_panel.no_console_output"))
                .small()
                .color(palette.text_muted),
        );
        return;
    }

    for line in logs.iter().rev() {
        ui.label(RichText::new(line).monospace().small());
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
