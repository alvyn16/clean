#!/usr/bin/env bash
# Local, user-level install for Omarchy / Arch. No root required.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PREFIX="${PREFIX:-$HOME/.local}"

command -v cargo >/dev/null || { echo "cargo is required" >&2; exit 1; }

echo "Building clean (release)…"
CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-8}" cargo build --release --manifest-path "$ROOT/Cargo.toml" --locked

install -Dm755 "$ROOT/target/release/clean" "$PREFIX/bin/clean"
install -Dm644 "$ROOT/clean.desktop" "$PREFIX/share/applications/clean.desktop"
install -Dm644 "$ROOT/assets/icons/hicolor/256x256/apps/clean.png" \
  "$PREFIX/share/icons/hicolor/256x256/apps/clean.png"
for size in 48 64 128; do
  install -Dm644 "$ROOT/assets/icons/hicolor/${size}x${size}/apps/clean.png" \
    "$PREFIX/share/icons/hicolor/${size}x${size}/apps/clean.png"
done

echo "Installed to $PREFIX/bin/clean"
echo "Log out and back in (or run 'update-desktop-database ~/.local/share/applications')."
