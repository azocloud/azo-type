# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.0.1](https://github.com/azocloud/azo-type/compare/v0.0.0...v0.0.1) - 2026-11-14

### Build

- *(license)* configure cargo-deny for GPL-3.0-or-later compatibility

## [0.0.0](https://github.com/azocloud/azo-type/releases/tag/v0.0.0) - 2026-11-14

### Added

- *(app)* wire background language checking, underlines, and actions into app
- *(ui)* add editor underlines, spell settings tab, and auto-correction panel
- *(language-check)* add offline spell/grammar worker, downloader, and tests
- *(platform)* add Hunspell dictionary file picker with companion auto-matching
- *(core)* add Typst prose masking, dictionary catalog, and spellcheck settings
- *(export)* emit detailed PDF export diagnostics and progress logs to console tab
- *(ui)* enhance bottom panel warnings & errors with sorting and collapsible cards
- *(ui)* add collapsible multi-tab content bottom panel with top-right toggle button
- *(ui)* add resizable horizontal split divider between sidebar tree and outline
- *(ui)* display live download progress bar with speed, elapsed time, and ETA in settings
- *(ui)* wire versioned typst selector, status bar indicator, and translations into settings and app
- *(engine)* decouple typst compiler behind TypstEngine trait with external worker bridge and download manager
- *(core)* add hardcoded typst release catalog, sha256 verifier, and version setting
- *(services)* persist IDE settings, zoom, and language via eframe storage
- *(core)* derive serde persistence and sanitization on IdeSettings and PreviewZoom
- *(platform)* add ThemePreference and cross-platform OS theme detection
- *(ui)* add preview zoom dropdown, pan mode, quick toggles, and clean menu bar
- *(core)* add preview interaction settings and PreviewZoom state
- *(ui)* add interactive preview selection and unified multi-file editor sync
- *(compiler)* extract glyph-level PreviewCharBox source mapping
- *(app)* add language persistence and runtime switcher in menu bar and settings
- *(i18n)* setup rust-i18n, sys-locale, and core i18n module
- *(project)* start with blank in-memory project instead of sample project
- *(platform)* add automated linux desktop integration and install script for wayland
- *(ui)* configure application icon and app id across main window and all os sub-windows
- *(platform)* implement application window icon loader and viewport helper
- *(checks)* add unified check runner, structural linter, and auto-generated PROJECT_MAP.md
- added wrapped line support and start for the outliner (but broken)
- changed ui style
- remove redundant UI menu bars and made sub windows actual os windows
- add support for p2p collaborating on the same project
- remove web support and added menubars

### Build

- *(workspace)* add root Cargo workspace and move Cargo.lock to repo root
- *(macos)* add Sonoma minimum version and metadata keys to Info.plist
- *(deps)* add egui_kittest dev-dependency for headless UI testing
- *(deps)* add spellbook, nlprule, lingua, encoding_rs, and flate2
- *(app)* configure windows pe icon resources, packaging metadata, and platform assets
- *(cargo)* rename crate to azo-type and update crate references

### Changed

- *(app)* expose headless render entrypoint and add app + full UI integration tests
- *(ui)* extract pure editor/action helpers and add unit + egui_kittest tests
- *(ui)* separate dialog bodies/logic and add unit + egui_kittest tests
- *(collab,ui)* decouple collab UI actions from session handle and add unit + egui_kittest tests
- *(tests)* centralize unit tests into app/src/tests/
- *(language-check)* split tokenization from engine and clean core doc
- *(core)* load Typst release catalog from embedded JSON
- *(app)* extract keyboard shortcuts and project file actions from orchestrator
- *(ui)* remove bottom close buttons from Settings and P2P windows
- *(ui)* replace editor font size slider with DragValue in Settings
- *(ui)* clean up Settings dialog with theme dropdown and numeric debounce input
- *(ui)* extract user-facing strings to i18n t! macro with German translation
- *(ui)* shorten window and dialog titles to clean names
- *(project)* update sample project branding to Azo Type
- *(ui)* update app coordinator and window titles to Azo Type
- *(ui)* extract editor context menu and action handlers into editor_actions
- *(collab,project)* extract collab wire protocol and project sample generator
- *(ui)* extract menu bar, status bar, sidebar, editor, and preview components
- *(ui)* extract theme styling and modal dialogs (export_pdf, file_modals, settings)
- *(platform)* extract native menu and async dialogs into platform layer
- *(core)* extract pure domain modules (diff, file_kind, outline, pdf, settings)

### Documentation

- *(agent)* update structural checks and docs for app/src/tests layout
- update documentation and agent specifications to Azo Type
- *(agent)* add root AGENTS.md and scoped app/src/core/AGENTS.md
- add //! module docs across app/src and enable missing_docs in core
- update README, document architecture and add comprehensive refactoring log

### Fixed

- *(tests)* gate unix-only imports in worker_tests to silence Windows warnings
- *(tests)* canonicalize saved file path in project_tests
- *(windows)* suppress console window in release builds via windows_subsystem
- *(macos)* declare winit dependency for macOS event loop hook
- *(macos)* fix menu_bar re-export, unused imports, poll borrow, and headless init
- *(tests)* canonicalize temp directory paths in app_tests for Windows and macOS
- *(preview)* hide horizontal scrollbar when pages fit and enforce minimum preview width
- *(export)* remove CLI preview line and show managed compiler version in PDF dialog
- *(engine)* install rustls provider before reqwest downloads, stream progress, and guard against worker panics
- *(ui)* remove duplicate zoom percentage label beside preview dropdown
- *(clippy)* resolve if_same_then_else, collapsible_match, and derivable_impls warnings
- Update .gitignore to not track the temporary P2P files in the
- fixed outline panel to actually show the correct outine content
- typst now loads imports correctly and can export in different pdf formats

### Miscellaneous

- *(app)* reset package version to 0.0.0 and derive user-agent from CARGO_PKG_VERSION
- *(app)* changed description and removed some comments
- *(lint)* add central rustfmt.toml, clippy.toml, and Cargo.toml lint rules

### Other

- Initial version

### Performance

- *(build)* optimize release profile for binary size and execution speed

### Testing

- *(app,ui)* pin locale storage to English in headless tests to prevent locale races
- *(services)* add unit tests for pdf_export, project_actions, screenshot, and language_actions
- *(ui)* add egui_kittest tests for remaining workspace UI components
- *(core,collab,platform,ui)* add unit tests for highlight, theme, shortcuts, protocol, and language host
- *(language-check)* add end-to-end integration tests and update project map
- *(engine)* verify TLS client setup, integrated 0.15.1, CLI worker, and live 0.14.2 compilation
- *(i18n)* add comprehensive integration test suite and update agent documentation
- relocate unit tests out of production files into companion test modules
- add comprehensive characterization test suite and smoke test checklist
