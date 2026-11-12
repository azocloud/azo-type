//! Sample Typst project creation on disk for immediate inspection and test compilation.

use std::fs;
use std::path::PathBuf;

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
        .map(|s| {
            s.contains("= Executive Summary") && !s.contains("=== Package & Universe Resolution")
        })
        .unwrap_or(true);
    if !main_path.exists() || needs_sample_refresh {
        let main_typ = r##"#import "template.typ": project, callout, metric-grid
#import "chapters/methods.typ": architecture-table

#show: project.with(
  title: "Native Desktop Azo Type",
  subtitle: "Direct OS Filesystem Workspace & Real-Time Typesetting in Rust",
  authors: (
    (name: "Dr. Elena Vance", affiliation: "Desktop Systems Group"),
    (name: "Marcus Lindqvist", affiliation: "Compiler Architecture Lab"),
  ),
  date: "November 2026",
  version: "v2.0.0",
)

= Executive Summary

*Azo Type* is a native desktop development environment for *Linux*, *macOS*, and *Windows* built in Rust with `egui`, `eframe`, and the `typst` compiler stack. It operates directly on real project folders and files on your local filesystem, providing an interactive side panel file tree, native OS menu integration, a customizable *Settings* window (`File > Settings`), and instant PDF export.

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
        let template_typ = r##"// Reusable document template & components for Azo Type

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
          align(right)[Azo Type #if version != none [(#version)]],
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
