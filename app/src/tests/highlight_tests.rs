//! Unit tests for Typst syntax highlighting into styled `LayoutJob` spans.

use super::*;

fn sample_typst_document() -> &'static str {
    concat!(
        "// Line comment\n",
        "/* Block comment */\n",
        "#import \"template.typ\": conf\n",
        "#set text(font: \"Libertinus Serif\", size: 11pt, lang: \"en\")\n",
        "#show heading: it => it\n",
        "#let items = (1, 2.5, 100%)\n",
        "#for x in items { if x > 1 { break } else { continue } }\n",
        "#while false { return }\n",
        "#include \"chap.typ\"\n",
        "= Main Heading <sec-1>\n",
        "Plain prose with *bold*, _italic_, `raw`, @sec-1, and https://typst.app .\n",
        "- List item\n",
        "$ f(x) = sum_(i=0)^n x_i $\n",
        "#let unclosed = (\n",
    )
}

#[test]
fn empty_input_returns_empty_layout_job() {
    let job = highlight_typst_code("", true, 14.0);
    assert!(job.text.is_empty());
    assert!(job.sections.is_empty());
}

#[test]
fn preserves_exact_source_text_and_applies_dark_and_light_palettes() {
    let src = sample_typst_document();

    for dark_mode in [true, false] {
        let palette = ThemePalette::for_mode(dark_mode);
        let job = highlight_typst_code(src, dark_mode, 13.5);
        assert_eq!(job.text, src);
        assert!(!job.sections.is_empty());

        let find_section = |needle: &str| {
            let start = src.find(needle).expect("needle present in sample");
            let end = start + needle.len();
            job.sections
                .iter()
                .find(|s| s.byte_range.start <= start && s.byte_range.end >= end)
                .unwrap_or_else(|| panic!("missing section for {needle:?}"))
        };

        let comment = find_section("// Line comment");
        assert_eq!(comment.format.color, palette.syn_comment);
        assert!(comment.format.italics);

        let block_comment = find_section("/* Block comment */");
        assert_eq!(block_comment.format.color, palette.syn_comment);
        assert!(block_comment.format.italics);

        let string_lit = find_section("\"Libertinus Serif\"");
        assert_eq!(string_lit.format.color, palette.syn_string);

        let num_dim = find_section("11pt");
        assert_eq!(num_dim.format.color, palette.syn_number);

        let named_param = find_section("font");
        assert_eq!(named_param.format.color, palette.syn_property_or_func);

        let let_kw = find_section("let");
        assert_eq!(let_kw.format.color, palette.syn_keyword);

        let set_kw = find_section("set");
        assert_eq!(
            set_kw.format.color,
            if dark_mode {
                palette.syn_operator
            } else {
                palette.syn_keyword
            }
        );

        let emph_start = src.find("_italic_").unwrap();
        let delim_sec = job
            .sections
            .iter()
            .find(|s| s.byte_range.start == emph_start)
            .unwrap();
        assert!(delim_sec.format.italics);
    }
}
