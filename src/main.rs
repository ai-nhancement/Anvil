//! Anvil — Structure for vibe coding.
//!
//! A model-agnostic, interactive coding agent that adds just enough structure to stop drift.
//! The coder is a real agent: it reads, writes, and edits files and runs commands itself
//! (via tools), the same way Claude Code / Cursor / Aider work — no manual file inclusion.
//!
//! Structure is imposed at exactly two human gates:
//!   - PLAN:  discuss → the coder writes plan.md itself → /lock-plan (R1+R2 reviewers, different
//!            models, critique plan.md) → coder revises → /accept-plan.
//!   - PHASE: build the phase directly → /accept-phase (R1+R2 reviewers critique the git diff)
//!            → fix findings → /ship-phase.
//!
//! Hard rule: exactly two review rounds per gate, from different model families. Review files
//! live at repo root. Designed explicitly to kill the drift that kills vibe coding projects.

mod cli;
mod config;
mod llm;
mod tools;
mod agent;
mod reality;
mod state;
mod talk;
mod plan;
mod phase;
mod ui;

use clap::{Parser, Subcommand};
use colored::Colorize;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "anvil", version, about = "Structure for vibe coding — coder writes plan + phase review docs; sequential R1/R2 critical reviews (different models) with explicit human approve gates between")]
#[command(long_about = "Anvil brings just enough structure to prevent drift in AI-assisted coding.\n\
The coder is a real agent — it reads, writes, and edits files and runs commands itself (no manual /include). Structure is imposed at two human gates: PLAN (discuss → coder writes plan.md → /lock-plan runs R1+R2 reviewers → coder revises → /accept-plan) and PHASE (build → /accept-phase runs R1+R2 on the git diff → fix → /ship-phase). The two reviewers are different model families for a genuine second opinion. All REVIEW_* live at repo root.\n\
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

    /// Legacy one-shot: generate a plan (coder) + run both R1 + R2 immediately.
    /// Preferred: launch the TUI (`anvil`), discuss with the coder which writes plan.md itself, then /lock-plan (R1+R2) and /accept-plan.
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

    /// Phase work (legacy direct). Preferred TUI flow: the coder implements the phase directly, then /accept-phase (R1+R2 reviewers critique the git diff) and /ship-phase.
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
    /// Start or continue work on a phase (sets current in state; gives plan excerpt). Preferred full flow lives in TUI chat.
    Start {
        /// Phase id from the plan, e.g. P0
        id: String,
    },

    /// Legacy: run R1+R2 immediately on "done" claim. New TUI flow: coder writes REVIEW_Px_R1.md (the briefing doc), user /save-r1 then /critical-r1 runs reviewer critical *on coder's doc* (human gate), coder fixes, coder writes R2 doc, /critical-r2, human gate, coder summarizes, /phase-accept.
    Review {
        /// Phase id, e.g. P0
        id: String,
    },

    /// Mark the phase as accepted after the *full* coder-doc + critical-R1 + coder-R2-doc + critical-R2 + human approvals cycle. Updates shipped_phases + clears current.
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
