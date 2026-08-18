#!/usr/bin/env sh
# Download a portable ffmpeg + ffprobe into $1 (destination directory).
set -eu

DEST="${1:?usage: vendor-ffmpeg.sh DEST_DIR}"
mkdir -p "$DEST"
WORK="$(mktemp -d)"
cleanup() { rm -rf "$WORK"; }
trap cleanup EXIT

OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS-$ARCH" in
    Linux-x86_64|Linux-amd64)
        echo "Downloading static ffmpeg (linux-x64)..."
        curl -fsSL -o "$WORK/ffmpeg.tar.xz" \
            "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-linux64-gpl.tar.xz"
        tar -xJf "$WORK/ffmpeg.tar.xz" -C "$WORK"
        FFMPEG_BIN="$(find "$WORK" -type f -name ffmpeg | head -n 1)"
        FFPROBE_BIN="$(find "$WORK" -type f -name ffprobe | head -n 1)"
        ;;
    Darwin-arm64)
        echo "Downloading ffmpeg (macOS arm64)..."
        curl -fsSL -A "vertify-release" -o "$WORK/ffmpeg.zip" "https://www.osxexperts.net/ffmpeg7arm.zip"
        curl -fsSL -A "vertify-release" -o "$WORK/ffprobe.zip" "https://www.osxexperts.net/ffprobe7arm.zip"
        unzip -qo "$WORK/ffmpeg.zip" -d "$WORK"
        unzip -qo "$WORK/ffprobe.zip" -d "$WORK"
        FFMPEG_BIN="$(find "$WORK" -type f -name ffmpeg | head -n 1)"
        FFPROBE_BIN="$(find "$WORK" -type f -name ffprobe | head -n 1)"
        ;;
    Darwin-x86_64)
        echo "Downloading ffmpeg (macOS x64)..."
        curl -fsSL -o "$WORK/ffmpeg.zip" "https://evermeet.cx/ffmpeg/getrelease/zip"
        curl -fsSL -o "$WORK/ffprobe.zip" "https://evermeet.cx/ffmpeg/getrelease/ffprobe/zip"
        unzip -qo "$WORK/ffmpeg.zip" -d "$WORK"
        unzip -qo "$WORK/ffprobe.zip" -d "$WORK"
        FFMPEG_BIN="$(find "$WORK" -type f -name ffmpeg | head -n 1)"
        FFPROBE_BIN="$(find "$WORK" -type f -name ffprobe | head -n 1)"
        ;;
    *)
        echo "error: no vendored ffmpeg source for $OS-$ARCH" >&2
        exit 1
        ;;
esac

if [ -z "${FFMPEG_BIN:-}" ] || [ -z "${FFPROBE_BIN:-}" ]; then
    echo "error: ffmpeg/ffprobe missing from the downloaded archive" >&2
    exit 1
fi

cp "$FFMPEG_BIN" "$DEST/ffmpeg"
cp "$FFPROBE_BIN" "$DEST/ffprobe"
chmod +x "$DEST/ffmpeg" "$DEST/ffprobe"

THIRD="$DEST/third_party/ffmpeg"
mkdir -p "$THIRD"
printf '%s\n' \
    "FFmpeg is a separate program bundled with official Vertify releases." \
    "License: GPL/LGPL depending on the build (x264 requires GPL)." \
    "https://ffmpeg.org" \
    "https://github.com/BtbN/FFmpeg-Builds" \
    > "$THIRD/SOURCE.txt"

echo "Vendored ffmpeg into $DEST"
