use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use chrono::{Datelike, FixedOffset, Local, Utc};
use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime};
use typst::syntax::{FileId, Source, VirtualPath};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, World};

use crate::vfs::{normalize_vpath, ProjectWorkspace};

/// Shared, immutable compiler resources initialized once (Standard Library + Bundled Fonts).
pub struct SharedWorldResources {
    pub library: LazyHash<Library>,
    pub book: LazyHash<FontBook>,
    pub fonts: Vec<Font>,
}

impl SharedWorldResources {
    pub fn new() -> Self {
        let library = LazyHash::new(Library::default());
        let mut book = FontBook::new();
        let mut fonts = Vec::new();

        // Load all embedded fonts from typst-assets so compilation works identically
        // on Windows, macOS, Linux, and WebAssembly without system font dependencies.
        for font_data in typst_assets::fonts() {
            let buffer = Bytes::from_static(font_data);
            for font in Font::iter(buffer) {
                book.push(font.info().clone());
                fonts.push(font);
            }
        }

        Self {
            library,
            book: LazyHash::new(book),
            fonts,
        }
    }

    pub fn font_family_count(&self) -> usize {
        self.book.families().count()
    }

    pub fn font_face_count(&self) -> usize {
        self.fonts.len()
    }
}

impl Default for SharedWorldResources {
    fn default() -> Self {
        Self::new()
    }
}

/// Custom `typst::World` implementation backed directly by our `ProjectWorkspace` VFS.
pub struct EditorWorld {
    resources: Arc<SharedWorldResources>,
    workspace: ProjectWorkspace,
    main_id: FileId,
    source_cache: Mutex<HashMap<FileId, Source>>,
    bytes_cache: Mutex<HashMap<FileId, Bytes>>,
}

impl EditorWorld {
    pub fn new(resources: Arc<SharedWorldResources>, workspace: ProjectWorkspace) -> Self {
        let main_vpath = VirtualPath::new(Path::new(&workspace.main_file));
        let main_id = FileId::new(None, main_vpath);

        Self {
            resources,
            workspace,
            main_id,
            source_cache: Mutex::new(HashMap::new()),
            bytes_cache: Mutex::new(HashMap::new()),
        }
    }

    fn resolve_vpath(id: FileId) -> String {
        let raw = id.vpath().as_rootless_path().to_string_lossy();
        normalize_vpath(&raw)
    }
}

impl World for EditorWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.resources.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &self.resources.book
    }

    fn main(&self) -> FileId {
        self.main_id
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if let Ok(cache) = self.source_cache.lock() {
            if let Some(src) = cache.get(&id) {
                return Ok(src.clone());
            }
        }

        let key = Self::resolve_vpath(id);
        let vfile = self
            .workspace
            .get_file(&key)
            .ok_or_else(|| FileError::NotFound(Path::new(&key).to_path_buf()))?;

        let text = if let Some(ref t) = vfile.text {
            t.clone()
        } else {
            String::from_utf8(vfile.bytes.clone()).map_err(|_| FileError::InvalidUtf8)?
        };

        let source = Source::new(id, text);
        if let Ok(mut cache) = self.source_cache.lock() {
            cache.insert(id, source.clone());
        }
        Ok(source)
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        if let Ok(cache) = self.bytes_cache.lock() {
            if let Some(b) = cache.get(&id) {
                return Ok(b.clone());
            }
        }

        let key = Self::resolve_vpath(id);
        let vfile = self
            .workspace
            .get_file(&key)
            .ok_or_else(|| FileError::NotFound(Path::new(&key).to_path_buf()))?;

        let bytes = if let Some(ref t) = vfile.text {
            Bytes::from(t.as_bytes().to_vec())
        } else {
            Bytes::from(vfile.bytes.clone())
        };

        if let Ok(mut cache) = self.bytes_cache.lock() {
            cache.insert(id, bytes.clone());
        }
        Ok(bytes)
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.resources.fonts.get(index).cloned()
    }

    fn today(&self, offset: Option<i64>) -> Option<Datetime> {
        let now = Utc::now();
        let with_offset = match offset {
            None => Local::now().fixed_offset(),
            Some(hours) => {
                let secs = i32::try_from(hours).ok()?.checked_mul(3600)?;
                now.with_timezone(&FixedOffset::east_opt(secs)?)
            }
        };
        Datetime::from_ymd(
            with_offset.year(),
            with_offset.month().try_into().ok()?,
            with_offset.day().try_into().ok()?,
        )
    }
}
