//! Workspace management (`DiskProject`), on-disk and in-memory projects, file tree scanning, and buffer caching.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::core::outline::SourceResolver;
pub use crate::core::FileKind;

pub mod buffer;
pub use buffer::{FsTreeNode, OpenBuffer};

/// Manages a real Typst project folder on the local OS filesystem (Linux, macOS, Windows).
#[derive(Debug, Clone)]
pub struct DiskProject {
    pub root_dir: PathBuf,
    pub project_name: String,
    pub tree: Vec<FsTreeNode>,
    pub main_file: PathBuf,
    pub active_file: PathBuf,
    pub open_tabs: Vec<PathBuf>,
    pub buffers: BTreeMap<PathBuf, OpenBuffer>,
    pub revision: u64,
    pub show_hidden_files: bool,
}

impl DiskProject {
    /// Creates a blank in-memory Typst project with an untitled `main.typ` document buffer.
    /// It has no on-disk backing directory and will not write to the filesystem unless
    /// the user explicitly saves it to a specified path.
    pub fn blank() -> Self {
        let main_file = PathBuf::from("main.typ");
        let mut buffers = BTreeMap::new();
        buffers.insert(
            main_file.clone(),
            OpenBuffer {
                rel_path: main_file.clone(),
                abs_path: PathBuf::new(),
                kind: FileKind::Typst,
                text: Some(String::new()),
                size_bytes: 0,
                dirty: false,
            },
        );

        let tree = vec![FsTreeNode::File {
            name: "main.typ".to_string(),
            rel_path: main_file.clone(),
            abs_path: PathBuf::new(),
            kind: FileKind::Typst,
            size_bytes: 0,
        }];

        Self {
            root_dir: PathBuf::new(),
            project_name: "Untitled".to_string(),
            tree,
            main_file: main_file.clone(),
            active_file: main_file.clone(),
            open_tabs: vec![main_file],
            buffers,
            revision: 1,
            show_hidden_files: false,
        }
    }

    /// Returns true if this project is backed by an existing directory on disk.
    pub fn has_disk_backing(&self) -> bool {
        !self.root_dir.as_os_str().is_empty() && self.root_dir.is_dir()
    }

    pub fn open_folder(dir: &Path, show_hidden_files: bool) -> Result<Self, String> {
        let canonical = fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
        if !canonical.is_dir() {
            return Err(format!("'{}' is not a directory", canonical.display()));
        }

        let project_name = canonical
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("typst-project")
            .to_string();

        let mut proj = Self {
            root_dir: canonical,
            project_name,
            tree: Vec::new(),
            main_file: PathBuf::from("main.typ"),
            active_file: PathBuf::from("main.typ"),
            open_tabs: Vec::new(),
            buffers: BTreeMap::new(),
            revision: 1,
            show_hidden_files,
        };

        proj.rescan_tree()?;

        // Pick main.typ if it exists on disk, or the first .typ file found
        let all_typ = proj.collect_typ_files();
        if all_typ.contains(&PathBuf::from("main.typ")) {
            proj.main_file = PathBuf::from("main.typ");
        } else if let Some(first_typ) = all_typ.first() {
            proj.main_file = first_typ.clone();
        }

        // Open initial tabs from disk
        let initial_candidates = [
            proj.main_file.clone(),
            PathBuf::from("template.typ"),
            PathBuf::from("chapters/intro.typ"),
            PathBuf::from("chapters/methods.typ"),
        ];
        for candidate in initial_candidates {
            if proj.root_dir.join(&candidate).is_file() {
                let _ = proj.open_file(&candidate);
            }
        }
        if !proj.open_tabs.is_empty() {
            proj.active_file = proj.open_tabs[0].clone();
        }

        Ok(proj)
    }

    pub fn rescan_tree(&mut self) -> Result<(), String> {
        if !self.has_disk_backing() {
            return Ok(());
        }
        self.tree = Self::scan_directory(&self.root_dir, &self.root_dir, self.show_hidden_files)?;
        Ok(())
    }

    fn scan_directory(
        root: &Path,
        current: &Path,
        show_hidden: bool,
    ) -> Result<Vec<FsTreeNode>, String> {
        let mut dirs = Vec::new();
        let mut files = Vec::new();

        let read_dir = fs::read_dir(current).map_err(|e| e.to_string())?;
        let mut entries: Vec<_> = read_dir.flatten().collect();
        entries.sort_by_key(|e| e.file_name());

        for entry in entries {
            let abs_path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if !show_hidden && name.starts_with('.') {
                continue;
            }
            if name == "target" || name == "node_modules" || name == "shots" {
                continue;
            }

            let rel_path = abs_path
                .strip_prefix(root)
                .unwrap_or(&abs_path)
                .to_path_buf();

            if abs_path.is_dir() {
                let children = Self::scan_directory(root, &abs_path, show_hidden)?;
                dirs.push(FsTreeNode::Directory {
                    name,
                    rel_path,
                    children,
                });
            } else if abs_path.is_file() {
                let size_bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
                let kind = FileKind::from_path(&abs_path);
                files.push(FsTreeNode::File {
                    name,
                    rel_path,
                    abs_path,
                    kind,
                    size_bytes,
                });
            }
        }

        // Put files at root/folder level and directories cleanly organized
        let mut combined = Vec::with_capacity(dirs.len() + files.len());
        combined.extend(files);
        combined.extend(dirs);
        Ok(combined)
    }

    pub fn collect_typ_files(&self) -> Vec<PathBuf> {
        let mut out = Vec::new();
        Self::collect_typ_recursive(&self.tree, &mut out);
        out
    }

    fn collect_typ_recursive(nodes: &[FsTreeNode], out: &mut Vec<PathBuf>) {
        for node in nodes {
            match node {
                FsTreeNode::File { rel_path, kind, .. } => {
                    if *kind == FileKind::Typst {
                        out.push(rel_path.clone());
                    }
                }
                FsTreeNode::Directory { children, .. } => {
                    Self::collect_typ_recursive(children, out);
                }
            }
        }
    }

    pub fn open_file(&mut self, rel_path: &Path) -> Result<(), String> {
        if !self.buffers.contains_key(rel_path) {
            if !self.has_disk_backing() {
                return Err(format!("File '{}' not found in memory", rel_path.display()));
            }
            let buf = OpenBuffer::load_from_disk(&self.root_dir, rel_path)?;
            self.buffers.insert(rel_path.to_path_buf(), buf);
        }
        if !self.open_tabs.iter().any(|p| p == rel_path) {
            self.open_tabs.push(rel_path.to_path_buf());
        }
        if self.active_file != rel_path {
            self.active_file = rel_path.to_path_buf();
            self.revision = self.revision.wrapping_add(1);
        }
        Ok(())
    }

    pub fn close_tab(&mut self, rel_path: &Path) {
        if self.open_tabs.len() <= 1 {
            return;
        }
        self.open_tabs.retain(|p| p != rel_path);
        if self.active_file == rel_path {
            if let Some(first) = self.open_tabs.first() {
                self.active_file = first.clone();
            }
        }
    }

    pub fn active_buffer(&self) -> Option<&OpenBuffer> {
        self.buffers.get(&self.active_file)
    }

    pub fn active_buffer_mut(&mut self) -> Option<&mut OpenBuffer> {
        self.buffers.get_mut(&self.active_file)
    }

    pub fn mark_active_modified(&mut self, auto_save: bool) {
        let has_disk = self.has_disk_backing();
        if let Some(buf) = self.buffers.get_mut(&self.active_file) {
            buf.dirty = true;
            if let Some(ref t) = buf.text {
                buf.size_bytes = t.len() as u64;
            }
            if auto_save && has_disk && !buf.abs_path.as_os_str().is_empty() {
                let _ = buf.save_to_disk();
            }
        }
        self.revision = self.revision.wrapping_add(1);
    }

    pub fn save_active_file(&mut self) -> Result<PathBuf, String> {
        let Some(buf) = self.buffers.get_mut(&self.active_file) else {
            return Err("No active file open".to_string());
        };
        if buf.abs_path.as_os_str().is_empty() {
            return Err(
                "No destination file path specified on disk. Use Save File to pick a path."
                    .to_string(),
            );
        }
        buf.save_to_disk()?;
        let saved = buf.abs_path.clone();
        self.revision = self.revision.wrapping_add(1);
        Ok(saved)
    }

    pub fn save_all_files(&mut self) -> Result<usize, String> {
        if !self.has_disk_backing() {
            return Ok(0);
        }
        let mut count = 0;
        for buf in self.buffers.values_mut() {
            if buf.dirty && !buf.abs_path.as_os_str().is_empty() {
                buf.save_to_disk()?;
                count += 1;
            }
        }
        self.revision = self.revision.wrapping_add(1);
        Ok(count)
    }

    /// Saves the active in-memory buffer to a specific disk path and attaches disk backing
    /// to the project if it was previously untitled/in-memory.
    pub fn save_active_to_path(&mut self, dest_abs_path: &Path) -> Result<PathBuf, String> {
        let dest_abs = dest_abs_path.to_path_buf();
        {
            let Some(buf) = self.buffers.get_mut(&self.active_file) else {
                return Err("No active file open".to_string());
            };
            buf.abs_path = dest_abs.clone();
            buf.save_to_disk()?;
        }
        if !self.has_disk_backing() {
            if let Some(parent) = dest_abs.parent() {
                self.root_dir = parent.to_path_buf();
                self.project_name = parent
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("typst-project")
                    .to_string();
                let _ = self.rescan_tree();
            }
        }
        self.revision = self.revision.wrapping_add(1);
        Ok(dest_abs)
    }

    pub fn create_file_on_disk(&mut self, rel_str: &str) -> Result<PathBuf, String> {
        let trimmed = rel_str.trim().trim_start_matches(['/', '\\']);
        if trimmed.is_empty() {
            return Err("File name cannot be empty".to_string());
        }
        let rel_path = PathBuf::from(trimmed);
        if !self.has_disk_backing() {
            let starter = if trimmed.ends_with(".typ") {
                format!("== {}\n\nCreated in memory.\n", trimmed)
            } else {
                String::new()
            };
            let kind = FileKind::from_path(&rel_path);
            let size_bytes = starter.len() as u64;
            self.buffers.insert(
                rel_path.clone(),
                OpenBuffer {
                    rel_path: rel_path.clone(),
                    abs_path: PathBuf::new(),
                    kind,
                    text: Some(starter),
                    size_bytes,
                    dirty: false,
                },
            );
            self.tree.push(FsTreeNode::File {
                name: trimmed.to_string(),
                rel_path: rel_path.clone(),
                abs_path: PathBuf::new(),
                kind,
                size_bytes,
            });
            self.open_file(&rel_path)?;
            self.revision = self.revision.wrapping_add(1);
            return Ok(rel_path);
        }
        let abs_path = self.root_dir.join(&rel_path);
        if let Some(parent) = abs_path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        if !abs_path.exists() {
            let starter = if trimmed.ends_with(".typ") {
                format!("== {}\n\nCreated on disk in `{}`.\n", trimmed, trimmed)
            } else {
                String::new()
            };
            fs::write(&abs_path, starter.as_bytes()).map_err(|e| e.to_string())?;
        }
        self.rescan_tree()?;
        self.open_file(&rel_path)?;
        self.revision = self.revision.wrapping_add(1);
        Ok(rel_path)
    }

    pub fn rename_file_on_disk(
        &mut self,
        old_rel: &Path,
        new_rel_str: &str,
    ) -> Result<PathBuf, String> {
        let trimmed = new_rel_str.trim().trim_start_matches(['/', '\\']);
        if trimmed.is_empty() {
            return Err("New file name cannot be empty".to_string());
        }
        let new_rel = PathBuf::from(trimmed);
        if self.has_disk_backing() {
            let old_abs = self.root_dir.join(old_rel);
            let new_abs = self.root_dir.join(&new_rel);
            if let Some(parent) = new_abs.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            if old_abs.exists() {
                fs::rename(&old_abs, &new_abs).map_err(|e| e.to_string())?;
            }
        }
        if let Some(mut buf) = self.buffers.remove(old_rel) {
            buf.rel_path = new_rel.clone();
            buf.abs_path = if self.has_disk_backing() {
                self.root_dir.join(&new_rel)
            } else {
                PathBuf::new()
            };
            buf.kind = FileKind::from_path(&new_rel);
            self.buffers.insert(new_rel.clone(), buf);
        }
        for tab in &mut self.open_tabs {
            if tab == old_rel {
                *tab = new_rel.clone();
            }
        }
        if self.active_file == old_rel {
            self.active_file = new_rel.clone();
        }
        if self.main_file == old_rel {
            self.main_file = new_rel.clone();
        }
        if self.has_disk_backing() {
            self.rescan_tree()?;
        } else {
            for node in &mut self.tree {
                if let FsTreeNode::File { name, rel_path, .. } = node {
                    if rel_path == old_rel {
                        *name = trimmed.to_string();
                        *rel_path = new_rel.clone();
                    }
                }
            }
        }
        self.revision = self.revision.wrapping_add(1);
        Ok(new_rel)
    }

    pub fn delete_file_on_disk(&mut self, rel_path: &Path) -> Result<(), String> {
        if self.has_disk_backing() {
            let abs = self.root_dir.join(rel_path);
            if abs.is_file() {
                fs::remove_file(&abs).map_err(|e| e.to_string())?;
            }
        }
        self.buffers.remove(rel_path);
        self.open_tabs.retain(|p| p != rel_path);
        if self.has_disk_backing() {
            self.rescan_tree()?;
        } else {
            self.tree.retain(|n| match n {
                FsTreeNode::File { rel_path: rp, .. } => rp != rel_path,
                _ => true,
            });
        }
        if self.open_tabs.is_empty() {
            let typ_files = self.collect_typ_files();
            if let Some(first) = typ_files.first() {
                let _ = self.open_file(first);
            }
        } else if self.active_file == rel_path {
            if let Some(first) = self.open_tabs.first() {
                self.active_file = first.clone();
            }
        }
        self.revision = self.revision.wrapping_add(1);
        Ok(())
    }

    /// Synchronizes open in-memory buffers and disk file tree with the latest CRDT file entries.
    pub fn sync_from_crdt_files(&mut self, entries: &[crate::collab::doc::CollabFileEntry]) {
        for entry in entries {
            let rel_path = PathBuf::from(&entry.resolved_path);
            let abs_path = self.root_dir.join(&rel_path);
            let kind = FileKind::from_path(&rel_path);
            if let Some(buf) = self.buffers.get_mut(&rel_path) {
                if buf.text.as_deref() != Some(entry.content.as_str()) {
                    buf.text = Some(entry.content.clone());
                    buf.size_bytes = entry.content.len() as u64;
                    buf.dirty = false;
                }
            } else if rel_path == self.active_file {
                self.buffers.insert(
                    rel_path.clone(),
                    OpenBuffer {
                        rel_path,
                        abs_path,
                        kind,
                        text: Some(entry.content.clone()),
                        size_bytes: entry.content.len() as u64,
                        dirty: false,
                    },
                );
            }
        }
        let _ = self.rescan_tree();
        self.revision = self.revision.wrapping_add(1);
    }

    pub fn set_main_file(&mut self, rel_path: PathBuf) {
        self.main_file = rel_path;
        self.revision = self.revision.wrapping_add(1);
    }

    /// Returns the effective `.typ` entrypoint to compile.
    /// If the active file is a `.typ` file that is not referenced (`#import`/`#include`) by `main_file`
    /// (or if `main_file` does not exist), compiles `active_file` directly so standalone files
    /// like `test.typ` preview out-of-the-box. Otherwise compiles `main_file`.
    pub fn effective_compile_entrypoint(&self) -> PathBuf {
        if FileKind::from_path(&self.active_file) == FileKind::Typst
            && self.active_file != self.main_file
        {
            if let Ok(main_text) = self.read_source_text(&self.main_file) {
                let active_norm = self.active_file.to_string_lossy().replace('\\', "/");
                let active_name = self
                    .active_file
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");
                let referenced_in_main = main_text.contains(&active_norm)
                    || (!active_name.is_empty() && main_text.contains(active_name));
                if !referenced_in_main {
                    return self.active_file.clone();
                }
            } else {
                return self.active_file.clone();
            }
        }
        self.main_file.clone()
    }

    /// Reads the latest text for a relative path (using any unsaved in-editor buffer first,
    /// otherwise reading directly from the OS filesystem if backed by disk).
    pub fn read_source_text(&self, rel_path: &Path) -> Result<String, std::io::Error> {
        if let Some(buf) = self.buffers.get(rel_path) {
            if let Some(ref text) = buf.text {
                return Ok(text.clone());
            }
        }
        if !self.has_disk_backing() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("File not found in memory: {}", rel_path.display()),
            ));
        }
        let abs = self.root_dir.join(rel_path);
        fs::read_to_string(abs)
    }

    /// Reads the latest bytes for a relative path directly from the OS filesystem
    /// (or unsaved editor buffer if currently edited).
    pub fn read_file_bytes(&self, rel_path: &Path) -> Result<Vec<u8>, std::io::Error> {
        if let Some(buf) = self.buffers.get(rel_path) {
            if let Some(ref text) = buf.text {
                return Ok(text.as_bytes().to_vec());
            }
        }
        if !self.has_disk_backing() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("File not found in memory: {}", rel_path.display()),
            ));
        }
        let abs = self.root_dir.join(rel_path);
        fs::read(abs)
    }
}

impl Default for DiskProject {
    fn default() -> Self {
        Self::blank()
    }
}

impl SourceResolver for DiskProject {
    fn read_source_text(&self, path: &Path) -> Result<String, String> {
        self.read_source_text(path).map_err(|e| e.to_string())
    }

    fn is_main_entrypoint(&self, path: &Path) -> bool {
        self.effective_compile_entrypoint() == path
    }
}
