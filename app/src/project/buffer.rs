//! Open buffer and file tree node types for workspace and in-memory documents.

use std::fs;
use std::path::{Path, PathBuf};

use crate::core::FileKind;

/// A node in the on-disk or in-memory project directory tree.
#[derive(Debug, Clone)]
pub enum FsTreeNode {
    Directory {
        name: String,
        rel_path: PathBuf,
        children: Vec<FsTreeNode>,
    },
    File {
        name: String,
        rel_path: PathBuf,
        abs_path: PathBuf,
        kind: FileKind,
        size_bytes: u64,
    },
}

/// An open text buffer backed by either an in-memory document or a real file on disk.
#[derive(Debug, Clone)]
pub struct OpenBuffer {
    pub rel_path: PathBuf,
    pub abs_path: PathBuf,
    pub kind: FileKind,
    pub text: Option<String>,
    pub size_bytes: u64,
    pub dirty: bool,
}

impl OpenBuffer {
    /// Loads a buffer from disk if the file exists.
    pub fn load_from_disk(root_dir: &Path, rel_path: &Path) -> Result<Self, String> {
        let abs_path = root_dir.join(rel_path);
        let meta = fs::metadata(&abs_path).map_err(|e| format!("{}: {}", abs_path.display(), e))?;
        let size_bytes = meta.len();
        let kind = FileKind::from_path(&abs_path);

        let text = if kind.is_editable_text() {
            fs::read_to_string(&abs_path).ok()
        } else {
            None
        };

        Ok(Self {
            rel_path: rel_path.to_path_buf(),
            abs_path,
            kind,
            text,
            size_bytes,
            dirty: false,
        })
    }

    /// Saves the buffer content to disk if a destination path has been specified.
    pub fn save_to_disk(&mut self) -> Result<(), String> {
        if self.abs_path.as_os_str().is_empty() {
            return Err("Cannot save in-memory buffer without a destination disk path".to_string());
        }
        if let Some(ref text) = self.text {
            if let Some(parent) = self.abs_path.parent() {
                if !parent.as_os_str().is_empty() {
                    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
            }
            fs::write(&self.abs_path, text.as_bytes()).map_err(|e| e.to_string())?;
            self.size_bytes = text.len() as u64;
            self.dirty = false;
        }
        Ok(())
    }

    pub fn line_count(&self) -> usize {
        self.text
            .as_ref()
            .map(|t| t.lines().count().max(1))
            .unwrap_or(0)
    }
}
