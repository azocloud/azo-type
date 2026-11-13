//! Unit tests for Typst AST prose masking, language directive detection, and span conversion.

use super::*;

#[test]
fn detects_set_text_language_directive() {
    let src = r#"
#set page(paper: "a4")
#set text(font: "Libertinus Serif", lang: "de-DE")
= Einleitung
Das ist ein deutscher Satz.
"#;
    assert_eq!(
        detect_typst_document_language(src).as_deref(),
        Some("de_de")
    );
    assert_eq!(normalize_language_code("de_de").as_deref(), Some("de"));
}

#[test]
fn returns_none_when_no_lang_directive_present() {
    let src = "= Hello\nJust plain Typst text.\n";
    assert_eq!(detect_typst_document_language(src), None);
}

#[test]
fn preserves_exact_byte_length_with_multibyte_syntax_and_content_blocks() {
    let src = "= Überschrift\nThis is *very* #emph[important] text with $alpha + beta$ and // kommentar café\n\nSecond paragraph.";
    let mask = extract_typst_prose_mask(src);

    assert_eq!(mask.masked_text.len(), src.len());
    assert_eq!(mask.prose_bytes.len(), src.len());

    let idx_important = src.find("important").unwrap();
    assert_eq!(
        &mask.masked_text[idx_important..idx_important + "important".len()],
        "important"
    );

    let idx_second = src.find("Second paragraph.").unwrap();
    assert_eq!(
        &mask.masked_text[idx_second..idx_second + "Second paragraph.".len()],
        "Second paragraph."
    );

    // Math and comment must be masked out.
    assert!(!mask.masked_text.contains("alpha"));
    assert!(!mask.masked_text.contains("kommentar"));
    assert!(!mask.masked_text.contains("emph"));
}

#[test]
fn splits_paragraphs_with_accurate_global_offsets() {
    let src = "#set text(lang: \"en\")\n\nFirst sentence here.\n\nSecond *bold* sentence.";
    let mask = extract_typst_prose_mask(src);
    let paras = split_prose_paragraphs(&mask.masked_text);

    assert_eq!(paras.len(), 2);
    assert_eq!(paras[0].text, "First sentence here.");
    assert_eq!(
        &src[paras[0].start_byte..paras[0].end_byte],
        "First sentence here."
    );

    let bold_idx = src.find("bold").unwrap();
    let p1 = &paras[1];
    let rel_bold = bold_idx - p1.start_byte;
    assert_eq!(&p1.text[rel_bold..rel_bold + 4], "bold");
}

#[test]
fn byte_span_to_char_range_handles_multibyte_and_emoji() {
    let src = "Grüße 🌍 Welt";
    let welt_start = src.find("Welt").unwrap();
    let welt_end = welt_start + "Welt".len();
    let (sc, ec) = byte_span_to_char_range(src, welt_start, welt_end);
    let chars: Vec<char> = src.chars().collect();
    let extracted: String = chars[sc..ec].iter().collect();
    assert_eq!(extracted, "Welt");
}
