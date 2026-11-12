use egui::text::{LayoutJob, TextFormat};
use egui::{FontFamily, FontId};
use typst::syntax::{highlight, parse, LinkedNode, SyntaxKind, Tag};

pub use crate::ui::theme::{ThemePalette, DESIGN_SYSTEM_CSS_TOKENS};

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
