#!/usr/bin/env sh
# One-shot installer for vertify. Safe to re-run after source changes.
set -e

if ! command -v cargo >/dev/null 2>&1; then
    echo "error: cargo (Rust) is required. Install it from https://rustup.rs" >&2
    exit 1
fi

echo "Building and installing vertify via cargo..."
cargo install --path "$(dirname "$0")" --locked --force

if ! command -v ffmpeg >/dev/null 2>&1; then
    echo ""
    echo "WARNING: ffmpeg was not found on your PATH. vertify needs it at runtime."
    echo "  macOS:  brew install ffmpeg"
    echo "  Ubuntu: sudo apt install ffmpeg"
    echo "  Other:  https://ffmpeg.org/download.html"
fi

echo ""
echo "Installed. Make sure ~/.cargo/bin is on your PATH, then run: vertify --help"
echo "GUI: vertify-gui"
