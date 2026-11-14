//! Unit tests for UI Design System palettes and `egui` theme application.

use egui::Context;

use super::*;

#[test]
fn palette_for_mode_selects_dark_and_light_constants() {
    let dark = ThemePalette::for_mode(true);
    let light = ThemePalette::for_mode(false);

    assert_eq!(dark.editor_canvas, ThemePalette::DARK.editor_canvas);
    assert_eq!(light.editor_canvas, ThemePalette::LIGHT.editor_canvas);
    assert_ne!(dark.editor_canvas, light.editor_canvas);
    assert!(DESIGN_SYSTEM_CSS_TOKENS.contains("[data-theme=\"dark\"]"));
}

#[test]
fn apply_theme_configures_context_visuals_for_dark_and_light_modes() {
    let ctx = Context::default();

    apply_theme(&ctx, true);
    let s = ctx.style();
    assert!(s.visuals.dark_mode);
    assert_eq!(s.visuals.panel_fill, ThemePalette::DARK.chrome_sidebar);
    assert_eq!(s.visuals.extreme_bg_color, ThemePalette::DARK.editor_canvas);
    assert_eq!(s.visuals.selection.bg_fill, ThemePalette::DARK.selection_bg);
    assert_eq!(s.visuals.widgets.inactive.expansion, 0.0);

    apply_theme(&ctx, false);
    let s = ctx.style();
    assert!(!s.visuals.dark_mode);
    assert_eq!(s.visuals.panel_fill, ThemePalette::LIGHT.chrome_sidebar);
    assert_eq!(
        s.visuals.extreme_bg_color,
        ThemePalette::LIGHT.editor_canvas
    );
    assert_eq!(
        s.visuals.selection.bg_fill,
        ThemePalette::LIGHT.selection_bg
    );
}
