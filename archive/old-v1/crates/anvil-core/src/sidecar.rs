//! Sidecar daemon lifecycle helpers shared by the CLI and future App crate.
//!
//! Lives in `anvil-core` so both the CLI and the v1.1 App can reuse spawn, probe,
//! and stop logic without duplication.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::error::AnvilError;

/// Locate the `anvil-sidecar` binary.
///
/// Checks `configured` first (from `sidecar.binary_path` in `anvil.toml`), then walks `$PATH`.
///
/// # Errors
///
/// Returns [`AnvilError::SidecarNotFound`] if the binary cannot be found.
pub fn find_sidecar_binary(configured: Option<&Path>) -> Result<PathBuf, AnvilError> {
    if let Some(p) = configured {
        if p.exists() {
            return Ok(p.to_path_buf());
        }
    }
    if let Ok(path_var) = std::env::var("PATH") {
        let name = if cfg!(windows) {
            "anvil-sidecar.exe"
        } else {
            "anvil-sidecar"
        };
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join(name);
            if candidate.exists() {
                return Ok(candidate);
            }
        }
    }
    Err(AnvilError::SidecarNotFound)
}

/// Poll `{workspace}/.anvil/run/sidecar.port` until a valid port is written or `timeout` elapses.
///
/// # Errors
///
/// Returns [`AnvilError::SidecarStartTimeout`] if the file is not written within the deadline.
pub fn wait_for_port_file(workspace: &Path, timeout: Duration) -> Result<u16, AnvilError> {
    let port_path = workspace.join(".anvil/run/sidecar.port");
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if let Ok(contents) = std::fs::read_to_string(&port_path) {
            if let Ok(port) = contents.trim().parse::<u16>() {
                return Ok(port);
            }
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    Err(AnvilError::SidecarStartTimeout)
}

/// Returns `true` if the OS reports the given PID is a live process.
#[cfg(windows)]
#[must_use]
pub fn is_process_alive(pid: u32) -> bool {
    std::process::Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"])
        .output()
        .is_ok_and(|o| String::from_utf8_lossy(&o.stdout).contains(&pid.to_string()))
}

/// Returns `true` if the OS reports the given PID is a live process.
#[cfg(not(windows))]
#[must_use]
pub fn is_process_alive(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Send a graceful termination signal to the process with the given PID.
#[cfg(windows)]
pub fn kill_process(pid: u32) {
    let _ = std::process::Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/F"])
        .status();
}

/// Send a graceful termination signal to the process with the given PID.
#[cfg(not(windows))]
pub fn kill_process(pid: u32) {
    let _ = std::process::Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .status();
}
