//! Unit tests for the collapsible multi-tab bottom workspace panel state.

use std::path::PathBuf;

use crate::engine::DiagnosticItem;
use crate::project::DiskProject;

use super::{diagnostic_char_offset, BottomPanelState, BottomPanelTab, MAX_CONSOLE_LOG_LINES};

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
    // "first line\n" = 11 chars; column 9 => 8 chars into line 2 => 19
    assert_eq!(diagnostic_char_offset(&project, &diag), 19);
}
