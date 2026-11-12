//! Unit tests for application icon decoding, caching, and viewport builder configuration.

use super::*;

#[test]
fn test_embedded_app_window_icon_loads_successfully() {
    let icon = app_window_icon();
    assert_eq!(icon.width, 512);
    assert_eq!(icon.height, 512);
    assert_eq!(
        icon.rgba.len(),
        (icon.width * icon.height * 4) as usize,
        "RGBA buffer length must equal width * height * 4"
    );
    assert!(!icon.is_empty());
}

#[test]
fn test_app_window_icon_caches_instance() {
    let icon1 = app_window_icon();
    let icon2 = app_window_icon();
    assert!(
        Arc::ptr_eq(&icon1, &icon2),
        "app_window_icon must return identical cached Arc instance"
    );
}

#[test]
fn test_load_icon_from_valid_png_bytes() {
    let raw_bytes = include_bytes!("../../../icons/win-16.png");
    let icon = load_icon_from_png_bytes(raw_bytes).expect("Valid 16x16 PNG must decode");
    assert_eq!(icon.width, 16);
    assert_eq!(icon.height, 16);
    assert_eq!(icon.rgba.len(), 16 * 16 * 4);
}

#[test]
fn test_load_icon_from_invalid_bytes_returns_error() {
    let garbage = b"not a png image data";
    let res = load_icon_from_png_bytes(garbage);
    assert!(res.is_err(), "Corrupt data must return an Err result");
    assert!(res.unwrap_err().contains("Failed to decode icon PNG"));
}

#[test]
fn test_configure_viewport_builder() {
    let builder = egui::ViewportBuilder::default().with_title("Test Window");
    let configured = configure_viewport_builder(builder);
    assert_eq!(configured.app_id.as_deref(), Some(APP_ID));
    assert!(configured.icon.is_some());
}

