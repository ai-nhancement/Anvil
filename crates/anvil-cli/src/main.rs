use std::path::{Path, PathBuf};

use anvil_audit::{AuditStore, RecordType};
use anvil_core::{
    choices::LockState,
    config::{check_plan_stage_gate, load_config, save_config},
    project,
};
use anvil_graph::ProvenanceGraph;
use clap::{Parser, Subcommand};

pub(crate) const BINARY_NAME: &str = "anvil";

#[derive(Parser)]
#[command(name = "anvil", version, about = "Anvil workflow CLI")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Initialize a new Anvil project at the given path.
    Init {
        /// Path to the new project directory.
        path: PathBuf,
    },
    /// Inspect or modify project configuration.
    #[command(subcommand)]
    Config(ConfigCmd),
    /// Gate checks for workflow stage transitions.
    #[command(subcommand)]
    Gate(GateCmd),
    /// Audit store operations.
    #[command(subcommand)]
    Audit(AuditCmd),
}

#[derive(Subcommand)]
enum ConfigCmd {
    /// Display current project configuration and Required-Choice lock status.
    Show {
        /// Project root directory (defaults to current directory).
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },
    /// Set a configuration value (key in dotted form, e.g. `sidecar.idle_timeout_secs`).
    Set {
        key: String,
        value: String,
        /// Project root directory (defaults to current directory).
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },
}

#[derive(Subcommand)]
enum GateCmd {
    /// Check whether all Required Choices are locked; required before entering Plan stage.
    CheckPlan {
        /// Project root directory (defaults to current directory).
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },
}

#[derive(Subcommand)]
enum AuditCmd {
    /// List all records of a given type (kebab-case, e.g. `gate-approval`).
    List {
        record_type: String,
        /// Project root directory (defaults to current directory).
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },
    /// Show the full JSON of a record by ID.
    Show {
        id: String,
        /// Project root directory (defaults to current directory).
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },
    /// Check that all indexed records are physically present on disk.
    Integrity {
        /// Project root directory (defaults to current directory).
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },
    /// Show which records back a cross-reference key (format: `path:section:version`).
    Provenance {
        cross_ref_key: String,
        /// Project root directory (defaults to current directory).
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },
}

fn main() {
    let cli = Cli::parse();
    let result = run(cli);
    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), anvil_core::error::AnvilError> {
    match cli.command {
        Command::Init { path } => cmd_init(&path),
        Command::Config(cmd) => match cmd {
            ConfigCmd::Show { project } => cmd_config_show(&project),
            ConfigCmd::Set {
                key,
                value,
                project,
            } => cmd_config_set(&project, &key, &value),
        },
        Command::Gate(cmd) => match cmd {
            GateCmd::CheckPlan { project } => cmd_gate_check_plan(&project),
        },
        Command::Audit(cmd) => match cmd {
            AuditCmd::List {
                record_type,
                project,
            } => cmd_audit_list(&project, &record_type),
            AuditCmd::Show { id, project } => cmd_audit_show(&project, &id),
            AuditCmd::Integrity { project } => cmd_audit_integrity(&project),
            AuditCmd::Provenance {
                cross_ref_key,
                project,
            } => cmd_audit_provenance(&project, &cross_ref_key),
        },
    }
}

fn cmd_init(path: &Path) -> Result<(), anvil_core::error::AnvilError> {
    match project::init(path)? {
        project::InitResult::Initialized { root, dirs_created } => {
            println!("Initialized Anvil project at {}", root.display());
            println!("  Created {dirs_created} directories + anvil.toml");
            println!(
                "  Run `anvil config show --project {}` to inspect choices.",
                root.display()
            );
        }
        project::InitResult::AlreadyInitialized { root } => {
            println!("Project already initialized at {}", root.display());
            cmd_config_show(&root)?;
        }
    }
    Ok(())
}

fn cmd_config_show(root: &Path) -> Result<(), anvil_core::error::AnvilError> {
    let config = load_config(root)?;

    println!("Required Choices ({} total):", config.choices.len());
    for (key, choice) in &config.choices {
        let lock_label = match choice.lock_state {
            LockState::Final => "final",
            LockState::Provisional => "provisional",
            LockState::Unlocked => "UNLOCKED",
        };
        println!("  {key} [{lock_label}]: {}", choice.value);
        if choice.lock_state == LockState::Provisional {
            if let Some(h) = &choice.hypothesis {
                println!("    hypothesis: {h}");
            }
            if let Some(rt) = &choice.revision_trigger {
                println!("    revision_trigger: {rt}");
            }
        }
    }

    println!();
    println!("Sidecar:");
    println!("  idle_timeout_secs: {}", config.sidecar.idle_timeout_secs);
    match &config.sidecar.binary_path {
        Some(p) => println!("  binary_path: {}", p.display()),
        None => println!("  binary_path: (from $PATH)"),
    }

    if config.provider_connections.is_empty() {
        println!();
        println!("Provider connections: (none — run `anvil setup` to configure)");
    } else {
        println!();
        println!("Provider connections:");
        for (name, conn) in &config.provider_connections {
            println!("  {name}: {:?}", conn.provider_type);
        }
    }

    if config.model_bindings.is_empty() {
        println!();
        println!("Model bindings: (none — run `anvil setup` to configure)");
    } else {
        println!();
        println!("Model bindings:");
        for binding in &config.model_bindings {
            println!(
                "  {} — {} via {}",
                binding.name, binding.model_identity, binding.provider_connection
            );
        }
    }

    Ok(())
}

fn cmd_config_set(
    root: &Path,
    key: &str,
    value: &str,
) -> Result<(), anvil_core::error::AnvilError> {
    let mut config = load_config(root)?;

    match key {
        "sidecar.idle_timeout_secs" => {
            let secs: u32 =
                value
                    .parse()
                    .map_err(|_| anvil_core::error::AnvilError::InvalidConfigValue {
                        key: key.to_owned(),
                        reason: format!("expected a non-negative integer, got '{value}'"),
                    })?;
            config.sidecar.idle_timeout_secs = secs;
        }
        "sidecar.binary_path" => {
            config.sidecar.binary_path = Some(PathBuf::from(value));
        }
        _ => {
            return Err(anvil_core::error::AnvilError::UnknownConfigKey(
                key.to_owned(),
            ));
        }
    }

    save_config(root, &config)?;
    println!("Set {key} = {value}");
    Ok(())
}

fn cmd_gate_check_plan(root: &Path) -> Result<(), anvil_core::error::AnvilError> {
    let config = load_config(root)?;
    let unlocked = check_plan_stage_gate(&config);
    if unlocked.is_empty() {
        println!("Gate check passed: all Required Choices are locked.");
        println!("Plan stage is unblocked.");
    } else {
        eprintln!(
            "Gate check failed: {} Required Choice(s) are unlocked:",
            unlocked.len()
        );
        for key in &unlocked {
            eprintln!("  - {key}");
        }
        eprintln!();
        eprintln!("Lock all choices before entering Plan stage.");
        eprintln!("Use `{BINARY_NAME} config set <key> <value>` to update values.");
        std::process::exit(1);
    }
    Ok(())
}

fn cmd_audit_list(
    root: &Path,
    record_type_str: &str,
) -> Result<(), anvil_core::error::AnvilError> {
    let rt = RecordType::from_dir_name(record_type_str)
        .or_else(|| RecordType::from_type_name(record_type_str))
        .ok_or_else(|| {
            anvil_core::error::AnvilError::InvalidRecordType(record_type_str.to_owned())
        })?;
    let store = AuditStore::open(root)?;
    let entries = store.list(rt)?;
    if entries.is_empty() {
        println!("No {} records found.", rt.as_str());
    } else {
        println!("{} record(s) of type {}:", entries.len(), rt.as_str());
        for entry in &entries {
            println!("  {}", entry.id);
        }
    }
    Ok(())
}

fn cmd_audit_show(root: &Path, id: &str) -> Result<(), anvil_core::error::AnvilError> {
    let store = AuditStore::open(root)?;
    let value = store.get(id)?;
    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
}

fn cmd_audit_integrity(root: &Path) -> Result<(), anvil_core::error::AnvilError> {
    let store = AuditStore::open(root)?;
    let report = store.check_integrity()?;
    match report.status {
        anvil_audit::IntegrityStatus::Pass => {
            println!("Integrity check passed: all indexed records are present on disk.");
        }
        anvil_audit::IntegrityStatus::Warn => {
            println!("Integrity check: warnings detected.");
            for v in &report.violations {
                println!("  WARN  {} — {}", v.path, v.reason);
            }
        }
        anvil_audit::IntegrityStatus::BlockShip => {
            eprintln!(
                "Integrity check FAILED: {} record(s) missing from disk:",
                report.violations.len()
            );
            for v in &report.violations {
                eprintln!("  MISSING  {} (id: {})", v.path, v.id);
            }
            std::process::exit(1);
        }
    }
    Ok(())
}

fn cmd_audit_provenance(
    root: &Path,
    cross_ref_key: &str,
) -> Result<(), anvil_core::error::AnvilError> {
    let key = anvil_audit::CrossRefKey::parse(cross_ref_key).ok_or_else(|| {
        anvil_core::error::AnvilError::InvalidCrossRefKey(cross_ref_key.to_owned())
    })?;
    let store = AuditStore::open(root)?;
    let graph = ProvenanceGraph::build(&store)?;
    let backing = graph.records_for_key(&key);
    if backing.is_empty() {
        println!("No records back '{cross_ref_key}'.");
    } else {
        println!("{} record(s) back '{cross_ref_key}':", backing.len());
        for id in backing {
            println!("  {id}");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::BINARY_NAME;

    // hinge_test: pins=1.80, intended=stable-floor, phase=P0
    #[test]
    fn test_rust_toolchain_version_floor() {
        // Pins: rust-toolchain.toml must set channel = "stable" exactly (floor ≥1.80).
        // Flipping requires updating rust-toolchain.toml and this annotation together.
        // Checks the key=value line, not just any occurrence of "stable" in comments.
        let toolchain = include_str!("../../../rust-toolchain.toml");
        assert!(
            toolchain
                .lines()
                .any(|l| l.trim() == r#"channel = "stable""#),
            r#"rust-toolchain.toml must contain exactly: channel = "stable" (floor: ≥1.80)"#
        );
    }

    // hinge_test: pins=anvil, intended=binary-entry-point, phase=P0
    #[test]
    fn test_cli_entry_point_exists() {
        // Pins: the CLI binary is named "anvil" (the [[bin]] name in Cargo.toml).
        // Flipping requires changing BINARY_NAME and the [[bin]] declaration together.
        assert_eq!(BINARY_NAME, "anvil");
    }
}
