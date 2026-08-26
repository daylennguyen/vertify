# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

- GitHub Releases are published automatically on every push to `main`
- Releases are published only for `daylennguyen/vertify` (not a GoDaddy identity)
- CLI: added `--ffmpeg-arg` passthrough, `--preset`, `--output-dir`, `--suffix`, `--audio-mode`, `--audio-bitrate`, `--map-metadata`, `--start`, `--duration`, `--loglevel`, `--no-faststart`, `--json-plan`, and `--open`
- GUI: added dry-run command copy action, open output folder action, custom long-edge input, richer color presets with inline color validation, reset defaults, target swap, keyboard shortcut help panel, and local persistence for last-used settings

## [0.1.1] — 2026-08-18

### Added

- Official releases vendor `ffmpeg` and `ffprobe` next to the Vertify binaries — no separate ffmpeg install for zip/installer users
- Windows installer (`VertifySetup-*-windows-x64.exe`): per-user install, Start Menu shortcut, optional PATH
- `VERTIFY_FFMPEG_DIR` override, plus lookup next to the executable (`./`, `./ffmpeg/`, `./bin/`) before PATH

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

[0.1.1]: https://github.com/daylennguyen/vertify/releases/tag/v0.1.1
[0.1.0]: https://github.com/daylennguyen/vertify/releases/tag/v0.1.0
