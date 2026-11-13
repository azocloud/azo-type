#!/usr/bin/env bash
# Generates HTML, LCOV, and JSON summary test coverage reports in app/coverage/
# using cargo-llvm-cov, prints coverage percentages, and compares against
# app/coverage/baseline-summary.json when present.

set -euo pipefail

export PATH="$HOME/.cargo/bin:$PATH"

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_DIR="$REPO_ROOT/app"
COVERAGE_DIR="$APP_DIR/coverage"

if ! cargo llvm-cov --version >/dev/null 2>&1; then
    echo "Error: cargo-llvm-cov is not installed or not in PATH." >&2
    echo "Install it with:" >&2
    echo "  rustup component add llvm-tools-preview && cargo install cargo-llvm-cov" >&2
    exit 1
fi

mkdir -p "$COVERAGE_DIR"
cd "$APP_DIR"

echo "==> [1/3] Running instrumented tests and generating HTML report..."
cargo llvm-cov --workspace --html --output-dir coverage

echo "==> [2/3] Generating LCOV report..."
cargo llvm-cov report --lcov --output-path coverage/lcov.info

echo "==> [3/3] Generating JSON summary..."
cargo llvm-cov report --json --summary-only --output-path coverage/summary.json

echo ""
echo "==> Coverage Summary (app/coverage/summary.json):"
python3 - "$COVERAGE_DIR/summary.json" "$COVERAGE_DIR/baseline-summary.json" <<'PY'
import json
import sys
from pathlib import Path

summary_path = Path(sys.argv[1])
baseline_path = Path(sys.argv[2])

def load_totals(path: Path) -> dict:
    data = json.loads(path.read_text(encoding="utf-8"))
    entries = data.get("data") or []
    if not entries:
        raise ValueError(f"No coverage data found in {path}")
    return entries[0].get("totals", {})

current = load_totals(summary_path)
baseline = load_totals(baseline_path) if baseline_path.exists() else None

metrics = [
    ("Lines", "lines"),
    ("Functions", "functions"),
    ("Regions", "regions"),
    ("Branches", "branches"),
]

for label, key in metrics:
    cur = current.get(key)
    if not isinstance(cur, dict):
        continue
    count = cur.get("count", 0)
    covered = cur.get("covered", 0)
    pct = cur.get("percent", 0.0)
    if key == "branches" and count == 0:
        print(f"  {label:<10}: n/a (0 instrumented branches)")
        continue

    delta_str = ""
    if baseline and isinstance(baseline.get(key), dict):
        base_cur = baseline[key]
        if not (key == "branches" and base_cur.get("count", 0) == 0):
            diff = pct - base_cur.get("percent", 0.0)
            delta_str = f"  (vs baseline: {diff:+.2f}%)"

    print(f"  {label:<10}: {pct:6.2f}% ({covered}/{count}){delta_str}")

if baseline is None:
    print("\nTip: Save a baseline to track coverage deltas across commits:")
    print("  cp app/coverage/summary.json app/coverage/baseline-summary.json")
PY

echo ""
echo "Reports generated:"
echo "  - HTML: app/coverage/html/index.html"
echo "  - LCOV: app/coverage/lcov.info"
echo "  - JSON: app/coverage/summary.json"

