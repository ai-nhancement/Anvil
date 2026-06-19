//! Self-update support.
//!
//! Checks GitHub Releases for a newer `anvil` and replaces the running binary in
//! place. Powers three entry points:
//!   - `anvil update` (CLI, headless)
//!   - the TUI `/update` command
//!   - the boot-time check that drives the "update available" header indicator
//!
//! All network/replace work is synchronous (self_update uses blocking reqwest),
//! so callers inside the async runtime must wrap these in `spawn_blocking`.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const REPO_OWNER: &str = "ai-nhancement";
const REPO_NAME: &str = "Anvil";
const BIN: &str = "anvil";

/// How long a successful check is trusted before we hit the network again.
/// Kept short during the rapid-release phase (we've shipped several versions in
/// a day) so users see new releases promptly; lengthen once releases stabilize.
const CACHE_TTL_SECS: i64 = 30 * 60;

/// The version compiled into this binary (from Cargo.toml). Must be bumped to
/// match each release tag for the check to be meaningful.
pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Release-asset target triple for the host, matching the names produced by
/// `.github/workflows/release.yml` (`anvil-<target>.tar.gz` / `.zip`). We ship
/// static musl binaries for Linux, so map Linux hosts to the musl target.
pub fn host_target() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => Some("x86_64-pc-windows-msvc"),
        ("macos", "x86_64") => Some("x86_64-apple-darwin"),
        ("macos", "aarch64") => Some("aarch64-apple-darwin"),
        ("linux", "x86_64") => Some("x86_64-unknown-linux-musl"),
        ("linux", "aarch64") => Some("aarch64-unknown-linux-musl"),
        _ => None,
    }
}

/// Returns `Some(version)` when `latest` is strictly newer than this binary.
fn newer_or_none(latest: &str) -> Option<String> {
    match self_update::version::bump_is_greater(current_version(), latest) {
        Ok(true) => Some(latest.to_string()),
        _ => None,
    }
}

/// Blocking: ask GitHub for the newest release version (tag without the leading `v`).
fn fetch_latest_version() -> Result<String> {
    let releases = self_update::backends::github::ReleaseList::configure()
        .repo_owner(REPO_OWNER)
        .repo_name(REPO_NAME)
        .build()?
        .fetch()?;
    let latest = releases
        .first()
        .ok_or_else(|| anyhow!("no releases found for {}/{}", REPO_OWNER, REPO_NAME))?;
    Ok(latest.version.clone())
}

#[derive(Serialize, Deserialize)]
struct UpdateCache {
    checked_at: String, // RFC3339
    latest: String,     // most recent release version seen
}

fn cache_path(root: &Path) -> PathBuf {
    root.join(crate::config::ANVIL_DIR)
        .join("update-check.json")
}

/// Read the cached latest-version if the cache is younger than the TTL.
fn read_fresh_cache(root: &Path) -> Option<String> {
    let raw = std::fs::read_to_string(cache_path(root)).ok()?;
    let cache: UpdateCache = serde_json::from_str(&raw).ok()?;
    let when = chrono::DateTime::parse_from_rfc3339(&cache.checked_at).ok()?;
    let age = chrono::Utc::now().signed_duration_since(when).num_seconds();
    if (0..CACHE_TTL_SECS).contains(&age) {
        Some(cache.latest)
    } else {
        None
    }
}

fn write_cache(root: &Path, latest: &str) {
    let cache = UpdateCache {
        checked_at: chrono::Utc::now().to_rfc3339(),
        latest: latest.to_string(),
    };
    if let Ok(json) = serde_json::to_string(&cache) {
        let _ = std::fs::create_dir_all(root.join(crate::config::ANVIL_DIR));
        let _ = std::fs::write(cache_path(root), json);
    }
}

/// Blocking: returns `Some(version)` if a newer release is available. Uses a
/// short on-disk cache (see `CACHE_TTL_SECS`) to avoid hitting the GitHub API on
/// every launch, and is a no-op (returns None) when `ANVIL_NO_UPDATE_CHECK` is
/// set. Network/parse
/// failures are swallowed (returns None) — an update check must never break boot.
pub fn check_with_cache_blocking(root: &Path) -> Option<String> {
    if std::env::var_os("ANVIL_NO_UPDATE_CHECK").is_some() {
        return None;
    }
    if let Some(latest) = read_fresh_cache(root) {
        return newer_or_none(&latest);
    }
    match fetch_latest_version() {
        Ok(latest) => {
            write_cache(root, &latest);
            newer_or_none(&latest)
        }
        Err(_) => None,
    }
}

/// Blocking: download the latest release asset for the host target and replace
/// the running binary. Returns the version installed.
pub fn apply_update_blocking() -> Result<String> {
    let target = host_target().ok_or_else(|| {
        anyhow!(
            "no prebuilt anvil binary for this platform ({}/{}) — build from source instead",
            std::env::consts::OS,
            std::env::consts::ARCH
        )
    })?;
    let status = self_update::backends::github::Update::configure()
        .repo_owner(REPO_OWNER)
        .repo_name(REPO_NAME)
        .bin_name(BIN)
        .target(target)
        .current_version(current_version())
        .show_download_progress(false)
        .show_output(false)
        .no_confirm(true)
        .build()?
        .update()?;
    Ok(status.version().to_string())
}
