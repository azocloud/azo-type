use std::fs;
use std::path::{Path, PathBuf};
use azo_type::app::{DocumentOutlineItem, AzoTypeApp};

use azo_type::project::{DiskProject, FileKind};

#[test]
fn extracts_typst_headings_at_various_depths() {
    let source = r#"
= Top Level Heading
Introductory prose.

== Second Level Subheading
More details here.

=== Third Level Subsection
Deep details.

==== Fourth Level
Even deeper.
"#;
    let items = AzoTypeApp::extract_document_outline(
        source,
        FileKind::Typst,
        Path::new("main.typ"),
        None,
        &[],
    );

    assert_eq!(items.len(), 4);
    assert_eq!(items[0].level, 1);
    assert_eq!(items[0].title, "Top Level Heading");
    assert_eq!(items[0].number_label, "1");
    assert_eq!(items[0].line_number, 2);

    assert_eq!(items[1].level, 2);
    assert_eq!(items[1].title, "Second Level Subheading");
    assert_eq!(items[1].number_label, "1.1");
    assert_eq!(items[1].line_number, 5);

    assert_eq!(items[2].level, 3);
    assert_eq!(items[2].title, "Third Level Subsection");
    assert_eq!(items[2].number_label, "1.1.1");
    assert_eq!(items[2].line_number, 8);

    assert_eq!(items[3].level, 4);
    assert_eq!(items[3].title, "Fourth Level");
    assert_eq!(items[3].number_label, "1.1.1.1");
    assert_eq!(items[3].line_number, 11);
}

#[test]
fn extracts_func_call_headings() {
    let source = r#"
#heading(level: 1)[First Level Function]
#heading(level: 2)[Second Level Function]
"#;
    let items = AzoTypeApp::extract_document_outline(
        source,
        FileKind::Typst,
        Path::new("func.typ"),
        None,
        &[],
    );

    assert_eq!(items.len(), 2);
    assert_eq!(items[0].level, 1);
    assert_eq!(items[0].title, "First Level Function");
    assert_eq!(items[0].number_label, "1");

    assert_eq!(items[1].level, 2);
    assert_eq!(items[1].title, "Second Level Function");
    assert_eq!(items[1].number_label, "1.1");
}

#[test]
fn ignores_headings_in_comments_code_blocks_and_show_rules() {
    let source = r#"
// = Commented Heading
/* = Block Commented */

```typst
= Code Block Heading
```

#show heading.where(level: 1): it => block[#it]

= Legitimate Heading
"#;
    let items = AzoTypeApp::extract_document_outline(
        source,
        FileKind::Typst,
        Path::new("clean.typ"),
        None,
        &[],
    );

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].level, 1);
    assert_eq!(items[0].title, "Legitimate Heading");
}

#[test]
fn extracts_bibtex_entries_with_citation_keys_and_entry_types() {
    let bib_source = r#"
@article{einstein1905,
  author = {Einstein, Albert},
  title = {Zur Elektrodynamik bewegter Körper},
  year = {1905}
}

@book{knuth1984,
  author = {Knuth, Donald},
  title = {The TeXbook},
  year = {1984}
}
"#;
    let items = AzoTypeApp::extract_document_outline(
        bib_source,
        FileKind::Bib,
        Path::new("refs.bib"),
        None,
        &[],
    );

    assert_eq!(items.len(), 2);
    assert_eq!(items[0].level, 1);
    assert_eq!(items[0].title, "@einstein1905 (article)");
    assert_eq!(items[0].number_label, "1");
    assert_eq!(items[0].line_number, 2);

    assert_eq!(items[1].level, 1);
    assert_eq!(items[1].title, "@knuth1984 (book)");
    assert_eq!(items[1].number_label, "2");
    assert_eq!(items[1].line_number, 8);
}

#[test]
fn extracts_markdown_headings_with_hash_depths() {
    let md_source = r#"
# Project README
Intro text.

## Features
List of features.

### Nested Detail
Deep detail.

# Architecture
Overview.
"#;
    let items = AzoTypeApp::extract_document_outline(
        md_source,
        FileKind::Text,
        Path::new("README.md"),
        None,
        &[],
    );

    assert_eq!(items.len(), 4);
    assert_eq!(items[0].level, 1);
    assert_eq!(items[0].title, "Project README");
    assert_eq!(items[0].number_label, "1");

    assert_eq!(items[1].level, 2);
    assert_eq!(items[1].title, "Features");
    assert_eq!(items[1].number_label, "1.1");

    assert_eq!(items[2].level, 3);
    assert_eq!(items[2].title, "Nested Detail");
    assert_eq!(items[2].number_label, "1.1.1");

    assert_eq!(items[3].level, 1);
    assert_eq!(items[3].title, "Architecture");
    assert_eq!(items[3].number_label, "2");
}

#[test]
fn assigns_correct_hierarchical_numbers_to_irregular_and_deep_levels() {
    let mut items = vec![
        DocumentOutlineItem {
            level: 1,
            number_label: String::new(),
            title: "Chapter 1".into(),
            file: PathBuf::from("doc.typ"),
            line_number: 1,
            char_offset: 0,
        },
        DocumentOutlineItem {
            level: 3, // Jumps directly from level 1 to level 3!
            number_label: String::new(),
            title: "Subsection 1.1.1".into(),
            file: PathBuf::from("doc.typ"),
            line_number: 5,
            char_offset: 20,
        },
        DocumentOutlineItem {
            level: 2,
            number_label: String::new(),
            title: "Section 1.2".into(),
            file: PathBuf::from("doc.typ"),
            line_number: 10,
            char_offset: 50,
        },
        DocumentOutlineItem {
            level: 1,
            number_label: String::new(),
            title: "Chapter 2".into(),
            file: PathBuf::from("doc.typ"),
            line_number: 15,
            char_offset: 80,
        },
    ];

    AzoTypeApp::assign_hierarchical_numbers(&mut items);

    assert_eq!(items[0].number_label, "1");
    assert_eq!(items[1].number_label, "1.1.1");
    assert_eq!(items[2].number_label, "1.2");
    assert_eq!(items[3].number_label, "2");
}

#[test]
fn builds_nested_outline_tree_with_accurate_parent_child_relationships() {
    let items = vec![
        DocumentOutlineItem {
            level: 1,
            number_label: "1".into(),
            title: "Root 1".into(),
            file: PathBuf::from("main.typ"),
            line_number: 1,
            char_offset: 0,
        },
        DocumentOutlineItem {
            level: 2,
            number_label: "1.1".into(),
            title: "Child 1.1".into(),
            file: PathBuf::from("main.typ"),
            line_number: 5,
            char_offset: 20,
        },
        DocumentOutlineItem {
            level: 2,
            number_label: "1.2".into(),
            title: "Child 1.2".into(),
            file: PathBuf::from("main.typ"),
            line_number: 10,
            char_offset: 40,
        },
        DocumentOutlineItem {
            level: 1,
            number_label: "2".into(),
            title: "Root 2".into(),
            file: PathBuf::from("main.typ"),
            line_number: 15,
            char_offset: 60,
        },
    ];

    let tree = AzoTypeApp::build_outline_tree(&items, Some(1));
    assert_eq!(tree.len(), 2);

    assert_eq!(tree[0].item.title, "Root 1");
    assert!(!tree[0].is_current);
    assert_eq!(tree[0].children.len(), 2);
    assert_eq!(tree[0].children[0].item.title, "Child 1.1");
    assert!(tree[0].children[0].is_current);
    assert_eq!(tree[0].children[1].item.title, "Child 1.2");

    assert_eq!(tree[1].item.title, "Root 2");
    assert_eq!(tree[1].children.len(), 0);
}

#[test]
fn resolves_module_includes_recursively_without_infinite_loops() {
    let temp_dir =
        std::env::temp_dir().join(format!("typst_test_recurse_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(temp_dir.join("chapters")).unwrap();

    // Cyclic inclusion: main includes intro, intro includes main (must not stack overflow!)
    fs::write(
        temp_dir.join("main.typ"),
        "= Main Document\n#include \"chapters/intro.typ\"\n",
    )
    .unwrap();
    fs::write(
        temp_dir.join("chapters/intro.typ"),
        "== Intro Document\n#include \"/main.typ\"\n",
    )
    .unwrap();

    let project = DiskProject::open_folder(&temp_dir, false).unwrap();
    let main_text = fs::read_to_string(temp_dir.join("main.typ")).unwrap();

    let outline = AzoTypeApp::extract_document_outline(
        &main_text,
        FileKind::Typst,
        Path::new("main.typ"),
        Some(&project),
        &[],
    );

    assert_eq!(outline.len(), 2);
    assert_eq!(outline[0].title, "Main Document");
    assert_eq!(outline[1].title, "Intro Document");

    let _ = fs::remove_dir_all(&temp_dir);
}
