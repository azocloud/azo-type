//! On-demand Tier-1 Hunspell dictionary and `nlprule` grammar model downloader and cache manager.

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use flate2::read::GzDecoder;
use uuid::Uuid;

use crate::core::engine::verify_sha256_hex;
use crate::core::spellcheck::{
    find_tier1_pack, AssetCompression, DictionaryFileAsset, DictionaryFileRole, Tier1LanguagePack,
};
use crate::engine::manager::ensure_tls_crypto_provider;

/// Installation state for a Tier-1 language pack on disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DictionaryPackStatus {
    NotInstalled,
    Downloading,
    Installed,
}

/// Live progress snapshot for an active Tier-1 language pack download.
#[derive(Debug, Clone)]
pub struct DictionaryDownloadProgress {
    pub lang_code: String,
    pub display_name: String,
    pub current_file_index: usize,
    pub total_files: usize,
    pub current_filename: String,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub started_at: Instant,
}

impl DictionaryDownloadProgress {
    /// Returns overall completion fraction in `0.0..=1.0`.
    pub fn fraction(&self) -> f32 {
        if self.total_bytes == 0 {
            return 0.0;
        }
        (self.downloaded_bytes as f64 / self.total_bytes as f64).clamp(0.0, 1.0) as f32
    }

    /// Elapsed seconds since download started.
    pub fn elapsed_secs(&self) -> f64 {
        self.started_at.elapsed().as_secs_f64()
    }
}

/// Resolved local file paths for an installed Tier-1 language pack.
#[derive(Debug, Clone)]
pub struct InstalledTier1Paths {
    pub aff_path: PathBuf,
    pub dic_path: PathBuf,
    pub tokenizer_path: Option<PathBuf>,
    pub rules_path: Option<PathBuf>,
}

/// Event sent from download workers to the language-check coordinator.
#[derive(Debug, Clone)]
pub struct DictionaryDownloadOutcome {
    pub lang_code: String,
    pub result: Result<(), String>,
}

/// Manages local storage and verified HTTPS downloads for Tier-1 language packs.
#[derive(Clone)]
pub struct DictionaryManager {
    cache_dir: PathBuf,
    downloads: Arc<Mutex<HashMap<String, DictionaryDownloadProgress>>>,
    last_error: Arc<Mutex<Option<String>>>,
}

impl Default for DictionaryManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DictionaryManager {
    /// Creates a manager rooted at the standard OS-local `azo-type/dictionaries` directory.
    pub fn new() -> Self {
        Self::with_dir(default_dictionaries_cache_dir())
    }

    /// Creates a manager rooted at an explicit cache directory (for isolated tests).
    pub fn with_dir(cache_dir: PathBuf) -> Self {
        Self {
            cache_dir,
            downloads: Arc::new(Mutex::new(HashMap::new())),
            last_error: Arc::new(Mutex::new(None)),
        }
    }

    /// Returns the base directory holding cached Tier-1 language packs.
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// Returns the directory for a specific primary language code (e.g. `"en"`, `"de"`).
    pub fn pack_dir(&self, lang_code: &str) -> PathBuf {
        self.cache_dir.join(lang_code)
    }

    /// Returns installation status for `lang_code`.
    pub fn pack_status(&self, lang_code: &str) -> DictionaryPackStatus {
        if self.is_downloading(lang_code) {
            return DictionaryPackStatus::Downloading;
        }
        if self.installed_paths(lang_code).is_some() {
            DictionaryPackStatus::Installed
        } else {
            DictionaryPackStatus::NotInstalled
        }
    }

    /// Returns true if `lang_code` is currently downloading.
    pub fn is_downloading(&self, lang_code: &str) -> bool {
        self.downloads
            .lock()
            .map(|m| m.contains_key(lang_code))
            .unwrap_or(false)
    }

    /// Returns active download progress for `lang_code`, if any.
    pub fn download_progress(&self, lang_code: &str) -> Option<DictionaryDownloadProgress> {
        self.downloads
            .lock()
            .ok()
            .and_then(|m| m.get(lang_code).cloned())
    }

    /// Returns any active download progress across all languages.
    pub fn any_download_progress(&self) -> Option<DictionaryDownloadProgress> {
        self.downloads
            .lock()
            .ok()
            .and_then(|m| m.values().next().cloned())
    }

    /// Returns the most recent download/installation error, if any.
    pub fn last_error(&self) -> Option<String> {
        self.last_error.lock().ok().and_then(|g| g.clone())
    }

    /// Clears the recorded download error.
    pub fn clear_error(&self) {
        if let Ok(mut g) = self.last_error.lock() {
            *g = None;
        }
    }

    /// Records a download error message.
    pub fn set_error(&self, msg: String) {
        if let Ok(mut g) = self.last_error.lock() {
            *g = Some(msg);
        }
    }

    /// Resolves local paths for `lang_code` only if every manifest file is present on disk.
    pub fn installed_paths(&self, lang_code: &str) -> Option<InstalledTier1Paths> {
        let (canonical, pack) = find_tier1_pack(lang_code)?;
        let dir = self.pack_dir(canonical);
        let mut aff_path = None;
        let mut dic_path = None;
        let mut tokenizer_path = None;
        let mut rules_path = None;

        for file in &pack.files {
            let candidate = dir.join(&file.filename);
            if !candidate.is_file() {
                return None;
            }
            match file.role {
                DictionaryFileRole::Aff => aff_path = Some(candidate),
                DictionaryFileRole::Dic => dic_path = Some(candidate),
                DictionaryFileRole::NlpruleTokenizer => tokenizer_path = Some(candidate),
                DictionaryFileRole::NlpruleRules => rules_path = Some(candidate),
            }
        }

        Some(InstalledTier1Paths {
            aff_path: aff_path?,
            dic_path: dic_path?,
            tokenizer_path,
            rules_path,
        })
    }

    /// Removes all cached files for `lang_code`.
    pub fn remove_pack(&self, lang_code: &str) -> Result<(), String> {
        let Some((canonical, _)) = find_tier1_pack(lang_code) else {
            return Err(format!("Unknown Tier-1 language pack: '{lang_code}'."));
        };
        let dir = self.pack_dir(canonical);
        if dir.exists() {
            fs::remove_dir_all(&dir).map_err(|e| {
                format!(
                    "Failed removing language pack '{}' at '{}': {e}",
                    canonical,
                    dir.display()
                )
            })?;
        }
        self.clear_error();
        Ok(())
    }

    /// Spawns a background download worker for all files in the Tier-1 language pack.
    pub fn spawn_download_pack(
        &self,
        lang_code: &str,
        tx: Sender<DictionaryDownloadOutcome>,
        ctx: egui::Context,
    ) {
        let Some((canonical, pack)) = find_tier1_pack(lang_code) else {
            let msg = format!("Language '{lang_code}' is not in the Tier-1 manifest.");
            self.set_error(msg.clone());
            let _ = tx.send(DictionaryDownloadOutcome {
                lang_code: lang_code.to_string(),
                result: Err(msg),
            });
            ctx.request_repaint();
            return;
        };

        {
            let Ok(mut map) = self.downloads.lock() else {
                return;
            };
            if map.contains_key(canonical) {
                return;
            }
            map.insert(
                canonical.to_string(),
                DictionaryDownloadProgress {
                    lang_code: canonical.to_string(),
                    display_name: pack.display_name.clone(),
                    current_file_index: 0,
                    total_files: pack.files.len(),
                    current_filename: pack
                        .files
                        .first()
                        .map(|f| f.filename.clone())
                        .unwrap_or_default(),
                    downloaded_bytes: 0,
                    total_bytes: pack.total_size_bytes(),
                    started_at: Instant::now(),
                },
            );
        }
        self.clear_error();

        let dest_dir = self.pack_dir(canonical);
        let downloads = Arc::clone(&self.downloads);
        let last_error = Arc::clone(&self.last_error);
        let canonical_owned = canonical.to_string();

        std::thread::spawn(move || {
            let result =
                download_pack_blocking(canonical_owned.as_str(), pack, &dest_dir, &downloads, &ctx);
            if let Ok(mut map) = downloads.lock() {
                map.remove(&canonical_owned);
            }
            if let Err(ref err_msg) = result {
                if let Ok(mut g) = last_error.lock() {
                    *g = Some(err_msg.clone());
                }
            }
            let _ = tx.send(DictionaryDownloadOutcome {
                lang_code: canonical_owned,
                result,
            });
            ctx.request_repaint();
        });
    }
}

fn download_pack_blocking(
    lang_code: &str,
    pack: &Tier1LanguagePack,
    dest_dir: &Path,
    downloads: &Arc<Mutex<HashMap<String, DictionaryDownloadProgress>>>,
    ctx: &egui::Context,
) -> Result<(), String> {
    ensure_tls_crypto_provider();

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("Failed initializing download runtime: {e}"))?;

    let client = reqwest::Client::builder()
        .user_agent("azo-type/0.3.0")
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(180))
        .build()
        .map_err(|e| format!("Failed building HTTPS client: {e}"))?;

    let staging_dir = std::env::temp_dir().join(format!("azo-dict-dl-{}", Uuid::new_v4()));
    fs::create_dir_all(&staging_dir)
        .map_err(|e| format!("Failed creating staging directory: {e}"))?;

    let outcome = (|| -> Result<(), String> {
        let mut completed_bytes: u64 = 0;

        for (idx, file) in pack.files.iter().enumerate() {
            update_dl_progress(downloads, lang_code, |p| {
                p.current_file_index = idx + 1;
                p.current_filename = file.filename.clone();
            });
            ctx.request_repaint();

            let raw_bytes = rt.block_on(download_single_asset(
                &client,
                file,
                lang_code,
                completed_bytes,
                downloads,
                ctx,
            ))?;
            completed_bytes += raw_bytes.len() as u64;

            if !verify_sha256_hex(&raw_bytes, &file.sha256) {
                return Err(format!(
                    "SHA-256 verification failed for '{}' ({}).",
                    file.filename, lang_code
                ));
            }

            let final_bytes = match file.compression {
                AssetCompression::None => raw_bytes,
                AssetCompression::Gzip => {
                    let mut decoder = GzDecoder::new(raw_bytes.as_slice());
                    let mut out = Vec::new();
                    decoder.read_to_end(&mut out).map_err(|e| {
                        format!("Failed decompressing gzip asset '{}': {e}", file.filename)
                    })?;
                    out
                }
            };

            let staged_path = staging_dir.join(&file.filename);
            fs::write(&staged_path, &final_bytes)
                .map_err(|e| format!("Failed writing staged asset '{}': {e}", file.filename))?;
        }

        fs::create_dir_all(dest_dir).map_err(|e| {
            format!(
                "Failed creating dictionary cache directory '{}': {e}",
                dest_dir.display()
            )
        })?;

        for file in &pack.files {
            let src = staging_dir.join(&file.filename);
            let dst = dest_dir.join(&file.filename);
            fs::copy(&src, &dst)
                .map_err(|e| format!("Failed installing '{}' into cache: {e}", dst.display()))?;
        }
        Ok(())
    })();

    let _ = fs::remove_dir_all(&staging_dir);
    outcome
}

async fn download_single_asset(
    client: &reqwest::Client,
    file: &DictionaryFileAsset,
    lang_code: &str,
    completed_bytes_before: u64,
    downloads: &Arc<Mutex<HashMap<String, DictionaryDownloadProgress>>>,
    ctx: &egui::Context,
) -> Result<Vec<u8>, String> {
    let mut response = client
        .get(&file.url)
        .send()
        .await
        .map_err(|e| format!("Failed downloading '{}': {e}", file.filename))?;

    if !response.status().is_success() {
        return Err(format!(
            "Download failed for '{}' with HTTP {}.",
            file.filename,
            response.status()
        ));
    }

    let mut buf = Vec::with_capacity(file.size_bytes.min(32 * 1024 * 1024) as usize);
    let mut last_repaint = Instant::now();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| format!("Failed reading stream for '{}': {e}", file.filename))?
    {
        buf.extend_from_slice(&chunk);
        let total_dl = completed_bytes_before + buf.len() as u64;
        update_dl_progress(downloads, lang_code, |p| {
            p.downloaded_bytes = total_dl;
        });
        if last_repaint.elapsed() >= Duration::from_millis(80) {
            ctx.request_repaint();
            last_repaint = Instant::now();
        }
    }
    Ok(buf)
}

fn update_dl_progress(
    downloads: &Arc<Mutex<HashMap<String, DictionaryDownloadProgress>>>,
    lang_code: &str,
    f: impl FnOnce(&mut DictionaryDownloadProgress),
) {
    if let Ok(mut map) = downloads.lock() {
        if let Some(entry) = map.get_mut(lang_code) {
            f(entry);
        }
    }
}

/// Resolves the cross-platform user data directory used to store downloaded dictionary packs.
pub fn default_dictionaries_cache_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            return PathBuf::from(local).join("azo-type").join("dictionaries");
        }
        if let Ok(roaming) = std::env::var("APPDATA") {
            return PathBuf::from(roaming).join("azo-type").join("dictionaries");
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("azo-type")
                .join("dictionaries");
        }
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
            if !xdg.is_empty() {
                return PathBuf::from(xdg).join("azo-type").join("dictionaries");
            }
        }
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home)
                .join(".local")
                .join("share")
                .join("azo-type")
                .join("dictionaries");
        }
    }
    std::env::temp_dir().join("azo-type").join("dictionaries")
}
