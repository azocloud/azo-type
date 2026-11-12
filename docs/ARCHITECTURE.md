# Architecture Specification & Migration Plan: Azo Type Desktop IDE

## 1. Executive Summary & Assessment of Current Codebase

Azo Type is a high-performance cross-platform desktop IDE for Typst written in Rust with `egui`, `eframe`, `typst`, and `iroh` (P2P QUIC transport). While the MVP features (live preview, P2P collaboration, package download, PDF export, outline navigation) function reliably and all 12 existing tests pass, the current internal structure suffers from severe architectural debt:

- **Monolithic God File**: `src/app.rs` spans **4,475 lines**, centralizing GUI layout, AST parsing, outline tree calculation, file tree management, text editing, syntax styling, native dialogs, modal state, and tests in a single file.
- **Entanglement of Pure Domain Logic and Immediate-Mode GUI**: Parsing document outline headings (`extract_typst_ast_outline`, `assign_hierarchical_numbers`, `build_outline_tree`), text diff calculation, and settings validation are trapped inside UI functions, requiring GUI dependencies for testing.
- **In-Source Test Contamination**: Unit tests reside inside production files across 6 source files (`app.rs`, `compiler.rs`, `collab/doc.rs`, `collab/awareness.rs`, `collab/session.rs`, `collab/transport.rs`).
- **Mixed Error & Dialog Handling**: Blocking busy loops (`pollster::block_on_or_spin`) run inside threads for native file dialogs.
- **Unused Dependencies**: Multiple crates in `Cargo.toml` (`log`, `ecow`, `comemo`, `typst-syntax`, `typst-assets`) are unreferenced.

---

## 2. Baseline Status

Recorded from initial checks on the pristine worktree before making any code modifications:

| Check | Tool Command | Result | Notes |
| :--- | :--- | :--- | :--- |
| **Build** | `cargo build --locked` | **Success (exit 0)** | Compiles in ~1m 28s. |
| **Tests** | `cargo test --locked` | **Success (exit 0)** | 12 tests passed, 0 failed. |
| **Clippy** | `cargo clippy --all-targets` | **Warnings (exit 0)** | 4 distinct warnings (`if_same_then_else` x2, `collapse_match`, `derivable_impls`). |
| **Strict Clippy** | `cargo clippy --all-targets -- -D warnings` | **Failed (exit 101)** | Blocked by the 4 clippy warnings above. |
| **Format** | `cargo fmt --check` | **Failed (exit 1)** | 59 formatting differences across 9 source files. |

---

## 3. Architectural Principles & Target Architecture

### 3.1 Core Principles

1. **Strict Layer Separation (Dependency Inversion)**:
   - **Domain / Core (innermost)**: Zero GUI dependencies (`egui`/`eframe`), zero network dependencies (`iroh`), zero filesystem I/O (`std::fs`). Pure data types, AST manipulation, algorithms, state vector diffing, settings models.
   - **Infrastructure**: Implementations of external systems (OS filesystem `std::fs`, Typst compiler runtime, `iroh` QUIC networking, `rfd` native dialogs).
   - **Application / Services**: Orchestrates domain objects, background compilation worker, and collaboration session actor.
   - **UI (outermost)**: Immediate-mode views (`egui`), presentation state, rendering widgets, modal dialogs, and user event dispatching.
2. **File Size & Single Responsibility**:
   - Standard target: **under 300–400 lines** per file. No file shall exceed 600 lines without documented justification.
   - Modules organized by functional domain (e.g. `outline`, `editor`, `pdf`, `project`), never generic technical dumps (`utils.rs`, `misc.rs`).
3. **Clean Test Separation**:
   - Zero test code in production files.
   - Unit tests located in companion files: `module.rs` + `module/tests.rs` via `#[cfg(test)] mod tests;` or in dedicated `tests/` integration suites.
   - All tests use behavior-describing names (`computes_hierarchical_numbering_for_nested_headings`).
4. **Platform-Specific Isolation**:
   - OS-specific integrations (macOS `muda` menu bar, macOS window activation, Windows window level) isolated behind clean platform abstraction traits or designated `platform::*` modules.
5. **Typed Errors**:
   - Library and domain errors use specific typed errors (`thiserror`); application boundary handles presentation.

---

## 4. Evaluation: Cargo Workspace vs. Modularized Single Crate

### Evaluation Matrix

| Metric | Multi-Crate Workspace (`core`, `collab`, `ide`) | Modularized Single Crate (`src/{core, infra, services, ui}`) |
| :--- | :--- | :--- |
| **Build Time Impact** | Faster parallel incremental builds when editing UI, but initial workspace metadata overhead. | Fast compilation with existing `Cargo.lock`, zero risk of CI workspace breakage. |
| **Domain Isolation** | Absolute physical boundary enforced by compiler `Cargo.toml`. | Enforced via module visibility (`pub(crate)`) and clear architectural layering. |
| **CI & Packaging Risk** | High risk of breaking `cargo build --locked` in existing CI scripts or external paths. | **Zero risk**: preserves existing `Cargo.toml` structure while cleanly modernizing code. |
| **Step-by-Step Feasibility** | Requires modifying `Cargo.toml`, moving directories, updating CI in one jump. | Allows pure incremental commits where every single step compiles and passes tests. |

### Architectural Decision

We adopt a **modularized, cleanly layered architecture within the crate** as the primary refactoring target. This guarantees:
1. Strict adherence to the rule: every intermediate commit must compile cleanly with `cargo test` and `cargo build`.
2. Existing CI workflows (`cargo build --locked`, `cargo test --locked`) remain 100% green at every step.
3. The directory structure directly mirrors the logical boundaries (`src/core/`, `src/services/`, `src/infra/`, `src/ui/`, `src/platform/`), so extracting into separate workspace crates in the future is a zero-friction, trivial relocation if desired.

---

## 5. Target Directory & Module Structure

```text
app/src/
├── main.rs                          // Minimal bootstrap (<80 lines): CLI parsing, window init
├── core/                            // Pure domain logic: NO egui, NO std::fs, NO iroh
│   ├── mod.rs
│   ├── diff.rs                      // UTF-16 diff, CharSelection, offset converters
│   │   └── tests.rs
│   ├── outline.rs                   // DocumentOutlineItem, OutlineTreeNode, numbering, AST walk
│   │   └── tests.rs
│   ├── settings.rs                  // IdeSettings, SettingsTab, defaults
│   │   └── tests.rs
│   ├── pdf.rs                       // PdfExportConfig, PdfVersionChoice, PdfArchivalStandard
│   │   └── tests.rs
│   └── syntax.rs                    // ThemePalette, design tokens, SyntaxKind color mapping
│       └── tests.rs
├── infra/                           // Infrastructure & external systems
│   ├── mod.rs
│   ├── fs.rs                        // FsTreeNode, FileKind, directory scanner, path sanitizer
│   │   └── tests.rs
│   ├── compiler.rs                  // Typst compiler invocation, page rasterization, diagnostics
│   │   └── tests.rs
│   └── world.rs                     // EditorWorld implementing typst::World, package downloader
├── collab/                          // Real-time P2P collaboration subsystem
│   ├── mod.rs
│   ├── doc.rs                       // CollabProjectDoc (yrs CRDT wrapper, collision resolution)
│   │   └── tests.rs
│   ├── awareness.rs                 // PeerPresence, CollabAwareness (yrs::sync::Awareness)
│   │   └── tests.rs
│   ├── persistence.rs               // CollabPersistence (crdt-snapshot.yrs, disk flush)
│   │   └── tests.rs
│   ├── transport.rs                 // CollabTransport trait, IrohTransport, ticket, wire protocol
│   │   └── tests.rs
│   └── session.rs                   // CollabSessionHandle, SessionActor (tokio background thread)
│       └── tests.rs
├── services/                        // Application services & state orchestration
│   ├── mod.rs
│   ├── project.rs                   // DiskProject, OpenBuffer, file operations, tab lifecycle
│   │   └── tests.rs
│   ├── compile_worker.rs            // Background compile thread worker & debouncing
│   └── screenshot.rs                // Headless screenshot lifecycle runner
├── platform/                        // Platform-specific OS integration
│   ├── mod.rs
│   ├── menu.rs                      // Native OS menu bar trait, macOS muda implementation
│   └── dialogs.rs                   // Async native dialog runner (folder picker, file saver)
└── ui/                              // Presentation & egui rendering
    ├── mod.rs
    ├── app.rs                       // AzoTypeApp (eframe::App implementation, event loop, ~250 lines)
    ├── theme.rs                     // Egui visuals & styling application from ThemePalette
    ├── state.rs                     // UI presentation state (split ratio, zoom, modal flags)
    ├── components/
    │   ├── mod.rs
    │   ├── menu_bar.rs              // Desktop menu bar (Linux & Windows)
    │   ├── status_bar.rs            // Bottom status bar
    │   ├── file_tree.rs             // Sidebar disk project tree
    │   ├── outline_panel.rs         // Sidebar document outline tree & navigation
    │   ├── editor.rs                // TextEdit, gutter line numbers, wrap glyphs, context menu
    │   ├── preview.rs               // Live document page canvas, zoom, split divider
    │   └── diagnostics.rs           // Compiler diagnostics banner
    └── dialogs/
        ├── mod.rs
        ├── settings.rs              // Settings OS sub-window (Appearance, Editor, Compiler)
        ├── export_pdf.rs            // Export PDF OS sub-window
        ├── file_modals.rs           // New File and Rename File OS sub-windows
        └── collab.rs                // Collaboration OS sub-window & remote cursor overlay
```

---

## 6. Step-by-Step Migration Plan

Every step is an isolated atomic commit. After every single step:
1. `cargo check` compiles with zero errors.
2. `cargo test` passes 100%.
3. `cargo clippy` and `cargo fmt` pass cleanly.

### Step 1: Characterization Tests & Pure Logic Extraction (Phase 1)
- Write comprehensive characterization unit tests for document outline parsing, hierarchical numbering, tree nesting, UTF-16 offset conversions, and PDF export configuration.
- Place all new characterization tests in dedicated test modules.
- Verify 100% pass rate.

### Step 2: Establish `core` Domain Layer
- Extract `CharSelection`, `Utf16TextDiff`, and offset converters from `collab/doc.rs` into `core::diff`.
- Extract `DocumentOutlineItem`, `OutlineTreeNode`, AST heading parsing, and hierarchical numbering from `app.rs` into `core::outline`.
- Extract `IdeSettings`, `SettingsTab` from `app.rs` into `core::settings`.
- Extract `PdfExportConfig`, `PdfVersionChoice`, `PdfArchivalStandard` from `compiler.rs` into `core::pdf`.
- Extract `ThemePalette` and syntax coloring from `highlight.rs` into `core::syntax`.
- Update imports; verify `cargo test` and `cargo build`.

### Step 3: Relocate Unit Tests out of Production Files
- Move tests from `collab/doc.rs` -> `collab/doc/tests.rs`.
- Move tests from `collab/awareness.rs` -> `collab/awareness/tests.rs`.
- Move tests from `collab/session.rs` -> `collab/session/tests.rs`.
- Move tests from `collab/transport.rs` -> `collab/transport/tests.rs`.
- Move tests from `compiler.rs` -> `infra/compiler/tests.rs`.
- Move tests from `app.rs` -> `core/outline/tests.rs` and `core/settings/tests.rs`.
- Verify production files have zero test code.

### Step 4: Extract Infrastructure & Platform Modules
- Create `platform::menu` isolating the macOS `muda` menu bar and Windows/Linux menu action dispatch.
- Create `platform::dialogs` wrapping `rfd::AsyncFileDialog`.
- Relocate `EditorWorld` and `SharedWorldResources` to `infra::world`.
- Relocate `DiskProject` and `OpenBuffer` to `services::project`.

### Step 5: Decompose `app.rs` UI into Focused Components & Dialogs
- Create `ui::dialogs::export_pdf` for the Export PDF OS sub-window.
- Create `ui::dialogs::settings` for the Settings OS sub-window.
- Create `ui::dialogs::file_modals` for New/Rename file OS sub-windows.
- Create `ui::dialogs::collab` moving collaboration window rendering from `collab/ui.rs`.
- Create `ui::components::editor` for text editing, line numbering gutter, wrap indicators, and right-click context menu.
- Create `ui::components::preview` for page canvas, zoom, resizable divider, and diagnostics banner.
- Create `ui::components::file_tree` for project tree rendering.
- Create `ui::components::outline_panel` for outline tree rendering.
- Create `ui::components::menu_bar` and `ui::components::status_bar`.
- Reduce `ui::app::AzoTypeApp` to a clean coordinator under 300 lines implementing `eframe::App`.

### Step 6: Code Quality, Dead Code Removal & Documentation (Phase 3)
- Address the 4 Clippy warnings documented in `docs/FINDINGS.md`.
- Run `cargo fmt` across all files.
- Ensure strict `cargo clippy --all-targets -- -D warnings` passes.
- Document changes in `docs/REFACTORING_LOG.md`.
- Update `README.md` with architectural layout, build instructions, and feature extension guide.

