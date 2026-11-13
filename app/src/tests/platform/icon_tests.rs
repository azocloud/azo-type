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

#[test]
fn test_ensure_linux_desktop_integration_to_dir() {
    let temp_dir = std::env::temp_dir().join(format!("azo_type_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);

    let dummy_exe = std::path::Path::new("/usr/bin/azo-type");
    let result = ensure_linux_desktop_integration_to_dir(&temp_dir, Some(dummy_exe));
    assert!(result.is_ok());

    let desktop_file = temp_dir
        .join("applications")
        .join(format!("{APP_ID}.desktop"));
    assert!(desktop_file.exists(), "Desktop file must be created");

    let content = std::fs::read_to_string(&desktop_file).expect("Desktop file must be readable");
    assert!(content.contains("StartupWMClass=azo-type"));
    assert!(content.contains("Icon=azo-type"));
    assert!(content.contains("Exec=\"/usr/bin/azo-type\" %F"));

    // Check all icons were installed
    for (rel_path, _) in LINUX_ICONS {
        let icon_path = temp_dir.join("icons").join("hicolor").join(rel_path);
        assert!(icon_path.exists(), "Icon at {} must exist", rel_path);
        assert!(
            std::fs::metadata(&icon_path).unwrap().len() > 0,
            "Icon must not be empty"
        );
    }

    let _ = std::fs::remove_dir_all(&temp_dir);
}
