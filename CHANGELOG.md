# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] — 2026-08-17

First public release.

### Added

- CLI (`vertify`) that converts landscape 16:9 video to 9:16 and portrait 9:16 video to 16:9 **without cropping**
- Auto-detect target orientation, or force `--to 9:16` / `--to 16:9`
- Fill empty space with a blurred copy of the clip (`--fill blur`) or solid color bars (`--fill color`)
- Desktop Flip Stage GUI (`vertify-gui`) with drag-and-drop, live letterbox preview, and keyboard shortcuts
- `--dry-run` to print the ffmpeg command without encoding
- Shell completions for bash, zsh, fish, PowerShell, and Elvish (`--completions`)
- Audio stream-copy with automatic AAC fallback when the container rejects copy
- `install.sh` / `install.ps1` one-shot installers

[0.1.0]: https://github.com/daylennguyen/vertify/releases/tag/v0.1.0
