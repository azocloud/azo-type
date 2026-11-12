use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, FontFamily, FontId};
use typst::syntax::{highlight, parse, LinkedNode, SyntaxKind, Tag};

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

pub fn highlight_typst_code(text: &str, dark_mode: bool, font_size: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    if text.is_empty() {
        return job;
    }

    let root = parse(text);
    let linked = LinkedNode::new(&root);
    let font_id = FontId::new(font_size, FontFamily::Monospace);
    let palette = ThemePalette::for_mode(dark_mode);

    append_highlighted_node(&linked, text, None, dark_mode, &palette, &font_id, &mut job);

    job
}

fn append_highlighted_node(
    node: &LinkedNode,
    source: &str,
    inherited_tag: Option<Tag>,
    dark_mode: bool,
    palette: &ThemePalette,
    font_id: &FontId,
    job: &mut LayoutJob,
) {
    let current_tag = highlight(node).or(inherited_tag);
    let children: Vec<LinkedNode> = node.children().collect();

    if children.is_empty() {
        let range = node.range();
        if let Some(slice) = source.get(range) {
            if !slice.is_empty() {
                let format =
                    format_for_leaf(node, current_tag, dark_mode, palette, font_id.clone());
                job.append(slice, 0.0, format);
            }
        }
    } else {
        for child in &children {
            append_highlighted_node(child, source, current_tag, dark_mode, palette, font_id, job);
        }
    }
}

fn format_for_leaf(
    node: &LinkedNode,
    tag: Option<Tag>,
    dark_mode: bool,
    palette: &ThemePalette,
    font_id: FontId,
) -> TextFormat {
    let kind = node.kind();
    let parent_kind = node.parent_kind();

    let mut format = TextFormat {
        font_id,
        color: palette.syn_default,
        ..Default::default()
    };

    // 1. Named parameters & attributes (`font:`, `lang:`, `width:`, `inset:`, etc.)
    // When an identifier (or its trailing colon in dark mode) is the key of a `SyntaxKind::Named` node:
    if parent_kind == Some(SyntaxKind::Named)
        && matches!(kind, SyntaxKind::Ident | SyntaxKind::Colon)
    {
        if kind == SyntaxKind::Ident {
            format.color = palette.syn_property_or_func;
            return format;
        } else if dark_mode {
            format.color = palette.syn_property_or_func;
            return format;
        }
    }

    // 2. Direct SyntaxKind checks for numbers/dimensions (`11pt`, `2.5cm`, `100%`, `8.5pt`),
    // directives (`#`, `#set`, `#show`), and prose markup content (`[...]`)
    match kind {
        SyntaxKind::Int | SyntaxKind::Float | SyntaxKind::Numeric => {
            format.color = palette.syn_number;
            return format;
        }
        SyntaxKind::LineComment | SyntaxKind::BlockComment => {
            format.color = palette.syn_comment;
            format.italics = true;
            return format;
        }
        SyntaxKind::Str => {
            format.color = palette.syn_string;
            return format;
        }
        SyntaxKind::Hash => {
            format.color = if dark_mode {
                palette.syn_operator // #F3B05A in Dark Mode ("Markup operators and prefixes: #, #set, math markers")
            } else {
                palette.syn_keyword // #B83A4B in Light Mode ("Declaration keywords and macros: #let, #set, #import")
            };
            return format;
        }
        SyntaxKind::Set | SyntaxKind::Show => {
            format.color = if dark_mode {
                palette.syn_operator // #F3B05A in Dark Mode ("#set")
            } else {
                palette.syn_keyword // #B83A4B in Light Mode ("#set")
            };
            return format;
        }
        SyntaxKind::Let
        | SyntaxKind::Import
        | SyntaxKind::Include
        | SyntaxKind::As
        | SyntaxKind::In
        | SyntaxKind::For
        | SyntaxKind::While
        | SyntaxKind::If
        | SyntaxKind::Else
        | SyntaxKind::Return
        | SyntaxKind::Break
        | SyntaxKind::Continue => {
            format.color = palette.syn_keyword;
            return format;
        }
        SyntaxKind::Text => {
            format.color = palette.syn_markup;
            return format;
        }
        _ => {}
    }

    // 3. Tag-based highlighting from `typst::syntax::highlight`
    let Some(tag) = tag else {
        return format;
    };

    if dark_mode {
        // Dark Mode Syntax Highlighting Specification:
        // - Comments: #757582
        // - Keywords & Functions: #82AAFF (#import, #let, counter())
        // - Strings & Literals: #448C6B ("Libertinus Serif", "de")
        // - Properties & Built-ins: #C792EA (font:, lang:, width:, inset:)
        // - Numbers & Dimensions: #CE5555 (11pt, 2.5cm, 100%, 8.5pt)
        // - Operators & Directives: #F3B05A (#, #set, math markers)
        // - Default Identifiers: #E6E6E6
        match tag {
            Tag::Comment => {
                format.color = palette.syn_comment;
                format.italics = true;
            }
            Tag::Function | Tag::Keyword => {
                format.color = palette.syn_keyword;
            }
            Tag::String | Tag::Raw | Tag::Label | Tag::Ref | Tag::Link => {
                format.color = palette.syn_string;
            }
            Tag::Number => {
                format.color = palette.syn_number;
            }
            Tag::Operator | Tag::MathDelimiter | Tag::MathOperator | Tag::ListMarker => {
                format.color = palette.syn_operator;
            }
            Tag::Heading | Tag::Strong => {
                format.color = palette.syn_keyword;
            }
            Tag::Emph => {
                format.color = palette.syn_property_or_func;
                format.italics = true;
            }
            Tag::Interpolated => {
                format.color = palette.syn_property_or_func;
            }
            Tag::Error => {
                format.color = palette.syn_number;
            }
            _ => {
                format.color = palette.syn_default;
            }
        }
    } else {
        // Light Mode Syntax Highlighting Specification:
        // - Comments: #8A8A93
        // - Keywords & Statements: #B83A4B (#let, #set, #import)
        // - Strings & Literals: #1C7D52 ("System-Spezifikation")
        // - Functions & Props: #0B60B0 (text(...), page(...), font:, lang:)
        // - Numbers & Units: #C75C14 (11pt, 2.5cm, 100%)
        // - Punctuation & Syntax: #2E2E38 ({ }, ( ), :)
        // - Markup Content: #1C1C20 (Prose and formatted body content inside [...])
        match tag {
            Tag::Comment => {
                format.color = palette.syn_comment;
                format.italics = true;
            }
            Tag::Keyword => {
                format.color = palette.syn_keyword;
            }
            Tag::Function => {
                format.color = palette.syn_property_or_func;
            }
            Tag::String | Tag::Raw | Tag::Label | Tag::Ref | Tag::Link => {
                format.color = palette.syn_string;
            }
            Tag::Number => {
                format.color = palette.syn_number;
            }
            Tag::Operator | Tag::MathDelimiter | Tag::MathOperator => {
                format.color = palette.syn_operator;
            }
            Tag::Heading | Tag::Strong => {
                format.color = palette.syn_markup;
            }
            Tag::Emph => {
                format.color = palette.syn_markup;
                format.italics = true;
            }
            Tag::ListMarker => {
                format.color = palette.syn_keyword;
            }
            Tag::Interpolated => {
                format.color = palette.syn_property_or_func;
            }
            Tag::Error => {
                format.color = palette.syn_keyword;
            }
            _ => {
                format.color = palette.syn_default;
            }
        }
    }

    format
}
