#!/usr/bin/env bash
# Builds and installs azo-type desktop launcher, binary, and icons for Linux.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "==> Building azo-type in release mode..."
cargo build --release --manifest-path "$REPO_ROOT/app/Cargo.toml"

BIN_TARGET="$HOME/.local/bin"
APPS_TARGET="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
ICONS_TARGET="${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor"

mkdir -p "$BIN_TARGET"
mkdir -p "$APPS_TARGET"

echo "==> Installing binary to $BIN_TARGET/azo-type..."
cp "$REPO_ROOT/app/target/release/azo-type" "$BIN_TARGET/azo-type"
chmod +x "$BIN_TARGET/azo-type"

echo "==> Installing desktop entry to $APPS_TARGET/azo-type.desktop..."
cp "$REPO_ROOT/app/azo-type.desktop" "$APPS_TARGET/azo-type.desktop"

echo "==> Installing icons to $ICONS_TARGET..."
for size in 16 32 48 64 128 256; do
    mkdir -p "$ICONS_TARGET/${size}x${size}/apps"
    cp "$REPO_ROOT/app/icons/win-${size}.png" "$ICONS_TARGET/${size}x${size}/apps/azo-type.png"
done

mkdir -p "$ICONS_TARGET/512x512/apps"
cp "$REPO_ROOT/app/icons/icon.png" "$ICONS_TARGET/512x512/apps/azo-type.png"

mkdir -p "$ICONS_TARGET/1024x1024/apps"
cp "$REPO_ROOT/app/icons/icon-1024.png" "$ICONS_TARGET/1024x1024/apps/azo-type.png"

mkdir -p "$ICONS_TARGET/scalable/apps"
cp "$REPO_ROOT/app/icons/icon.svg" "$ICONS_TARGET/scalable/apps/azo-type.svg"

if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$APPS_TARGET" || true
fi

if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -f -t "$ICONS_TARGET" || true
fi

if command -v kbuildsycoca6 >/dev/null 2>&1; then
    kbuildsycoca6 || true
fi

echo "✅ Azo Type desktop application installed successfully!"

