//! Cross-platform OS dark/light theme detection (`System-Default`) with safe Dark fallback.

#[cfg(test)]
#[path = "../tests/platform/theme_tests.rs"]
mod tests;

#[cfg(target_os = "linux")]
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
#[cfg(target_os = "linux")]
use std::time::{SystemTime, UNIX_EPOCH};

use egui::Context;

use crate::core::settings::ThemePreference;

/// Resolves the effective `dark_mode` boolean for `preference`, querying OS appearance if needed.
///
/// Defaults to `true` (Dark theme) whenever OS theme detection is unavailable.
pub fn resolve_effective_dark_mode(ctx: &Context, preference: ThemePreference) -> bool {
    let system_dark = if preference == ThemePreference::SystemDefault {
        detect_system_dark_mode(ctx)
    } else {
        None
    };
    preference.resolve_dark_mode(system_dark)
}

/// Detects the operating system's current dark/light theme preference (`None` if unknown).
///
/// Uses `egui`/`winit` native detection (Windows registry, macOS AppKit `effectiveAppearance`,
/// and live `WindowEvent::ThemeChanged` events) and falls back to non-blocking Linux portal,
/// GNOME, GTK, and KDE probes.
pub fn detect_system_dark_mode(ctx: &Context) -> Option<bool> {
    if let Some(theme) = ctx.system_theme() {
        return Some(matches!(theme, egui::Theme::Dark));
    }

    #[cfg(target_os = "linux")]
    {
        detect_linux_system_dark_mode_cached(ctx)
    }

    #[cfg(not(target_os = "linux"))]
    None
}

/// Parses XDG Desktop Portal `org.freedesktop.appearance.color-scheme` CLI output.
///
/// Spec values: `1` = prefer dark, `2` = prefer light, `0` = no preference (`None`).
pub fn parse_portal_color_scheme(stdout: &str) -> Option<bool> {
    let normalized = stdout.trim();
    if normalized.contains("uint32 1")
        || normalized.contains(" 1)")
        || normalized.contains(" 1>")
        || normalized == "1"
    {
        Some(true)
    } else if normalized.contains("uint32 2")
        || normalized.contains(" 2)")
        || normalized.contains(" 2>")
        || normalized == "2"
    {
        Some(false)
    } else {
        None
    }
}

/// Parses `gsettings get org.gnome.desktop.interface color-scheme` output.
pub fn parse_gsettings_color_scheme(stdout: &str) -> Option<bool> {
    let lower = stdout.trim().trim_matches('\'').to_ascii_lowercase();
    if lower == "prefer-dark" {
        Some(true)
    } else if lower == "prefer-light" {
        Some(false)
    } else {
        None
    }
}

/// Parses `gsettings get org.gnome.desktop.interface gtk-theme` or theme name strings.
pub fn parse_gtk_theme_name_is_dark(theme_name: &str) -> Option<bool> {
    let cleaned = theme_name.trim().trim_matches('\'').to_ascii_lowercase();
    if cleaned.is_empty() {
        return None;
    }
    if cleaned.contains("dark") {
        Some(true)
    } else if cleaned.contains("light") {
        Some(false)
    } else {
        None
    }
}

/// Parses `gtk-3.0/settings.ini` / `gtk-4.0/settings.ini` contents for dark-theme preference.
pub fn parse_gtk_settings_ini(contents: &str) -> Option<bool> {
    let mut theme_name_guess: Option<bool> = None;
    for raw_line in contents.lines() {
        let line = raw_line.split('#').next().unwrap_or("").trim();
        if let Some((key, val)) = line.split_once('=') {
            let key = key.trim().to_ascii_lowercase();
            let val = val.trim();
            if key == "gtk-application-prefer-dark-theme" {
                let v = val.to_ascii_lowercase();
                if v == "1" || v == "true" {
                    return Some(true);
                } else if v == "0" || v == "false" {
                    return Some(false);
                }
            } else if key == "gtk-theme-name" {
                theme_name_guess = parse_gtk_theme_name_is_dark(val);
            }
        }
    }
    theme_name_guess
}

/// Parses KDE `~/.config/kdeglobals` contents for dark vs light color scheme hints.
pub fn parse_kdeglobals(contents: &str) -> Option<bool> {
    let mut look_and_feel_guess: Option<bool> = None;
    for raw_line in contents.lines() {
        let line = raw_line.trim();
        if line.starts_with('#') || line.starts_with('[') {
            continue;
        }
        if let Some((key, val)) = line.split_once('=') {
            let key = key.trim();
            let val_lower = val.trim().to_ascii_lowercase();
            if key.eq_ignore_ascii_case("ColorScheme") {
                if val_lower.contains("dark") {
                    return Some(true);
                }
                if val_lower.contains("light") || val_lower == "breeze" {
                    return Some(false);
                }
            } else if key.eq_ignore_ascii_case("LookAndFeelPackage") {
                if val_lower.contains("dark") {
                    look_and_feel_guess = Some(true);
                } else if val_lower.contains("light") || val_lower.ends_with(".breeze.desktop") {
                    look_and_feel_guess = Some(false);
                }
            }
        }
    }
    look_and_feel_guess
}

#[cfg(target_os = "linux")]
const CACHE_UNSET: u8 = 0;
#[cfg(target_os = "linux")]
const CACHE_DARK: u8 = 1;
#[cfg(target_os = "linux")]
const CACHE_LIGHT: u8 = 2;
#[cfg(target_os = "linux")]
const CACHE_UNKNOWN: u8 = 3;

#[cfg(target_os = "linux")]
static LINUX_THEME_CACHE: AtomicU8 = AtomicU8::new(CACHE_UNSET);
#[cfg(target_os = "linux")]
static LINUX_PROBE_RUNNING: AtomicBool = AtomicBool::new(false);
#[cfg(target_os = "linux")]
static LINUX_LAST_PROBE_SECS: AtomicU64 = AtomicU64::new(0);

#[cfg(target_os = "linux")]
fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(target_os = "linux")]
fn detect_linux_system_dark_mode_cached(ctx: &Context) -> Option<bool> {
    let now = now_unix_secs();
    let last = LINUX_LAST_PROBE_SECS.load(Ordering::Relaxed);
    let cache = LINUX_THEME_CACHE.load(Ordering::Relaxed);

    if (cache == CACHE_UNSET || now.saturating_sub(last) >= 5)
        && LINUX_PROBE_RUNNING
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    {
        LINUX_LAST_PROBE_SECS.store(now, Ordering::Relaxed);
        let ctx_clone = ctx.clone();
        std::thread::spawn(move || {
            let detected = probe_linux_theme_sync();
            let encoded = match detected {
                Some(true) => CACHE_DARK,
                Some(false) => CACHE_LIGHT,
                None => CACHE_UNKNOWN,
            };
            let prev = LINUX_THEME_CACHE.swap(encoded, Ordering::SeqCst);
            LINUX_PROBE_RUNNING.store(false, Ordering::SeqCst);
            if prev != encoded {
                ctx_clone.request_repaint();
            }
        });
    }

    match LINUX_THEME_CACHE.load(Ordering::Relaxed) {
        CACHE_DARK => Some(true),
        CACHE_LIGHT => Some(false),
        CACHE_UNKNOWN => None,
        _ => probe_linux_config_files_only(),
    }
}

#[cfg(target_os = "linux")]
fn probe_linux_theme_sync() -> Option<bool> {
    use std::process::Command;

    if let Ok(out) = Command::new("gdbus")
        .args([
            "call",
            "--session",
            "--dest",
            "org.freedesktop.portal.Desktop",
            "--object-path",
            "/org/freedesktop/portal/desktop",
            "--method",
            "org.freedesktop.portal.Settings.Read",
            "org.freedesktop.appearance",
            "color-scheme",
        ])
        .output()
    {
        if out.status.success() {
            let stdout = String::from_utf8_lossy(&out.stdout);
            if let Some(dark) = parse_portal_color_scheme(&stdout) {
                return Some(dark);
            }
        }
    }

    if let Ok(out) = Command::new("dbus-send")
        .args([
            "--session",
            "--print-reply=literal",
            "--dest=org.freedesktop.portal.Desktop",
            "/org/freedesktop/portal/desktop",
            "org.freedesktop.portal.Settings.Read",
            "string:org.freedesktop.appearance",
            "string:color-scheme",
        ])
        .output()
    {
        if out.status.success() {
            let stdout = String::from_utf8_lossy(&out.stdout);
            if let Some(dark) = parse_portal_color_scheme(&stdout) {
                return Some(dark);
            }
        }
    }

    if let Ok(out) = Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "color-scheme"])
        .output()
    {
        if out.status.success() {
            let stdout = String::from_utf8_lossy(&out.stdout);
            if let Some(dark) = parse_gsettings_color_scheme(&stdout) {
                return Some(dark);
            }
        }
    }

    if let Ok(out) = Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "gtk-theme"])
        .output()
    {
        if out.status.success() {
            let stdout = String::from_utf8_lossy(&out.stdout);
            if let Some(dark) = parse_gtk_theme_name_is_dark(&stdout) {
                return Some(dark);
            }
        }
    }

    probe_linux_config_files_only()
}

#[cfg(target_os = "linux")]
fn probe_linux_config_files_only() -> Option<bool> {
    if let Ok(gtk_theme) = std::env::var("GTK_THEME") {
        if let Some(dark) = parse_gtk_theme_name_is_dark(&gtk_theme) {
            return Some(dark);
        }
    }

    let config_dir = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config"))
        })?;

    for gtk_ver in ["gtk-4.0", "gtk-3.0"] {
        let path = config_dir.join(gtk_ver).join("settings.ini");
        if let Ok(contents) = std::fs::read_to_string(path) {
            if let Some(dark) = parse_gtk_settings_ini(&contents) {
                return Some(dark);
            }
        }
    }

    let kdeglobals = config_dir.join("kdeglobals");
    if let Ok(contents) = std::fs::read_to_string(kdeglobals) {
        if let Some(dark) = parse_kdeglobals(&contents) {
            return Some(dark);
        }
    }

    None
}
