#!/usr/bin/env bash
# Unified quality & structural gate for Azo Type.
# Runs formatting, strict clippy lints, full test suite, and structural convention verification.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

echo "==> [1/4] Checking Rust code formatting (cargo fmt --check)..."
cargo fmt --check --manifest-path app/Cargo.toml

echo "==> [2/4] Running Clippy linter (cargo clippy --all-targets -- -D warnings)..."
cargo clippy --all-targets --manifest-path app/Cargo.toml -- -D warnings

echo "==> [3/4] Verifying structural conventions & PROJECT_MAP.md freshness..."
python3 scripts/verify_structural_rules.py

echo "==> [4/4] Running unit and integration test suites (cargo test)..."
cargo test --manifest-path app/Cargo.toml

echo "✅ All checks passed!"
