//! Search & Replace tab inside the collapsible bottom workspace panel.

use egui::{Key, RichText, TextEdit, Ui};
use rust_i18n::t;

use crate::core::search::{
    find_all_matches, find_next_match_index, replace_all_matches, replace_match_at, SearchMatch,
    SearchOptions,
};
use crate::ui::components::bottom_panel::BottomPanelOutput;
use crate::ui::theme::ThemePalette;

const INPUT_WIDTH: f32 = 220.0;

/// Persistent UI state for the Search & Replace tab in the bottom panel.
#[derive(Debug, Clone, Default)]
pub struct SearchAndReplaceState {
    pub query: String,
    pub replace_text: String,
    pub case_sensitive: bool,
    pub current_match_idx: usize,
    pub focus_search_requested: bool,
    pub focus_replace_requested: bool,
    pub status_message: Option<String>,
}

/// Renders the Search & Replace tab controls and executes match navigation / replacement actions.
pub fn render_search_and_replace_tab(
    ui: &mut Ui,
    palette: &ThemePalette,
    state: &mut SearchAndReplaceState,
    active_text: &str,
    active_cursor_char: usize,
    output: &mut BottomPanelOutput,
) {
    let mut matches: Vec<SearchMatch> = Vec::new();
    let mut prev_triggered = false;
    let mut next_triggered = false;
    let mut replace_clicked = false;
    let mut replace_all_clicked = false;

    egui::Grid::new("bottom_panel_search_replace_grid")
        .num_columns(3)
        .spacing([8.0, 6.0])
        .show(ui, |ui| {
            // Row 1: Search input, case sensitivity, match count, Prev/Next buttons.
            let search_label = ui.label(
                RichText::new(t!("bottom_panel.search_label"))
                    .small()
                    .strong(),
            );
            let search_resp = ui
                .add(
                    TextEdit::singleline(&mut state.query)
                        .desired_width(INPUT_WIDTH)
                        .hint_text(t!("bottom_panel.search_placeholder")),
                )
                .labelled_by(search_label.id);
            if state.focus_search_requested {
                search_resp.request_focus();
                state.focus_search_requested = false;
            }

            let enter_in_search =
                search_resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
            let shift_held = ui.input(|i| i.modifiers.shift);
            if enter_in_search {
                search_resp.request_focus();
            }

            ui.horizontal(|ui| {
                let case_resp =
                    ui.checkbox(&mut state.case_sensitive, t!("bottom_panel.match_case"));

                matches = find_all_matches(
                    active_text,
                    &state.query,
                    SearchOptions {
                        case_sensitive: state.case_sensitive,
                    },
                );

                if search_resp.changed() || case_resp.changed() {
                    state.status_message = None;
                    if let Some(idx) = find_next_match_index(&matches, active_cursor_char, true) {
                        state.current_match_idx = idx;
                        let m = matches[idx];
                        output.jump_to_selection = Some((m.start_char, m.end_char));
                    } else {
                        state.current_match_idx = 0;
                    }
                } else if matches.is_empty() {
                    state.current_match_idx = 0;
                } else if state.current_match_idx >= matches.len() {
                    state.current_match_idx = matches.len() - 1;
                }

                let count_text = if state.query.is_empty() {
                    String::new()
                } else if matches.is_empty() {
                    t!("bottom_panel.no_matches").to_string()
                } else {
                    t!(
                        "bottom_panel.matches_count",
                        current = state.current_match_idx + 1,
                        total = matches.len()
                    )
                    .to_string()
                };
                if !count_text.is_empty() {
                    ui.label(
                        RichText::new(count_text)
                            .small()
                            .monospace()
                            .color(palette.text_muted),
                    );
                }

                let has_matches = !matches.is_empty();
                prev_triggered = ui
                    .add_enabled(
                        has_matches,
                        egui::Button::new(t!("bottom_panel.prev_match_btn")),
                    )
                    .clicked()
                    || (enter_in_search && shift_held);
                next_triggered = ui
                    .add_enabled(
                        has_matches,
                        egui::Button::new(t!("bottom_panel.next_match_btn")),
                    )
                    .clicked()
                    || (enter_in_search && !shift_held);
            });
            ui.end_row();

            // Row 2: Replace input, Replace / Replace All buttons, status message.
            let replace_label = ui.label(
                RichText::new(t!("bottom_panel.replace_label"))
                    .small()
                    .strong(),
            );
            let replace_resp = ui
                .add(
                    TextEdit::singleline(&mut state.replace_text)
                        .desired_width(INPUT_WIDTH)
                        .hint_text(t!("bottom_panel.replace_placeholder")),
                )
                .labelled_by(replace_label.id);
            if state.focus_replace_requested {
                replace_resp.request_focus();
                state.focus_replace_requested = false;
            }

            ui.horizontal(|ui| {
                let has_matches = !matches.is_empty();
                replace_clicked = ui
                    .add_enabled(
                        has_matches,
                        egui::Button::new(t!("bottom_panel.replace_btn")),
                    )
                    .clicked();
                replace_all_clicked = ui
                    .add_enabled(
                        has_matches,
                        egui::Button::new(t!("bottom_panel.replace_all_btn")),
                    )
                    .clicked();

                if let Some(status) = &state.status_message {
                    ui.label(RichText::new(status).small().color(palette.text_muted));
                }
            });
            ui.end_row();
        });

    if !matches.is_empty() {
        if next_triggered {
            state.status_message = None;
            state.current_match_idx = (state.current_match_idx + 1) % matches.len();
            let m = matches[state.current_match_idx];
            output.jump_to_selection = Some((m.start_char, m.end_char));
        } else if prev_triggered {
            state.status_message = None;
            state.current_match_idx = if state.current_match_idx == 0 {
                matches.len() - 1
            } else {
                state.current_match_idx - 1
            };
            let m = matches[state.current_match_idx];
            output.jump_to_selection = Some((m.start_char, m.end_char));
        } else if replace_clicked {
            state.status_message = None;
            let cur_match = matches[state.current_match_idx];
            let (updated, replacement_end) =
                replace_match_at(active_text, cur_match, &state.replace_text);
            output.replace_in_active_text = Some(updated.clone());
            let next_matches = find_all_matches(
                &updated,
                &state.query,
                SearchOptions {
                    case_sensitive: state.case_sensitive,
                },
            );
            if let Some(next_idx) = find_next_match_index(&next_matches, replacement_end, true) {
                state.current_match_idx = next_idx;
                let m = next_matches[next_idx];
                output.jump_to_selection = Some((m.start_char, m.end_char));
            } else {
                state.current_match_idx = 0;
                output.jump_to_char = Some(replacement_end);
            }
        } else if replace_all_clicked {
            if let Some((updated, count)) = replace_all_matches(
                active_text,
                &state.query,
                &state.replace_text,
                SearchOptions {
                    case_sensitive: state.case_sensitive,
                },
            ) {
                output.replace_in_active_text = Some(updated);
                state.current_match_idx = 0;
                state.status_message =
                    Some(t!("bottom_panel.replaced_all_status", count = count).to_string());
            }
        }
    }
}

#[cfg(test)]
#[path = "../../tests/ui/components/bottom_panel_search_tests.rs"]
mod tests;
