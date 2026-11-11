#import "template.typ": project, callout, metric-grid
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
