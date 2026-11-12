# Typst Local Editor (`typst-ide`)

A cross-platform native Rust desktop GUI IDE for [Typst](https://typst.app/) featuring live compiled visual document previews, hierarchical outline tree navigation, and zero-server peer-to-peer real-time collaborative editing using Yrs CRDT and Iroh QUIC networking.

---

## Architecture Overview

The codebase is organized into clean, modular architectural layers following dependency inversion principles:
Inner domain layers have zero dependencies on immediate-mode GUI frameworks or async network runtimes, enabling comprehensive headless unit testing.

```
┌──────────────────────────────────────────────────────────────┐
│                      UI Presentation                         │
│  - app::TypstIdeApp (orchestration & event loop)            │
│  - ui::components (menu_bar, status_bar, sidebar, editor)    │
│  - ui::dialogs (export_pdf, file_modals, settings)           │
│  - ui::theme (ThemePalette & design tokens)                  │
└──────────────────────────────┬───────────────────────────────┘
                               │ delegates to
┌──────────────────────────────▼───────────────────────────────┐
│                    Application Services                      │
│  - compiler (TypstWorld, incremental memoized evaluation)    │
│  - collab::session (P2P background actor & Iroh wire sync)   │
│  - services::screenshot (headless automated capture)         │
│  - project (DiskProject, FileBuffer, sample generator)       │
└──────────────────────────────┬───────────────────────────────┘
                               │ depends on
┌──────────────────────────────▼───────────────────────────────┐
│             Platform & Infrastructure Layer                  │
│  - platform::menu (muda native macOS menu bar)               │
│  - platform::dialogs (rfd native async file/folder pickers)  │
│  - collab::transport (Iroh QUIC endpoint abstraction)        │
│  - collab::persistence (atomic snapshot file storage)        │
└──────────────────────────────┬───────────────────────────────┘
                               │ depends on
┌──────────────────────────────▼───────────────────────────────┐
│                       Core Domain                            │
│  - core::outline (Document outline AST parser & resolver)    │
│  - core::diff (UTF-16 diffing & collision formatting)        │
│  - core::pdf (PDF/A validation & version standards)          │
│  - core::settings (IdeSettings schema & defaults)            │
│  - core::file_kind (File kind classification & icons)        │
└──────────────────────────────────────────────────────────────┘
```

### Module Responsibilities

| Module Path | Responsibilities & Invariants | Line Count |
| :--- | :--- | :--- |
| `app/src/core/` | **Pure Domain Logic**: Zero UI, zero network I/O. Decoupled using the `SourceResolver` trait. | ~800 lines total |
| `app/src/platform/` | **Operating System Abstraction**: Native menu bars (`muda`), native dialogs (`rfd`) running on background threads to prevent event loop blocking. | ~270 lines total |
| `app/src/services/` | **Application Services**: Headless automation, background workers, and life-cycle tasks. | ~64 lines total |
| `app/src/collab/` | **Peer-to-Peer CRDT Engine**: Modular decomposition into `doc` (Yrs CRDT), `awareness` (cursor/presence), `protocol` (wire types & tickets), and `transport` (Iroh QUIC). | ~2,100 lines total |
| `app/src/project/` | **Workspace Management**: Buffer caching, dirty state tracking, and sample project generator (`sample.rs`). | ~830 lines total |
| `app/src/compiler/` | **Typst Evaluation**: Compiles Typst source into vector page frames and rendered preview rasters. | ~290 lines |
| `app/src/ui/` | **View Components & Modals**: Decoupled view rendering, separated into `components/` (editor, sidebar, preview, menu bar, status bar) and `dialogs/` (export PDF, settings, file management). | ~2,400 lines total |
| `app/src/app.rs` | **Top-Level Orchestrator**: Manages state channels, event routing, and window life-cycle. | ~1,080 lines |

---

## Features

- **Cross-Platform Native Desktop**: Runs natively on Linux (Wayland & X11), macOS, and Windows.
- **Live Incremental Preview**: Immediate compiled rendering with interactive zoom and page panning.
- **P2P Collaboration**: Real-time collaborative multi-user editing with cursor visibility using CRDTs (`yrs`) over direct QUIC (`iroh`) connections without central servers.
- **Hierarchical Document Outliner**: Deep AST-based outline navigation supporting Typst sections (`= Heading`), Markdown headings (`# Heading`), and BibTeX citation keys.
- **Rich Code Editor**: Syntax highlighting, gutter line numbering, smart indentation, undo/redo stacks, and quick element snippet inserts.
- **PDF Standards Export**: Built-in export with configurable PDF versions (1.4–2.0) and PDF/A archival standards (PDF/A-1b, PDF/A-2b, PDF/A-3b/a).
- **Embedded & OS-Level Dialogs**: Native dialog integration across platforms with graceful fallback.

---

## Building and Testing

### Prerequisites

- Rust 1.80+ (stable toolchain recommended)
- Standard C compiler and development libraries (`fontconfig` on Linux)

### Commands

From the workspace root or the `app/` directory:

```bash
# Build binary (debug)
cargo build --manifest-path app/Cargo.toml

# Build optimized release binary
cargo build --release --manifest-path app/Cargo.toml

# Run the complete test suite (unit + characterization tests)
cargo test --manifest-path app/Cargo.toml

# Check code formatting
cargo fmt --check --manifest-path app/Cargo.toml

# Run strict linter checks
cargo clippy --all-targets --manifest-path app/Cargo.toml -- -D warnings
```

---

## How to Add a New Feature

When extending `typst-ide`, follow these architectural guidelines to maintain modularity, testability, and zero regression.

### 1. Place Pure Domain Logic in `core/`
- If your feature involves data structures, validation, parsing, calculations, or algorithms that do not require GUI or network access, implement them in `app/src/core/<feature_name>.rs`.
- Decouple disk or external dependencies behind traits (like `SourceResolver`).
- Write comprehensive unit tests in a companion test file `app/src/core/<feature_name>/tests.rs` or in `app/tests/`.

### 2. Implement Background or I/O Operations in `services/` or `platform/`
- For OS-specific features (e.g., custom window badges, shell integration), place them under `app/src/platform/` using appropriate `#[cfg(target_os = "...")]` attributes.
- Long-running or async tasks (file watcher, network fetcher, headless exporter) should live in `app/src/services/` and communicate with the UI via crossbeam or tokio mpsc channels.

### 3. Build UI Components in `ui/components/` or `ui/dialogs/`
- **Reusable widgets & panes**: Place in `app/src/ui/components/<component_name>.rs`. Follow the `render_*` pattern:
  ```rust
  pub struct MyComponentParams<'a> { ... }
  pub struct MyComponentOutput { ... }
  pub fn render_my_component(ui: &mut Ui, params: MyComponentParams<'_>) -> MyComponentOutput { ... }
  ```
- **Modal sub-windows**: Place in `app/src/ui/dialogs/<dialog_name>.rs`. Ensure dialogs can be opened either as embedded floating windows or native sub-windows depending on platform capability.

### 4. Wire the Component in `app.rs`
- In `TypstIdeApp`, store any necessary state, pass parameters to your component's render function, and match on the returned `MyComponentOutput` struct to dispatch actions cleanly.

### 5. Validate Before Submitting
Ensure all checks pass cleanly without warnings:
```bash
cargo fmt --check --manifest-path app/Cargo.toml
cargo clippy --all-targets --manifest-path app/Cargo.toml -- -D warnings
cargo test --manifest-path app/Cargo.toml
```

---

## License

GNU General Public License v3.0
