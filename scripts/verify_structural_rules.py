#!/usr/bin/env python3
"""
Mechanical structural convention verifier and PROJECT_MAP.md generator for azo-type.

Enforces:
1. No forbidden generic module names (utils.rs, helpers.rs, misc.rs, common.rs).
2. Every .rs file in app/src/ begins with an inner module doc comment (`//!`).
3. No inline `mod tests { ... }` blocks in production files (only `mod tests;`).
4. File size cap (<= 600 lines for production modules, with explicit frozen caps for legacy orchestrators).
5. Architectural layer dependency direction (e.g., `core/` never imports GUI, async runtime, or outer layers).
6. `docs/agent/PROJECT_MAP.md` is 100% synchronized with `app/src/` modules and their `//!` docs.
7. Every repository file path referenced in `AGENTS.md` and `docs/agent/*.md` exists on disk.
"""

import argparse
import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
APP_SRC = REPO_ROOT / "app" / "src"
APP_TESTS = REPO_ROOT / "app" / "tests"
PROJECT_MAP_PATH = REPO_ROOT / "docs" / "agent" / "PROJECT_MAP.md"

FORBIDDEN_FILENAMES = {"utils.rs", "helpers.rs", "misc.rs", "common.rs", "util.rs", "helper.rs"}
DEFAULT_MAX_LINES = 600

# Explicit frozen line caps for the two existing orchestrators above 600 lines so they cannot grow.
FROZEN_MAX_LINES = {
    "app/src/app.rs": 1100,
    "app/src/collab/session.rs": 800,
}

# Forbidden imports in `app/src/core/`
CORE_FORBIDDEN_IMPORTS = {
    "egui",
    "eframe",
    "rfd",
    "muda",
    "tokio",
    "iroh",
    "arboard",
    "reqwest",
    "crate::app",
    "crate::ui",
    "crate::collab",
    "crate::platform",
    "crate::services",
    "crate::compiler",
    "crate::project",
    "crate::world",
    "crate::highlight",
}

# Forbidden imports in `app/src/platform/`
PLATFORM_FORBIDDEN_IMPORTS = {
    "crate::app",
    "crate::ui",
    "crate::collab",
    "crate::compiler",
    "crate::project",
    "crate::world",
}

# Integration test mapping for modules
INTEGRATION_TEST_MAP = {
    "app/src/lib.rs": ["app/tests/cli_tests.rs"],
    "app/src/main.rs": ["app/tests/cli_tests.rs"],
    "app/src/core/diff.rs": ["app/tests/diff_tests.rs"],
    "app/src/core/file_kind.rs": ["app/tests/project_tests.rs"],
    "app/src/core/outline.rs": ["app/tests/outline_tests.rs"],
    "app/src/core/pdf.rs": ["app/tests/pdf_config_tests.rs"],
    "app/src/project.rs": ["app/tests/project_tests.rs"],
    "app/src/collab/persistence.rs": ["app/tests/persistence_tests.rs"],
    "app/src/core/spellcheck.rs": ["app/tests/language_check_tests.rs"],
    "app/src/core/typst_prose.rs": ["app/tests/language_check_tests.rs"],
    "app/src/language_check/engine.rs": ["app/tests/language_check_tests.rs"],
    "app/src/app.rs": ["app/tests/ui_tests.rs"],
}

MODULE_NOTES = {
    "app/src/core/outline.rs": "Uses `SourceResolver` trait to stay independent of filesystem I/O.",
    "app/src/core/diff.rs": "Converts between Rust `char` indices (`egui`) and UTF-16 offsets (`yrs`).",
    "app/src/platform/dialogs.rs": "Only module using `unsafe` (`block_on_or_spin`); runs `rfd` on background threads.",
    "app/src/platform/menu.rs": "Native `muda` menu bar active only under `#[cfg(target_os = \"macos\")]`.",
    "app/src/collab/protocol.rs": "`SessionTicket` redacts secret bytes in `Debug` impl.",
    "app/src/collab/session.rs": "Runs a dedicated `tokio` runtime on a background thread; communicates via `mpsc`.",
    "app/src/app.rs": "Top-level `eframe::App` state orchestrator; delegates view rendering to `ui/`.",
}


def is_test_file(rel_path: str) -> bool:
    return rel_path.startswith("app/src/tests/") or rel_path.startswith("app/tests/")


def extract_first_module_doc(lines: list[str]) -> str | None:
    for line in lines[:10]:
        stripped = line.strip()
        if stripped.startswith("//!"):
            text = stripped[3:].strip()
            if text:
                return text
    return None


def extract_pub_items(content: str) -> list[str]:
    items = []
    pattern = re.compile(
        r"^\s*pub(?:\(crate\))?\s+(?:const\s+fn|async\s+fn|struct|enum|trait|fn|const|type)\s+([A-Za-z0-9_]+)",
        re.MULTILINE,
    )
    for match in pattern.finditer(content):
        name = match.group(1)
        if name not in items and name != "new":
            items.append(name)
    return items[:6]


def extract_unit_test_paths(path: Path, content: str) -> list[str]:
    paths: list[str] = []
    pattern = re.compile(r'#\[path\s*=\s*"([^"]+_tests\.rs)"\]')
    for match in pattern.finditer(content):
        resolved = (path.parent / match.group(1)).resolve()
        try:
            rel = resolved.relative_to(REPO_ROOT).as_posix()
        except ValueError:
            continue
        if rel not in paths:
            paths.append(rel)
    return paths


def extract_crate_deps(content: str) -> list[str]:
    deps = []
    pattern = re.compile(r"use\s+crate::([a-zA-Z0-9_]+)")
    for match in pattern.finditer(content):
        mod_name = match.group(1)
        if mod_name not in deps:
            deps.append(mod_name)
    return sorted(deps)


def generate_project_map() -> str:
    prod_files = sorted(
        p for p in APP_SRC.rglob("*.rs")
        if not is_test_file(p.relative_to(REPO_ROOT).as_posix())
    )

    out = [
        "# PROJECT_MAP.md — Auto-Generated Module & Architecture Reference",
        "",
        "> **DO NOT EDIT MANUALLY.** This file is generated from `//!` module doc comments and",
        "> verified by `python3 scripts/verify_structural_rules.py`. To regenerate after adding or",
        "> updating a module, run: `python3 scripts/verify_structural_rules.py --write-map`.",
        "",
        "## 1. Workspace & Crate Overview",
        "",
        "| Crate / Directory | Role | Manifest / Entrypoint |",
        "| :--- | :--- | :--- |",
        "| `app/` (`azo-type`) | Native cross-platform desktop GUI IDE for Typst | `app/Cargo.toml`, `app/src/main.rs`, `app/src/lib.rs` |",
        "| `app/tests/` | Black-box integration & characterization test suites | `app/tests/*_tests.rs` |",
        "| `scripts/` | Mechanical convention checks and verification runners | `scripts/check.sh`, `scripts/verify_structural_rules.py` |",
        "| `docs/agent/` | AI agent onboarding, recipes, conventions, and gotchas | `AGENTS.md`, `docs/agent/PROJECT_MAP.md` |",
        "",
        "## 2. Production Modules (`app/src/`)",
        "",
        "| Module Path | Purpose (`//!` Summary) | Key Public Types / Entrypoints | Internal `crate::` Deps | Tests Location | Notes |",
        "| :--- | :--- | :--- | :--- | :--- | :--- |",
    ]

    for path in prod_files:
        rel = path.relative_to(REPO_ROOT).as_posix()
        content = path.read_text(encoding="utf-8")
        lines = content.splitlines()
        doc = extract_first_module_doc(lines) or "(missing //! doc)"
        pub_items = extract_pub_items(content)
        pub_str = ", ".join(f"`{i}`" for i in pub_items) if pub_items else "*(re-exports / entry)*"
        deps = extract_crate_deps(content)
        deps_str = ", ".join(f"`{d}`" for d in deps) if deps else "*(none)*"
        test_paths = extract_unit_test_paths(path, content)
        for integ in INTEGRATION_TEST_MAP.get(rel, []):
            if integ not in test_paths:
                test_paths.append(integ)
        test_loc = ", ".join(f"`{p}`" for p in test_paths) if test_paths else "*(covered via integration/UI)*"
        note = MODULE_NOTES.get(rel, "—")
        out.append(f"| `{rel}` | {doc} | {pub_str} | {deps_str} | {test_loc} | {note} |")

    out.extend([
        "",
        "## 3. Centralized Unit Test Modules (`app/src/tests/`)",
        "",
        "| Test Module Path | Purpose (`//!` Summary) |",
        "| :--- | :--- |",
    ])

    unit_test_files = sorted((APP_SRC / "tests").rglob("*_tests.rs")) if (APP_SRC / "tests").exists() else []
    for path in unit_test_files:
        rel = path.relative_to(REPO_ROOT).as_posix()
        lines = path.read_text(encoding="utf-8").splitlines()
        doc = extract_first_module_doc(lines) or "Unit tests"
        out.append(f"| `{rel}` | {doc} |")

    out.extend([
        "",
        "## 4. Integration & Characterization Suites (`app/tests/`)",
        "",
        "| Suite Path | Tested Domain / Subsystem | Command |",
        "| :--- | :--- | :--- |",
        "| `app/tests/cli_tests.rs` | CLI argument parsing (`--screenshot`, folder paths) | `cargo test --manifest-path app/Cargo.toml --test cli_tests` |",
        "| `app/tests/diff_tests.rs` | UTF-16 diffing, astral emoji offsets, path collisions | `cargo test --manifest-path app/Cargo.toml --test diff_tests` |",
        "| `app/tests/i18n_tests.rs` | Locale normalization, switching, and translation keys | `cargo test --manifest-path app/Cargo.toml --test i18n_tests` |",
        "| `app/tests/language_check_tests.rs` | Offline spellchecking, grammar, and suggestion application | `cargo test --manifest-path app/Cargo.toml --test language_check_tests` |",
        "| `app/tests/outline_tests.rs` | Typst AST, BibTeX, and Markdown outline extraction | `cargo test --manifest-path app/Cargo.toml --test outline_tests` |",
        "| `app/tests/pdf_config_tests.rs` | PDF versions (1.4–2.0) and PDF/A standard validation | `cargo test --manifest-path app/Cargo.toml --test pdf_config_tests` |",
        "| `app/tests/persistence_tests.rs` | CRDT snapshot saving, loading, and path traversal safety | `cargo test --manifest-path app/Cargo.toml --test persistence_tests` |",
        "| `app/tests/project_tests.rs` | DiskProject file CRUD, buffers, dirty flags, entrypoints | `cargo test --manifest-path app/Cargo.toml --test project_tests` |",
        "| `app/tests/ui_tests.rs` | End-to-end headless `egui_kittest` cross-panel UI flows | `cargo test --manifest-path app/Cargo.toml --test ui_tests` |",
        "",
    ])

    return "\n".join(out)


def check_all(write_map: bool = False) -> int:
    if write_map:
        PROJECT_MAP_PATH.parent.mkdir(parents=True, exist_ok=True)
        PROJECT_MAP_PATH.write_text(generate_project_map(), encoding="utf-8")
        print(f"Updated {PROJECT_MAP_PATH.relative_to(REPO_ROOT)}")
        return 0

    errors: list[str] = []

    all_rs_files = sorted(APP_SRC.rglob("*.rs"))
    for path in all_rs_files:
        rel = path.relative_to(REPO_ROOT).as_posix()
        filename = path.name
        content = path.read_text(encoding="utf-8")
        lines = content.splitlines()

        # Rule 1: Forbidden generic filenames
        if filename in FORBIDDEN_FILENAMES:
            errors.append(
                f"[RULE: NO_GENERIC_FILENAMES] Forbidden module name `{rel}`.\n"
                f"  -> Fix: Name modules after their domain responsibility (e.g., `diff.rs`, `outline.rs`), never `{filename}`.\n"
                f"  -> Ref: docs/agent/CONVENTIONS.md#module-naming-and-file-size"
            )

        # Rule 2: Every module in app/src/ must have a //! doc comment
        if extract_first_module_doc(lines) is None:
            errors.append(
                f"[RULE: MISSING_MODULE_DOC] `{rel}` is missing a top-of-file `//!` inner doc comment.\n"
                f"  -> Fix: Add a concise `//!` summary on line 1 explaining the module's responsibility, then run `python3 scripts/verify_structural_rules.py --write-map`.\n"
                f"  -> Ref: docs/agent/CONVENTIONS.md#documentation-rules"
            )

        if rel.endswith("/tests.rs"):
            errors.append(
                f"[RULE: MISPLACED_UNIT_TESTS] `{rel}` uses the deprecated `<module>/tests.rs` layout.\n"
                f"  -> Fix: Move unit tests into `app/src/tests/` (e.g., `app/src/tests/<path>/<module>_tests.rs`) and link via `#[cfg(test)] #[path = \"...\"] mod tests;`.\n"
                f"  -> Ref: docs/agent/CONVENTIONS.md#5-test-organization--naming"
            )

        if not is_test_file(rel):
            # Rule 3: No inline `mod tests {` in production files
            if re.search(r"\bmod\s+tests\s*\{", content):
                errors.append(
                    f"[RULE: NO_INLINE_TESTS] `{rel}` contains an inline `mod tests {{ ... }}` block.\n"
                    f"  -> Fix: Replace the inline block with `#[cfg(test)] #[path = \"...\"] mod tests;` and move the tests into `app/src/tests/`.\n"
                    f"  -> Ref: docs/agent/CONVENTIONS.md#5-test-organization--naming"
                )

            # Rule 4: File size limit
            max_allowed = FROZEN_MAX_LINES.get(rel, DEFAULT_MAX_LINES)
            if len(lines) > max_allowed:
                errors.append(
                    f"[RULE: MAX_FILE_SIZE] `{rel}` has {len(lines)} lines (maximum allowed: {max_allowed}).\n"
                    f"  -> Fix: Split the module by domain sub-responsibility into smaller focused files (< 400 lines recommended).\n"
                    f"  -> Ref: docs/agent/CONVENTIONS.md#module-naming-and-file-size"
                )

            # Rule 5a: Architectural layer boundary for `app/src/core/`
            if rel.startswith("app/src/core/"):
                for forbidden in sorted(CORE_FORBIDDEN_IMPORTS):
                    pattern = rf"\buse\s+{re.escape(forbidden)}\b|\b{re.escape(forbidden)}::"
                    if re.search(pattern, content):
                        errors.append(
                            f"[RULE: CORE_LAYER_ISOLATION] `{rel}` imports or references forbidden dependency `{forbidden}`.\n"
                            f"  -> Fix: `app/src/core/` must remain pure domain logic with zero GUI, OS, or async runtime imports. Inject external data via traits.\n"
                            f"  -> Ref: app/src/core/AGENTS.md and docs/agent/CONVENTIONS.md#layer-boundaries"
                        )
                if re.search(r"\bstd::fs\b", content):
                    errors.append(
                        f"[RULE: CORE_NO_FS_IO] `{rel}` references `std::fs` directly.\n"
                        f"  -> Fix: Decouple filesystem access in `core/` behind a trait such as `SourceResolver`.\n"
                        f"  -> Ref: app/src/core/AGENTS.md"
                    )

            # Rule 5b: Architectural layer boundary for `app/src/platform/`
            if rel.startswith("app/src/platform/"):
                for forbidden in sorted(PLATFORM_FORBIDDEN_IMPORTS):
                    pattern = rf"\buse\s+{re.escape(forbidden)}\b|\b{re.escape(forbidden)}::"
                    if re.search(pattern, content):
                        errors.append(
                            f"[RULE: PLATFORM_LAYER_ISOLATION] `{rel}` imports outer layer `{forbidden}`.\n"
                            f"  -> Fix: `app/src/platform/` may only depend on `core` and OS crates (`rfd`, `muda`), never UI or app state.\n"
                            f"  -> Ref: docs/agent/CONVENTIONS.md#layer-boundaries"
                        )

    # Rule 6: PROJECT_MAP.md freshness check
    expected_map = generate_project_map()
    if not PROJECT_MAP_PATH.exists():
        errors.append(
            f"[RULE: PROJECT_MAP_MISSING] `{PROJECT_MAP_PATH.relative_to(REPO_ROOT)}` does not exist.\n"
            f"  -> Fix: Run `python3 scripts/verify_structural_rules.py --write-map`.\n"
            f"  -> Ref: AGENTS.md"
        )
    else:
        actual_map = PROJECT_MAP_PATH.read_text(encoding="utf-8")
        if actual_map != expected_map:
            errors.append(
                f"[RULE: PROJECT_MAP_OUTDATED] `docs/agent/PROJECT_MAP.md` is out of sync with `app/src/`.\n"
                f"  -> Fix: Run `python3 scripts/verify_structural_rules.py --write-map` and commit the updated `docs/agent/PROJECT_MAP.md`.\n"
                f"  -> Ref: AGENTS.md#8-token-saving-reading-protocol--maintenance-rule"
            )

    # Rule 7: Verify all repo file paths referenced in AGENTS.md and docs/agent/*.md exist
    doc_files = [REPO_ROOT / "AGENTS.md", REPO_ROOT / "app" / "src" / "core" / "AGENTS.md"]
    agent_docs_dir = REPO_ROOT / "docs" / "agent"
    if agent_docs_dir.exists():
        doc_files.extend(sorted(p for p in agent_docs_dir.glob("*.md") if p.name != "VALIDATION.md"))

    path_token_re = re.compile(
        r"`((?:app|docs|scripts|assets|\.gemini)/[A-Za-z0-9_./-]+|"
        r"(?:AGENTS|CLAUDE|GEMINI|README)\.md|rustfmt\.toml|clippy\.toml|\.aider\.conf\.yml)`"
    )
    for doc_file in doc_files:
        if not doc_file.exists():
            continue
        doc_rel = doc_file.relative_to(REPO_ROOT).as_posix()
        doc_text = doc_file.read_text(encoding="utf-8")
        for match in path_token_re.finditer(doc_text):
            candidate = match.group(1).rstrip("/")
            # Skip glob/placeholder patterns and gitignored generated output paths
            if (
                any(ch in candidate for ch in ("*", "<", ">"))
                or "foo" in candidate
                or candidate == "app/coverage"
                or candidate.startswith("app/coverage/")
            ):
                continue
            target = REPO_ROOT / candidate
            if not target.exists():
                errors.append(
                    f"[RULE: BROKEN_DOC_PATH] `{doc_rel}` references non-existent path `{candidate}`.\n"
                    f"  -> Fix: Update `{doc_rel}` to point to the actual path on disk.\n"
                    f"  -> Ref: AGENTS.md#8-token-saving-reading-protocol--maintenance-rule"
                )

    if errors:
        print("❌ Structural Convention Checks FAILED:\n", file=sys.stderr)
        for err in errors:
            print(err + "\n", file=sys.stderr)
        return 1

    print("✅ All structural convention checks passed (module docs, no inline tests, file sizes, layer boundaries, PROJECT_MAP sync, doc paths).")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description="Verify structural rules or regenerate PROJECT_MAP.md.")
    parser.add_argument("--write-map", action="store_true", help="Regenerate docs/agent/PROJECT_MAP.md from source.")
    args = parser.parse_args()
    return check_all(write_map=args.write_map)


if __name__ == "__main__":
    sys.exit(main())
