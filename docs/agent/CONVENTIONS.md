# CONVENTIONS.md — Detailed Project Standards & Concrete Examples

This document explains the enforced coding, architectural, testing, and git conventions in `typst-ide`. Every rule includes concrete **Good** and **Bad** examples taken directly from this codebase.

---

## 1. Git Workflow & Atomic Conventional Commits

### Rule
- **Never work directly on `main` or `master`:** Always create a feature, fix, or chore branch (`feat/<name>`, `fix/<name>`, `chore/<name>`).
- **Commit after every single logical step:** When implementing a feature that spans multiple concerns (e.g. domain model + UI + documentation), commit each step separately. Never bundle an entire afternoon of multiple changes into one end-of-session commit.
- **Conventional Commits format:** Use `<type>(<scope>): <summary>` in lowercase imperative mood.
  - Allowed types: `feat`, `fix`, `refactor`, `docs`, `test`, `chore`, `style`, `perf`, `build`, `ci`.
  - Common scopes: `core`, `ui`, `collab`, `compiler`, `project`, `platform`, `deps`, `checks`, `agent`.

### Examples

#### ❌ Bad: Monolithic Commit at Session End
```bash
# BAD: Bundling 4 separate concerns into one giant commit at the end of the day
git add .
git commit -m "add export pdf dialog, fix highlight bug, refactor project.rs and update readme"
```

#### ✅ Good: Atomic Conventional Commits
```bash
# Step 1: Core domain logic
git add app/src/core/pdf.rs app/src/core/pdf/tests.rs
git commit -m "feat(core): add PdfExportConfig and PDF/A archival validation"

# Step 2: UI presentation
git add app/src/ui/dialogs/export_pdf.rs app/src/ui/dialogs/mod.rs
git commit -m "feat(ui): add export PDF modal sub-window dialog"

# Step 3: Application orchestration
git add app/src/app.rs
git commit -m "feat(app): wire export PDF dialog into menu bar and shortcuts"

# Step 4: Documentation
git add docs/agent/PROJECT_MAP.md
git commit -m "docs(agent): update project map for export_pdf dialog"
```

---

## 2. Layer Boundaries & Dependency Direction

### Rule
Dependencies point strictly **inward**:
`ui` / `app` → `collab` / `compiler` / `project` / `services` → `platform` → `core`.

- `app/src/core/` contains pure domain logic. It MUST NOT import `egui`, `eframe`, `rfd`, `muda`, `tokio`, `iroh`, `arboard`, or `reqwest`.
- `app/src/core/` must not perform direct filesystem access (`std::fs`). When source code or file contents are needed, abstract it via a trait (e.g., `SourceResolver`).
- `app/src/platform/` must not import outer layers (`crate::ui` or `crate::app`).

### Examples

#### ❌ Bad: Core Reading Files from Disk and Importing UI
```rust
// In app/src/core/outline.rs - BAD!
use egui::Color32; // FORBIDDEN in core!
use std::fs; // FORBIDDEN in core!

pub fn extract_headings(path: &Path) -> Vec<Heading> {
    let content = fs::read_to_string(path).unwrap(); // Directly couples core to filesystem
    // ...
}
```

#### ✅ Good: Pure Trait Decoupling (`app/src/core/outline.rs`)
```rust
// In app/src/core/outline.rs - GOOD!
pub trait SourceResolver {
    fn read_source_text(&self, path: &Path) -> Result<String, String>;
    fn is_main_entrypoint(&self, path: &Path) -> bool;
}

pub fn extract_typst_ast_outline(
    text: &str,
    current_file: &Path,
    source_resolver: &dyn SourceResolver,
    visited: &mut HashSet<PathBuf>,
) -> Vec<DocumentOutlineItem> {
    // Pure logic: operates purely in memory!
}
```

---

## 3. Module Naming and File Size Guidelines

### Rule
- **No generic grab-bag filenames:** Never create `utils.rs`, `helpers.rs`, `misc.rs`, or `common.rs`. Name files after their domain responsibility (e.g., `diff.rs`, `outline.rs`, `theme.rs`, `file_kind.rs`).
- **File size guideline:** Production modules should stay under ~400 lines. The hard ceiling is **600 lines** (mechanically verified by `scripts/verify_structural_rules.py`).
- If a file approaches 500 lines, decompose it by sub-responsibility (e.g., extracting `editor_actions.rs` from `editor.rs`, or `protocol.rs` from `transport.rs`).

### Examples

#### ❌ Bad: Generic Helper Module
```rust
// app/src/utils.rs - BAD!
pub fn diff_strings(...) { ... }
pub fn get_file_icon(...) { ... }
pub fn load_font(...) { ... }
```

#### ✅ Good: Domain-Focused Separation
```
app/src/core/diff.rs       # Pure UTF-16 differencing algorithms
app/src/core/file_kind.rs  # Semantic file types and icons
app/src/highlight.rs       # Typst syntax highlighting
```

---

## 4. Documentation Rules

### Rule
- Every `.rs` file in `app/src/` must begin with an inner module doc comment (`//!`) on line 1 describing its single responsibility.
- In `app/src/core/`, `#![warn(missing_docs)]` is active: all `pub` items, struct fields, and enum variants must have a `///` doc comment.
- Comments must explain **why** something is done (invariants, background, non-obvious design choices), not restate the obvious Rust syntax.

### Examples

#### ❌ Bad: Missing or Redundant Documentation
```rust
// No //! at the top of the file!

/// Does diff
pub fn compute_utf16_diff(old_text: &str, new_text: &str) -> Option<Utf16TextDiff>
```

#### ✅ Good: Explaining Context and Invariants (`app/src/core/diff.rs`)
```rust
//! Pure text differencing and UTF-16 code-unit conversion utilities.
//!
//! Provides conversion between Unicode scalar (`char`) counts used by editor widgets
//! and UTF-16 code units used by CRDT text models (such as `yrs`).

/// Computes the minimal single-span difference between `old_text` and `new_text` using
/// common prefix and common suffix over Unicode scalar values (`char`), and returns the
/// corresponding UTF-16 offset, removal length, and inserted substring.
pub fn compute_utf16_diff(old_text: &str, new_text: &str) -> Option<Utf16TextDiff>
```

---

## 5. Test Organization & Naming

### Rule
- **Zero inline `mod tests { ... }` in production files:** Production files remain lean and focused exclusively on production code.
- Declare `#[cfg(test)] mod tests;` in `foo.rs` and place the unit tests in `foo/tests.rs`.
- Integration and characterization tests belong in `app/tests/<topic>_tests.rs`.
- **Descriptive test names:** Name tests after the exact behavior or edge case being verified (`verb_expectation_under_condition`).

### Examples

#### ❌ Bad: Inline Tests with Uninformative Names
```rust
// In app/src/core/pdf.rs - BAD!
#[cfg(test)]
mod tests {
    #[test]
    fn test1() { ... }
    #[test]
    fn test_pdf() { ... }
}
```

#### ✅ Good: Companion File with Behavioral Names
In `app/src/core/pdf.rs`:
```rust
#[cfg(test)]
mod tests;
```

In `app/src/core/pdf/tests.rs`:
```rust
//! Unit tests for PDF export version choices and archival standard validation.

use super::*;

#[test]
fn test_default_pdf_config() { ... }

#[test]
fn test_incompatible_version_and_standard_fails() { ... }
```

In `app/tests/pdf_config_tests.rs`:
```rust
#[test]
fn rejects_invalid_standard_and_version_combinations_with_descriptive_error() { ... }
```

---

## 6. UI Rendering vs. State Mutation (Params + Output Pattern)

### Rule
- UI component functions in `app/src/ui/components/` must be pure rendering functions:
  ```rust
  pub fn render_<name>(ui: &mut Ui, params: <Name>Params<'_>) -> <Name>Output
  ```
- Components take inputs via borrowed `<Name>Params` and return user intentions via `<Name>Output`.
- Components must **never** mutate global variables, spawn threads, or perform blocking disk/network calls directly.

### Examples

#### ❌ Bad: UI Component Performing Side Effects
```rust
// In app/src/ui/components/sidebar.rs - BAD!
if ui.button("Delete File").clicked() {
    std::fs::remove_file(&active_file).unwrap(); // BLOCKING I/O in render loop!
    std::process::Command::new("git").spawn().unwrap(); // Spawning processes in UI!
}
```

#### ✅ Good: Intent Returned to Orchestrator (`app/src/ui/components/sidebar.rs`)
```rust
// In app/src/ui/components/sidebar.rs - GOOD!
#[derive(Debug, Default)]
pub struct SidebarOutput {
    pub file_selected: Option<PathBuf>,
    pub delete_file_requested: Option<PathBuf>,
    pub rename_file_requested: Option<(PathBuf, String)>,
}

pub fn render_sidebar(ui: &mut Ui, params: SidebarParams<'_>) -> SidebarOutput {
    let mut output = SidebarOutput::default();
    if ui.button("Delete File").clicked() {
        output.delete_file_requested = Some(params.active_file.clone());
    }
    output
}
```

---

## 7. Error Handling & Asynchronous I/O

### Rule
- Never use `.unwrap()` or `.expect()` on fallible external inputs (user input, JSON, disk, network).
- Return `Result<T, String>` or a typed error.
- All network I/O (`iroh`) and file dialogs (`rfd`) must run on background threads or Tokio tasks. The `eframe` GUI thread must never be blocked.

### Examples

#### ❌ Bad: Panicking on Fallible Network Input
```rust
// In collab/protocol.rs - BAD!
let ticket_data = base64::decode(ticket_str).unwrap(); // Will crash the app on invalid user input!
```

#### ✅ Good: Descriptive `Result` Error Handling (`app/src/collab/protocol.rs`)
```rust
// In collab/protocol.rs - GOOD!
pub fn decode(encoded: &str) -> Result<Self, String> {
    if !encoded.starts_with(TICKET_PREFIX) {
        return Err(format!("Ticket missing required '{}' prefix.", TICKET_PREFIX));
    }
    let raw = BASE64
        .decode(&encoded[TICKET_PREFIX.len()..])
        .map_err(|e| format!("Base64 ticket decoding failed: {}", e))?;
    // ...
}
```
