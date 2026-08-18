# One-shot installer for vertify on Windows. Safe to re-run after source changes.
$ErrorActionPreference = "Stop"

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Error "cargo (Rust) is required. Install it from https://rustup.rs"
}

Write-Host "Building and installing vertify via cargo..."
cargo install --path $PSScriptRoot --locked --force

if (-not (Get-Command ffmpeg -ErrorAction SilentlyContinue)) {
    Write-Host ""
    Write-Host "WARNING: ffmpeg was not found on your PATH. vertify needs it at runtime."
    Write-Host "  winget install Gyan.FFmpeg"
    Write-Host "  Other: https://ffmpeg.org/download.html"
}

Write-Host ""
Write-Host "Installed. Make sure cargo's bin dir is on your PATH, then run: vertify --help"
Write-Host "GUI: vertify-gui"
