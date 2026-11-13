//! Cross-platform Typst engine registry, async HTTPS downloader, and cache manager.

mod archive;
#[cfg(test)]
mod tests;

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};

use crate::compiler::NativeTypstEngine;
use crate::core::engine::{
    find_host_asset, find_release_for_version, host_target_triple, EngineArchiveKind,
    BUILTIN_TYPST_VERSION,
};
use crate::engine::types::{CompileSnapshot, TypstEngine};
use crate::engine::worker::ExternalWorkerEngine;
use crate::project::DiskProject;
use crate::world::SharedWorldResources;

pub use archive::{default_engines_cache_dir, default_typst_package_cache_dir};
use archive::verify_and_extract_archive;

/// Local installation status of a cataloged Typst engine version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineInstallState {
    /// Compiled directly into this binary (cannot be removed).
    BuiltIn,
    /// Downloaded, checksum-verified, and cached locally.
    Installed,
    /// Available in catalog for on-demand download.
    NotInstalled,
    /// Asynchronous download/extraction is currently in progress.
    Downloading,
    /// Release does not provide a binary for the current host target triple.
    UnsupportedPlatform,
}

/// Completion event emitted on the background download channel to the UI thread.
#[derive(Debug, Clone)]
pub struct EngineDownloadEvent {
    pub version: String,
    pub result: Result<PathBuf, String>,
}

/// Coordinates engine selection, cached binary resolution, and background verified downloads.
pub struct EngineManager {
    builtin_resources: Arc<SharedWorldResources>,
    engines_dir: PathBuf,
    package_cache_dir: Option<PathBuf>,
    downloading: Arc<Mutex<HashSet<String>>>,
    last_error: Arc<Mutex<Option<String>>>,
}

impl EngineManager {
    /// Creates an engine manager using standard OS-local cache directories.
    pub fn new(builtin_resources: Arc<SharedWorldResources>) -> Self {
        Self::with_paths(
            builtin_resources,
            default_engines_cache_dir(),
            default_typst_package_cache_dir(),
        )
    }

    /// Creates an engine manager pointing at explicit cache directories (useful for isolated tests).
    pub fn with_paths(
        builtin_resources: Arc<SharedWorldResources>,
        engines_dir: PathBuf,
        package_cache_dir: Option<PathBuf>,
    ) -> Self {
        Self {
            builtin_resources,
            engines_dir,
            package_cache_dir,
            downloading: Arc::new(Mutex::new(HashSet::new())),
            last_error: Arc::new(Mutex::new(None)),
        }
    }

    /// Returns the base directory where downloaded Typst engine binaries are stored.
    pub fn engines_dir(&self) -> &Path {
        &self.engines_dir
    }

    /// Returns the shared host Typst Universe package cache directory, if known.
    pub fn package_cache_dir(&self) -> Option<&PathBuf> {
        self.package_cache_dir.as_ref()
    }

    /// Reports the local state of a Typst engine version.
    pub fn install_state(&self, version: &str) -> EngineInstallState {
        if version == BUILTIN_TYPST_VERSION {
            return EngineInstallState::BuiltIn;
        }
        if self.is_downloading(version) {
            return EngineInstallState::Downloading;
        }
        if find_release_for_version(version).is_none() || find_host_asset(version).is_none() {
            return EngineInstallState::UnsupportedPlatform;
        }
        if self.installed_version_path(version).is_some() {
            EngineInstallState::Installed
        } else {
            EngineInstallState::NotInstalled
        }
    }

    /// Returns true if a download for `version` is currently active.
    pub fn is_downloading(&self, version: &str) -> bool {
        self.downloading
            .lock()
            .map(|set| set.contains(version))
            .unwrap_or(false)
    }

    /// Returns the version currently being downloaded, if any.
    pub fn downloading_version(&self) -> Option<String> {
        self.downloading
            .lock()
            .ok()
            .and_then(|set| set.iter().next().cloned())
    }

    /// Returns the most recent engine download/verification error, if any.
    pub fn last_error(&self) -> Option<String> {
        self.last_error.lock().ok().and_then(|err| err.clone())
    }

    /// Clears the last recorded engine error message.
    pub fn clear_error(&self) {
        if let Ok(mut guard) = self.last_error.lock() {
            *guard = None;
        }
    }

    /// Records or clears an engine error produced by a completed download event.
    pub fn record_download_outcome(&self, event: &EngineDownloadEvent) {
        if let Ok(mut guard) = self.last_error.lock() {
            match &event.result {
                Ok(_) => *guard = None,
                Err(msg) => *guard = Some(msg.clone()),
            }
        }
    }

    /// Returns the cached executable path for `version` if present on disk.
    pub fn installed_version_path(&self, version: &str) -> Option<PathBuf> {
        if version == BUILTIN_TYPST_VERSION {
            return None;
        }
        let binary_name = find_host_asset(version)
            .map(|a| a.binary_name)
            .unwrap_or(default_exe_filename());

        let direct = self.engines_dir.join(version).join(binary_name);
        if direct.is_file() {
            return Some(direct);
        }
        None
    }

    /// Resolves `requested_version` to a concrete [`TypstEngine`], falling back cleanly to the
    /// bundled `0.15.1` compiler with a user-facing notice when a cached binary is unavailable.
    pub fn resolve_engine(&self, requested_version: &str) -> (Arc<dyn TypstEngine>, Option<String>) {
        if requested_version == BUILTIN_TYPST_VERSION {
            return (
                Arc::new(NativeTypstEngine::new(Arc::clone(&self.builtin_resources))),
                None,
            );
        }

        if find_release_for_version(requested_version).is_none() {
            return (
                Arc::new(NativeTypstEngine::new(Arc::clone(&self.builtin_resources))),
                Some(format!(
                    "Typst {} is not in the catalog; using built-in {}.",
                    requested_version, BUILTIN_TYPST_VERSION
                )),
            );
        }

        if find_host_asset(requested_version).is_none() {
            return (
                Arc::new(NativeTypstEngine::new(Arc::clone(&self.builtin_resources))),
                Some(format!(
                    "Typst {} has no binary for {}; using built-in {}.",
                    requested_version,
                    host_target_triple().unwrap_or("unknown"),
                    BUILTIN_TYPST_VERSION
                )),
            );
        }

        if let Some(bin_path) = self.installed_version_path(requested_version) {
            return (
                Arc::new(ExternalWorkerEngine::new(
                    requested_version.to_string(),
                    bin_path,
                    self.package_cache_dir.clone(),
                )),
                None,
            );
        }

        (
            Arc::new(NativeTypstEngine::new(Arc::clone(&self.builtin_resources))),
            Some(format!(
                "Typst {} is not downloaded yet; using built-in {}.",
                requested_version, BUILTIN_TYPST_VERSION
            )),
        )
    }

    /// Returns the active resolved version string and optional fallback notice for status display.
    pub fn active_version_and_fallback(&self, requested_version: &str) -> (String, Option<String>) {
        let (engine, notice) = self.resolve_engine(requested_version);
        (engine.metadata().version, notice)
    }

    /// Spawns a background compilation thread using the resolved engine for `requested_version`.
    pub fn spawn_compile(
        self: &Arc<Self>,
        requested_version: &str,
        project: DiskProject,
        pixel_per_pt: f32,
        tx: Sender<CompileSnapshot>,
        ctx: egui::Context,
    ) {
        let mgr = Arc::clone(self);
        let requested = requested_version.to_string();
        std::thread::spawn(move || {
            let (engine, _) = mgr.resolve_engine(&requested);
            let snapshot = engine.compile_project(&project, pixel_per_pt);
            let _ = tx.send(snapshot);
            ctx.request_repaint();
        });
    }

    /// Applies a completed background engine download event, returning `(status_notice, should_recompile)`.
    pub fn apply_download_event(
        &self,
        event: EngineDownloadEvent,
        selected_version: &mut String,
        fallback_notice: &mut Option<String>,
    ) -> (String, bool) {
        self.record_download_outcome(&event);
        match event.result {
            Ok(_) => {
                *selected_version = event.version.clone();
                *fallback_notice = None;
                let msg =
                    rust_i18n::t!("app.status_engine_ready", version = event.version.as_str())
                        .to_string();
                (msg, true)
            }
            Err(err) => {
                let msg = rust_i18n::t!("app.status_engine_download_failed", error = err.as_str())
                    .to_string();
                (msg, false)
            }
        }
    }

    /// Applies settings-dialog engine requests, returning `(optional_status_notice, should_recompile)`.
    #[allow(clippy::too_many_arguments)]
    pub fn handle_settings_actions(
        &self,
        download_version: Option<String>,
        remove_version: Option<String>,
        engine_changed: bool,
        selected_version: &str,
        fallback_notice: &mut Option<String>,
        tx: &Sender<EngineDownloadEvent>,
        ctx: &egui::Context,
    ) -> (Option<String>, bool) {
        let mut status = None;
        let mut recompile = engine_changed;

        if let Some(ver) = download_version {
            status = Some(
                rust_i18n::t!("app.status_engine_downloading", version = ver.as_str()).to_string(),
            );
            self.spawn_download_engine(&ver, tx.clone(), ctx.clone());
        }
        if let Some(ver) = remove_version {
            match self.remove_cached_engine(&ver) {
                Ok(()) => {
                    status = Some(
                        rust_i18n::t!("app.status_engine_removed", version = ver.as_str())
                            .to_string(),
                    );
                    recompile = true;
                }
                Err(err) => status = Some(err),
            }
        }
        if recompile {
            let (_, fb) = self.active_version_and_fallback(selected_version);
            *fallback_notice = fb;
        }
        (status, recompile)
    }

    /// Removes a previously downloaded version from the local cache directory.
    pub fn remove_cached_engine(&self, version: &str) -> Result<(), String> {
        if version == BUILTIN_TYPST_VERSION {
            return Err("Built-in Typst engine cannot be removed.".to_string());
        }
        let dir = self.engines_dir.join(version);
        if dir.exists() {
            fs::remove_dir_all(&dir).map_err(|e| {
                format!(
                    "Failed to delete cached Typst {} directory '{}': {e}",
                    version,
                    dir.display()
                )
            })?;
        }
        Ok(())
    }

    /// Downloads, SHA-256 verifies, and installs `version` in a background thread.
    pub fn spawn_download_engine(
        &self,
        version: &str,
        tx: Sender<EngineDownloadEvent>,
        ctx: egui::Context,
    ) {
        if find_release_for_version(version).is_none() {
            let msg = format!("Unknown Typst version '{version}'.");
            if let Ok(mut guard) = self.last_error.lock() {
                *guard = Some(msg.clone());
            }
            let _ = tx.send(EngineDownloadEvent {
                version: version.to_string(),
                result: Err(msg),
            });
            ctx.request_repaint();
            return;
        }

        let Some(asset) = find_host_asset(version) else {
            let msg = format!(
                "No downloadable Typst {version} artifact for platform '{}'.",
                host_target_triple().unwrap_or("unknown")
            );
            if let Ok(mut guard) = self.last_error.lock() {
                *guard = Some(msg.clone());
            }
            let _ = tx.send(EngineDownloadEvent {
                version: version.to_string(),
                result: Err(msg),
            });
            ctx.request_repaint();
            return;
        };

        {
            let Ok(mut set) = self.downloading.lock() else {
                return;
            };
            if !set.insert(version.to_string()) {
                return;
            }
        }
        self.clear_error();

        let version_owned = version.to_string();
        let dest_version_dir = self.engines_dir.join(version);
        let downloading_flag = Arc::clone(&self.downloading);

        std::thread::spawn(move || {
            let result = download_and_install_blocking(
                &version_owned,
                asset.download_url,
                asset.sha256_hex,
                asset.archive_kind,
                asset.binary_name,
                &dest_version_dir,
            );

            if let Ok(mut set) = downloading_flag.lock() {
                set.remove(&version_owned);
            }

            let _ = tx.send(EngineDownloadEvent {
                version: version_owned,
                result,
            });
            ctx.request_repaint();
        });
    }
}

fn download_and_install_blocking(
    version: &str,
    url: &str,
    expected_sha256: &str,
    archive_kind: EngineArchiveKind,
    binary_name: &str,
    dest_version_dir: &Path,
) -> Result<PathBuf, String> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("Failed initializing async download runtime: {e}"))?;

    let bytes = rt.block_on(async {
        let client = reqwest::Client::builder()
            .user_agent("azo-type/0.1.0")
            .build()
            .map_err(|e| format!("Failed building HTTPS client: {e}"))?;

        let response = client
            .get(url)
            .send()
            .await
            .map_err(|e| format!("Failed downloading Typst {version}: {e}"))?;

        if !response.status().is_success() {
            return Err(format!(
                "Download failed for Typst {version} with HTTP {}",
                response.status()
            ));
        }

        response
            .bytes()
            .await
            .map(|b| b.to_vec())
            .map_err(|e| format!("Failed reading Typst {version} download stream: {e}"))
    })?;

    verify_and_extract_archive(
        version,
        &bytes,
        expected_sha256,
        archive_kind,
        binary_name,
        dest_version_dir,
    )
}

fn default_exe_filename() -> &'static str {
    if cfg!(target_os = "windows") {
        "typst.exe"
    } else {
        "typst"
    }
}

/// Bundles [`EngineManager`], async download channels, and active fallback state for `AzoTypeApp`.
pub struct EngineHost {
    pub manager: Arc<EngineManager>,
    pub tx: Sender<EngineDownloadEvent>,
    pub rx: Receiver<EngineDownloadEvent>,
    pub fallback_notice: Option<String>,
}

impl EngineHost {
    /// Initializes the engine manager, compiles the initial project snapshot, and opens channels.
    pub fn init(
        resources: Arc<SharedWorldResources>,
        project: &DiskProject,
        requested_version: &str,
        pixel_per_pt: f32,
    ) -> (Self, CompileSnapshot) {
        let manager = Arc::new(EngineManager::new(resources));
        let (engine, fallback_notice) = manager.resolve_engine(requested_version);
        let initial_snapshot = engine.compile_project(project, pixel_per_pt);
        let (tx, rx) = mpsc::channel();
        (
            Self {
                manager,
                tx,
                rx,
                fallback_notice,
            },
            initial_snapshot,
        )
    }

    /// Refreshes fallback notice and starts a background compilation thread.
    pub fn trigger_compile(
        &mut self,
        requested_version: &str,
        project: DiskProject,
        pixel_per_pt: f32,
        tx: Sender<CompileSnapshot>,
        ctx: egui::Context,
    ) {
        let (_, fb) = self.manager.active_version_and_fallback(requested_version);
        self.fallback_notice = fb;
        self.manager
            .spawn_compile(requested_version, project, pixel_per_pt, tx, ctx);
    }

    /// Drains finished background download events, returning `(latest_notice, should_recompile)`.
    pub fn poll_downloads(&mut self, selected_version: &mut String) -> (Option<String>, bool) {
        let mut latest_notice = None;
        let mut should_recompile = false;
        while let Ok(ev) = self.rx.try_recv() {
            let (msg, recompile) = self.manager.apply_download_event(
                ev,
                selected_version,
                &mut self.fallback_notice,
            );
            latest_notice = Some(msg);
            should_recompile |= recompile;
        }
        (latest_notice, should_recompile)
    }
}
