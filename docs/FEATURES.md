# Feature Specification & Regression Checklist: Typst Studio Desktop IDE

This document provides a comprehensive inventory of all user workflows, UI capabilities, compiler integrations, and P2P collaboration features in the Typst Studio codebase. It serves as the regression testing checklist during and after refactoring.

---

## 1. Application Startup, CLI & Lifecycle

- [ ] **CLI Argument Parsing (`main.rs`)**:
  - [ ] Opening folder by passing positional argument: `typst-ide <path>` opens the target directory if it exists.
  - [ ] Specifying screenshot destination via `--screenshot <path>` or `--screenshot=<path>`.
  - [ ] Fallback: If no directory is supplied, ensures and opens `./sample-project` on disk.
- [ ] **Window Configuration**:
  - [ ] Title set to `"Typst Studio — Native Desktop Typst IDE"`.
  - [ ] Default window dimensions `1360 x 860`, minimum size `800 x 520`.
  - [ ] Platform-specific window adjustments (e.g., Windows `WindowLevel::AlwaysOnBottom` when `--screenshot` is active; macOS `with_activate_ignoring_other_apps(false)`).
- [ ] **Headless / CI Screenshot Capture**:
  - [ ] Repaint pump until frame count reaches 120 frames.
  - [ ] Dispatches viewport screenshot command, captures `egui::ColorImage`, encodes to target image format (PNG/JPEG) on disk, and cleanly closes the application.

---

## 2. Project & Filesystem Management (`DiskProject`)

- [ ] **Folder Operations**:
  - [ ] Open project folder via native OS file dialog (`File > Open Project Folder...` / `Ctrl+O`).
  - [ ] Rescan folder from disk (`File > Rescan Folder on Disk` / `F5` / Sidebar "Refresh" button) updates tree structure and re-triggers document compilation.
  - [ ] Automatic fallback to `./sample-project` if chosen folder cannot be opened.
  - [ ] Ignore internal directories during directory scan (`target/`, `node_modules/`, `shots/`).
- [ ] **File Tree (Sidebar Pane)**:
  - [ ] Collapsible hierarchical tree view of folders and files.
  - [ ] File type detection and icon attribution (`.typ` -> `[T]`, `.bib`/`.yml`/`.yaml` -> `[B]`, `.svg` -> `[S]`, raster images -> `[I]`, text documents -> `[F]`, binary -> `[#]`).
  - [ ] Active file indicator and main file badge (`★`).
  - [ ] Dirty (unsaved) file indicator (`*`).
  - [ ] Remote collaborator presence badges (`● <PeerName>`) on files currently open by collaborators.
  - [ ] Context menu on tree files:
    - [ ] Display absolute path in tooltip/header.
    - [ ] "Set as Main File" (for `.typ` files).
    - [ ] "Rename File...".
    - [ ] "Delete File" (disabled for current main file).
- [ ] **File Manipulation**:
  - [ ] "New File in Project..." dialog (`+ File` button / `Ctrl+N`): creates file on disk including parent directories; seeds default heading template for `.typ` files.
  - [ ] "Rename File..." dialog: renames on disk, preserves open editor buffer, updates tab titles and main file reference.
  - [ ] "Delete File": deletes from disk, removes buffer and closes tab; selects next available `.typ` file if open.
  - [ ] Path traversal protection: prevents file operations escaping project root.
- [ ] **Buffer & Tab Management**:
  - [ ] Open file tabs bar across top of editor pane.
  - [ ] Switching active tab switches editor buffer and updates status bar.
  - [ ] Closing tab via `×` button (tabs retain at least one open file).
  - [ ] Saving active buffer (`File > Save Active File` / `Ctrl+S`).
  - [ ] Saving all modified buffers (`File > Save All Project Files`).
  - [ ] Auto-save on edit option (`settings.auto_save_on_edit`).
  - [ ] Show/hide hidden dotfiles toggle (`settings.show_hidden_files`).

---

## 3. Typst Code Editor

- [ ] **Editing & Layout**:
  - [ ] Multi-line code editing backed by `egui::TextEdit`.
  - [ ] Monospace typography with configurable font size (`settings.editor_font_size`, 10pt–22pt).
  - [ ] Line numbers gutter accurately aligned with visual rows, showing logical line numbers and wrap continuation glyphs (`↪`).
  - [ ] Word wrapping toggle (`↩ Wrap` toolbar button / View menu / Settings) wrapping to panel width with visual `↩` marker.
  - [ ] Full-width active line background highlight (`#262530` dark / `#EBF3FE` light) updating with caret position.
- [ ] **Syntax Highlighting**:
  - [ ] Complete Typst syntax highlighter (`highlight_typst_code`) using `typst::syntax::LinkedNode`.
  - [ ] Dark theme palette: keywords (`#82AAFF`), comments (`#757582`), strings (`#448C6B`), properties/functions (`#C792EA`), numbers (`#CE5555`), operators/hash (`#F3B05A`), markup (`#E6E6E6`).
  - [ ] Light theme palette: keywords (`#B83A4B`), comments (`#8A8A93`), strings (`#1C7D52`), functions (`#0B60B0`), numbers (`#C75C14`), punctuation (`#2E2E38`), markup (`#1C1C20`).
  - [ ] Syntax highlighting toggle (`settings.syntax_highlighting`).
- [ ] **Formatting Toolbar & Snippets**:
  - [ ] Heading button (`= Heading`): inserts `\n\n== New Section\n`.
  - [ ] Bold button (`*Bold*`): wraps selection or inserts ` *bold text* `.
  - [ ] Italic button (`_Italic_`): wraps selection or inserts ` _emphasized_ `.
  - [ ] Math button (`$ Math $`): inserts `\n$ f(x) = integral_0^x e^(-t^2) dif t $\n`.
  - [ ] Table button (`+ Table`): inserts formatted `#table(...)` snippet.
  - [ ] Figure button (`+ Figure`): inserts `#figure(image("assets/diagram.svg", ...))` snippet.
  - [ ] Font size quick controls (`A+`, `A-`).
- [ ] **Editor Context Menu (Right-Click)**:
  - [ ] Preserves existing selection when right-clicking.
  - [ ] Undo (`Ctrl+Z`) and Redo (`Ctrl+Y` / `Ctrl+Shift+Z`).
  - [ ] Cut (`Ctrl+X`), Copy (`Ctrl+C`), Paste (`Ctrl+V`) with OS clipboard synchronization and fallback.
  - [ ] Select All (`Ctrl+A`).
  - [ ] Text wrapping: Bold (`*...*`), Italic (`_..._`), Inline Code (`` `...` ``), Inline Math (`$...$`).
  - [ ] Snippet insertion submenu: Section Heading, Display Math Block, Formatted Table, Figure / Image, Code Block.
  - [ ] Save File, Compile Document Now, Export PDF, Set as Main Entrypoint.
- [ ] **Asset Inspector View**:
  - [ ] Triggered when opening binary or image files (`.png`, `.jpg`, `.svg`).
  - [ ] Displays file path, kind, size in bytes, and example `#image(...)` embed snippet.
  - [ ] "Edit SVG XML" / "Switch to Source Code Editor" toggle for `.svg` files.

---

## 4. Document Outline & Navigation

- [ ] **Outline Extraction**:
  - [ ] Typst source parsing: extracts AST headings (`= ...`, `== ...`, `=== ...`) and function-based headings (`#heading(level: ...)[...]`).
  - [ ] Ignores headings inside comments, raw code blocks (` ``` `), and styling rules (`show heading.where(...)`).
  - [ ] Recursive resolution: follows `#include "..."` and `#module` includes across project files.
  - [ ] Merges introspector-compiled outline items from templates/imported functions.
  - [ ] Bibliography outline: extracts `@article{key, ...}` entries.
  - [ ] Markdown outline: extracts `# Heading` lines.
- [ ] **Hierarchy & Numbering**:
  - [ ] Automatically assigns hierarchical numbering (`1`, `1.1`, `1.1.1`, `1.2`).
  - [ ] Builds multi-level nested tree (`OutlineTreeNode`) with parent-child relationships.
  - [ ] Tree view rendering with vertical hierarchy guidelines, expand/collapse toggles, and level badges (`= 1`, `== 1.1`).
- [ ] **Interactive Navigation**:
  - [ ] Clicking any outline item opens the target file (if not active), places cursor at target character offset, centers editor scroll area, and highlights active line.
  - [ ] Active heading tracking: highlights current outline item matching cursor character offset in active editor buffer.

---

## 5. Live Document Preview & Compiler

- [ ] **Compilation Pipeline**:
  - [ ] Non-blocking background thread compilation (`std::thread::spawn` + channels).
  - [ ] Configurable debounced compilation on edit (`settings.debounce_ms`, default 120ms).
  - [ ] Manual compile trigger (`Project > Compile Document Now`).
  - [ ] `EditorWorld` implementing `typst::World`:
    - [ ] Resolves local files from disk and unsaved in-memory buffers.
    - [ ] Resolves embedded fonts (17 bundled faces) + system installed fonts.
    - [ ] Package resolution: downloads external packages (`@preview/...`) via `SystemPackages` and `ReqwestDownloader`.
    - [ ] Effective compile entrypoint: compiles `main_file`, or active standalone `.typ` if unreferenced by main.
- [ ] **Page Preview Rendering**:
  - [ ] Renders document pages to RGBA textures via `typst_render`.
  - [ ] Configurable rasterization scale: 1.5x Fast, 2.0x Crisp (Default), 3.0x Ultra HiDPI.
  - [ ] Vertical scrolling page list with page count labels ("Page X of Y").
  - [ ] Page border and subtle drop shadows.
  - [ ] Zoom controls: Zoom In (`+`), Zoom Out (`-`), Fit Width (`1.0`), and Ctrl+Wheel zoom delta clamping between 0.4x and 2.5x.
- [ ] **Resizable Split Divider**:
  - [ ] Interactive draggable vertical divider between editor and preview panes.
  - [ ] Hover cursor icon (`CursorIcon::ResizeHorizontal`) and visual grip handle.
  - [ ] Double-click to reset split ratio to 50/50.
- [ ] **Compiler Diagnostics**:
  - [ ] Diagnostics banner for errors and warnings below preview pane.
  - [ ] Color-coded badges with file location (`file:line:col`).
  - [ ] Clicking diagnostic location opens the file in editor.
  - [ ] Displays diagnostic messages and compiler hints.

---

## 6. PDF Export & Standards Conformance

- [ ] **PDF Export Dialog (`File > Export PDF...` / `Ctrl+E`)**:
  - [ ] Rendered as native OS sub-window (`typst_studio_export_pdf_os_window`).
  - [ ] Displays target document path and current configuration summary badge.
- [ ] **PDF Specification Versions**:
  - [ ] Selectable version: PDF 1.4, PDF 1.5, PDF 1.6, PDF 1.7 (Default), PDF 2.0.
- [ ] **Archival Standards (PDF/A)**:
  - [ ] Presets & selector for ISO 19005 standards: None, PDF/A-1b, PDF/A-1a, PDF/A-2b, PDF/A-2u, PDF/A-2a, PDF/A-3b, PDF/A-3u, PDF/A-3a, PDF/A-4, PDF/A-4f, PDF/A-4e.
  - [ ] PDF/UA-1 (ISO 14289-1 Universal Accessibility) enforcement.
  - [ ] Tagged PDF structure tree generation.
- [ ] **Standards Compatibility & CLI Validation**:
  - [ ] Validates version/standard compatibility (e.g. PDF/A-1 requires PDF 1.4; PDF/A-4 requires PDF 2.0).
  - [ ] Shows recommended version switch button when mismatch occurs.
  - [ ] Live preview of equivalent CLI command: `typst compile --pdf-standard <flags> <entry>`.
  - [ ] "Reset Defaults" button.
- [ ] **Disk Export Flow**:
  - [ ] Compiles PDF bytes in background with timestamp metadata.
  - [ ] Native file save dialog (`rfd::AsyncFileDialog`) with `.pdf` extension filter.
  - [ ] Writes PDF to chosen location and reports file size in status notice.

---

## 7. Real-Time P2P Collaboration (CRDT + Iroh QUIC)

- [ ] **Architecture & Actor Lifecycle**:
  - [ ] Background `tokio` actor thread owning `CollabProjectDoc`, `CollabAwareness`, and `CollabPersistence`.
  - [ ] Communicates with UI thread strictly via non-blocking channels (`UiToSessionCmd` and `SessionToUiEvent`).
- [ ] **Hosting a Session**:
  - [ ] Binds `iroh` QUIC endpoint using ALPN `b"typst-studio/collab/v1"`.
  - [ ] Generates cryptographically secure 256-bit random session secret.
  - [ ] Generates URL-safe invite ticket prefixed with `typst-collab-v1:`.
  - [ ] Reconciles all local editable text files into CRDT `"files"` map.
  - [ ] Seeds snapshot to `.typst-collab/crdt-snapshot.yrs`.
  - [ ] Local-only / LAN mode toggle (disables public relays).
- [ ] **Joining a Session**:
  - [ ] Decodes invite ticket, validates protocol version and 256-bit secret.
  - [ ] Connects to host endpoint via QUIC / TLS 1.3.
  - [ ] Exchanges `HandshakeHello` and `HandshakeAck`.
  - [ ] Initial state synchronization via `SyncStep1` and `SyncStep2` state vector diffing.
  - [ ] Populates received remote files directly to local project folder on disk.
- [ ] **Real-Time Collaboration**:
  - [ ] Multi-file editing: different collaborators can edit different files concurrently.
  - [ ] Local text edits compute minimal UTF-16 diff and commit with `Origin::from("local")`.
  - [ ] Incremental `CrdtUpdate` frames broadcast over QUIC bi-streams (capped at 8 MiB).
  - [ ] Remote updates applied with `Origin::from("remote")` without echo or local undo pollution.
  - [ ] Cursor preservation (`StickyIndex`) ensures local cursor position is maintained during remote edits.
  - [ ] Remote cursor, selection, and floating name tags painted directly onto editor canvas.
  - [ ] Deterministic path collision resolution for concurrent file creations (`<stem>.conflict-<short_id>.<ext>`).
  - [ ] Local undo/redo isolation: `yrs::UndoManager` only reverts local changes.
  - [ ] Path traversal protection when writing files to disk.
  - [ ] Auto-reconnect with exponential backoff on client connection drop.
- [ ] **Collaboration Window UI**:
  - [ ] Rendered as native OS sub-window (`Project > Collaboration (P2P)...`).
  - [ ] Displays local node endpoint ID, ticket copy button, mask/reveal toggle.
  - [ ] Connected peer list with display name, color swatch, active file name, and connection route (`Direct (QUIC)` vs `Relay` with RTT).

---

## 8. Settings & User Customization

- [ ] **Settings Sub-Window (`File > Settings` / `Ctrl+,`)**:
  - [ ] Rendered as native OS sub-window.
  - [ ] Three tabs: Appearance, Editor & Filesystem, Compiler & Preview.
- [ ] **Appearance Tab**:
  - [ ] Theme toggle: Dark Mode (`#1C1B22`) vs Light Mode (`#FFFFFF`).
  - [ ] Editor Font Size slider (10.0pt – 22.0pt).
  - [ ] Automatic line wrapping checkbox.
  - [ ] Line numbers gutter checkbox.
  - [ ] Real-time syntax highlighting checkbox.
  - [ ] Design System Tokens preview swatches and "Copy CSS Tokens" button.
- [ ] **Editor & Filesystem Tab**:
  - [ ] Active project directory display.
  - [ ] Automatic line wrapping checkbox.
  - [ ] Auto-save modified buffers on edit checkbox.
  - [ ] Show hidden dotfiles in project tree checkbox.
  - [ ] "Open Another Project Folder..." button.
- [ ] **Compiler & Preview Tab**:
  - [ ] Auto-compile while typing checkbox.
  - [ ] Compile debounce delay slider (50ms – 800ms).
  - [ ] Preview rasterization DPI selector (1.5x, 2.0x, 3.0x).
- [ ] **Global Reset**:
  - [ ] "Reset to Defaults" button restores all settings to default values.

---

## 9. Bottom Status Bar & Indicators

- [ ] Left section:
  - [ ] Compilation state indicator: `"Compiling..."` (orange), `"[N] Error(s)"` (red), or `"Compiled ([T] ms)"` (green).
  - [ ] Warning count indicator (if warnings exist).
  - [ ] Main entrypoint file path.
  - [ ] Rendered page count.
  - [ ] P2P collaboration badge (`"P2P Host/Peer: [N] peer(s)"`).
  - [ ] Status notice message.
- [ ] Right section:
  - [ ] Active file relative path.
  - [ ] Save state: `"Saved"` vs `"Modified"`.
  - [ ] Active file line count.
  - [ ] Active file size in bytes.

---

## 10. Platform-Specific Integration

- [ ] **Linux**:
  - [ ] In-window desktop menu bar (`File`, `Edit`, `View`, `Project`).
  - [ ] Native file dialogs via `rfd` / GTK / portal.
  - [ ] Sub-windows via `show_viewport_immediate`.
- [ ] **macOS**:
  - [ ] Native system screen-top menu bar using `muda::Menu`.
  - [ ] In-window menu bar suppressed.
  - [ ] Cmd-based shortcut bindings (`Cmd+S`, `Cmd+O`, `Cmd+N`, etc.).
- [ ] **Windows**:
  - [ ] In-window desktop menu bar.
  - [ ] Native file dialogs via `rfd` / Win32.
  - [ ] Windows window level adjustments for headless capture.
