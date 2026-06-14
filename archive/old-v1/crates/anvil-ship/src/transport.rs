//! Transport actions for `anvil ship` (P9).
//!
//! Transport actions are shell commands declared in `anvil.toml` under
//! `[[transport_actions]]`. They are executed in order when `anvil ship` runs;
//! the first failure aborts the sequence.

use std::path::Path;
use std::process::Command;

use anvil_core::{config::AnvilConfig, error::AnvilError};

pub use anvil_core::config::TransportAction;

/// Extracts the transport action list from the project config.
///
/// The list is ordered; actions are executed in declaration order by
/// [`execute_transport`]. An empty list is valid — `anvil ship` succeeds without
/// running any external command.
#[must_use]
pub fn parse_transport_actions(config: &AnvilConfig) -> Vec<TransportAction> {
    config.transport_actions.clone()
}

/// Executes transport actions in declared order.
///
/// Each action runs as a shell command in `project_root`. Actions are invoked via
/// the system shell (`cmd /C` on Windows, `sh -c` on other platforms).
///
/// Returns [`AnvilError::TransportFailed`] on the first action that fails to spawn
/// or exits with a non-zero status. Successfully completed actions are not retried
/// or rolled back.
///
/// # Errors
///
/// Returns [`AnvilError::TransportFailed`] if any action fails.
pub fn execute_transport(
    actions: &[TransportAction],
    project_root: &Path,
) -> Result<(), AnvilError> {
    for action in actions {
        let label = action.label.as_deref().unwrap_or(&action.command);
        println!("  Transport: {label}");

        let status = run_shell_command(&action.command, project_root);

        match status {
            Err(e) => {
                return Err(AnvilError::TransportFailed {
                    action: label.to_owned(),
                    reason: format!("failed to spawn: {e}"),
                });
            }
            Ok(s) if !s.success() => {
                return Err(AnvilError::TransportFailed {
                    action: label.to_owned(),
                    reason: format!("exited with {s}"),
                });
            }
            Ok(_) => {}
        }
    }
    Ok(())
}

#[cfg(windows)]
fn run_shell_command(command: &str, cwd: &Path) -> std::io::Result<std::process::ExitStatus> {
    Command::new("cmd")
        .args(["/C", command])
        .current_dir(cwd)
        .status()
}

#[cfg(not(windows))]
fn run_shell_command(command: &str, cwd: &Path) -> std::io::Result<std::process::ExitStatus> {
    Command::new("sh")
        .args(["-c", command])
        .current_dir(cwd)
        .status()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use anvil_core::config::{AnvilConfig, TransportAction, TransportKind};

    fn config_with_actions(cmds: &[(&str, &str)]) -> AnvilConfig {
        let mut cfg = AnvilConfig::default_locked();
        cfg.transport_actions = cmds
            .iter()
            .map(|(label, cmd)| TransportAction {
                kind: TransportKind::Shell,
                command: cmd.to_string(),
                label: Some(label.to_string()),
            })
            .collect();
        cfg
    }

    #[test]
    fn test_parse_transport_actions_empty() {
        let cfg = AnvilConfig::default_locked();
        assert!(
            parse_transport_actions(&cfg).is_empty(),
            "default config must have no transport actions"
        );
    }

    #[test]
    fn test_parse_transport_actions_returns_all() {
        let cfg = config_with_actions(&[("Stage", "git add -A"), ("Commit", "git commit -m x")]);
        let actions = parse_transport_actions(&cfg);
        assert_eq!(actions.len(), 2);
        assert_eq!(actions[0].command, "git add -A");
        assert_eq!(actions[1].command, "git commit -m x");
    }

    #[test]
    fn test_execute_transport_empty_succeeds() {
        let tmp = tempfile::TempDir::new().unwrap();
        // Empty action list must succeed without error.
        execute_transport(&[], tmp.path()).unwrap();
    }

    #[test]
    fn test_execute_transport_failing_command_errors() {
        let tmp = tempfile::TempDir::new().unwrap();
        let actions = vec![TransportAction {
            kind: TransportKind::Shell,
            command: "exit 1".to_owned(),
            label: Some("Fail".to_owned()),
        }];
        let err = execute_transport(&actions, tmp.path()).unwrap_err();
        assert!(
            matches!(err, AnvilError::TransportFailed { .. }),
            "failing shell command must produce TransportFailed"
        );
    }

    #[test]
    fn test_execute_transport_succeeding_command_ok() {
        let tmp = tempfile::TempDir::new().unwrap();
        // Platform-portable no-op: `cd .` exits 0 on both Windows (cmd) and sh.
        let actions = vec![TransportAction {
            kind: TransportKind::Shell,
            command: "cd .".to_owned(),
            label: Some("No-op".to_owned()),
        }];
        execute_transport(&actions, tmp.path()).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn test_execute_transport_windows_embedded_quotes() {
        // Confirms that cmd /C correctly passes a command containing embedded double-quotes.
        // `echo "hello world"` exits 0 on cmd.exe regardless of the quoted argument.
        let tmp = tempfile::TempDir::new().unwrap();
        let actions = vec![TransportAction {
            kind: TransportKind::Shell,
            command: r#"echo "hello world""#.to_owned(),
            label: Some("EchoQuoted".to_owned()),
        }];
        execute_transport(&actions, tmp.path()).unwrap();
    }
}
