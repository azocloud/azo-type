use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, FontFamily, FontId};
use typst::syntax::{highlight, parse, LinkedNode, Tag};

pub fn highlight_typst_code(text: &str, dark_mode: bool, font_size: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    if text.is_empty() {
        return job;
    }

    let root = parse(text);
    let linked = LinkedNode::new(&root);
    let font_id = FontId::new(font_size, FontFamily::Monospace);

    let default_color = if dark_mode {
        Color32::from_rgb(226, 232, 240)
    } else {
        Color32::from_rgb(30, 41, 59)
    };

    append_highlighted_node(
        &linked,
        text,
        None,
        dark_mode,
        &font_id,
        default_color,
        &mut job,
    );

    job
}

fn append_highlighted_node(
    node: &LinkedNode,
    source: &str,
    inherited_tag: Option<Tag>,
    dark_mode: bool,
    font_id: &FontId,
    default_color: Color32,
    job: &mut LayoutJob,
) {
    let current_tag = highlight(node).or(inherited_tag);
    let children: Vec<LinkedNode> = node.children().collect();

    if children.is_empty() {
        let range = node.range();
        if let Some(slice) = source.get(range) {
            if !slice.is_empty() {
                let format = format_for_tag(current_tag, dark_mode, font_id.clone(), default_color);
                job.append(slice, 0.0, format);
            }
        }
    } else {
        for child in &children {
            append_highlighted_node(
                child,
                source,
                current_tag,
                dark_mode,
                font_id,
                default_color,
                job,
            );
        }
    }
}

fn format_for_tag(
    tag: Option<Tag>,
    dark_mode: bool,
    font_id: FontId,
    default_color: Color32,
) -> TextFormat {
    let mut format = TextFormat {
        font_id,
        color: default_color,
        ..Default::default()
    };

    let Some(tag) = tag else {
        return format;
    };

    if dark_mode {
        match tag {
            Tag::Comment => {
                format.color = Color32::from_rgb(100, 116, 139);
                format.italics = true;
            }
            Tag::Heading | Tag::Strong => {
                format.color = Color32::from_rgb(96, 165, 250);
            }
            Tag::Emph => {
                format.color = Color32::from_rgb(192, 132, 252);
                format.italics = true;
            }
            Tag::Keyword | Tag::Operator => {
                format.color = Color32::from_rgb(244, 114, 182);
            }
            Tag::Function => {
                format.color = Color32::from_rgb(56, 189, 248);
            }
            Tag::String | Tag::Raw => {
                format.color = Color32::from_rgb(74, 222, 128);
            }
            Tag::Number => {
                format.color = Color32::from_rgb(251, 191, 36);
            }
            Tag::MathDelimiter | Tag::MathOperator => {
                format.color = Color32::from_rgb(45, 212, 191);
            }
            Tag::Label | Tag::Ref => {
                format.color = Color32::from_rgb(251, 146, 60);
            }
            Tag::ListMarker => {
                format.color = Color32::from_rgb(129, 140, 248);
            }
            Tag::Error => {
                format.color = Color32::from_rgb(248, 113, 113);
            }
            _ => {
                format.color = Color32::from_rgb(165, 180, 252);
            }
        }
    } else {
        match tag {
            Tag::Comment => {
                format.color = Color32::from_rgb(100, 116, 139);
                format.italics = true;
            }
            Tag::Heading | Tag::Strong => {
                format.color = Color32::from_rgb(29, 78, 216);
            }
            Tag::Emph => {
                format.color = Color32::from_rgb(126, 34, 206);
                format.italics = true;
            }
            Tag::Keyword | Tag::Operator => {
                format.color = Color32::from_rgb(190, 24, 93);
            }
            Tag::Function => {
                format.color = Color32::from_rgb(3, 105, 161);
            }
            Tag::String | Tag::Raw => {
                format.color = Color32::from_rgb(21, 128, 61);
            }
            Tag::Number => {
                format.color = Color32::from_rgb(180, 83, 9);
            }
            Tag::MathDelimiter | Tag::MathOperator => {
                format.color = Color32::from_rgb(15, 118, 110);
            }
            Tag::Label | Tag::Ref => {
                format.color = Color32::from_rgb(194, 65, 12);
            }
            Tag::ListMarker => {
                format.color = Color32::from_rgb(67, 56, 202);
            }
            Tag::Error => {
                format.color = Color32::from_rgb(220, 38, 38);
            }
            _ => {
                format.color = Color32::from_rgb(79, 70, 229);
            }
        }
    }

    format
}
