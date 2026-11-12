//! Implementation of `typst::World` (`IdeWorld`) providing fonts, packages, and file sources to the compiler.

use std::any::Any;
use std::collections::HashMap;
use std::io::{self, Cursor, Read};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime, Duration};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};
use typst_kit::datetime::Time;
use typst_kit::downloader::Downloader;
use typst_kit::fonts::{self, FontStore};
use typst_kit::packages::SystemPackages;

use crate::project::DiskProject;

/// HTTP Downloader backed by `reqwest` + `rustls` for automatically fetching missing
/// `@preview/...` packages from the official Typst Universe registry (`packages.typst.org`).
pub struct ReqwestDownloader;

impl Downloader for ReqwestDownloader {
    fn stream(&self, _key: &dyn Any, url: &str) -> io::Result<(Option<usize>, Box<dyn Read>)> {
        let url_owned = url.to_string();
        let join_res = std::thread::spawn(move || -> Result<Vec<u8>, io::Error> {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(io::Error::other)?;
            rt.block_on(async move {
                let client = reqwest::Client::builder()
                    .user_agent("azo-type/0.3.0")
                    .build()
                    .map_err(io::Error::other)?;
                let response = client
                    .get(&url_owned)
                    .send()
                    .await
                    .map_err(io::Error::other)?;
                let status = response.status();
                if status == reqwest::StatusCode::NOT_FOUND {
                    return Err(io::Error::new(
                        io::ErrorKind::NotFound,
                        format!("package archive not found at {}", url_owned),
                    ));
                }
                if !status.is_success() {
                    return Err(io::Error::other(format!(
                        "HTTP {} while downloading {}",
                        status, url_owned
                    )));
                }
                let bytes = response.bytes().await.map_err(io::Error::other)?;
                Ok(bytes.to_vec())
            })
        })
        .join();

        match join_res {
            Ok(Ok(bytes)) => {
                let len = bytes.len();
                Ok((Some(len), Box::new(Cursor::new(bytes))))
            }
            Ok(Err(e)) => Err(e),
            Err(_) => Err(io::Error::other("package download worker panicked")),
        }
    }
}

/// Shared, immutable compiler resources initialized once (Standard Library, Embedded + System Fonts,
/// and Local + Universe Package Resolver).
pub struct SharedWorldResources {
    pub library: LazyHash<Library>,
    pub fonts: FontStore,
    pub packages: SystemPackages,
}

impl SharedWorldResources {
    pub fn new() -> Self {
        let library = LazyHash::new(Library::default());
        let mut fonts = FontStore::new();
        fonts.extend(fonts::embedded());
        fonts.extend(fonts::system());
        let packages = SystemPackages::new(ReqwestDownloader);

        Self {
            library,
            fonts,
            packages,
        }
    }

    pub fn font_family_count(&self) -> usize {
        self.fonts.book().families().count()
    }
}

impl Default for SharedWorldResources {
    fn default() -> Self {
        Self::new()
    }
}

/// Custom `typst::World` implementation backed by the local OS filesystem (`DiskProject`)
/// for project files and `SystemPackages` for local/cached/remote `@preview` Typst packages.
pub struct EditorWorld {
    resources: Arc<SharedWorldResources>,
    project: DiskProject,
    main_id: FileId,
    source_cache: Mutex<HashMap<FileId, Source>>,
    bytes_cache: Mutex<HashMap<FileId, Bytes>>,
}

impl EditorWorld {
    pub fn new(resources: Arc<SharedWorldResources>, project: DiskProject) -> Self {
        let entry_rel = project.effective_compile_entrypoint();
        let entry_str = entry_rel.to_string_lossy().replace('\\', "/");
        let main_vpath = VirtualPath::new(&entry_str)
            .or_else(|_| VirtualPath::new("main.typ"))
            .expect("valid fallback VirtualPath");
        let main_id = RootedPath::new(VirtualRoot::Project, main_vpath).intern();

        Self {
            resources,
            project,
            main_id,
            source_cache: Mutex::new(HashMap::new()),
            bytes_cache: Mutex::new(HashMap::new()),
        }
    }

    pub fn display_path_for_id(&self, id: FileId) -> PathBuf {
        let rel = id.vpath().get_without_slash();
        match id.root() {
            VirtualRoot::Project => PathBuf::from(rel),
            VirtualRoot::Package(spec) => PathBuf::from(format!(
                "@{}/{}:{}/{}",
                spec.namespace, spec.name, spec.version, rel
            )),
        }
    }

    fn read_text_for_id(&self, id: FileId) -> FileResult<String> {
        let vpath = id.vpath();
        match id.root() {
            VirtualRoot::Project => {
                let rel_path = PathBuf::from(vpath.get_without_slash());
                self.project
                    .read_source_text(&rel_path)
                    .map_err(|e| FileError::from_io(e, &rel_path))
            }
            VirtualRoot::Package(spec) => {
                let pkg_root = self.resources.packages.obtain(spec)?;
                let raw_bytes = pkg_root.load(vpath)?;
                let s = std::str::from_utf8(raw_bytes.as_slice())
                    .map_err(|_| FileError::InvalidUtf8)?;
                Ok(s.to_owned())
            }
        }
    }

    fn read_bytes_for_id(&self, id: FileId) -> FileResult<Bytes> {
        let vpath = id.vpath();
        match id.root() {
            VirtualRoot::Project => {
                let rel_path = PathBuf::from(vpath.get_without_slash());
                let raw = self
                    .project
                    .read_file_bytes(&rel_path)
                    .map_err(|e| FileError::from_io(e, &rel_path))?;
                Ok(Bytes::new(raw))
            }
            VirtualRoot::Package(spec) => {
                let pkg_root = self.resources.packages.obtain(spec)?;
                pkg_root.load(vpath)
            }
        }
    }
}

impl World for EditorWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.resources.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        self.resources.fonts.book()
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

        let text = self.read_text_for_id(id)?;
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

        let bytes = self.read_bytes_for_id(id)?;
        if let Ok(mut cache) = self.bytes_cache.lock() {
            cache.insert(id, bytes.clone());
        }
        Ok(bytes)
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.resources.fonts.font(index)
    }

    fn today(&self, offset: Option<Duration>) -> Option<Datetime> {
        Time::system().today(offset)
    }
}
