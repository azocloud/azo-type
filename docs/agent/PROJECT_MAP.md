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
| `app/src/app.rs` | Top-level application state orchestrator (`AzoTypeApp`), event loop, and channel dispatcher. | `IoEvent`, `TypstIdeApp`, `AzoTypeApp`, `new_headless`, `set_language`, `dispatch_menu_action` | `collab`, `compiler`, `core`, `engine`, `highlight`, `language_check`, `platform`, `project`, `ui`, `world` | `app/src/tests/app_tests.rs`, `app/tests/ui_tests.rs` | Top-level `eframe::App` state orchestrator; delegates view rendering to `ui/`. |
| `app/src/collab/awareness.rs` | Ephemeral peer presence, active file tracking, and remote cursor awareness synchronization. | `PeerPresence`, `CollabAwareness`, `client_id`, `set_local_presence`, `encode_full_update`, `apply_remote_awareness_bytes` | *(none)* | `app/src/tests/collab/awareness_tests.rs` | — |
| `app/src/collab/cursor_overlay.rs` | In-editor remote peer cursor, selection highlight, and name-tag overlay painting. | `paint_remote_cursors_in_editor` | `collab` | `app/src/tests/collab/cursor_overlay_tests.rs` | — |
| `app/src/collab/doc.rs` | Multi-file Yrs CRDT document container (`CollabProjectDoc`), sticky cursors, and local undo manager. | `LOCAL_ORIGIN`, `REMOTE_ORIGIN`, `FILES_MAP_KEY`, `CollabFileEntry`, `CollabProjectDoc`, `with_client_id` | `core` | `app/src/tests/collab/doc_tests.rs` | — |
| `app/src/collab/mod.rs` | Peer-to-peer real-time collaborative editing subsystem (Yrs CRDT + Iroh QUIC transport). | *(re-exports / entry)* | *(none)* | *(covered via integration/UI)* | — |
| `app/src/collab/persistence.rs` | Atomic disk persistence for CRDT state snapshots (`.typst-collab/state.yrs`) and workspace reconciliation. | `PersistedFileIdMap`, `CollabPersistence`, `snapshot_path`, `file_ids_path`, `load_and_reconcile_from_disk`, `save_snapshot` | `collab`, `project` | `app/tests/persistence_tests.rs` | — |
| `app/src/collab/protocol.rs` | Wire protocol definitions, tickets, and abstract transport trait for P2P collaboration. | `COLLAB_ALPN`, `COLLAB_PROTOCOL_VERSION`, `MAX_WIRE_FRAME_BYTES`, `TICKET_PREFIX`, `ConnectionRoute`, `badge_label` | *(none)* | `app/src/tests/collab/protocol_tests.rs` | `SessionTicket` redacts secret bytes in `Debug` impl. |
| `app/src/collab/session.rs` | Background asynchronous collaboration session actor coordinating CRDT state, transport, and UI channels. | `PeerLifecycleState`, `label`, `ConnectedPeerStatus`, `UiToSessionCmd`, `SessionToUiEvent`, `CollabSessionHandle` | `collab` | `app/src/tests/collab/session_tests.rs` | Runs a dedicated `tokio` runtime on a background thread; communicates via `mpsc`. |
| `app/src/collab/transport.rs` | Concrete `Iroh` QUIC peer-to-peer network transport implementation (`IrohCollabTransport`). | `inspect_connection_route`, `write_framed_message`, `read_framed_message`, `IrohTransport`, `bind_host`, `connect_with_ticket` | `collab` | `app/src/tests/collab/transport_tests.rs` | — |
| `app/src/collab/ui.rs` | Collaborative UI window, peer list badges, session controls, and in-editor remote cursor painting. | `CollabWindowAction`, `CollabUiState`, `connected_peer_count`, `peers_in_file`, `on_session_started`, `reset_session` | `collab`, `highlight` | `app/src/tests/collab/ui_tests.rs` | — |
| `app/src/compiler.rs` | Incremental Typst document compiler, rasterizer, PDF exporter, and diagnostic collector. | `NativeTypstEngine`, `compile_project`, `export_pdf_bytes_with_config`, `build_byte_to_char_table` | `core`, `engine`, `project`, `world` | `app/src/tests/compiler_tests.rs` | — |
| `app/src/core/diff.rs` | Pure text differencing and UTF-16 code-unit conversion utilities. | `Utf16TextDiff`, `CharSelection`, `caret`, `char_to_utf16_offset`, `utf16_to_char_offset`, `compute_utf16_diff` | *(none)* | `app/src/tests/core/diff_tests.rs`, `app/tests/diff_tests.rs` | Converts between Rust `char` indices (`egui`) and UTF-16 offsets (`yrs`). |
| `app/src/core/engine.rs` | Typst engine release catalog loaded from embedded JSON, target matching, and pure-Rust SHA-256 checks. | `BUILTIN_TYPST_VERSION`, `EngineArchiveKind`, `EngineReleaseAsset`, `TypstEngineRelease`, `available_typst_releases`, `is_builtin_version` | *(none)* | `app/src/tests/core/engine_tests.rs` | — |
| `app/src/core/file_kind.rs` | File type identification and classification based on file extensions. | `FileKind`, `from_path`, `is_editable_text`, `icon`, `label` | *(none)* | `app/src/tests/core/file_kind_tests.rs`, `app/tests/project_tests.rs` | — |
| `app/src/core/i18n.rs` | Cross-platform locale normalization, system language detection, and native display names. | `normalize_locale_tag`, `resolve_locale`, `detect_system_locale`, `locale_native_name`, `AppSavedState` | *(none)* | `app/src/tests/core/i18n_tests.rs` | — |
| `app/src/core/mod.rs` | Core domain models, algorithms, and data types. | *(re-exports / entry)* | *(none)* | *(covered via integration/UI)* | — |
| `app/src/core/outline.rs` | Pure document outline extraction, AST traversal, and hierarchical numbering. | `SourceResolver`, `CompiledOutlineItem`, `DocumentOutlineItem`, `OutlineTreeNode`, `extract_typst_markup_plain_text`, `extract_typst_ast_outline` | `core` | `app/src/tests/core/outline_tests.rs`, `app/tests/outline_tests.rs` | Uses `SourceResolver` trait to stay independent of filesystem I/O. |
| `app/src/core/pdf.rs` | PDF export configuration, PDF versions, and PDF/A archival standards. | `PdfVersionChoice`, `ALL`, `label`, `short_label`, `to_standard`, `PdfArchivalStandard` | *(none)* | `app/src/tests/core/pdf_tests.rs`, `app/tests/pdf_config_tests.rs` | — |
| `app/src/core/settings.rs` | IDE user preferences, configuration defaults, and settings dialog state. | `SettingsTab`, `ThemePreference`, `ALL`, `resolve_dark_mode`, `IdeSettings`, `sanitized` | `core` | `app/src/tests/core/settings_tests.rs` | — |
| `app/src/core/spellcheck.rs` | Pure spellcheck/grammar domain types, embedded dictionary catalog, Hunspell encoding | `TIER1_LANGUAGES`, `DictionaryFileRole`, `AssetCompression`, `DictionaryFileAsset`, `Tier1LanguagePack`, `total_size_bytes` | `core` | `app/src/tests/core/spellcheck_tests.rs`, `app/tests/language_check_tests.rs` | — |
| `app/src/core/typst_prose.rs` | Pure Typst AST prose extraction, `#set text(lang: ...)` detection, and byte-preserving masking. | `TypstProseMask`, `ProseParagraph`, `normalize_language_code`, `normalize_full_language_tag`, `detect_typst_document_language`, `extract_typst_prose_mask` | *(none)* | `app/src/tests/core/typst_prose_tests.rs`, `app/tests/language_check_tests.rs` | — |
| `app/src/engine/manager/archive.rs` | SHA-256 verification, release archive extraction, TLS provider setup, streaming HTTPS downloads, | `ensure_tls_crypto_provider`, `DownloadStage`, `DownloadProgress`, `elapsed_secs`, `fraction`, `speed_bytes_per_sec` | `core`, `engine` | *(covered via integration/UI)* | — |
| `app/src/engine/manager.rs` | Cross-platform Typst engine registry, async HTTPS downloader, and cache manager. | `EngineInstallState`, `EngineDownloadEvent`, `EngineManager`, `with_paths`, `engines_dir`, `package_cache_dir` | `compiler`, `core`, `engine`, `project`, `world` | `app/src/tests/engine/manager_tests.rs` | — |
| `app/src/engine/mod.rs` | Version-agnostic Typst engine abstraction, external worker bridge, and download manager. | *(re-exports / entry)* | *(none)* | *(covered via integration/UI)* | — |
| `app/src/engine/types.rs` | Version-agnostic compiler trait, plain Rust boundary types, and external worker DTOs. | `DiagnosticItem`, `PreviewCharBox`, `RenderedPageData`, `RenderedPageTexture`, `CompileSnapshot`, `EngineMetadata` | `core`, `project` | `app/src/tests/engine/types_tests.rs` | — |
| `app/src/engine/worker/cli_parse.rs` | Isolated workspace staging, PNG page decoding, and CLI stderr/query parsing helpers. | `TempDirGuard`, `path`, `StagedWorkspace`, `prepare`, `normalize_rel_pathbuf`, `decode_rendered_png_pages` | `core`, `engine`, `project` | *(covered via integration/UI)* | — |
| `app/src/engine/worker.rs` | External Typst compiler process bridge supporting official CLI binaries and JSON-stdio workers. | `WorkerMode`, `ExternalWorkerEngine`, `with_json_stdio` | `core`, `engine`, `project` | `app/src/tests/engine/worker_tests.rs` | — |
| `app/src/highlight.rs` | Syntax highlighting engine mapping Typst AST tokens to styled `egui::text::LayoutJob` spans. | `highlight_typst_code` | `ui` | `app/src/tests/highlight_tests.rs` | — |
| `app/src/language_check/downloader.rs` | On-demand Tier-1 Hunspell dictionary and `nlprule` grammar model downloader and cache manager. | `DictionaryPackStatus`, `DictionaryDownloadProgress`, `fraction`, `elapsed_secs`, `InstalledTier1Paths`, `DictionaryDownloadOutcome` | `core`, `engine` | `app/src/tests/language_check/downloader_tests.rs` | — |
| `app/src/language_check/engine.rs` | Offline spellcheck (`spellbook`), grammar (`nlprule`), and language detection (`lingua`) | `LanguageSource`, `DocumentCheckResult`, `LanguageCheckEngine`, `invalidate_dictionaries`, `resolve_language`, `check_document` | `core`, `language_check` | `app/src/tests/language_check/engine_tests.rs`, `app/tests/language_check_tests.rs` | — |
| `app/src/language_check/host.rs` | Background worker actor, debounce scheduler, and UI channel bridge for offline spell & grammar checking. | `LanguageCheckHost`, `on_text_edited`, `check_now`, `clear_results`, `invalidate_dictionaries`, `download_pack` | `core`, `language_check`, `project` | `app/src/tests/language_check/host_tests.rs` | — |
| `app/src/language_check/mod.rs` | Offline spellchecking (`spellbook`), grammar checking (`nlprule`), language detection | *(re-exports / entry)* | *(none)* | *(covered via integration/UI)* | — |
| `app/src/language_check/tokenize.rs` | Word tokenization and UTF-8 byte-to-char/line-column source offset mapping. | `tokenize_prose_words`, `SourceOffsetMapper`, `map_range` | *(none)* | *(covered via integration/UI)* | — |
| `app/src/lib.rs` | Library root for `azo-type`, exposing domain, platform, collaboration, compiler, and UI modules. | `parse_cli_args_from` | *(none)* | `app/tests/cli_tests.rs` | — |
| `app/src/main.rs` | Binary entrypoint launching the native `eframe` desktop window and parsing CLI arguments. | *(re-exports / entry)* | *(none)* | `app/tests/cli_tests.rs` | — |
| `app/src/platform/dialogs.rs` | Platform-native async file and folder dialogs using `rfd`. | `block_on_or_spin`, `spawn_open_folder_dialog`, `spawn_save_file_dialog`, `spawn_open_dictionary_dialog`, `resolve_dictionary_pair` | `core` | `app/src/tests/platform/dialogs_tests.rs` | Only module using `unsafe` (`block_on_or_spin`); runs `rfd` on background threads. |
| `app/src/platform/icon.rs` | Application icon loader, PNG image decoding, and desktop viewport icon configuration. | `APP_ID`, `app_window_icon`, `configure_viewport_builder`, `load_icon_from_png_bytes`, `LINUX_ICONS`, `ensure_linux_desktop_integration_to_dir` | *(none)* | `app/src/tests/platform/icon_tests.rs` | — |
| `app/src/platform/menu.rs` | Platform-specific native OS application menu integration. | `MenuAction`, `MacOsMenuBar`, `init`, `poll_action` | *(none)* | *(covered via integration/UI)* | Native `muda` menu bar active only under `#[cfg(target_os = "macos")]`. |
| `app/src/platform/mod.rs` | Platform-specific integrations and OS services. | *(re-exports / entry)* | *(none)* | *(covered via integration/UI)* | — |
| `app/src/platform/shortcuts.rs` | Keyboard shortcut polling for desktop editor, panels, and collaboration undo/redo. | `KeyboardShortcutOutcome`, `poll_keyboard_shortcuts` | `platform` | `app/src/tests/platform/shortcuts_tests.rs` | — |
| `app/src/platform/theme.rs` | Cross-platform OS dark/light theme detection (`System-Default`) with safe Dark fallback. | `resolve_effective_dark_mode`, `detect_system_dark_mode`, `parse_portal_color_scheme`, `parse_gsettings_color_scheme`, `parse_gtk_theme_name_is_dark`, `parse_gtk_settings_ini` | `core` | `app/src/tests/platform/theme_tests.rs` | — |
| `app/src/project/buffer.rs` | Open buffer and file tree node types for workspace and in-memory documents. | `FsTreeNode`, `OpenBuffer`, `load_from_disk`, `save_to_disk`, `line_count` | `core` | *(covered via integration/UI)* | — |
| `app/src/project.rs` | Workspace management (`DiskProject`), on-disk and in-memory projects, file tree scanning, and buffer caching. | `DiskProject`, `blank`, `has_disk_backing`, `open_folder`, `rescan_tree`, `collect_typ_files` | `core` | `app/tests/project_tests.rs` | — |
| `app/src/services/language_actions.rs` | Orchestration helpers wiring Settings/Bottom-Panel spell & grammar actions to the app state. | `apply_language_suggestion`, `register_user_dictionary`, `apply_picked_dictionary_result`, `handle_settings_language_actions`, `handle_bottom_panel_language_actions` | `core`, `language_check`, `project`, `ui` | `app/src/tests/services/language_actions_tests.rs` | — |
| `app/src/services/mod.rs` | Application background services and runners. | *(re-exports / entry)* | *(none)* | *(covered via integration/UI)* | — |
| `app/src/services/pdf_export.rs` | PDF compilation and asynchronous OS save-dialog workflow orchestration. | `PdfExportFlowOutcome`, `CompiledPdfExport`, `compile_pdf_for_export`, `run_pdf_export_flow` | `core`, `engine`, `platform`, `project` | `app/src/tests/services/pdf_export_tests.rs` | — |
| `app/src/services/persistence.rs` | Persistent application preferences, language, and preview zoom backed by `eframe::Storage`. | `SETTINGS_STORAGE_KEY`, `PREVIEW_ZOOM_STORAGE_KEY`, `PersistedAppState`, `load_persisted_state`, `save_persisted_state`, `save_persisted_parts` | `core` | `app/src/tests/services/persistence_tests.rs` | — |
| `app/src/services/project_actions.rs` | Project file creation, renaming, and deletion workflows synchronized with collaboration state. | `open_and_clamp_source`, `initial_project_status_notice`, `create_project_file`, `rename_project_file`, `delete_project_file`, `save_active_or_prompt` | `collab`, `platform`, `project` | `app/src/tests/services/project_actions_tests.rs` | — |
| `app/src/services/screenshot.rs` | Headless screenshot capture automation for tests and CI documentation. | `handle_screenshot_lifecycle` | *(none)* | `app/src/tests/services/screenshot_tests.rs` | — |
| `app/src/ui/components/bottom_panel.rs` | Collapsible multi-tab bottom workspace panel rendered beneath the editor and preview panes. | `BottomPanelTab`, `ALL`, `label`, `BottomPanelState`, `toggle_visible`, `toggle_diagnostic_expanded` | `core`, `engine`, `language_check`, `project`, `ui` | `app/src/tests/ui/components/bottom_panel_tests.rs` | — |
| `app/src/ui/components/bottom_panel_autocorrect.rs` | Auto-Correction & Language Analysis tab inside the collapsible bottom workspace panel. | `render_autocorrect_tab` | `core`, `language_check`, `ui` | `app/src/tests/ui/components/bottom_panel_autocorrect_tests.rs` | — |
| `app/src/ui/components/editor.rs` | Multiline text editor pane featuring syntax highlighting, gutter line numbers, wrap glyphs, and remote peer cursors. | `EditorPaneOutput`, `EditorPaneParams`, `adjust_editor_font_size`, `compute_gutter_width`, `render_editor_pane` | `collab`, `core`, `highlight`, `project`, `ui` | `app/src/tests/ui/components/editor_tests.rs` | — |
| `app/src/ui/components/editor_actions.rs` | Context menu and edit action handlers (Undo, Redo, Cut, Copy, Paste, Format Snippets) for the text editor. | `render_editor_context_menu`, `char_to_byte_index`, `normalize_char_selection`, `apply_cut_text`, `apply_paste_text`, `apply_wrap_or_insert_text` | `core`, `platform`, `ui` | `app/src/tests/ui/components/editor_actions_tests.rs` | — |
| `app/src/ui/components/editor_diagnostics.rs` | Editor overlay rendering spell, grammar, and typography underlines plus hover tooltips. | `diagnostic_color`, `compute_wavy_points`, `paint_language_diagnostics` | `core`, `ui` | `app/src/tests/ui/components/editor_diagnostics_tests.rs` | — |
| `app/src/ui/components/menu_bar.rs` | Desktop menu bar rendered inside the application window on Linux and Windows. | `EditorContextAction`, `MenuBarOutput`, `MenuBarParams`, `render_desktop_menu_bar` | `core`, `platform`, `ui` | `app/src/tests/ui/components/menu_bar_tests.rs` | — |
| `app/src/ui/components/mod.rs` | UI reusable panels, controls, and components. | *(re-exports / entry)* | *(none)* | *(covered via integration/UI)* | — |
| `app/src/ui/components/preview.rs` | Live document page canvas, zoom controls, interactive text selection, and compiler diagnostics banner. | `PreviewPaneOutput`, `PreviewPaneParams`, `render_preview_pane` | `compiler`, `core`, `ui` | `app/src/tests/ui/components/preview_tests.rs` | — |
| `app/src/ui/components/preview_selection.rs` | Preview page coordinate mapping, glyph hit-testing, drag selection, and clipboard text extraction. | `CLICK_HIT_THRESHOLD_PT`, `DRAG_START_THRESHOLD_PT`, `PreviewSelectionEndpoint`, `PreviewSelectionState`, `is_empty`, `clear` | `compiler`, `core` | `app/src/tests/ui/components/preview_selection_tests.rs` | — |
| `app/src/ui/components/sidebar.rs` | Collapsible sidebar displaying the disk project tree and the document outline. | `SidebarOutput`, `SidebarParams`, `render_sidebar` | `collab`, `core`, `project`, `ui` | `app/src/tests/ui/components/sidebar_tests.rs` | — |
| `app/src/ui/components/split_divider.rs` | Resizable split dividers between the editor/preview panes and sidebar panels. | `SplitGeometry`, `compute_vertical_split`, `compute_horizontal_split`, `render_vertical_split_divider`, `render_horizontal_split_divider` | `ui` | `app/src/tests/ui/components/split_divider_tests.rs` | — |
| `app/src/ui/components/status_bar.rs` | Bottom status bar indicating compilation status, error counts, P2P peers, and active buffer stats. | `StatusBarBufferInfo`, `StatusBarParams`, `StatusBarOutput`, `render_status_bar` | `ui` | `app/src/tests/ui/components/status_bar_tests.rs` | — |
| `app/src/ui/dialogs/export_pdf.rs` | Export PDF sub-window dialog for selecting PDF versions and PDF/A standards. | `apply_pdf_version_choice`, `apply_archival_standard_choice`, `apply_pdf_ua1_toggle`, `standard_requires_tagged`, `render_export_pdf_dialog`, `render_export_pdf_body` | `core`, `ui` | `app/src/tests/ui/dialogs/export_pdf_tests.rs` | — |
| `app/src/ui/dialogs/file_modals.rs` | Modal dialogs for creating new files and renaming existing files on disk. | `try_confirm_new_file`, `try_confirm_rename_file`, `render_new_file_body`, `render_rename_file_body`, `render_new_file_modal`, `render_rename_file_modal` | `ui` | `app/src/tests/ui/dialogs/file_modals_tests.rs` | — |
| `app/src/ui/dialogs/mod.rs` | Secondary OS sub-windows and modal dialogs. | *(re-exports / entry)* | *(none)* | *(covered via integration/UI)* | — |
| `app/src/ui/dialogs/settings.rs` | Settings modal dialog (`File > Settings`) managing editor, appearance, and compilation preferences. | `SettingsModalOutput`, `apply_theme_preference`, `reset_settings_to_defaults`, `render_settings_dialog`, `render_settings_body` | `core`, `engine`, `language_check`, `ui` | `app/src/tests/ui/dialogs/settings_tests.rs` | — |
| `app/src/ui/dialogs/settings_spell.rs` | Settings dialog tab for offline spellchecking, grammar checking, and dictionary management. | `render_spell_and_grammar_tab` | `core`, `language_check`, `ui` | `app/src/tests/ui/dialogs/settings_spell_tests.rs` | — |
| `app/src/ui/mod.rs` | UI presentation layer, components, dialogs, and styling. | *(re-exports / entry)* | *(none)* | *(covered via integration/UI)* | — |
| `app/src/ui/theme.rs` | UI Design System tokens, ThemePalette, and egui Visuals styling. | `DESIGN_SYSTEM_CSS_TOKENS`, `ThemePalette`, `DARK`, `LIGHT`, `for_mode`, `apply_theme` | *(none)* | `app/src/tests/ui/theme_tests.rs` | — |
| `app/src/world.rs` | Implementation of `typst::World` (`IdeWorld`) providing fonts, packages, and file sources to the compiler. | `ReqwestDownloader`, `SharedWorldResources`, `font_family_count`, `EditorWorld`, `display_path_for_id` | `project` | *(covered via integration/UI)* | — |

## 3. Centralized Unit Test Modules (`app/src/tests/`)

| Test Module Path | Purpose (`//!` Summary) |
| :--- | :--- |
| `app/src/tests/app_tests.rs` | Unit tests for `AzoTypeApp` orchestration, menu dispatch, snapshot application, and channel polling. |
| `app/src/tests/collab/awareness_tests.rs` | Unit tests for peer awareness state encoding, decoding, and synchronization. |
| `app/src/tests/collab/cursor_overlay_tests.rs` | Unit tests for painting remote collaboration cursors, selections, and name tags. |
| `app/src/tests/collab/doc_tests.rs` | Unit tests for Yrs CRDT convergence, sticky cursors, path collisions, and local undo. |
| `app/src/tests/collab/protocol_tests.rs` | Unit tests for P2P collaboration wire protocol types, ticket validation, and secret redaction. |
| `app/src/tests/collab/session_tests.rs` | Unit tests for end-to-end multi-file P2P session synchronization over Iroh. |
| `app/src/tests/collab/transport_tests.rs` | Unit tests for session ticket encoding, redaction, and Iroh QUIC handshake. |
| `app/src/tests/collab/ui_tests.rs` | Unit and `egui_kittest` UI tests for the P2P collaboration window state and controls. |
| `app/src/tests/compiler_tests.rs` | Unit tests for the Typst compiler, package fetching, and PDF export pipeline. |
| `app/src/tests/core/diff_tests.rs` | Unit tests for UTF-16 text differencing, emoji offsets, and collision path formatting. |
| `app/src/tests/core/engine_tests.rs` | Unit tests for the hardcoded Typst release catalog and pure-Rust SHA-256 verification. |
| `app/src/tests/core/file_kind_tests.rs` | Unit tests for file kind extension classification and text editability. |
| `app/src/tests/core/i18n_tests.rs` | Unit tests for locale normalization, system language detection, and native display names. |
| `app/src/tests/core/outline_tests.rs` | Unit tests for Typst AST heading extraction and outline hierarchy construction. |
| `app/src/tests/core/pdf_tests.rs` | Unit tests for PDF export version choices and archival standard validation. |
| `app/src/tests/core/settings_tests.rs` | Unit tests for default IDE user preferences and settings initialization. |
| `app/src/tests/core/spellcheck_tests.rs` | Unit tests for the embedded dictionary manifest, Hunspell encoding transcoding, and pure rules. |
| `app/src/tests/core/typst_prose_tests.rs` | Unit tests for Typst AST prose masking, language directive detection, and span conversion. |
| `app/src/tests/engine/manager_tests.rs` | Unit tests for [`EngineManager`] resolution, fallback behavior, and SHA-256 verification. |
| `app/src/tests/engine/types_tests.rs` | Unit tests for version-agnostic engine DTO serialization and RGBA/glyph roundtrips. |
| `app/src/tests/engine/worker_tests.rs` | Unit tests for the external engine worker bridge (`cli` parsers, staging, and JSON-stdio protocol). |
| `app/src/tests/highlight_tests.rs` | Unit tests for Typst syntax highlighting into styled `LayoutJob` spans. |
| `app/src/tests/language_check/downloader_tests.rs` | Unit tests for the Tier-1 dictionary cache and download status tracking. |
| `app/src/tests/language_check/engine_tests.rs` | Unit tests for offline spell/grammar engine tokenization, language detection, and caching. |
| `app/src/tests/language_check/host_tests.rs` | Unit tests for `LanguageCheckHost` background checking, debouncing, and poll transitions. |
| `app/src/tests/platform/dialogs_tests.rs` | Unit tests for Hunspell dictionary companion file resolution. |
| `app/src/tests/platform/icon_tests.rs` | Unit tests for application icon decoding, caching, and viewport builder configuration. |
| `app/src/tests/platform/shortcuts_tests.rs` | Unit tests for global keyboard shortcut polling across editor and collab modes. |
| `app/src/tests/platform/theme_tests.rs` | Unit tests for OS theme detection parsers and System-Default fallback behavior. |
| `app/src/tests/services/language_actions_tests.rs` | Unit tests for spell/grammar suggestion application and Tier-2 dictionary registration. |
| `app/src/tests/services/pdf_export_tests.rs` | Unit tests for PDF export compilation preparation and failure diagnostics. |
| `app/src/tests/services/persistence_tests.rs` | Unit tests for `eframe::Storage` settings persistence and reload behavior. |
| `app/src/tests/services/project_actions_tests.rs` | Unit tests for project file actions, folder switching, and collaboration event routing. |
| `app/src/tests/services/screenshot_tests.rs` | Unit tests for headless screenshot capture lifecycle management. |
| `app/src/tests/ui/components/bottom_panel_autocorrect_tests.rs` | Headless `egui_kittest` tests for the bottom panel Auto-Correction tab. |
| `app/src/tests/ui/components/bottom_panel_tests.rs` | Unit tests for the collapsible multi-tab bottom workspace panel state and diagnostics sorting. |
| `app/src/tests/ui/components/editor_actions_tests.rs` | Pure unit and headless `egui_kittest` tests for editor buffer actions and shortcuts. |
| `app/src/tests/ui/components/editor_diagnostics_tests.rs` | Unit tests for editor language diagnostic underline geometry and color mapping. |
| `app/src/tests/ui/components/editor_tests.rs` | Pure unit and headless `egui_kittest` tests for the editor pane component. |
| `app/src/tests/ui/components/menu_bar_tests.rs` | Headless `egui_kittest` tests for the in-window desktop menu bar component. |
| `app/src/tests/ui/components/preview_selection_tests.rs` | Unit tests for preview coordinate mapping, hit-testing, multi-file selection, and text extraction. |
| `app/src/tests/ui/components/preview_tests.rs` | Headless `egui_kittest` tests for the live Typst preview pane. |
| `app/src/tests/ui/components/sidebar_tests.rs` | Headless `egui_kittest` tests for the collapsible project sidebar and outline. |
| `app/src/tests/ui/components/split_divider_tests.rs` | Unit tests for split divider pane geometry calculations. |
| `app/src/tests/ui/components/status_bar_tests.rs` | Headless `egui_kittest` tests for the bottom status bar component. |
| `app/src/tests/ui/dialogs/export_pdf_tests.rs` | Pure unit and headless `egui_kittest` tests for the Export PDF dialog. |
| `app/src/tests/ui/dialogs/file_modals_tests.rs` | Pure unit and headless `egui_kittest` tests for New File and Rename File modals. |
| `app/src/tests/ui/dialogs/settings_spell_tests.rs` | Headless `egui_kittest` tests for the Spell & Grammar Settings tab. |
| `app/src/tests/ui/dialogs/settings_tests.rs` | Pure unit and headless `egui_kittest` tests for the Settings dialog. |
| `app/src/tests/ui/theme_tests.rs` | Unit tests for UI Design System palettes and `egui` theme application. |

## 4. Integration & Characterization Suites (`app/tests/`)

| Suite Path | Tested Domain / Subsystem | Command |
| :--- | :--- | :--- |
| `app/tests/cli_tests.rs` | CLI argument parsing (`--screenshot`, folder paths) | `cargo test --manifest-path app/Cargo.toml --test cli_tests` |
| `app/tests/diff_tests.rs` | UTF-16 diffing, astral emoji offsets, path collisions | `cargo test --manifest-path app/Cargo.toml --test diff_tests` |
| `app/tests/i18n_tests.rs` | Locale normalization, switching, and translation keys | `cargo test --manifest-path app/Cargo.toml --test i18n_tests` |
| `app/tests/language_check_tests.rs` | Offline spellchecking, grammar, and suggestion application | `cargo test --manifest-path app/Cargo.toml --test language_check_tests` |
| `app/tests/outline_tests.rs` | Typst AST, BibTeX, and Markdown outline extraction | `cargo test --manifest-path app/Cargo.toml --test outline_tests` |
| `app/tests/pdf_config_tests.rs` | PDF versions (1.4–2.0) and PDF/A standard validation | `cargo test --manifest-path app/Cargo.toml --test pdf_config_tests` |
| `app/tests/persistence_tests.rs` | CRDT snapshot saving, loading, and path traversal safety | `cargo test --manifest-path app/Cargo.toml --test persistence_tests` |
| `app/tests/project_tests.rs` | DiskProject file CRUD, buffers, dirty flags, entrypoints | `cargo test --manifest-path app/Cargo.toml --test project_tests` |
| `app/tests/ui_tests.rs` | End-to-end headless `egui_kittest` cross-panel UI flows | `cargo test --manifest-path app/Cargo.toml --test ui_tests` |
