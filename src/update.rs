//! Self-update: check GitHub Releases and replace the running binary.
//!
//! Uses `curl` (always available on the supported platforms) to avoid adding
//! an HTTP-client dependency. JSON is extracted with simple string scanning —
//! the GitHub API response format is stable enough that a full JSON parser
//! would be overkill here.

use anyhow::{bail, Context, Result};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// A single release asset (archive file) from the GitHub Releases API.
#[derive(Clone, Debug, PartialEq)]
pub struct ReleaseAsset {
    /// File name, e.g. `vertify-v0.2.0-abc1234-linux-x64.tar.gz`
    pub name: String,
    /// Direct download URL.
    pub browser_download_url: String,
}

/// Parsed information from the latest GitHub release.
#[derive(Clone, Debug)]
pub struct ReleaseInfo {
    /// Semver string extracted from the tag, e.g. `"0.2.0"`.
    pub version: String,
    /// All release assets.
    pub assets: Vec<ReleaseAsset>,
}

/// Default GitHub Releases API endpoint. Override with `VERTIFY_UPDATE_URL`
/// (useful for tests and air-gapped environments).
const RELEASES_URL: &str = "https://api.github.com/repos/daylennguyen/vertify/releases/latest";

// ── Public API ──────────────────────────────────────────────────────────────

/// Return the compile-time package version (`CARGO_PKG_VERSION`).
pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Return `true` when `latest` is a strictly higher semver than `current`.
///
/// Comparison is major.minor.patch; pre-release suffixes are ignored.
pub fn is_newer(latest: &str, current: &str) -> bool {
    match (parse_semver(latest), parse_semver(current)) {
        (Some(l), Some(c)) => l > c,
        _ => false,
    }
}

/// Choose the release asset that matches the current platform, if any.
///
/// Asset-name conventions (from `release.yml`):
/// - Linux x64   → `*-linux-x64.tar.gz`
/// - Windows x64 → `*-windows-x64.zip`
/// - macOS arm64 → `*-macos-arm64.tar.gz`
///
/// Installer `.exe` files are excluded.
pub fn select_asset(assets: &[ReleaseAsset]) -> Option<&ReleaseAsset> {
    let suffix = platform_asset_suffix();
    if suffix.is_empty() {
        return None;
    }
    assets
        .iter()
        .find(|a| a.name.contains(suffix) && !a.name.ends_with(".exe"))
}

/// Fetch the latest release from GitHub (or `VERTIFY_UPDATE_URL`).
///
/// Requires `curl` to be available on `PATH`.
pub fn fetch_latest_release() -> Result<ReleaseInfo> {
    let url = env::var("VERTIFY_UPDATE_URL").unwrap_or_else(|_| RELEASES_URL.to_string());

    let out = Command::new("curl")
        .args([
            "--silent",
            "--show-error",
            "--location",
            "--max-time",
            "15",
            "-H",
            "Accept: application/vnd.github+json",
            "-H",
            "X-GitHub-Api-Version: 2022-11-28",
            "--user-agent",
            &format!("vertify/{}", current_version()),
            &url,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .context("could not run curl — is it installed?")?;

    if !out.status.success() {
        bail!(
            "curl failed ({}): {}",
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }

    let body = String::from_utf8_lossy(&out.stdout);
    parse_release_json(&body)
}

/// Download `asset`, extract the vertify binary, and replace `std::env::current_exe()`.
///
/// On Unix the replacement is atomic (write `.new` → `rename`). On Windows the
/// current executable is renamed to `.old` first (NTFS allows renaming a running
/// binary) and then the new binary is copied into place. The `.old` file is
/// removed on subsequent runs.
pub fn download_and_replace(asset: &ReleaseAsset) -> Result<()> {
    let current_exe = env::current_exe().context("could not determine current executable path")?;

    // Clean up any leftover from a previous Windows update before starting.
    #[cfg(target_os = "windows")]
    clean_old_windows_binary(&current_exe);

    let exe_filename = current_exe
        .file_name()
        .and_then(|n| n.to_str())
        .context("could not get executable filename")?
        .to_string();

    // Work inside a per-PID temp directory.
    let tmp_dir = env::temp_dir().join(format!("vertify-update-{}", std::process::id()));
    fs::create_dir_all(&tmp_dir)
        .with_context(|| format!("could not create temp dir {}", tmp_dir.display()))?;

    // ── Download ─────────────────────────────────────────────────────────────
    let archive_path = tmp_dir.join(&asset.name);
    let status = Command::new("curl")
        .args([
            "--silent",
            "--show-error",
            "--location",
            "--max-time",
            "120",
            "--user-agent",
            &format!("vertify/{}", current_version()),
            "-o",
        ])
        .arg(&archive_path)
        .arg(&asset.browser_download_url)
        .status()
        .context("could not run curl for download")?;
    if !status.success() {
        bail!("download failed");
    }

    // ── Extract ───────────────────────────────────────────────────────────────
    let extract_dir = tmp_dir.join("extracted");
    fs::create_dir_all(&extract_dir)?;

    let name = &asset.name;
    if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
        extract_tar_gz(&archive_path, &extract_dir)?;
    } else if name.ends_with(".zip") {
        extract_zip(&archive_path, &extract_dir)?;
    } else {
        bail!("unknown archive format: {}", name);
    }

    // ── Locate the binary inside the extracted tree ───────────────────────────
    let new_binary = find_file_in_dir(&extract_dir, &exe_filename)
        .with_context(|| format!("could not find {} in the downloaded archive", exe_filename))?;

    // ── Replace current exe ───────────────────────────────────────────────────
    replace_exe(&new_binary, &current_exe)?;

    // Best-effort cleanup.
    let _ = fs::remove_dir_all(&tmp_dir);

    Ok(())
}

// ── Internal helpers ─────────────────────────────────────────────────────────

fn platform_asset_suffix() -> &'static str {
    if cfg!(target_os = "linux") {
        "linux-x64"
    } else if cfg!(target_os = "windows") {
        "windows-x64"
    } else if cfg!(target_os = "macos") {
        "macos-arm64"
    } else {
        ""
    }
}

fn parse_semver(s: &str) -> Option<(u32, u32, u32)> {
    // Strip a leading `v` and any build-metadata suffix after the first `-`.
    let s = s.trim_start_matches('v');
    let s = s.split('-').next().unwrap_or(s);
    let mut parts = s.split('.');
    let major: u32 = parts.next()?.parse().ok()?;
    let minor: u32 = parts.next()?.parse().ok()?;
    let patch: u32 = parts.next()?.parse().ok()?;
    Some((major, minor, patch))
}

/// Parse the GitHub Releases API JSON response.
fn parse_release_json(json: &str) -> Result<ReleaseInfo> {
    let tag_name = extract_json_string(json, "tag_name")
        .context("could not find tag_name in GitHub API response")?;
    // Tag is like `v0.2.0-abc1234`; strip the SHA slug to get the semver part.
    let version = parse_semver(&tag_name)
        .map(|(ma, mi, pa)| format!("{ma}.{mi}.{pa}"))
        .context("could not parse semver from tag")?;

    let assets = parse_assets(json);
    Ok(ReleaseInfo { version, assets })
}

/// Extract the value of `"key":"<value>"` (or `"key": "<value>"`) from a JSON fragment.
///
/// Handles `\"` escapes inside the value and optional whitespace after `:`.
fn extract_json_string(json: &str, key: &str) -> Option<String> {
    let needle = format!("\"{}\":", key);
    let colon_pos = json.find(&needle)? + needle.len();
    // Skip any whitespace after the colon, then expect an opening double-quote.
    let rest = json[colon_pos..].trim_start();
    let rest = rest.strip_prefix('"')?;
    let mut value = String::new();
    let mut escaped = false;
    for c in rest.chars() {
        if escaped {
            // Preserve the literal char after a backslash.
            value.push(c);
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == '"' {
            break;
        } else {
            value.push(c);
        }
    }
    Some(value)
}

/// Locate the last occurrence of `"key": "..."` (or `"key":"..."`) in `s`.
fn find_last_json_string(s: &str, key: &str) -> Option<String> {
    let needle = format!("\"{}\":", key);
    let mut last = None;
    let mut pos = 0;
    while let Some(rel) = s[pos..].find(&needle) {
        let abs = pos + rel + needle.len();
        let rest = s[abs..].trim_start();
        if let Some(rest) = rest.strip_prefix('"') {
            let mut value = String::new();
            let mut escaped = false;
            for c in rest.chars() {
                if escaped {
                    value.push(c);
                    escaped = false;
                } else if c == '\\' {
                    escaped = true;
                } else if c == '"' {
                    break;
                } else {
                    value.push(c);
                }
            }
            last = Some(value);
        }
        pos = pos + rel + needle.len();
    }
    last
}

/// Parse the `assets` array from a GitHub Releases API response.
fn parse_assets(json: &str) -> Vec<ReleaseAsset> {
    let mut assets = Vec::new();
    // We scan for each browser_download_url and then look backwards for the
    // asset name in the same object.  Both compact and pretty-printed JSON
    // are handled because extract/find_last_json_string tolerate whitespace.
    let marker = "\"browser_download_url\"";
    let mut pos = 0;
    while let Some(rel) = json[pos..].find(marker) {
        let abs = pos + rel;
        // Extract the URL value (handles optional space after colon).
        let url = extract_json_string(&json[abs..], "browser_download_url").unwrap_or_default();

        // The asset `name` field appears before `browser_download_url` in the
        // same object; scan back up to 2 048 bytes to find the last occurrence.
        let window_start = abs.saturating_sub(2048);
        let window = &json[window_start..abs];
        if let Some(name) = find_last_json_string(window, "name") {
            if !name.is_empty() && !url.is_empty() {
                assets.push(ReleaseAsset {
                    name,
                    browser_download_url: url,
                });
            }
        }

        pos = abs + 1;
    }
    assets
}

fn extract_tar_gz(archive: &Path, dest: &Path) -> Result<()> {
    let status = Command::new("tar")
        .args(["-xzf"])
        .arg(archive)
        .arg("-C")
        .arg(dest)
        .status()
        .context("could not run tar (install tar or update manually)")?;
    if !status.success() {
        bail!("tar extraction failed");
    }
    Ok(())
}

fn extract_zip(archive: &Path, dest: &Path) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        let cmd = format!(
            "Expand-Archive -Path '{}' -DestinationPath '{}' -Force",
            archive.display(),
            dest.display()
        );
        let status = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &cmd])
            .status()
            .context("could not run PowerShell Expand-Archive")?;
        if !status.success() {
            bail!("PowerShell Expand-Archive failed");
        }
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let status = Command::new("unzip")
            .arg(archive)
            .arg("-d")
            .arg(dest)
            .status()
            .context("could not run unzip")?;
        if !status.success() {
            bail!("unzip failed");
        }
        Ok(())
    }
}

/// Recursively find a file with the given name in `dir`.
fn find_file_in_dir(dir: &Path, name: &str) -> Option<PathBuf> {
    let entries = fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_file_in_dir(&path, name) {
                return Some(found);
            }
        } else if path.file_name().and_then(|n| n.to_str()) == Some(name) {
            return Some(path);
        }
    }
    None
}

/// Copy `new_binary` over `current_exe` as atomically as possible.
fn replace_exe(new_binary: &Path, current_exe: &Path) -> Result<()> {
    let parent = current_exe
        .parent()
        .context("executable has no parent directory")?;
    let exe_filename = current_exe
        .file_name()
        .context("executable has no filename")?
        .to_string_lossy();

    // Stage in the same directory so the rename is on the same filesystem.
    let staging = parent.join(format!(".{}.new", exe_filename));
    fs::copy(new_binary, &staging).context("failed to stage new binary")?;

    // Make the staged file executable on Unix.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&staging, fs::Permissions::from_mode(0o755))
            .context("failed to set executable permissions")?;
    }

    #[cfg(not(target_os = "windows"))]
    {
        // Atomic rename: the kernel keeps the old inode mapped into running
        // processes; the new file becomes visible immediately.
        fs::rename(&staging, current_exe).context("failed to replace binary")?;
    }

    #[cfg(target_os = "windows")]
    {
        // Windows prevents deleting a running .exe but allows renaming it.
        // Rename current → .old, then rename staged → current.
        let old_path = parent.join(format!("{}.old", exe_filename));
        let _ = fs::remove_file(&old_path); // clean up previous leftover
        fs::rename(current_exe, &old_path).context("failed to move current executable aside")?;
        fs::rename(&staging, current_exe).context("failed to put new executable in place")?;
    }

    Ok(())
}

/// Remove a leftover `.old` binary from a previous Windows update.
#[cfg(target_os = "windows")]
fn clean_old_windows_binary(current_exe: &Path) {
    if let Some(parent) = current_exe.parent() {
        if let Some(name) = current_exe.file_name().and_then(|n| n.to_str()) {
            let old = parent.join(format!("{}.old", name));
            let _ = fs::remove_file(old);
        }
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_newer_detects_upgrade() {
        assert!(is_newer("0.2.0", "0.1.1"));
        assert!(is_newer("1.0.0", "0.9.99"));
        assert!(is_newer("0.1.2", "0.1.1"));
    }

    #[test]
    fn is_newer_equal_is_not_newer() {
        assert!(!is_newer("0.1.1", "0.1.1"));
    }

    #[test]
    fn is_newer_older_is_not_newer() {
        assert!(!is_newer("0.1.0", "0.1.1"));
        assert!(!is_newer("0.0.9", "0.1.0"));
    }

    #[test]
    fn is_newer_strips_v_prefix_and_sha_slug() {
        // Tags look like "v0.2.0-abc1234"
        assert!(is_newer("0.2.0-abc1234", "0.1.1"));
        assert!(!is_newer("0.1.1-xyz9999", "0.1.1"));
    }

    #[test]
    fn current_version_matches_cargo_pkg_version() {
        let cv = current_version();
        assert!(!cv.is_empty());
        assert_eq!(cv, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn select_asset_finds_platform_archive() {
        let assets = vec![
            ReleaseAsset {
                name: "vertify-v0.2.0-abc-linux-x64.tar.gz".into(),
                browser_download_url: "https://example.com/linux.tar.gz".into(),
            },
            ReleaseAsset {
                name: "vertify-v0.2.0-abc-windows-x64.zip".into(),
                browser_download_url: "https://example.com/windows.zip".into(),
            },
            ReleaseAsset {
                name: "VertifySetup-0.2.0-abc-windows-x64.exe".into(),
                browser_download_url: "https://example.com/setup.exe".into(),
            },
            ReleaseAsset {
                name: "vertify-v0.2.0-abc-macos-arm64.tar.gz".into(),
                browser_download_url: "https://example.com/macos.tar.gz".into(),
            },
        ];

        let found = select_asset(&assets);

        #[cfg(target_os = "linux")]
        assert_eq!(
            found.map(|a| a.name.as_str()),
            Some("vertify-v0.2.0-abc-linux-x64.tar.gz")
        );

        #[cfg(target_os = "windows")]
        assert_eq!(
            found.map(|a| a.name.as_str()),
            Some("vertify-v0.2.0-abc-windows-x64.zip")
        );

        #[cfg(target_os = "macos")]
        assert_eq!(
            found.map(|a| a.name.as_str()),
            Some("vertify-v0.2.0-abc-macos-arm64.tar.gz")
        );
    }

    #[test]
    fn select_asset_skips_installer_exe() {
        let assets = vec![ReleaseAsset {
            name: "VertifySetup-0.2.0-abc-windows-x64.exe".into(),
            browser_download_url: "https://example.com/setup.exe".into(),
        }];
        #[cfg(target_os = "windows")]
        assert!(select_asset(&assets).is_none());
        #[cfg(not(target_os = "windows"))]
        let _ = assets; // not applicable on non-Windows
    }

    #[test]
    fn parse_release_json_extracts_version_and_assets() {
        let json = r#"{
  "tag_name": "v0.2.0-abc1234",
  "name": "v0.2.0-abc1234",
  "assets": [
    {
      "name": "vertify-v0.2.0-abc1234-linux-x64.tar.gz",
      "browser_download_url": "https://example.com/linux.tar.gz"
    },
    {
      "name": "SHA256SUMS.txt",
      "browser_download_url": "https://example.com/SHA256SUMS.txt"
    }
  ]
}"#;
        let info = parse_release_json(json).unwrap();
        assert_eq!(info.version, "0.2.0");
        assert_eq!(info.assets.len(), 2);
        assert_eq!(
            info.assets[0].name,
            "vertify-v0.2.0-abc1234-linux-x64.tar.gz"
        );
        assert_eq!(
            info.assets[0].browser_download_url,
            "https://example.com/linux.tar.gz"
        );
    }

    #[test]
    fn extract_json_string_handles_basic_key() {
        let json = r#"{"tag_name":"v0.1.1","other":"val"}"#;
        assert_eq!(
            extract_json_string(json, "tag_name").as_deref(),
            Some("v0.1.1")
        );
    }

    #[test]
    fn extract_json_string_returns_none_for_missing_key() {
        let json = r#"{"foo":"bar"}"#;
        assert!(extract_json_string(json, "tag_name").is_none());
    }
}
