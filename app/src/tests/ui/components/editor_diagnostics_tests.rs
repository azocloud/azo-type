//! Unit tests for editor language diagnostic underline geometry and color mapping.

use super::*;

#[test]
fn computes_wavy_points_spanning_segment_width() {
    let pts = compute_wavy_points(10.0, 22.0, 40.0);
    assert!(pts.len() >= 4);
    assert_eq!(pts.first().unwrap().x, 10.0);
    assert_eq!(pts.last().unwrap().x, 22.0);
    assert!(pts.iter().any(|p| p.y < 40.0));
    assert!(pts.iter().any(|p| p.y > 40.0));
}

#[test]
fn skips_wavy_points_for_degenerate_spans() {
    assert!(compute_wavy_points(10.0, 10.5, 20.0).is_empty());
}

#[test]
fn assigns_distinct_colors_per_diagnostic_category() {
    let spell = diagnostic_color(DiagnosticCategory::Spelling, true);
    let grammar = diagnostic_color(DiagnosticCategory::Grammar, true);
    let typo = diagnostic_color(DiagnosticCategory::Typography, true);
    assert_ne!(spell, grammar);
    assert_ne!(grammar, typo);
    assert_ne!(spell, typo);
}

#[test]
fn paint_language_diagnostics_renders_wavy_and_straight_underlines() {
    let text = "teh quick\nbrown fox  jumps".to_string();
    let diagnostics = vec![
        LanguageCheckDiagnostic {
            category: DiagnosticCategory::Spelling,
            start_byte: 0,
            end_byte: 3,
            start_char: 0,
            end_char: 3,
            line: 1,
            column: 1,
            excerpt: "teh".to_string(),
            message: "Possible typo".to_string(),
            rule_id: "SPELLBOOK".to_string(),
            suggestions: vec!["the".to_string()],
        },
        LanguageCheckDiagnostic {
            category: DiagnosticCategory::Grammar,
            start_byte: 4,
            end_byte: 9,
            start_char: 4,
            end_char: 9,
            line: 1,
            column: 5,
            excerpt: "quick".to_string(),
            message: "Grammar note".to_string(),
            rule_id: "GRAMMAR/NOTE".to_string(),
            suggestions: vec![],
        },
        LanguageCheckDiagnostic {
            category: DiagnosticCategory::Typography,
            start_byte: 19,
            end_byte: 21,
            start_char: 19,
            end_char: 21,
            line: 2,
            column: 10,
            excerpt: "  ".to_string(),
            message: "Double space".to_string(),
            rule_id: "TYPO/SPACE".to_string(),
            suggestions: vec![" ".to_string()],
        },
        // Out-of-bounds / degenerate range should safely be skipped.
        LanguageCheckDiagnostic {
            category: DiagnosticCategory::Spelling,
            start_byte: 200,
            end_byte: 205,
            start_char: 200,
            end_char: 205,
            line: 99,
            column: 1,
            excerpt: String::new(),
            message: String::new(),
            rule_id: "OOB".to_string(),
            suggestions: vec![],
        },
    ];

    let mut harness = egui_kittest::Harness::new_ui_state(
        |ui, text: &mut String| {
            let edit_output = egui::TextEdit::multiline(text).show(ui);
            paint_language_diagnostics(ui, &edit_output, &diagnostics, true, text.chars().count());
            paint_language_diagnostics(ui, &edit_output, &diagnostics, false, text.chars().count());
        },
        text.clone(),
    );
    harness.run();
    assert_eq!(*harness.state(), text);
}
