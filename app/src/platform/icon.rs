//! Application icon loader, PNG image decoding, and desktop viewport icon configuration.

use std::sync::Arc;
use std::sync::OnceLock;

/// Application ID used for Wayland compositors, desktop window grouping, and application matching.
pub const APP_ID: &str = "azo-type";

/// Decodes and returns the cached high-resolution application window icon.
///
/// The icon is embedded at compile time from `app/icons/icon.png` and decoded once
/// into an [`egui::IconData`] structure suitable for window managers, taskbars, and Alt+Tab switchers.
#[must_use]
pub fn app_window_icon() -> Arc<egui::IconData> {
    static ICON: OnceLock<Arc<egui::IconData>> = OnceLock::new();
    ICON.get_or_init(|| {
        let icon_bytes = include_bytes!("../../icons/icon.png");
        let icon_data = load_icon_from_png_bytes(icon_bytes)
            .expect("Embedded application icon PNG must be a valid image");
        Arc::new(icon_data)
    })
    .clone()
}

/// Helper function to configure an [`egui::ViewportBuilder`] with the application ID and window icon.
///
/// Applies both `.with_app_id(APP_ID)` and `.with_icon(app_window_icon())` to ensure
/// consistent icon rendering on Wayland, X11, Windows, and macOS sub-windows.
#[must_use]
pub fn configure_viewport_builder(builder: egui::ViewportBuilder) -> egui::ViewportBuilder {
    builder.with_app_id(APP_ID).with_icon(app_window_icon())
}

/// Decodes raw PNG image bytes into an [`egui::IconData`].
///
/// Returns an error if the byte slice is not a valid PNG image or cannot be decoded.
pub fn load_icon_from_png_bytes(bytes: &[u8]) -> Result<egui::IconData, String> {
    let img = image::load_from_memory(bytes)
        .map_err(|err| format!("Failed to decode icon PNG: {err}"))?;
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    Ok(egui::IconData {
        rgba: rgba.into_raw(),
        width,
        height,
    })
}

/// Embedded icon assets for multi-resolution Linux hicolor theme registration.
pub const LINUX_ICONS: &[(&str, &[u8])] = &[
    (
        "scalable/apps/azo-type.svg",
        include_bytes!("../../icons/icon.svg"),
    ),
    (
        "512x512/apps/azo-type.png",
        include_bytes!("../../icons/icon.png"),
    ),
    (
        "256x256/apps/azo-type.png",
        include_bytes!("../../icons/win-256.png"),
    ),
    (
        "128x128/apps/azo-type.png",
        include_bytes!("../../icons/win-128.png"),
    ),
    (
        "64x64/apps/azo-type.png",
        include_bytes!("../../icons/win-64.png"),
    ),
    (
        "48x48/apps/azo-type.png",
        include_bytes!("../../icons/win-48.png"),
    ),
    (
        "32x32/apps/azo-type.png",
        include_bytes!("../../icons/win-32.png"),
    ),
    (
        "16x16/apps/azo-type.png",
        include_bytes!("../../icons/win-16.png"),
    ),
];

/// Installs the `.desktop` file and hicolor icon assets into `data_dir` (e.g. `~/.local/share`).
///
/// On Wayland (KDE Plasma, GNOME, etc.), window managers do not accept in-protocol window icon buffers;
/// they look up the application icon from installed `.desktop` files matching the window's `app_id`.
pub fn ensure_linux_desktop_integration_to_dir(
    data_dir: &std::path::Path,
    exe_path: Option<&std::path::Path>,
) -> Result<(), String> {
    let icons_base = data_dir.join("icons").join("hicolor");
    for (rel_path, bytes) in LINUX_ICONS {
        let target = icons_base.join(rel_path);
        if let Some(parent) = target.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let should_write = match std::fs::read(&target) {
            Ok(existing) => existing != *bytes,
            Err(_) => true,
        };
        if should_write {
            let _ = std::fs::write(&target, bytes);
        }
    }

    let apps_dir = data_dir.join("applications");
    let _ = std::fs::create_dir_all(&apps_dir);
    let desktop_file = apps_dir.join(format!("{APP_ID}.desktop"));

    let exec_cmd = if let Some(exe) = exe_path {
        format!("\"{}\" %F", exe.display())
    } else {
        format!("{APP_ID} %F")
    };

    let desktop_content = format!(
        "[Desktop Entry]\n\
        Type=Application\n\
        Name=Azo Type\n\
        GenericName=Typst IDE\n\
        Comment=Native cross-platform Typst IDE with incremental preview and P2P collaboration\n\
        Exec={exec_cmd}\n\
        Icon={APP_ID}\n\
        Terminal=false\n\
        Categories=Development;IDE;TextEditor;\n\
        MimeType=text/x-typst;application/x-typst;\n\
        StartupWMClass={APP_ID}\n\
        Keywords=typst;editor;latex;ide;preview;collaboration;\n"
    );

    let should_write_desktop = match std::fs::read_to_string(&desktop_file) {
        Ok(existing) => existing != desktop_content,
        Err(_) => true,
    };
    if should_write_desktop {
        let _ = std::fs::write(&desktop_file, desktop_content);
    }

    Ok(())
}

/// Automatically ensures desktop integration and icon installation on Linux.
///
/// On Wayland, compositors require an installed desktop entry matching `app_id` to display
/// the application icon in the taskbar and application switcher. This function installs
/// the desktop entry and all hicolor icon resolutions into `~/.local/share` on startup.
#[cfg(target_os = "linux")]
pub fn ensure_linux_desktop_integration() {
    let data_dir = match std::env::var_os("XDG_DATA_HOME") {
        Some(val) => std::path::PathBuf::from(val),
        None => match std::env::var_os("HOME") {
            Some(home) => std::path::PathBuf::from(home).join(".local").join("share"),
            None => return,
        },
    };

    let current_exe = std::env::current_exe().ok();
    let _ = ensure_linux_desktop_integration_to_dir(&data_dir, current_exe.as_deref());

    // Best-effort cache refresh in a detached background thread
    let apps_path = data_dir.join("applications");
    std::thread::spawn(move || {
        let _ = std::process::Command::new("update-desktop-database")
            .arg(apps_path)
            .output();
        let _ = std::process::Command::new("kbuildsycoca6").output();
    });
}

/// No-op on non-Linux platforms.
#[cfg(not(target_os = "linux"))]
pub fn ensure_linux_desktop_integration() {}

#[cfg(test)]
mod tests;
