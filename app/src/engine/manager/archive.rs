//! SHA-256 verification, release archive extraction, and cross-platform engine/package cache paths.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use uuid::Uuid;

use crate::core::engine::{verify_sha256_hex, EngineArchiveKind};

/// Verifies the downloaded archive's SHA-256 digest before extracting `binary_name` into `dest_version_dir`.
pub(crate) fn verify_and_extract_archive(
    version: &str,
    archive_bytes: &[u8],
    expected_sha256: &str,
    archive_kind: EngineArchiveKind,
    binary_name: &str,
    dest_version_dir: &Path,
) -> Result<PathBuf, String> {
    if !verify_sha256_hex(archive_bytes, expected_sha256) {
        return Err(format!(
            "Security check rejected Typst {version} download: SHA-256 checksum mismatch."
        ));
    }

    let work_dir = std::env::temp_dir().join(format!("azo-engine-dl-{}", Uuid::new_v4()));
    let extract_dir = work_dir.join("extracted");
    fs::create_dir_all(&extract_dir)
        .map_err(|e| format!("Failed creating temporary extraction directory: {e}"))?;

    let res = (|| -> Result<PathBuf, String> {
        let ext = match archive_kind {
            EngineArchiveKind::TarXz => "tar.xz",
            EngineArchiveKind::Zip => "zip",
        };
        let archive_path = work_dir.join(format!("typst-{version}.{ext}"));
        fs::write(&archive_path, archive_bytes)
            .map_err(|e| format!("Failed writing downloaded archive to temp path: {e}"))?;

        extract_archive(&archive_path, archive_kind, &extract_dir)?;

        let found_bin = find_file_recursive(&extract_dir, binary_name, 4).ok_or_else(|| {
            format!(
                "Extracted archive for Typst {version} did not contain expected binary '{binary_name}'."
            )
        })?;

        fs::create_dir_all(dest_version_dir)
            .map_err(|e| format!("Failed creating engine cache directory: {e}"))?;
        let final_path = dest_version_dir.join(binary_name);
        fs::copy(&found_bin, &final_path).map_err(|e| {
            format!(
                "Failed installing Typst {version} binary to '{}': {e}",
                final_path.display()
            )
        })?;
        ensure_executable(&final_path)?;
        Ok(final_path)
    })();

    let _ = fs::remove_dir_all(&work_dir);
    res
}

fn extract_archive(
    archive_path: &Path,
    archive_kind: EngineArchiveKind,
    extract_dir: &Path,
) -> Result<(), String> {
    let status = Command::new("tar")
        .arg("-xf")
        .arg(archive_path)
        .arg("-C")
        .arg(extract_dir)
        .status();

    match status {
        Ok(st) if st.success() => return Ok(()),
        Ok(st) if archive_kind == EngineArchiveKind::TarXz => {
            return Err(format!("Failed extracting archive with `tar` (exit {st})."));
        }
        Err(e) if archive_kind == EngineArchiveKind::TarXz => {
            return Err(format!("Failed spawning `tar` to extract archive: {e}"));
        }
        _ => {}
    }

    // Windows PowerShell fallback for ZIP archives if `tar` is unavailable or fails.
    #[cfg(target_os = "windows")]
    {
        let script = format!(
            "Expand-Archive -LiteralPath '{}' -DestinationPath '{}' -Force",
            archive_path.display(),
            extract_dir.display()
        );
        let ps_status = Command::new("powershell")
            .arg("-NoProfile")
            .arg("-NonInteractive")
            .arg("-Command")
            .arg(script)
            .status()
            .map_err(|e| format!("Failed spawning PowerShell Expand-Archive: {e}"))?;
        if ps_status.success() {
            return Ok(());
        }
        return Err(format!(
            "PowerShell Expand-Archive failed with exit {ps_status}."
        ));
    }

    #[cfg(not(target_os = "windows"))]
    Err("Failed extracting ZIP archive with `tar`.".to_string())
}

fn find_file_recursive(dir: &Path, target_name: &str, max_depth: usize) -> Option<PathBuf> {
    let entries = fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && path.file_name().and_then(|n| n.to_str()) == Some(target_name) {
            return Some(path);
        }
        if max_depth > 0 && path.is_dir() {
            if let Some(found) = find_file_recursive(&path, target_name, max_depth - 1) {
                return Some(found);
            }
        }
    }
    None
}

#[cfg(unix)]
fn ensure_executable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(path)
        .map_err(|e| format!("Failed reading permissions for '{}': {e}", path.display()))?
        .permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms)
        .map_err(|e| format!("Failed setting executable bit on '{}': {e}", path.display()))
}

#[cfg(not(unix))]
fn ensure_executable(_path: &Path) -> Result<(), String> {
    Ok(())
}

/// Resolves the cross-platform user data directory used to store downloaded Typst binaries.
pub fn default_engines_cache_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            return PathBuf::from(local).join("azo-type").join("engines");
        }
        if let Ok(roaming) = std::env::var("APPDATA") {
            return PathBuf::from(roaming).join("azo-type").join("engines");
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("azo-type")
                .join("engines");
        }
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
            if !xdg.is_empty() {
                return PathBuf::from(xdg).join("azo-type").join("engines");
            }
        }
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home)
                .join(".local")
                .join("share")
                .join("azo-type")
                .join("engines");
        }
    }
    std::env::temp_dir().join("azo-type").join("engines")
}

/// Resolves the host system's standard Typst Universe package cache directory so legacy
/// external binaries share already-downloaded packages with the built-in compiler.
pub fn default_typst_package_cache_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        return std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .map(|p| p.join("typst").join("packages"));
    }
    #[cfg(target_os = "macos")]
    {
        return std::env::var_os("HOME").map(PathBuf::from).map(|p| {
            p.join("Library")
                .join("Caches")
                .join("typst")
                .join("packages")
        });
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Some(xdg) = std::env::var_os("XDG_CACHE_HOME") {
            if !xdg.is_empty() {
                return Some(PathBuf::from(xdg).join("typst").join("packages"));
            }
        }
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|p| p.join(".cache").join("typst").join("packages"))
    }
}
