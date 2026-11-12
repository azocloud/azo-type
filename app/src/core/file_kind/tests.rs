use super::*;

#[test]
fn test_file_kind_from_extension() {
    assert_eq!(FileKind::from_path(Path::new("doc.typ")), FileKind::Typst);
    assert_eq!(FileKind::from_path(Path::new("refs.bib")), FileKind::Bib);
    assert_eq!(FileKind::from_path(Path::new("config.yml")), FileKind::Bib);
    assert_eq!(FileKind::from_path(Path::new("diagram.svg")), FileKind::Svg);
    assert_eq!(FileKind::from_path(Path::new("photo.png")), FileKind::Image);
    assert_eq!(FileKind::from_path(Path::new("notes.md")), FileKind::Text);
    assert_eq!(
        FileKind::from_path(Path::new("binary.pdf")),
        FileKind::Binary
    );
}

#[test]
fn test_editable_text_kinds() {
    assert!(FileKind::Typst.is_editable_text());
    assert!(FileKind::Bib.is_editable_text());
    assert!(FileKind::Svg.is_editable_text());
    assert!(FileKind::Text.is_editable_text());
    assert!(!FileKind::Image.is_editable_text());
    assert!(!FileKind::Binary.is_editable_text());
}
