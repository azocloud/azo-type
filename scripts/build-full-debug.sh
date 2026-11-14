#!/usr/bin/env bash
# Builds Azo Type with full DWARF/PDB debug symbols across workspace crates and
# third-party dependencies for interactive debugger sessions.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

echo "==> Building Azo Type with full debug symbols (profile: dev-full)..."
cargo build --profile dev-full --manifest-path app/Cargo.toml "$@"

echo "✅ Full debug build ready in target/dev-full/"
