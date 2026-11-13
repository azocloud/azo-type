//! Unit tests for the collapsible multi-tab bottom workspace panel state and diagnostics sorting.

use std::path::PathBuf;

use crate::engine::DiagnosticItem;
use crate::project::DiskProject;

use super::{
    diagnostic_char_offset, diagnostic_fingerprint, sort_diagnostics_errors_first,
    truncate_diagnostic_summary, BottomPanelState, BottomPanelTab, MAX_CONSOLE_LOG_LINES,
};

fn sample_diag(is_error: bool, line: usize, msg: &str) -> DiagnosticItem {
    DiagnosticItem {
        is_error,
        file: PathBuf::from("main.typ"),
        line: Some(line),
        column: Some(1),
        message: msg.to_string(),
        hints: Vec::new(),
    }
}

#[test]
fn test_bottom_panel_starts_invisible_by_default() {
    let state = BottomPanelState::default();
    assert!(!state.visible);
    assert_eq!(state.active_tab, BottomPanelTab::WarningsAndErrors);
    assert!(state.console_logs.is_empty());
}

#[test]
fn test_bottom_panel_toggle_and_console_log_capping() {
    let mut state = BottomPanelState::default();
    state.toggle_visible();
    assert!(state.visible);
    state.toggle_visible();
    assert!(!state.visible);

    state.push_console_log("   ");
    assert!(state.console_logs.is_empty());

    state.push_console_log("Compiled in 12 ms");
    state.push_console_log("Compiled in 12 ms");
    assert_eq!(state.console_logs.len(), 1);
    assert!(state.console_logs[0].contains("Compiled in 12 ms"));

    for i in 0..(MAX_CONSOLE_LOG_LINES + 15) {
        state.push_console_log(format!("Log line {i}"));
    }
    assert_eq!(state.console_logs.len(), MAX_CONSOLE_LOG_LINES);
    assert!(state
        .console_logs
        .last()
        .is_some_and(|l| l.contains(&(MAX_CONSOLE_LOG_LINES + 14).to_string())));

    state.clear_console();
    assert!(state.console_logs.is_empty());
}

#[test]
fn test_sort_diagnostics_places_all_errors_before_warnings() {
    let items = vec![
        sample_diag(false, 2, "first warning"),
        sample_diag(true, 5, "first error"),
        sample_diag(false, 8, "second warning"),
        sample_diag(true, 10, "second error"),
    ];
    let sorted = sort_diagnostics_errors_first(&items);
    assert_eq!(sorted.len(), 4);
    assert!(sorted[0].is_error && sorted[0].message == "first error");
    assert!(sorted[1].is_error && sorted[1].message == "second error");
    assert!(!sorted[2].is_error && sorted[2].message == "first warning");
    assert!(!sorted[3].is_error && sorted[3].message == "second warning");
}

#[test]
fn test_toggle_diagnostic_expanded_flips_state_by_fingerprint() {
    let mut state = BottomPanelState::default();
    let diag = sample_diag(true, 4, "unknown variable");
    let fp = diagnostic_fingerprint(&diag);

    assert!(!state.is_diagnostic_expanded(fp));
    state.toggle_diagnostic_expanded(fp);
    assert!(state.is_diagnostic_expanded(fp));
    state.toggle_diagnostic_expanded(fp);
    assert!(!state.is_diagnostic_expanded(fp));
}

#[test]
fn test_truncate_diagnostic_summary_handles_multiline_and_unicode() {
    let (short, shortened_short) = truncate_diagnostic_summary("line one\nline two", 50);
    assert_eq!(short, "line one line two");
    assert!(shortened_short);

    let unicode_long = "äöüß ".repeat(20);
    let (clipped, shortened_unicode) = truncate_diagnostic_summary(&unicode_long, 15);
    assert!(shortened_unicode);
    assert_eq!(clipped.chars().count(), 15);
    assert!(clipped.ends_with('…'));
}

#[test]
fn test_diagnostic_char_offset_maps_line_and_column() {
    let mut project = DiskProject::blank();
    if let Some(buf) = project.buffers.get_mut(&PathBuf::from("main.typ")) {
        buf.text = Some("first line\nsecond ä line\nthird".to_string());
    }
    let diag = DiagnosticItem {
        is_error: true,
        file: PathBuf::from("main.typ"),
        line: Some(2),
        column: Some(9),
        message: "err".to_string(),
        hints: Vec::new(),
    };
    assert_eq!(diagnostic_char_offset(&project, &diag), 19);
}
