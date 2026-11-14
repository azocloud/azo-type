//! UI Design System tokens, ThemePalette, and egui Visuals styling.

use egui::{Color32, Context, CornerRadius, Stroke, Visuals};

/// Standardized CSS token definition for the Typst Editor UI Design System.
pub const DESIGN_SYSTEM_CSS_TOKENS: &str = r#":root {
  /* Surface & Frame Tokens (Light Mode) */
  --color-canvas-editor: #ffffff;
  --color-canvas-chrome: #f4f4f6;
  --color-canvas-workspace: #e8e8ec;
  --color-border-subtle: #d8d8e0;
  --color-line-highlight: #ebf3fe;

  /* Typography & Icons (Light Mode) */
  --color-text-primary: #1e1e24;
  --color-text-muted: #a0a0aa;

  /* Syntax Highlighting Tokens (Light Mode) */
  --syn-comment: #8a8a93;
  --syn-keyword: #b83a4b;
  --syn-string: #1c7d52;
  --syn-function: #0b60b0;
  --syn-number: #c75c14;
  --syn-punctuation: #2e2e38;
}

[data-theme="dark"] {
  /* Surface & Frame Tokens (Dark Mode) */
  --color-canvas-editor: #1c1b22;
  --color-canvas-chrome: #2b2a33;
  --color-canvas-workspace: #23222b;
  --color-border-subtle: #383742;
  --color-line-highlight: #262530;

  /* Typography & Icons (Dark Mode) */
  --color-text-primary: #bfbfe0;
  --color-text-muted: #6e6d7a;

  /* Syntax Highlighting Tokens (Dark Mode) */
  --syn-comment: #757582;
  --syn-keyword: #82aaff;
  --syn-string: #448c6b;
  --syn-function: #c792ea;
  --syn-number: #ce5555;
  --syn-operator: #f3b05a;
  --syn-punctuation: #e6e6e6;
}"#;

/// Complete UI Frame, Typography, and Syntax Highlighting token palette
/// implementing the Typst Editor UI Design System specification.
#[derive(Debug, Clone, Copy)]
pub struct ThemePalette {
    // UI Frame & Structural Tokens
    pub editor_canvas: Color32,
    pub chrome_sidebar: Color32,
    pub preview_workspace: Color32,
    pub panel_border: Color32,
    pub active_line_highlight: Color32,
    pub text_primary: Color32,
    pub text_muted: Color32,

    // Subtle surface luminance steps for interactive widgets (buttons, inputs, popups)
    pub widget_inactive_bg: Color32,
    pub widget_hovered_bg: Color32,
    pub widget_active_bg: Color32,
    pub selection_bg: Color32,
    pub selection_stroke: Color32,

    // Syntax Highlighting Tokens
    pub syn_comment: Color32,
    pub syn_keyword: Color32,
    pub syn_string: Color32,
    pub syn_property_or_func: Color32,
    pub syn_number: Color32,
    pub syn_operator: Color32,
    pub syn_default: Color32,
    pub syn_markup: Color32,
}

impl ThemePalette {
    /// Section 2: Dark Mode Specification
    pub const DARK: Self = Self {
        editor_canvas: Color32::from_rgb(0x1C, 0x1B, 0x22), // #1C1B22
        chrome_sidebar: Color32::from_rgb(0x2B, 0x2A, 0x33), // #2B2A33
        preview_workspace: Color32::from_rgb(0x23, 0x22, 0x2B), // #23222B
        panel_border: Color32::from_rgb(0x38, 0x37, 0x42),  // #383742
        active_line_highlight: Color32::from_rgb(0x26, 0x25, 0x30), // #262530
        text_primary: Color32::from_rgb(0xBF, 0xBF, 0xE0),  // #BFBFE0
        text_muted: Color32::from_rgb(0x6E, 0x6D, 0x7A),    // #6E6D7A

        widget_inactive_bg: Color32::from_rgb(0x32, 0x31, 0x3B),
        widget_hovered_bg: Color32::from_rgb(0x3C, 0x3B, 0x47),
        widget_active_bg: Color32::from_rgb(0x45, 0x44, 0x52),
        selection_bg: Color32::from_rgb(0x35, 0x4A, 0x73),
        selection_stroke: Color32::from_rgb(0x82, 0xAA, 0xFF),

        syn_comment: Color32::from_rgb(0x75, 0x75, 0x82), // #757582
        syn_keyword: Color32::from_rgb(0x82, 0xAA, 0xFF), // #82AAFF
        syn_string: Color32::from_rgb(0x44, 0x8C, 0x6B),  // #448C6B
        syn_property_or_func: Color32::from_rgb(0xC7, 0x92, 0xEA), // #C792EA
        syn_number: Color32::from_rgb(0xCE, 0x55, 0x55),  // #CE5555
        syn_operator: Color32::from_rgb(0xF3, 0xB0, 0x5A), // #F3B05A
        syn_default: Color32::from_rgb(0xE6, 0xE6, 0xE6), // #E6E6E6
        syn_markup: Color32::from_rgb(0xE6, 0xE6, 0xE6),  // #E6E6E6
    };

    /// Section 3: Light Mode Specification
    pub const LIGHT: Self = Self {
        editor_canvas: Color32::from_rgb(0xFF, 0xFF, 0xFF), // #FFFFFF
        chrome_sidebar: Color32::from_rgb(0xF4, 0xF4, 0xF6), // #F4F4F6
        preview_workspace: Color32::from_rgb(0xE8, 0xE8, 0xEC), // #E8E8EC
        panel_border: Color32::from_rgb(0xD8, 0xD8, 0xE0),  // #D8D8E0
        active_line_highlight: Color32::from_rgb(0xEB, 0xF3, 0xFE), // #EBF3FE
        text_primary: Color32::from_rgb(0x1E, 0x1E, 0x24),  // #1E1E24
        text_muted: Color32::from_rgb(0xA0, 0xA0, 0xAA),    // #A0A0AA

        widget_inactive_bg: Color32::from_rgb(0xEA, 0xEA, 0xEF),
        widget_hovered_bg: Color32::from_rgb(0xDF, 0xDF, 0xE7),
        widget_active_bg: Color32::from_rgb(0xD4, 0xD4, 0xDF),
        selection_bg: Color32::from_rgb(0xC8, 0xDE, 0xFC),
        selection_stroke: Color32::from_rgb(0x0B, 0x60, 0xB0),

        syn_comment: Color32::from_rgb(0x8A, 0x8A, 0x93), // #8A8A93
        syn_keyword: Color32::from_rgb(0xB8, 0x3A, 0x4B), // #B83A4B
        syn_string: Color32::from_rgb(0x1C, 0x7D, 0x52),  // #1C7D52
        syn_property_or_func: Color32::from_rgb(0x0B, 0x60, 0xB0), // #0B60B0
        syn_number: Color32::from_rgb(0xC7, 0x5C, 0x14),  // #C75C14
        syn_operator: Color32::from_rgb(0x2E, 0x2E, 0x38), // #2E2E38
        syn_default: Color32::from_rgb(0x2E, 0x2E, 0x38), // #2E2E38
        syn_markup: Color32::from_rgb(0x1C, 0x1C, 0x20),  // #1C1C20
    };

    #[inline]
    pub const fn for_mode(dark_mode: bool) -> Self {
        if dark_mode {
            Self::DARK
        } else {
            Self::LIGHT
        }
    }
}

/// Applies design system colors, corner radii, and widget visual states to the egui context.
pub fn apply_theme(ctx: &Context, dark_mode: bool) {
    let palette = ThemePalette::for_mode(dark_mode);
    let mut visuals = if dark_mode {
        Visuals::dark()
    } else {
        Visuals::light()
    };

    // Surface & Structural Tokens
    visuals.panel_fill = palette.chrome_sidebar;
    visuals.window_fill = palette.chrome_sidebar;
    visuals.window_stroke = Stroke::new(1.0_f32, palette.panel_border);
    visuals.window_corner_radius = CornerRadius::same(6);
    visuals.menu_corner_radius = CornerRadius::same(5);
    visuals.extreme_bg_color = palette.editor_canvas;
    visuals.faint_bg_color = palette.active_line_highlight;
    visuals.code_bg_color = palette.editor_canvas;

    // Subtle Layered Depth (minimal shadow instead of heavy drop shadows)
    visuals.window_shadow = egui::epaint::Shadow {
        offset: [0, 2],
        blur: 8,
        spread: 0,
        color: Color32::from_black_alpha(if dark_mode { 60 } else { 25 }),
    };
    visuals.popup_shadow = visuals.window_shadow;

    // Selection & Accents
    visuals.selection.bg_fill = palette.selection_bg;
    visuals.selection.stroke = Stroke::new(1.0_f32, palette.selection_stroke);

    // Widgets: 4px curvature, 1px hairline border, instant surface background shift (0px expansion)
    visuals.widgets.noninteractive.bg_fill = palette.chrome_sidebar;
    visuals.widgets.noninteractive.weak_bg_fill = palette.chrome_sidebar;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, palette.panel_border);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, palette.text_primary);
    visuals.widgets.noninteractive.corner_radius = CornerRadius::same(4);

    visuals.widgets.inactive.bg_fill = palette.widget_inactive_bg;
    visuals.widgets.inactive.weak_bg_fill = palette.widget_inactive_bg;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, palette.panel_border);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, palette.text_primary);
    visuals.widgets.inactive.corner_radius = CornerRadius::same(4);
    visuals.widgets.inactive.expansion = 0.0;

    visuals.widgets.hovered.bg_fill = palette.widget_hovered_bg;
    visuals.widgets.hovered.weak_bg_fill = palette.widget_hovered_bg;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, palette.panel_border);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, palette.text_primary);
    visuals.widgets.hovered.corner_radius = CornerRadius::same(4);
    visuals.widgets.hovered.expansion = 0.0;

    visuals.widgets.active.bg_fill = palette.widget_active_bg;
    visuals.widgets.active.weak_bg_fill = palette.widget_active_bg;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, palette.selection_stroke);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, palette.text_primary);
    visuals.widgets.active.corner_radius = CornerRadius::same(4);
    visuals.widgets.active.expansion = 0.0;

    visuals.widgets.open.bg_fill = palette.widget_hovered_bg;
    visuals.widgets.open.weak_bg_fill = palette.widget_hovered_bg;
    visuals.widgets.open.bg_stroke = Stroke::new(1.0_f32, palette.panel_border);
    visuals.widgets.open.fg_stroke = Stroke::new(1.0_f32, palette.text_primary);
    visuals.widgets.open.corner_radius = CornerRadius::same(4);
    visuals.widgets.open.expansion = 0.0;

    ctx.set_visuals(visuals);
}

#[cfg(test)]
#[path = "../tests/ui/theme_tests.rs"]
mod tests;
