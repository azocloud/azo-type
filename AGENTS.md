# AGENTS.md — AI Coding Agent Onboarding & Project Rules

`azo-type` (Azo Type) is a native cross-platform desktop IDE for authoring [Typst](https://typst.app/) documents with live incremental preview, AST outline navigation, and zero-server P2P collaborative editing. It is built for technical writers, researchers, and teams who want local-first speed combined with real-time collaboration. The desktop application lives in `app/` and uses Rust (2021 edition), `egui`/`eframe` 0.31, `typst` 0.15.1, `yrs` 0.28 (Yjs CRDT), and `iroh` 1.4.1 (QUIC P2P transport).

## 1. Essential Commands (Run from Repo Root)
- **Full Check (Mandatory Gate):** `./scripts/check.sh` (runs fmt, clippy `-D warnings`, all tests, and structural/map checks)
- **Build (Debug):** `cargo build --manifest-path app/Cargo.toml`
- **Run Application:** `cargo run --manifest-path app/Cargo.toml`
- **Run Single Integration Test:** `cargo test --manifest-path app/Cargo.toml --test outline_tests`
- **Run Single Unit Test:** `cargo test --manifest-path app/Cargo.toml --lib core::diff::tests::test_compute_diff_deletions`
- **Run All Tests:** `cargo test --manifest-path app/Cargo.toml`
- **Format Check:** `cargo fmt --check --manifest-path app/Cargo.toml`
- **Linter (Strict):** `cargo clippy --all-targets --manifest-path app/Cargo.toml -- -D warnings`
- **Regenerate Project Map:** `python3 scripts/verify_structural_rules.py --write-map`
- *Platform Notes:* Linux requires `fontconfig` dev headers (`libfontconfig1-dev`) and X11/Wayland libs; macOS uses `muda` for native menus (`#[cfg(target_os = "macos")]`); headless screenshot mode runs via `cargo run --manifest-path app/Cargo.toml -- --screenshot /tmp/out.png`.

## 2. Repository & Module Map (Summary)
Full auto-verified module map: [`docs/agent/PROJECT_MAP.md`](docs/agent/PROJECT_MAP.md).
- `app/src/core/` — Pure domain logic (diffing, file kinds, AST outline, PDF standards, settings); zero GUI/OS/network dependencies. See [`app/src/core/AGENTS.md`](app/src/core/AGENTS.md).
- `app/src/platform/` — OS abstractions: native macOS menus (`menu.rs`) and background-threaded `rfd` file dialogs (`dialogs.rs`).
- `app/src/services/` — Headless and lifecycle background services (`screenshot.rs`).
- `app/src/collab/` — P2P real-time collaboration: Yrs CRDT (`doc.rs`), cursor presence (`awareness.rs`), wire types (`protocol.rs`), Iroh QUIC (`transport.rs`), disk snapshots (`persistence.rs`), background actor (`session.rs`), and collab UI (`ui.rs`).
- `app/src/project.rs` & `app/src/project/buffer.rs` — On-disk workspace tree, open file buffers, and in-memory blank project support.
- `app/src/world.rs` & `app/src/compiler.rs` — `typst::World` implementation (`IdeWorld`), package downloading, rasterization, and PDF export.
- `app/src/highlight.rs` — Typst syntax highlighting into `egui::text::LayoutJob`.
- `app/src/ui/` — Presentation layer: `theme.rs` (tokens/palettes), `components/` (docked panels), and `dialogs/` (modal sub-windows).
- `app/src/app.rs` — Top-level `AzoTypeApp` state orchestrator, channel polling, and `eframe::App` update loop.
- `app/tests/` — Integration and characterization test suites (`cli_tests`, `diff_tests`, `i18n_tests`, `outline_tests`, `pdf_config_tests`, `persistence_tests`, `project_tests`).

## 3. Architecture & Dependency Direction
Dependencies point strictly **inward**:
`ui` / `app` → `collab` / `compiler` / `project` / `services` → `platform` → `core`.
- `core` MUST NEVER import `egui`, `eframe`, `rfd`, `muda`, `tokio`, or `iroh`. Inject external data into `core` via traits (reference: `SourceResolver` in `app/src/core/outline.rs`).
- UI components in `app/src/ui/` must not spawn threads or perform blocking disk/network I/O directly; return intent via `*Output` structs to `app.rs`.

## 4. "Where Does What Go?" Lookup Table
| Task / Change Type | Target Location | Reference Implementation |
| :--- | :--- | :--- |
| Pure domain type, parser, or algorithm | `app/src/core/<module>.rs` | `app/src/core/diff.rs` |
| New user setting / preference | `app/src/core/settings.rs` + `app/src/ui/dialogs/settings.rs` | `app/src/core/settings.rs` |
| New docked UI panel or toolbar | `app/src/ui/components/<name>.rs` | `app/src/ui/components/status_bar.rs` |
| New modal dialog / OS sub-window | `app/src/ui/dialogs/<name>.rs` | `app/src/ui/dialogs/export_pdf.rs` |
| New menu item or keyboard shortcut | `app/src/platform/menu.rs` + `app/src/ui/components/menu_bar.rs` + `app/src/app.rs` | `app/src/platform/menu.rs` |
| New editor right-click / edit action | `app/src/ui/components/editor_actions.rs` | `app/src/ui/components/editor_actions.rs` |
| OS-specific dialog or native integration | `app/src/platform/<name>.rs` | `app/src/platform/dialogs.rs` |
| New localization key or translation | `locales/<lang>.yml` + `app/src/core/i18n.rs` | `locales/en.yml` |
| P2P wire message or CRDT operation | `app/src/collab/protocol.rs` + `app/src/collab/doc.rs` | `app/src/collab/protocol.rs` |
| Unit tests for module `foo.rs` | `app/src/tests/<path>/foo_tests.rs` (via `#[cfg(test)] #[path = "..."] mod tests;`) | `app/src/tests/core/pdf_tests.rs` |
| Cross-module / black-box test | `app/tests/<topic>_tests.rs` | `app/tests/outline_tests.rs` |

## 5. Core Conventions (Short Form)
Full details with Good/Bad examples: [`docs/agent/CONVENTIONS.md`](docs/agent/CONVENTIONS.md).
- **Git & Atomic Conventional Commits:** Never work directly on `main`/`master`; create a feature/fix/chore branch first. Commit **after every logical step** using `<type>(<scope>): <summary>` (`feat`, `fix`, `refactor`, `docs`, `test`, `chore`, `style`, `perf`, `build`, `ci`). Never batch multiple features or steps into one end-of-session commit.
- **File Size & Naming:** Keep production files under ~400 lines (hard cap: 600 lines unless explicitly whitelisted in `scripts/verify_structural_rules.py`). Never create `utils.rs`, `helpers.rs`, `misc.rs`, or `common.rs`.
- **Module Docs:** Every `.rs` file in `app/src/` must begin with an inner `//!` doc comment; public items in `core/` require `///` docs (`#![warn(missing_docs)]`).
- **Tests:** Zero inline `mod tests { ... }` blocks in production files. Always use `#[cfg(test)] #[path = "..."] mod tests;` pointing into `app/src/tests/`, or integration tests in `app/tests/`.
- **UI State vs. Rendering:** Use the `Params` + `Output` struct pattern (`render_*(ui, Params) -> Output`).
- **Error Handling & Threading:** Return `Result<T, String>` or typed errors; never `unwrap()` user input or network/disk I/O. Never block the `egui` thread with synchronous dialogs or network calls.

## 6. Always / Ask First / Never
- **ALWAYS:** Work on a non-main branch; create atomic Conventional Commits after each logical unit of work; run `./scripts/check.sh` before finishing; update `docs/agent/PROJECT_MAP.md` when adding/moving modules.
- **ASK FIRST:** Adding or upgrading Cargo dependencies in `app/Cargo.toml`; changing on-disk persistence formats (`.typst-collab/state.yrs`) or P2P wire protocol (`CollabWireMsg`); altering public CLI flags.
- **NEVER:** Commit directly to `main`; batch multiple distinct changes into one commit at the end; place `mod tests { ... }` inline in production `.rs` files; import `egui` or `tokio` inside `app/src/core/`; silently fix unrelated bugs during a refactoring without documenting them.

## 7. Definition of Done (Every Change)
1. Logical steps committed atomically with Conventional Commit messages on a dedicated branch.
2. New behavior covered by unit (`app/src/tests/<path>/foo_tests.rs`) or integration (`app/tests/`) tests.
3. Documentation (`//!` module docs, `PROJECT_MAP.md`, and relevant `docs/agent/*.md`) updated in the same commit as structural changes.
4. `./scripts/check.sh` exits with `0` (fmt, clippy `-D warnings`, 100% tests green, all structural checks passing).

## 8. Token-Saving Reading Protocol & Maintenance Rule
1. **Strict Reading Order:** Read `AGENTS.md` → [`docs/agent/PROJECT_MAP.md`](docs/agent/PROJECT_MAP.md) → only the target module.
2. **Signatures First:** Read the `//!` header and public structs/signatures (`rg "^pub "` or line-ranged reads) before reading function bodies.
3. **Targeted Search:** Use `rg <symbol>` to locate call sites; never read the entire source tree into context.
4. **Follow Reference Implementations:** Check [`docs/agent/RECIPES.md`](docs/agent/RECIPES.md) and [`docs/agent/GOTCHAS.md`](docs/agent/GOTCHAS.md) before writing code.
5. **Same-Commit Maintenance:** Any commit that changes modules, commands, or rules MUST update the corresponding agent docs in that same commit.

## 9. Detailed Agent Documentation Index
- [`docs/agent/PROJECT_MAP.md`](docs/agent/PROJECT_MAP.md) — Auto-verified module reference, entrypoints, and test locations.
- [`docs/agent/RECIPES.md`](docs/agent/RECIPES.md) — Step-by-step blueprints for common features and extensions.
- [`docs/agent/CONVENTIONS.md`](docs/agent/CONVENTIONS.md) — Enforced coding patterns with Good/Bad examples from this codebase.
- [`docs/agent/GOTCHAS.md`](docs/agent/GOTCHAS.md) — Non-obvious threading, UTF-16 CRDT, platform, and `egui` viewport pitfalls.
