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
