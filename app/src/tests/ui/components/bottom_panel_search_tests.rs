//! Headless `egui_kittest` tests for the bottom panel Search & Replace tab.

use egui_kittest::{kittest::Queryable, Harness};
use rust_i18n::t;

use super::*;
use crate::ui::components::bottom_panel::BottomPanelOutput;
use crate::ui::theme::ThemePalette;

struct SearchHarnessState {
    search: SearchAndReplaceState,
    active_text: String,
    active_cursor_char: usize,
    last_jump_selection: Option<(usize, usize)>,
    last_jump_char: Option<usize>,
}

fn make_harness(
    initial_text: &str,
    search: SearchAndReplaceState,
) -> Harness<'static, SearchHarnessState> {
    make_sized_harness(800.0, 400.0, initial_text, search)
}

fn make_sized_harness(
    width: f32,
    height: f32,
    initial_text: &str,
    search: SearchAndReplaceState,
) -> Harness<'static, SearchHarnessState> {
    Harness::builder()
        .with_size(egui::vec2(width, height))
        .build_ui_state(
            |ui, state: &mut SearchHarnessState| {
                let palette = ThemePalette::for_mode(false);
                let mut output = BottomPanelOutput::default();
                render_search_and_replace_tab(
                    ui,
                    &palette,
                    &mut state.search,
                    &state.active_text,
                    state.active_cursor_char,
                    &mut output,
                );
                if let Some(updated) = output.replace_in_active_text {
                    state.active_text = updated;
                }
                if output.jump_to_selection.is_some() {
                    state.last_jump_selection = output.jump_to_selection;
                }
                if output.jump_to_char.is_some() {
                    state.last_jump_char = output.jump_to_char;
                }
            },
            SearchHarnessState {
                search,
                active_text: initial_text.to_string(),
                active_cursor_char: 0,
                last_jump_selection: None,
                last_jump_char: None,
            },
        )
}

#[test]
fn search_tab_shows_empty_and_non_matching_and_matching_counts() {
    let no_matches = t!("bottom_panel.no_matches", locale = "en").to_string();
    let mut harness = make_harness("Alpha beta alpha", SearchAndReplaceState::default());

    assert!(harness.query_by_label(&no_matches).is_none());

    harness.state_mut().search.query = "missing".to_string();
    harness.run();
    harness.get_by_label(&no_matches);

    harness.state_mut().search.query = "alpha".to_string();
    harness.run();
    let count_two = t!(
        "bottom_panel.matches_count",
        locale = "en",
        current = 1,
        total = 2
    )
    .to_string();
    harness.get_by_label(&count_two);

    let match_case = t!("bottom_panel.match_case", locale = "en").to_string();
    harness.get_by_label(&match_case).click();
    harness.run();
    let count_one = t!(
        "bottom_panel.matches_count",
        locale = "en",
        current = 1,
        total = 1
    )
    .to_string();
    harness.get_by_label(&count_one);
    assert_eq!(harness.state().last_jump_selection, Some((11, 16)));
}

#[test]
fn search_tab_next_and_prev_navigate_with_wrap_around() {
    let next_btn = t!("bottom_panel.next_match_btn", locale = "en").to_string();
    let prev_btn = t!("bottom_panel.prev_match_btn", locale = "en").to_string();

    let mut harness = make_harness(
        "foo one foo two foo",
        SearchAndReplaceState {
            query: "foo".to_string(),
            ..Default::default()
        },
    );

    harness.get_by_label(&next_btn).click();
    harness.run();
    assert_eq!(harness.state().search.current_match_idx, 1);
    assert_eq!(harness.state().last_jump_selection, Some((8, 11)));

    harness.get_by_label(&next_btn).click();
    harness.run();
    assert_eq!(harness.state().search.current_match_idx, 2);
    assert_eq!(harness.state().last_jump_selection, Some((16, 19)));

    // Wrap forward to index 0
    harness.get_by_label(&next_btn).click();
    harness.run();
    assert_eq!(harness.state().search.current_match_idx, 0);
    assert_eq!(harness.state().last_jump_selection, Some((0, 3)));

    // Wrap backward to index 2
    harness.get_by_label(&prev_btn).click();
    harness.run();
    assert_eq!(harness.state().search.current_match_idx, 2);
    assert_eq!(harness.state().last_jump_selection, Some((16, 19)));
}

#[test]
fn search_tab_replace_jumps_to_next_match_and_handles_final_match() {
    let replace_btn = t!("bottom_panel.replace_btn", locale = "en").to_string();

    let mut harness = make_harness(
        "cat and cat",
        SearchAndReplaceState {
            query: "cat".to_string(),
            replace_text: "dog".to_string(),
            ..Default::default()
        },
    );

    // Replace first match; jumps to second match (now at char 8..11).
    harness.get_by_label(&replace_btn).click();
    harness.run();
    assert_eq!(harness.state().active_text, "dog and cat");
    assert_eq!(harness.state().last_jump_selection, Some((8, 11)));

    // Replace final match; no matches remain so cursor jumps to replacement end (11).
    harness.state_mut().last_jump_selection = None;
    harness.get_by_label(&replace_btn).click();
    harness.run();
    assert_eq!(harness.state().active_text, "dog and dog");
    assert_eq!(harness.state().last_jump_selection, None);
    assert_eq!(harness.state().last_jump_char, Some(11));
}

#[test]
fn search_tab_replace_all_updates_text_and_status_count() {
    let replace_all_btn = t!("bottom_panel.replace_all_btn", locale = "en").to_string();

    let mut harness = make_harness(
        "typst is fast; Typst is great",
        SearchAndReplaceState {
            query: "typst".to_string(),
            replace_text: "Azo".to_string(),
            ..Default::default()
        },
    );

    harness.get_by_label(&replace_all_btn).click();
    harness.run();

    assert_eq!(harness.state().active_text, "Azo is fast; Azo is great");
    let expected_status =
        t!("bottom_panel.replaced_all_status", locale = "en", count = 2).to_string();
    assert_eq!(
        harness.state().search.status_message.as_deref(),
        Some(expected_status.as_str())
    );
    harness.get_by_label(&expected_status);
}

#[test]
fn search_tab_focus_requests_and_enter_navigation_work() {
    let mut harness = make_harness(
        "one two one three one",
        SearchAndReplaceState {
            query: "one".to_string(),
            focus_search_requested: true,
            ..Default::default()
        },
    );

    assert!(!harness.state().search.focus_search_requested);
    let search_label = t!("bottom_panel.search_label", locale = "en").to_string();
    let replace_label = t!("bottom_panel.replace_label", locale = "en").to_string();
    assert!(harness.get_by_label(&search_label).is_focused());

    // Enter jumps to next match and preserves search focus.
    harness.press_key(egui::Key::Enter);
    harness.run();
    assert_eq!(harness.state().search.current_match_idx, 1);
    assert_eq!(harness.state().last_jump_selection, Some((8, 11)));
    assert!(harness.get_by_label(&search_label).is_focused());

    // Shift+Enter jumps to previous match.
    harness.press_key_modifiers(egui::Modifiers::SHIFT, egui::Key::Enter);
    harness.run();
    assert_eq!(harness.state().search.current_match_idx, 0);
    assert_eq!(harness.state().last_jump_selection, Some((0, 3)));

    // focus_replace_requested focuses the Replace input and clears its flag.
    harness.state_mut().search.focus_replace_requested = true;
    harness.run();
    assert!(!harness.state().search.focus_replace_requested);
    assert!(harness.get_by_label(&replace_label).is_focused());
}

#[test]
fn search_input_width_scales_responsively_across_viewports() {
    let wide = search_input_width(1400.0);
    let desktop = search_input_width(1100.0);
    let narrow = search_input_width(600.0);
    let very_narrow = search_input_width(400.0);

    assert!(wide >= 660.0, "expected >= 660px on wide panel, got {wide}");
    assert!(
        desktop >= 440.0,
        "expected >= 440px on 1100px panel, got {desktop}"
    );
    assert!(narrow < desktop);
    assert!(narrow >= 160.0);
    assert_eq!(very_narrow, 160.0);
}

fn assert_search_tab_layout_fits(panel_width: f32, min_input_width: f64, max_input_width: f64) {
    let mut harness = make_sized_harness(
        panel_width,
        240.0,
        "alpha beta alpha gamma alpha",
        SearchAndReplaceState {
            query: "alpha".to_string(),
            replace_text: "omega".to_string(),
            ..Default::default()
        },
    );
    harness.run();

    let search_label = t!("bottom_panel.search_label", locale = "en").to_string();
    let replace_label = t!("bottom_panel.replace_label", locale = "en").to_string();
    let match_case = t!("bottom_panel.match_case", locale = "en").to_string();
    let prev_btn = t!("bottom_panel.prev_match_btn", locale = "en").to_string();
    let next_btn = t!("bottom_panel.next_match_btn", locale = "en").to_string();
    let replace_btn = t!("bottom_panel.replace_btn", locale = "en").to_string();
    let replace_all_btn = t!("bottom_panel.replace_all_btn", locale = "en").to_string();

    for (name, label) in [("Find", search_label), ("Replace", replace_label)] {
        let bounds = harness
            .get_by_label(&label)
            .bounding_box()
            .unwrap_or_else(|| panic!("{name} input should have bounding box"));
        let width = bounds.width();
        assert!(
            (min_input_width..=max_input_width).contains(&width),
            "expected {name} input width in {min_input_width}..={max_input_width}, got {width}"
        );
    }

    for label in [match_case, prev_btn, next_btn, replace_btn, replace_all_btn] {
        let bounds = harness
            .get_by_label(&label)
            .bounding_box()
            .unwrap_or_else(|| panic!("control '{label}' should have bounding box"));
        assert!(bounds.width() > 0.0, "control '{label}' has zero width");
        assert!(
            bounds.x1 <= panel_width as f64,
            "control '{label}' clipped past {panel_width}px (x1 = {})",
            bounds.x1
        );
    }
}

#[test]
fn search_tab_inputs_widen_on_desktop_without_clipping_controls() {
    assert_search_tab_layout_fits(1100.0, 440.0, 680.0);
}

#[test]
fn search_tab_inputs_scale_down_on_narrow_panel_without_clipping_controls() {
    assert_search_tab_layout_fits(600.0, 160.0, 439.9);
}
