//! Unit tests for Typst AST heading extraction and outline hierarchy construction.

use std::path::Path;

use super::*;
use crate::core::file_kind::FileKind;

#[test]
fn test_extract_document_outline_typst_headings() {
    let sample = "#set page(paper: \"a4\")\n\n= Executive Summary\nSome intro text.\n\n$ x\n= y + 1 $\n\n== Filesystem & Compilation Pipeline <sec:pipe>\nDetails.\n\n```typst\n= Not A Heading Inside Code Block\n```\n\n=== Mathematical Formulation\n\n#heading(level: 2)[Functional Subheading]\n";
    let outline =
        extract_document_outline(sample, FileKind::Typst, Path::new("main.typ"), None, &[]);
    assert_eq!(outline.len(), 4);
    assert_eq!(outline[0].level, 1);
    assert_eq!(outline[0].number_label, "1");
    assert_eq!(outline[0].title, "Executive Summary");
    assert_eq!(outline[0].line_number, 3);
    assert_eq!(outline[1].level, 2);
    assert_eq!(outline[1].number_label, "1.1");
    assert_eq!(outline[1].title, "Filesystem & Compilation Pipeline");
    assert_eq!(outline[1].line_number, 9);
    assert_eq!(outline[2].level, 3);
    assert_eq!(outline[2].number_label, "1.1.1");
    assert_eq!(outline[2].title, "Mathematical Formulation");
    assert_eq!(outline[2].line_number, 16);
    assert_eq!(outline[3].level, 2);
    assert_eq!(outline[3].number_label, "1.2");
    assert_eq!(outline[3].title, "Functional Subheading");

    // Verify tree construction builds true nested Typst hierarchy
    let tree = build_outline_tree(&outline, Some(2));
    assert_eq!(tree.len(), 1);
    assert_eq!(tree[0].item.title, "Executive Summary");
    assert_eq!(tree[0].children.len(), 2);
    assert_eq!(
        tree[0].children[0].item.title,
        "Filesystem & Compilation Pipeline"
    );
    assert_eq!(tree[0].children[0].children.len(), 1);
    assert_eq!(
        tree[0].children[0].children[0].item.title,
        "Mathematical Formulation"
    );
    assert!(tree[0].children[0].children[0].is_current);
    assert_eq!(tree[0].children[1].item.title, "Functional Subheading");

    // Verify char_offset points accurately to the heading line in the buffer
    let h2_slice: String = sample
        .chars()
        .skip(outline[1].char_offset)
        .take(5)
        .collect();
    assert_eq!(h2_slice, "== Fi");
}
