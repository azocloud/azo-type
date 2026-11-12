//! Atomic disk persistence for CRDT state snapshots (`.typst-collab/state.yrs`) and workspace reconciliation.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::collab::doc::{normalize_rel_path, CollabFileEntry, CollabProjectDoc};
use crate::project::FileKind;

const COLLAB_META_DIR: &str = ".typst-collab";
const SNAPSHOT_FILE_NAME: &str = "crdt-snapshot.yrs";
const FILE_IDS_JSON_NAME: &str = "file-ids.json";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PersistedFileIdMap {
    /// Maps normalized relative path -> UUID string
    pub path_to_id: BTreeMap<String, String>,
}

/// Manages local CRDT binary snapshots and bidirectional synchronization with the real files on disk.
pub struct CollabPersistence {
    root_dir: PathBuf,
    meta_dir: PathBuf,
}

impl CollabPersistence {
    pub fn new(root_dir: &Path) -> Self {
        let meta_dir = root_dir.join(COLLAB_META_DIR);
        Self {
            root_dir: root_dir.to_path_buf(),
            meta_dir,
        }
    }

    pub fn snapshot_path(&self) -> PathBuf {
        self.meta_dir.join(SNAPSHOT_FILE_NAME)
    }

    pub fn file_ids_path(&self) -> PathBuf {
        self.meta_dir.join(FILE_IDS_JSON_NAME)
    }

    /// Loads any saved CRDT binary snapshot from `.typst-collab/crdt-snapshot.yrs` and then
    /// reconciles all editable text files currently on disk so external/offline edits are merged.
    pub fn load_and_reconcile_from_disk(
        &self,
        doc: &mut CollabProjectDoc,
        populate_disk_files: bool,
    ) -> Result<(), String> {
        // 1. Apply stored binary CRDT snapshot if it exists
        let snap_path = self.snapshot_path();
        if snap_path.is_file() {
            if let Ok(bytes) = fs::read(&snap_path) {
                if !bytes.is_empty() {
                    let _ = doc.apply_remote_update_v1(&bytes);
                }
            }
        }

        if !populate_disk_files {
            return Ok(());
        }

        // 2. Load persisted path -> file_id mapping (or create a deterministic UUID v5-style fallback)
        let mut id_map = self.load_file_id_map();

        // 3. Scan editable text files on disk and reconcile with the CRDT
        let mut disk_files = Vec::new();
        Self::scan_editable_files(&self.root_dir, &self.root_dir, &mut disk_files)?;

        for (rel_path, disk_text) in disk_files {
            if let Some(existing_id) = doc.find_file_id_by_path(Path::new(&rel_path)) {
                id_map
                    .path_to_id
                    .insert(rel_path.clone(), existing_id.clone());
                doc.apply_local_text_edit(&existing_id, &disk_text);
            } else {
                let fid = id_map
                    .path_to_id
                    .entry(rel_path.clone())
                    .or_insert_with(|| Uuid::new_v4().to_string())
                    .clone();
                doc.create_file(Some(fid), &rel_path, &disk_text);
            }
        }

        let _ = self.save_file_id_map(&id_map);
        let _ = self.save_snapshot(doc);
        Ok(())
    }

    /// Persists the binary CRDT state (`encode_state_as_update_v1`) and path->ID index to `.typst-collab/`.
    pub fn save_snapshot(&self, doc: &CollabProjectDoc) -> Result<(), String> {
        fs::create_dir_all(&self.meta_dir).map_err(|e| e.to_string())?;
        let snapshot_bytes = doc.encode_full_snapshot_v1();
        fs::write(self.snapshot_path(), snapshot_bytes).map_err(|e| e.to_string())?;

        let mut id_map = PersistedFileIdMap::default();
        for entry in doc.list_files() {
            id_map.path_to_id.insert(entry.resolved_path, entry.file_id);
        }
        self.save_file_id_map(&id_map)?;
        Ok(())
    }

    /// Writes all active CRDT file entries to the real project folder on disk (creating parent
    /// directories as needed) if their content differs from disk. Returns the list of relative
    /// paths that were created or updated on disk.
    pub fn flush_crdt_files_to_disk(
        &self,
        entries: &[CollabFileEntry],
    ) -> Result<Vec<PathBuf>, String> {
        let mut modified_paths = Vec::new();

        for entry in entries {
            let rel = PathBuf::from(&entry.resolved_path);
            // Prevent path traversal outside root_dir
            if rel
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
            {
                continue;
            }
            let abs = self.root_dir.join(&rel);
            let needs_write = match fs::read_to_string(&abs) {
                Ok(existing) => existing != entry.content,
                Err(_) => true,
            };

            if needs_write {
                if let Some(parent) = abs.parent() {
                    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                fs::write(&abs, entry.content.as_bytes()).map_err(|e| e.to_string())?;
                modified_paths.push(rel);
            }
        }

        Ok(modified_paths)
    }

    fn load_file_id_map(&self) -> PersistedFileIdMap {
        fs::read_to_string(self.file_ids_path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn save_file_id_map(&self, map: &PersistedFileIdMap) -> Result<(), String> {
        fs::create_dir_all(&self.meta_dir).map_err(|e| e.to_string())?;
        let json = serde_json::to_string_pretty(map).map_err(|e| e.to_string())?;
        fs::write(self.file_ids_path(), json).map_err(|e| e.to_string())
    }

    fn scan_editable_files(
        root: &Path,
        current: &Path,
        out: &mut Vec<(String, String)>,
    ) -> Result<(), String> {
        let entries = fs::read_dir(current).map_err(|e| e.to_string())?;
        let mut sorted: Vec<_> = entries.flatten().collect();
        sorted.sort_by_key(|e| e.file_name());

        for entry in sorted {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.')
                || name == "target"
                || name == "shots"
                || name == "node_modules"
            {
                continue;
            }
            if path.is_dir() {
                Self::scan_editable_files(root, &path, out)?;
            } else if path.is_file() {
                let kind = FileKind::from_path(&path);
                if kind.is_editable_text() {
                    if let Ok(rel) = path.strip_prefix(root) {
                        let norm = normalize_rel_path(&rel.to_string_lossy());
                        if let Ok(text) = fs::read_to_string(&path) {
                            out.push((norm, text));
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
