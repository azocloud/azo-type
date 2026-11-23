//! Search & Replace tab inside the collapsible bottom workspace panel.

use egui::{Key, RichText, TextEdit, Ui};
use rust_i18n::t;

use crate::core::search::{
    find_all_matches, find_next_match_index, replace_all_matches, replace_match_at, SearchMatch,
    SearchOptions,
};
use crate::ui::components::bottom_panel::BottomPanelOutput;
use crate::ui::theme::ThemePalette;

const MIN_INPUT_WIDTH: f32 = 160.0;
const MAX_INPUT_WIDTH: f32 = 680.0;
const CONTROLS_RESERVE_WIDTH: f32 = 380.0;

/// Computes a responsive width for the Find and Replace inputs from the panel's
/// full available width before entering the layout grid, widening inputs on
/// desktop panels while keeping right-side controls visible on narrower viewports.
pub(crate) fn search_input_width(available_width: f32) -> f32 {
    let max_fitting = (available_width - CONTROLS_RESERVE_WIDTH).max(MIN_INPUT_WIDTH);
    (available_width * 0.50)
        .min(max_fitting)
        .clamp(MIN_INPUT_WIDTH, MAX_INPUT_WIDTH)
}

fn render_labeled_input(
    ui: &mut Ui,
    label_text: String,
    label_width: f32,
    row_height: f32,
    input_width: f32,
    value: &mut String,
    placeholder: &str,
    focus_requested: &mut bool,
) -> egui::Response {
    let label_resp = ui
        .allocate_ui_with_layout(
            egui::vec2(label_width, row_height),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| ui.label(RichText::new(label_text).small().strong()),
        )
        .inner;
    let resp = ui
        .add(
            TextEdit::singleline(value)
                .desired_width(input_width)
                .hint_text(placeholder),
        )
        .labelled_by(label_resp.id);
    if *focus_requested {
        resp.request_focus();
        *focus_requested = false;
    }
    resp
}

/// Persistent UI state for the Search & Replace tab in the bottom panel.
#[derive(Debug, Clone, Default)]
pub struct SearchAndReplaceState {
    pub query: String,
    pub replace_text: String,
    pub case_sensitive: bool,
    pub current_match_idx: usize,
    pub active_match_range: Option<(usize, usize)>,
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

    let input_width = search_input_width(ui.available_width());
    let search_label_text = t!("bottom_panel.search_label").to_string();
    let replace_label_text = t!("bottom_panel.replace_label").to_string();
    let label_font = egui::TextStyle::Small.resolve(ui.style());
    let label_width = [&search_label_text, &replace_label_text]
        .into_iter()
        .map(|text| {
            ui.painter()
                .layout_no_wrap(text.clone(), label_font.clone(), palette.text_primary)
                .size()
                .x
        })
        .fold(52.0_f32, f32::max)
        + 4.0;
    let row_height = ui.spacing().interact_size.y;

    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 6.0;

        // Row 1: Search input, case sensitivity, match count, Prev/Next buttons.
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;

            let search_placeholder = t!("bottom_panel.search_placeholder").to_string();
            let search_resp = render_labeled_input(
                ui,
                search_label_text,
                label_width,
                row_height,
                input_width,
                &mut state.query,
                &search_placeholder,
                &mut state.focus_search_requested,
            );

            let shift_held = ui.input(|i| i.modifiers.shift);
            let enter_in_search = (search_resp.lost_focus() || search_resp.has_focus())
                && ui.input(|i| i.key_pressed(Key::Enter));
            if enter_in_search {
                search_resp.request_focus();
            }

            let case_resp = ui.checkbox(&mut state.case_sensitive, t!("bottom_panel.match_case"));

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

        // Row 2: Replace input, Replace / Replace All buttons, status message.
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;

            let replace_placeholder = t!("bottom_panel.replace_placeholder").to_string();
            render_labeled_input(
                ui,
                replace_label_text,
                label_width,
                row_height,
                input_width,
                &mut state.replace_text,
                &replace_placeholder,
                &mut state.focus_replace_requested,
            );

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
    });

    if !matches.is_empty() {
        if next_triggered {
            state.status_message = None;
            state.current_match_idx = (state.current_match_idx + 1) % matches.len();
            let m = matches[state.current_match_idx];
            state.active_match_range = Some((m.start_char, m.end_char));
            output.jump_to_selection = Some((m.start_char, m.end_char));
        } else if prev_triggered {
            state.status_message = None;
            state.current_match_idx = if state.current_match_idx == 0 {
                matches.len() - 1
            } else {
                state.current_match_idx - 1
            };
            let m = matches[state.current_match_idx];
            state.active_match_range = Some((m.start_char, m.end_char));
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
                state.active_match_range = Some((m.start_char, m.end_char));
                output.jump_to_selection = Some((m.start_char, m.end_char));
            } else {
                state.current_match_idx = 0;
                state.active_match_range = None;
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
                state.active_match_range = None;
                state.status_message =
                    Some(t!("bottom_panel.replaced_all_status", count = count).to_string());
            }
        } else {
            let m = matches[state.current_match_idx];
            state.active_match_range = Some((m.start_char, m.end_char));
        }
    } else {
        state.active_match_range = None;
    }
}

#[cfg(test)]
#[path = "../../tests/ui/components/bottom_panel_search_tests.rs"]
mod tests;
