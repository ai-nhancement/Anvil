mod arbiter;
mod charter;
mod discuss;
mod graph;
mod phase;
mod plan;
mod session;
mod setup;
mod status;
mod utils;

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
    /// Run the interactive setup wizard (provider connections, model bindings, credentials).
    Setup {
        /// Project directory (defaults to current directory).
        #[arg(default_value = ".")]
        path: PathBuf,
    },
    /// Sidecar daemon management.
    #[command(subcommand)]
    Sidecar(SidecarCmd),
    /// Interactive Interlocutor discussion — produces charter.md.
    Discuss {
        /// Project directory (defaults to current directory).
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },
    /// Charter stage pipeline (review, findings curation).
    #[command(subcommand)]
    Charter(CharterCmd),
    /// Arbiter commands: declare convergence and resolve findings (P6).
    #[command(subcommand)]
    Arbiter(ArbiterCmd),
    /// Plan stage pipeline (invoke, review, findings, consolidate).
    #[command(subcommand)]
    Plan(PlanCmd),
    /// Build stage pipeline — per-phase build, review, and ship loop (P8).
    #[command(subcommand)]
    Phase(PhaseCmd),
    /// Phase dependency graph queries.
    #[command(subcommand)]
    Graph(GraphCmd),
    /// Show project workflow status (rotation position, round count, advisory findings, pool clean check).
    Status {
        /// Project directory (defaults to current directory).
        #[arg(long, default_value = ".")]
        project: PathBuf,
        /// Artifact to scope status counts to (default: charter.md).
        #[arg(long, default_value = "charter.md")]
        artifact: String,
    },
}

#[derive(Subcommand)]
enum SidecarCmd {
    /// Show the running sidecar daemon status for this project.
    Status {
        /// Project root directory (defaults to current directory).
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },
    /// Stop the running sidecar daemon for this project.
    Stop {
        /// Project root directory (defaults to current directory).
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },
    /// Start the sidecar daemon for this project (no-op if already running).
    Start {
        /// Project root directory (defaults to current directory).
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },
}

#[derive(Subcommand)]
enum ArbiterCmd {
    /// Declare convergence for an artifact (requires non-empty --reason).
    DeclareConvergence {
        /// Artifact identifier (e.g. `charter.md`).
        artifact: String,
        /// Non-empty reasoning for the convergence declaration.
        #[arg(long)]
        reason: String,
        /// Project directory (defaults to current directory).
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },
    /// Resolve a specific finding as Arbiter-Decided (requires non-empty --reason).
    ///
    /// `finding-id` must be in composite form `<packet_id>:<finding_id>` (e.g. `uuid:F1`).
    ResolveFinding {
        /// Composite finding ID (`<packet_id>:<finding_id>`).
        finding_id: String,
        /// Non-empty reasoning for the arbiter resolution.
        #[arg(long)]
        reason: String,
        /// Summary of the chosen direction.
        #[arg(long, default_value = "")]
        chosen_direction: String,
        /// Which other findings or rounds this contradicts or relates to.
        #[arg(long, default_value = "")]
        contradiction_context: String,
        /// Project directory (defaults to current directory).
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },
}

#[derive(Subcommand)]
enum PlanCmd {
    /// Invoke the Planner model against the approved Charter; validate and write `ANVIL_PLAN.md`.
    Invoke {
        /// Project directory (defaults to current directory).
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },
    /// Invoke the reviewer model against `ANVIL_PLAN.md` and store findings.
    Review {
        /// Project directory (defaults to current directory).
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },
    /// Interactively curate Plan review findings and render the disposition document.
    Findings {
        /// Project directory (defaults to current directory).
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },
    /// Absorb hardening notes into the Plan, bump version, store provenance snapshot.
    Consolidate {
        /// Non-empty description of why this consolidation is being performed.
        #[arg(long, default_value = "end-of-phase")]
        trigger: String,
        /// Project directory (defaults to current directory).
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },
}

#[derive(Subcommand)]
enum GraphCmd {
    /// Display all phases and their direct dependencies.
    Show {
        /// Project directory (defaults to current directory).
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },
    /// Show transitive dependents of a phase (phases affected by a change to it).
    BlastRadius {
        /// Phase ID to query (e.g. `P3`).
        phase_id: String,
        /// Project directory (defaults to current directory).
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },
}

#[derive(Subcommand)]
enum PhaseCmd {
    /// Invoke the Coder for a phase and produce the Phase Review Briefing.
    Build {
        /// Phase ID to build (e.g. `P8`).
        id: String,
        /// Output format: `text` (default) or `json` (prints briefing contract JSON; no file write).
        #[arg(long, value_name = "FORMAT", default_value = "text")]
        format: String,
        /// Print the `PhaseBriefingContract` JSON Schema and exit.
        #[arg(long)]
        describe_schema: bool,
        /// Project directory (defaults to current directory).
        #[arg(long, default_value = ".")]
        project: std::path::PathBuf,
    },
    /// Send the latest phase briefing to the next reviewer in rotation; store findings.
    Review {
        /// Phase ID to review (e.g. `P8`).
        id: String,
        /// Project directory (defaults to current directory).
        #[arg(long, default_value = ".")]
        project: std::path::PathBuf,
    },
    /// Ship a phase (requires full-pool clean termination condition).
    Ship {
        /// Phase ID to ship (e.g. `P8`).
        id: String,
        /// Project directory (defaults to current directory).
        #[arg(long, default_value = ".")]
        project: std::path::PathBuf,
    },
}

#[derive(Subcommand)]
enum CharterCmd {
    /// Invoke the reviewer model against charter.md and store the findings packet.
    Review {
        /// Project directory (defaults to current directory).
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },
    /// Interactively curate verified findings and render the disposition document.
    Findings {
        /// Project directory (defaults to current directory).
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },
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
        Command::Setup { path } => cmd_setup(&path),
        Command::Sidecar(cmd) => match cmd {
            SidecarCmd::Status { project } => cmd_sidecar_status(&project),
            SidecarCmd::Stop { project } => cmd_sidecar_stop(&project),
            SidecarCmd::Start { project } => cmd_sidecar_start(&project),
        },
        Command::Discuss { project } => discuss::run_discuss(&project),
        Command::Charter(cmd) => match cmd {
            CharterCmd::Review { project } => charter::run_charter_review(&project),
            CharterCmd::Findings { project } => charter::run_charter_findings(&project),
        },
        Command::Arbiter(cmd) => match cmd {
            ArbiterCmd::DeclareConvergence {
                artifact,
                reason,
                project,
            } => arbiter::run_declare_convergence(&project, &artifact, &reason),
            ArbiterCmd::ResolveFinding {
                finding_id,
                reason,
                chosen_direction,
                contradiction_context,
                project,
            } => arbiter::run_resolve_finding(
                &project,
                &finding_id,
                &reason,
                &chosen_direction,
                &contradiction_context,
            ),
        },
        Command::Plan(cmd) => match cmd {
            PlanCmd::Invoke { project } => plan::run_plan_invoke(&project),
            PlanCmd::Review { project } => plan::run_plan_review(&project),
            PlanCmd::Findings { project } => plan::run_plan_findings(&project),
            PlanCmd::Consolidate { trigger, project } => {
                plan::run_plan_consolidate(&project, &trigger)
            }
        },
        Command::Phase(cmd) => match cmd {
            PhaseCmd::Build {
                id,
                format,
                describe_schema,
                project,
            } => {
                let fmt = match format.as_str() {
                    "json" => phase::OutputFormat::Json,
                    _ => phase::OutputFormat::Text,
                };
                phase::run_phase_build(&project, &id, fmt, describe_schema)
            }
            PhaseCmd::Review { id, project } => phase::run_phase_review(&project, &id),
            PhaseCmd::Ship { id, project } => phase::run_phase_ship(&project, &id),
        },
        Command::Graph(cmd) => match cmd {
            GraphCmd::Show { project } => graph::run_graph_show(&project),
            GraphCmd::BlastRadius { phase_id, project } => {
                graph::run_graph_blast_radius(&project, &phase_id)
            }
        },
        Command::Status { project, artifact } => status::run_status(&project, &artifact),
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

    println!();
    if config.reviewer_pool.is_empty() {
        println!("Reviewer pool: (empty — defaults to [reviewer-1])");
    } else {
        println!("Reviewer pool:");
        for (i, name) in config.reviewer_pool.iter().enumerate() {
            println!("  [{i}] {name}");
        }
    }
    println!(
        "Single-clean-pass override: {}",
        if config.single_clean_pass_override {
            "ON"
        } else {
            "off"
        }
    );

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
        "reviewer_pool" => {
            config.reviewer_pool = if value.trim().is_empty() {
                Vec::new()
            } else {
                value.split(',').map(|s| s.trim().to_owned()).collect()
            };
        }
        "single_clean_pass_override" => {
            config.single_clean_pass_override = match value {
                "true" | "1" | "on" => true,
                "false" | "0" | "off" => false,
                _ => {
                    return Err(anvil_core::error::AnvilError::InvalidConfigValue {
                        key: key.to_owned(),
                        reason: format!("expected true/false, got '{value}'"),
                    });
                }
            };
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

fn cmd_audit_list(root: &Path, record_type_str: &str) -> Result<(), anvil_core::error::AnvilError> {
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

fn cmd_setup(path: &Path) -> Result<(), anvil_core::error::AnvilError> {
    setup::run_wizard(path)
}

fn cmd_sidecar_status(root: &Path) -> Result<(), anvil_core::error::AnvilError> {
    let pid_path = root.join(".anvil/run/sidecar.pid");
    let port_path = root.join(".anvil/run/sidecar.port");

    if !pid_path.exists() {
        println!(
            "Sidecar: not running (no PID file at {}).",
            pid_path.display()
        );
        return Ok(());
    }

    let pid_str = std::fs::read_to_string(&pid_path)?;
    let pid: u32 = pid_str.trim().parse().unwrap_or(0);

    let port = port_path
        .exists()
        .then(|| std::fs::read_to_string(&port_path).ok())
        .flatten()
        .and_then(|s| s.trim().parse::<u16>().ok());

    if anvil_core::sidecar::is_process_alive(pid) {
        print!("Sidecar: running (PID {pid}");
        if let Some(p) = port {
            print!(", port {p}");
            // Probe the Health RPC to confirm the process is actually a sidecar.
            let healthy = probe_health_sync(p);
            print!(", health: {}", if healthy { "OK" } else { "UNREACHABLE" });
        }
        println!(")");
        println!("  PID file: {}", pid_path.display());
    } else {
        println!("Sidecar: stale PID file (PID {pid} is not running).");
        println!("  Run `anvil sidecar stop` to clean up stale files.");
    }

    Ok(())
}

fn cmd_sidecar_stop(root: &Path) -> Result<(), anvil_core::error::AnvilError> {
    let pid_path = root.join(".anvil/run/sidecar.pid");
    let port_path = root.join(".anvil/run/sidecar.port");

    if !pid_path.exists() {
        println!("Sidecar: not running.");
        return Ok(());
    }

    let pid_str = std::fs::read_to_string(&pid_path)?;
    let pid: u32 = pid_str.trim().parse().unwrap_or(0);

    if pid == 0 {
        // Invalid PID file — clean up stale files.
        let _ = std::fs::remove_file(&pid_path);
        let _ = std::fs::remove_file(&port_path);
        println!("Cleaned up invalid PID file.");
        return Ok(());
    }

    if anvil_core::sidecar::is_process_alive(pid) {
        anvil_core::sidecar::kill_process(pid);
        println!("Stop signal sent to sidecar (PID {pid}).");
        // Brief settle time before cleaning up runtime files.
        std::thread::sleep(std::time::Duration::from_millis(500));
    } else {
        println!("Sidecar process (PID {pid}) is not running — cleaning up stale files.");
    }

    let _ = std::fs::remove_file(&pid_path);
    let _ = std::fs::remove_file(&port_path);
    println!("Runtime files removed.");
    Ok(())
}

fn cmd_sidecar_start(root: &Path) -> Result<(), anvil_core::error::AnvilError> {
    let config = anvil_core::config::load_config(root)?;
    let port = session::ensure_sidecar_running(root, &config)?;
    println!("Sidecar running on port {port}.");
    println!("  Run `anvil sidecar status` to inspect.");
    Ok(())
}

/// Probe the sidecar Health RPC synchronously. Returns `false` on any failure.
fn probe_health_sync(port: u16) -> bool {
    use anvil_sidecar_client::client::AnvilSidecarClient;
    setup::with_tokio(async move {
        let addr = format!("http://127.0.0.1:{port}");
        match AnvilSidecarClient::connect(addr).await {
            Ok(mut c) => c.probe_health().await.is_ok(),
            Err(_) => false,
        }
    })
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
