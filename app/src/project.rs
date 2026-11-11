use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    Typst,
    Bib,
    Svg,
    Image,
    Text,
    Binary,
}

impl FileKind {
    pub fn from_path(path: &Path) -> Self {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        match ext.as_str() {
            "typ" => Self::Typst,
            "bib" | "yml" | "yaml" => Self::Bib,
            "svg" => Self::Svg,
            "png" | "jpg" | "jpeg" | "gif" | "webp" => Self::Image,
            "txt" | "md" | "csv" | "json" | "toml" => Self::Text,
            _ => Self::Binary,
        }
    }

    pub fn is_editable_text(self) -> bool {
        matches!(self, Self::Typst | Self::Bib | Self::Svg | Self::Text)
    }

    pub fn icon(self) -> &'static str {
        match self {
            Self::Typst => "[T]",
            Self::Bib => "[B]",
            Self::Svg => "[S]",
            Self::Image => "[I]",
            Self::Text => "[F]",
            Self::Binary => "[#]",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Typst => "Typst Source (.typ)",
            Self::Bib => "Bibliography (.bib)",
            Self::Svg => "Vector Graphic (.svg)",
            Self::Image => "Raster Image",
            Self::Text => "Text Document",
            Self::Binary => "Binary Asset",
        }
    }
}

/// A node in the on-disk project directory tree.
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

/// An open text buffer backed by a real file on disk.
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

    pub fn save_to_disk(&mut self) -> Result<(), String> {
        if let Some(ref text) = self.text {
            if let Some(parent) = self.abs_path.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
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
        if let Some(buf) = self.buffers.get_mut(&self.active_file) {
            buf.dirty = true;
            if let Some(ref t) = buf.text {
                buf.size_bytes = t.len() as u64;
            }
            if auto_save {
                let _ = buf.save_to_disk();
            }
        }
        self.revision = self.revision.wrapping_add(1);
    }

    pub fn save_active_file(&mut self) -> Result<PathBuf, String> {
        let Some(buf) = self.buffers.get_mut(&self.active_file) else {
            return Err("No active file open".to_string());
        };
        buf.save_to_disk()?;
        let saved = buf.abs_path.clone();
        self.revision = self.revision.wrapping_add(1);
        Ok(saved)
    }

    pub fn save_all_files(&mut self) -> Result<usize, String> {
        let mut count = 0;
        for buf in self.buffers.values_mut() {
            if buf.dirty {
                buf.save_to_disk()?;
                count += 1;
            }
        }
        self.revision = self.revision.wrapping_add(1);
        Ok(count)
    }

    pub fn create_file_on_disk(&mut self, rel_str: &str) -> Result<PathBuf, String> {
        let trimmed = rel_str.trim().trim_start_matches(['/', '\\']);
        if trimmed.is_empty() {
            return Err("File name cannot be empty".to_string());
        }
        let rel_path = PathBuf::from(trimmed);
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

    pub fn rename_file_on_disk(&mut self, old_rel: &Path, new_rel_str: &str) -> Result<PathBuf, String> {
        let trimmed = new_rel_str.trim().trim_start_matches(['/', '\\']);
        if trimmed.is_empty() {
            return Err("New file name cannot be empty".to_string());
        }
        let new_rel = PathBuf::from(trimmed);
        let old_abs = self.root_dir.join(old_rel);
        let new_abs = self.root_dir.join(&new_rel);
        if let Some(parent) = new_abs.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        if old_abs.exists() {
            fs::rename(&old_abs, &new_abs).map_err(|e| e.to_string())?;
        }
        if let Some(mut buf) = self.buffers.remove(old_rel) {
            buf.rel_path = new_rel.clone();
            buf.abs_path = new_abs;
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
        self.rescan_tree()?;
        self.revision = self.revision.wrapping_add(1);
        Ok(new_rel)
    }

    pub fn delete_file_on_disk(&mut self, rel_path: &Path) -> Result<(), String> {
        let abs = self.root_dir.join(rel_path);
        if abs.is_file() {
            fs::remove_file(&abs).map_err(|e| e.to_string())?;
        }
        self.buffers.remove(rel_path);
        self.open_tabs.retain(|p| p != rel_path);
        self.rescan_tree()?;
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
    /// otherwise reading directly from the OS filesystem).
    pub fn read_source_text(&self, rel_path: &Path) -> Result<String, std::io::Error> {
        if let Some(buf) = self.buffers.get(rel_path) {
            if let Some(ref text) = buf.text {
                return Ok(text.clone());
            }
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
        let abs = self.root_dir.join(rel_path);
        fs::read(abs)
    }
}

/// Ensures a real sample Typst project folder exists on disk so the user can immediately
/// inspect, edit, and compile real files on the filesystem out-of-the-box.
pub fn ensure_sample_project_on_disk() -> PathBuf {
    let base_dir = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("sample-project");

    let _ = fs::create_dir_all(base_dir.join("chapters"));
    let _ = fs::create_dir_all(base_dir.join("assets"));

    let main_path = base_dir.join("main.typ");
    let needs_sample_refresh = main_path
        .exists()
        .then(|| fs::read_to_string(&main_path).ok())
        .flatten()
        .map(|s| s.contains("= Executive Summary") && !s.contains("=== Package & Universe Resolution"))
        .unwrap_or(true);
    if !main_path.exists() || needs_sample_refresh {
        let main_typ = r##"#import "template.typ": project, callout, metric-grid
#import "chapters/methods.typ": architecture-table

#show: project.with(
  title: "Native Desktop Typst Studio",
  subtitle: "Direct OS Filesystem Workspace & Real-Time Typesetting in Rust",
  authors: (
    (name: "Dr. Elena Vance", affiliation: "Desktop Systems Group"),
    (name: "Marcus Lindqvist", affiliation: "Compiler Architecture Lab"),
  ),
  date: "November 2026",
  version: "v2.0.0",
)

= Executive Summary

*Typst Studio* is a native desktop development environment for *Linux*, *macOS*, and *Windows* built in Rust with `egui`, `eframe`, and the `typst` compiler stack. It operates directly on real project folders and files on your local filesystem, providing an interactive side panel file tree, native OS menu integration, a customizable *Settings* window (`File > Settings`), and instant PDF export.

#metric-grid(
  ("Storage Engine", "Direct Disk I/O", "Native OS Filesystem"),
  ("Font Bundle", "17 Faces", "Libertinus + NewCM Math"),
  ("Desktop OS", "Linux / macOS / Win", "Native Menu & Dialogs"),
)

#include "chapters/intro.typ"

== Filesystem & Compilation Pipeline

When you open a project folder from disk, `EditorWorld` resolves `#import`, `#include`, and image paths directly from your project directory on disk while merging any unsaved editor buffers in real time.

=== Package & Universe Resolution

External `@preview` packages and local project modules are resolved automatically and cached on disk.

#figure(
  image("assets/diagram.svg", width: 92%),
  caption: [Direct filesystem project tree, `EditorWorld` compiler pipeline, and live GPU page rasterization.],
) <fig-pipeline>

== Mathematical Formulation

Typst's mathematical engine typesets equations using embedded *New Computer Modern Math* fonts:

$ cal(L)_("layout")(x, lambda) = sum_(k=1)^N integral_0^T (1/2 norm(nabla phi_k (t))^2 + lambda_k dot psi_k (x, t)) dif t $

=== Multi-Pass Layout Convergence

For multi-page convergence, the layout introspector validates fixed-point stability across $m <= 5$ passes:

$ bold(H) = mat(
  partial f_1 / partial x_1, partial f_1 / partial x_2, dots.c, partial f_1 / partial x_n;
  partial f_2 / partial x_1, partial f_2 / partial x_2, dots.c, partial f_2 / partial x_n;
  dots.v, dots.v, dots.down, dots.v;
  partial f_n / partial x_1, partial f_n / partial x_2, dots.c, partial f_n / partial x_n;
) quad "==>" quad rho(bold(H)^(-1)) < 1 $

= Architecture & PDF Standards

#architecture-table()

== Archival Conformance & Settings

#callout(
  title: "Desktop Workflow & Settings",
  tone: "info",
)[
  Use the top menu bar (`File > Open Project Folder...` or `File > Settings...`) to open any Typst folder on disk or customize editor font size, auto-save, render DPI, and theme preferences.
]
"##;
        let _ = fs::write(&main_path, main_typ);
    }

    let template_path = base_dir.join("template.typ");
    if !template_path.exists() {
        let template_typ = r##"// Reusable document template & components for Typst Studio

#let project(
  title: "",
  subtitle: none,
  authors: (),
  date: none,
  version: none,
  body,
) = {
  set document(title: title, author: authors.map(a => a.name))
  set page(
    paper: "a4",
    margin: (x: 1.8cm, top: 2.0cm, bottom: 2.0cm),
    header: context {
      if counter(page).get().first() > 1 [
        #set text(size: 8.5pt, fill: rgb("#64748b"))
        #grid(
          columns: (1fr, 1fr),
          align(left)[#emph(title)],
          align(right)[Typst Studio #if version != none [(#version)]],
        )
        #v(-4pt)
        #line(length: 100%, stroke: 0.4pt + rgb("#cbd5e1"))
      ]
    },
    footer: context [
      #set text(size: 8.5pt, fill: rgb("#64748b"))
      #line(length: 100%, stroke: 0.4pt + rgb("#e2e8f0"))
      #v(2pt)
      #grid(
        columns: (1fr, 1fr),
        align(left)[Compiled by Typst 0.12 Engine],
        align(right)[Page #counter(page).display("1 of 1", both: true)],
      )
    ],
  )

  set text(font: "Libertinus Serif", size: 10.5pt, lang: "en")
  set par(justify: true, leading: 0.62em)
  set heading(numbering: "1.1")

  show heading.where(level: 1): it => block(above: 1.3em, below: 0.7em)[
    #set text(size: 13.5pt, weight: "bold", fill: rgb("#0f172a"))
    #it
    #v(-4pt)
    #line(length: 100%, stroke: 1pt + rgb("#3b82f6"))
  ]

  show heading.where(level: 2): it => block(above: 1.1em, below: 0.55em)[
    #set text(size: 11.5pt, weight: "bold", fill: rgb("#1e3a8a"))
    #it
  ]

  show raw.where(block: false): box.with(
    fill: rgb("#f1f5f9"),
    inset: (x: 3pt, y: 0pt),
    outset: (y: 3pt),
    radius: 2pt,
  )

  block(
    width: 100%,
    fill: rgb("#0f172a"),
    inset: (x: 16pt, y: 14pt),
    radius: 6pt,
  )[
    #set text(fill: white)
    #grid(
      columns: (1fr, auto),
      align(left)[
        #text(size: 8.5pt, weight: "bold", fill: rgb("#38bdf8"))[NATIVE DESKTOP TYPST WORKSPACE]
        #v(3pt)
        #text(size: 18pt, weight: "bold")[#title]
        #if subtitle != none [
          #v(3pt)
          #text(size: 10.5pt, fill: rgb("#cbd5e1"))[#subtitle]
        ]
      ],
      if version != none {
        align(right + top)[
          #box(
            fill: rgb("#1e293b"),
            stroke: 0.8pt + rgb("#38bdf8"),
            inset: (x: 7pt, y: 4pt),
            radius: 4pt,
          )[#text(size: 8.5pt, weight: "bold", fill: rgb("#38bdf8"))[#version]]
        ]
      }
    )
    #v(8pt)
    #grid(
      columns: (1fr, auto),
      align(left)[
        #set text(size: 9pt, fill: rgb("#e2e8f0"))
        #authors.map(a => [*#a.name* (#a.affiliation)]).join("  •  ")
      ],
      if date != none [
        #align(right)[#text(size: 9pt, fill: rgb("#94a3b8"))[#date]]
      ]
    )
  ]

  v(8pt)
  body
}

#let callout(title: "Note", tone: "info", body) = {
  let (bg, border, fg) = if tone == "warn" {
    (rgb("#fffbeb"), rgb("#f59e0b"), rgb("#92400e"))
  } else {
    (rgb("#eff6ff"), rgb("#3b82f6"), rgb("#1e40af"))
  }
  block(
    width: 100%,
    fill: bg,
    stroke: (left: 3.5pt + border, rest: 0.5pt + border.lighten(50%)),
    inset: (x: 12pt, y: 9pt),
    radius: (right: 4pt),
  )[
    #text(weight: "bold", size: 9.5pt, fill: fg)[#title]
    #v(3pt)
    #set text(size: 9.5pt, fill: rgb("#1e293b"))
    #body
  ]
}

#let metric-grid(..cards) = {
  let items = cards.pos()
  grid(
    columns: (1fr,) * items.len(),
    gutter: 10pt,
    ..items.map(card => {
      let (label, value, sub) = card
      block(
        width: 100%,
        fill: rgb("#f8fafc"),
        stroke: 0.7pt + rgb("#cbd5e1"),
        inset: (x: 10pt, y: 8pt),
        radius: 5pt,
      )[
        #text(size: 8pt, weight: "bold", fill: rgb("#64748b"))[#upper(label)]
        #v(2pt)
        #text(size: 13pt, weight: "bold", fill: rgb("#0f172a"))[#value]
        #v(2pt)
        #text(size: 8pt, fill: rgb("#475569"))[#sub]
      ]
    })
  )
}
"##;
        let _ = fs::write(&template_path, template_typ);
    }

    let intro_path = base_dir.join("chapters/intro.typ");
    if !intro_path.exists() {
        let intro_typ = r##"== Core Native Desktop Architecture

By targeting native desktop platforms (*Linux*, *macOS*, and *Windows*) directly, our IDE works seamlessly with standard directories and files on disk:

+ *Direct Filesystem Project Tree (`DiskProject`):* Reads and writes `.typ`, `.bib`, and image assets directly in your chosen project folder on disk.
+ *Filesystem-Backed `EditorWorld`:* Resolves multi-file `#import` and `#include` statements directly from the project root directory on disk.
+ *Platform-Aware Menu Bar & Settings Window:* Uses the native OS screen-top menu bar on macOS and an integrated desktop menu bar on Linux/Windows, including `File > Settings` for live configuration.
"##;
        let _ = fs::write(&intro_path, intro_typ);
    }

    let methods_path = base_dir.join("chapters/methods.typ");
    if !methods_path.exists() {
        let methods_typ = r##"// Module exported to main.typ via #import "chapters/methods.typ": architecture-table

#let architecture-table() = {
  figure(
    table(
      columns: (auto, 1.2fr, 2fr),
      inset: (x: 8pt, y: 6pt),
      align: (left, left, left),
      fill: (x, y) => if y == 0 { rgb("#1e293b") } else if calc.odd(y) { rgb("#f8fafc") } else { white },
      stroke: 0.5pt + rgb("#cbd5e1"),
      table.header(
        [#text(fill: white, weight: "bold")[Subsystem]],
        [#text(fill: white, weight: "bold")[Module]],
        [#text(fill: white, weight: "bold")[Desktop Responsibility]],
      ),
      [*Desktop Menu & UI*], [`src/app.rs`], [Native OS menu bar on macOS, top menu bar on Linux/Win, Settings window],
      [*Disk Project I/O*], [`src/project.rs`], [Direct `std::fs` folder scanning, file tree, and disk read/write],
      [*Compiler World*], [`src/world.rs`], [Implements `typst::World` over the local project directory on disk],
      [*Page Rasterizer*], [`typst-render`], [Renders `typst::layout::Page` frames to RGBA GPU textures],
      [*PDF Exporter*], [`typst-pdf` + `rfd`], [Exports PDF directly to disk via native file dialogs],
    ),
    caption: [Native desktop architecture across Linux, macOS, and Windows.],
  )
}
"##;
        let _ = fs::write(&methods_path, methods_typ);
    }

    let svg_path = base_dir.join("assets/diagram.svg");
    if !svg_path.exists() {
        let diagram_svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 680 120" width="680" height="120">
  <rect x="2" y="2" width="676" height="116" rx="8" fill="#f8fafc" stroke="#cbd5e1" stroke-width="1.5"/>
  <!-- Box 1: Disk Folder -->
  <rect x="20" y="22" width="135" height="76" rx="6" fill="#eff6ff" stroke="#3b82f6" stroke-width="1.8"/>
  <text x="87" y="48" font-family="DejaVu Sans Mono, monospace" font-size="11" font-weight="bold" fill="#1e3a8a" text-anchor="middle">DiskProject</text>
  <text x="87" y="66" font-family="Libertinus Serif, serif" font-size="10" fill="#475569" text-anchor="middle">Local OS Folder</text>
  <text x="87" y="82" font-family="Libertinus Serif, serif" font-size="9.5" fill="#64748b" text-anchor="middle">std::fs Read/Write</text>
  <!-- Arrow 1 -->
  <line x1="155" y1="60" x2="188" y2="60" stroke="#64748b" stroke-width="2"/>
  <polygon points="188,55 198,60 188,65" fill="#64748b"/>
  <!-- Box 2: EditorWorld -->
  <rect x="198" y="22" width="135" height="76" rx="6" fill="#f0fdf4" stroke="#16a34a" stroke-width="1.8"/>
  <text x="265" y="48" font-family="DejaVu Sans Mono, monospace" font-size="11" font-weight="bold" fill="#14532d" text-anchor="middle">EditorWorld</text>
  <text x="265" y="66" font-family="Libertinus Serif, serif" font-size="10" fill="#475569" text-anchor="middle">typst::World Trait</text>
  <text x="265" y="82" font-family="Libertinus Serif, serif" font-size="9.5" fill="#64748b" text-anchor="middle">+ Bundled Fonts</text>
  <!-- Arrow 2 -->
  <line x1="333" y1="60" x2="366" y2="60" stroke="#64748b" stroke-width="2"/>
  <polygon points="366,55 376,60 366,65" fill="#64748b"/>
  <!-- Box 3: Compiler -->
  <rect x="376" y="22" width="130" height="76" rx="6" fill="#fefce8" stroke="#ca8a04" stroke-width="1.8"/>
  <text x="441" y="48" font-family="DejaVu Sans Mono, monospace" font-size="11" font-weight="bold" fill="#713f12" text-anchor="middle">typst::compile</text>
  <text x="441" y="66" font-family="Libertinus Serif, serif" font-size="10" fill="#475569" text-anchor="middle">Layout Document</text>
  <text x="441" y="82" font-family="Libertinus Serif, serif" font-size="9.5" fill="#64748b" text-anchor="middle">+ Diagnostics</text>
  <!-- Arrow 3 -->
  <line x1="506" y1="60" x2="538" y2="60" stroke="#64748b" stroke-width="2"/>
  <polygon points="538,55 548,60 538,65" fill="#64748b"/>
  <!-- Box 4: Outputs -->
  <rect x="548" y="22" width="112" height="76" rx="6" fill="#f3e8ff" stroke="#9333ea" stroke-width="1.8"/>
  <text x="604" y="48" font-family="DejaVu Sans Mono, monospace" font-size="10.5" font-weight="bold" fill="#581c87" text-anchor="middle">Outputs</text>
  <text x="604" y="66" font-family="Libertinus Serif, serif" font-size="10" fill="#475569" text-anchor="middle">GPU Preview</text>
  <text x="604" y="82" font-family="Libertinus Serif, serif" font-size="9.5" fill="#64748b" text-anchor="middle">&amp; Native PDF</text>
</svg>
"##;
        let _ = fs::write(&svg_path, diagram_svg);
    }

    let bib_path = base_dir.join("refs.bib");
    if !bib_path.exists() {
        let refs_bib = r#"@article{typst2023,
  author = {Mädje, Laurenz and Haug, Martin},
  title = {Typst: A Programmable Markup Language for Typesetting},
  journal = {Proceedings of Document Engineering},
  year = {2023},
}
"#;
        let _ = fs::write(&bib_path, refs_bib);
    }

    base_dir
}
