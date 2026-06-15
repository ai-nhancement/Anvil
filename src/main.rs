//! Anvil — Structure for vibe coding.
//!
//! A model-agnostic, interactive CLI that enforces a lightweight disciplined flow:
//!   Talk → Plan (exactly R1 + R2 by different providers/models) → Build by phases
//!   (for each phase: implement with tool assistance, then exactly R1 + R2).
//!
//! Hard rule: only two review rounds per gate. Different model + provider for R1 vs R2.
//! Designed explicitly to kill the drift that kills vibe coding projects.

mod cli;
mod config;
mod llm;
mod state;
mod talk;
mod plan;
mod phase;
mod ui;

use clap::{Parser, Subcommand};
use colored::Colorize;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "anvil", version, about = "Structure for vibe coding — Talk, Plan (R1+R2), Build phases (R1+R2)")]
#[command(long_about = "Anvil brings just enough structure to prevent drift in AI-assisted coding.\n\
Talk with a model to capture intent. Produce a plan, reviewed by exactly two different models from different providers.\n\
Implement phase by phase with the tool's help. Each phase gets exactly two reviews before you move on.\n\
No R3+. Cross-provider by design. Ollama, local, Azure, Bedrock, every gateway supported.")]
struct Cli {
    /// Subcommand. When omitted (i.e. bare `anvil` or `cargo run --`), we launch the interactive TUI.
    #[command(subcommand)]
    command: Option<Commands>,

    /// Project root (defaults to current directory)
    #[arg(long, global = true, default_value = ".")]
    project: PathBuf,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new Anvil project (creates anvil.toml + .anvil/)
    Init {
        /// Optional path for the new project
        path: Option<PathBuf>,
    },

    /// Interactive setup: add providers, connections, assign roles (coder, planner, reviewers)
    Setup,

    /// Show or edit configuration
    #[command(subcommand)]
    Config(ConfigCmd),

    /// Open an interactive Talk session with a model (captures intent, goals, open questions)
    Talk {
        /// Specific model binding to use (e.g. "coder" or "interlocutor"). Defaults to primary.
        #[arg(long)]
        model: Option<String>,
    },

    /// Generate / refine the phased Plan, then run exactly R1 + R2 reviews (different providers)
    Plan {
        /// Force a fresh plan generation even if one exists
        #[arg(long)]
        fresh: bool,

        /// Record that R1+R2 findings have been addressed and lock the plan hash.
        /// Unlocks `anvil phase start`. Requires plan.md + both review files to exist.
        #[arg(long)]
        accept: bool,

        /// Path to a context file (e.g. a saved talk artifact) to feed into plan generation.
        /// Use after `anvil talk` when you have saved a charter or goals doc.
        #[arg(long, value_name = "FILE")]
        context: Option<std::path::PathBuf>,
    },

    /// Work on a phase: implementation assistance + exactly two reviews when ready
    #[command(subcommand)]
    Phase(PhaseCmd),

    /// Show current workflow status (current phase, last gates, which reviewers used)
    Status,

    /// Launch the full interactive TUI (this is also the default when no subcommand is provided)
    Ui,
}

#[derive(Subcommand)]
enum ConfigCmd {
    /// Display current configuration (providers, bindings, roles)
    Show,

    /// Add or update a provider connection interactively
    AddProvider,
}

#[derive(Subcommand)]
enum PhaseCmd {
    /// Start or continue work on a phase (enters assisted implementation chat for the phase)
    Start {
        /// Phase id from the plan, e.g. P3
        id: String,
    },

    /// When you believe the phase is complete, run exactly R1 then R2 review (different models/providers)
    Review {
        /// Phase id, e.g. P3
        id: String,
    },

    /// Mark the phase as accepted after successful R1 + R2. Records the gate.
    Accept {
        /// Phase id
        id: String,
        /// Short note about what was addressed from reviews
        #[arg(long)]
        note: Option<String>,
    },

    /// List phases and their status
    List,
}

fn main() {
    let cli = Cli::parse();

    if let Err(err) = run(cli) {
        eprintln!("{} error: {}", "anvil".red(), err);
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> anyhow::Result<()> {
    // Bare `anvil` (no subcommand) or explicit `anvil ui` → the CLINE/Codex-style TUI.
    // Every previous subcommand remains fully functional and headless.
    let command = cli.command.unwrap_or(Commands::Ui);

    match command {
        Commands::Ui => ui::run_ui(&cli.project),
        Commands::Init { path } => {
            let root = path.unwrap_or_else(|| cli.project.clone());
            cli::cmd_init(&root)
        }
        Commands::Setup => cli::cmd_setup(&cli.project),
        Commands::Config(sub) => match sub {
            ConfigCmd::Show => cli::cmd_config_show(&cli.project),
            ConfigCmd::AddProvider => cli::cmd_config_add_provider(&cli.project),
        },
        Commands::Talk { model } => talk::run_talk(&cli.project, model.as_deref()),
        Commands::Plan { fresh, accept, context } => {
            if accept {
                plan::accept_plan(&cli.project)
            } else {
                plan::run_plan(&cli.project, fresh, context.as_deref())
            }
        }
        Commands::Phase(sub) => match sub {
            PhaseCmd::Start { id } => phase::run_phase_start(&cli.project, &id),
            PhaseCmd::Review { id } => phase::run_phase_review(&cli.project, &id),
            PhaseCmd::Accept { id, note } => phase::run_phase_accept(&cli.project, &id, note.as_deref()),
            PhaseCmd::List => phase::run_phase_list(&cli.project),
        },
        Commands::Status => cli::cmd_status(&cli.project),
    }
}
