use std::collections::BTreeMap;
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
    pub fn from_path(path: &str) -> Self {
        let lower = path.to_ascii_lowercase();
        if lower.ends_with(".typ") {
            Self::Typst
        } else if lower.ends_with(".bib") || lower.ends_with(".yml") || lower.ends_with(".yaml") {
            Self::Bib
        } else if lower.ends_with(".svg") {
            Self::Svg
        } else if lower.ends_with(".png")
            || lower.ends_with(".jpg")
            || lower.ends_with(".jpeg")
            || lower.ends_with(".gif")
        {
            Self::Image
        } else if lower.ends_with(".txt")
            || lower.ends_with(".md")
            || lower.ends_with(".csv")
            || lower.ends_with(".json")
            || lower.ends_with(".toml")
        {
            Self::Text
        } else {
            Self::Binary
        }
    }

    pub fn is_editable_text(self) -> bool {
        matches!(
            self,
            Self::Typst | Self::Bib | Self::Svg | Self::Text
        )
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

#[derive(Debug, Clone)]
pub struct VirtualFile {
    pub path: String,
    pub kind: FileKind,
    pub text: Option<String>,
    pub bytes: Vec<u8>,
    pub dirty: bool,
}

impl VirtualFile {
    pub fn new_text(path: impl Into<String>, content: impl Into<String>) -> Self {
        let path = normalize_vpath(&path.into());
        let text_str = content.into();
        let bytes = text_str.as_bytes().to_vec();
        let kind = FileKind::from_path(&path);
        Self {
            path,
            kind,
            text: Some(text_str),
            bytes,
            dirty: false,
        }
    }

    pub fn new_bytes(path: impl Into<String>, bytes: Vec<u8>) -> Self {
        let path = normalize_vpath(&path.into());
        let kind = FileKind::from_path(&path);
        let text = if kind.is_editable_text() {
            String::from_utf8(bytes.clone()).ok()
        } else {
            None
        };
        Self {
            path,
            kind,
            text,
            bytes,
            dirty: false,
        }
    }

    pub fn sync_bytes_from_text(&mut self) {
        if let Some(ref t) = self.text {
            self.bytes.clear();
            self.bytes.extend_from_slice(t.as_bytes());
        }
    }

    pub fn size_bytes(&self) -> usize {
        if let Some(ref t) = self.text {
            t.len()
        } else {
            self.bytes.len()
        }
    }

    pub fn line_count(&self) -> usize {
        self.text
            .as_ref()
            .map(|t| t.lines().count().max(1))
            .unwrap_or(0)
    }
}

pub fn normalize_vpath(path: &str) -> String {
    let replaced = path.replace('\\', "/");
    let mut parts: Vec<&str> = Vec::new();
    for segment in replaced.split('/') {
        let trimmed = segment.trim();
        if trimmed.is_empty() || trimmed == "." {
            continue;
        }
        if trimmed == ".." {
            parts.pop();
        } else {
            parts.push(trimmed);
        }
    }
    parts.join("/")
}

/// Unified Virtual File System (VFS) holding an in-memory file tree for both
/// WASM and native builds, with optional native filesystem synchronization.
#[derive(Debug, Clone)]
pub struct ProjectWorkspace {
    pub project_name: String,
    pub files: BTreeMap<String, VirtualFile>,
    pub main_file: String,
    pub active_file: String,
    pub open_tabs: Vec<String>,
    pub revision: u64,
    #[cfg(not(target_arch = "wasm32"))]
    pub root_dir: Option<PathBuf>,
}

impl Default for ProjectWorkspace {
    fn default() -> Self {
        Self::new_sample_project()
    }
}

impl ProjectWorkspace {
    pub fn new_sample_project() -> Self {
        let mut files = BTreeMap::new();

        let main_typ = r#"#import "template.typ": project, callout, metric-grid
#import "chapters/methods.typ": architecture-table

#show: project.with(
  title: "Unified Cross-Platform Typst Studio",
  subtitle: "Deterministic Typesetting & Real-Time Incremental Preview in Rust",
  authors: (
    (name: "Dr. Elena Vance", affiliation: "Systems Architecture Lab"),
    (name: "Marcus Lindqvist", affiliation: "WebAssembly Compiler Group"),
  ),
  date: "November 2026",
  version: "v1.4.0",
)

= Executive Summary

*Typst Studio* is a native and WebAssembly-powered integrated development environment built in Rust using `egui`, `eframe`, and the `typst` compiler stack. All project files reside in a unified *Virtual File System (VFS)* backed by an in-memory `EditorWorld` that resolves `#import`, `#include`, vector graphics, and embedded fonts identically on Linux, macOS, Windows, and Web browsers.

#metric-grid(
  ("Frame Budget", "< 16.6 ms", "60 FPS non-blocking UI"),
  ("Font Bundle", "17 Faces", "Libertinus + NewCM Math"),
  ("Targets", "Native + WASM", "Single Rust codebase"),
)

#include "chapters/intro.typ"

== System Pipeline & Compilation Model

When source text changes in the left-hand code editor, the workspace increments its revision counter and schedules a debounced compilation pass through `typst::compile(&world)`. Each layouted page frame is rasterized via `typst-render` (`tiny-skia`) and uploaded to GPU textures.

#figure(
  image("assets/diagram.svg", width: 92%),
  caption: [Unified VFS and `EditorWorld` compilation pipeline from source buffer to GPU texture and PDF export.],
) <fig-pipeline>

== Mathematical Formulation

Typst's built-in mathematical engine typesets complex equations using bundled *New Computer Modern Math* fonts without external dependencies:

$ cal(L)_("layout")(x, lambda) = sum_(k=1)^N integral_0^T (1/2 norm(nabla phi_k (t))^2 + lambda_k dot psi_k (x, t)) dif t $

For multi-page document convergence, the layout introspector validates fixed-point stability across $m <= 5$ iterations:

$ bold(H) = mat(
  partial f_1 / partial x_1, partial f_1 / partial x_2, dots.c, partial f_1 / partial x_n;
  partial f_2 / partial x_1, partial f_2 / partial x_2, dots.c, partial f_2 / partial x_n;
  dots.v, dots.v, dots.down, dots.v;
  partial f_n / partial x_1, partial f_n / partial x_2, dots.c, partial f_n / partial x_n;
) quad "==>" quad rho(bold(H)^(-1)) < 1 $

#architecture-table()

#callout(
  title: "Live Multi-File Editing Tip",
  tone: "info",
)[
  Try editing this file (`main.typ`) or switching to `template.typ` and `chapters/intro.typ` in the left File Overview Tree. Changes across any imported module immediately re-render the document!
]
"#;

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

  // Title banner
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
        #text(size: 8.5pt, weight: "bold", fill: rgb("#38bdf8"))[TECHNICAL SPECIFICATION & LIVE WORKSPACE]
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

        let intro_typ = r##"== Core Architectural Pillars

Our cross-platform Typst IDE is built around three foundational abstractions that decouple the user interface from OS-specific storage and font resolution:

+ *Unified Virtual File System (`ProjectWorkspace`):* Holds an in-memory B-tree of text and binary files (`main.typ`, `template.typ`, `chapters/*.typ`, and SVG/PNG assets) with seamless desktop synchronization.
+ *Custom `EditorWorld` (`typst::World`):* Resolves `#import` and `#include` paths directly against the virtual workspace and indexes all embedded font families via `FontBook`.
+ *Debounced Incremental Preview:* Rasterizes pages to RGBA buffers using `typst-render` and caches `egui::TextureHandle` objects per page.
"##;

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
        [#text(fill: white, weight: "bold")[Crate / Module]],
        [#text(fill: white, weight: "bold")[Responsibility in IDE]],
      ),
      [*GUI & Windowing*], [`egui` + `eframe`], [Dockable side panel, split code/preview panes, light/dark themes],
      [*Virtual File System*], [`src/vfs.rs`], [In-memory tree for WASM & native disk sync for multi-file projects],
      [*Compiler World*], [`src/world.rs`], [Implements `typst::World` with embedded `typst-assets` fonts],
      [*Page Rasterizer*], [`typst-render`], [Renders `typst::layout::Page` frames to crisp RGBA pixel buffers],
      [*PDF Exporter*], [`typst-pdf` + `rfd`], [Compiles standards-compliant PDF bytes & triggers native/web save],
    ),
    caption: [Core subsystem breakdown across native desktop and WebAssembly targets.],
  )
}
"##;

        let diagram_svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 680 120" width="680" height="120">
  <rect x="2" y="2" width="676" height="116" rx="8" fill="#f8fafc" stroke="#cbd5e1" stroke-width="1.5"/>
  <!-- Box 1: VFS -->
  <rect x="20" y="22" width="135" height="76" rx="6" fill="#eff6ff" stroke="#3b82f6" stroke-width="1.8"/>
  <text x="87" y="48" font-family="DejaVu Sans Mono, monospace" font-size="11" font-weight="bold" fill="#1e3a8a" text-anchor="middle">ProjectWorkspace</text>
  <text x="87" y="66" font-family="Libertinus Serif, serif" font-size="10" fill="#475569" text-anchor="middle">In-Memory VFS</text>
  <text x="87" y="82" font-family="Libertinus Serif, serif" font-size="9.5" fill="#64748b" text-anchor="middle">main.typ + assets</text>
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
  <text x="604" y="82" font-family="Libertinus Serif, serif" font-size="9.5" fill="#64748b" text-anchor="middle">&amp; PDF Export</text>
</svg>
"##;

        let refs_bib = r#"@article{typst2023,
  author = {Mädje, Laurenz and Haug, Martin},
  title = {Typst: A Programmable Markup Language for Typesetting},
  journal = {Proceedings of Document Engineering},
  year = {2023},
}

@manual{egui2026,
  author = {Ernerfeldt, Emil},
  title = {egui: An Immediate Mode GUI Library in Rust},
  year = {2026},
}
"#;

        files.insert(
            "main.typ".to_string(),
            VirtualFile::new_text("main.typ", main_typ),
        );
        files.insert(
            "template.typ".to_string(),
            VirtualFile::new_text("template.typ", template_typ),
        );
        files.insert(
            "chapters/intro.typ".to_string(),
            VirtualFile::new_text("chapters/intro.typ", intro_typ),
        );
        files.insert(
            "chapters/methods.typ".to_string(),
            VirtualFile::new_text("chapters/methods.typ", methods_typ),
        );
        files.insert(
            "assets/diagram.svg".to_string(),
            VirtualFile::new_text("assets/diagram.svg", diagram_svg),
        );
        files.insert(
            "refs.bib".to_string(),
            VirtualFile::new_text("refs.bib", refs_bib),
        );

        Self {
            project_name: "typst-studio-demo".to_string(),
            files,
            main_file: "main.typ".to_string(),
            active_file: "main.typ".to_string(),
            open_tabs: vec![
                "main.typ".to_string(),
                "template.typ".to_string(),
                "chapters/intro.typ".to_string(),
                "chapters/methods.typ".to_string(),
            ],
            revision: 1,
            #[cfg(not(target_arch = "wasm32"))]
            root_dir: None,
        }
    }

    pub fn get_file(&self, path: &str) -> Option<&VirtualFile> {
        let key = normalize_vpath(path);
        self.files.get(&key)
    }

    pub fn get_file_mut(&mut self, path: &str) -> Option<&mut VirtualFile> {
        let key = normalize_vpath(path);
        self.files.get_mut(&key)
    }

    pub fn upsert_text_file(&mut self, path: &str, content: String) -> String {
        let key = normalize_vpath(path);
        if key.is_empty() {
            return self.active_file.clone();
        }
        let mut vf = VirtualFile::new_text(key.clone(), content);
        vf.dirty = true;
        self.files.insert(key.clone(), vf);
        if !self.open_tabs.contains(&key) {
            self.open_tabs.push(key.clone());
        }
        self.active_file = key.clone();
        self.revision = self.revision.wrapping_add(1);
        key
    }

    pub fn upsert_binary_file(&mut self, path: &str, bytes: Vec<u8>) -> String {
        let key = normalize_vpath(path);
        if key.is_empty() {
            return self.active_file.clone();
        }
        let mut vf = VirtualFile::new_bytes(key.clone(), bytes);
        vf.dirty = true;
        self.files.insert(key.clone(), vf);
        if !self.open_tabs.contains(&key) {
            self.open_tabs.push(key.clone());
        }
        self.active_file = key.clone();
        if self.active_file.ends_with(".typ") && !self.files.contains_key(&self.main_file) {
            self.main_file = key.clone();
        }
        self.revision = self.revision.wrapping_add(1);
        key
    }

    pub fn select_file(&mut self, path: &str) {
        let key = normalize_vpath(path);
        if self.files.contains_key(&key) {
            if !self.open_tabs.contains(&key) {
                self.open_tabs.push(key.clone());
            }
            self.active_file = key;
        }
    }

    pub fn close_tab(&mut self, path: &str) {
        if self.open_tabs.len() <= 1 {
            return;
        }
        self.open_tabs.retain(|p| p != path);
        if self.active_file == path {
            if let Some(first) = self.open_tabs.first() {
                self.active_file = first.clone();
            }
        }
    }

    pub fn delete_file(&mut self, path: &str) -> bool {
        let key = normalize_vpath(path);
        if key == self.main_file || self.files.len() <= 1 {
            return false;
        }
        if self.files.remove(&key).is_some() {
            self.open_tabs.retain(|p| p != &key);
            if self.active_file == key {
                self.active_file = self.main_file.clone();
                if !self.open_tabs.contains(&self.active_file) {
                    self.open_tabs.push(self.active_file.clone());
                }
            }
            self.revision = self.revision.wrapping_add(1);
            true
        } else {
            false
        }
    }

    pub fn set_main_file(&mut self, path: &str) {
        let key = normalize_vpath(path);
        if let Some(f) = self.files.get(&key) {
            if f.kind == FileKind::Typst {
                self.main_file = key;
                self.revision = self.revision.wrapping_add(1);
            }
        }
    }

    pub fn mark_modified(&mut self, path: &str) {
        let key = normalize_vpath(path);
        if let Some(f) = self.files.get_mut(&key) {
            f.sync_bytes_from_text();
            f.dirty = true;
        }
        self.revision = self.revision.wrapping_add(1);
    }

    /// Groups all files by directory for hierarchical rendering in the Left File Overview Panel.
    /// Key `""` holds top-level root files; other keys hold directory names (e.g., `"chapters"`).
    pub fn grouped_by_folder(&self) -> BTreeMap<String, Vec<&VirtualFile>> {
        let mut groups: BTreeMap<String, Vec<&VirtualFile>> = BTreeMap::new();
        for (path, file) in &self.files {
            if let Some((folder, _name)) = path.rsplit_once('/') {
                groups.entry(folder.to_string()).or_default().push(file);
            } else {
                groups.entry(String::new()).or_default().push(file);
            }
        }
        groups
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn load_from_native_dir(dir: &Path) -> Result<Self, String> {
        if !dir.is_dir() {
            return Err(format!("'{}' is not a directory", dir.display()));
        }
        let mut files = BTreeMap::new();
        Self::scan_dir_recursive(dir, dir, &mut files)?;

        if files.is_empty() {
            let main_content = "= New Project\n\nStart writing your Typst document here.\n";
            files.insert(
                "main.typ".to_string(),
                VirtualFile::new_text("main.typ", main_content),
            );
        }

        let main_file = if files.contains_key("main.typ") {
            "main.typ".to_string()
        } else {
            files
                .values()
                .find(|f| f.kind == FileKind::Typst)
                .map(|f| f.path.clone())
                .unwrap_or_else(|| {
                    files
                        .keys()
                        .next()
                        .cloned()
                        .unwrap_or_else(|| "main.typ".to_string())
                })
        };

        let project_name = dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("workspace")
            .to_string();

        Ok(Self {
            project_name,
            files,
            active_file: main_file.clone(),
            open_tabs: vec![main_file.clone()],
            main_file,
            revision: 1,
            root_dir: Some(dir.to_path_buf()),
        })
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn scan_dir_recursive(
        root: &Path,
        current: &Path,
        out: &mut BTreeMap<String, VirtualFile>,
    ) -> Result<(), String> {
        let entries = std::fs::read_dir(current).map_err(|e| e.to_string())?;
        for entry in entries.flatten() {
            let path = entry.path();
            let file_name = entry.file_name();
            let name_str = file_name.to_string_lossy();
            if name_str.starts_with('.')
                || name_str == "target"
                || name_str == "dist"
                || name_str == "node_modules"
            {
                continue;
            }
            if path.is_dir() {
                Self::scan_dir_recursive(root, &path, out)?;
            } else if path.is_file() {
                if let Ok(rel) = path.strip_prefix(root) {
                    let vpath = normalize_vpath(&rel.to_string_lossy());
                    if let Ok(bytes) = std::fs::read(&path) {
                        // Skip huge binary files > 10 MB
                        if bytes.len() <= 10 * 1024 * 1024 {
                            out.insert(vpath.clone(), VirtualFile::new_bytes(vpath, bytes));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn save_all_to_native_dir(&mut self, dir: &Path) -> Result<usize, String> {
        let mut count = 0;
        for file in self.files.values_mut() {
            file.sync_bytes_from_text();
            let target = dir.join(&file.path);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            std::fs::write(&target, &file.bytes).map_err(|e| e.to_string())?;
            file.dirty = false;
            count += 1;
        }
        self.root_dir = Some(dir.to_path_buf());
        Ok(count)
    }
}
