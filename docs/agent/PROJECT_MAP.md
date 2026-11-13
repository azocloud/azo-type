# PROJECT_MAP.md — Auto-Generated Module & Architecture Reference

> **DO NOT EDIT MANUALLY.** This file is generated from `//!` module doc comments and
> verified by `python3 scripts/verify_structural_rules.py`. To regenerate after adding or
> updating a module, run: `python3 scripts/verify_structural_rules.py --write-map`.

## 1. Workspace & Crate Overview

| Crate / Directory | Role | Manifest / Entrypoint |
| :--- | :--- | :--- |
| `app/` (`azo-type`) | Native cross-platform desktop GUI IDE for Typst | `app/Cargo.toml`, `app/src/main.rs`, `app/src/lib.rs` |
| `app/tests/` | Black-box integration & characterization test suites | `app/tests/*_tests.rs` |
| `scripts/` | Mechanical convention checks and verification runners | `scripts/check.sh`, `scripts/verify_structural_rules.py` |
| `docs/agent/` | AI agent onboarding, recipes, conventions, and gotchas | `AGENTS.md`, `docs/agent/PROJECT_MAP.md` |

## 2. Production Modules (`app/src/`)

| Module Path | Purpose (`//!` Summary) | Key Public Types / Entrypoints | Internal `crate::` Deps | Tests Location | Notes |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `app/src/app.rs` | Top-level application state orchestrator (`AzoTypeApp`), event loop, and channel dispatcher. | `IoEvent`, `TypstIdeApp`, `AzoTypeApp`, `set_language` | `collab`, `compiler`, `core`, `engine`, `highlight`, `platform`, `project`, `ui`, `world` | *(covered via integration/UI)* | Top-level `eframe::App` state orchestrator; delegates view rendering to `ui/`. |
| `app/src/collab/awareness.rs` | Ephemeral peer presence, active file tracking, and remote cursor awareness synchronization. | `PeerPresence`, `CollabAwareness`, `client_id`, `set_local_presence`, `encode_full_update`, `apply_remote_awareness_bytes` | *(none)* | `app/src/collab/awareness/tests.rs` | — |
| `app/src/collab/doc.rs` | Multi-file Yrs CRDT document container (`CollabProjectDoc`), sticky cursors, and local undo manager. | `LOCAL_ORIGIN`, `REMOTE_ORIGIN`, `FILES_MAP_KEY`, `CollabFileEntry`, `CollabProjectDoc`, `with_client_id` | `core` | `app/src/collab/doc/tests.rs` | — |
| `app/src/collab/mod.rs` | Peer-to-peer real-time collaborative editing subsystem (Yrs CRDT + Iroh QUIC transport). | *(re-exports / entry)* | *(none)* | *(covered via integration/UI)* | — |
| `app/src/collab/persistence.rs` | Atomic disk persistence for CRDT state snapshots (`.typst-collab/state.yrs`) and workspace reconciliation. | `PersistedFileIdMap`, `CollabPersistence`, `snapshot_path`, `file_ids_path`, `load_and_reconcile_from_disk`, `save_snapshot` | `collab`, `project` | `app/tests/persistence_tests.rs` | — |
| `app/src/collab/protocol.rs` | Wire protocol definitions, tickets, and abstract transport trait for P2P collaboration. | `COLLAB_ALPN`, `COLLAB_PROTOCOL_VERSION`, `MAX_WIRE_FRAME_BYTES`, `TICKET_PREFIX`, `ConnectionRoute`, `badge_label` | *(none)* | *(covered via integration/UI)* | `SessionTicket` redacts secret bytes in `Debug` impl. |
| `app/src/collab/session.rs` | Background asynchronous collaboration session actor coordinating CRDT state, transport, and UI channels. | `PeerLifecycleState`, `label`, `ConnectedPeerStatus`, `UiToSessionCmd`, `SessionToUiEvent`, `CollabSessionHandle` | `collab` | `app/src/collab/session/tests.rs` | Runs a dedicated `tokio` runtime on a background thread; communicates via `mpsc`. |
| `app/src/collab/transport.rs` | Concrete `Iroh` QUIC peer-to-peer network transport implementation (`IrohCollabTransport`). | `inspect_connection_route`, `write_framed_message`, `read_framed_message`, `IrohTransport`, `bind_host`, `connect_with_ticket` | `collab` | `app/src/collab/transport/tests.rs` | — |
| `app/src/collab/ui.rs` | Collaborative UI window, peer list badges, session controls, and in-editor remote cursor painting. | `CollabUiState`, `connected_peer_count`, `peers_in_file`, `on_session_started`, `reset_session`, `render_collaboration_window` | `collab`, `highlight` | *(covered via integration/UI)* | — |
| `app/src/compiler.rs` | Incremental Typst document compiler, rasterizer, PDF exporter, and diagnostic collector. | `NativeTypstEngine`, `compile_project`, `export_pdf_bytes_with_config`, `build_byte_to_char_table` | `core`, `engine`, `project`, `world` | `app/src/compiler/tests.rs` | — |
| `app/src/core/diff.rs` | Pure text differencing and UTF-16 code-unit conversion utilities. | `Utf16TextDiff`, `CharSelection`, `caret`, `char_to_utf16_offset`, `utf16_to_char_offset`, `compute_utf16_diff` | *(none)* | `app/src/core/diff/tests.rs`, `app/tests/diff_tests.rs` | Converts between Rust `char` indices (`egui`) and UTF-16 offsets (`yrs`). |
| `app/src/core/engine.rs` | Hardcoded Typst engine release catalog, target matching, and pure-Rust SHA-256 integrity checks. | `BUILTIN_TYPST_VERSION`, `EngineArchiveKind`, `EngineReleaseAsset`, `TypstEngineRelease`, `AVAILABLE_TYPST_RELEASES`, `is_builtin_version` | *(none)* | *(covered via integration/UI)* | — |
| `app/src/core/file_kind.rs` | File type identification and classification based on file extensions. | `FileKind`, `from_path`, `is_editable_text`, `icon`, `label` | *(none)* | `app/src/core/file_kind/tests.rs`, `app/tests/project_tests.rs` | — |
| `app/src/core/i18n.rs` | Cross-platform locale normalization, system language detection, and native display names. | `normalize_locale_tag`, `resolve_locale`, `detect_system_locale`, `locale_native_name`, `AppSavedState` | *(none)* | *(covered via integration/UI)* | — |
| `app/src/core/mod.rs` | Core domain models, algorithms, and data types. | *(re-exports / entry)* | *(none)* | *(covered via integration/UI)* | — |
| `app/src/core/outline.rs` | Pure document outline extraction, AST traversal, and hierarchical numbering. | `SourceResolver`, `CompiledOutlineItem`, `DocumentOutlineItem`, `OutlineTreeNode`, `extract_typst_markup_plain_text`, `extract_typst_ast_outline` | `core` | `app/src/core/outline/tests.rs`, `app/tests/outline_tests.rs` | Uses `SourceResolver` trait to stay independent of filesystem I/O. |
| `app/src/core/pdf.rs` | PDF export configuration, PDF versions, and PDF/A archival standards. | `PdfVersionChoice`, `ALL`, `label`, `short_label`, `to_standard`, `PdfArchivalStandard` | *(none)* | `app/src/core/pdf/tests.rs`, `app/tests/pdf_config_tests.rs` | — |
| `app/src/core/settings.rs` | IDE user preferences, configuration defaults, and settings dialog state. | `SettingsTab`, `ThemePreference`, `ALL`, `resolve_dark_mode`, `IdeSettings`, `sanitized` | `core` | `app/src/core/settings/tests.rs` | — |
| `app/src/engine/manager/archive.rs` | SHA-256 verification, release archive extraction, and cross-platform engine/package cache paths. | `verify_and_extract_archive`, `default_engines_cache_dir`, `default_typst_package_cache_dir` | `core` | *(covered via integration/UI)* | — |
| `app/src/engine/manager.rs` | Cross-platform Typst engine registry, async HTTPS downloader, and cache manager. | `EngineInstallState`, `EngineDownloadEvent`, `EngineManager`, `with_paths`, `engines_dir`, `package_cache_dir` | `compiler`, `core`, `engine`, `project`, `world` | *(covered via integration/UI)* | — |
| `app/src/engine/mod.rs` | Version-agnostic Typst engine abstraction, external worker bridge, and download manager. | *(re-exports / entry)* | *(none)* | *(covered via integration/UI)* | — |
| `app/src/engine/types.rs` | Version-agnostic compiler trait, plain Rust boundary types, and external worker DTOs. | `DiagnosticItem`, `PreviewCharBox`, `RenderedPageData`, `RenderedPageTexture`, `CompileSnapshot`, `EngineMetadata` | `core`, `project` | *(covered via integration/UI)* | — |
| `app/src/engine/worker/cli_parse.rs` | Isolated workspace staging, PNG page decoding, and CLI stderr/query parsing helpers. | `TempDirGuard`, `path`, `StagedWorkspace`, `prepare`, `normalize_rel_pathbuf`, `decode_rendered_png_pages` | `core`, `engine`, `project` | *(covered via integration/UI)* | — |
| `app/src/engine/worker.rs` | External Typst compiler process bridge supporting official CLI binaries and JSON-stdio workers. | `WorkerMode`, `ExternalWorkerEngine`, `with_json_stdio` | `core`, `engine`, `project` | *(covered via integration/UI)* | — |
| `app/src/highlight.rs` | Syntax highlighting engine mapping Typst AST tokens to styled `egui::text::LayoutJob` spans. | `highlight_typst_code` | `ui` | *(covered via integration/UI)* | — |
| `app/src/lib.rs` | Library root for `azo-type`, exposing domain, platform, collaboration, compiler, and UI modules. | `parse_cli_args_from` | *(none)* | `app/tests/cli_tests.rs` | — |
| `app/src/main.rs` | Binary entrypoint launching the native `eframe` desktop window and parsing CLI arguments. | *(re-exports / entry)* | *(none)* | `app/tests/cli_tests.rs` | — |
| `app/src/platform/dialogs.rs` | Platform-native async file and folder dialogs using `rfd`. | `block_on_or_spin`, `spawn_open_folder_dialog`, `spawn_save_file_dialog` | *(none)* | *(covered via integration/UI)* | Only module using `unsafe` (`block_on_or_spin`); runs `rfd` on background threads. |
| `app/src/platform/icon.rs` | Application icon loader, PNG image decoding, and desktop viewport icon configuration. | `APP_ID`, `app_window_icon`, `configure_viewport_builder`, `load_icon_from_png_bytes`, `LINUX_ICONS`, `ensure_linux_desktop_integration_to_dir` | *(none)* | *(covered via integration/UI)* | — |
| `app/src/platform/menu.rs` | Platform-specific native OS application menu integration. | `MenuAction`, `MacOsMenuBar`, `init`, `poll_action` | *(none)* | *(covered via integration/UI)* | Native `muda` menu bar active only under `#[cfg(target_os = "macos")]`. |
| `app/src/platform/mod.rs` | Platform-specific integrations and OS services. | *(re-exports / entry)* | *(none)* | *(covered via integration/UI)* | — |
| `app/src/platform/theme.rs` | Cross-platform OS dark/light theme detection (`System-Default`) with safe Dark fallback. | `resolve_effective_dark_mode`, `detect_system_dark_mode`, `parse_portal_color_scheme`, `parse_gsettings_color_scheme`, `parse_gtk_theme_name_is_dark`, `parse_gtk_settings_ini` | `core` | *(covered via integration/UI)* | — |
| `app/src/project/buffer.rs` | Open buffer and file tree node types for workspace and in-memory documents. | `FsTreeNode`, `OpenBuffer`, `load_from_disk`, `save_to_disk`, `line_count` | `core` | *(covered via integration/UI)* | — |
| `app/src/project.rs` | Workspace management (`DiskProject`), on-disk and in-memory projects, file tree scanning, and buffer caching. | `DiskProject`, `blank`, `has_disk_backing`, `open_folder`, `rescan_tree`, `collect_typ_files` | `core` | `app/tests/project_tests.rs` | — |
| `app/src/services/mod.rs` | Application background services and runners. | *(re-exports / entry)* | *(none)* | *(covered via integration/UI)* | — |
| `app/src/services/pdf_export.rs` | PDF compilation and asynchronous OS save-dialog workflow orchestration. | `PdfExportFlowOutcome`, `run_pdf_export_flow` | `core`, `engine`, `platform`, `project` | *(covered via integration/UI)* | — |
| `app/src/services/persistence.rs` | Persistent application preferences, language, and preview zoom backed by `eframe::Storage`. | `SETTINGS_STORAGE_KEY`, `PREVIEW_ZOOM_STORAGE_KEY`, `PersistedAppState`, `load_persisted_state`, `save_persisted_state`, `save_persisted_parts` | `core` | *(covered via integration/UI)* | — |
| `app/src/services/screenshot.rs` | Headless screenshot capture automation for tests and CI documentation. | `handle_screenshot_lifecycle` | *(none)* | *(covered via integration/UI)* | — |
| `app/src/ui/components/editor.rs` | Multiline text editor pane featuring syntax highlighting, gutter line numbers, wrap glyphs, and remote peer cursors. | `EditorPaneOutput`, `EditorPaneParams`, `render_editor_pane` | `collab`, `core`, `highlight`, `project`, `ui` | *(covered via integration/UI)* | — |
| `app/src/ui/components/editor_actions.rs` | Context menu and edit action handlers (Undo, Redo, Cut, Copy, Paste, Format Snippets) for the text editor. | `render_editor_context_menu`, `apply_editor_context_action` | `core`, `platform`, `ui` | *(covered via integration/UI)* | — |
| `app/src/ui/components/menu_bar.rs` | Desktop menu bar rendered inside the application window on Linux and Windows. | `EditorContextAction`, `MenuBarOutput`, `MenuBarParams`, `render_desktop_menu_bar` | `core`, `platform`, `ui` | *(covered via integration/UI)* | — |
| `app/src/ui/components/mod.rs` | UI reusable panels, controls, and components. | *(re-exports / entry)* | *(none)* | *(covered via integration/UI)* | — |
| `app/src/ui/components/preview.rs` | Live document page canvas, zoom controls, interactive text selection, and compiler diagnostics banner. | `PreviewPaneOutput`, `PreviewPaneParams`, `render_preview_pane` | `compiler`, `core`, `ui` | *(covered via integration/UI)* | — |
| `app/src/ui/components/preview_selection.rs` | Preview page coordinate mapping, glyph hit-testing, drag selection, and clipboard text extraction. | `CLICK_HIT_THRESHOLD_PT`, `DRAG_START_THRESHOLD_PT`, `PreviewSelectionEndpoint`, `PreviewSelectionState`, `is_empty`, `clear` | `compiler`, `core` | *(covered via integration/UI)* | — |
| `app/src/ui/components/sidebar.rs` | Collapsible sidebar displaying the disk project tree and the document outline. | `SidebarOutput`, `SidebarParams`, `render_sidebar` | `collab`, `core`, `project`, `ui` | *(covered via integration/UI)* | — |
| `app/src/ui/components/split_divider.rs` | Resizable vertical split divider between the code editor and live preview panes. | `render_vertical_split_divider` | `ui` | *(covered via integration/UI)* | — |
| `app/src/ui/components/status_bar.rs` | Bottom status bar indicating compilation status, error counts, P2P peers, and active buffer stats. | `StatusBarBufferInfo`, `StatusBarParams`, `StatusBarOutput`, `render_status_bar` | `ui` | *(covered via integration/UI)* | — |
| `app/src/ui/dialogs/export_pdf.rs` | Export PDF sub-window dialog for selecting PDF versions and PDF/A standards. | `render_export_pdf_dialog` | `core`, `ui` | *(covered via integration/UI)* | — |
| `app/src/ui/dialogs/file_modals.rs` | Modal dialogs for creating new files and renaming existing files on disk. | `render_new_file_modal`, `render_rename_file_modal` | `ui` | *(covered via integration/UI)* | — |
| `app/src/ui/dialogs/mod.rs` | Secondary OS sub-windows and modal dialogs. | *(re-exports / entry)* | *(none)* | *(covered via integration/UI)* | — |
| `app/src/ui/dialogs/settings.rs` | Settings modal dialog (`File > Settings`) managing editor, appearance, and compilation preferences. | `SettingsModalOutput`, `render_settings_dialog` | `core`, `engine`, `ui` | *(covered via integration/UI)* | — |
| `app/src/ui/mod.rs` | UI presentation layer, components, dialogs, and styling. | *(re-exports / entry)* | *(none)* | *(covered via integration/UI)* | — |
| `app/src/ui/theme.rs` | UI Design System tokens, ThemePalette, and egui Visuals styling. | `DESIGN_SYSTEM_CSS_TOKENS`, `ThemePalette`, `DARK`, `LIGHT`, `for_mode`, `apply_theme` | *(none)* | *(covered via integration/UI)* | — |
| `app/src/world.rs` | Implementation of `typst::World` (`IdeWorld`) providing fonts, packages, and file sources to the compiler. | `ReqwestDownloader`, `SharedWorldResources`, `font_family_count`, `EditorWorld`, `display_path_for_id` | `project` | *(covered via integration/UI)* | — |

## 3. Companion Unit Test Modules (`app/src/**/tests.rs`)

| Test Module Path | Purpose (`//!` Summary) |
| :--- | :--- |
| `app/src/collab/awareness/tests.rs` | Unit tests for peer awareness state encoding, decoding, and synchronization. |
| `app/src/collab/doc/tests.rs` | Unit tests for Yrs CRDT convergence, sticky cursors, path collisions, and local undo. |
| `app/src/collab/session/tests.rs` | Unit tests for end-to-end multi-file P2P session synchronization over Iroh. |
| `app/src/collab/transport/tests.rs` | Unit tests for session ticket encoding, redaction, and Iroh QUIC handshake. |
| `app/src/compiler/tests.rs` | Unit tests for the Typst compiler, package fetching, and PDF export pipeline. |
| `app/src/core/diff/tests.rs` | Unit tests for UTF-16 text differencing, emoji offsets, and collision path formatting. |
| `app/src/core/engine/tests.rs` | Unit tests for the hardcoded Typst release catalog and pure-Rust SHA-256 verification. |
| `app/src/core/file_kind/tests.rs` | Unit tests for file kind extension classification and text editability. |
| `app/src/core/i18n/tests.rs` | Unit tests for locale normalization, system language detection, and native display names. |
| `app/src/core/outline/tests.rs` | Unit tests for Typst AST heading extraction and outline hierarchy construction. |
| `app/src/core/pdf/tests.rs` | Unit tests for PDF export version choices and archival standard validation. |
| `app/src/core/settings/tests.rs` | Unit tests for default IDE user preferences and settings initialization. |
| `app/src/engine/manager/tests.rs` | Unit tests for [`EngineManager`] resolution, fallback behavior, and SHA-256 verification. |
| `app/src/engine/types/tests.rs` | Unit tests for version-agnostic engine DTO serialization and RGBA/glyph roundtrips. |
| `app/src/engine/worker/tests.rs` | Unit tests for the external engine worker bridge (`cli` parsers, staging, and JSON-stdio protocol). |
| `app/src/platform/icon/tests.rs` | Unit tests for application icon decoding, caching, and viewport builder configuration. |
| `app/src/platform/theme/tests.rs` | Unit tests for OS theme detection parsers and System-Default fallback behavior. |
| `app/src/services/persistence/tests.rs` | Unit tests for `eframe::Storage` settings persistence and reload behavior. |
| `app/src/ui/components/preview_selection/tests.rs` | Unit tests for preview coordinate mapping, hit-testing, multi-file selection, and text extraction. |

## 4. Integration & Characterization Suites (`app/tests/`)

| Suite Path | Tested Domain / Subsystem | Command |
| :--- | :--- | :--- |
| `app/tests/cli_tests.rs` | CLI argument parsing (`--screenshot`, folder paths) | `cargo test --manifest-path app/Cargo.toml --test cli_tests` |
| `app/tests/diff_tests.rs` | UTF-16 diffing, astral emoji offsets, path collisions | `cargo test --manifest-path app/Cargo.toml --test diff_tests` |
| `app/tests/outline_tests.rs` | Typst AST, BibTeX, and Markdown outline extraction | `cargo test --manifest-path app/Cargo.toml --test outline_tests` |
| `app/tests/pdf_config_tests.rs` | PDF versions (1.4–2.0) and PDF/A standard validation | `cargo test --manifest-path app/Cargo.toml --test pdf_config_tests` |
| `app/tests/persistence_tests.rs` | CRDT snapshot saving, loading, and path traversal safety | `cargo test --manifest-path app/Cargo.toml --test persistence_tests` |
| `app/tests/project_tests.rs` | DiskProject file CRUD, buffers, dirty flags, entrypoints | `cargo test --manifest-path app/Cargo.toml --test project_tests` |
