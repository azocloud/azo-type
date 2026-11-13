//! Unit tests for version-agnostic engine DTO serialization and RGBA/glyph roundtrips.

use super::*;
use egui::Color32;

#[test]
fn test_rendered_page_dto_and_compile_response_roundtrip() {
    let image = ColorImage::new([2, 1], Color32::from_rgba_unmultiplied(12, 34, 56, 255));
    let char_boxes = vec![PreviewCharBox {
        x_min_pt: 10.0,
        y_min_pt: 20.0,
        x_max_pt: 18.5,
        y_max_pt: 32.0,
        file: PathBuf::from("chapters/ch1.typ"),
        char_idx: 42,
        text: "A".to_string(),
    }];
    let page = RenderedPageData {
        page_number: 1,
        width_pt: 595.0,
        height_pt: 842.0,
        image: Arc::new(image),
        char_boxes: Arc::new(char_boxes.clone()),
    };

    let dto = encode_rendered_page_dto(&page);
    let response = EngineCompileResponse {
        compiled_ok: true,
        pages: vec![dto],
        outline: vec![CompiledOutlineItem {
            level: 1,
            numbers: Some("1".to_string()),
            title: "Heading".to_string(),
            file: PathBuf::from("main.typ"),
            line: 1,
            char_offset: 0,
        }],
        diagnostics: vec![DiagnosticItem {
            is_error: false,
            file: PathBuf::from("main.typ"),
            line: Some(3),
            column: Some(5),
            message: "warning text".to_string(),
            hints: vec!["hint".to_string()],
        }],
    };

    let json = serde_json::to_string(&response).expect("serialize EngineCompileResponse");
    let decoded_resp: EngineCompileResponse =
        serde_json::from_str(&json).expect("deserialize EngineCompileResponse");

    assert!(decoded_resp.compiled_ok);
    assert_eq!(decoded_resp.outline, response.outline);
    assert_eq!(decoded_resp.diagnostics, response.diagnostics);

    let decoded_page =
        decode_rendered_page_dto(decoded_resp.pages.into_iter().next().unwrap()).unwrap();
    assert_eq!(decoded_page.page_number, 1);
    assert_eq!(decoded_page.image.size, [2, 1]);
    assert_eq!(decoded_page.char_boxes.as_ref(), &char_boxes);
}
