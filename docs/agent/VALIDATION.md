# VALIDATION.md — Agent Onboarding Verification & Dry-Run Report

This report validates that an incoming AI coding agent can orient itself and successfully perform modifications in **at most 2–3 documents** without tree-crawling, and confirms that all structural rules are mechanically enforced.

---

## 1. Entry Document Size & Token Budgets

Measurements taken across onboarding documents:

| Document | Lines | Word Count | Approx. Tokens (~1.3x words) | Budget Target | Status |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `AGENTS.md` (Repo Root) | 81 | 682 | ~890 | < 150 lines | **Well within budget** (54% of limit) |
| `app/src/core/AGENTS.md` | 15 | 163 | ~215 | < 40 lines | **Well within budget** (38% of limit) |
| `docs/agent/PROJECT_MAP.md` | 85 | 680 | ~980 | Reference map | Compact tabular overview |
| `docs/agent/RECIPES.md` | 199 | 1,440 | ~1,900 | Blueprints | Modular step-by-step |
| `docs/agent/CONVENTIONS.md` | 267 | 1,480 | ~1,980 | Detailed standards | Good/Bad real examples |
| `docs/agent/GOTCHAS.md` | 110 | 850 | ~1,120 | Pitfalls | Invariant explanations |

A new agent needs to read **only `AGENTS.md` (~890 tokens)** on startup, keeping the context clean and token expenditure minimal.

---

## 2. Dry-Run Walkthroughs (3 Realistic Agent Tasks)

### Task 1: Adding a New User Setting (`auto_close_brackets: bool`)
- **Reading Path:**
  1. `AGENTS.md` § 4 ("Where Does What Go?"):
     - Target: `app/src/core/settings.rs` + `app/src/ui/dialogs/settings.rs` (Ref: `app/src/core/settings.rs`).
  2. `docs/agent/RECIPES.md` § Recipe 3 ("Adding a New User Setting Field"):
     - Lists exact 3-step sequence:
       - Step 1: Add field to `IdeSettings` in `app/src/core/settings.rs` with `///` doc comment, update `Default`, add unit test in `app/src/core/settings/tests.rs`. Commit atomically.
       - Step 2: Add checkbox widget to `app/src/ui/dialogs/settings.rs`.
       - Step 3: Pass into `EditorPaneParams` in `app/src/app.rs`. Commit atomically.
- **Documents Read:** **2 documents** (`AGENTS.md` → `RECIPES.md`) before directly opening the 3 target files. Zero tree-search required.

### Task 2: Building a New Modal Dialog ("Document Statistics")
- **Reading Path:**
  1. `AGENTS.md` § 4 ("Where Does What Go?"):
     - Target: `app/src/ui/dialogs/<name>.rs` (Ref: `app/src/ui/dialogs/export_pdf.rs`).
  2. `docs/agent/RECIPES.md` § Recipe 1 ("Adding a New Feature with Its Own UI View"):
     - Explains the `*Params` and `*Output` struct pattern.
     - Crucial gotcha highlighted: must support `embed_dialogs` fallback for headless `--screenshot` mode.
     - Sequence: create module → register in `app/src/ui/dialogs/mod.rs` → wire in `app/src/app.rs` → regenerate map.
- **Documents Read:** **2 documents** (`AGENTS.md` → `RECIPES.md`) plus referencing `app/src/ui/dialogs/export_pdf.rs`.

### Task 3: Modifying or Debugging UTF-16 Text Diffing
- **Reading Path:**
  1. `AGENTS.md` § 4 ("Where Does What Go?"):
     - Target: `app/src/core/<module>.rs` (Ref: `app/src/core/diff.rs`).
  2. `docs/agent/PROJECT_MAP.md` (Row `app/src/core/diff.rs`):
     - Summarizes purpose: UTF-16 code units vs `char` conversions.
     - Points to test locations: `app/src/core/diff/tests.rs` (unit) and `app/tests/diff_tests.rs` (integration).
  3. `docs/agent/GOTCHAS.md` § 2 ("The Three String Offset Systems"):
     - Clarifies the non-obvious difference between byte offsets, `char` indices, and `yrs` UTF-16 surrogate pairs.
- **Documents Read:** **3 documents** (`AGENTS.md` → `PROJECT_MAP.md` → `GOTCHAS.md`).

---

## 3. Intentional Failure Tests of Structural Checks

The mechanical enforcement script (`scripts/verify_structural_rules.py`, invoked via `./scripts/check.sh`) was tested against intentional deliberate violations to confirm that every check fails with clear, actionable error messages:

### Test A: Forbidden Generic Filename (`app/src/utils.rs`)
```
❌ Structural Convention Checks FAILED:
[RULE: NO_GENERIC_FILENAMES] Forbidden module name `app/src/utils.rs`.
  -> Fix: Name modules after their domain responsibility (e.g., `diff.rs`, `outline.rs`), never `utils.rs`.
  -> Ref: docs/agent/CONVENTIONS.md#module-naming-and-file-size
```

### Test B: Inline Test Block in Production File (`mod tests { ... }`)
```
❌ Structural Convention Checks FAILED:
[RULE: NO_INLINE_TESTS] `app/src/core/file_kind.rs` contains an inline `mod tests { ... }` block.
  -> Fix: Replace the inline block with `#[cfg(test)] mod tests;` and move the tests into a companion `tests.rs` file.
  -> Ref: docs/agent/CONVENTIONS.md#test-organization
```

### Test C: Forbidden UI Dependency inside `app/src/core/` (`use egui::Color32;`)
```
❌ Structural Convention Checks FAILED:
[RULE: CORE_LAYER_ISOLATION] `app/src/core/file_kind.rs` imports or references forbidden dependency `egui`.
  -> Fix: `app/src/core/` must remain pure domain logic with zero GUI, OS, or async runtime imports. Inject external data via traits.
  -> Ref: app/src/core/AGENTS.md and docs/agent/CONVENTIONS.md#layer-boundaries
```

### Test D: Broken File Path in Agent Documentation
```
❌ Structural Convention Checks FAILED:
[RULE: BROKEN_DOC_PATH] `AGENTS.md` references non-existent path `app/src/does_not_exist.rs`.
  -> Fix: Update `AGENTS.md` to point to the actual path on disk.
  -> Ref: AGENTS.md#8-token-saving-reading-protocol--maintenance-rule
```

---

## 4. Full Verification Gate Result

Running `./scripts/check.sh` executes all four gates:
1. `cargo fmt --check --manifest-path app/Cargo.toml`
2. `cargo clippy --all-targets --manifest-path app/Cargo.toml -- -D warnings`
3. `python3 scripts/verify_structural_rules.py`
4. `cargo test --manifest-path app/Cargo.toml` (62 tests: 24 unit, 38 integration)

**Result:** `✅ All checks passed!`

