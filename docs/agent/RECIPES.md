# RECIPES.md — Step-by-Step Extension Blueprints

Each recipe lists the exact files to modify **in dependency order**, points to an existing reference implementation in the codebase, and concludes with a verification checklist.

> **Git Workflow Reminder:** Work on a feature branch (never `main`). Commit atomically after each logical step using Conventional Commits (`feat(scope): ...`, `test(scope): ...`, `docs(scope): ...`).

---

## Recipe 1: Adding a New Feature with Its Own UI View (Panel or Sub-Window)

Use this recipe when adding a new docked panel (e.g., a diagnostics drawer) or a secondary modal dialog sub-window (e.g., an export or document statistics dialog).

- **Reference Implementations:**
  - Docked component: `app/src/ui/components/status_bar.rs`
  - Modal OS sub-window / dialog: `app/src/ui/dialogs/export_pdf.rs`

### Step-by-Step Order
1. **Domain Model (`app/src/core/`):**
   - If the view needs pure configuration or state calculation, add types/functions in `app/src/core/` (e.g., `app/src/core/pdf.rs`) with `///` doc comments and unit tests in `app/src/tests/core/<module>_tests.rs`.
   - *Commit:* `feat(core): add domain model for <feature>`
2. **View Module (`app/src/ui/components/` or `app/src/ui/dialogs/`):**
   - Create the new module starting with a `//!` inner doc comment.
   - Define `pub struct <Name>Params<'a>` (inputs borrowed from `AzoTypeApp`) and `pub struct <Name>Output` (events/intents returned to `AzoTypeApp`).
   - Implement `pub fn render_<name>(ctx: &Context, ...) -> <Name>Output`.
   - For dialogs (`app/src/ui/dialogs/`), support both `ctx.show_viewport_immediate(...)` and the `embed_dialogs` headless fallback flag (see `app/src/ui/dialogs/export_pdf.rs`).
3. **Register & Re-export (`app/src/ui/components/mod.rs` or `app/src/ui/dialogs/mod.rs`):**
   - Declare `pub mod <name>;` and `pub use <name>::{render_<name>, ...};`.
4. **Wire into Application State (`app/src/app.rs`):**
   - Add any persistent visibility boolean or state fields to `AzoTypeApp`.
   - Call `render_<name>` from `AzoTypeApp::update` (or a helper method on `AzoTypeApp`) and handle the returned `<Name>Output`.
   - *Commit:* `feat(ui): add <name> view and wire into AzoTypeApp`
5. **Update Project Map:**
   - Run `python3 scripts/verify_structural_rules.py --write-map` and commit `docs/agent/PROJECT_MAP.md`.

### Checklist
- [ ] Module begins with `//!` and is under 400 lines.
- [ ] View function does not perform direct disk/network I/O; side effects are returned via `*Output`.
- [ ] Modal dialogs check `embed_dialogs` so headless `--screenshot` mode can render them inside the main viewport.
- [ ] `docs/agent/PROJECT_MAP.md` regenerated and `./scripts/check.sh` passes.

---

## Recipe 2: Adding a New Action / Menu Command / Editor Shortcut

Use this recipe when adding a top-level menu command, keyboard shortcut, or editor right-click action.

- **Reference Implementations:**
  - Global menu & shortcut action: `MenuAction` in `app/src/platform/menu.rs` and `app/src/ui/components/menu_bar.rs`
  - Editor-local text action: `EditorContextAction` in `app/src/ui/components/menu_bar.rs` and `app/src/ui/components/editor_actions.rs`
  - Async collaboration command: `UiToSessionCmd` / `SessionToUiEvent` in `app/src/collab/session.rs`

### Step-by-Step Order
1. **Define the Action Variant:**
   - For an app-wide command: add a variant to `MenuAction` in `app/src/platform/menu.rs`.
   - For a text-buffer edit command: add a variant to `EditorContextAction` in `app/src/ui/components/menu_bar.rs`.
2. **Expose in Menus & Shortcuts:**
   - Add the menu button to `app/src/ui/components/menu_bar.rs` (Linux/Windows in-window menu bar).
   - If applicable on macOS, add the native `muda::MenuItem` in `app/src/platform/menu.rs` under `#[cfg(target_os = "macos")]`.
   - If triggered via keyboard shortcut, bind it in `AzoTypeApp::handle_keyboard_shortcuts` inside `app/src/app.rs`.
3. **Implement the Handler:**
   - For `MenuAction`: handle the new variant in `AzoTypeApp::dispatch_menu_action` in `app/src/app.rs`.
   - For `EditorContextAction`: handle buffer mutation in `apply_editor_context_action` in `app/src/ui/components/editor_actions.rs` and optionally expose it in `render_editor_context_menu`.
4. **Commit Atomically:**
   - *Commit:* `feat(ui): add <action_name> menu command and shortcut handler`

### Checklist
- [ ] Both `app/src/ui/components/menu_bar.rs` and `app/src/platform/menu.rs` updated if it is a top-level menu item.
- [ ] Editor mutations update `edit_output.state.undoer()` and set `output.text_changed = true` + `output.edited_text_snapshot`.
- [ ] `./scripts/check.sh` passes.

---

## Recipe 3: Adding a New User Setting Field

Use this recipe when adding a configurable user preference (e.g., editor behavior, compiler toggle, or preview option).

- **Reference Implementation:** `IdeSettings` in `app/src/core/settings.rs` and `render_settings_dialog` in `app/src/ui/dialogs/settings.rs`

### Step-by-Step Order
1. **Extend `IdeSettings` (`app/src/core/settings.rs`):**
   - Add the `pub` field with a `///` doc comment to `IdeSettings`.
   - Set its default value in `impl Default for IdeSettings`.
   - Add a unit test in `app/src/tests/core/settings_tests.rs` verifying the default value.
   - *Commit:* `feat(core): add <field_name> to IdeSettings`
2. **Expose Controls in Settings Dialog (`app/src/ui/dialogs/settings.rs`):**
   - Add the UI control (checkbox, slider, or text input) under the appropriate `SettingsTab` (`Appearance`, `EditorAndDisk`, or `CompilerAndPreview`).
   - If changing the setting requires an immediate side effect (such as recompiling, rescanning disk, or updating theme), set the corresponding flag on `SettingsModalOutput`.
3. **Propagate in `AzoTypeApp` (`app/src/app.rs`):**
   - Pass `&self.settings.<field>` into the target component (`EditorPaneParams`, `PreviewPaneParams`, or `compile_project`).
   - *Commit:* `feat(ui): expose <field_name> in settings dialog and wire to <component>`

### Checklist
- [ ] Field has a `///` doc comment in `app/src/core/settings.rs` (`#![warn(missing_docs)]` is enforced).
- [ ] Clicking "Restore Defaults" in `app/src/ui/dialogs/settings.rs` resets the field via `IdeSettings::default()`.
- [ ] Unit test in `app/src/tests/core/settings_tests.rs` covers the default.

---

## Recipe 4: Adding a New Platform-Specific Function

Use this recipe when integrating an OS-dependent capability (native dialogs, clipboard, menus, platform paths).

- **Reference Implementations:** `app/src/platform/dialogs.rs` and `app/src/platform/menu.rs`

### Step-by-Step Order
1. **Isolate in `app/src/platform/`:**
   - Add the function or struct in `app/src/platform/<feature>.rs` (or `dialogs.rs` / `menu.rs`).
   - Place all `#[cfg(target_os = "...")]` attributes inside `app/src/platform/`, never scattered across `ui/` or `app.rs`.
   - Provide a no-op or cross-platform fallback for other targets so callers in `app.rs` use an unconditional API (see `MacOsMenuBar::init` in `app/src/platform/menu.rs`).
2. **Background Threading for Blocking/Async OS Calls:**
   - If the OS API is asynchronous or blocking (like `rfd::AsyncFileDialog`), spawn a `std::thread::spawn` worker and send the result back to `AzoTypeApp` via `Sender<IoEvent>`, calling `ctx.request_repaint()` on completion.
3. **Re-export in `app/src/platform/mod.rs`:**
   - Expose a clean platform-agnostic signature and update `docs/agent/PROJECT_MAP.md` (`python3 scripts/verify_structural_rules.py --write-map`).

### Checklist
- [ ] No `#[cfg(target_os = ...)]` leaked into `app/src/ui/` or `app/src/core/`.
- [ ] `app/src/platform/` does not import `crate::ui` or `crate::app`.
- [ ] Blocking OS calls run off the `egui` render thread and wake the UI with `ctx.request_repaint()`.

---

## Recipe 5: Adding or Extending Domain Error Handling

Use this recipe when introducing fallible operations in `core/`, `project.rs`, `compiler.rs`, or `collab/`.

- **Reference Implementations:**
  - `PdfExportConfig::build_standards` in `app/src/core/pdf.rs`
  - `DiskProject::create_file` / `rename_file` in `app/src/project.rs`
  - `SessionTicket::decode` in `app/src/collab/protocol.rs`

### Step-by-Step Order
1. **Return `Result<T, String>` (or a descriptive domain error enum):**
   - Never call `.unwrap()` or `.expect()` on file I/O, network streams, user-provided paths, or ticket strings.
   - Format actionable, user-facing error descriptions (e.g., `"File name cannot be empty."` or `"Path escapes project root"`).
2. **Surface Errors in UI State (`app/src/app.rs`):**
   - Map `Err(msg)` at the `AzoTypeApp` boundary into `self.file_dialog_error = Some(msg)`, `self.export_pdf_error = Some(msg)`, `self.collab_ui.last_error = Some(msg)`, or `self.status_notice = format!("Error: {}", msg)`.
3. **Add Error-Case Tests:**
   - Every error branch must have a corresponding negative test (see `rejects_invalid_standard_and_version_combinations_with_descriptive_error` in `app/tests/pdf_config_tests.rs`).

### Checklist
- [ ] Zero `unwrap()` / `expect()` on fallible runtime inputs.
- [ ] Error message is deterministic and tested in unit or integration tests.

---

## Recipe 6: Adding a New Test (Unit, Headless `egui_kittest`, or Integration)

Use this recipe when adding test coverage for new features, UI changes, or bug fixes.

- **Reference Implementations:**
  - Centralized pure unit test: `app/src/tests/core/diff_tests.rs` (declared via `#[cfg(test)] #[path = "../tests/core/diff_tests.rs"] mod tests;` in `app/src/core/diff.rs`)
  - Centralized UI unit + `egui_kittest` tests: `app/src/tests/ui/dialogs/export_pdf_tests.rs`, `app/src/tests/ui/components/editor_tests.rs`, and `app/src/tests/collab/ui_tests.rs`
  - End-to-end headless application UI suite: `app/tests/ui_tests.rs`
  - Domain integration test suite: `app/tests/outline_tests.rs` and `app/tests/project_tests.rs`

### Step-by-Step Order
1. **Choose the Right Test Tier:**
   - **Layer 1 — Pure Unit Test (`app/src/tests/<path>/<module>_tests.rs`):** Test pure domain algorithms, `*State` transitions, dialog validation, and buffer math without needing `egui`.
   - **Layer 2 — Headless UI Smoke/Interaction Test (`egui_kittest`):** Inside `app/src/tests/ui/` or `app/src/tests/collab/ui_tests.rs`, use `Harness::new_ui_state` (for `Ui` panels/dialog bodies) or `Harness::new_state` (for `TopBottomPanel`/`SidePanel`) to verify rendering and click/toggle interactions via AccessKit queries (`get_by_label`). Call `harness.run()` after each click (or `harness.step()` for spinners/animated progress bars) and pass `locale = "en"` to `t!` in parallel `--lib` tests.
   - **Integration Test (`app/tests/<topic>_tests.rs`):** Test public library APIs (`azo_type::...`) or multi-panel `AzoTypeApp::new_headless` + `render_ui` flows (`app/tests/ui_tests.rs`).
2. **No Inline Tests Rule:**
   - In `app/src/<path>/<module>.rs`, add only:
     ```rust
     #[cfg(test)]
     #[path = "../tests/<path>/<module>_tests.rs"] // adjust `..` depth to `app/src/`
     mod tests;
     ```
   - Do **not** declare `mod tests;` in `app/src/lib.rs`.
   - Put the test functions in `app/src/tests/<path>/<module>_tests.rs` starting with a `//!` inner doc comment.
3. **Isolated Temporary Directories for Disk Tests:**
   - When a test requires a filesystem directory (e.g., `DiskProject` or `CollabPersistence`), create a unique temp directory under `std::env::temp_dir()` using process ID / atomic counter and remove it at the end of the test (see `unique_temp_dir` in `app/tests/project_tests.rs`).
4. **Descriptive Behavioral Naming & Coverage Check:**
   - Name tests after the behavior they verify (e.g., `prevents_path_traversal_when_flushing_files`, `export_pdf_body_renders_valid_state_and_emits_export_action`).
   - Run `./scripts/coverage.sh` to confirm modified UI/service files have measurable coverage and total coverage has not regressed by more than 2%.

### Checklist
- [ ] No `mod tests { ... }` inline in any production `.rs` file.
- [ ] Unlabeled `TextEdit` or icon widgets queried by tests have `.labelled_by(...)`.
- [ ] New `*_tests.rs` file starts with a `//!` module doc comment.
- [ ] `python3 scripts/verify_structural_rules.py --write-map` run if a new unit test file was created.

---

## Recipe 7: Adding a New Cargo Dependency

Adding dependencies increases compile times, binary size, and supply-chain surface.

- **Target File:** `app/Cargo.toml`

### Step-by-Step Order
1. **Ask First:**
   - Per `AGENTS.md`, always ask the user for approval before adding a new crate to `app/Cargo.toml`.
2. **Check Existing Dependencies First:**
   - Verify whether an existing crate in `app/Cargo.toml` (`serde`, `uuid`, `base64`, `chrono`, `image`, `tokio`, `typst-syntax`, `egui`) already provides the needed functionality.
3. **Minimal Feature Flags:**
   - Use `default-features = false` where possible and enable only the required feature flags (see `chrono`, `image`, and `eframe` in `app/Cargo.toml`).
4. **Layer Placement Check:**
   - Never expose UI or I/O crates inside `app/src/core/`.
5. **Check License Compatibility (`GPL-3.0-or-later`):**
   - Run `cargo deny --locked --all-features check licenses` locally against `deny.toml` (`cargo install --locked cargo-deny` if not installed).
   - Ensure every newly added or upgraded crate uses a license compatible with `GPL-3.0-or-later`; do **not** weaken `deny.toml` with incompatible licenses or `GPL-3.0-only` without explicit user approval.
6. **Verify Full Build & Lockfile:**
   - Run `./scripts/check.sh` and commit `app/Cargo.toml` and `Cargo.lock` together in a `build(deps): ...` or `chore(deps): ...` commit.

### Checklist
- [ ] User explicitly approved the new dependency.
- [ ] Minimal `features` enabled in `app/Cargo.toml`.
- [ ] License compatibility verified locally (`cargo deny --locked --all-features check licenses`).
- [ ] `./scripts/check.sh` passes with zero warnings.

---

## Recipe 8: Adding New Localization Strings or Locales (i18n & Weblate)

Use this recipe when extracting hardcoded user-facing strings or adding support for an additional language.

- **Reference Implementation:** `locales/en.yml`, `locales/de.yml`, `app/src/core/i18n.rs`, and `app/src/ui/components/menu_bar.rs`

### Step-by-Step Order
1. **Define Keys in Monolingual Files (`locales/<lang>.yml`):**
   - In `locales/en.yml`, add the new key under the appropriate semantic section (`menu`, `status`, `sidebar`, `editor`, `preview`, `settings`, `export_pdf`, `file_modals`, `collab`, `app`).
   - Use `%{param}` for interpolated dynamic values (e.g. `path_prefix: "Path: %{path}"`).
   - Add the translated equivalent in all other monolingual files (e.g. `locales/de.yml`).
   - Files MUST start with `_version: 1` (Weblate standard single-locale format).
2. **Support Native Display Name (`app/src/core/i18n.rs`):**
   - If adding a new locale, register its native human-readable name in `locale_native_name(locale: &str)`.
   - Update unit tests in `app/src/tests/core/i18n_tests.rs`.
3. **Use the `t!` Macro in Presentation Modules (`app/src/ui/`):**
   - Import `use rust_i18n::t;`.
   - Replace literal strings with `t!("section.key")` or `t!("section.key", param = val)`.
4. **Trigger UI Recompilation:**
   - Run `touch app/src/lib.rs` so `rust_i18n::i18n!` rebuilds with the updated YAML files.
5. **Verify:**
   - Run `cargo test --manifest-path app/Cargo.toml --test i18n_tests`.

### Checklist
- [ ] `locales/` contains separate monolingual `.yml` files with `_version: 1`.
- [ ] No hardcoded user-facing strings in `egui` UI calls.
- [ ] Dynamic values use `%{var}` in YAML and `var = val` in `t!`.
- [ ] `i18n_tests` pass.

