//! ratatui TUI for Anvil (Phase 2 complete: real LLM streaming chat).
//!
//! Default launch target: `anvil` (no subcommand) or `cargo run --`.
//! Persistent chat-centric UI. All legacy headless subcommands remain fully functional.
//!
//! Phase 2: normal typing now resolves the planner (or coder) role, calls the real LlmClient
//! via the new chat_stream_to_channel path, and appends tokens live using mpsc from a
//! background multi-thread tokio runtime. No stdout writes from LLM code while in TUI.
//! Headless paths (plan/phase/talk + their block_on + prints) untouched.

use std::fs::OpenOptions;
use std::io::{stdout, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use chrono::Utc;
use crossterm::{
    event::{
        self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEventKind,
        KeyModifiers,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::agent::{Agent, ConfirmHandle};
use crate::config::{
    ensure_anvil_dir, ensure_anvil_gitignored, load_config, load_local_env, save_global_config,
    set_local_env_var, AnvilConfig, CredentialRef, ModelBinding, ProviderConnection,
};
use crate::llm::{ChatMessage, LlmClient, Role};
use crate::state::{active_plan_name, active_plan_path, load_state, reviews_dir, save_state};

/// Turn the CLI's project argument (often ".") into a real absolute path for
/// display and tool use. Canonicalizes when possible and strips Windows'
/// `\\?\` verbatim prefix; falls back to joining the current dir.
fn resolve_project_root(root: &Path) -> PathBuf {
    let abs = std::fs::canonicalize(root)
        .ok()
        .or_else(|| std::env::current_dir().ok().map(|cwd| cwd.join(root)));
    match abs {
        Some(p) => {
            let s = p.to_string_lossy();
            // \\?\C:\Anvil -> C:\Anvil ; \\?\UNC\server\share -> \\server\share
            let cleaned = s
                .strip_prefix(r"\\?\UNC\")
                .map(|rest| format!(r"\\{}", rest))
                .or_else(|| s.strip_prefix(r"\\?\").map(|rest| rest.to_string()));
            cleaned.map(PathBuf::from).unwrap_or(p)
        }
        None => root.to_path_buf(),
    }
}

/// A short, human-friendly name for the project root (its final component),
/// e.g. "Anvil" for C:\Anvil. Falls back to the full path, then "project".
fn project_display_name(root: &Path) -> String {
    root.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .filter(|s| !s.is_empty() && s != ".")
        .unwrap_or_else(|| {
            let d = root.display().to_string();
            if d.is_empty() || d == "." {
                "project".to_string()
            } else {
                d
            }
        })
}

/// The default "Built with Anvil" badge, bundled into the binary so `/tag` works
/// out of the box on any machine (no file needed). Overridable per-user with
/// `/tag set <path.png>`. Source lives at the repo root as `tag.png`.
const DEFAULT_BADGE_PNG: &[u8] = include_bytes!("../tag.png");

/// System prompt for the coder agent. Short and agentic: the model has real
/// tools and works directly on the repo. Structure is imposed at exactly two
/// human gates (lock the plan, accept a phase); everything else is free-form.
/// The coder's system prompt. When the coder's binding configures a `contract`
/// (a local-model tier — see `contracts/MODEL_FINDINGS.md`), the model runs under
/// that bench-validated contract instead of the built-in prompt below. A name that
/// resolves to nothing falls back to the built-in prompt (with a warning at the call
/// site), so a typo can never leave the coder prompt-less. Frontier/cloud bindings
/// leave `contract` unset and get the built-in prompt.
fn coder_system_prompt(contract: Option<&str>, root: &std::path::Path) -> String {
    if let Some(name) = contract {
        if let Some(text) = crate::contracts::resolve(name, root) {
            return text;
        }
    }
    coder_builtin_prompt()
}

fn coder_builtin_prompt() -> String {
    "You are Anvil's coder: an autonomous, hands-on software engineer working directly in the user's project.\n\
\n\
You have tools, scoped to the project root:\n\
- read_file(path), write_file(path, content)\n\
- apply_patch(patch) — PREFERRED for editing existing files: a context-located diff (*** Update File / @@ / space-context / -removed / +added), can change several files at once, validated before writing. Use this over edit_file.\n\
- edit_file(path, old_string, new_string) — simple exact-snippet replace; fine for a single tiny edit, but apply_patch is more reliable.\n\
- list_dir(path), grep(pattern, [path])\n\
- run_command(command)  — e.g. cargo build, cargo test, git diff (the user confirms each run)\n\
- delegate(specialist, task) — hand a focused EVIDENCE-GATHERING task to a scoped, read-only specialist sub-agent when you need information from OUTSIDE this project. researcher: searches the web + reads pages (library/API docs, usage, best practices, error explanations). repo-scout: shallow-clones an external git repo and studies how it does something. The specialist can't see this chat, so put all needed context in `task`; it returns evidence, and YOU stay the decision-maker and the only one who edits code. Its outward actions (web fetch, repo clone) ask the user to confirm.\n\
- flag_risk(note) — surface a risk or decision that deserves the user's eyes NOW, mid-task, without waiting for a review gate. Use it when you proceed past real uncertainty (an ambiguous requirement, a risky tradeoff, a possible breaking change, a load-bearing assumption that might be wrong). It shows immediately in the UI and is saved to .anvil/risks.md. It does NOT block you — flag it and keep working. Prefer this over silently parking doubt in assumptions.md when the decision could send the work down the wrong path.\n\
\n\
Always use your built-in read_file / grep / list_dir tools to read and search the code — they work everywhere and need no confirmation. Reserve run_command for build/test/lint/git; do NOT shell out to grep/findstr/Select-String/python to search files (the platform varies; your grep tool is reliable). If a search returns nothing, try a shorter or different pattern rather than switching to the shell.\n\
For LARGE files (hundreds of lines, e.g. src/ui.rs), do NOT read the whole file repeatedly — grep for the symbol or text you need to find its line, then read_file with offset+limit to read just that section. Reading whole large files wastes context and makes you lose track. Don't re-run the same read/list/project_state call you already ran this turn.\n\
\n\
Work like a real engineer: read what you need before editing (never ask the user to paste files — open them yourself), make the changes with write_file/edit_file, and verify with run_command. Keep prose short; let the tools do the work. Prefer small, precise edits over rewriting whole files. Stay on the user's current request; don't switch to a different task from background context.\n\
Keep going until the task is actually done — do not stop after a partial step or narrate what you are *about* to do without doing it. ACT with tool calls, don't describe. When a run_command FAILS (non-zero exit), that is not a stopping point: read the error output in the tool result, fix the cause (edit the file, add the dependency, correct the command), and run it again. Repeat until it passes. Only stop and ask the user when you are genuinely blocked (a real decision is needed, or you've tried and cannot resolve it) — never stop merely because a build or test failed.\n\
Match effort to the request. When the user only acknowledges or gives a short status (e.g. 'thanks', 'looks good', 'tested and working'), reply in one short line and STOP — do not start new work, re-read files, or re-verify. Never read the same file or run the same search twice in one turn; you already have the result.\n\
\n\
Anvil adds just enough structure to stop drift, at exactly TWO human gates:\n\
1. PLAN: discuss the work with the user, then write the plan yourself to plan.md (phases ## P0 — Name, each with a goal, 3–8 actions, a deliverable, and 2–5 acceptance criteria). When the user is happy they run /lock-plan, which drives a SEQUENTIAL review loop: reviewer R1 critiques plan.md → you are asked to apply R1's fixes to plan.md → the user reviews and continues → reviewer R2 critiques the revised plan → you apply R2's fixes → you summarize → the user runs /accept-plan. When Anvil hands you a round's findings, edit plan.md to address the real issues and then STOP (don't summarize until asked).\n\
2. PHASES: implement the current phase directly (write code + tests, run them). When it's done the user runs /accept-phase, which FIRST asks you to write a review briefing to REVIEW_<id>_BRIEF.md (what you built and WHY, design decisions, test coverage, and anything intentionally deferred — the reviewers read this alongside the diff so they have intent, not just the patch), THEN drives the same sequential loop: R1 → you apply fixes to the code → (user continues) → R2 → you apply fixes → you summarize → the user runs /ship-phase. R2 deliberately re-reviews after your R1 fixes, so it can catch bugs those fixes introduced.\n\
\n\
GIT IS THE SOURCE OF TRUTH FOR REVIEWS. This project is a git repository, and the review gates work by diffing git (a phase review is the git diff of your work; /review diffs the tree). So COMMIT your work with git (via run_command: `git add -A` then `git commit -m \"…\"`) as you finish each phase — and ideally at meaningful checkpoints within it — so every review sees a clean, self-contained diff and phases stay isolated. Write clear, conventional commit messages (e.g. `feat(P1): …`). Don't leave a finished phase uncommitted: uncommitted work still appears in the diff, but committing per phase is what keeps each phase's review focused on just that phase. Never use `git reset --hard`, force-push, or other history-destroying commands unless the user explicitly asks.\n\
\n\
Outside those two gates, just collaborate normally — answer questions, explore, refactor, debug — using your tools. Don't fake a gate or claim a review happened; only the /lock-plan and /accept-phase commands trigger the reviewers. When asked to address a round's findings, fix the real ones and skip spurious ones — don't expand scope. Be precise, skeptical of scope creep, and surface risks early.\n\
\n\
PROJECT CONTEXT FILES you maintain with your write_file/edit_file tools (all plain, user-visible files — no hidden state):\n\
- .anvil/decisions.md — durable preferences/conventions and verification commands that actually worked (e.g. how to test/lint/build). Append here when the user states a standing preference or you confirm a project convention.\n\
- .anvil/assumptions.md — things you are ASSUMING but have NOT verified. Add one when you proceed on an unconfirmed belief. When you verify it, move it to decisions.md (or just delete it); delete it if it proves wrong. Keep facts and guesses separate.\n\
- .anvil/scratch.md — a disposable scratchpad for investigation notes, alternative designs, command output. Never injected; not memory, not truth.\n\
- ARCHITECTURE.md (repo root) — a small, maintained map of the codebase; update it when structure changes.\n\
decisions.md, assumptions.md and working memory are injected into your context every turn; scratch.md and ARCHITECTURE.md are NOT — read them on demand. Keep all of them short and high-signal.\n\
\n\
When implementing a phase, follow this checklist: read the relevant files first → make the minimal diff → add/update tests → run the project's verification commands (from decisions.md) → inspect the diff before declaring it done.\n\
Prefer verification commands that terminate on their own. If a test runner or build hangs (open handles, watch mode, a started server that never exits, an infinite loop), don't just re-run it — make it exit (e.g. a force-exit / non-watch / timeout flag appropriate to that tool) and record the working, terminating invocation in decisions.md so future runs and the reviewers use it. A command that times out is killed and reported, but a fast clean exit is far better."
        .to_string()
}

/// Workflow stage machine for the TUI (reconciled from disk artifacts + state on every relevant action).
/// This makes the "source of truth is the files" contract visible and enforces the gates.
/// Note: the detailed sequential R1-then-approve-then-R2 flow for plan (and per-phase coder-doc + critical reviewer) is
/// primarily driven by slash commands + chat messages + presence of REVIEW_* at root; the high-level stage is still
/// Talk / PlanReviewsComplete / PlanAccepted for UI chrome.
#[derive(Clone, Debug, Default, PartialEq)]
enum WorkflowStage {
    #[default]
    Talk,
    PlanReviewsComplete, // R1 + R2 review files present for the plan (after the sequential /lock-plan gate)
    PlanAccepted,        // hash recorded after user approved the final post-R1/R2 plan
    Unconfigured,
}

/// What a review gate is critiquing: the plan, or a phase's git diff.
#[derive(Clone)]
enum GateArtifact {
    Plan,
    Phase(String), // phase id, e.g. "P0"
    /// Ad-hoc `/review` of recent work (not a planned phase). `deep` adds the
    /// opt-in R2 second opinion; otherwise the gate is R1-only.
    Addition {
        slug: String,
        deep: bool,
    },
}

/// Which review round.
#[derive(Clone, Copy, PartialEq)]
enum Round {
    R1,
    R2,
}

/// The sequential review gate is a small state machine driven by async
/// completions: a reviewer finishing (gate_rx disconnect) advances a *Reviewing
/// step; a coder fix/summary turn finishing (llm_rx disconnect) advances a
/// *Fixing / Summarizing step. The two Paused steps wait for the user (/continue
/// or Enter on an empty line) so they can inspect each round before proceeding.
#[derive(Clone, PartialEq)]
enum GateStep {
    /// Phase gate only: the coder writes its review briefing (what was built + why)
    /// before the reviewers run. Plan gates skip straight to R1Reviewing.
    BriefWriting,
    R1Reviewing,
    R1Fixing,
    PausedAfterR1,
    R2Reviewing,
    R2Fixing,
    PausedAfterR2,
    Summarizing,
    Done,
}

#[derive(Clone)]
struct GateFlow {
    artifact: GateArtifact,
    step: GateStep,
}

/// One row in the `/approvals` checklist: a command prefix and whether it's
/// currently approved (auto-runs without a prompt).
#[derive(Clone)]
struct ApprovalItem {
    prefix: String,
    approved: bool,
}

/// The `/approvals` editor: a scrollable checklist of command prefixes the user
/// toggles to set which commands auto-run vs prompt. Built from their current list
/// (checked) unioned with a suggested catalog (unchecked). Saved to global config.
#[derive(Clone)]
struct ApprovalsEditor {
    items: Vec<ApprovalItem>,
    selected: usize,
}

/// Slash commands shown in the interactive palette (triggered by typing `/`).
/// Descriptions appear in the popup to help users discover the flow.
const SLASH_COMMANDS: &[(&str, &str)] = &[
    ("/plan", "How to plan: discuss with the coder, then it writes plan.md itself"),
    ("/lock-plan", "Plan gate: R1 → coder fixes → (pause) → R2 → coder fixes → (pause) → summary"),
    ("/accept-plan", "Quench the reviewed plan — lock it in (records the hash, unlocks phases)"),
    ("/phase-start <id>", "Set the current phase (e.g. P0). Optional — you can also just tell the coder to start"),
    ("/accept-phase [id]", "Phase gate: coder writes review briefing → R1 → coder fixes → (pause) → R2 → coder fixes → (pause) → summary"),
    ("/ship-phase [id]", "Quench the phase — ship it after its reviews (run /accept-phase first)"),
    ("/review [--deep] [label]", "Ad-hoc review of recent work (no plan needed): coder writes a briefing → R1 critiques the diff. Add --deep for a second cross-vendor R2 opinion"),
    ("/debug <description>", "Bug-hunt mode (no plan needed): describe the bug and the coder reproduces it, finds the root cause, makes the minimal fix + a regression test, and leaves it uncommitted — then run /review to gate it (alias /fix)"),
    ("/refresh", "Show the live reality snapshot (stage, phase, plan slice, git) the coder is grounded on"),
    ("/compact", "Clinker the forge: fold the conversation into .anvil/working-memory.md and rake out older turns (alias /clinker)"),
    ("/context", "Show how full the coder's context window is (tokens used / budget / % · whether compaction is imminent)"),
    ("/memory", "Inspect the coder's memory + context files (ledger, history window, working memory, decisions, assumptions, token estimate)"),
    ("/clear-memory", "Reset the in-session history + working memory (the append-only ledger is kept)"),
    ("/decisions", "View .anvil/decisions.md — durable preferences + verification commands (injected each turn)"),
    ("/assumptions", "View .anvil/assumptions.md — the coder's unverified working hypotheses (injected each turn)"),
    ("/scratch", "View .anvil/scratch.md — disposable notes (never injected)"),
    ("/architecture", "View ARCHITECTURE.md — the maintained code map (read on demand)"),
    ("/y", "Approve a pending run_command once (or ↑/↓ + Enter on the prompt)"),
    ("/a", "Approve + allow all of this program's commands for the session"),
    ("/n", "Deny a pending run_command"),
    ("/config", "Configure providers, model bindings, roles & API keys (full setup)"),
    ("/setup", "Alias for /config — providers, models, keys"),
    ("/swap", "Hot-swap one role's model (coder, R1, or R2): pick the role, then pick or type the model id"),
    ("/approvals", "Edit which shell commands auto-run without a y/n prompt (checklist; Space toggles, Esc saves)"),
    ("/tag", "Tag this build: add a 'Built with Anvil' + badge footer (bundled default badge works out of the box). /tag set <path.png> overrides it"),
    ("/status", "Show roles, config state, and current gate progress"),
    ("/models", "Show each role's model facts (context window, tool-call, price) via models.dev"),
    ("/loaded", "/ps /ollama-ps — list Ollama models currently in VRAM + sizes"),
    ("/unload [model]", "Force immediate unload (keep_alive=0) of one or all loaded models"),
    ("/help", "Show key bindings and available commands"),
    ("/continue", "Resume a paused review gate (run the next round / summary)"),
    ("/update", "Update anvil to the latest release (when one is available)"),
    ("/quit", "Exit the TUI"),
    ("/view-plan", "Open the active plan in a focused review popup"),
    ("/view-reviews", "Open the REVIEW_* files (plan + current phase) in a focused popup"),
    ("/readme", "Open the Anvil README in a scrollable popup (↑/↓/PgUp/PgDn, Esc to close)"),
    ("/new-plan <name>", "Start a fresh feature-named plan (e.g. frontpage_plan.md); archives the current plan + its reviews"),
    ("/plans", "List the active plan and any archived plans"),
];

/// The runnable part of a palette command's display string — the leading tokens up
/// to the first usage placeholder (`[...]` / `<...>`). The palette shows usage hints
/// like `/review [--deep] [label]` or `/ship-phase [id]`, but those bracketed tokens
/// aren't literal input; inserting them verbatim made the command un-runnable. This
/// strips them (→ `/review`, `/ship-phase`) while keeping any literal subcommand.
fn command_token(display: &str) -> String {
    display
        .split_whitespace()
        .take_while(|t| !t.starts_with('[') && !t.starts_with('<'))
        .collect::<Vec<_>>()
        .join(" ")
}

const SPLASH_DURATION: u8 = 1; // any nonzero value; splash waits for keypress, not a timer

/// Forge "heat pulse" spinner — a glowing ember that swells and fades while the
/// smith works. Rendered in the live heat color (see `heat_color`).
const FORGE_SPINNER: &[&str] = &["·", "∘", "○", "◉", "●", "◉", "○", "∘"];

/// Anvil's molten brand orange (#FF8C00) — title bar + the ⬡ mark.
const FORGE_MOLTEN: Color = Color::Rgb(255, 140, 0);

/// Blacksmith heat scale: stops from cold iron (steel blue-grey) up through
/// dull red, cherry, and orange to amber. `heat_color` interpolates between them.
const HEAT_STOPS: &[(f32, (u8, u8, u8))] = &[
    (0.00, (84, 96, 120)),  // cold iron
    (0.30, (120, 60, 50)),  // warming
    (0.50, (170, 55, 38)),  // dull red
    (0.72, (210, 75, 30)),  // cherry
    (0.88, (226, 110, 34)), // orange ember
    (1.00, (255, 150, 20)), // amber / molten
];

/// Color for a heat value in 0.0..=1.0, linearly interpolated across HEAT_STOPS.
fn heat_color(h: f32) -> Color {
    let h = h.clamp(0.0, 1.0);
    let mut lo = HEAT_STOPS[0];
    let mut hi = HEAT_STOPS[HEAT_STOPS.len() - 1];
    for w in HEAT_STOPS.windows(2) {
        if h >= w[0].0 && h <= w[1].0 {
            lo = w[0];
            hi = w[1];
            break;
        }
    }
    let span = (hi.0 - lo.0).max(f32::EPSILON);
    let t = ((h - lo.0) / span).clamp(0.0, 1.0);
    let lerp = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
    Color::Rgb(
        lerp(lo.1 .0, hi.1 .0),
        lerp(lo.1 .1, hi.1 .1),
        lerp(lo.1 .2, hi.1 .2),
    )
}

/// A forged blade used as the live "is it working?" indicator. Anatomy:
/// `--{====>` — `--` grip, `{` crossguard, `====` blade, `>` tip.
/// While the smith works the whole blade glows in the live heat color and grows
/// from a stub to full length, then repeats (a forge pulse). When the forge is
/// idle (`ready`) it rests as a finished, cooled sword in its true colors:
/// brown grip, dark-gray guard, steel blade. The rendered width is constant in
/// both states so trailing text never jitters frame to frame.
const SWORD_BLADE_MAX: usize = 8;
const SWORD_GRIP_COLOR: Color = Color::Rgb(139, 90, 43); // brown
const SWORD_GUARD_COLOR: Color = Color::Rgb(90, 90, 90); // dark gray
const SWORD_BLADE_COLOR: Color = Color::Rgb(205, 205, 205); // steel / light gray

fn forge_sword_spans(tick: u64, heat: f32, ready: bool) -> Vec<Span<'static>> {
    let bold = Modifier::BOLD;
    if ready {
        // Finished, cooled blade at full length, in its true colors.
        let blade = format!("{}>", "=".repeat(SWORD_BLADE_MAX));
        vec![
            Span::styled(
                "--",
                Style::default().fg(SWORD_GRIP_COLOR).add_modifier(bold),
            ),
            Span::styled(
                "{",
                Style::default().fg(SWORD_GUARD_COLOR).add_modifier(bold),
            ),
            Span::styled(
                blade,
                Style::default().fg(SWORD_BLADE_COLOR).add_modifier(bold),
            ),
        ]
    } else {
        // Forge pulse: blade grows 1..=MAX then repeats, whole sword glowing hot.
        let n = 1 + (tick / 3) as usize % SWORD_BLADE_MAX;
        let pad = " ".repeat(SWORD_BLADE_MAX - n);
        vec![Span::styled(
            format!("--{{{}>{}", "=".repeat(n), pad),
            Style::default().fg(heat_color(heat)).add_modifier(bold),
        )]
    }
}

/// The blacksmith's name for a heat level — shown in the title bar.
fn heat_name(h: f32) -> &'static str {
    match h {
        x if x < 0.12 => "cold",
        x if x < 0.34 => "warming",
        x if x < 0.55 => "dull red",
        x if x < 0.74 => "cherry",
        x if x < 0.90 => "orange",
        _ => "amber",
    }
}

/// Role-specific colors used consistently for labels, headers, splash, chat prefixes,
/// and during quick-setup model picking. Coder=blue, R1=purple (magenta), R2=lime (bright green).
const ROLE_CODER: Color = Color::LightBlue;
const ROLE_R1: Color = Color::Magenta;
const ROLE_R2: Color = Color::Rgb(50, 255, 127);

/// Wizard amethyst — the setup wizard's signature color (🪄 title + default
/// border). Deliberately purple/magical and distinct from the molten-orange
/// brand color the other popups share.
const WIZARD_PURPLE: Color = Color::Rgb(168, 120, 240);

/// Color for [system] messages (e.g. "Work in this Repo: C:\Anvil", gate instructions,
/// confirmations after /save-*/critical-*, "✓ ... complete" notices, etc.).
/// Distinct from normal chat, [you] (green), [coder] (light blue), reviewer findings (cyan), etc.
/// Yellow provides good visual pop for meta/system notices (as it did before).
const SYSTEM_COLOR: Color = Color::Yellow;

/// Known providers: (display_name, suggested_connection_name, provider_type, base_url, needs_api_key)
/// base_url = "" means the client uses the provider's SDK default (anthropic, google).
const PROVIDER_PRESETS: &[(&str, &str, &str, &str, bool)] = &[
    ("Anthropic", "anthropic", "anthropic", "", true),
    (
        "OpenAI",
        "openai",
        "openai_compat",
        "https://api.openai.com/v1",
        true,
    ),
    ("xAI", "xai", "openai_compat", "https://api.x.ai/v1", true),
    ("Google", "google", "google", "", true),
    (
        "Groq",
        "groq",
        "openai_compat",
        "https://api.groq.com/openai/v1",
        true,
    ),
    (
        "Mistral",
        "mistral",
        "openai_compat",
        "https://api.mistral.ai/v1",
        true,
    ),
    (
        "Together AI",
        "together",
        "openai_compat",
        "https://api.together.xyz/v1",
        true,
    ),
    (
        "OpenRouter",
        "openrouter",
        "openai_compat",
        "https://openrouter.ai/api/v1",
        true,
    ),
    (
        "Fireworks",
        "fireworks",
        "openai_compat",
        "https://api.fireworks.ai/inference/v1",
        true,
    ),
    (
        "Perplexity",
        "perplexity",
        "openai_compat",
        "https://api.perplexity.ai",
        true,
    ),
    (
        "DeepSeek",
        "deepseek",
        "openai_compat",
        "https://api.deepseek.com",
        true,
    ),
    (
        "Cohere",
        "cohere",
        "openai_compat",
        "https://api.cohere.com/v2",
        true,
    ),
    ("Azure", "azure", "azure_openai", "", true),
    ("AWS", "aws", "openai_compat", "", true),
    ("Vertex AI", "vertex", "openai_compat", "", true),
    (
        "Gradient",
        "gradient",
        "openai_compat",
        "https://inference.do-ai.run/v1",
        true,
    ),
    (
        "Ollama (local)",
        "ollama",
        "openai_compat",
        "http://localhost:11434/v1",
        false,
    ),
    (
        "LM Studio (local)",
        "lmstudio",
        "openai_compat",
        "http://localhost:1234/v1",
        false,
    ),
    ("Other / custom", "custom", "openai_compat", "", true),
];

/// Suggest a conventional environment variable name for a provider connection.
/// Used so that when the user pastes a key we can auto `std::env::set_var` it (current process),
/// store CredentialRef::Env, and give the user exact `setx` / profile instructions.
/// Prioritizes well-known names (XAI_API_KEY etc.) so tools and user scripts stay compatible.
fn suggest_env_var_name(conn_name: &str, base_url: Option<&str>) -> String {
    let n = conn_name.to_lowercase();
    if n == "xai" || n.contains("xai") {
        return "XAI_API_KEY".to_string();
    }
    if n == "openai" || n.contains("openai") {
        return "OPENAI_API_KEY".to_string();
    }
    if n.contains("groq") {
        return "GROQ_API_KEY".to_string();
    }
    if n.contains("anthropic") {
        return "ANTHROPIC_API_KEY".to_string();
    }
    if n.contains("mistral") {
        return "MISTRAL_API_KEY".to_string();
    }
    if n.contains("together") {
        return "TOGETHER_API_KEY".to_string();
    }
    if n.contains("fireworks") {
        return "FIREWORKS_API_KEY".to_string();
    }
    if n.contains("perplexity") {
        return "PERPLEXITY_API_KEY".to_string();
    }
    if n.contains("deepseek") {
        return "DEEPSEEK_API_KEY".to_string();
    }
    if n.contains("cohere") {
        return "COHERE_API_KEY".to_string();
    }
    if n.contains("azure") {
        return "AZURE_OPENAI_API_KEY".to_string();
    }
    if n.contains("google") || n.contains("gemini") || n.contains("vertex") {
        return "GOOGLE_API_KEY".to_string();
    }
    if n.contains("aws") {
        return "AWS_BEDROCK_API_KEY".to_string();
    }
    if n.contains("ollama") {
        return "OLLAMA_API_KEY".to_string();
    }
    if n.contains("lmstudio") || n.contains("lm-studio") {
        return "LMSTUDIO_API_KEY".to_string();
    }

    // Generic fallback based on the connection name the user chose (e.g. "my-xai" -> MY_XAI_API_KEY)
    let base = base_url.unwrap_or_default();
    let mut stem = conn_name.to_uppercase();
    stem = stem.replace(|c: char| !c.is_alphanumeric(), "_");
    stem = stem.trim_matches('_').to_string();
    if stem.is_empty() {
        stem = "ANVIL".to_string();
    }
    if base.contains("x.ai") && !stem.contains("XAI") {
        stem = "XAI".to_string();
    }
    format!("{}_API_KEY", stem)
}

fn models_for_connection(provider_type: &str, base_url: Option<&str>) -> &'static [&'static str] {
    let url = base_url.unwrap_or("");
    if url.contains("x.ai") {
        // Static suggestions as last-resort fallback only. The live /v1/models path (when the
        // provider connection + key are valid) should return the current catalog for the key.
        return &[
            "grok-3",
            "grok-3-fast",
            "grok-3-mini",
            "grok-3-mini-fast",
            "grok-2-1212",
            "grok-beta",
            "grok-4.3",
            "grok-4.2",
            "grok-build-0.1",
        ];
    }
    if url.contains("groq.com") {
        return &[
            "llama-3.3-70b-versatile",
            "llama-3.1-70b-versatile",
            "llama-3.1-8b-instant",
            "mixtral-8x7b-32768",
            "gemma2-9b-it",
            "llama-guard-3-8b",
        ];
    }
    if url.contains("mistral.ai") {
        return &[
            "mistral-large-latest",
            "mistral-medium-latest",
            "mistral-small-latest",
            "codestral-latest",
            "open-mistral-nemo",
            "open-codestral-mamba",
        ];
    }
    if url.contains("together.xyz") {
        return &[
            "meta-llama/Llama-3.3-70B-Instruct-Turbo",
            "meta-llama/Meta-Llama-3.1-8B-Instruct-Turbo",
            "Qwen/Qwen2.5-72B-Instruct-Turbo",
            "deepseek-ai/DeepSeek-R1",
            "mistralai/Mixtral-8x7B-Instruct-v0.1",
        ];
    }
    if url.contains("openrouter.ai") {
        return &[
            "anthropic/claude-opus-4-8",
            "anthropic/claude-sonnet-4-6",
            "openai/gpt-4o",
            "google/gemini-2.5-pro-preview",
            "meta-llama/llama-3.3-70b-instruct",
            "deepseek/deepseek-r1",
        ];
    }
    if url.contains("fireworks.ai") {
        return &[
            "accounts/fireworks/models/llama-v3p3-70b-instruct",
            "accounts/fireworks/models/llama-v3p1-405b-instruct",
            "accounts/fireworks/models/mixtral-8x7b-instruct",
            "accounts/fireworks/models/qwen2p5-72b-instruct",
        ];
    }
    if url.contains("perplexity.ai") {
        return &[
            "llama-3.1-sonar-large-128k-online",
            "llama-3.1-sonar-small-128k-online",
            "llama-3.1-sonar-huge-128k-online",
        ];
    }
    if url.contains("deepseek.com") {
        return &["deepseek-chat", "deepseek-coder", "deepseek-reasoner"];
    }
    if url.contains("cohere.com") {
        return &["command-r-plus", "command-r", "command-light", "command"];
    }
    if url.contains("openai.com") {
        return &[
            "gpt-4o",
            "gpt-4o-mini",
            "gpt-4-turbo",
            "gpt-4",
            "gpt-3.5-turbo",
            "o1-preview",
            "o1-mini",
            "o3-mini",
        ];
    }
    match provider_type {
        "anthropic" => &[
            "claude-opus-4-8",
            "claude-sonnet-4-6",
            "claude-haiku-4-5-20251001",
            "claude-3-5-sonnet-20241022",
            "claude-3-5-haiku-20241022",
            "claude-3-opus-20240229",
        ],
        "google" => &[
            "gemini-2.5-pro-preview-06-05",
            "gemini-2.5-flash-preview-05-20",
            "gemini-2.0-flash",
            "gemini-1.5-pro",
            "gemini-1.5-flash",
        ],
        _ => &[],
    }
}

/// ASCII anvil silhouette (44 cols wide, 9 rows) — fallback when PNG decode fails.
const SPLASH_ANVIL: &[&str] = &[
    "              ┌──────────────┐              ",
    "              │  ░░░░░░░░░░  │              ",
    "     ┌────────┴──────────────┴────────┐     ",
    "     │                                │     ",
    "     │                                │     ",
    "     └─────────────┬──────────────────┘     ",
    "                   │                        ",
    "             ┌─────┴──────┐                 ",
    "             └────────────┘                 ",
];

/// Logo PNG bundled at compile time — decoded at runtime into half-block pixels.
static LOGO_BYTES: &[u8] = include_bytes!("../anvil_logo.png");

/// Sentinel list entry (one per configured provider) shown in role assignment so a
/// provider whose `/models` endpoint returns nothing (Gradient, locked-down gateways,
/// some self-hosted vLLM) can still have a role assigned by typing the exact model id.
/// Encoded as "<label>  [provider]" so the existing provider parsing/coloring applies.
const MANUAL_ENTRY_LABEL: &str = "+ Enter a model ID manually";

/// Heuristic: is the OS keyring unusable on this host? On Linux the keyring needs a
/// running Secret Service (gnome-keyring/KWallet) reachable over D-Bus. Headless
/// servers (SSH sessions, containers) have none, so keyring reads/writes fail silently.
/// When this is true the setup wizard steers credentials to environment variables.
fn keyring_likely_unavailable() -> bool {
    cfg!(target_os = "linux")
        && std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_none()
        && std::env::var_os("DISPLAY").is_none()
        && std::env::var_os("WAYLAND_DISPLAY").is_none()
}

/// Turn a free-text plan name into a filesystem-safe slug (lowercase, `[a-z0-9_]`).
/// "Front Page" -> "front_page"; collapses runs of separators; trims leading/trailing `_`.
fn slugify_plan_name(raw: &str) -> String {
    let mut out = String::new();
    let mut prev_sep = false;
    for ch in raw.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_sep = false;
        } else if (ch == '_' || ch == '-' || ch == ' ') && !out.is_empty() && !prev_sep {
            out.push('_');
            prev_sep = true;
        }
    }
    out.trim_matches('_').to_string()
}

/// Steps in the in-TUI configuration wizard (launched via /config or /setup).
#[derive(Clone, Debug, PartialEq)]
enum WizardStep {
    MainMenu,
    // Provider connection flow
    ProviderType,
    ProviderName,
    BaseUrl,
    CredentialKind,
    #[allow(dead_code)]
    EnvVarName,
    ApiKeySecret,
    // Model binding flow
    BindingProvider,
    ModelName,
    BindingNote,
    // Role assignment
    RoleAssignment {
        role: String,
    },
    // Free-text model-id entry for a role, reached from the "+ Enter a model ID
    // manually" sentinel when a provider doesn't publish a usable /models list.
    RoleManualModel {
        role: String,
        provider: String,
    },
    // /swap: pick which role (coder / R1 / R2) to re-point before choosing a model.
    SwapRolePick,
    // Special first-run quick Ollama path: after auto-adding the local provider,
    // user scrolls the *live* fetched model list and picks (no more hardcoded defaults).
    QuickOllamaModelPick {
        role: String,
    },
}

/// Lightweight state for the /config wizard.
#[derive(Clone, Debug)]
struct ConfigWizard {
    step: WizardStep,
    // Current working lists for scrolling selection
    list_items: Vec<String>,
    list_selected: usize,
    list_title: String,

    // Scratch data for the current provider / binding being created
    provider_type: Option<String>,
    provider_name: Option<String>,
    base_url: Option<String>,
    cred_kind: Option<String>, // "keyring" or "env"
    env_var: Option<String>,
    api_key: Option<String>,
    no_auth: bool, // true for local providers — skips credential steps
    model_options: Vec<String>,

    binding_provider: Option<String>,
    model: Option<String>,
    note: Option<String>,

    // Which role we are currently assigning (for RoleAssignment step)
    current_role: Option<String>,

    // Populated only for the Quick Ollama model picker flow so we can present
    // the real models the user has `ollama pull`'ed (no baked-in llama3.2 etc).
    ollama_model_list: Vec<String>,

    // True while a /swap flow is active: assign exactly one role then return to chat,
    // instead of walking the coder -> R1 -> R2 setup chain.
    swap_mode: bool,
}

/// The program a shell command invokes — its first whitespace token, lowercased
/// (e.g. "cargo build" → "cargo"). Used for the session approval allowlist.
fn program_of(cmd: &str) -> String {
    cmd.split_whitespace().next().unwrap_or("").to_lowercase()
}

/// Whether a config layer explicitly sets the given role (for /status provenance).
fn role_is_set(c: &AnvilConfig, role: &str) -> bool {
    match role {
        "coder" => c.roles.coder.is_some(),
        "reviewer_a" => c.roles.reviewer_a.is_some(),
        "reviewer_b" => c.roles.reviewer_b.is_some(),
        _ => false,
    }
}

/// Entry point called from main when no subcommand (or `anvil ui`) is given.
pub fn run_ui(root: &Path) -> Result<()> {
    // Load any secrets from .anvil/.env into the process env *before* we do anything
    // that might resolve credentials (providers, roles, chat, etc.). This is what makes
    // "paste once during setup" work across PowerShell, bash, fish, WSL, Docker, CI, etc.
    load_local_env(root);

    let mut app = App::new(root.to_path_buf());

    // Detect first-run (no anvil.toml or roles incomplete). This drives the prominent banner
    // and (most importantly) the automatic first-time configuration wizard.
    app.first_run = is_unconfigured(root);

    // For a brand-new user (or incomplete roles) we immediately walk them through setup
    // via the wizard. This is the "someone off the street can sit down and get configured"
    // requirement. /config (or /setup) still works later for changes.
    if app.first_run && app.config_wizard.is_none() {
        app.start_config_wizard();
    }

    // For an already-configured project, just announce the working directory.
    // The coder is a real agent now — it reads files on demand via its tools, so
    // there's no "Work in this Repo" prompt and no manual /include to grant access.
    if !app.first_run {
        // Seed the project context files (decisions / assumptions / scratch /
        // ARCHITECTURE.md) with explanatory templates if missing, so they're
        // discoverable. Templates aren't injected until they have real content.
        crate::agent::ensure_context_files(&app.root);
        app.push_system(&format!(
            "Working in {}. The coder reads, edits, and runs the project directly — just tell it what to build.",
            app.root.display()
        ));
        // The review gates diff git, so make sure this project actually is a git repo
        // with a baseline before any work starts (resolves the non-git hole on boot).
        app.bootstrap_git_repo();
        // Continuity: if a prior session was persisted for this project, show a
        // short transcript tail so the chat doesn't look blank. The agent itself
        // reloads the full bounded history when it's first used this run.
        let prior = crate::agent::load_session(&app.root);
        if !prior.is_empty() {
            app.restore_session_preview(&prior);
        }
    }

    // Fire a background check for a newer release. If one exists, the header shows
    // a pulsing "UPDATE vX.Y.Z — /update to apply" indicator. Non-blocking; silent
    // on failure; honors ANVIL_NO_UPDATE_CHECK.
    app.spawn_update_check();

    // Refresh models.dev metadata in the background (cached 7 days), then warn if
    // the coder's model is known to lack tool-calling (it can't drive the loop).
    app.spawn_modelsdev_refresh();
    app.warn_coder_tool_calling();

    // Setup terminal (raw mode + alternate screen). We must restore on any exit path.
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableBracketedPaste)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run the app loop; capture result so we can always restore the terminal.
    let run_result = run_app_loop(&mut terminal, &mut app);

    // Restore terminal state (critical on Windows and for users who ^C).
    let _ = disable_raw_mode();
    let _ = execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableBracketedPaste
    );
    let _ = terminal.show_cursor();

    run_result
}

fn is_unconfigured(root: &Path) -> bool {
    // Use the *merged* config (global base + optional per-repo override), not the
    // presence of a project anvil.toml — a fresh repo has no local file yet is
    // fully configured by the global config. load_config errors only when neither
    // exists; otherwise we're configured once both reviewers resolve.
    match load_config(root) {
        Ok(cfg) => cfg.roles.reviewer_a.is_none() || cfg.roles.reviewer_b.is_none(),
        Err(_) => true,
    }
}

/// Lightweight live GPU stats (primarily NVIDIA via nvidia-smi).
/// Refreshed periodically for local model users to see util + VRAM usage (used/total).
/// Note: nvidia-smi "used" includes CUDA context overhead + driver reservations in addition
/// to actual model weights/KV cache reported by Ollama /api/ps. Discrepancies of several GB
/// are normal; use /loaded + header together to cross-check.
#[derive(Clone, Debug, Default)]
struct GpuStat {
    name: String,
    util: u8,       // 0-100
    mem_used: u32,  // MiB used (driver view)
    mem_free: u32,  // MiB free
    mem_total: u32, // MiB total
}

struct App {
    root: PathBuf,
    messages: Vec<String>,
    input: String,
    // Byte offset of the edit cursor into `input` (always on a char boundary,
    // always <= input.len()). Drives arrow/Home/End navigation and where typed
    // chars are inserted. 0 = before the first char, input.len() = at the end.
    input_cursor: usize,
    view_offset: usize, // wrapped-row offset into full transcript (manual scroll when !follow_bottom)
    follow_bottom: bool, // when true, render auto-scrolls so newest content is visible at bottom of chat area
    last_max_scroll: u16, // cached from the last chat render: max scroll offset (in wrapped rows) that still shows content
    should_quit: bool,
    first_run: bool,
    /// One-shot guard so the git bootstrap (ensure repo + baseline commit) runs at
    /// most once per session, whether triggered at boot or right after setup.
    git_bootstrapped: bool,
    status_line: String,

    // For real LLM chat (phase 2+)
    runtime: Option<tokio::runtime::Runtime>,
    llm: LlmClient,
    cfg: Option<AnvilConfig>,
    llm_rx: Option<mpsc::UnboundedReceiver<String>>,

    // The autonomous coding agent (tool loop). Built lazily on the first chat
    // turn from the configured `coder` role, then reused so conversation history
    // and tool context persist across turns. Wrapped in Arc<Mutex> so the
    // streaming task can own a handle while the App keeps one.
    agent: Option<Arc<Mutex<Agent>>>,
    // Abort handle for the in-flight agent turn, so Ctrl+B can interrupt it.
    agent_task: Option<tokio::task::AbortHandle>,
    // Sends the user's y/n decision to an agent blocked on a run_command confirm.
    confirm_tx: Option<mpsc::UnboundedSender<bool>>,
    // The command awaiting confirmation (Some => render the selectable prompt).
    awaiting_confirm: Option<String>,
    // Highlighted option in the confirm prompt (0=yes once, 1=yes+remember, 2=no).
    confirm_selected: usize,
    // Programs (first command token) the user allowed for the rest of the session,
    // so the same kind of command isn't re-confirmed over and over.
    approved_programs: std::collections::HashSet<String>,
    // The /approvals checklist editor (Some => render + capture keys for it).
    approvals_editor: Option<ApprovalsEditor>,
    // True while we have an open "[coder] " line accumulating streamed text. A
    // tool/confirm line closes it so the next text delta starts a fresh line.
    assistant_open: bool,
    // True while a tool is actually executing (between [tool-start] and [tool-end]).
    // Drives the status label: "smithing…" when acting vs "forging…" when thinking.
    tool_active: bool,

    // Self-update. The boot check sends the newer version (if any) over update_rx;
    // update_available then drives the pulsing header indicator. /update applies it
    // and streams status lines back over update_apply_rx.
    update_rx: Option<mpsc::UnboundedReceiver<String>>,
    update_available: Option<String>,
    update_apply_rx: Option<mpsc::UnboundedReceiver<String>>,
    update_in_progress: bool,

    // Workflow + plan gate (phase 3)
    stage: WorkflowStage,
    gate_rx: Option<mpsc::UnboundedReceiver<String>>, // signals from spawn_blocking plan gate
    // Active sequential review gate (R1 → fix → pause → R2 → fix → pause → summary).
    // None when no gate is running.
    gate_flow: Option<GateFlow>,

    // Slash command palette (opened by pressing / ; supports arrows + live filter)
    showing_command_palette: bool,
    command_selected: usize,

    // In-TUI configuration wizard (/config). When Some, normal chat is suspended
    // and the UI drives a step-by-step provider / binding / role + key flow.
    config_wizard: Option<ConfigWizard>,

    // Animation state (frame counter + splash countdown)
    splash_ticks: u8, // nonzero = showing splash; cleared on first keypress
    anim_tick: u64,   // increments every frame, drives spinner + cursor blink

    // Forge "heat" 0.0 (cold iron) .. 1.0 (amber). Ramps up while the agent works
    // (streaming / tool calls) or the GPU is busy, and cools slowly when idle —
    // the header borders + spinner glow along the blacksmith heat scale.
    forge_heat: f32,

    // When true the input characters are masked in the UI (for API keys)
    input_secret: bool,

    // Large clipboard pastes are collapsed to a "[Pasted Content N chars]" placeholder in
    // the input box; the full text is stored here (placeholder -> full) and expanded back
    // when the message is submitted. Keeps a 5,000-line paste from flooding the composer.
    pending_pastes: Vec<(String, String)>,

    // Cached result of the Ollama probe (localhost:11434). Decides whether the
    // "Quick local Ollama setup" option is offered on first boot / in the wizard.
    // None = not yet probed. Populated lazily by is_ollama_available().
    ollama_available_cached: Option<bool>,

    // Lightweight document viewer popup (for /view-plan, /view-reviews etc.).
    // Gives a focused "card" experience for inspecting gate artifacts (plan + the two reviews)
    // before the explicit accept step — inspired by deliberate plan/approve flows.
    viewing_doc: Option<(String, String)>, // (title, full_content)
    doc_scroll: u16,                       // vertical scroll offset (rows) for the doc viewer popup

    // Live GPU stats (NVIDIA etc.) polled for the top-right header display.
    // Useful when running local models via Ollama / LM Studio / etc.
    gpu_stats: Vec<GpuStat>,

    // Active chat turn context for structured logging (turn_id correlates user + all deltas + full wire + final UI).
    // Cleared on stream finish. Enables debugging truncation/chopping and later retrieval of prior interactions.
    current_turn_id: Option<String>,
    current_role: Option<String>,
    current_binding: Option<String>,
    current_model: Option<String>,

    // Per-session transcript log file (one timestamped .jsonl inside .anvil/ per launch of the TUI).
    // This replaces the single always-appending chat.jsonl so each session is isolated and easy to delete when not useful.
    session_chat_log: Option<PathBuf>,
}

impl App {
    fn new(root: PathBuf) -> Self {
        // Resolve the project root to a real absolute path. The CLI default is ".",
        // which is useless to display and awkward for the agent's tools — turn it into
        // the actual directory (e.g. C:\Anvil). Strip Windows' \\?\ verbatim prefix so
        // it reads cleanly in the header and "Working in ..." line.
        let root = resolve_project_root(&root);

        // Defensive: make sure any .anvil/.env secrets are visible even if someone constructs
        // an App directly (the normal call site run_ui already calls load_local_env too).
        load_local_env(&root);

        // Multi-thread runtime so that spawned LLM streaming tasks (reqwest + SSE parsing)
        // make progress on background worker threads. Tokens are sent over the mpsc to the
        // main UI thread which only does cheap non-blocking drains + redraws.
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .worker_threads(2)
            .build()
            .ok();

        let mut app = Self {
            root,
            messages: vec![],
            input: String::new(),
            input_cursor: 0,
            view_offset: 0,
            follow_bottom: true,
            last_max_scroll: 0,
            should_quit: false,
            first_run: false,
            git_bootstrapped: false,
            status_line: String::new(),
            runtime,
            llm: LlmClient::new(),
            cfg: None,
            llm_rx: None,
            agent: None,
            agent_task: None,
            confirm_tx: None,
            awaiting_confirm: None,
            approvals_editor: None,
            confirm_selected: 0,
            approved_programs: std::collections::HashSet::new(),
            assistant_open: false,
            tool_active: false,
            update_rx: None,
            update_available: None,
            update_apply_rx: None,
            update_in_progress: false,
            stage: WorkflowStage::Talk,
            gate_rx: None,
            gate_flow: None,
            showing_command_palette: false,
            command_selected: 0,
            config_wizard: None,
            splash_ticks: SPLASH_DURATION,
            anim_tick: 0,
            forge_heat: 0.0,
            input_secret: false,
            pending_pastes: Vec::new(),
            ollama_available_cached: None,
            viewing_doc: None,
            doc_scroll: 0,
            gpu_stats: vec![],
            current_turn_id: None,
            current_role: None,
            current_binding: None,
            current_model: None,
            session_chat_log: None,
        };

        // Establish the per-session chat log file *immediately*, before any push_system (which will now write into it).
        // One file per TUI launch: .anvil/chat-YYYY-MM-DD-HH-MM-SS-mmm.jsonl
        // This makes it trivial to look at exactly one session or rm old ones you don't care about.
        // Keep the local session state out of git (no-op outside a git repo / if
        // already ignored). Now that keys live globally, .anvil/ is just logs +
        // working memory — housekeeping, not secrets.
        ensure_anvil_gitignored(&app.root);

        if let Ok(dir) = ensure_anvil_dir(&app.root) {
            let now = Utc::now();
            // Use milliseconds and replace '.' so the filename is clean on all filesystems.
            let ts = now
                .format("%Y-%m-%d-%H-%M-%S%.3f")
                .to_string()
                .replace('.', "-");
            let filename = format!("chat-{}.jsonl", ts);
            app.session_chat_log = Some(dir.join(&filename));

            // Write a self-describing header record for this session (handy when you have many old session logs).
            let start_rec = serde_json::json!({
                "ts": now.to_rfc3339(),
                "event": "session_start",
                "root": app.root.display().to_string(),
                "version": env!("CARGO_PKG_VERSION"),
            });
            if let Ok(mut f) = OpenOptions::new()
                .create(true)
                .append(true)
                .open(app.session_chat_log.as_ref().unwrap())
            {
                let _ = writeln!(f, "{}", start_rec);
            }
        }

        // Best-effort load of existing config so we can do real chat immediately if roles are set.
        app.cfg = load_config(&app.root).ok();
        let has_reviewers = app
            .cfg
            .as_ref()
            .is_some_and(|c| c.roles.reviewer_a.is_some() && c.roles.reviewer_b.is_some());

        app.push_system("Welcome");
        app.push_system("Type to chat with your coder. Real streaming to your configured model. /plan /phase-done /status /help /quit");
        if !has_reviewers {
            app.first_run = true;
            // The smooth first-time experience auto-launches the config wizard in run_ui().
            // We keep one gentle note here; the wizard itself will guide the user.
            app.push_system("First run detected — the setup wizard will open so you can connect a model and assign roles in under a minute.");
        } else {
            app.push_system(
                "Configuration loaded. Reviewers are distinct — gates will enforce R1 then R2.",
            );
        }
        app.reconcile_stage_from_disk();
        app.update_status();

        // Warm the Ollama probe cache on first run so the main menu can decide
        // immediately whether to show the Quick Ollama option (no surprise delay or flicker).
        if app.first_run {
            let _ = app.is_ollama_available();
        }

        // Initial GPU snapshot so the top-right stats appear immediately.
        app.refresh_gpu_stats();

        app
    }

    fn push(&mut self, line: String) {
        self.messages.push(line);
        // Note: we no longer mutate view_offset here. Follow-bottom behavior (and manual scroll)
        // is applied at render time using follow_bottom + Paragraph::scroll so the chat grows
        // naturally toward the bottom without jumping the window on every token or submit.
    }

    fn push_system(&mut self, text: &str) {
        self.log_chat_event("system", None, None, None, None, text);
        self.push(format!("[system] {}", text));
    }

    /// Finish the current open "[coder] " streaming line (if any). Drops it
    /// entirely when it never received any text (e.g. the model went straight
    /// to a tool call). Called before showing tool/confirm lines so segments
    /// don't run together.
    fn close_assistant_line(&mut self) {
        if self.assistant_open {
            let drop_empty = self
                .messages
                .last()
                .map(|l| l.trim_end() == "[coder]")
                .unwrap_or(false);
            if drop_empty {
                self.messages.pop();
            }
            self.assistant_open = false;
        }
    }

    /// Scroll the chat up by `n` wrapped rows. Leaving follow-bottom starts from
    /// the current bottom so the view moves up smoothly (not jumping to the top).
    fn scroll_up(&mut self, n: usize) {
        if self.follow_bottom {
            self.follow_bottom = false;
            self.view_offset = self.last_max_scroll as usize;
        }
        self.view_offset = self.view_offset.saturating_sub(n);
    }

    /// Scroll the chat down by `n` wrapped rows. Reaching the bottom re-engages
    /// follow-bottom so new output keeps the live line in view automatically.
    fn scroll_down(&mut self, n: usize) {
        let max = self.last_max_scroll as usize;
        let next = self.view_offset.saturating_add(n);
        if next >= max {
            self.view_offset = max;
            self.follow_bottom = true;
        } else {
            self.follow_bottom = false;
            self.view_offset = next;
        }
    }

    /// Display a project context file (decisions / assumptions / scratch / arch)
    /// in the transcript, or note it's empty.
    fn show_context_file(&mut self, path: PathBuf, label: &str) {
        match std::fs::read_to_string(&path) {
            Ok(content) if !content.trim().is_empty() => {
                self.push_system(label);
                self.push(content);
                self.follow_bottom = true;
            }
            _ => self.push_system(&format!("{} — empty.", label)),
        }
    }

    /// The prompt shown at the start of the input box (varies in the config wizard).
    fn input_prompt(&self) -> &'static str {
        if let Some(w) = &self.config_wizard {
            match &w.step {
                WizardStep::ProviderName => "provider name> ",
                WizardStep::BaseUrl => "base url> ",
                WizardStep::EnvVarName => "env var name> ",
                WizardStep::ApiKeySecret => "api key (hidden)> ",
                WizardStep::ModelName => "model id> ",
                WizardStep::RoleManualModel { .. } => "model id> ",
                WizardStep::BindingNote => "note (optional)> ",
                _ => "config> ",
            }
        } else {
            "> "
        }
    }

    // ─── Input editing + cursor navigation ──────────────────────────────────
    // The cursor is a byte offset into `self.input`. Every helper below first
    // snaps it back onto a valid char boundary (<= len) so direct assignments
    // to `self.input` elsewhere can never make a later edit panic.

    /// Clamp the cursor to a valid char boundary within the current input.
    fn clamp_cursor(&mut self) {
        if self.input_cursor > self.input.len() {
            self.input_cursor = self.input.len();
        }
        while self.input_cursor > 0 && !self.input.is_char_boundary(self.input_cursor) {
            self.input_cursor -= 1;
        }
    }

    /// Replace the whole input buffer and park the cursor at the end (used by
    /// wizard prefills and similar). Clearing should set the cursor to 0.
    fn set_input(&mut self, s: String) {
        self.input = s;
        self.input_cursor = self.input.len();
    }

    /// Insert a char at the cursor and advance past it.
    fn input_insert(&mut self, ch: char) {
        self.clamp_cursor();
        self.input.insert(self.input_cursor, ch);
        self.input_cursor += ch.len_utf8();
    }

    /// Insert a string at the cursor and advance past it (used for paste).
    fn input_insert_str(&mut self, s: &str) {
        self.clamp_cursor();
        self.input.insert_str(self.input_cursor, s);
        self.input_cursor += s.len();
    }

    /// Delete the char before the cursor (Backspace).
    fn input_backspace(&mut self) {
        self.clamp_cursor();
        if self.input_cursor == 0 {
            return;
        }
        // Walk back to the previous char boundary, then remove that char.
        let mut prev = self.input_cursor - 1;
        while prev > 0 && !self.input.is_char_boundary(prev) {
            prev -= 1;
        }
        self.input.replace_range(prev..self.input_cursor, "");
        self.input_cursor = prev;
    }

    /// Delete the char at the cursor (Delete / forward-delete).
    fn input_delete_forward(&mut self) {
        self.clamp_cursor();
        if self.input_cursor >= self.input.len() {
            return;
        }
        let mut next = self.input_cursor + 1;
        while next < self.input.len() && !self.input.is_char_boundary(next) {
            next += 1;
        }
        self.input.replace_range(self.input_cursor..next, "");
    }

    /// Move the cursor one char left.
    fn input_left(&mut self) {
        self.clamp_cursor();
        if self.input_cursor == 0 {
            return;
        }
        let mut prev = self.input_cursor - 1;
        while prev > 0 && !self.input.is_char_boundary(prev) {
            prev -= 1;
        }
        self.input_cursor = prev;
    }

    /// Move the cursor one char right.
    fn input_right(&mut self) {
        self.clamp_cursor();
        if self.input_cursor >= self.input.len() {
            return;
        }
        let mut next = self.input_cursor + 1;
        while next < self.input.len() && !self.input.is_char_boundary(next) {
            next += 1;
        }
        self.input_cursor = next;
    }

    /// Jump the cursor to the start / end of the current line (Home / End). With
    /// a single-line input these are start/end of the whole buffer; with
    /// Shift+Enter multi-line input they stop at the surrounding newlines.
    fn input_home(&mut self) {
        self.clamp_cursor();
        self.input_cursor = self.input[..self.input_cursor]
            .rfind('\n')
            .map(|i| i + 1)
            .unwrap_or(0);
    }

    fn input_end(&mut self) {
        self.clamp_cursor();
        self.input_cursor = match self.input[self.input_cursor..].find('\n') {
            Some(off) => self.input_cursor + off,
            None => self.input.len(),
        };
    }

    /// Move the cursor one word left / right (Ctrl+Left / Ctrl+Right). A word is
    /// a run of non-whitespace; we skip whitespace first, then the word.
    fn input_word_left(&mut self) {
        self.clamp_cursor();
        let bytes = self.input.as_bytes();
        let mut i = self.input_cursor;
        while i > 0 && bytes[i - 1].is_ascii_whitespace() {
            i -= 1;
        }
        while i > 0 && !bytes[i - 1].is_ascii_whitespace() {
            i -= 1;
        }
        self.input_cursor = i;
    }

    fn input_word_right(&mut self) {
        self.clamp_cursor();
        let bytes = self.input.as_bytes();
        let len = bytes.len();
        let mut i = self.input_cursor;
        while i < len && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        while i < len && !bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        self.input_cursor = i;
    }

    /// Move the cursor up one input line, keeping the same column where possible.
    /// Returns false when already on the first line so the caller can fall back to
    /// scrolling the chat (single-line input always returns false → chat scroll).
    fn input_up(&mut self) -> bool {
        self.clamp_cursor();
        let cur = self.input_cursor;
        let line_start = self.input[..cur].rfind('\n').map(|i| i + 1).unwrap_or(0);
        if line_start == 0 {
            return false; // already on the first line
        }
        let col = self.input[line_start..cur].chars().count();
        let prev_end = line_start - 1; // the '\n' that ends the previous line
        let prev_start = self.input[..prev_end]
            .rfind('\n')
            .map(|i| i + 1)
            .unwrap_or(0);
        self.input_cursor = self.input[prev_start..prev_end]
            .char_indices()
            .nth(col)
            .map(|(i, _)| prev_start + i)
            .unwrap_or(prev_end); // column past line end → clamp to end of that line
        true
    }

    /// Move the cursor down one input line, keeping the column. Returns false when
    /// already on the last line so the caller can fall back to scrolling the chat.
    fn input_down(&mut self) -> bool {
        self.clamp_cursor();
        let cur = self.input_cursor;
        let line_start = self.input[..cur].rfind('\n').map(|i| i + 1).unwrap_or(0);
        let col = self.input[line_start..cur].chars().count();
        let line_end = match self.input[cur..].find('\n') {
            Some(off) => cur + off,
            None => return false, // already on the last line
        };
        let next_start = line_end + 1;
        let next_end = match self.input[next_start..].find('\n') {
            Some(off) => next_start + off,
            None => self.input.len(),
        };
        self.input_cursor = self.input[next_start..next_end]
            .char_indices()
            .nth(col)
            .map(|(i, _)| next_start + i)
            .unwrap_or(next_end);
        true
    }

    /// The full text rendered in the input box (prompt + current input, masked if secret).
    fn input_full_text(&self) -> String {
        let display = if self.input_secret {
            "•".repeat(self.input.chars().count())
        } else {
            self.input.clone()
        };
        format!("{}{}", self.input_prompt(), display)
    }

    /// Show a short tail of a restored session so a relaunched TUI isn't blank.
    /// Renders only the last few user/assistant lines (one line each); the agent
    /// reloads the full bounded history (including tool turns) for its context.
    fn restore_session_preview(&mut self, msgs: &[ChatMessage]) {
        let turns = msgs
            .iter()
            .filter(|m| matches!(m.role, Role::User | Role::Assistant) && !m.text.trim().is_empty())
            .count();
        if turns == 0 {
            return;
        }
        self.push_system(&format!(
            "Session continued — the coder remembers {} prior message(s) from this project. Recent tail:",
            turns
        ));
        let convo: Vec<&ChatMessage> = msgs
            .iter()
            .filter(|m| matches!(m.role, Role::User | Role::Assistant) && !m.text.trim().is_empty())
            .collect();
        let start = convo.len().saturating_sub(6);
        for m in &convo[start..] {
            let first = m.text.lines().next().unwrap_or("").trim();
            let more = if m.text.lines().count() > 1 {
                " …"
            } else {
                ""
            };
            match m.role {
                Role::User => self.push(format!("[you] {}{}", first, more)),
                Role::Assistant => self.push(format!("[coder] {}{}", first, more)),
                _ => {}
            }
        }
        self.follow_bottom = true;
    }

    /// Drive the forge heat (0.0 cold .. 1.0 amber). The forge is hottest while
    /// the agent is actively streaming/calling tools, spikes on fresh activity,
    /// and is kept warm by GPU load (so local-model inference glows too). Heat
    /// ramps up fast and cools slowly, so embers linger after a turn ends.
    fn update_forge_heat(&mut self, fresh_activity: bool) {
        let active = self.llm_rx.is_some() || self.gate_rx.is_some();
        // Hottest GPU utilization as a 0..1 contribution.
        let gpu = self
            .gpu_stats
            .iter()
            .map(|g| g.util as f32 / 100.0)
            .fold(0.0_f32, f32::max);

        let mut target: f32 = if active { 0.85 } else { 0.0 };
        if fresh_activity {
            target = 1.0; // spike on new tokens / tool events
        }
        target = target.max(gpu * 0.9);

        // Asymmetric easing: stoke quickly, cool gradually.
        let rate = if target > self.forge_heat { 0.35 } else { 0.06 };
        self.forge_heat += (target - self.forge_heat) * rate;
        self.forge_heat = self.forge_heat.clamp(0.0, 1.0);
    }

    /// Write one JSON event line into this session's dedicated chat log file.
    /// The file is created on first write for the session (timestamped name chosen in App::new).
    /// All events for one launch of the TUI go into exactly one file so you can inspect or delete per-session.
    fn log_chat_event(
        &self,
        event: &str,
        turn_id: Option<&str>,
        role: Option<&str>,
        binding: Option<&str>,
        model: Option<&str>,
        content: &str,
    ) {
        if let Some(path) = &self.session_chat_log {
            let ts = Utc::now().to_rfc3339();
            let rec = serde_json::json!({
                "ts": ts,
                "event": event,
                "turn_id": turn_id,
                "role": role,
                "binding": binding,
                "model": model,
                "content": content,
            });
            if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
                let _ = writeln!(f, "{}", rec);
            }
        }
    }

    fn update_status(&mut self) {
        let proj = project_display_name(&self.root);
        let stage = if self.first_run || self.stage == WorkflowStage::Unconfigured {
            "UNCONFIGURED — Ctrl+S or /config for quick setup (Ollama if present)"
        } else {
            match self.stage {
                WorkflowStage::Talk => {
                    "TALK (build with the coder; /lock-plan when plan.md is ready)"
                }
                WorkflowStage::PlanReviewsComplete => {
                    "PLAN REVIEWED (R1/R2 done) — /accept-plan to quench"
                }
                WorkflowStage::PlanAccepted => {
                    "PLAN ACCEPTED — build phases; /accept-phase when done"
                }
                _ => "TALK",
            }
        };
        self.status_line = format!("Anvil — {}  |  {}", proj, stage);
    }

    /// Poll nvidia-smi (if present) for current GPU util + VRAM.
    /// Safe to call often; clears on any failure so the UI simply omits the GPU box.
    fn refresh_gpu_stats(&mut self) {
        let output = match std::process::Command::new("nvidia-smi")
            .args([
                "--query-gpu=name,utilization.gpu,memory.used,memory.total,memory.free",
                "--format=csv,noheader,nounits",
            ])
            .output()
        {
            Ok(o) => o,
            Err(_) => {
                self.gpu_stats.clear();
                return;
            }
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut stats = vec![];
        for line in stdout.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let cols: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
            if cols.len() < 5 {
                continue;
            }
            let name = cols[0].to_owned();
            let util = cols[1].parse::<u8>().unwrap_or(0).clamp(0, 100);
            let used: u32 = cols[2].parse().unwrap_or(0);
            let total: u32 = cols[3].parse().unwrap_or(0);
            let free: u32 = cols[4].parse().unwrap_or(0);
            stats.push(GpuStat {
                name,
                util,
                mem_used: used,
                mem_free: free,
                mem_total: total,
            });
        }
        self.gpu_stats = stats;
    }

    fn open_doc_viewer(&mut self, title: &str, path: &Path) {
        match std::fs::read_to_string(path) {
            Ok(content) => {
                self.viewing_doc = Some((title.to_string(), content));
                self.doc_scroll = 0;
                self.push_system(&format!("Opened '{}' — Esc to close the card. (Content also available in your editor for deep review.)", path.display()));
            }
            Err(e) => {
                self.push_system(&format!("Could not open {}: {}", path.display(), e));
            }
        }
    }

    /// Open the doc viewer with in-memory content (e.g. a compile-time embedded
    /// document) rather than a file on disk. Used by /readme.
    fn open_doc_content(&mut self, title: &str, content: &str) {
        self.viewing_doc = Some((title.to_string(), content.to_string()));
        self.doc_scroll = 0;
        self.push_system(&format!(
            "Opened '{}' — ↑/↓/PgUp/PgDn to scroll, Esc to close.",
            title
        ));
    }

    /// Production-quality message renderer for the main chat log.
    /// - Respects [you], [system], [review R*], [coder], [planner], [reviewer*] (assistant) prefixes with distinct colors.
    /// - Parses ```lang ... ``` fences anywhere in the message (including inside the big REVIEW_*.md dumps
    ///   and context-augmented LLM replies) and renders them as visual "code cards" using box-drawing
    ///   characters + muted style. This gives a Cline-like richer reading experience for code suggestions
    ///   and the structured review findings.
    /// - Crude but effective markdown-ish treatment for # headers and **bold** lines (common in plan/reviews).
    /// - Properly splits on embedded \n (so one big [review R1]\n<full md with its own code> becomes many clean Lines).
    ///
    /// Used by both the main chat Paragraph and the document viewer popups.
    fn render_message_as_lines(m: &str) -> Vec<Line<'static>> {
        let (base_style, body) = if m.starts_with("[system]") {
            let body = m.strip_prefix("[system] ").unwrap_or(m);
            // The greeting renders white; all other system notes use SYSTEM_COLOR.
            let color = if body == "Welcome" {
                Color::White
            } else {
                SYSTEM_COLOR
            };
            (Style::default().fg(color), body)
        } else if m.starts_with("[you]") {
            (
                Style::default().fg(Color::Green),
                m.strip_prefix("[you] ").unwrap_or(m),
            )
        } else if m.starts_with("[demo]") {
            (
                Style::default().fg(Color::Magenta),
                m.strip_prefix("[demo] ").unwrap_or(m),
            )
        } else if m.starts_with("[review") || m.starts_with("[R1") || m.starts_with("[R2") {
            // Prominent treatment for the gate reviews and findings (the heart of the "exactly two" contract).
            // This covers both legacy "[review ...]" and our current "[R1 Plan Findings]", "[R2 Critical...]" etc.
            // The whole block (header + content) gets tinted so review output stands out from coder chat (light blue).
            (Style::default().fg(Color::Cyan).bold(), m)
        } else if m.starts_with("[coder]")
            || m.starts_with("[planner]")
            || m.starts_with("[assistant")
        {
            // Coder (and planner fallback) responses are always blue.
            (Style::default().fg(ROLE_CODER), m)
        } else if m.starts_with("[reviewer-a]") || m.starts_with("[R1]") {
            // R1 / reviewer-a responses are purple.
            (Style::default().fg(ROLE_R1), m)
        } else if m.starts_with("[reviewer-b]") || m.starts_with("[R2]") {
            // R2 / reviewer-b responses use the dedicated bright lime role color.
            (Style::default().fg(ROLE_R2), m)
        } else if m.starts_with("[reviewer") {
            // Generic reviewer prefix fallback.
            (Style::default().fg(ROLE_R1), m)
        } else if m.starts_with("[") && m.contains(" via ") {
            // Legacy LLM responses that still contain the old " via model" form.
            (Style::default().fg(ROLE_CODER), m)
        } else {
            (Style::default(), m)
        };

        let mut out: Vec<Line<'static>> = vec![];
        let mut in_code = false;

        // Split on real newlines so long REVIEW files and multi-line LLM replies render as distinct lines.
        for raw in body.lines() {
            let line = raw.trim_end(); // keep leading spaces for code indentation

            if line.trim_start().starts_with("```") {
                if !in_code {
                    in_code = true;
                    let code_lang = line.trim_start_matches('`').trim().to_string();
                    let header = if code_lang.is_empty() {
                        "┌─── code ".to_string()
                    } else {
                        format!("┌─── {} ", code_lang)
                    };
                    out.push(Line::from(Span::styled(
                        header,
                        Style::default().fg(Color::Blue).bold(),
                    )));
                } else {
                    in_code = false;
                    out.push(Line::from(Span::styled(
                        "└─── end code ",
                        Style::default().fg(Color::Blue),
                    )));
                }
                continue;
            }

            let style = if in_code {
                // Code inside the visual card — muted so it doesn't fight the surrounding text.
                Style::default().fg(Color::Gray)
            } else if line.starts_with('#')
                || line.starts_with("**")
                || line.starts_with("Reviewer:")
                || line.starts_with("Date:")
            {
                // Common structural lines from plan.rs REVIEW headers + md in reviews/findings.
                base_style.add_modifier(Modifier::BOLD)
            } else {
                base_style
            };

            let displayed = if in_code {
                // Visual "card" gutter so code blocks feel contained (Cline-like).
                format!("│ {}", line)
            } else {
                line.to_string()
            };

            // Force checkmarks (✓) to always be green + bold, no matter what base color the
            // message type (system yellow, review cyan, coder light blue, default, etc.) uses.
            // This makes success/acceptance markers pop consistently everywhere in the chat.
            let line_spans: Vec<Span<'static>> = if displayed.contains('✓') {
                let mut spans: Vec<Span<'static>> = vec![];
                let green_check = Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD);
                let mut rest = displayed.as_str();
                while let Some(idx) = rest.find('✓') {
                    if idx > 0 {
                        spans.push(Span::styled(rest[..idx].to_string(), style));
                    }
                    spans.push(Span::styled("✓".to_string(), green_check));
                    rest = &rest[idx + '✓'.len_utf8()..];
                }
                if !rest.is_empty() {
                    spans.push(Span::styled(rest.to_string(), style));
                }
                if spans.is_empty() {
                    spans.push(Span::styled(displayed.clone(), style));
                }
                spans
            } else {
                vec![Span::styled(displayed, style)]
            };
            out.push(Line::from(line_spans));
        }

        // If the original had a review prefix, make the very first line a strong banner for production feel.
        if (m.starts_with("[review") || m.starts_with("[R1") || m.starts_with("[R2"))
            && !out.is_empty()
        {
            // Prepend a clear separator banner (the first real content line will follow).
            let banner = Line::from(Span::styled(
                "════════════════════════════════════════════════════════════",
                Style::default().fg(Color::Cyan),
            ));
            out.insert(0, banner);
        }

        if out.is_empty() {
            // Fallback: color any checkmarks even here.
            let fb_style = base_style;
            let line_spans: Vec<Span<'static>> = if m.contains('✓') {
                let mut spans: Vec<Span<'static>> = vec![];
                let green_check = Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD);
                let mut rest = m;
                while let Some(idx) = rest.find('✓') {
                    if idx > 0 {
                        spans.push(Span::styled(rest[..idx].to_string(), fb_style));
                    }
                    spans.push(Span::styled("✓".to_string(), green_check));
                    rest = &rest[idx + '✓'.len_utf8()..];
                }
                if !rest.is_empty() {
                    spans.push(Span::styled(rest.to_string(), fb_style));
                }
                if spans.is_empty() {
                    spans.push(Span::styled(m.to_string(), fb_style));
                }
                spans
            } else {
                vec![Span::styled(m.to_string(), fb_style)]
            };
            out.push(Line::from(line_spans));
        }
        out
    }

    /// Threshold (chars) above which a clipboard paste is collapsed into a placeholder
    /// instead of being inserted inline. Mirrors the Codex composer's behavior.
    const LARGE_PASTE_CHAR_THRESHOLD: usize = 1000;

    /// Handle a bracketed-paste event. Small pastes are inserted inline; large ones are
    /// collapsed to a "[Pasted Content N chars]" placeholder, with the full text stored in
    /// pending_pastes and expanded back when the message is submitted.
    fn handle_paste(&mut self, text: String) {
        let char_count = text.chars().count();
        // Keep masked fields (API keys) and modest pastes inline.
        if self.input_secret || char_count <= Self::LARGE_PASTE_CHAR_THRESHOLD {
            self.input_insert_str(&text);
            return;
        }
        let placeholder = self.make_paste_placeholder(char_count);
        self.pending_pastes.push((placeholder.clone(), text));
        self.input_insert_str(&placeholder);
        self.push_system(&format!(
            "Collapsed a {char_count}-char paste into {placeholder}. It expands to the full text when you send; backspace over the placeholder to drop it."
        ));
    }

    /// Build a unique placeholder label for a paste of the given size.
    fn make_paste_placeholder(&self, char_count: usize) -> String {
        let base = format!("[Pasted Content {char_count} chars]");
        if !self.pending_pastes.iter().any(|(p, _)| p == &base) {
            return base;
        }
        let mut k = 2;
        loop {
            let cand = format!("[Pasted Content {char_count} chars #{k}]");
            if !self.pending_pastes.iter().any(|(p, _)| p == &cand) {
                return cand;
            }
            k += 1;
        }
    }

    /// Replace any paste placeholders still present in `input` with their full stored text.
    /// Placeholders the user deleted simply aren't found and are dropped on the next clear.
    fn expand_pasted_input(&self, mut input: String) -> String {
        for (placeholder, full) in &self.pending_pastes {
            if input.contains(placeholder.as_str()) {
                input = input.replace(placeholder.as_str(), full);
            }
        }
        input
    }

    fn handle_input(&mut self) {
        let taken = std::mem::take(&mut self.input);
        let input = self.expand_pasted_input(taken);
        self.pending_pastes.clear();
        self.input_cursor = 0;
        self.showing_command_palette = false;

        // Wizard is active — treat Enter as "submit answer to current step".
        // Collapse any newlines (from Shift+Enter or paste) so config values like names/URLs/models stay clean single-line.
        if self.config_wizard.is_some() {
            let answer = input.trim().replace(['\n', '\r'], " ").to_string();
            self.input_secret = false; // always leave secret mode after submit
            self.advance_config_wizard(answer);
            return;
        }

        let trimmed = input.trim();
        if trimmed.is_empty() {
            // An empty Enter resumes a paused review gate.
            if self.gate_paused() {
                self.continue_gate();
            }
            return;
        }

        // While a review gate is actively running a step, hold back plain chat so
        // it doesn't collide with the gate's own coder turns. Slash commands (and
        // the y/n confirm) still work.
        if self.gate_busy() && !trimmed.starts_with('/') {
            self.push_system(
                "A review gate is running — please wait (it will pause for you to review).",
            );
            return;
        }

        // Doc viewer card (plan / reviews) is modal for focused reading. Plain Enter closes it (you ack the content).
        // Commands like /view-* or /accept-plan will overwrite or work with it visible.
        if self.viewing_doc.is_some() && !trimmed.starts_with('/') {
            self.viewing_doc = None;
        }

        // Echo user input. For multi-line (Shift+Enter or paste) we split across log entries
        // with a small indent so the chat history shows the block nicely.
        if trimmed.contains('\n') {
            let mut lines = trimmed.lines();
            if let Some(first) = lines.next() {
                self.push(format!("[you] {}", first));
                for line in lines {
                    self.push(format!("      {}", line));
                }
            }
        } else {
            self.push(format!("[you] {}", trimmed));
        }
        self.follow_bottom = true;
        let cmd = trimmed.to_lowercase();

        // A run_command confirmation is pending — the agent task is blocked
        // waiting for the user's y/n. Resolve it (or nudge for an answer) and
        // do not start a new chat turn until it's settled.
        if self.awaiting_confirm.is_some() {
            if cmd == "/y" || cmd == "/yes" {
                self.resolve_confirm(0);
            } else if cmd == "/a" || cmd == "/always" {
                self.resolve_confirm(1);
            } else if cmd == "/n" || cmd == "/no" {
                self.resolve_confirm(2);
            } else {
                self.push_system(
                    "A command is awaiting your decision — ↑/↓ then Enter, or /y (yes) / /a (yes+remember) / /n (no).",
                );
            }
            return;
        }

        // Built-in slash commands for the skeleton (real gates added in later phases)
        if cmd == "/quit" || cmd == "/q" || cmd == ":q" {
            self.should_quit = true;
            return;
        }

        if cmd == "/status" {
            let configured = !self.first_run;
            self.push_system(&format!(
                "Working in project root: {}\n  configured={}  agent_ready={}  messages_in_this_session={}",
                self.root.display(),
                configured,
                self.agent.is_some(),
                self.messages.len()
            ));
            self.push_system("  (the coder reads/edits files and runs commands directly via its tools — no manual context needed)");

            // Roles + config provenance ("why is this repo using that model?").
            // Build the lines first (borrows self.cfg), then push (needs &mut self).
            let role_lines: Vec<String> = if let Some(cfg) = &self.cfg {
                let (global, project) = crate::config::config_layers(&self.root);
                [
                    ("CODER", "coder"),
                    ("R1 (reviewer-a)", "reviewer_a"),
                    ("R2 (reviewer-b)", "reviewer_b"),
                ]
                .iter()
                .map(|(label, role)| match cfg.resolve_role_or_binding(role) {
                    Ok((_n, binding, provider)) => {
                        let in_project = project
                            .as_ref()
                            .map(|c| role_is_set(c, role))
                            .unwrap_or(false);
                        let in_global = global
                            .as_ref()
                            .map(|c| role_is_set(c, role))
                            .unwrap_or(false);
                        let src = if in_project {
                            "project anvil.toml (overrides global)"
                        } else if in_global {
                            "global config"
                        } else {
                            "default"
                        };
                        format!(
                            "  {}: {} (via {}) — from {}",
                            label, binding.model, provider.r#type, src
                        )
                    }
                    Err(_) => format!("  {}: (not configured)", label),
                })
                .collect()
            } else {
                Vec::new()
            };
            if !role_lines.is_empty() {
                self.push_system("Roles (active model — source):");
                for l in &role_lines {
                    self.push_system(l);
                }
                if let Some(g) = crate::config::global_config_path() {
                    self.push_system(&format!("  global config: {}", g.display()));
                }
                self.push_system(&format!(
                    "  project config: {}",
                    crate::config::config_path(&self.root).display()
                ));
                self.push_system("  change a role with /swap · full setup with /config");
            }

            // Snapshot to avoid long-lived & borrow of self while calling push_system (mut).
            let gpu_snap: Vec<(usize, f32, f32, u8)> = self
                .gpu_stats
                .iter()
                .enumerate()
                .map(|(i, g)| {
                    (
                        i,
                        g.mem_used as f32 / 1024.0,
                        g.mem_total as f32 / 1024.0,
                        g.util,
                    )
                })
                .collect();

            // Quick VRAM/GPU snapshot in /status for convenience when debugging "full" cards.
            if !gpu_snap.is_empty() {
                self.push_system("GPUs (nvidia-smi):");
                for (i, used_g, tot_g, util) in &gpu_snap {
                    self.push_system(&format!(
                        "  {}: {:.1}/{:.1}G used @ {}% util",
                        i, used_g, tot_g, util
                    ));
                }
            }

            // Also surface how many models Ollama currently claims are loaded.
            // Snapshot the summary first (block_on borrow of runtime).
            let ollama_info: Option<(usize, f64)> = if let Some(rt) = &self.runtime {
                match rt.block_on(self.llm.list_ollama_ps()) {
                    Ok(models) if !models.is_empty() => {
                        let total_vram: f64 = models
                            .iter()
                            .map(|m| (m.size_vram.max(m.size)) as f64 / 1e9)
                            .sum();
                        Some((models.len(), total_vram))
                    }
                    _ => None,
                }
            } else {
                None
            };
            if let Some((cnt, vram)) = ollama_info {
                self.push_system(&format!(
                    "Ollama /ps: {} model(s) resident, ~{:.1} GB claimed VRAM",
                    cnt, vram
                ));
            }
            return;
        }

        if cmd == "/plan" {
            if !self.is_configured() {
                self.push_system("Cannot start planning: no models configured. Use the wizard (s or /config) first.");
                return;
            }
            self.push_system("=== PLANNING ===");
            self.push_system("Just talk with the coder about what you want to build. When the shape is clear, ask it to write the plan to plan.md (it writes the file itself — phases with id/name/goal/actions/criteria).");
            self.push_system("When you're happy with plan.md, run /lock-plan. It runs R1, the coder applies fixes, you review and /continue, then R2, the coder applies fixes, you /continue, and the coder summarizes — then /accept-plan to approve and start building.");
            return;
        }

        if cmd == "/lock-plan" {
            if self.gate_flow.is_some() {
                self.push_system(
                    "A review gate is already running. Let it finish (or /continue when paused).",
                );
                return;
            }
            if !self.is_configured() {
                self.push_system("Reviewers not configured. Use /config.");
                return;
            }
            // Pick up a coder-named <feature>_plan.md right here, in case the coder just
            // wrote it this turn and no stage reconcile has run yet (otherwise we'd look
            // for the default plan.md and miss it).
            self.adopt_coder_named_plan_if_needed();
            let plan_path = self.plan_path();
            if !plan_path.exists() {
                self.push_system(&format!(
                    "No plan file found ({}). Ask the coder to write the plan first — any name works (e.g. trusteazy_plan.md, or plain plan.md) and it creates the file itself — then /lock-plan.",
                    active_plan_name(&self.root)
                ));
                return;
            }
            self.start_gate_flow(GateArtifact::Plan);
            return;
        }

        if cmd == "/accept-plan" || cmd == "/accept plan" {
            self.adopt_coder_named_plan_if_needed();
            let plan_path = self.plan_path();
            let rev_dir = reviews_dir(&self.root);
            let r1 = rev_dir.join("REVIEW_plan_R1.md");
            let r2 = rev_dir.join("REVIEW_plan_R2.md");

            if !plan_path.exists() || !r1.exists() || !r2.exists() {
                self.push_system("plan.md + both REVIEW_plan_R1.md and R2.md (at root) must exist. Run /lock-plan first.");
                return;
            }

            if let Ok(plan_txt) = std::fs::read_to_string(&plan_path) {
                let hash = crate::plan::simple_hash(&plan_txt);
                let mut st = load_state(&self.root);
                st.accepted_plan_hash = Some(hash);
                // Mark the boundary before P0 so the first phase's review diffs
                // from here regardless of when work was built.
                st.phase_base = crate::phase::git_head_sha(&self.root);
                if let Err(e) = save_state(&self.root, &st) {
                    self.push_system(&format!("Warning: could not persist accept hash: {}", e));
                }
            }

            self.stage = WorkflowStage::PlanAccepted;
            self.reconcile_stage_from_disk();
            self.update_status();

            self.push_system("✓ Quenched — plan locked in (R1 + R2 reviewed, hash recorded). Now build: tell the coder to start the first phase, or /phase-start P0. When a phase is done, /accept-phase.");
            return;
        }

        if cmd.starts_with("/phase-start ") || cmd.starts_with("/start ") {
            let raw = cmd.split_once(' ').map(|(_, r)| r.trim()).unwrap_or("");
            if raw.is_empty() {
                self.push_system("Usage: /phase-start P0   (or /start P0)");
                return;
            }
            let id = crate::phase::normalize_phase_id(raw);
            match crate::phase::run_phase_start(&self.root, &id) {
                Ok(excerpt) => {
                    self.push_system(&format!("Current phase set to {}. Build it with the coder (it reads/edits files and runs tests directly) — or just tell it to start. When done, run /accept-phase.", id));
                    if let Some(slice) = excerpt {
                        self.push_system(&format!("Plan excerpt for {}:\n{}", id, slice));
                    }
                }
                Err(e) => self.push_system(&format!("phase start failed: {}", e)),
            }
            self.reconcile_stage_from_disk();
            self.update_status();
            return;
        }

        if cmd.starts_with("/accept-phase") {
            if self.gate_flow.is_some() {
                self.push_system(
                    "A review gate is already running. Let it finish (or /continue when paused).",
                );
                return;
            }
            if !self.is_configured() {
                self.push_system("Reviewers not configured. Use /config.");
                return;
            }
            let id = if cmd.contains(' ') {
                cmd.split_once(' ')
                    .map(|(_, r)| r.trim().to_string())
                    .unwrap_or_default()
            } else {
                load_state(&self.root).current_phase.unwrap_or_default()
            };
            if id.is_empty() {
                self.push_system(
                    "Usage: /accept-phase P0   (or just /accept-phase while a phase is current)",
                );
                return;
            }
            self.start_gate_flow(GateArtifact::Phase(id));
            return;
        }

        if cmd.starts_with("/ship-phase") {
            let id = if cmd.contains(' ') {
                cmd.split_once(' ')
                    .map(|(_, r)| r.trim().to_string())
                    .unwrap_or_default()
            } else {
                load_state(&self.root).current_phase.unwrap_or_default()
            };
            if id.is_empty() {
                self.push_system(
                    "Usage: /ship-phase P0   (or just /ship-phase while a phase is current)",
                );
                return;
            }
            match crate::phase::run_phase_accept(&self.root, &id) {
                Ok(None) => self.push_system(&format!("✓ Quenched — phase {} shipped (R1 + R2 reviewed and accepted). Start the next phase with /phase-start, or just tell the coder to continue.", id)),
                Ok(Some(c)) => self.push_system(&format!(
                    "✓ Quenched — phase {} shipped, and it was the LAST phase. Plan closed: {} → {}. \
                     All phases cleared. Discuss the next piece of work and have the coder write a new <feature>_plan.md, then /lock-plan.",
                    id, c.old_name, c.new_name
                )),
                Err(e) => self.push_system(&format!("ship phase: {} (run /accept-phase {} first to produce the reviews)", e, id)),
            }
            self.reconcile_stage_from_disk();
            self.update_status();
            return;
        }

        // `/review [--deep] [label]` — ad-hoc review of recent work that doesn't
        // warrant a whole plan. Coder writes a briefing → R1 critiques the diff;
        // `--deep` adds the opt-in R2 second opinion. No ship/accept step.
        if cmd == "/review" || cmd.starts_with("/review ") {
            if self.gate_flow.is_some() {
                self.push_system(
                    "A review gate is already running. Let it finish (or /continue when paused).",
                );
                return;
            }
            if !self.is_configured() {
                self.push_system("Reviewers not configured. Use /config.");
                return;
            }
            let rest = cmd.strip_prefix("/review").unwrap_or("").trim();
            let mut deep = false;
            let mut label_parts: Vec<&str> = Vec::new();
            for tok in rest.split_whitespace() {
                match tok {
                    "--deep" | "-d" => deep = true,
                    // Ignore usage-hint placeholders if they leak in (e.g. a pasted
                    // "/review [--deep] [label]") instead of treating them as a label.
                    t if t.starts_with('[') || t.starts_with('<') => {}
                    t => label_parts.push(t),
                }
            }
            // Pre-flight: don't spend a reviewer turn on an empty diff. `/review` is
            // git-based, so a non-git folder (or a clean tree) has nothing to review —
            // say so clearly instead of running a misleading "working tree is clean".
            match crate::phase::addition_review_readiness(&self.root) {
                crate::phase::AdditionReviewReadiness::NoGit => {
                    self.push_system(
                        "/review reviews a git diff, but this folder isn't a git repository (or `git` isn't installed / on PATH), so there's nothing to diff. \
                         Initialize it — `git init`, then commit a baseline — and from then on Anvil can review your changes. (The phase/plan gates are git-based too.)",
                    );
                    return;
                }
                crate::phase::AdditionReviewReadiness::NothingToReview => {
                    self.push_system(
                        "Nothing to review — the working tree is clean and there's no recent commit. \
                         Make the change you want reviewed (or commit it), then run /review.",
                    );
                    return;
                }
                crate::phase::AdditionReviewReadiness::Ready => {}
            }
            let label = label_parts.join(" ");
            let slug = crate::phase::addition_slug(&self.root, &label);
            self.start_gate_flow(GateArtifact::Addition { slug, deep });
            return;
        }

        // `/debug <description>` (alias `/fix`) — bug-hunt mode. No plan/phase
        // needed: frame the coder with debugging discipline (reproduce → root
        // cause → minimal fix → regression test → verify) and let it go. The fix
        // is left UNCOMMITTED on purpose so the existing /review — which gates the
        // working-tree diff — sees exactly it. Nothing new downstream.
        if cmd == "/debug"
            || cmd.starts_with("/debug ")
            || cmd == "/fix"
            || cmd.starts_with("/fix ")
        {
            if self.gate_flow.is_some() {
                self.push_system(
                    "A review gate is running. Let it finish (or /continue when paused) before starting a debug task.",
                );
                return;
            }
            if !self.is_configured() {
                self.push_system("No coder configured. Use /config.");
                return;
            }
            // `cmd` is lowercased; take the description from `trimmed` so the bug
            // report keeps its original case. Everything after the command word.
            let desc = trimmed
                .split_once(char::is_whitespace)
                .map(|(_, rest)| rest)
                .unwrap_or("")
                .trim();
            if desc.is_empty() {
                self.push_system(
                    "Usage: /debug <describe the bug> — e.g. `/debug clicking Save twice creates two records`. \
                     The coder reproduces it, finds the root cause, fixes it minimally + adds a regression test, then you run /review.",
                );
                return;
            }
            // /review diffs the WHOLE working tree, so any changes already sitting
            // uncommitted will be folded into the debug review alongside the fix.
            // Surface that up front so the user can commit/stash unrelated work and
            // keep the review focused (a dirty tree is what made an early /review
            // critique files that had nothing to do with the debug task).
            let pending = crate::phase::pending_change_count(&self.root);
            if pending > 0 {
                self.push_system(&format!(
                    "Heads up: {pending} file(s) already have uncommitted changes. /review diffs the whole working tree, so it'll include them next to the fix. \
                     Commit or stash unrelated work first if you want the debug review focused on just this fix.",
                ));
            }
            let prompt = format!(
                "DEBUG TASK — the user reports a bug. Hunt down the ROOT CAUSE, then fix it.\n\
                 \n\
                 Reported issue:\n{desc}\n\
                 \n\
                 Work as a disciplined debugger:\n\
                 1. REPRODUCE / LOCATE first. Read the code paths involved and, if there is a failing test or command, run it to see the real error before changing anything. Confirm the symptom — don't guess.\n\
                 2. Find the ROOT CAUSE, not the surface symptom. Trace the failure to the line that is actually wrong, and state the cause in one sentence before you fix it.\n\
                 3. Make the MINIMAL fix that addresses that cause. Don't refactor unrelated code or expand scope.\n\
                 4. Add or update a test that fails before the fix and passes after (a regression guard) — unless the project has no test harness.\n\
                 5. VERIFY: run the project's build/test/lint (see .anvil/decisions.md) and confirm the symptom is gone and nothing else broke.\n\
                 6. Do NOT `git commit` the fix — leave it in the working tree. /review gates UNCOMMITTED changes, so leaving your fix unstaged is exactly what lets the reviewers see just it; the user commits after the review passes.\n\
                 \n\
                 When the fix is in place, verified, and left uncommitted, STOP and tell the user to run /review (add --deep for a second cross-vendor opinion) to gate the change."
            );
            self.start_real_chat(&prompt);
            return;
        }

        if cmd == "/continue" {
            if self.gate_flow.is_some() {
                self.continue_gate();
            } else {
                self.push_system("Nothing to continue — no review gate is paused.");
            }
            return;
        }

        if cmd == "/refresh" || cmd == "/reground" {
            // Show the live reality snapshot the coder is grounded on every turn.
            let snap = crate::reality::snapshot(&self.root);
            self.push_system("Reality snapshot (the coder receives this each turn):");
            self.push(snap);
            self.follow_bottom = true;
            return;
        }

        if cmd == "/update" {
            if self.update_in_progress {
                self.push_system("An update is already in progress…");
            } else if self.update_available.is_some() {
                self.spawn_update_apply();
            } else {
                self.push_system(&format!(
                    "anvil is up to date (v{}), or no newer release has been detected yet.",
                    crate::update::current_version()
                ));
            }
            self.follow_bottom = true;
            return;
        }

        if cmd == "/models" {
            self.show_models();
            self.follow_bottom = true;
            return;
        }

        if cmd == "/decisions" {
            self.show_context_file(
                crate::agent::decisions_path(&self.root),
                "Decisions (.anvil/decisions.md — injected each turn)",
            );
            return;
        }
        if cmd == "/assumptions" {
            self.show_context_file(
                crate::agent::assumptions_path(&self.root),
                "Assumptions (.anvil/assumptions.md — unverified hypotheses, injected each turn)",
            );
            return;
        }
        if cmd == "/scratch" {
            self.show_context_file(
                crate::agent::scratch_path(&self.root),
                "Scratchpad (.anvil/scratch.md — disposable, never injected)",
            );
            return;
        }
        if cmd == "/architecture" || cmd == "/arch" {
            self.show_context_file(
                crate::agent::architecture_path(&self.root),
                "Architecture map (ARCHITECTURE.md — read on demand)",
            );
            return;
        }

        if cmd == "/compact" || cmd == "/clinker" || cmd == "/summarize" {
            if self.gate_flow.is_some() {
                self.push_system(
                    "Can't clinker during a review gate — finish or let it abort first.",
                );
                return;
            }
            let agent = match &self.agent {
                Some(a) => a.clone(),
                None => {
                    self.push_system("Nothing to clinker yet — chat with the coder first.");
                    return;
                }
            };
            self.push_system("Clinkering the forge — folding the conversation into working memory (.anvil/working-memory.md), then raking out the older turns…");
            let ts = Utc::now().format("%Y-%m-%d %H:%M UTC").to_string();
            let (tx, rx) = mpsc::unbounded_channel::<String>();
            self.gate_rx = Some(rx);
            if let Some(rt) = &self.runtime {
                rt.spawn(async move {
                    let mut guard = agent.lock().await;
                    match guard.compact(&ts).await {
                        Ok(summary) => { let _ = tx.send(format!("[findings]✓ Clinkered into .anvil/working-memory.md (injected each turn now); older turns raked out:\n\n{}", summary)); }
                        Err(e) => { let _ = tx.send(format!("[gate-error]clinker: {}", e)); }
                    }
                });
            }
            return;
        }

        if cmd == "/context" || cmd == "/ctx" {
            // Focused readout of how full the coder's context window is and whether
            // auto-compaction ("clinkering") is about to fire — so the memory
            // behavior isn't a black box.
            match self.agent.as_ref().and_then(|a| {
                a.try_lock().ok().map(|g| {
                    (
                        g.history_len(),
                        g.context_chars(),
                        g.context_budget(),
                        g.compaction_pending(),
                    )
                })
            }) {
                Some((hist, used_chars, budget_chars, pending)) => {
                    // Grounding injected on top of the history window each turn.
                    let wm = std::fs::read_to_string(crate::agent::working_memory_path(&self.root))
                        .unwrap_or_default();
                    let snap = crate::reality::snapshot(&self.root).len();
                    let dec = std::fs::read_to_string(crate::agent::decisions_path(&self.root))
                        .map(|s| s.len().min(2000))
                        .unwrap_or(0);
                    let asm = std::fs::read_to_string(crate::agent::assumptions_path(&self.root))
                        .map(|s| s.len().min(2000))
                        .unwrap_or(0);
                    let grounding_tok = (wm.trim().len() + snap + dec + asm) / 4;
                    let used_tok = used_chars / 4;
                    let budget_tok = budget_chars.max(1) / 4;
                    let pct =
                        (used_chars as f64 / budget_chars.max(1) as f64 * 100.0).round() as u32;
                    self.push_system("Context window (coder):");
                    self.push_system(&format!(
                        "  history window: ~{} / ~{} tokens ({}% full) · {} messages in memory",
                        used_tok, budget_tok, pct, hist
                    ));
                    self.push_system(&format!(
                        "  + grounding injected each turn (working memory + reality + decisions/assumptions): ~{} tokens",
                        grounding_tok
                    ));
                    self.push_system(&format!(
                        "  working memory: {}",
                        if wm.trim().is_empty() {
                            "empty"
                        } else {
                            "present (re-injected each turn)"
                        }
                    ));
                    self.push_system(&format!(
                        "  compaction: {}",
                        if pending {
                            "IMMINENT — older turns fold into working memory at the end of the next turn"
                        } else {
                            "not imminent"
                        }
                    ));
                    self.push_system("  (budget is sized from the coder model's context window; see /models. /compact forces it now.)");
                }
                None => self.push_system(
                    "No coder agent yet — start chatting first, then /context shows window usage.",
                ),
            }
            return;
        }

        if cmd == "/memory" || cmd == "/mem" {
            // What the coder carries: in-RAM history, working memory, the ledger,
            // and a rough estimate of tokens sent next turn.
            let ledger_lines = std::fs::read_to_string(crate::agent::session_path(&self.root))
                .map(|s| s.lines().filter(|l| !l.trim().is_empty()).count())
                .unwrap_or(0);
            let wm = std::fs::read_to_string(crate::agent::working_memory_path(&self.root))
                .unwrap_or_default();
            let wm_bytes = wm.trim().len();
            let snap_chars = crate::reality::snapshot(&self.root).len();
            let (hist_len, ctx_chars) = self
                .agent
                .as_ref()
                .and_then(|a| {
                    a.try_lock()
                        .ok()
                        .map(|g| (g.history_len(), g.context_chars()))
                })
                .unwrap_or((0, 0));
            let dec_bytes = std::fs::read_to_string(crate::agent::decisions_path(&self.root))
                .map(|s| s.len().min(2000))
                .unwrap_or(0);
            let asm_bytes = std::fs::read_to_string(crate::agent::assumptions_path(&self.root))
                .map(|s| s.len().min(2000))
                .unwrap_or(0);
            let est_tokens = (ctx_chars + wm_bytes + snap_chars + dec_bytes + asm_bytes) / 4;
            // Project context files: size + whether they're injected this turn.
            let file_line = |path: std::path::PathBuf, label: &str, injected: bool| -> String {
                let content = std::fs::read_to_string(&path).unwrap_or_default();
                let body = content.lines().any(|l| {
                    let t = l.trim();
                    !t.is_empty()
                        && !t.starts_with('#')
                        && !t.starts_with("<!--")
                        && !t.starts_with('>')
                });
                let status = if !body {
                    "empty".to_string()
                } else if injected {
                    format!("{} bytes, injected", content.trim().len())
                } else {
                    format!("{} bytes, on demand", content.trim().len())
                };
                format!("  {}: {}", label, status)
            };
            let ctx_files = format!(
                "{}\n{}\n{}\n{}",
                file_line(
                    crate::agent::decisions_path(&self.root),
                    "decisions.md",
                    true
                ),
                file_line(
                    crate::agent::assumptions_path(&self.root),
                    "assumptions.md",
                    true
                ),
                file_line(
                    crate::agent::scratch_path(&self.root),
                    "scratch.md (never injected)",
                    false
                ),
                file_line(
                    crate::agent::architecture_path(&self.root),
                    "ARCHITECTURE.md",
                    false
                ),
            );
            self.push_system(&format!(
                "Memory layers:\n  ledger (.anvil/session.json): {} entries (append-only, full record)\n  in-session history: {} messages (recent window sent to the coder)\n  working memory (.anvil/working-memory.md): {} bytes\n  reality snapshot: {} bytes (rebuilt every turn)\nProject context files:\n{}\n  ≈ {}k tokens sent next turn (window + working memory + decisions + assumptions + snapshot)",
                ledger_lines, hist_len, wm_bytes, snap_chars, ctx_files, est_tokens / 1000
            ));
            self.push_system("  /decisions /assumptions /scratch /architecture to view · /compact folds chat into working memory · /clear-memory resets the session (ledger kept)");
            return;
        }

        if cmd == "/clear-memory" || cmd == "/clear-mem" {
            crate::agent::append_reset_marker(&self.root);
            let _ = std::fs::write(crate::agent::working_memory_path(&self.root), "");
            let cleared = self
                .agent
                .as_ref()
                .and_then(|a| {
                    a.try_lock().ok().map(|mut g| {
                        g.clear_history();
                        true
                    })
                })
                .unwrap_or(false);
            if cleared || self.agent.is_none() {
                self.push_system("Memory cleared: in-session history reset and working memory emptied. The ledger keeps the full record (a reset marker was written); plan.md / REVIEW_* are untouched.");
            } else {
                self.push_system("Working memory emptied + ledger reset marker written, but the coder is mid-turn — its in-session history clears on the next idle moment.");
            }
            return;
        }

        if cmd == "/config" || cmd == "/setup" {
            self.start_config_wizard();
            return;
        }

        if cmd == "/swap" || cmd == "/swap-model" {
            self.start_role_swap();
            return;
        }

        if cmd == "/approvals" || cmd == "/commands" || cmd == "/approve-list" {
            self.open_approvals_editor();
            return;
        }

        if cmd == "/tag" || cmd == "/tag show" || cmd.starts_with("/tag ") {
            self.handle_tag(cmd.strip_prefix("/tag").unwrap_or("").trim());
            return;
        }

        if cmd == "/new-plan" || cmd.starts_with("/new-plan ") {
            let name = cmd
                .strip_prefix("/new-plan")
                .unwrap_or("")
                .trim()
                .to_string();
            self.start_new_plan(&name);
            return;
        }
        if cmd == "/plans" {
            self.list_plans();
            return;
        }

        if cmd == "/view-plan" {
            let plan_path = self.plan_path();
            self.open_doc_viewer("Plan (read before accept)", &plan_path);
            return;
        }
        if cmd == "/readme" {
            // The README is embedded at compile time — Anvil ships as a single binary,
            // so it is not guaranteed to exist on disk wherever the user runs anvil.
            self.open_doc_content("Anvil README", include_str!("../README.md"));
            return;
        }
        if cmd == "/view-reviews" {
            let rev_dir = reviews_dir(&self.root);
            let r1 = rev_dir.join("REVIEW_plan_R1.md");
            let r2 = rev_dir.join("REVIEW_plan_R2.md");
            // Build a combined document for the popup card.
            let mut combined = String::new();
            combined.push_str(
                "=== PLAN REVIEW R1 (reviewer-a critical on the plan written by coder) ===\n\n",
            );
            if let Ok(c) = std::fs::read_to_string(&r1) {
                combined.push_str(&c);
            } else {
                combined.push_str(
                    "(REVIEW_plan_R1.md not found — the coder writes plan.md, then run /lock-plan)\n",
                );
            }
            combined.push_str("\n\n=== PLAN REVIEW R2 (reviewer-b) ===\n\n");
            if let Ok(c) = std::fs::read_to_string(&r2) {
                combined.push_str(&c);
            } else {
                combined.push_str("(R2 not found)\n");
            }
            // Also show the current phase's review docs if a phase is active.
            let st = load_state(&self.root);
            if let Some(pid) = &st.current_phase {
                combined.push_str(&format!(
                    "\n\n--- Current phase {} reviews (from /accept-phase) ---\n",
                    pid
                ));
                for nm in [
                    format!("REVIEW_{}_R1.md", pid),
                    format!("REVIEW_{}_R2.md", pid),
                ] {
                    let p = rev_dir.join(&nm);
                    if p.exists() {
                        if let Ok(c) = std::fs::read_to_string(&p) {
                            combined.push_str(&format!("\n=== {} ===\n{}\n", nm, c));
                        }
                    }
                }
            }
            combined.push_str("\n\n--- Source of truth: these REVIEW_* files at repo root + plan.md + state.json. ---\n");
            self.viewing_doc = Some((
                "Reviews (plan + current phase) — Esc to close".to_string(),
                combined,
            ));
            self.push_system("Opened focused review card. Close with Esc.");
            return;
        }

        if cmd == "/help" || cmd == "?" {
            self.push_system("Keys: Enter=chat (streams), Ctrl-B=break in / interrupt the coder, Ctrl-X or /q=quit (Esc no longer quits; Ctrl-C is free for copy), Ctrl-S=quick-setup, ↑/↓ scroll chat (or command list), / for palette (filter + arrows + Enter to pick), Backspace");
            self.push_system("Editing: ←/→ move cursor (Ctrl+←/→ by word), ↑/↓ move between input lines (or scroll chat at the edges), Home/End start/end of line, Del forward-delete, Shift+Enter newline.");
            self.push_system("The coder is a real agent: it reads, writes and edits files and runs commands itself (you confirm each command with /y or /n). No manual /include needed.");
            self.push_system("Grounding: the coder sees a live reality snapshot (stage, phase, plan slice, git) every turn, and can call its project_state tool. /refresh shows it to you.");
            self.push_system("Memory: chat persists across restarts (append-only ledger). /compact summarizes into .anvil/working-memory.md (injected each turn). /memory inspects the layers; /clear-memory resets the session (ledger kept).");
            self.push_system("Context files (coder-maintained, visible): /decisions (prefs + verify commands), /assumptions (unverified hypotheses) — both injected each turn · /scratch (disposable, never injected) · /architecture (code map, on demand).");
            self.push_system("Plan gate: coder writes plan.md → /lock-plan → R1 → coder fixes → (pause) /continue → R2 → coder fixes → (pause) /continue → summary → /accept-plan.");
            self.push_system("Phase gate: build with the coder → /accept-phase → same R1 → fix → R2 → fix → summary loop on the git diff → /ship-phase. Each pause: /continue or Enter on an empty line.");
            self.push_system("No plan? /debug <bug> sends the coder root-cause hunting (reproduce → minimal fix → regression test, left UNCOMMITTED); /review [--deep] then gates that working-tree diff (R1, +R2 with --deep). Commit/stash unrelated changes first so the review stays focused. No /ship — you commit after it passes.");
            self.push_system("Ollama VRAM: /ps (or /loaded) shows models currently in VRAM • /unload [model] frees VRAM (all if no model given)");
            return;
        }

        if cmd == "/ps" || cmd == "/ollama" || cmd == "/loaded" || cmd == "/ollama-ps" {
            self.show_ollama_loaded();
            return;
        }

        if cmd.starts_with("/unload") {
            let arg = cmd.strip_prefix("/unload").unwrap_or("").trim().to_string();
            self.unload_ollama_models(&arg);
            return;
        }

        // Real LLM chat (planner or coder role) with live streaming via mpsc.
        if self.is_configured() {
            self.start_real_chat(trimmed);
        } else {
            self.push_system("Not configured yet — the wizard should have opened automatically (or Ctrl+S for instant local Ollama, or /config).");
            if trimmed.len() > 3 {
                self.push("[system] (demo) Understood. After setup your messages will stream from the real model.".to_string());
            }
        }
    }

    /// Returns whether Ollama appears to be running and reachable on the default port.
    /// Result is cached for the lifetime of the App so menu building doesn't spam probes.
    fn is_ollama_available(&mut self) -> bool {
        if let Some(cached) = self.ollama_available_cached {
            return cached;
        }
        let ok = if let Some(rt) = &self.runtime {
            rt.block_on(self.llm.probe_ollama())
        } else {
            false
        };
        self.ollama_available_cached = Some(ok);
        ok
    }

    /// Seed (or update) just the "local-ollama" provider connection with the standard
    /// localhost openai_compat URL and no credential. Called as the first half of quick setup.
    fn ensure_local_ollama_provider(&mut self) -> Result<()> {
        ensure_anvil_dir(&self.root)?;
        let mut cfg = load_config(&self.root).unwrap_or_default();
        cfg.providers.insert(
            "local-ollama".to_string(),
            ProviderConnection {
                r#type: "openai_compat".to_string(),
                base_url: Some("http://localhost:11434/v1".to_string()),
                credential: CredentialRef::None,
                extra: Default::default(),
                keep_alive: Some("30s".to_string()),
            },
        );
        save_global_config(&cfg)?;
        self.cfg = load_config(&self.root).ok();
        Ok(())
    }

    /// Show currently loaded Ollama models (with VRAM sizes if reported) + current nvidia-smi
    /// per-GPU used/total snapshot for cross-check.
    /// This is the best way to verify whether the header "full" numbers are accurate for your
    /// 8000-series (or other) cards: Ollama reports what it has resident; nvidia-smi reports the
    /// broader driver/CUDA allocation (always >= Ollama's number, often by several GB overhead).
    fn show_ollama_loaded(&mut self) {
        // Snapshot GPU data up front so we can print while the Ollama runtime borrow is live.
        let gpu_snap: Vec<(usize, f32, f32, f32)> = self
            .gpu_stats
            .iter()
            .enumerate()
            .map(|(i, g)| {
                (
                    i,
                    g.mem_used as f32 / 1024.0,
                    g.mem_total as f32 / 1024.0,
                    g.mem_free as f32 / 1024.0,
                )
            })
            .collect();

        if let Some(rt) = &self.runtime {
            match rt.block_on(self.llm.list_ollama_ps()) {
                Ok(models) if !models.is_empty() => {
                    self.push_system("Ollama loaded models (from /api/ps):");
                    for m in &models {
                        let v = if m.size_vram > 0 {
                            format!("{:.1} GB VRAM", m.size_vram as f64 / 1_000_000_000.0)
                        } else if m.size > 0 {
                            format!("{:.1} GB", m.size as f64 / 1_000_000_000.0)
                        } else {
                            "size unknown".to_string()
                        };
                        self.push_system(&format!("  • {} — {}", m.name, v));
                    }
                    // Side-by-side nvidia-smi numbers so user can judge if header "full" VRAM is accurate.
                    // (Common: Ollama size_vram sum < nvidia-smi used because of CUDA contexts, KV cache during
                    // inference, and driver reservations. After /unload the gap should shrink.)
                    if !gpu_snap.is_empty() {
                        self.push_system("nvidia-smi VRAM (driver view, for comparison):");
                        for (i, used_g, tot_g, free_g) in &gpu_snap {
                            self.push_system(&format!(
                                "  GPU {}: {:.1}/{:.1}G used (free {:.1}G)",
                                i, used_g, tot_g, free_g
                            ));
                        }
                    }
                    self.push_system("Tip: /unload or /unload <exact-model-tag> to free VRAM immediately. New calls default to 30s keep-alive for local Ollama.");
                }
                Ok(_) => {
                    self.push_system("No models currently loaded in Ollama (api/ps empty).");
                    if !gpu_snap.is_empty() {
                        self.push_system("nvidia-smi VRAM (current):");
                        for (i, used_g, tot_g, free_g) in &gpu_snap {
                            self.push_system(&format!(
                                "  GPU {}: {:.1}/{:.1}G used (free {:.1}G)",
                                i, used_g, tot_g, free_g
                            ));
                        }
                    }
                }
                Err(e) => {
                    self.push_system(&format!("Could not reach Ollama /api/ps: {}", e));
                }
            }
        } else {
            self.push_system("No runtime available for Ollama query.");
        }
    }

    /// Unload one or all models from Ollama to free VRAM.
    /// With no arg: unloads everything currently reported by /api/ps.
    /// With arg: unloads the exact model name you pass (use the full tag from /ps).
    fn unload_ollama_models(&mut self, specific: &str) {
        if let Some(rt) = &self.runtime {
            if !specific.trim().is_empty() {
                let _ = rt.block_on(self.llm.ollama_unload(specific));
                self.push_system(&format!(
                    "Requested unload for '{}'. (Ollama will drop it from VRAM.)",
                    specific
                ));
                // Give nvidia-smi a moment on next refresh
                self.refresh_gpu_stats();
                return;
            }

            // No arg: unload everything that is currently loaded.
            match rt.block_on(self.llm.list_ollama_ps()) {
                Ok(models) if !models.is_empty() => {
                    for m in &models {
                        let _ = rt.block_on(self.llm.ollama_unload(&m.name));
                    }
                    self.push_system(&format!(
                        "Unloaded {} model(s). VRAM should be freeing up.",
                        models.len()
                    ));
                    self.refresh_gpu_stats();
                }
                Ok(_) => {
                    self.push_system("Nothing to unload (no models reported by Ollama).");
                }
                Err(e) => {
                    self.push_system(&format!("Could not list for unload: {}", e));
                }
            }
        }
    }

    /// Entry point for the improved Quick local Ollama first-run experience.
    /// - Only offered when is_ollama_available() is true (checked on first boot + before showing menu).
    /// - Seeds the provider connection automatically (no key).
    /// - Fetches the *live* list of models from the user's Ollama (/v1/models or /api/tags).
    /// - Puts the user into a scrolling picker (reusing the wizard list UI) for CODER, then R1, then R2.
    /// - No more hardcoded model names (llama3.2 etc) that 404 on machines with different tags.
    fn start_quick_ollama_setup(&mut self) {
        if !self.is_ollama_available() {
            self.push_system("Ollama not detected at http://localhost:11434.");
            self.push_system("Install from https://ollama.com, run `ollama serve` (or launch the app), pull a couple models, then Ctrl+S or /config to set up again.");
            if self.config_wizard.is_none() {
                self.start_config_wizard();
            }
            return;
        }

        if let Err(e) = self.ensure_local_ollama_provider() {
            self.push_system(&format!("Quick Ollama provider setup failed: {}", e));
            return;
        }

        // Fetch the real models the user has available right now.
        let models: Vec<String> = if let Some(rt) = &self.runtime {
            match rt.block_on(self.llm.list_ollama_models()) {
                Ok(m) if !m.is_empty() => m,
                Ok(_) => {
                    self.push_system("Ollama is running but returned no models. Use `ollama pull <name>` for at least one model (e.g. llama3.1:8b), then retry quick setup.");
                    vec![]
                }
                Err(e) => {
                    self.push_system(&format!(
                        "Could not list Ollama models: {}. (Is Ollama still running?)",
                        e
                    ));
                    vec![]
                }
            }
        } else {
            vec![]
        };

        // Make sure we have an active wizard so the list scroller appears.
        if self.config_wizard.is_none() {
            self.start_config_wizard();
        }

        if let Some(w) = &mut self.config_wizard {
            w.ollama_model_list = models.clone();
            w.step = WizardStep::QuickOllamaModelPick {
                role: "coder".to_string(),
            };
            w.list_items = models;
            if !w.list_items.is_empty() {
                w.list_items.push("Other / type manually".to_string());
            }
            w.list_selected = 0;
            w.list_title = "Quick local Ollama — pick model for CODER (blue):".to_string();
        }

        self.first_run = false;
        self.reconcile_stage_from_disk();
        self.update_status();

        self.push_system("Local Ollama provider added (http://localhost:11434/v1, no key).");
        self.push_system("Scroll ↑↓ and Enter to choose your model for CODER (blue). You will then pick for R1 (purple) and R2 (lime).");
        self.push_system("This keeps the quick path fast while letting you use exactly the tags you have pulled.");
    }

    /// Helper used by the QuickOllamaModelPick wizard steps to advance to the next role
    /// while reusing the already-fetched ollama_model_list.
    fn enter_next_quick_pick(&mut self, next_role: &str, display: &str) {
        if let Some(w) = &mut self.config_wizard {
            w.step = WizardStep::QuickOllamaModelPick {
                role: next_role.to_string(),
            };
            w.list_items = w.ollama_model_list.clone();
            if !w.list_items.is_empty() {
                w.list_items.push("Other / type manually".to_string());
            }
            w.list_selected = 0;
            w.list_title = format!("Quick local Ollama — pick model for {}:", display);
        }
        self.push_system(&format!("Now pick for {} (scroll and Enter).", display));
    }

    /// For local Ollama (detected by base_url or provider name), fetch the *live* tags via
    /// the same probe the quick setup uses. For any other configured openai_compat / openai /
    /// azure_openai provider (xAI, Groq, OpenAI, Together, custom gateways, etc.) we call its
    /// /models endpoint (authenticated via the provider's credential) so the role assignment
    /// and "add model" pickers show the provider's current actual catalog instead of a stale
    /// static list. Always falls back to models_for_connection static suggestions on failure,
    /// missing base, no runtime, or non-compat provider types (anthropic/google keep their statics).
    /// When falling back for a remote provider we emit a visible [system] note so you can see
    /// why the live list wasn't used (bad/missing key, endpoint returned nothing, network, etc.).
    fn live_or_static_models_for_provider(
        &mut self,
        prov_name: &str,
        ptype: &str,
        base_url: Option<&str>,
    ) -> Vec<String> {
        let base = base_url.unwrap_or("");
        let is_local_ollama = base.contains("11434") || prov_name.to_lowercase().contains("ollama");
        if is_local_ollama {
            if let Some(rt) = &self.runtime {
                match rt.block_on(self.llm.list_ollama_models()) {
                    Ok(m) if !m.is_empty() => return m,
                    _ => {}
                }
            }
        } else if ptype == "openai_compat" || ptype == "openai" || ptype.starts_with("azure") {
            // Live pull for any set-up openai-compat provider (the key case for xAI etc.).
            if let Some(cfg) = &self.cfg {
                if let Some(conn) = cfg.providers.get(prov_name) {
                    let b = conn.base_url.as_deref().unwrap_or(base).trim();
                    if !b.is_empty() {
                        match self.llm.get_credential(prov_name, conn) {
                            Ok(key) => {
                                if let Some(rt) = &self.runtime {
                                    match rt.block_on(self.llm.list_openai_compat_models(b, &key)) {
                                        Ok(m) if !m.is_empty() => return m,
                                        Ok(_) => {
                                            self.push_system(&format!("[models] '{}' live /models returned no models — using built-in suggestions.", prov_name));
                                        }
                                        Err(e) => {
                                            self.push_system(&format!("[models] Live list error for '{}': {} (using suggestions)", prov_name, e));
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                self.push_system(&format!("[models] Could not get credential for '{}' ({}). Live model list skipped.", prov_name, e));
                            }
                        }
                    } else {
                        self.push_system(&format!("[models] Provider '{}' has no base_url configured; skipping live model fetch.", prov_name));
                    }
                }
            }
        }
        models_for_connection(ptype, base_url)
            .iter()
            .map(|s| s.to_string())
            .collect()
    }

    /// Returns true if the user has already configured at least one connection
    /// that appears to correspond to this provider preset (by name containing the
    /// suggested key, by type for strong-typed presets, or by base_url signature
    /// for well-known local endpoints like Ollama). Used to show a green check
    /// in the "Add / update a provider connection" list.
    fn is_provider_preset_configured(
        &self,
        display_name: &str,
        suggested_name: &str,
        ptype: &str,
    ) -> bool {
        let Some(cfg) = &self.cfg else {
            return false;
        };
        if suggested_name == "custom" {
            return false;
        }
        let s_lower = suggested_name.to_lowercase();
        let d_lower = display_name.to_lowercase();
        cfg.providers.iter().any(|(name, conn)| {
            let n = name.to_lowercase();
            // Name contains suggested (covers "local-ollama", "my-groq", "prod-anthropic" etc.)
            if n == s_lower || n.contains(&s_lower) || s_lower.contains(&n) {
                return true;
            }
            if n == d_lower
                || n.contains(
                    &d_lower
                        .replace(" (local)", "")
                        .replace(" (", "")
                        .replace(")", "")
                        .replace(" ", ""),
                )
            {
                return true;
            }
            // Strong type match for non-openai_compat presets (e.g. "anthropic", "google", "azure_openai")
            if conn.r#type == ptype && ptype != "openai_compat" {
                return true;
            }
            // Distinctive localhost ports for local openai_compat presets
            let base = conn.base_url.as_deref().unwrap_or("");
            if suggested_name == "ollama" && base.contains("11434") {
                return true;
            }
            if suggested_name == "lmstudio" && base.contains("1234") {
                return true;
            }
            false
        })
    }

    /// Build the list of choices shown when assigning a role (coder / reviewer-R1 / reviewer-R2).
    /// Now discovers *all* models from *all* configured providers (using live /models fetch for
    /// any openai_compat provider that is set up — xAI, Groq, OpenAI, Together, Ollama, custom etc. —
    /// and live tags for local Ollama; static suggestions only as last-resort fallback).
    /// Entries are grouped by provider (for visual separation) and each provider gets its own
    /// consistent color in the list. Plain binding nicknames (e.g. from quick setup) are included
    /// as a fallback at the end. Picking a model that has no binding yet auto-creates one using
    /// the correct provider.
    fn build_available_bindings_for_roles(&mut self) -> Vec<String> {
        let mut choices: Vec<String> = vec![];

        // Snapshot providers + existing bindings first. This ends the &self.cfg borrow
        // before we do any live model fetches (which require &mut self).
        // local snapshot tuple; a named alias would obscure more than it helps
        #[allow(clippy::type_complexity)]
        let (provider_infos, binding_keys): (
            Vec<(String, String, Option<String>)>,
            Vec<String>,
        ) = if let Some(cfg) = &self.cfg {
            let provs = cfg
                .providers
                .iter()
                .map(|(name, conn)| (name.clone(), conn.r#type.clone(), conn.base_url.clone()))
                .collect();
            let binds = cfg.model_bindings.keys().cloned().collect();
            (provs, binds)
        } else {
            (vec![], vec![])
        };

        if !provider_infos.is_empty() {
            // Collect models from every configured provider (live for Ollama-compatible,
            // static suggestions for others). Group by provider (sorted for stable order)
            // so the list visually separates models by their source provider.
            let mut by_prov: Vec<(String, Vec<String>)> = provider_infos
                .into_iter()
                .map(|(name, ptype, base)| {
                    let mods =
                        self.live_or_static_models_for_provider(&name, &ptype, base.as_deref());
                    (name, mods)
                })
                .collect();
            by_prov.sort_by(|a, b| a.0.cmp(&b.0));

            for (prov, models) in by_prov {
                for m in models {
                    // Encode provider in the choice string so render can color by provider
                    // and the selection handler can auto-bind to the correct provider.
                    choices.push(format!("{}  [{}]", m, prov));
                }
            }
        }

        // Include any existing binding keys (custom nicknames like "local-coder" from quick setup,
        // or ad-hoc names) that aren't already represented by a direct model entry above.
        // These go at the end; primary content is the per-provider models.
        for bname in binding_keys {
            let already = choices
                .iter()
                .any(|c| c == &bname || c.starts_with(&format!("{}  [", bname)));
            if !already {
                choices.push(bname);
            }
        }

        // Always offer a manual model-id entry per configured provider. Providers whose
        // /models endpoint returns nothing (Gradient, locked-down gateways) surface zero
        // models above; without this the role flow would dead-end with no way to type the
        // model id by hand. Encoded "<label>  [provider]" like the live entries.
        let prov_names: Vec<String> = if let Some(cfg) = &self.cfg {
            cfg.providers.keys().cloned().collect()
        } else {
            vec![]
        };
        for prov in prov_names {
            choices.push(format!("{}  [{}]", MANUAL_ENTRY_LABEL, prov));
        }

        choices
    }

    fn is_configured(&self) -> bool {
        self.cfg
            .as_ref()
            .is_some_and(|c| c.roles.reviewer_a.is_some() && c.roles.reviewer_b.is_some())
    }

    /// Parse a choice string from the role assignment list (which may be a plain binding
    /// nickname or an encoded "model  [provider]" entry) into (binding_name, provider, model).
    fn parse_role_choice(&self, choice: &str) -> (String, String, String) {
        let trimmed = choice.trim();
        if let Some(start) = trimmed.find('[') {
            if let Some(end_rel) = trimmed[start..].find(']') {
                let end = start + end_rel;
                let model = trimmed[..start].trim().to_string();
                let prov = trimmed[start + 1..end].trim().to_string();
                if !model.is_empty() && !prov.is_empty() {
                    // For encoded entries, use the short model as the binding key (consistent
                    // with prior live-tag and "add a model" behavior) and the indicated provider.
                    return (model.clone(), prov, model);
                }
            }
        }

        // Plain binding name (existing nickname or legacy entry). Use it as-is for binding/model,
        // and look up (or guess) the provider so auto-register would use the right one if needed.
        let prov = if let Some(cfg) = &self.cfg {
            if let Some(b) = cfg.model_bindings.get(trimmed) {
                b.provider.clone()
            } else if cfg.providers.contains_key("local-ollama") {
                "local-ollama".to_string()
            } else if let Some(first) = cfg.providers.keys().next() {
                first.clone()
            } else {
                "local-ollama".to_string()
            }
        } else {
            "local-ollama".to_string()
        };
        (trimmed.to_string(), prov, trimmed.to_string())
    }

    fn extract_provider_for_choice(&self, choice: &str) -> String {
        if let Some(start) = choice.find('[') {
            if let Some(end_rel) = choice[start..].find(']') {
                let p = choice[start + 1..start + end_rel].trim();
                if !p.is_empty() {
                    return p.to_string();
                }
            }
        }
        if let Some(cfg) = &self.cfg {
            if let Some(b) = cfg.model_bindings.get(choice) {
                return b.provider.clone();
            }
        }
        "unknown".to_string()
    }

    fn color_for_provider(&self, prov: &str) -> Color {
        if prov == "unknown" || prov.is_empty() {
            return Color::White;
        }
        const PALETTE: &[Color] = &[
            Color::Cyan,
            Color::LightGreen,
            Color::LightCyan,
            Color::Yellow,
            Color::LightMagenta,
            Color::Blue,
            Color::Green,
            Color::LightRed,
            Color::Rgb(255, 165, 0),   // orange
            Color::Rgb(180, 100, 255), // distinct purple-ish
        ];
        // Stable hash on provider name so the same provider always gets the same color.
        let mut h: u32 = 2166136261;
        for &b in prov.as_bytes() {
            h ^= b as u32;
            h = h.wrapping_mul(16777619);
        }
        PALETTE[(h as usize) % PALETTE.len()]
    }

    /// Send user text as a real chat turn to the "planner" role (falling back to coder).
    /// Starts a streaming response using the channel API so tokens append live in the UI.
    fn start_real_chat(&mut self, text: &str) {
        let cfg = match &self.cfg {
            Some(c) => c,
            None => {
                self.push_system("No configuration loaded. Press 's' for quick Ollama setup or run `anvil setup`.");
                return;
            }
        };

        let role = if cfg.roles.coder.is_some() {
            "coder"
        } else {
            self.push_system("No coder role configured. Use 's' or /config to assign a model.");
            return;
        };

        let (binding_name, binding, provider) = match cfg.resolve_role_full(role) {
            Ok(triple) => triple,
            Err(e) => {
                self.push_system(&format!("Role resolution failed for '{}': {}", role, e));
                return;
            }
        };

        let api_key = match self.llm.get_credential(&binding.provider, provider) {
            Ok(k) => k,
            Err(e) => {
                self.push_system(&format!(
                    "Credential error for binding '{}' (provider '{}'): {}",
                    binding_name, binding.provider, e
                ));
                self.push_system("For local providers (Ollama etc.) use the quick setup or /config and pick 'No authentication' / CredentialRef::None. Real providers need a key in the keyring or a valid env var.");
                return;
            }
        };

        // Clone what we need *before* any mutable calls on self (releases the
        // immutable borrow on self.cfg / binding / provider).
        let binding_name = binding_name.to_string();
        let model = binding.model.clone();
        let contract_for_agent = binding.contract.clone();
        let conn_for_task = provider.clone();
        let key_for_task = api_key.clone();
        // Effective config for the agent (specialist delegation needs it). Cloned
        // here while the `cfg` borrow is live, before the mutable self calls below.
        let cfg_for_agent = cfg.clone();

        // Per-turn correlation id for the jsonl log.
        let turn_id = Uuid::new_v4().to_string();
        self.current_turn_id = Some(turn_id.clone());
        self.current_role = Some(role.to_string());
        self.current_binding = Some(binding_name.clone());
        self.current_model = Some(model.clone());
        self.log_chat_event(
            "user",
            Some(&turn_id),
            Some(role),
            Some(&binding_name),
            Some(&model),
            text,
        );

        // Take an owned runtime handle so we can freely mutate self below before
        // spawning (a borrow of self.runtime would conflict with self.push etc.).
        let handle = match self.runtime.as_ref() {
            Some(rt) => rt.handle().clone(),
            None => {
                self.push_system("(internal) no runtime available for the agent");
                return;
            }
        };

        // Build the agent lazily on first use, then reuse it so conversation
        // history + tool context persist across turns. The agent reads/writes
        // the repo itself via tools — no manual /include, no /save-* needed.
        if self.agent.is_none() {
            // A LOCAL model can run under a bench-validated contract tier (set per
            // binding). Resolve it; a configured-but-unresolvable name warns and
            // falls back to the built-in prompt rather than silently mis-running.
            let system_prompt = coder_system_prompt(contract_for_agent.as_deref(), &self.root);
            if let Some(name) = contract_for_agent.as_deref() {
                if crate::contracts::resolve(name, &self.root).is_some() {
                    self.push_system(&format!("Coder contract: {name}"));
                } else {
                    self.push_system(&format!(
                        "(config) coder contract '{name}' not found (unknown tier or missing file) — using the built-in prompt"
                    ));
                }
            }
            let (confirm_tx, confirm_rx) = mpsc::unbounded_channel::<bool>();
            let agent = Agent::new(
                self.llm.clone(),
                conn_for_task,
                model.clone(),
                key_for_task,
                system_prompt,
                self.root.clone(),
                cfg_for_agent,
                ConfirmHandle::Channel(confirm_rx),
            );
            self.agent = Some(Arc::new(Mutex::new(agent)));
            self.confirm_tx = Some(confirm_tx);
        }

        // Streaming channel: receiver in the App, sender moved into the task.
        let (tx, rx) = mpsc::unbounded_channel::<String>();
        self.llm_rx = Some(rx);

        // Open the visible "[coder] " streaming line; tool/confirm lines will
        // close it (see drain_llm_stream) so interleaved text starts fresh lines.
        self.push(format!("[{}] ", role));
        self.assistant_open = true;
        self.follow_bottom = true;
        self.log_chat_event(
            "assistant_begin",
            Some(&turn_id),
            Some(role),
            Some(&binding_name),
            Some(&model),
            "[agent turn]",
        );

        let agent = self.agent.as_ref().unwrap().clone();
        let input = text.to_string();
        let join = handle.spawn(async move {
            // Hold the agent lock for the whole turn. The UI never locks the
            // agent during a turn (it only drains llm_rx and may send a confirm
            // decision over the separate confirm channel), so this can't deadlock.
            let mut guard = agent.lock().await;
            let _ = guard.run_turn(&input, tx).await;
            // When the task ends, `tx` drops → the UI sees stream disconnect.
        });
        // Keep an abort handle so Ctrl+B can pull the work off the anvil mid-turn.
        self.agent_task = Some(join.abort_handle());
    }

    /// Drain any pending token deltas from the current LLM stream and append them
    /// to the last message (the active streaming assistant response). Called frequently
    /// from the event loop so text appears live without blocking crossterm poll.
    fn drain_llm_stream(&mut self) -> bool {
        let mut changed = false;

        // Collect incoming deltas (and detect disconnect) without holding a mutable borrow on
        // the receiver across calls that also mutate self (push_system etc.).
        let mut deltas: Vec<String> = Vec::new();
        let mut stream_finished = false;

        if let Some(rx) = &mut self.llm_rx {
            loop {
                match rx.try_recv() {
                    Ok(delta) => deltas.push(delta),
                    Err(mpsc::error::TryRecvError::Empty) => break,
                    Err(mpsc::error::TryRecvError::Disconnected) => {
                        stream_finished = true;
                        break;
                    }
                }
            }
        }

        // Snapshot turn info so we can attribute every delta/full/final even as we mutate self.
        let turn = self.current_turn_id.clone();
        let role = self.current_role.clone();
        let binding = self.current_binding.clone();
        let model = self.current_model.clone();

        let mut had_llm_error = false;
        for delta in deltas {
            // The streaming layer captured the exact request a provider rejected.
            // Persist it so the failing payload can be inspected/shared (the API
            // key is in the auth header, not this body, so nothing secret leaks).
            if let Some(diag) = delta.strip_prefix("[error-request]") {
                let dir = self.root.join(".anvil");
                let _ = std::fs::create_dir_all(&dir);
                let path = dir.join("last-llm-error.json");
                let _ = std::fs::write(&path, diag);
                self.push_system(&format!(
                    "↳ Wrote the failing request + response to {} (share it to diagnose the provider error).",
                    path.display()
                ));
                changed = true;
                continue;
            }

            // Special handling for errors injected by the streaming layer (so the user
            // sees *why* there was no reply, e.g. Ollama not running, model not pulled,
            // bad endpoint, auth, etc.). We remove the placeholder assistant line we
            // started with the role prefix and surface a clean system message instead.
            if delta.contains("[llm-error]") {
                // Drop the "[coder] " starter if it's still empty.
                if let Some(last) = self.messages.last() {
                    if last.trim_end() == "[coder]" || last.trim_end().is_empty() {
                        let _ = self.messages.pop();
                    }
                }
                self.assistant_open = false;
                let clean = delta
                    .trim_start_matches('\n')
                    .trim_start_matches("[llm-error]")
                    .trim_start_matches(": ")
                    .trim_start_matches(' ')
                    .to_string();
                self.log_chat_event(
                    "error",
                    turn.as_deref(),
                    role.as_deref(),
                    binding.as_deref(),
                    model.as_deref(),
                    &clean,
                );
                self.push_system(&format!("model error: {}", clean));
                had_llm_error = true;
                changed = true;
                continue;
            }

            // A tool is about to run: close the current assistant line and show it.
            if let Some(label) = delta.strip_prefix("[tool-start]") {
                self.close_assistant_line();
                self.tool_active = true;
                self.log_chat_event(
                    "tool_start",
                    turn.as_deref(),
                    role.as_deref(),
                    binding.as_deref(),
                    model.as_deref(),
                    label,
                );
                self.push(format!("  🔨 {}", label.trim()));
                changed = true;
                continue;
            }
            // A tool finished: show its short result summary.
            if let Some(label) = delta.strip_prefix("[tool-end]") {
                self.tool_active = false;
                self.log_chat_event(
                    "tool_end",
                    turn.as_deref(),
                    role.as_deref(),
                    binding.as_deref(),
                    model.as_deref(),
                    label,
                );
                self.push(format!("    ↳ {}", label.trim()));
                changed = true;
                continue;
            }
            // The exact assembled prompt sent to the model this turn — logged to
            // the session JSONL for a complete audit trail, never shown in the UI.
            if let Some(prompt) = delta.strip_prefix("[prompt-log]") {
                self.log_chat_event(
                    "prompt_sent",
                    turn.as_deref(),
                    role.as_deref(),
                    binding.as_deref(),
                    model.as_deref(),
                    prompt,
                );
                continue;
            }
            // A non-intrusive advisory from the agent (e.g. the /compact nudge).
            if let Some(note) = delta.strip_prefix("[note]") {
                self.close_assistant_line();
                self.push_system(note.trim());
                changed = true;
                continue;
            }
            // The coder flagged a risk mid-task (flag_risk tool) — surface it
            // prominently now, not just at a gate. Also persisted to .anvil/risks.md.
            if let Some(note) = delta.strip_prefix("[risk]") {
                self.close_assistant_line();
                self.push_system(&format!("⚠ RISK FLAGGED by the coder: {}", note.trim()));
                self.push_system("  (recorded in .anvil/risks.md — review when convenient)");
                self.follow_bottom = true;
                changed = true;
                continue;
            }
            // A run_command needs the user's decision.
            if let Some(cmd) = delta.strip_prefix("[confirm]") {
                self.close_assistant_line();
                let cmd = cmd.trim().to_string();
                self.log_chat_event(
                    "confirm_request",
                    turn.as_deref(),
                    role.as_deref(),
                    binding.as_deref(),
                    model.as_deref(),
                    &cmd,
                );
                // Auto-approve commands the user's approval list covers (defaults to the
                // safe read-only set: `git status`/`git diff`/`cd`/`ls`/… — edit via
                // /approvals). These skip the prompt; the gate stays on everything else.
                if crate::tools::command_matches_prefixes(&cmd, &self.effective_auto_approve()) {
                    self.push_system(&format!(
                        "↳ auto-approved (in your /approvals list):  $ {}",
                        cmd
                    ));
                    if let Some(tx) = &self.confirm_tx {
                        let _ = tx.send(true);
                    }
                    changed = true;
                    continue;
                }
                // Auto-approve if this program was already allowed this session.
                let prog = program_of(&cmd);
                if !prog.is_empty() && self.approved_programs.contains(&prog) {
                    self.push_system(&format!(
                        "↳ auto-approved (`{}` allowed this session):  $ {}",
                        prog, cmd
                    ));
                    if let Some(tx) = &self.confirm_tx {
                        let _ = tx.send(true);
                    }
                    changed = true;
                    continue;
                }
                self.awaiting_confirm = Some(cmd.clone());
                self.confirm_selected = 0;
                self.push_system(&format!("Run command?  $ {}", cmd));
                self.push_system("↑/↓ to choose, Enter to confirm (or /y / /n).");
                changed = true;
                continue;
            }

            // Plain streamed text. If no assistant line is open (e.g. a tool line
            // was just shown), start a fresh "[coder] " line for the new segment.
            if !self.assistant_open {
                self.push("[coder] ".to_string());
                self.assistant_open = true;
            }
            self.log_chat_event(
                "assistant_delta",
                turn.as_deref(),
                role.as_deref(),
                binding.as_deref(),
                model.as_deref(),
                &delta,
            );
            if let Some(last) = self.messages.last_mut() {
                last.push_str(&delta);
            }
            changed = true;
        }

        if stream_finished {
            // The agent turn ended. Clear the receiver so we don't poll a dead channel.
            self.llm_rx = None;
            self.tool_active = false;
            // Drop a dangling empty "[coder] " line (turn ended on a tool call),
            // otherwise make sure the final line ends with a newline.
            self.close_assistant_line();
            {
                if let Some(last) = self.messages.last_mut() {
                    if !last.ends_with('\n') {
                        last.push('\n');
                    }
                }
            }
            // Log the final UI string for the turn. We take a fresh immutable borrow here so it doesn't overlap
            // with the previous mutable borrow from last_mut() when calling the &self logging method.
            if let Some(last) = self.messages.last() {
                self.log_chat_event(
                    "assistant_final_ui",
                    turn.as_deref(),
                    role.as_deref(),
                    binding.as_deref(),
                    model.as_deref(),
                    last,
                );
            }
            // Clear turn correlation so next chat gets a fresh id.
            self.current_turn_id = None;
            self.current_role = None;
            self.current_binding = None;
            self.current_model = None;

            // If this turn was a gate-driven coder fix/summary, advance the gate
            // (or abort if the model errored mid-fix).
            if self.gate_busy() {
                if had_llm_error {
                    self.abort_gate("the coder turn failed");
                } else {
                    self.gate_after_coder();
                }
            }
            changed = true;
        }

        changed
    }

    /// Inspect on-disk artifacts (plan.md + REVIEW_plan_R*.md at root + accepted hash in state) and derive
    /// the high-level WorkflowStage. The fine-grained sequential gate (/lock-plan and /accept-phase, each
    /// running R1 → coder fix → R2 → coder fix → summary) is enforced by command handlers + chat + file presence.
    /// Source of truth remains the REVIEW_* and plan.md files at repo root.
    /// Ensure the project is a git repo with a baseline commit (once per session).
    /// Anvil's review gates diff git, so a non-git folder silently produces empty
    /// reviews — this resolves it on first run and tells the user what happened.
    fn bootstrap_git_repo(&mut self) {
        if self.git_bootstrapped {
            return;
        }
        self.git_bootstrapped = true;
        let outcome = crate::git::ensure_repo_ready(&self.root);
        if let Some(msg) = crate::git::bootstrap_message(&outcome) {
            self.push_system(&msg);
        }
    }

    fn reconcile_stage_from_disk(&mut self) {
        if !self.is_configured() {
            self.stage = WorkflowStage::Unconfigured;
            return;
        }

        self.adopt_coder_named_plan_if_needed();

        let plan_path = self.plan_path();
        let rev_dir = reviews_dir(&self.root);
        let r1 = rev_dir.join("REVIEW_plan_R1.md");
        let r2 = rev_dir.join("REVIEW_plan_R2.md");

        if plan_path.exists() && r1.exists() && r2.exists() {
            let st = load_state(&self.root);
            if let Ok(plan_txt) = std::fs::read_to_string(&plan_path) {
                let current_hash = crate::plan::simple_hash(&plan_txt);
                if st.accepted_plan_hash.as_deref() == Some(current_hash.as_str()) {
                    self.stage = WorkflowStage::PlanAccepted;
                    return;
                }
            }
            self.stage = WorkflowStage::PlanReviewsComplete;
        } else {
            self.stage = WorkflowStage::Talk;
        }
    }

    /// If the user told the coder to write a plan and it created a feature-named
    /// `<name>_plan.md` rather than the literal active plan file, adopt the newest such
    /// file as the active plan. Only fires pre-accept and only when the current active
    /// plan file is absent, so it never overrides an explicit /new-plan choice.
    fn adopt_coder_named_plan_if_needed(&mut self) {
        let st = load_state(&self.root);
        if st.accepted_plan_hash.is_some() {
            return;
        }
        let active = active_plan_name(&self.root);
        if self.root.join(&active).exists() {
            return;
        }
        // Active plan file is missing — look for a coder-written plan at the repo root.
        let mut newest: Option<(std::time::SystemTime, String)> = None;
        if let Ok(entries) = std::fs::read_dir(&self.root) {
            for entry in entries.flatten() {
                let p = entry.path();
                if !p.is_file() {
                    continue;
                }
                let Some(name) = p.file_name().and_then(|s| s.to_str()) else {
                    continue;
                };
                let is_plan = name == "plan.md"
                    || (name.ends_with("_plan.md") && name.len() > "_plan.md".len());
                if !is_plan {
                    continue;
                }
                let mtime = entry
                    .metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::UNIX_EPOCH);
                if newest.as_ref().map(|(t, _)| mtime > *t).unwrap_or(true) {
                    newest = Some((mtime, name.to_string()));
                }
            }
        }
        if let Some((_, name)) = newest {
            if name != active {
                let mut st2 = load_state(&self.root);
                st2.active_plan = Some(name.clone());
                let _ = save_state(&self.root, &st2);
                self.push_system(&format!(
                    "Adopted '{name}' as the active plan (the coder named it). /lock-plan when ready."
                ));
            }
        }
    }

    /// Drain reviewer-gate output (from /lock-plan and /accept-phase). The gate
    /// task sends display-ready strings (one per round, plus a final marker) and
    /// then drops its sender. We surface each line in the transcript and, on
    /// disconnect, reconcile the stage from disk. A gate run can emit several
    /// messages (R1 then R2), so we drain them all rather than stopping at one.
    fn drain_gate_events(&mut self) -> bool {
        let mut changed = false;
        let mut msgs: Vec<String> = Vec::new();
        let mut finished = false;
        if let Some(rx) = &mut self.gate_rx {
            loop {
                match rx.try_recv() {
                    Ok(msg) => msgs.push(msg),
                    Err(mpsc::error::TryRecvError::Empty) => break,
                    Err(mpsc::error::TryRecvError::Disconnected) => {
                        finished = true;
                        break;
                    }
                }
            }
        }

        let mut had_error = false;
        for msg in msgs {
            if let Some(findings) = msg.strip_prefix("[findings]") {
                // "[findings]<header>\n<body>" — header on its own system line, body verbatim.
                let (header, body) = findings.split_once('\n').unwrap_or((findings, ""));
                self.push_system(header);
                if !body.trim().is_empty() {
                    self.push(body.to_string());
                }
            } else if let Some(err) = msg.strip_prefix("[gate-error]") {
                self.push_system(&format!("Reviewer run failed: {}", err.trim()));
                had_error = true;
            } else {
                self.push_system(&msg);
            }
            changed = true;
        }

        if finished {
            self.gate_rx = None;
            // If a sequential gate is waiting on this reviewer, advance it (or abort
            // on error). Otherwise just reconcile the stage from disk as before.
            let reviewing = matches!(
                self.gate_flow.as_ref().map(|f| f.step.clone()),
                Some(GateStep::R1Reviewing) | Some(GateStep::R2Reviewing)
            );
            if reviewing {
                if had_error {
                    self.abort_gate("the reviewer run failed");
                } else {
                    self.gate_after_review();
                }
            } else {
                self.reconcile_stage_from_disk();
                self.update_status();
            }
            changed = true;
        }
        changed
    }

    // ─── Sequential review gate (R1 → fix → pause → R2 → fix → pause → summary) ───

    /// Short human label for the artifact under review.
    fn gate_artifact_label(artifact: &GateArtifact) -> String {
        match artifact {
            GateArtifact::Plan => "plan.md".to_string(),
            GateArtifact::Phase(id) => format!("phase {} diff", id),
            GateArtifact::Addition { slug, .. } => format!("addition '{}'", slug),
        }
    }

    /// Path to the review file the reviewer just wrote, for a given round.
    fn gate_review_path(&self, artifact: &GateArtifact, round: Round) -> PathBuf {
        let r = if round == Round::R1 { "R1" } else { "R2" };
        let stem = match artifact {
            GateArtifact::Plan => "plan".to_string(),
            GateArtifact::Phase(id) => id.clone(),
            GateArtifact::Addition { slug, .. } => slug.clone(),
        };
        reviews_dir(&self.root).join(format!("REVIEW_{}_{}.md", stem, r))
    }

    /// Begin a sequential review gate: kick off R1.
    fn start_gate_flow(&mut self, artifact: GateArtifact) {
        let label = Self::gate_artifact_label(&artifact);
        match &artifact {
            // Phase gate: the coder first writes a review briefing (what was built
            // and WHY) so the reviewers have intent + rationale, not just the diff.
            GateArtifact::Phase(id) => {
                let id = id.clone();
                self.gate_flow = Some(GateFlow {
                    artifact: artifact.clone(),
                    step: GateStep::BriefWriting,
                });
                self.push_system(&format!(
                    "── Review gate started on {} ──  coder writes the review briefing → R1 → coder fixes → (pause) → R2 → coder fixes → (pause) → summary.",
                    label
                ));
                self.push_system(
                    "→ Coder writing the phase review briefing (what was built & why)…",
                );
                self.follow_bottom = true;
                let prompt = crate::phase::briefing_prompt(&id);
                self.gate_drive_coder(&prompt);
            }
            // Addition gate (`/review`): like the phase gate, the coder first writes
            // a short briefing, then R1 critiques the diff. R2 is opt-in (`--deep`),
            // handled later in the R1Fixing step — there's no ship/accept here.
            GateArtifact::Addition { slug, deep } => {
                let slug = slug.clone();
                let rounds = if *deep {
                    "R1 → coder fixes → (pause) → R2 → coder fixes → (pause) → summary"
                } else {
                    "R1 → coder fixes → summary"
                };
                self.gate_flow = Some(GateFlow {
                    artifact: artifact.clone(),
                    step: GateStep::BriefWriting,
                });
                self.push_system(&format!(
                    "── Review gate started on {} ──  coder writes the review briefing → {}.",
                    label, rounds
                ));
                self.push_system(
                    "→ Coder writing the addition review briefing (what changed & why)…",
                );
                self.follow_bottom = true;
                let prompt = crate::phase::addition_briefing_prompt(&slug);
                self.gate_drive_coder(&prompt);
            }
            // Plan gate: reviews plan.md directly — no diff/briefing, start at R1.
            GateArtifact::Plan => {
                self.gate_flow = Some(GateFlow {
                    artifact: artifact.clone(),
                    step: GateStep::R1Reviewing,
                });
                self.push_system(&format!(
                    "── Review gate started on {} ──  R1 → coder fixes → (pause) → R2 → coder fixes → (pause) → summary.",
                    label
                ));
                self.push_system("Running R1 (reviewer-a)…");
                self.follow_bottom = true;
                self.spawn_review(&artifact, Round::R1);
            }
        }
    }

    /// Spawn a single review round (writes REVIEW_*.md, streams findings over gate_rx).
    fn spawn_review(&mut self, artifact: &GateArtifact, round: Round) {
        let (tx, rx) = mpsc::unbounded_channel::<String>();
        self.gate_rx = Some(rx);
        let round_lbl = if round == Round::R1 { "R1" } else { "R2" };
        let reviewer = if round == Round::R1 {
            "reviewer-a"
        } else {
            "reviewer-b"
        };
        let label = Self::gate_artifact_label(artifact);
        let artifact = artifact.clone();
        let root = self.root.clone();
        if let Some(rt) = &self.runtime {
            rt.spawn(async move {
                let result: anyhow::Result<String> =
                    tokio::task::spawn_blocking(move || match (artifact, round) {
                        (GateArtifact::Plan, Round::R1) => crate::plan::run_plan_r1(&root),
                        (GateArtifact::Plan, Round::R2) => crate::plan::run_plan_r2(&root),
                        (GateArtifact::Phase(id), Round::R1) => {
                            crate::phase::run_phase_r1_diff(&root, &id)
                        }
                        (GateArtifact::Phase(id), Round::R2) => {
                            crate::phase::run_phase_r2_diff(&root, &id)
                        }
                        (GateArtifact::Addition { slug, .. }, Round::R1) => {
                            crate::phase::run_addition_r1_diff(&root, &slug)
                        }
                        (GateArtifact::Addition { slug, .. }, Round::R2) => {
                            crate::phase::run_addition_r2_diff(&root, &slug)
                        }
                    })
                    .await
                    .unwrap_or_else(|e| Err(anyhow::anyhow!("{}", e)));
                match result {
                    Ok(f) => {
                        let _ = tx.send(format!(
                            "[findings]✓ {} ({}) on {} — review written:\n{}",
                            round_lbl, reviewer, label, f
                        ));
                    }
                    Err(e) => {
                        let _ = tx.send(format!("[gate-error]{}: {}", round_lbl, e));
                    }
                }
            });
        }
    }

    /// A reviewer round just finished: send the findings to the coder to apply
    /// fixes (a real agent turn). Advances R1Reviewing→R1Fixing / R2Reviewing→R2Fixing.
    fn gate_after_review(&mut self) {
        let Some(flow) = self.gate_flow.clone() else {
            return;
        };
        let round = match flow.step {
            GateStep::R1Reviewing => Round::R1,
            GateStep::R2Reviewing => Round::R2,
            _ => return,
        };
        let findings = std::fs::read_to_string(self.gate_review_path(&flow.artifact, round))
            .unwrap_or_else(|_| "(could not read the review file)".to_string());
        let round_lbl = if round == Round::R1 { "R1" } else { "R2" };

        let prompt = match &flow.artifact {
            GateArtifact::Plan => format!(
                "Review round {0} on plan.md returned the findings below. Edit plan.md directly \
                 (use your tools) to address the real, actionable issues. Ignore anything spurious, \
                 keep changes tight, and don't expand scope. When done, stop — do not summarize yet.\n\n\
                 --- {0} FINDINGS ---\n{1}\n--- END FINDINGS ---",
                round_lbl, findings
            ),
            GateArtifact::Phase(id) => format!(
                "Review round {0} on the {1} phase diff returned the findings below. Apply fixes to \
                 the code for the real issues they raised (edit files and run tests). Don't expand \
                 scope. When done, stop — do not summarize yet.\n\n\
                 --- {0} FINDINGS ---\n{2}\n--- END FINDINGS ---",
                round_lbl, id, findings
            ),
            GateArtifact::Addition { slug, .. } => format!(
                "Review round {0} on the addition '{1}' returned the findings below. Apply fixes to \
                 the code for the real issues they raised (edit files and run tests). Keep it tight — \
                 this is a small addition, don't expand scope. When done, stop — do not summarize yet.\n\n\
                 --- {0} FINDINGS ---\n{2}\n--- END FINDINGS ---",
                round_lbl, slug, findings
            ),
        };

        self.push_system(&format!("→ Coder addressing {} findings…", round_lbl));
        self.set_gate_step(if round == Round::R1 {
            GateStep::R1Fixing
        } else {
            GateStep::R2Fixing
        });
        self.gate_drive_coder(&prompt);
    }

    /// A coder fix/summary turn just finished: advance the gate. Called from
    /// drain_llm_stream when a gate-driven turn completes.
    fn gate_after_coder(&mut self) {
        let Some(flow) = self.gate_flow.clone() else {
            return;
        };
        match flow.step {
            GateStep::BriefWriting => {
                // Briefing written by the coder — now run R1 with it in context.
                self.push_system("→ Briefing written. Running R1 (reviewer-a)…");
                self.set_gate_step(GateStep::R1Reviewing);
                self.follow_bottom = true;
                self.spawn_review(&flow.artifact, Round::R1);
            }
            GateStep::R1Fixing => {
                // R1-only additions have no R2 — go straight to the wrap-up summary
                // instead of pausing. Everything else (plan, phase, --deep addition)
                // pauses so the user can run R2.
                let r1_only_addition =
                    matches!(&flow.artifact, GateArtifact::Addition { deep, .. } if !*deep);
                if r1_only_addition {
                    self.push_system("→ Coder summarizing the R1 review…");
                    self.set_gate_step(GateStep::Summarizing);
                    self.follow_bottom = true;
                    let prompt =
                        "You've had your addition reviewed (R1) and applied fixes. Summarize \
                         concisely for the user: what the addition does, the key issues R1 raised, \
                         and the fixes you applied. The change is already in the working tree — there \
                         is no ship/accept step, so do NOT suggest a command."
                            .to_string();
                    self.gate_drive_coder(&prompt);
                } else {
                    self.set_gate_step(GateStep::PausedAfterR1);
                    self.push_system(
                        "⏸  R1 complete and fixes applied. Review the changes above, then /continue \
                         (or press Enter on an empty line) to run R2.",
                    );
                    self.follow_bottom = true;
                }
            }
            GateStep::R2Fixing => {
                self.set_gate_step(GateStep::PausedAfterR2);
                self.push_system(
                    "⏸  R2 complete and fixes applied. Review the changes above, then /continue \
                     (or press Enter on an empty line) for the coder's summary.",
                );
                self.follow_bottom = true;
            }
            GateStep::Summarizing => {
                self.set_gate_step(GateStep::Done);
                let done_msg = match &flow.artifact {
                    GateArtifact::Plan => "✓ Review gate complete (R1 + R2, fixes applied both rounds) — the work is tempered. When you're happy, /accept-plan to quench it.".to_string(),
                    GateArtifact::Phase(id) => format!(
                        "✓ Review gate complete (R1 + R2, fixes applied both rounds) — the work is tempered. When you're happy, /ship-phase {} to quench it.",
                        id
                    ),
                    GateArtifact::Addition { slug, deep } => {
                        let rounds = if *deep { "R1 + R2" } else { "R1" };
                        let mut m = format!(
                            "✓ Addition review complete ({}, fixes applied) — the work is in your working tree; commit it when you're happy.",
                            rounds
                        );
                        if !*deep {
                            m.push_str(&format!(
                                " For a second cross-vendor opinion, run /review --deep {}.",
                                slug
                            ));
                        }
                        m
                    }
                };
                self.push_system(&done_msg);
                self.gate_flow = None; // gate done; the accept/ship command checks files as before
                self.reconcile_stage_from_disk();
                self.update_status();
                self.follow_bottom = true;
            }
            _ => {}
        }
    }

    /// Resume a paused gate: run R2, or kick off the final summary.
    fn continue_gate(&mut self) {
        let Some(flow) = self.gate_flow.clone() else {
            return;
        };
        match flow.step {
            GateStep::PausedAfterR1 => {
                self.push_system("Running R2 (reviewer-b)…");
                self.set_gate_step(GateStep::R2Reviewing);
                self.follow_bottom = true;
                self.spawn_review(&flow.artifact, Round::R2);
            }
            GateStep::PausedAfterR2 => {
                // Give the coder the EXACT proceed command so its summary can't invent
                // one (it had been saying /lock-plan instead of /accept-plan). Additions
                // have no proceed command — the work is already in the tree.
                let prompt = match &flow.artifact {
                    GateArtifact::Addition { .. } => {
                        "Both review rounds are done and you've applied fixes for each. Summarize \
                         concisely for the user: what the addition does, the key issues R1 and R2 \
                         raised, and the fixes you applied in each round. The change is already in \
                         the working tree — there is no ship/accept step, so do NOT suggest a command."
                            .to_string()
                    }
                    other => {
                        let accept = match other {
                            GateArtifact::Plan => "/accept-plan".to_string(),
                            GateArtifact::Phase(id) => format!("/ship-phase {}", id),
                            GateArtifact::Addition { .. } => unreachable!(),
                        };
                        format!(
                            "Both review rounds are done and you've applied fixes for each. Summarize for \
                             the user, concisely: what this plan/phase delivers, the key issues R1 and R2 \
                             raised, and the fixes you applied in each round. If you mention how to proceed, \
                             the exact command is `{}` — use it verbatim; do not suggest any other command.",
                            accept
                        )
                    }
                };
                self.push_system("→ Coder summarizing the review rounds…");
                self.set_gate_step(GateStep::Summarizing);
                self.follow_bottom = true;
                self.gate_drive_coder(&prompt);
            }
            _ => {
                self.push_system("The gate isn't paused right now — let the current step finish.");
            }
        }
    }

    /// Abort the active gate (reviewer or coder error mid-flow).
    fn abort_gate(&mut self, why: &str) {
        if self.gate_flow.take().is_some() {
            self.push_system(&format!(
                "Review gate stopped: {}. Fix the issue and re-run /lock-plan or /accept-phase.",
                why
            ));
            self.reconcile_stage_from_disk();
            self.update_status();
        }
    }

    fn set_gate_step(&mut self, step: GateStep) {
        if let Some(flow) = &mut self.gate_flow {
            flow.step = step;
        }
    }

    /// Start a gate-driven coder turn. If it can't start (e.g. no coder role /
    /// bad credentials), `start_real_chat` leaves `llm_rx` unset — abort the gate
    /// rather than hang forever waiting on a turn that never runs.
    fn gate_drive_coder(&mut self, prompt: &str) {
        self.start_real_chat(prompt);
        if self.llm_rx.is_none() {
            self.abort_gate(
                "the coder turn could not start (check the coder role and credentials)",
            );
        }
    }

    /// True while the gate is actively running a step (not paused / not idle), so
    /// plain chat input should be held back to avoid colliding with gate turns.
    fn gate_busy(&self) -> bool {
        matches!(
            self.gate_flow.as_ref().map(|f| &f.step),
            Some(GateStep::BriefWriting)
                | Some(GateStep::R1Reviewing)
                | Some(GateStep::R1Fixing)
                | Some(GateStep::R2Reviewing)
                | Some(GateStep::R2Fixing)
                | Some(GateStep::Summarizing)
        )
    }

    /// True when the gate is paused waiting for the user to /continue.
    fn gate_paused(&self) -> bool {
        matches!(
            self.gate_flow.as_ref().map(|f| &f.step),
            Some(GateStep::PausedAfterR1) | Some(GateStep::PausedAfterR2)
        )
    }

    /// One-line status of the active review gate, shown in the header just right
    /// of the phase progress. None when no gate is running (so it clears when the
    /// gate finishes). For reviewing steps it names the reviewer's model.
    fn gate_header_status(&self) -> Option<String> {
        let flow = self.gate_flow.as_ref()?;
        let model = |role: &str| -> String {
            self.cfg
                .as_ref()
                .and_then(|c| c.resolve_role_or_binding(role).ok())
                .map(|(_, b, _)| b.model.clone())
                .unwrap_or_else(|| "?".to_string())
        };
        let s = match flow.step {
            GateStep::BriefWriting => "coder writing review briefing".to_string(),
            GateStep::R1Reviewing => format!("R1 reviewing — {}", model("reviewer_a")),
            GateStep::R1Fixing => "coder applying R1 fixes".to_string(),
            GateStep::PausedAfterR1 => "R1 done — /continue for R2".to_string(),
            GateStep::R2Reviewing => format!("R2 reviewing — {}", model("reviewer_b")),
            GateStep::R2Fixing => "coder applying R2 fixes".to_string(),
            GateStep::PausedAfterR2 => "R2 done — /continue for summary".to_string(),
            GateStep::Summarizing => "coder summarizing".to_string(),
            GateStep::Done => return None,
        };
        Some(s)
    }

    /// Refresh models.dev metadata in the background (cached 7 days; no-op when
    /// fresh). Populates the cache the rest of the session reads from.
    fn spawn_modelsdev_refresh(&self) {
        if let Some(rt) = &self.runtime {
            rt.spawn(async move {
                crate::modelsdev::refresh_if_stale().await;
            });
        }
    }

    /// Warn at startup if the coder's model is known (via models.dev) to lack
    /// tool-calling — the coder needs it to read/edit/run. Best-effort: silent
    /// when there's no config or no cached metadata yet (first run before the
    /// background refresh finishes).
    fn warn_coder_tool_calling(&mut self) {
        let model = match &self.cfg {
            Some(cfg) => match cfg.resolve_role_or_binding("coder") {
                Ok((_, binding, _)) => binding.model.clone(),
                Err(_) => return,
            },
            None => return,
        };
        let Some(db) = crate::modelsdev::load() else {
            return;
        };
        if db.lookup(&model).and_then(|i| i.tool_call) == Some(false) {
            self.push_system(&format!(
                "⚠ The coder model '{}' is listed (models.dev) as NOT supporting tool calls — the coder needs them to read, edit, and run. It will likely just reply in text. Consider a tool-calling model for the coder role (Ctrl+S / /config).",
                model
            ));
        }
    }

    /// Ctrl+B: pull the current work off the anvil — abort the in-flight coder
    /// turn (and any review gate), deny a pending command confirm, and reset
    /// transient turn state so the user can redirect.
    fn interrupt_agent(&mut self) {
        let running = self.llm_rx.is_some()
            || self.gate_rx.is_some()
            || self.awaiting_confirm.is_some()
            || self.agent_task.is_some();
        if !running {
            self.push_system("Nothing running to interrupt.");
            return;
        }
        // Tell any in-flight run_command to kill its (possibly hung) child tree NOW.
        // Aborting the async task alone can't stop a synchronous blocking command;
        // this flag is what actually terminates a looping test/server.
        crate::tools::request_command_interrupt();
        if let Some(h) = self.agent_task.take() {
            h.abort();
        }
        // Deny a pending run_command confirm so the (aborted) task can't proceed.
        if self.awaiting_confirm.take().is_some() {
            if let Some(tx) = &self.confirm_tx {
                let _ = tx.send(false);
            }
        }
        self.llm_rx = None;
        self.gate_rx = None;
        self.tool_active = false;
        self.gate_flow = None;
        self.close_assistant_line();
        self.push_system(
            "⏹ Pulled the work off the anvil (Ctrl+B). The coder stopped — tell it what to do next.",
        );
        self.follow_bottom = true;
    }

    /// Resolve the pending run_command confirm. choice: 0=yes once, 1=yes and
    /// remember this program for the session, 2=no.
    fn resolve_confirm(&mut self, choice: usize) {
        let Some(cmd) = self.awaiting_confirm.take() else {
            return;
        };
        let allow = choice <= 1;
        if choice == 1 {
            let prog = program_of(&cmd);
            if !prog.is_empty() {
                self.approved_programs.insert(prog.clone());
                self.push_system(&format!(
                    "✓ Approved — and won't ask again for `{}` commands this session.",
                    prog
                ));
            } else {
                self.push_system("✓ Approved — running the command.");
            }
        } else if allow {
            self.push_system("✓ Approved — running the command.");
        } else {
            self.push_system("✗ Declined.");
        }
        if let Some(tx) = &self.confirm_tx {
            let _ = tx.send(allow);
        }
        self.follow_bottom = true;
    }

    /// The command prefixes that currently auto-approve: the user's configured list
    /// if they've set one, otherwise the built-in safe read-only defaults.
    fn effective_auto_approve(&self) -> Vec<String> {
        self.cfg
            .as_ref()
            .and_then(|c| c.approvals.auto_approve.clone())
            .unwrap_or_else(crate::tools::default_safe_prefixes)
    }

    /// `/approvals` — open the command-approval checklist. Rows = the user's current
    /// approved prefixes (checked) unioned with a suggested catalog (unchecked).
    fn open_approvals_editor(&mut self) {
        if self.cfg.is_none() {
            self.cfg = load_config(&self.root).ok();
        }
        let approved = self.effective_auto_approve();
        let mut items: Vec<ApprovalItem> = Vec::new();
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

        // Approved entries first (checked), preserving the user's order.
        for p in &approved {
            if seen.insert(p.clone()) {
                items.push(ApprovalItem {
                    prefix: p.clone(),
                    approved: true,
                });
            }
        }
        // Then suggestions not already present (unchecked, opt-in).
        for p in crate::tools::suggested_command_catalog() {
            if seen.insert(p.clone()) {
                items.push(ApprovalItem {
                    prefix: p,
                    approved: false,
                });
            }
        }

        self.approvals_editor = Some(ApprovalsEditor { items, selected: 0 });
        self.push_system("=== COMMAND APPROVALS ===");
        self.push_system("Checked commands run WITHOUT a prompt. ↑/↓ move · Space toggles · type a prefix then Enter to add a custom one · Esc saves & closes.");
        self.follow_bottom = true;
    }

    /// Toggle the highlighted row in the approvals editor.
    fn approvals_toggle_selected(&mut self) {
        if let Some(ed) = &mut self.approvals_editor {
            if let Some(item) = ed.items.get_mut(ed.selected) {
                item.approved = !item.approved;
            }
        }
    }

    /// Add a typed custom prefix to the approvals editor (approved), if not present.
    fn approvals_add_custom(&mut self, raw: &str) {
        let prefix = raw.trim().to_string();
        if prefix.is_empty() {
            return;
        }
        if let Some(ed) = &mut self.approvals_editor {
            if let Some(pos) = ed.items.iter().position(|i| i.prefix == prefix) {
                // Already listed — just approve + highlight it.
                ed.items[pos].approved = true;
                ed.selected = pos;
            } else {
                ed.items.insert(
                    0,
                    ApprovalItem {
                        prefix: prefix.clone(),
                        approved: true,
                    },
                );
                ed.selected = 0;
            }
            self.push_system(&format!("+ added \"{}\" to approvals", prefix));
        }
    }

    /// Save the approvals editor's checked rows to the GLOBAL config and close it.
    fn approvals_save_and_close(&mut self) {
        let Some(ed) = self.approvals_editor.take() else {
            return;
        };
        let approved: Vec<String> = ed
            .items
            .iter()
            .filter(|i| i.approved)
            .map(|i| i.prefix.clone())
            .collect();
        let count = approved.len();
        let cfg = self.cfg.get_or_insert_with(AnvilConfig::default);
        cfg.approvals.auto_approve = Some(approved);
        match save_global_config(cfg) {
            Ok(()) => self.push_system(&format!(
                "✓ Saved {} approved command prefix(es) to your global config. They apply in every repo.",
                count
            )),
            Err(e) => self.push_system(&format!("Could not save approvals: {}", e)),
        }
        // Reload so the merged in-memory view matches what's on disk.
        self.cfg = load_config(&self.root).ok();
        self.follow_bottom = true;
    }

    /// `/models` — show each role's model with its models.dev facts (context
    /// window, tool-call support, price).
    fn show_models(&mut self) {
        if self.cfg.is_none() {
            self.push_system("No config loaded yet — use Ctrl+S or /config to set up models.");
            return;
        }
        // Collect (label, model) per role, then drop the &self.cfg borrow.
        let entries: Vec<(&'static str, String)> = {
            let cfg = self.cfg.as_ref().unwrap();
            [
                ("CODER", "coder"),
                ("R1 reviewer", "reviewer_a"),
                ("R2 reviewer", "reviewer_b"),
            ]
            .iter()
            .filter_map(|(label, role)| {
                cfg.resolve_role_or_binding(role)
                    .ok()
                    .map(|(_, b, _)| (*label, b.model.clone()))
            })
            .collect()
        };

        let db = crate::modelsdev::load();
        self.push_system("Configured models (facts via models.dev):");
        for (label, model) in entries {
            let line = match db.as_ref().and_then(|d| d.lookup(&model)) {
                Some(i) => {
                    let ctx = i
                        .context
                        .map(|c| format!("{}k ctx", c / 1000))
                        .unwrap_or_else(|| "ctx ?".into());
                    let max_out = i
                        .output
                        .map(|o| format!(", {}k out", o / 1000))
                        .unwrap_or_default();
                    let tools = match i.tool_call {
                        Some(true) => "tools ✓",
                        Some(false) => "tools ✗",
                        None => "tools ?",
                    };
                    let price = match (i.cost_input, i.cost_output) {
                        (Some(ci), Some(co)) => format!("${:.2}/${:.2} per 1M", ci, co),
                        _ => "price ?".to_string(),
                    };
                    let named = if i.name != model {
                        format!(" [{}]", i.name)
                    } else {
                        String::new()
                    };
                    format!(
                        "  {} — {}{} · {}{} · {} · {}",
                        label, model, named, ctx, max_out, tools, price
                    )
                }
                None => format!(
                    "  {} — {} · (not in models.dev / metadata unavailable)",
                    label, model
                ),
            };
            self.push_system(&line);
        }
        if db.is_none() {
            self.push_system(
                "(models.dev metadata isn't cached yet — it refreshes in the background; try /models again shortly.)",
            );
        }
    }

    /// Kick off a one-shot, non-blocking check for a newer release. Runs the
    /// (blocking) GitHub call on the runtime's blocking pool so boot stays
    /// instant; only sends if a newer version exists. Best-effort — any failure
    /// is silent. Respects ANVIL_NO_UPDATE_CHECK (handled inside the check).
    fn spawn_update_check(&mut self) {
        if self.runtime.is_none() {
            return;
        }
        let (tx, rx) = mpsc::unbounded_channel::<String>();
        self.update_rx = Some(rx);
        let root = self.root.clone();
        if let Some(rt) = &self.runtime {
            rt.spawn(async move {
                if let Ok(Some(v)) = tokio::task::spawn_blocking(move || {
                    crate::update::check_with_cache_blocking(&root)
                })
                .await
                {
                    let _ = tx.send(v);
                }
            });
        }
    }

    /// Drain the boot update-check result + any in-flight /update apply status.
    fn drain_update_events(&mut self) -> bool {
        let mut changed = false;

        if let Some(rx) = &mut self.update_rx {
            if let Ok(v) = rx.try_recv() {
                self.update_available = Some(v);
                self.update_rx = None; // one-shot
                changed = true;
            }
        }

        let mut apply_msgs: Vec<String> = Vec::new();
        let mut apply_done = false;
        if let Some(rx) = &mut self.update_apply_rx {
            loop {
                match rx.try_recv() {
                    Ok(msg) => apply_msgs.push(msg),
                    Err(mpsc::error::TryRecvError::Empty) => break,
                    Err(mpsc::error::TryRecvError::Disconnected) => {
                        apply_done = true;
                        break;
                    }
                }
            }
        }
        for msg in apply_msgs {
            if let Some(ok) = msg.strip_prefix("[update-ok]") {
                self.update_available = None; // applied — clear the indicator
                self.push_system(ok.trim());
            } else if let Some(err) = msg.strip_prefix("[update-error]") {
                self.push_system(&format!("Update failed: {}", err.trim()));
            } else {
                self.push_system(&msg);
            }
            changed = true;
        }
        if apply_done {
            self.update_apply_rx = None;
            self.update_in_progress = false;
            changed = true;
        }
        changed
    }

    /// Apply the available update in the background (download + self-replace),
    /// streaming status back over update_apply_rx.
    fn spawn_update_apply(&mut self) {
        if self.update_in_progress || self.runtime.is_none() {
            return;
        }
        self.update_in_progress = true;
        self.push_system("Updating anvil — downloading the latest release…");
        let (tx, rx) = mpsc::unbounded_channel::<String>();
        self.update_apply_rx = Some(rx);
        let current = crate::update::current_version().to_string();
        if let Some(rt) = &self.runtime {
            rt.spawn(async move {
                let res = tokio::task::spawn_blocking(crate::update::apply_update_blocking).await;
                match res {
                    Ok(Ok(v)) if v == current => {
                        let _ = tx.send(format!("[update-ok]Already up to date (v{}).", current));
                    }
                    Ok(Ok(v)) => {
                        let _ = tx.send(format!(
                            "[update-ok]✓ Updated {} → {}. Quit and relaunch anvil to use the new version.",
                            current, v
                        ));
                    }
                    Ok(Err(e)) => {
                        let _ = tx.send(format!("[update-error]{}", e));
                    }
                    Err(e) => {
                        let _ = tx.send(format!("[update-error]{}", e));
                    }
                }
            });
        }
    }

    /// Returns the subset of SLASH_COMMANDS that match the text after the leading `/`.
    /// Empty filter (just "/") shows everything. Used for the live palette popup.
    fn filtered_commands(&self) -> Vec<(&'static str, &'static str)> {
        let filter = self
            .input
            .strip_prefix('/')
            .unwrap_or("")
            .trim()
            .to_lowercase();
        SLASH_COMMANDS
            .iter()
            .copied()
            .filter(|(cmd, _desc)| {
                if filter.is_empty() {
                    true
                } else {
                    cmd.strip_prefix('/')
                        .unwrap_or("")
                        .to_lowercase()
                        .starts_with(&filter)
                }
            })
            .collect()
    }

    // ---------------------------------------------------------------------
    // In-TUI configuration wizard implementation (/config)
    // Provides scrollable menus for providers, bindings, roles + secret key entry.
    // ---------------------------------------------------------------------

    /// Start the interactive configuration wizard from the /config or /setup command.
    /// Active plan file for this project (defaults to plan.md; /new-plan re-points it).
    fn plan_path(&self) -> std::path::PathBuf {
        active_plan_path(&self.root)
    }

    /// /new-plan <name> — start a fresh, feature-named plan (sequential model). Archives
    /// the current plan + its REVIEW_* files under .anvil/plans/archive/, resets the gate
    /// state, and points the coder at <slug>_plan.md.
    fn start_new_plan(&mut self, raw_name: &str) {
        let cleaned = raw_name
            .trim()
            .trim_end_matches(".md")
            .trim_end_matches("_plan");
        let slug = slugify_plan_name(cleaned);
        if slug.is_empty() {
            self.push_system(
                "Usage: /new-plan <name>   (e.g. /new-plan frontpage  ->  frontpage_plan.md)",
            );
            return;
        }
        let filename = format!("{}_plan.md", slug);

        // Archive the current active plan (+ its reviews) if it exists on disk. Bail on
        // failure rather than risk clobbering the previous plan.
        match self.archive_current_plan() {
            Ok(Some(dir)) => {
                self.push_system(&format!("Archived the previous plan to {}", dir.display()));
            }
            Ok(None) => {}
            Err(e) => {
                self.push_system(&format!(
                    "Could not archive the previous plan ({e}). Aborting /new-plan to avoid clobbering it."
                ));
                return;
            }
        }

        // Point state at the new plan and reset the gate.
        let mut st = load_state(&self.root);
        st.active_plan = Some(filename.clone());
        st.current_phase = None;
        st.accepted_plan_hash = None;
        st.shipped_phases.clear();
        st.phase_base = None;
        if let Err(e) = save_state(&self.root, &st) {
            self.push_system(&format!("Failed to save state for the new plan: {e}"));
            return;
        }

        self.reconcile_stage_from_disk();
        self.update_status();
        self.push_system(&format!(
            "New plan: {filename}. Discuss what you want, then have the coder write the plan to {filename} and run /lock-plan."
        ));
    }

    /// Move the active plan file and its REVIEW_*.md files into
    /// .anvil/plans/archive/<stem>[_n]/. Returns the archive dir, or None if there was no
    /// plan file to archive.
    fn archive_current_plan(&self) -> std::io::Result<Option<std::path::PathBuf>> {
        let current = self.plan_path();
        if !current.exists() {
            return Ok(None);
        }
        let stem = current
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("plan")
            .to_string();
        let archive_base = self.root.join(".anvil").join("plans").join("archive");
        let mut dir = archive_base.join(&stem);
        let mut n = 2;
        while dir.exists() {
            dir = archive_base.join(format!("{stem}_{n}"));
            n += 1;
        }
        std::fs::create_dir_all(&dir)?;
        if let Some(fname) = current.file_name() {
            std::fs::rename(&current, dir.join(fname))?;
        }
        // Sweep REVIEW_*.md files at the repo root (they belong to the plan being archived).
        if let Ok(entries) = std::fs::read_dir(&self.root) {
            for entry in entries.flatten() {
                let p = entry.path();
                if let Some(fname) = p.file_name().and_then(|s| s.to_str()) {
                    if fname.starts_with("REVIEW_") && fname.ends_with(".md") {
                        let _ = std::fs::rename(&p, dir.join(fname));
                    }
                }
            }
        }
        Ok(Some(dir))
    }

    /// /plans — show the active plan and any archived ones.
    fn list_plans(&mut self) {
        let active = active_plan_name(&self.root);
        self.push_system(&format!("Active plan: {active}"));
        let archive_base = self.root.join(".anvil").join("plans").join("archive");
        let mut names: Vec<String> = std::fs::read_dir(&archive_base)
            .map(|entries| {
                entries
                    .flatten()
                    .filter_map(|e| e.file_name().into_string().ok())
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        if names.is_empty() {
            self.push_system("Archived plans: (none)");
        } else {
            self.push_system(&format!("Archived plans ({}):", names.len()));
            for n in names {
                self.push_system(&format!("  {n}"));
            }
        }
    }

    fn start_config_wizard(&mut self) {
        // Make sure we have a cfg to work with (may be empty on first real setup)
        if self.cfg.is_none() {
            self.cfg = load_config(&self.root).ok();
        }
        let w = ConfigWizard {
            step: WizardStep::MainMenu,
            list_items: vec![],
            list_selected: 0,
            list_title: String::new(),
            provider_type: None,
            provider_name: None,
            base_url: None,
            cred_kind: None,
            env_var: None,
            api_key: None,
            no_auth: false,
            model_options: vec![],
            binding_provider: None,
            model: None,
            note: None,
            current_role: None,
            ollama_model_list: vec![],
            swap_mode: false,
        };

        self.push_system("=== CONFIGURATION WIZARD ===");
        self.push_system("Scroll lists with ↑/↓, Enter to pick, Esc to go back/cancel, or type answers for text fields.");
        self.push_system(
            "All changes are saved to anvil.toml + keyring (when you choose keyring).",
        );

        if self.first_run {
            self.push_system("Welcome! A 60-second setup gets you chatting with a real model and using the full Talk → /plan (coder writes) → /lock-plan (R1 → coder fixes → /continue → R2 → coder fixes → /continue → summary) → /accept-plan flow, with the same sequential review loop per phase at /accept-phase → /ship-phase.");
            self.push_system("Tip: the top menu choice is the fastest on-ramp (local Ollama, zero secrets). Arrow to it and hit Enter.");
        }

        self.config_wizard = Some(w);
        self.populate_main_menu();
        self.update_status();
    }

    fn populate_main_menu(&mut self) {
        // Always probe (cached) so that after initial quick setup users can still easily
        // re-enter the live model picker to change which pulled tags are used for CODER/R1/R2.
        let ollama_here = self.is_ollama_available();

        if let Some(w) = &mut self.config_wizard {
            w.step = WizardStep::MainMenu;
            if self.first_run {
                if ollama_here {
                    w.list_items = vec![
                        "1. Quick local Ollama setup (pick models for CODER / R1 / R2 from live `ollama list`)".to_string(),
                        "2. Add / update a provider connection (Anthropic, OpenAI, xAI, Google, Azure, AWS, Groq, Gradient, local, custom, ...)".to_string(),
                        "3. Assign roles  (coder / reviewer-R1 / reviewer-R2)".to_string(),
                        "4. Show current configuration".to_string(),
                        "5. Finish & return to chat".to_string(),
                    ];
                    w.list_title = "First-time setup — ↑↓ to the top item then Enter (or just type 1). This is the simplest on-ramp.".to_string();
                } else {
                    w.list_items = vec![
                        "1. Add / update a provider connection (Anthropic, OpenAI, xAI, Google, Azure, AWS, Groq, Gradient, local, custom, ...)".to_string(),
                        "2. Assign roles  (coder / reviewer-R1 / reviewer-R2)".to_string(),
                        "3. Show current configuration".to_string(),
                        "4. Finish & return to chat".to_string(),
                    ];
                    w.list_title = "First-time setup — Ollama not detected on :11434. Pick 1 to add any provider (including local providers later).".to_string();
                }
                w.list_selected = 0;
            } else {
                let mut items = vec![];
                if ollama_here {
                    items.push("Quick local Ollama — re-pick / change models for CODER, R1, R2 (live from what you have pulled)".to_string());
                }
                items.extend([
                    "Add / update a provider connection".to_string(),
                    "Assign roles  (coder / reviewer-R1 / reviewer-R2)".to_string(),
                    "Show current configuration".to_string(),
                    "Finish & return to chat".to_string(),
                ]);
                w.list_items = items;
                w.list_selected = 0;
                w.list_title = if ollama_here {
                    "Config"
                } else {
                    "What would you like to do? (↑↓ Enter)"
                }
                .to_string();
            }
        }
        if self.first_run {
            self.push_system("Main menu — first-run mode. Pick the top item (or arrow + Enter).");
        } else {
            self.push_system("Main menu — use arrows then Enter, or type a number / keywords.");
        }
    }

    /// Called on Enter while a wizard is active (from handle_input or from list Enter in handle_key).
    fn advance_config_wizard(&mut self, answer: String) {
        // Snapshot the current list choice without holding a long borrow.
        let (_is_listy, chosen_from_list) = if let Some(w) = &self.config_wizard {
            let listy = matches!(
                w.step,
                WizardStep::MainMenu
                    | WizardStep::ProviderType
                    | WizardStep::CredentialKind
                    | WizardStep::BindingProvider
                    | WizardStep::ModelName
                    | WizardStep::RoleAssignment { .. }
                    | WizardStep::SwapRolePick
                    | WizardStep::QuickOllamaModelPick { .. }
            );
            let chosen = if listy && !w.list_items.is_empty() {
                w.list_items.get(w.list_selected).cloned()
            } else {
                None
            };
            (listy, chosen)
        } else {
            (false, None)
        };

        let effective = chosen_from_list.unwrap_or(answer);

        // We will mutate the wizard via short-lived if-let borrows inside the arms.
        match &self.config_wizard.as_ref().map(|w| w.step.clone()) {
            Some(WizardStep::MainMenu) => {
                let s = effective.trim().to_lowercase();
                // Quick local Ollama (live model list) — available on first run and later so users
                // can re-pick different tags for the roles without going through manual add+assign.
                if s.contains("quick local ollama") || s.contains("re-pick / change models") {
                    self.start_quick_ollama_setup();
                    // The picker steps will call finish_config_wizard() themselves after the third choice
                    // (or update the local-coder/local-r1/local-r2 bindings on re-runs).
                } else if (s == "1" || s == "2")
                    || s.contains("add / update a provider")
                    || s.contains("provider connection")
                    || s.contains("provider")
                {
                    // Covers first-run layouts (provider at 1 or 2 depending on Ollama presence).
                    // Uses distinctive phrases from the actual menu item labels (list selection gives "1. Add..." or "Add / update...").
                    self.start_add_provider();
                } else if s.starts_with("add") || s.contains("binding") || s.contains("model") {
                    // Hidden keyword access to full "add model for provider" flow (no longer listed in main menu).
                    self.start_add_binding(None);
                } else if (s == "2" || s == "3") || s.contains("role") || s.contains("assign") {
                    self.start_role_assignment();
                } else if (s == "3" || s == "4") || s.contains("show") || s.contains("current") {
                    self.show_current_config();
                    self.populate_main_menu();
                } else if (s == "4" || s == "5")
                    || s.contains("finish")
                    || s.contains("return")
                    || s.contains("done")
                {
                    self.finish_config_wizard();
                } else {
                    self.push_system(
                        "Please choose a number or use the arrow keys + Enter on the list.",
                    );
                }
            }

            Some(WizardStep::ProviderType) => {
                let selected = effective.trim();
                if selected.is_empty() {
                    return;
                }
                // Look up the named preset (display_name, suggested_name, type, url, needs_key)
                let preset = PROVIDER_PRESETS.iter().find(|p| p.0 == selected);
                let (ptype, suggested, url, needs_key) = preset
                    .map(|p| (p.2, p.1, p.3, p.4))
                    .unwrap_or(("openai_compat", "custom", "", true));

                if let Some(w) = &mut self.config_wizard {
                    w.provider_type = Some(ptype.to_string());
                    w.base_url = if url.is_empty() {
                        None
                    } else {
                        Some(url.to_string())
                    };
                    w.no_auth = !needs_key;
                    w.step = WizardStep::ProviderName;
                    w.list_items.clear();
                    w.list_title.clear();
                }
                // Pre-fill input with the suggested connection name so user can just press Enter
                self.set_input(suggested.to_string());

                let url_note = if url.is_empty() {
                    "provider default".to_string()
                } else {
                    url.to_string()
                };
                self.push_system(&format!(
                    "Provider: {}  (type={}, url={})",
                    selected, ptype, url_note
                ));
                self.push_system("Enter a name for this connection — press Enter to accept the suggestion, or type your own:");
            }

            Some(WizardStep::ProviderName) => {
                let name = effective.trim();
                if name.is_empty() {
                    return;
                }
                let current_url = if let Some(w) = &self.config_wizard {
                    w.base_url.clone().unwrap_or_default()
                } else {
                    String::new()
                };

                if let Some(w) = &mut self.config_wizard {
                    w.provider_name = Some(name.to_string());
                    w.step = WizardStep::BaseUrl;
                    w.list_items.clear();
                    w.list_title.clear();
                }
                // Pre-fill the base URL so user can just press Enter to accept
                self.set_input(current_url.clone());

                self.push_system(&format!("Connection name: '{}'.", name));
                if !current_url.is_empty() {
                    self.push_system("Base URL (pre-filled — press Enter to accept, or edit):");
                } else {
                    self.push_system("Enter the base URL for this provider (leave empty to use provider SDK default):");
                }
            }

            Some(WizardStep::BaseUrl) => {
                let url = effective.trim();
                let no_auth = if let Some(w) = &mut self.config_wizard {
                    if !url.is_empty() {
                        w.base_url = Some(url.to_string());
                    }
                    w.no_auth
                } else {
                    false
                };
                if no_auth {
                    // Local providers (Ollama, LM Studio, etc.) need no credential — finish directly.
                    if let Some(w) = &mut self.config_wizard {
                        w.cred_kind = Some("none".to_string());
                    }
                    self.finish_add_provider();
                } else {
                    self.start_credential_list();
                }
            }

            Some(WizardStep::CredentialKind) => {
                let kind = effective.to_lowercase();
                if (kind.contains("keyring") || kind == "3") && keyring_likely_unavailable() {
                    self.push_system("OS keyring needs a desktop secret service (D-Bus) that this headless host lacks. Falling back to an environment variable.");
                }
                if (kind.contains("keyring") || kind == "3") && !keyring_likely_unavailable() {
                    // Keyring is last / advanced because it has been unreliable for some users on Windows.
                    if let Some(w) = &mut self.config_wizard {
                        w.cred_kind = Some("keyring".to_string());
                        w.step = WizardStep::ApiKeySecret;
                        w.list_items.clear();
                        w.list_title.clear();
                    }
                    self.input_secret = true;
                    self.push_system(
                        "Using OS keyring (may not be readable on all Windows setups).",
                    );
                    self.push_system(
                        "Paste or type the API key / token now (input will be hidden):",
                    );
                } else if kind.contains("no auth") || kind.contains("none") || kind == "2" {
                    if let Some(w) = &mut self.config_wizard {
                        w.cred_kind = Some("none".to_string());
                        w.list_items.clear();
                        w.list_title.clear();
                    }
                    self.push_system("No authentication required for this provider.");
                    self.finish_add_provider();
                } else {
                    // Recommended path (1): env var. Route to secret paste so we can auto-capture the value,
                    // set it in the process env immediately, derive a conventional name, and print persistence help.
                    if let Some(w) = &mut self.config_wizard {
                        w.cred_kind = Some("env".to_string());
                        w.step = WizardStep::ApiKeySecret;
                        w.list_items.clear();
                        w.list_title.clear();
                    }
                    self.input_secret = true;
                    self.push_system(
                        "Using environment variable (auto-captured from the key you paste).",
                    );
                    self.push_system(
                        "Paste or type the API key / token now (input will be hidden):",
                    );
                }
            }

            Some(WizardStep::EnvVarName) => {
                let var = effective.trim();
                if var.is_empty() {
                    return;
                }
                if let Some(w) = &mut self.config_wizard {
                    w.env_var = Some(var.to_string());
                    w.list_items.clear();
                    w.list_title.clear();
                }
                self.finish_add_provider();
            }

            Some(WizardStep::ApiKeySecret) => {
                let key = effective;
                if key.is_empty() {
                    return;
                }
                if let Some(w) = &mut self.config_wizard {
                    w.api_key = Some(key);
                    w.list_items.clear();
                    w.list_title.clear();
                }
                self.input_secret = false;
                self.finish_add_provider();
            }

            Some(WizardStep::BindingProvider) => {
                let prov = effective.trim();
                if prov.is_empty() {
                    return;
                }
                // Look up provider connection to derive known model IDs (before mutable borrow).
                // For local Ollama: live tags. For xAI / Groq / OpenAI / any openai_compat provider
                // that is already set up: live /models pull (so the picker shows the provider's real
                // current catalog rather than a hardcoded snapshot).
                let (ptype_for_live, base_for_live) = if let Some(cfg) = &self.cfg {
                    cfg.providers
                        .get(prov)
                        .map(|c| (c.r#type.clone(), c.base_url.clone()))
                        .unwrap_or_default()
                } else {
                    (String::new(), None)
                };
                let model_opts: Vec<String> = if !prov.is_empty() && !ptype_for_live.is_empty() {
                    self.live_or_static_models_for_provider(
                        prov,
                        &ptype_for_live,
                        base_for_live.as_deref(),
                    )
                } else {
                    vec![]
                };
                if let Some(w) = &mut self.config_wizard {
                    w.binding_provider = Some(prov.to_string());
                    w.model_options = model_opts.clone();
                    w.step = WizardStep::ModelName;
                    if !model_opts.is_empty() {
                        w.list_items = model_opts;
                        w.list_items.push("Other / type manually".to_string());
                        w.list_selected = 0;
                        w.list_title = "Select the model ID (↑↓ then Enter):".to_string();
                    } else {
                        w.list_items.clear();
                        w.list_title.clear();
                    }
                    w.model = None;
                    w.note = None;
                }
                self.push_system(&format!("Provider: '{}'.", prov));
                if self
                    .config_wizard
                    .as_ref()
                    .map(|w| !w.list_items.is_empty())
                    .unwrap_or(false)
                {
                    self.push_system(
                        "Select the model ID from the list, or choose 'Other / type manually':",
                    );
                } else {
                    self.push_system(
                        "Enter the model ID (e.g. grok-3, claude-sonnet-4-6, gpt-4o):",
                    );
                }
            }

            Some(WizardStep::ModelName) => {
                let model = effective.trim();
                if model.is_empty() {
                    return;
                }
                if model == "Other / type manually" {
                    if let Some(w) = &mut self.config_wizard {
                        w.list_items.clear();
                        w.list_title.clear();
                    }
                    self.input.clear();
                    self.push_system(
                        "Type the model ID (e.g. claude-sonnet-4-6, gpt-4o, llama3.1:8b):",
                    );
                    return;
                }
                if let Some(w) = &mut self.config_wizard {
                    w.model = Some(model.to_string());
                    w.step = WizardStep::BindingNote;
                    w.list_items.clear();
                    w.list_title.clear();
                }
                self.push_system(&format!("Model: {}", model));
                self.push_system("Optional short note (press Enter to skip):");
            }

            Some(WizardStep::BindingNote) => {
                let note = effective.trim();
                if let Some(w) = &mut self.config_wizard {
                    if !note.is_empty() {
                        w.note = Some(note.to_string());
                    }
                    w.list_items.clear();
                    w.list_title.clear();
                }
                self.finish_add_binding();
            }

            Some(WizardStep::RoleAssignment { role }) => {
                let picked = effective.trim().to_string();
                if picked.is_empty() {
                    return;
                }

                // Manual-entry sentinel: the provider's model list was empty (or the user
                // wants a model id not shown). Route to a free-text model-id prompt bound to
                // the provider encoded in the sentinel.
                if picked.contains(MANUAL_ENTRY_LABEL) {
                    let prov = self.extract_provider_for_choice(&picked);
                    let display_role = match role.as_str() {
                        "coder" => "coder",
                        "reviewer_a" => "reviewer-R1",
                        "reviewer_b" => "reviewer-R2",
                        other => other,
                    };
                    if let Some(w) = &mut self.config_wizard {
                        w.list_items.clear();
                        w.list_title.clear();
                        w.step = WizardStep::RoleManualModel {
                            role: role.clone(),
                            provider: prov.clone(),
                        };
                    }
                    self.input.clear();
                    self.push_system(&format!(
                        "Type the exact model ID for {} (provider '{}') — the slug exactly as your provider names it:",
                        display_role, prov
                    ));
                    return;
                }

                let (binding_name, prov, model) = self.parse_role_choice(&picked);
                self.assign_role_and_advance(role, binding_name, prov, model);
            }

            Some(WizardStep::RoleManualModel { role, provider }) => {
                let model = effective.trim().to_string();
                if model.is_empty() {
                    return;
                }
                if let Some(w) = &mut self.config_wizard {
                    w.list_items.clear();
                    w.list_title.clear();
                }
                self.push_system(&format!("Model: {}  (provider '{}')", model, provider));
                // Use the model id as the binding key (consistent with the list-pick path).
                self.assign_role_and_advance(role, model.clone(), provider.clone(), model);
            }

            Some(WizardStep::SwapRolePick) => {
                let choice = effective.trim().to_lowercase();
                let role = if choice.starts_with("coder") {
                    "coder"
                } else if choice.starts_with("reviewer-r1") {
                    "reviewer_a"
                } else if choice.starts_with("reviewer-r2") {
                    "reviewer_b"
                } else {
                    return;
                };
                // Reuse the standard model list (live models + per-provider manual entry).
                // swap_mode stays set, so assignment finishes after this one role.
                self.start_role_list(role);
            }

            Some(WizardStep::QuickOllamaModelPick { role }) => {
                let model = effective.trim();
                if model.is_empty() {
                    return;
                }
                if model == "Other / type manually" {
                    if let Some(w) = &mut self.config_wizard {
                        w.list_items.clear();
                        w.list_title.clear();
                    }
                    self.input.clear();
                    self.push_system(&format!(
                        "Type the exact Ollama model tag for {} (from `ollama list` or the picker above):",
                        role
                    ));
                    return;
                }
                // Create (or overwrite) a stable binding name for this role under the local-ollama provider.
                let binding_name = match role.as_str() {
                    "coder" => "local-coder".to_string(),
                    "reviewer_a" => "local-r1".to_string(),
                    "reviewer_b" => "local-r2".to_string(),
                    _ => format!("local-{}", role.replace('_', "-")),
                };

                {
                    let cfg = self.cfg.get_or_insert_with(AnvilConfig::default);
                    cfg.model_bindings.insert(
                        binding_name.clone(),
                        ModelBinding {
                            provider: "local-ollama".to_string(),
                            model: model.to_string(),
                            note: Some("quick-ollama".to_string()),
                            contract: None,
                        },
                    );
                    match role.as_str() {
                        "coder" => cfg.roles.coder = Some(binding_name.clone()),
                        "reviewer_a" => cfg.roles.reviewer_a = Some(binding_name.clone()),
                        "reviewer_b" => cfg.roles.reviewer_b = Some(binding_name.clone()),
                        _ => {}
                    }
                }

                save_global_config(self.cfg.as_ref().unwrap()).ok();
                // Re-pointing the coder must rebuild the cached agent (see
                // invalidate_agent) so the new model takes effect without a restart.
                if role == "coder" {
                    self.invalidate_agent();
                }
                self.reconcile_stage_from_disk();
                self.update_status();

                if role == "coder" {
                    self.enter_next_quick_pick("reviewer_a", "R1 (purple)");
                } else if role == "reviewer_a" {
                    self.enter_next_quick_pick("reviewer_b", "R2 (lime)");
                } else {
                    // All three chosen — finish the quick flow.
                    self.push_system("Quick setup complete!");
                    self.push_system("CODER (blue) • R1 (purple) • R2 (lime) are now assigned from your live Ollama models.");
                    self.push_system(
                        "Type to chat, or run /plan for the full R1+R2 gated workflow.",
                    );
                    self.finish_config_wizard();
                }
            }

            None => {
                self.finish_config_wizard();
            }
        }

        self.reconcile_stage_from_disk();
        self.update_status();
    }

    fn start_add_provider(&mut self) {
        if let Some(w) = &mut self.config_wizard {
            w.step = WizardStep::ProviderType;
            w.list_items = PROVIDER_PRESETS.iter().map(|p| p.0.to_string()).collect();
            w.list_selected = 0;
            w.list_title = "Choose your AI provider (↑↓ then Enter):".to_string();
            w.no_auth = false;
        }
        self.push_system("Adding a provider connection.");
        self.push_system("Select your provider from the list — base URL and connection type are pre-filled automatically.");
    }

    fn start_credential_list(&mut self) {
        let mut items = vec![
            "1. Environment variable (recommended — paste the key once; we auto-set e.g. XAI_API_KEY for this session + print persistence steps)".to_string(),
            "2. No authentication required (local Ollama, unauthenticated self-hosted, etc.)".to_string(),
        ];
        // Only offer the OS keyring where it can actually work. On a headless Linux host
        // there is no Secret Service, so keyring writes vanish — don't tempt the user with it.
        if !keyring_likely_unavailable() {
            items.push("3. OS keyring (advanced; known to be unreliable on some Windows Credential Manager setups — you may see 'No matching entry')".to_string());
        }
        if let Some(w) = &mut self.config_wizard {
            w.step = WizardStep::CredentialKind;
            w.list_items = items;
            w.list_selected = 0;
            w.list_title = "How will the API key be provided?".to_string();
        }
        self.push_system("Choose how the credential will be supplied for this provider.");
        if keyring_likely_unavailable() {
            self.push_system("(OS keyring is unavailable on this headless host. Use the environment variable option.)");
        }
    }

    fn finish_add_provider(&mut self) {
        let (ptype, name, base, cred_kind, api_key, env_var) = if let Some(w) = &self.config_wizard
        {
            (
                w.provider_type.clone().unwrap_or_default(),
                w.provider_name.clone().unwrap_or_default(),
                w.base_url.clone().filter(|s| !s.is_empty()),
                w.cred_kind.clone(),
                w.api_key.clone(),
                w.env_var.clone(),
            )
        } else {
            return;
        };

        if name.is_empty() {
            self.push_system("Provider name was empty — cancelling this provider.");
            self.populate_main_menu();
            return;
        }

        // When the user pastes a key we *always* go through the env path (std::env + CredentialRef::Env).
        // This is the only reliable cross-platform mechanism:
        //   - Works in PowerShell, cmd, bash, zsh, fish, WSL, Git Bash, etc.
        //   - Works in CI (GitHub Actions secrets, GitLab CI vars, etc.)
        //   - Works in Docker, systemd, launchd, etc. (just pass the var in the environment).
        //   - We also write the secret to .anvil/.env so that "future anvil runs from this
        //     project directory just work" with zero shell profile changes on any OS.
        //
        // The var *name* (e.g. XAI_API_KEY) is stored in anvil.toml. The secret value lives
        // only in the OS environment or the local .anvil/.env file.
        let (cred, auto_var) = if let Some(key) = &api_key {
            let var_name = suggest_env_var_name(&name, base.as_deref());

            // This does the set_var for the running process + writes/updates .anvil/.env
            // (and tries to chmod 600 on Unix). Future calls to load_local_env (done at the
            // top of run_ui, run_talk, run_plan, run_phase_*) will pick it up.
            set_local_env_var(&self.root, &var_name, key);

            // Best-effort dual write to keyring (backup only; we will use the Env ref below).
            if cred_kind.as_deref() == Some("keyring") {
                let entry_name = format!("provider:{}", name);
                if let Ok(entry) = keyring::Entry::new("anvil", &entry_name) {
                    if entry.set_password(key).is_ok() {
                        self.push_system("  (Also stored a copy in the OS keyring as a bonus.)");
                    }
                }
            }

            // Cross-platform explanation printed right after the user pastes the key.
            self.push_system(&format!(
                "✓ Key captured as environment variable {} (current session).",
                var_name
            ));
            self.push_system("  We also wrote it to .anvil/.env (plain text — keep the .anvil directory private).");
            self.push_system("  Any future `anvil` run from this project directory will auto-load it (no shell config required).");
            self.push_system("");
            self.push_system("  How this works everywhere (PowerShell, bash, Docker, CI, WSL, macOS, Linux servers...):");
            self.push_system(
                "    • The *runtime* (std::env::var + set_var) is the same on every OS and shell.",
            );
            self.push_system("    • .anvil/.env is loaded automatically by anvil on every start (TUI + all CLI commands).");
            self.push_system("    • For global use or when running anvil from other directories, set the variable");
            self.push_system("      in your normal environment:");
            self.push_system(&format!(
                "        Windows (PowerShell):  $env:{} = \"<key>\"     (or use setx)",
                var_name
            ));
            self.push_system(&format!(
                "        Windows (cmd):         set {}=\"<key>\"",
                var_name
            ));
            self.push_system(&format!(
                "        Linux / macOS / WSL / Git Bash:   export {}=\"<key>\"",
                var_name
            ));
            self.push_system(&format!(
                "        fish:                  set -x {} \"<key>\"",
                var_name
            ));
            self.push_system("    • CI / Docker / scripts / systemd: just make sure the variable is present in the");
            self.push_system("      environment of the process that executes `anvil` (GitHub secrets, -e flags, etc.).");
            self.push_system("    • The exact same variable names (XAI_API_KEY, OPENAI_API_KEY, ...) are used by");
            self.push_system("      many other tools, so you can often reuse existing secrets.");

            (
                CredentialRef::Env {
                    var_name: var_name.clone(),
                },
                Some(var_name),
            )
        } else if cred_kind.as_deref() == Some("keyring") {
            if let Some(key) = &api_key {
                let entry_name = format!("provider:{}", name);
                match keyring::Entry::new("anvil", &entry_name) {
                    Ok(entry) => {
                        if let Err(e) = entry.set_password(key) {
                            self.push_system(&format!(
                                "Warning: could not store key in keyring: {}",
                                e
                            ));
                        } else if entry.get_password().is_ok() {
                            self.push_system("✓ Key stored securely in OS keyring.");
                        } else {
                            self.push_system("✓ Key passed to OS keyring (readback not confirmed on this Windows setup).");
                        }
                    }
                    Err(e) => {
                        self.push_system(&format!("Warning: keyring unavailable ({}).", e));
                    }
                }
            }
            (CredentialRef::Keyring, None)
        } else if cred_kind.as_deref() == Some("none") {
            (CredentialRef::None, None)
        } else {
            (
                CredentialRef::Env {
                    var_name: env_var.unwrap_or_else(|| "API_KEY".to_string()),
                },
                None,
            )
        };

        let _auto_var = auto_var; // already explained in the messages above

        let normalized_type = if ptype.starts_with("openai_compat") {
            "openai_compat".to_string()
        } else if ptype.starts_with("azure") {
            "azure_openai".to_string()
        } else if ptype.starts_with("aws") {
            "aws_bedrock".to_string()
        } else {
            ptype.clone()
        };
        let is_remote_compat = normalized_type == "openai_compat"
            || normalized_type == "openai"
            || normalized_type.starts_with("azure");

        {
            let cfg = self.cfg.get_or_insert_with(AnvilConfig::default);

            let mut pc = ProviderConnection {
                r#type: normalized_type.clone(),
                base_url: base.clone(),
                credential: cred,
                extra: Default::default(),
                keep_alive: None,
            };
            // Default a friendly keep-alive for any local Ollama the user adds via the wizard.
            if let Some(b) = &pc.base_url {
                if b.contains("11434") || name.to_lowercase().contains("ollama") {
                    pc.keep_alive = Some("30s".to_string());
                }
            }
            cfg.providers.insert(name.clone(), pc);
        } // end the get_or_insert borrow

        self.save_current_config();
        // Reload so subsequent &self.cfg borrows in the probe (and any later wizard steps) are clean.
        self.cfg = load_config(&self.root).ok();
        self.push_system(&format!("✓ Provider '{}' saved.", name));

        // Proactively try the live /models fetch right after setup so the user gets
        // immediate feedback whether the dynamic list worked for this provider (especially xAI etc.).
        // For keyring (or env) we prefer the plaintext `api_key` captured from the wizard step
        // (the value the user just pasted/typed). This bypasses get_credential/keyring read for
        // the one-time post-add verification. Windows Credential Manager can report set success
        // while the entry is not yet visible to a get_password in the same process.
        // Subsequent "Assign roles" / live_or_static calls still use the normal stored credential path.
        if is_remote_compat {
            let b = base.as_deref().unwrap_or("").trim().to_string();
            let probe_key: Option<String> =
                if cred_kind.as_deref() == Some("keyring") || cred_kind.as_deref() == Some("env") {
                    api_key.clone().filter(|k| !k.trim().is_empty())
                } else {
                    None
                };
            let note = if let Some(key) = probe_key {
                // Use the just-entered key directly so the live list succeeds even if keyring readback is flaky right now.
                if let Some(rt) = &self.runtime {
                    match rt.block_on(self.llm.list_openai_compat_models(&b, &key)) {
                        Ok(models) if !models.is_empty() => {
                            let preview: Vec<String> = models.iter().take(3).cloned().collect();
                            format!("✓ Live model list for '{}': {} models. Examples: {}  (using just-entered key)", name, models.len(), preview.join(", "))
                        }
                        Ok(_) => {
                            format!("[models] '{}' live /models returned no results (or auth issue). Role/model pickers will use built-in suggestions.", name)
                        }
                        Err(e) => {
                            format!("[models] Error fetching live models for '{}': {} (using suggestions)", name, e)
                        }
                    }
                } else {
                    String::new()
                }
            } else if let Some(c) = &self.cfg {
                // No just-entered key available (e.g. pure env var flow); fall back to normal credential lookup.
                if let Some(conn) = c.providers.get(&name) {
                    let bb = conn.base_url.as_deref().unwrap_or(&b).trim().to_string();
                    if !bb.is_empty() {
                        match self.llm.get_credential(&name, conn) {
                            Ok(key) => {
                                if let Some(rt) = &self.runtime {
                                    match rt.block_on(self.llm.list_openai_compat_models(&bb, &key))
                                    {
                                        Ok(models) if !models.is_empty() => {
                                            let preview: Vec<String> =
                                                models.iter().take(3).cloned().collect();
                                            format!("✓ Live model list for '{}': {} models. Examples: {}", name, models.len(), preview.join(", "))
                                        }
                                        Ok(_) => {
                                            format!("[models] '{}' live /models returned no results (or auth issue). Role/model pickers will use built-in suggestions.", name)
                                        }
                                        Err(e) => {
                                            format!("[models] Error fetching live models for '{}': {} (using suggestions)", name, e)
                                        }
                                    }
                                } else {
                                    String::new()
                                }
                            }
                            Err(e) => {
                                format!("[models] Could not read credential for '{}' after add ({}). Live models unavailable.", name, e)
                            }
                        }
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                }
            } else {
                String::new()
            };
            if !note.is_empty() {
                self.push_system(&note);
            }
        }

        if self.first_run || !self.is_configured() {
            self.push_system("Provider ready — launching role assignment to pick coder / reviewer-R1 / reviewer-R2 from the live models.");
            self.start_role_assignment();
        } else {
            self.push_system("Provider ready. Use 'Assign roles' from the menu to pick from the live list (or built-in suggestions).");
            self.populate_main_menu();
        }
    }

    fn start_add_binding(&mut self, preselected_provider: Option<String>) {
        // Use shared view for the emptiness test so we don't hold a long-lived &mut borrow on .cfg
        // (which would conflict with later &self calls to live_or_static or &mut wizard).
        if self.cfg.as_ref().is_none_or(|c| c.providers.is_empty()) {
            // Match original side-effect: materialize a default cfg if there was none.
            let _ = self.cfg.get_or_insert_with(AnvilConfig::default);
            self.push_system("No providers configured yet. Add a provider first.");
            self.populate_main_menu();
            return;
        }

        if let Some(prov) = preselected_provider {
            // Provider is already known — go straight to model selection.
            // Use live fetch for local Ollama (same as the BindingProvider -> ModelName path)
            // so reconfig after quick setup can surface currently-pulled tags instead of [] .
            let (ptype_for_live, base_for_live) = if let Some(cfg) = &self.cfg {
                cfg.providers
                    .get(&prov)
                    .map(|c| (c.r#type.clone(), c.base_url.clone()))
                    .unwrap_or_default()
            } else {
                (String::new(), None)
            };
            let model_opts: Vec<String> = if !prov.is_empty() && !ptype_for_live.is_empty() {
                self.live_or_static_models_for_provider(
                    &prov,
                    &ptype_for_live,
                    base_for_live.as_deref(),
                )
            } else {
                vec![]
            };

            if let Some(w) = &mut self.config_wizard {
                w.binding_provider = Some(prov.clone());
                w.model_options = model_opts.clone();
                w.step = WizardStep::ModelName;
                if !model_opts.is_empty() {
                    w.list_items = model_opts;
                    w.list_items.push("Other / type manually".to_string());
                    w.list_selected = 0;
                    w.list_title = "Select the model ID (↑↓ then Enter):".to_string();
                } else {
                    w.list_items.clear();
                    w.list_title.clear();
                }
                w.model = None;
                w.note = None;
            }
            self.push_system(&format!("Adding a model for provider '{}'.", prov));
            if self
                .config_wizard
                .as_ref()
                .map(|w| !w.list_items.is_empty())
                .unwrap_or(false)
            {
                self.push_system(
                    "Select the model ID from the list, or choose 'Other / type manually':",
                );
            } else {
                self.push_system("Enter the model ID (e.g. grok-3, claude-sonnet-4-6, gpt-4o):");
            }
        } else {
            // Show the provider list for the user to choose from.
            let prov_names: Vec<String> = if let Some(cfg) = &self.cfg {
                cfg.providers.keys().cloned().collect()
            } else {
                vec![]
            };
            if let Some(w) = &mut self.config_wizard {
                w.step = WizardStep::BindingProvider;
                w.list_items = prov_names;
                w.list_selected = 0;
                w.list_title = "Which provider does this model use? (↑↓ then Enter)".to_string();
                w.binding_provider = None;
                w.model = None;
                w.note = None;
            }
            self.push_system("Adding a model.");
            self.push_system("Select the provider this model is accessed through:");
        }
    }

    fn finish_add_binding(&mut self) {
        let (prov, model, note) = if let Some(w) = &self.config_wizard {
            (
                w.binding_provider.clone().unwrap_or_default(),
                w.model.clone().unwrap_or_default(),
                w.note.clone(),
            )
        } else {
            return;
        };

        let cfg = self.cfg.get_or_insert_with(AnvilConfig::default);

        if model.is_empty() || prov.is_empty() {
            self.push_system("Incomplete — cancelling.");
            self.populate_main_menu();
            return;
        }

        cfg.model_bindings.insert(
            model.clone(),
            ModelBinding {
                provider: prov.clone(),
                model: model.clone(),
                note,
                contract: None,
            },
        );

        self.save_current_config();
        self.push_system(&format!(
            "✓ Model '{}' saved via provider '{}'.",
            model, prov
        ));
        self.push_system(
            "Use 'Assign roles' from the menu to assign it to coder / reviewer-R1 / reviewer-R2.",
        );

        self.populate_main_menu();
    }

    fn start_role_assignment(&mut self) {
        // Delegate to start_role_list. It now pulls live/static models from *all* configured
        // providers (grouped + color-coded in the UI) and falls back gracefully with a message
        // if nothing is available yet.
        self.start_role_list("coder");
    }

    /// /swap — hot-swap the model for a single role mid-workflow. Spins up a minimal
    /// wizard (swap_mode = true) that picks one role then re-points it, returning to chat
    /// without walking the full coder -> R1 -> R2 chain.
    fn start_role_swap(&mut self) {
        if self.cfg.is_none() {
            self.cfg = load_config(&self.root).ok();
        }
        if self.cfg.as_ref().is_none_or(|c| c.providers.is_empty()) {
            self.push_system(
                "No providers configured yet — run /config first, then /swap can re-point a role.",
            );
            return;
        }
        let (c, a, b) = self
            .cfg
            .as_ref()
            .map(|cfg| {
                (
                    cfg.roles
                        .coder
                        .clone()
                        .unwrap_or_else(|| "unset".to_string()),
                    cfg.roles
                        .reviewer_a
                        .clone()
                        .unwrap_or_else(|| "unset".to_string()),
                    cfg.roles
                        .reviewer_b
                        .clone()
                        .unwrap_or_else(|| "unset".to_string()),
                )
            })
            .unwrap_or_else(|| {
                (
                    "unset".to_string(),
                    "unset".to_string(),
                    "unset".to_string(),
                )
            });

        let w = ConfigWizard {
            step: WizardStep::SwapRolePick,
            list_items: vec![
                format!("coder       (now: {})", c),
                format!("reviewer-R1 (now: {})", a),
                format!("reviewer-R2 (now: {})", b),
            ],
            list_selected: 0,
            list_title: "Swap which role's model? (↑↓ then Enter)".to_string(),
            provider_type: None,
            provider_name: None,
            base_url: None,
            cred_kind: None,
            env_var: None,
            api_key: None,
            no_auth: false,
            model_options: vec![],
            binding_provider: None,
            model: None,
            note: None,
            current_role: None,
            ollama_model_list: vec![],
            swap_mode: true,
        };
        self.config_wizard = Some(w);
        self.push_system("=== SWAP MODEL ===");
        self.push_system("Pick the role to re-point, then choose a model from the list or '+ Enter a model ID manually'. Esc cancels.");
        self.update_status();
    }

    fn start_role_list(&mut self, role: &str) {
        let binding_names = self.build_available_bindings_for_roles();

        if binding_names.is_empty() {
            // build_available_bindings_for_roles always appends a manual-entry sentinel
            // per provider, so an empty list means no providers are configured at all.
            self.push_system("No providers configured yet — add one via Config / 'Add / update a provider connection', or use Quick local Ollama setup first.");
            self.populate_main_menu();
            return;
        }

        if let Some(w) = &mut self.config_wizard {
            w.step = WizardStep::RoleAssignment {
                role: role.to_string(),
            };
            w.list_items = binding_names;
            w.list_selected = 0;
            w.current_role = Some(role.to_string());
            w.list_title = match role {
                "coder" => "Choose model for coder".to_string(),
                "reviewer_a" => "Choose model for reviewer-R1".to_string(),
                "reviewer_b" => "Choose model for reviewer-R2".to_string(),
                other => format!("Choose model for {}", other),
            };
        }

        let role_desc = match role {
            "coder"      => "coder  (your primary model — used for chat, planning, and code)",
            "reviewer_a" => "reviewer-R1  (first independent review — use a different model than coder)",
            "reviewer_b" => "reviewer-R2  (second independent review — should be a DIFFERENT model than reviewer-R1)",
            other        => other,
        };
        self.push_system(&format!("Assigning role: {}", role_desc));
        self.push_system("Pick a model from the list, or choose '+ Enter a model ID manually' to type it (↑↓ then Enter):");
    }

    /// Auto-register a model binding if it doesn't exist, assign it to `role`, persist,
    /// and advance to the next role (or finish setup). Shared by the role-list pick path
    /// and the manual model-id entry path so both behave identically.
    /// Drop the cached coder agent (and its confirm channel) so the next turn
    /// rebuilds it with the current coder binding — new model/provider/api_key —
    /// instead of the one captured when it was first created. History reloads from
    /// the append-only ledger on rebuild, so the conversation continues seamlessly.
    fn invalidate_agent(&mut self) {
        self.agent = None;
        self.confirm_tx = None;
    }

    /// Where to drop a copied asset in a project: the first conventional static dir
    /// that exists, else the project root. Returns (absolute dir, relative label).
    fn project_asset_dir(root: &std::path::Path) -> (std::path::PathBuf, String) {
        for cand in ["public", "static", "assets", "src/assets"] {
            if root.join(cand).is_dir() {
                return (root.join(cand), cand.to_string());
            }
        }
        (root.to_path_buf(), ".".to_string())
    }

    /// `/tag` — stamp a "Built with Anvil" + badge footer on the current build.
    /// `/tag set <path.png>` stores the global badge; `/tag show` reports status;
    /// `/tag` copies the badge into the project and drives the coder to add the
    /// footer (the coder's tools are text-only, so Anvil does the binary copy).
    fn handle_tag(&mut self, arg: &str) {
        // /tag set <path-to-png>
        if let Some(rest) = arg.strip_prefix("set") {
            let path = rest.trim().trim_matches('"').trim_matches('\'').trim();
            if path.is_empty() {
                self.push_system(
                    "Usage: /tag set <path-to-png> — store a badge image to stamp on builds.",
                );
                return;
            }
            let src = std::path::Path::new(path);
            let Some(dest) = crate::config::global_badge_path() else {
                self.push_system(
                    "Could not resolve the global config directory to store the badge.",
                );
                return;
            };
            if !src.is_file() {
                self.push_system(&format!("No file at '{}'.", path));
                return;
            }
            let is_png = src
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("png"))
                .unwrap_or(false);
            if let Some(parent) = dest.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            match std::fs::copy(src, &dest) {
                Ok(bytes) => {
                    self.push_system(&format!(
                        "✓ Badge stored ({} bytes) — applies to every project.",
                        bytes
                    ));
                    if !is_png {
                        self.push_system(
                            "  (note: expected a .png — stored it anyway, but PNG is recommended.)",
                        );
                    }
                    self.push_system("  Run /tag in a project to add a 'Built with Anvil' footer with this badge.");
                }
                Err(e) => self.push_system(&format!("Could not store badge: {}", e)),
            }
            return;
        }

        let custom = crate::config::global_badge_path().filter(|p| p.is_file());

        // /tag show — status only.
        if arg == "show" {
            match &custom {
                Some(p) => {
                    self.push_system(&format!("Build-tag badge: custom badge set ({}).", p.display()))
                }
                None => self.push_system(
                    "Build-tag badge: using the bundled default. Override with /tag set <path-to-png>.",
                ),
            }
            return;
        }

        // /tag — apply to the current project. Badge bytes are the custom global
        // badge if set, otherwise the default bundled into the binary (tag.png).
        let badge_bytes: Vec<u8> = match &custom {
            Some(p) => match std::fs::read(p) {
                Ok(b) => b,
                Err(e) => {
                    self.push_system(&format!("Could not read the stored badge: {}", e));
                    return;
                }
            },
            None => DEFAULT_BADGE_PNG.to_vec(),
        };

        // Anvil writes the binary into the project (the coder can't — text tools only).
        let (asset_abs, asset_rel) = Self::project_asset_dir(&self.root);
        let _ = std::fs::create_dir_all(&asset_abs);
        let dest = asset_abs.join("anvil-badge.png");
        if let Err(e) = std::fs::write(&dest, &badge_bytes) {
            self.push_system(&format!(
                "Could not write the badge into the project: {}",
                e
            ));
            return;
        }
        let rel = if asset_rel == "." {
            "anvil-badge.png".to_string()
        } else {
            format!("{}/anvil-badge.png", asset_rel)
        };
        self.push_system(&format!(
            "Badge copied to {} — asking the coder to add the footer…",
            rel
        ));

        let task = format!(
            "The user ran /tag to tag this build with their badge. The badge image is already in the project at `{rel}`. \
             FIRST ask the user to confirm they want a \"Built with Anvil\" footer added, and make NO changes until they confirm in their next message. \
             On confirm: add a footer to the site that shows the badge image (`{rel}`) AND a text link \"Built with Anvil\" pointing to https://github.com/ai-nhancement/Anvil (open in a new tab). \
             Integrate it the correct way for THIS project's stack — find the shared layout/footer so it appears site-wide; keep it small and unobtrusive. \
             If the user declines, delete `{rel}` and make no other changes."
        );
        self.start_real_chat(&task);
    }

    fn assign_role_and_advance(
        &mut self,
        role: &str,
        binding_name: String,
        prov: String,
        model: String,
    ) {
        let mut did_auto_register = false;
        if let Some(cfg) = &mut self.cfg {
            if !cfg.model_bindings.contains_key(&binding_name) {
                if !cfg.providers.contains_key(&prov) {
                    // Safety net mirroring prior behavior: ensure a plausible local-ollama
                    // entry exists for the very first quick-ollama case.
                    if prov == "local-ollama" || !cfg.providers.contains_key("local-ollama") {
                        cfg.providers.insert(
                            "local-ollama".to_string(),
                            ProviderConnection {
                                r#type: "openai_compat".to_string(),
                                base_url: Some("http://localhost:11434/v1".to_string()),
                                credential: CredentialRef::None,
                                extra: Default::default(),
                                keep_alive: Some("30s".to_string()),
                            },
                        );
                    }
                }
                cfg.model_bindings.insert(
                    binding_name.clone(),
                    ModelBinding {
                        provider: prov.clone(),
                        model: model.clone(),
                        note: Some("from role assignment".to_string()),
                        contract: None,
                    },
                );
                did_auto_register = true;
            }

            match role {
                "coder" => cfg.roles.coder = Some(binding_name.clone()),
                "reviewer_a" => cfg.roles.reviewer_a = Some(binding_name.clone()),
                "reviewer_b" => cfg.roles.reviewer_b = Some(binding_name.clone()),
                _ => {}
            }
        }

        // Borrows on cfg have ended; safe to call other &mut self methods now.
        self.save_current_config();
        // The coder Agent is built once and cached, holding its own model/provider/
        // api_key from construction. A /swap or /config change to the coder must
        // invalidate it, or the next turn keeps calling the OLD provider until the
        // user restarts Anvil. The rebuilt agent reloads history from the ledger, so
        // continuity is preserved. (Reviewers are constructed fresh per review, so
        // they already pick up changes.)
        if role == "coder" {
            self.invalidate_agent();
        }
        if did_auto_register {
            self.push_system(&format!(
                "✓ Auto-registered model binding '{}' via {}.",
                binding_name, prov
            ));
        }
        let display_role = match role {
            "coder" => "coder",
            "reviewer_a" => "reviewer-R1",
            "reviewer_b" => "reviewer-R2",
            _ => role,
        };
        self.push_system(&format!("Set {} → {}", display_role, binding_name));

        // /swap flow: a single role was re-pointed. Return to the workflow instead of
        // walking the coder -> R1 -> R2 setup chain.
        if self
            .config_wizard
            .as_ref()
            .map(|w| w.swap_mode)
            .unwrap_or(false)
        {
            self.push_system("Model swapped. It's live for that role now. Back to the workflow.");
            self.config_wizard = None;
            self.input_secret = false;
            self.reconcile_stage_from_disk();
            self.update_status();
            return;
        }

        let next_role = match role {
            "coder" => Some("reviewer_a".to_string()),
            "reviewer_a" => Some("reviewer_b".to_string()),
            "reviewer_b" => None,
            _ => None,
        };

        if let Some(next) = next_role {
            self.start_role_list(&next);
        } else {
            self.save_current_config();
            self.push_system("All roles assigned and saved.");
            if self.is_configured() {
                let was_first = self.first_run;
                self.first_run = false;
                if was_first {
                    self.push_system("First-time setup complete!");
                    self.push_system("Just type to chat with the coder — it reads, edits, and runs the project directly.");
                    self.push_system("Plan gate: coder writes plan.md → /lock-plan → R1 → fix → /continue → R2 → fix → /continue → summary → /accept-plan. Phase gate: build → /accept-phase (same loop on the diff) → /ship-phase.");
                    self.push_system("This is the lightweight structure that keeps vibe coding from drifting — valuable for beginners and hardcore users alike.");
                    // A brand-new user just configured — bootstrap git now so their
                    // first phase/review has a real diff (boot skipped this while
                    // unconfigured).
                    crate::agent::ensure_context_files(&self.root);
                    self.bootstrap_git_repo();
                }
            }
            self.populate_main_menu();
        }
    }

    fn go_back_in_wizard(&mut self) {
        // Snapshot the current step using an immutable borrow first. This lets us
        // safely call &mut self methods (like populate_main_menu / finish) in the
        // decision arms without holding a long-lived &mut ConfigWizard.
        let current_step = if let Some(w) = &self.config_wizard {
            w.step.clone()
        } else {
            return;
        };

        // In a /swap flow, Esc/back simply cancels and returns to chat (no config menu).
        if self
            .config_wizard
            .as_ref()
            .map(|w| w.swap_mode)
            .unwrap_or(false)
        {
            self.config_wizard = None;
            self.input_secret = false;
            self.push_system("(swap cancelled)");
            self.update_status();
            return;
        }

        if matches!(current_step, WizardStep::MainMenu) {
            self.finish_config_wizard();
            return;
        }

        // Determine the logical previous step for the current flow.
        let prev = match &current_step {
            WizardStep::ProviderName => WizardStep::ProviderType,
            WizardStep::BaseUrl => WizardStep::ProviderName,
            WizardStep::CredentialKind => WizardStep::BaseUrl,
            WizardStep::EnvVarName | WizardStep::ApiKeySecret => WizardStep::CredentialKind,
            WizardStep::ModelName => WizardStep::BindingProvider,
            WizardStep::BindingNote => WizardStep::ModelName,
            WizardStep::RoleManualModel { role, .. } => {
                WizardStep::RoleAssignment { role: role.clone() }
            }
            WizardStep::RoleAssignment { role } => {
                match role.as_str() {
                    "reviewer_a" => WizardStep::RoleAssignment {
                        role: "coder".to_string(),
                    },
                    "reviewer_b" => WizardStep::RoleAssignment {
                        role: "reviewer_a".to_string(),
                    },
                    _ => {
                        // Backing from "coder" role assignment (or unknown) goes to main menu
                        self.populate_main_menu();
                        self.push_system("(back)");
                        return;
                    }
                }
            }
            WizardStep::ProviderType | WizardStep::BindingProvider => {
                // Backing out of the first step of a provider or binding flow
                self.populate_main_menu();
                self.push_system("(back)");
                return;
            }
            WizardStep::QuickOllamaModelPick { role } => match role.as_str() {
                "reviewer_b" => WizardStep::QuickOllamaModelPick {
                    role: "reviewer_a".to_string(),
                },
                "reviewer_a" => WizardStep::QuickOllamaModelPick {
                    role: "coder".to_string(),
                },
                _ => {
                    self.populate_main_menu();
                    self.push_system("(back)");
                    return;
                }
            },
            _ => {
                self.populate_main_menu();
                self.push_system("(back)");
                return;
            }
        };

        // Snapshot role list (with live models) *before* taking the long &mut borrow on .config_wizard.
        // The build now performs &mut self live fetches (for the per-provider /models calls), so
        // we do the snapshot early while no wizard state is mutably borrowed.
        let role_list_items: Option<Vec<String>> =
            if matches!(prev, WizardStep::RoleAssignment { .. }) {
                Some(self.build_available_bindings_for_roles())
            } else {
                None
            };

        // Now take a short-lived mutable borrow to apply the back step + update lists/input state.
        if let Some(w) = &mut self.config_wizard {
            w.step = prev;

            // Rebuild the list (if any) for the step we just moved to. We do this silently
            // (no "Adding ..." progress messages that the start_* helpers emit on forward entry).
            match &w.step {
                WizardStep::ProviderType => {
                    w.list_items = PROVIDER_PRESETS.iter().map(|p| p.0.to_string()).collect();
                    w.list_selected = 0;
                    w.list_title = "Choose your AI provider (↑↓ then Enter):".to_string();
                }
                WizardStep::CredentialKind => {
                    w.list_items = vec![
                        "1. Store in OS keyring (recommended — secure, works everywhere)".to_string(),
                        "2. Environment variable (you will set the var yourself)".to_string(),
                        "3. No authentication required (local Ollama, unauthenticated self-hosted, etc.)".to_string(),
                    ];
                    w.list_selected = 0;
                    w.list_title = "How will the API key be provided?".to_string();
                }
                WizardStep::BindingProvider => {
                    let cfg = self.cfg.get_or_insert_with(AnvilConfig::default);
                    w.list_items = cfg.providers.keys().cloned().collect();
                    w.list_selected = 0;
                    w.list_title = "Which provider connection should this binding use?".to_string();
                }
                WizardStep::ModelName => {
                    if !w.model_options.is_empty() {
                        w.list_items = w.model_options.clone();
                        w.list_items.push("Other / type manually".to_string());
                        w.list_selected = 0;
                        w.list_title =
                            "Select a model ID (↑↓ then Enter, or choose 'Other' to type):"
                                .to_string();
                    } else {
                        w.list_items.clear();
                        w.list_title.clear();
                    }
                }
                WizardStep::RoleAssignment { role } => {
                    // Use the pre-snapshot list (includes live tags) so backing up still offers
                    // the full set of models for re-assigning roles.
                    if let Some(items) = &role_list_items {
                        w.list_items = items.clone();
                    } else {
                        w.list_items = vec![];
                    }
                    w.list_selected = 0;
                    w.current_role = Some(role.clone());
                    w.list_title = match role.as_str() {
                        "coder" => "Choose model for coder".to_string(),
                        "reviewer_a" => "Choose model for reviewer-R1".to_string(),
                        "reviewer_b" => "Choose model for reviewer-R2".to_string(),
                        other => format!("Choose model for {}", other),
                    };
                }
                WizardStep::QuickOllamaModelPick { role } => {
                    w.list_items = w.ollama_model_list.clone();
                    if !w.list_items.is_empty() {
                        w.list_items.push("Other / type manually".to_string());
                    }
                    w.list_selected = 0;
                    let display = match role.as_str() {
                        "coder" => "CODER (blue)",
                        "reviewer_a" => "R1 (purple)",
                        "reviewer_b" => "R2 (lime)",
                        _ => role,
                    };
                    w.list_title = format!("Quick local Ollama — pick model for {}:", display);
                }
                _ => {
                    // Text steps or anything else: ensure no stale list popup
                    w.list_items.clear();
                    w.list_title.clear();
                }
            }

            // Prepare the input buffer + secret flag for the step we landed on.
            // Text steps get their prior value pre-filled so the user can re-Enter or edit it.
            // List steps get input cleared (they are driven by arrows + Enter on the visible list).
            self.input = String::new();
            self.input_secret = false;

            match &w.step {
                WizardStep::ProviderName => {
                    self.input = w.provider_name.clone().unwrap_or_default();
                }
                WizardStep::BaseUrl => {
                    self.input = w.base_url.clone().unwrap_or_default();
                }
                WizardStep::EnvVarName => {
                    self.input = w.env_var.clone().unwrap_or_default();
                }
                WizardStep::ApiKeySecret => {
                    self.input = w.api_key.clone().unwrap_or_default();
                    self.input_secret = true;
                }
                WizardStep::ModelName => {
                    self.input = w.model.clone().unwrap_or_default();
                }
                WizardStep::BindingNote => {
                    self.input = w.note.clone().unwrap_or_default();
                }
                WizardStep::QuickOllamaModelPick { .. } => {
                    // Pure list step (models already in w.list_items); input stays cleared.
                }
                _ => {}
            }

            // Best-effort: when landing on a list step, highlight the item that corresponds to a
            // previously made choice (if any) instead of always starting at index 0.
            match &w.step {
                WizardStep::ProviderType => {
                    if let Some(pt) = &w.provider_type {
                        if let Some(idx) = w.list_items.iter().position(|s| {
                            s.to_lowercase().starts_with(&pt.to_lowercase()) || s.contains(pt)
                        }) {
                            w.list_selected = idx;
                        }
                    }
                }
                WizardStep::CredentialKind => {
                    if let Some(k) = &w.cred_kind {
                        w.list_selected = match k.as_str() {
                            "keyring" => 0,
                            "env" => 1,
                            "none" => 2,
                            _ => 0,
                        };
                    }
                }
                WizardStep::QuickOllamaModelPick { .. } => {
                    // Fresh list each time we enter/back into a pick; 0 is fine (or could remember last choice per role).
                }
                WizardStep::BindingProvider => {
                    if let Some(bp) = &w.binding_provider {
                        if let Some(idx) = w.list_items.iter().position(|s| s == bp) {
                            w.list_selected = idx;
                        }
                    }
                }
                WizardStep::RoleAssignment { role } => {
                    if let Some(cfg) = &self.cfg {
                        let assigned = match role.as_str() {
                            "coder" => &cfg.roles.coder,
                            "planner" => &cfg.roles.planner,
                            "reviewer_a" => &cfg.roles.reviewer_a,
                            "reviewer_b" => &cfg.roles.reviewer_b,
                            _ => &None,
                        };
                        if let Some(name) = assigned {
                            if let Some(idx) = w.list_items.iter().position(|s| {
                                s == name
                                    || s.starts_with(name)
                                    || s.starts_with(&format!("{}  [", name))
                                    || s.contains(name)
                            }) {
                                w.list_selected = idx;
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        // Park the edit cursor at the end of whatever the step pre-filled (0 if cleared).
        self.input_cursor = self.input.len();
        self.push_system("(back)");
    }

    fn show_current_config(&mut self) {
        let lines = if let Some(cfg) = &self.cfg {
            let mut out = vec![
                "--- Current Configuration ---".to_string(),
                format!(
                    "Roles: coder={} reviewer-R1={} reviewer-R2={}",
                    cfg.roles.coder.as_deref().unwrap_or("(none)"),
                    cfg.roles.reviewer_a.as_deref().unwrap_or("(none)"),
                    cfg.roles.reviewer_b.as_deref().unwrap_or("(none)")
                ),
            ];
            out.push("Providers:".to_string());
            for (name, p) in &cfg.providers {
                let base = p.base_url.as_deref().unwrap_or("<default>");
                let auth = match &p.credential {
                    CredentialRef::None => "auth=none".to_string(),
                    CredentialRef::Keyring => "auth=keyring".to_string(),
                    CredentialRef::Env { var_name } => format!("auth=env:{}", var_name),
                };
                let ka = p
                    .keep_alive
                    .as_deref()
                    .map(|k| format!(" keep_alive={}", k))
                    .unwrap_or_default();
                out.push(format!(
                    "  {} (type={}, base={}, {}{})",
                    name, p.r#type, base, auth, ka
                ));
            }
            out.push("Model Bindings:".to_string());
            for (name, b) in &cfg.model_bindings {
                let note = b
                    .note
                    .as_deref()
                    .map(|n| format!(" ({})", n))
                    .unwrap_or_default();
                out.push(format!(
                    "  {} → {} via {}{}",
                    name, b.model, b.provider, note
                ));
            }
            out.push("-----------------------------".to_string());
            out
        } else {
            vec!["No configuration loaded yet.".to_string()]
        };

        for line in lines {
            self.push_system(&line);
        }
    }

    fn finish_config_wizard(&mut self) {
        self.save_current_config();

        self.cfg = load_config(&self.root).ok();

        // If this exit from the wizard is what made the user fully configured for the
        // very first time, give the smooth "ready to code" onboarding message.
        if self.is_configured() {
            let was_first = self.first_run;
            self.first_run = false;
            if was_first {
                self.push_system("Setup complete! Just type to chat with the coder — it reads, edits, and runs the project directly.");
                self.push_system("Plan gate: coder writes plan.md → /lock-plan → R1 → fix → /continue → R2 → fix → /continue → summary → /accept-plan. Phase gate: build → /accept-phase (same loop) → /ship-phase.");
                self.push_system("The workflow is deliberately simple to start yet powerful enough for serious use: structure that prevents drift without killing velocity.");
                crate::agent::ensure_context_files(&self.root);
                self.bootstrap_git_repo();
            } else {
                self.push_system("Configuration wizard finished. Changes saved to anvil.toml (and keyring where used).");
                self.push_system("You can now chat with the coder and run /plan for the gate.");
            }
        } else {
            self.push_system("Configuration wizard finished. Changes saved to anvil.toml (and keyring where used).");
            self.push_system("Reviewers are still missing — use /config again or 's' for quick Ollama setup before running /plan.");
        }

        self.config_wizard = None;
        self.input_secret = false;
        self.showing_command_palette = false;

        self.reconcile_stage_from_disk();
        self.update_status();
    }

    fn save_current_config(&mut self) {
        // Setup writes to the GLOBAL config so providers/models are shared across
        // every repo (per-repo anvil.toml overrides remain a manual opt-in).
        if let Some(cfg) = &self.cfg {
            if let Err(e) = save_global_config(cfg) {
                self.push_system(&format!("Warning: could not save global config: {}", e));
            }
        }
    }
}

/// The main event/draw loop. Returns on quit or error.
fn run_app_loop<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> Result<()> {
    loop {
        // Drain live LLM chat tokens + any plan-gate completion signals.
        // Non-blocking so the TUI stays responsive during long planner/reviewer calls.
        let chat = app.drain_llm_stream();
        let gate = app.drain_gate_events();
        app.drain_update_events();
        app.anim_tick = app.anim_tick.wrapping_add(1);

        // Live GPU stats ~every 2 seconds (80ms * 25). Cheap and useful for local models.
        if app.anim_tick.is_multiple_of(25) {
            app.refresh_gpu_stats();
        }

        // Stoke / cool the forge based on agent activity (and GPU load).
        app.update_forge_heat(chat || gate);

        terminal.draw(|f| render_ui(f, app))?;

        if event::poll(std::time::Duration::from_millis(80))? {
            // handle_key() has side effects and uses `?`, so it can't fold into a
            // match guard — keep the inner `if` despite clippy::collapsible_match.
            #[allow(clippy::collapsible_match)]
            match event::read()? {
                Event::Key(key)
                    if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) =>
                {
                    if handle_key(app, key)? {
                        break;
                    }
                }
                Event::Paste(text) => {
                    // Paste arrives as a single string. Small pastes go in inline; large ones are
                    // collapsed to a placeholder (handle_paste). Either way no per-char processing,
                    // which avoids crashes from escape sequences inside bracketed paste streams.
                    app.handle_paste(text);
                    if !app.input.starts_with('/') || app.config_wizard.is_some() {
                        app.showing_command_palette = false;
                    }
                }
                _ => {}
            }
        }

        if app.should_quit {
            break;
        }
    }
    Ok(())
}

fn handle_key(app: &mut App, key: event::KeyEvent) -> Result<bool> {
    // Any keypress dismisses the splash screen — consume the key without further processing.
    if app.splash_ticks > 0 {
        app.splash_ticks = 0;
        return Ok(false);
    }

    // Command palette navigation (when user pressed / and palette is visible).
    // Arrows move selection, Enter picks (replaces input with full command and executes),
    // Esc closes without executing, other keys (letters) fall through to live filter.
    if app.showing_command_palette {
        let filtered = app.filtered_commands();
        match key.code {
            KeyCode::Up => {
                if !filtered.is_empty() {
                    app.command_selected = app.command_selected.saturating_sub(1);
                }
                return Ok(false);
            }
            KeyCode::Down => {
                if !filtered.is_empty() {
                    app.command_selected =
                        (app.command_selected + 1).min(filtered.len().saturating_sub(1));
                }
                return Ok(false);
            }
            KeyCode::Enter => {
                if let Some((cmd, _)) = filtered.get(app.command_selected) {
                    // Insert only the runnable command, not the usage hint shown in
                    // the palette (e.g. "/review [--deep] [label]" → "/review"). The
                    // bracketed tokens are placeholders; the user types real args.
                    app.input = command_token(cmd);
                }
                app.showing_command_palette = false;
                app.handle_input();
                return Ok(false);
            }
            KeyCode::Esc => {
                app.showing_command_palette = false;
                // Leave any partial "/foo" in the input so the user can edit or backspace.
                return Ok(false);
            }
            _ => {}
        }
    }

    // /approvals checklist editor: a modal popup. ↑/↓ move; Space toggles the
    // highlighted row (when not mid-typing); typing builds a custom prefix and
    // Enter adds it (Enter with empty input toggles instead); Esc saves & closes.
    // All keys are consumed while it's open.
    if app.approvals_editor.is_some() {
        match key.code {
            KeyCode::Esc => {
                app.input.clear();
                app.approvals_save_and_close();
                return Ok(false);
            }
            KeyCode::Up => {
                if let Some(ed) = &mut app.approvals_editor {
                    ed.selected = ed.selected.saturating_sub(1);
                }
                return Ok(false);
            }
            KeyCode::Down => {
                if let Some(ed) = &mut app.approvals_editor {
                    let n = ed.items.len();
                    ed.selected = (ed.selected + 1).min(n.saturating_sub(1));
                }
                return Ok(false);
            }
            KeyCode::Enter => {
                if app.input.trim().is_empty() {
                    app.approvals_toggle_selected();
                } else {
                    let v = app.input.clone();
                    app.approvals_add_custom(&v);
                    app.input.clear();
                }
                return Ok(false);
            }
            KeyCode::Char(' ') if app.input.is_empty() => {
                app.approvals_toggle_selected();
                return Ok(false);
            }
            KeyCode::Char(c) => {
                app.input.push(c);
                return Ok(false);
            }
            KeyCode::Backspace => {
                app.input.pop();
                return Ok(false);
            }
            _ => return Ok(false),
        }
    }

    // Wizard navigation (when /config or /setup is active).
    // Esc always goes back one step / exits the config menu (never quits the whole TUI while wizard is open).
    // For list steps (main menu, provider/cred choices, bindings, roles) arrows + Enter also work.
    if let Some(wizard) = &mut app.config_wizard {
        if key.code == KeyCode::Esc {
            app.go_back_in_wizard();
            return Ok(false);
        }

        let is_list_step = matches!(
            wizard.step,
            WizardStep::MainMenu
                | WizardStep::ProviderType
                | WizardStep::CredentialKind
                | WizardStep::BindingProvider
                | WizardStep::ModelName
                | WizardStep::RoleAssignment { .. }
                | WizardStep::QuickOllamaModelPick { .. }
                | WizardStep::SwapRolePick
        );
        if is_list_step && !wizard.list_items.is_empty() {
            match key.code {
                KeyCode::Up => {
                    wizard.list_selected = wizard.list_selected.saturating_sub(1);
                    return Ok(false);
                }
                KeyCode::Down => {
                    wizard.list_selected =
                        (wizard.list_selected + 1).min(wizard.list_items.len().saturating_sub(1));
                    return Ok(false);
                }
                KeyCode::Enter => {
                    // Submit the highlighted choice as the "answer" for the current step.
                    // advance_config_wizard will pick the list item (or the input).
                    if let Some(choice) = wizard.list_items.get(wizard.list_selected) {
                        app.input = choice.clone();
                    }
                    app.handle_input(); // will route into advance_config_wizard
                    return Ok(false);
                }
                _ => {}
            }
        }
    }

    // Document viewer ( /view-plan /view-reviews "card" popups for deliberate plan + R1/R2 review before approve).
    // Esc closes the card without quitting the TUI (consistent with wizard and palette).
    if app.viewing_doc.is_some() {
        match key.code {
            KeyCode::Esc => {
                app.viewing_doc = None;
                app.doc_scroll = 0;
                return Ok(false);
            }
            KeyCode::Up => {
                app.doc_scroll = app.doc_scroll.saturating_sub(1);
                return Ok(false);
            }
            KeyCode::Down => {
                app.doc_scroll = app.doc_scroll.saturating_add(1);
                return Ok(false);
            }
            KeyCode::PageUp => {
                app.doc_scroll = app.doc_scroll.saturating_sub(10);
                return Ok(false);
            }
            KeyCode::PageDown => {
                app.doc_scroll = app.doc_scroll.saturating_add(10);
                return Ok(false);
            }
            KeyCode::Home => {
                app.doc_scroll = 0;
                return Ok(false);
            }
            _ => {}
        }
        // Read-only display otherwise; the content is also in the main chat log.
        return Ok(false);
    }

    // Run-command confirm prompt: ↑/↓ move the choice, Enter (on an empty input)
    // picks it, Esc denies. Typing still flows through (so /y / /a / /n work).
    if app.awaiting_confirm.is_some() && app.config_wizard.is_none() {
        match key.code {
            KeyCode::Up => {
                app.confirm_selected = app.confirm_selected.saturating_sub(1);
                return Ok(false);
            }
            KeyCode::Down => {
                app.confirm_selected = (app.confirm_selected + 1).min(2);
                return Ok(false);
            }
            KeyCode::Enter if app.input.trim().is_empty() => {
                app.resolve_confirm(app.confirm_selected);
                return Ok(false);
            }
            KeyCode::Esc => {
                app.resolve_confirm(2);
                return Ok(false);
            }
            _ => {}
        }
    }

    match key.code {
        // Quit is Ctrl+X (or /q / :q). Ctrl+C is deliberately NOT bound — it's left
        // free so the terminal can use it to copy selected text from the chat. ESC
        // no longer quits either; it only closes popups / steps back in the wizard
        // (handled above this match). No bare single-letter quit, so a stray
        // keystroke on an empty line can never eject you.
        KeyCode::Char('x') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.should_quit = true;
            return Ok(true);
        }

        // Ctrl+S: quick local Ollama setup / re-pick of CODER / R1 / R2 models.
        // (Was a bare 's', which hijacked any sentence starting with 's' on an
        // empty line.) If a terminal swallows Ctrl+S for flow control, /config or
        // /setup do exactly the same thing.
        KeyCode::Char('s')
            if key.modifiers.contains(KeyModifiers::CONTROL) && app.config_wizard.is_none() =>
        {
            app.showing_command_palette = false;
            app.start_quick_ollama_setup();
            return Ok(false);
        }

        // Ctrl+B: break in — interrupt the coder mid-turn and take back control.
        KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.interrupt_agent();
            return Ok(false);
        }

        KeyCode::Enter => {
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                // Shift+Enter inserts a newline for multi-line input (the input box is several lines tall and auto-tails).
                app.input_insert('\n');
                // Keep palette closed unless this is starting a command (unlikely with shift).
                if !app.input.starts_with('/') {
                    app.showing_command_palette = false;
                }
                return Ok(false);
            }
            // Paste robustness: on terminals that don't deliver bracketed paste,
            // a pasted newline arrives as a plain Enter. If more terminal input is
            // already queued, this Enter is part of a paste burst — insert a newline
            // instead of submitting, so the whole block accumulates and only the
            // trailing (real) Enter sends it. A lone human Enter has nothing queued.
            if event::poll(std::time::Duration::from_millis(0)).unwrap_or(false) {
                app.input_insert('\n');
                if !app.input.starts_with('/') {
                    app.showing_command_palette = false;
                }
                return Ok(false);
            }
            app.handle_input();
            return Ok(false);
        }

        KeyCode::Char(ch) => {
            // Normal typing (ignore when modifiers are control etc for simplicity in skeleton)
            if !key.modifiers.contains(KeyModifiers::CONTROL) {
                app.input_insert(ch);
                // Live palette: open/filter when input starts with / ; close otherwise.
                if app.input.starts_with('/') {
                    app.showing_command_palette = true;
                    app.command_selected = 0;
                } else {
                    app.showing_command_palette = false;
                }
                if app.config_wizard.is_some() {
                    // Never open the / command palette while the config wizard is active
                    app.showing_command_palette = false;
                }
                // Clamp in case previous selection is now past the end of a narrowed list.
                let n = app.filtered_commands().len();
                if n > 0 {
                    app.command_selected = app.command_selected.min(n - 1);
                }
            }
            return Ok(false);
        }

        KeyCode::Backspace => {
            app.input_backspace();
            if !app.input.starts_with('/') || app.input.is_empty() {
                app.showing_command_palette = false;
            } else {
                app.command_selected = 0;
            }
            // Clamp selection against possibly shorter filtered list after backspace.
            let n = app.filtered_commands().len();
            if n > 0 {
                app.command_selected = app.command_selected.min(n - 1);
            }
            return Ok(false);
        }

        // Up/Down move between input lines when the input is multi-line; once at
        // the top/bottom input line (or when single-line) they scroll the chat.
        KeyCode::Up => {
            if !app.input_up() {
                app.scroll_up(1);
            }
            return Ok(false);
        }
        KeyCode::Down => {
            if !app.input_down() {
                app.scroll_down(1);
            }
            return Ok(false);
        }

        // Cursor navigation within the input. Ctrl jumps by word; plain by char.
        KeyCode::Left => {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                app.input_word_left();
            } else {
                app.input_left();
            }
            return Ok(false);
        }
        KeyCode::Right => {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                app.input_word_right();
            } else {
                app.input_right();
            }
            return Ok(false);
        }
        KeyCode::Home => {
            app.input_home();
            return Ok(false);
        }
        KeyCode::End => {
            app.input_end();
            return Ok(false);
        }
        KeyCode::Delete => {
            app.input_delete_forward();
            if !app.input.starts_with('/') || app.input.is_empty() {
                app.showing_command_palette = false;
            }
            return Ok(false);
        }

        KeyCode::PageUp => {
            app.scroll_up(10);
            return Ok(false);
        }
        KeyCode::PageDown => {
            app.scroll_down(10);
            return Ok(false);
        }

        _ => {}
    }
    Ok(false)
}

fn render_ui(f: &mut Frame, app: &mut App) {
    if app.splash_ticks > 0 {
        render_splash(f, app);
        return;
    }
    render_main(f, app);
}

// ─── Splash screen ────────────────────────────────────────────────────────────

/// Decode a PNG and render it into ratatui `Line`s using the Unicode half-block technique.
///
/// Each `▀` character encodes two vertical pixels: fg = top pixel, bg = bottom pixel.
/// Half-blocks at 2px-per-row cancel out the 2:1 terminal character aspect ratio, so the
/// image appears undistorted in any truecolor terminal (Windows Terminal, iTerm2, etc.).
///
/// Returns an empty Vec on any decode failure — caller falls back to ASCII art.
fn render_png_as_halfblocks(png_bytes: &[u8], max_cols: u16, max_rows: u16) -> Vec<Line<'static>> {
    let img = match image::load_from_memory(png_bytes) {
        Ok(i) => i,
        Err(_) => return vec![],
    };

    let orig_w = img.width();
    let orig_h = img.height();
    if orig_w == 0 || orig_h == 0 {
        return vec![];
    }

    // Scale to fit inside max_cols × (max_rows*2) pixel budget, preserving aspect ratio.
    // Half-blocks give 2 pixels per char row, so the pixel grid is max_cols wide × (max_rows*2) tall.
    let target_w = max_cols as u32;
    let target_h = (max_rows * 2) as u32;
    let scale = (target_w as f32 / orig_w as f32).min(target_h as f32 / orig_h as f32);
    let scaled_w = ((orig_w as f32 * scale).round() as u32).max(1);
    let scaled_h = ((orig_h as f32 * scale).round() as u32).max(1);

    let img = img.resize_exact(scaled_w, scaled_h, image::imageops::FilterType::Lanczos3);
    let img = img.to_rgba8();
    let (w, h) = img.dimensions();

    // Alpha-composite against the splash background colour rgb(10, 10, 20).
    let blend = |p: image::Rgba<u8>| -> (u8, u8, u8) {
        let a = p[3] as f32 / 255.0;
        (
            (p[0] as f32 * a + 10.0 * (1.0 - a)) as u8,
            (p[1] as f32 * a + 10.0 * (1.0 - a)) as u8,
            (p[2] as f32 * a + 20.0 * (1.0 - a)) as u8,
        )
    };

    let mut lines = Vec::new();
    for y in (0..h).step_by(2) {
        let mut spans = Vec::new();
        for x in 0..w {
            let top = *img.get_pixel(x, y);
            let bot = if y + 1 < h {
                *img.get_pixel(x, y + 1)
            } else {
                image::Rgba([10u8, 10, 20, 255])
            };
            let (tr, tg, tb) = blend(top);
            let (br, bg_c, bb) = blend(bot);
            spans.push(Span::styled(
                "▀".to_string(),
                Style::default()
                    .fg(Color::Rgb(tr, tg, tb))
                    .bg(Color::Rgb(br, bg_c, bb)),
            ));
        }
        lines.push(Line::from(spans));
    }
    lines
}

fn render_splash(f: &mut Frame, app: &App) {
    use ratatui::layout::Rect;

    let area = f.area();
    let bg = Block::default().style(Style::default().bg(Color::Rgb(10, 10, 20)));
    f.render_widget(bg, area);

    let mut lines: Vec<Line<'static>> = Vec::new();

    // PNG logo — fill the screen, reserving ~10 rows for tagline/version/hint below.
    // Make the PNG 1 column and 1 row smaller (in terminal cells) per request.
    let img_max_cols = area.width.saturating_sub(6).saturating_sub(1);
    let img_max_rows = area.height.saturating_sub(10).max(8).saturating_sub(1);
    let img_rows = render_png_as_halfblocks(LOGO_BYTES, img_max_cols, img_max_rows);
    if !img_rows.is_empty() {
        lines.extend(img_rows);
    } else {
        for row in SPLASH_ANVIL {
            lines.push(Line::from(Span::styled(
                row.to_string(),
                Style::default().fg(Color::Rgb(210, 120, 30)),
            )));
        }
    }

    lines.push(Line::from(Span::raw("".to_string())));

    lines.push(Line::from(Span::styled(
        "        Structure for vibe coding.        ".to_string(),
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::ITALIC),
    )));
    lines.push(Line::from(Span::styled(
        "  Talk  →  Plan  →  Code  →  Review  →  Ship  ".to_string(),
        Style::default().fg(Color::Rgb(150, 200, 255)),
    )));

    lines.push(Line::from(Span::raw("".to_string())));

    let ver_line = format!(
        "  v{}  —  Model-agnostic, Cross-provider  ",
        env!("CARGO_PKG_VERSION")
    );
    lines.push(Line::from(Span::styled(
        ver_line,
        Style::default().fg(Color::DarkGray),
    )));

    if let Some(cfg) = &app.cfg {
        let coder = splash_model_label(cfg, "coder");
        let r1 = splash_model_label(cfg, "reviewer-a");
        let r2 = splash_model_label(cfg, "reviewer-b");
        if coder != "—" || r1 != "—" {
            lines.push(Line::from(vec![
                Span::styled("  CODER ".to_string(), Style::default().fg(ROLE_CODER)),
                Span::styled(coder, Style::default().fg(Color::White)),
                Span::styled("   R1 ".to_string(), Style::default().fg(ROLE_R1)),
                Span::styled(r1, Style::default().fg(Color::White)),
                Span::styled("   R2 ".to_string(), Style::default().fg(ROLE_R2)),
                Span::styled(r2, Style::default().fg(Color::White)),
                Span::raw("  ".to_string()),
            ]));
        }
    }

    lines.push(Line::from(Span::raw("".to_string())));

    // Pulsing dismiss hint
    let hint_color = if (app.anim_tick / 6).is_multiple_of(2) {
        Color::DarkGray
    } else {
        Color::Gray
    };
    lines.push(Line::from(Span::styled(
        "           Press any key to continue…           ".to_string(),
        Style::default().fg(hint_color),
    )));

    let total_h = lines.len() as u16;
    let top_pad = area.height.saturating_sub(total_h) / 2;

    for (i, line) in lines.into_iter().enumerate() {
        let y = area.y + top_pad + i as u16;
        if y >= area.y + area.height {
            break;
        }
        // Center each line independently so short text lines don't inherit the image's left_pad.
        let line_w = line
            .spans
            .iter()
            .map(|s| s.content.chars().count())
            .sum::<usize>() as u16;
        let left_pad = area.width.saturating_sub(line_w) / 2;
        let row_area = Rect {
            x: area.x + left_pad,
            y,
            width: area.width.saturating_sub(left_pad),
            height: 1,
        };
        f.render_widget(Paragraph::new(line), row_area);
    }
}

// ─── Main UI ─────────────────────────────────────────────────────────────────

fn render_main(f: &mut Frame, app: &mut App) {
    let area = f.area();

    // Input box auto-grows with the number of (wrapped) input rows, up to a cap,
    // so multi-line input stays visible instead of dropping below a fixed box.
    let input_inner_w = area.width.saturating_sub(2).max(1);
    let input_rows = Paragraph::new(format!("{}▌", app.input_full_text()))
        .wrap(Wrap { trim: false })
        .line_count(input_inner_w)
        .clamp(1, 8) as u16;
    let input_h = input_rows + 2; // + top/bottom borders

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),       // bordered header — 5 info rows
            Constraint::Min(4),          // chat log
            Constraint::Length(input_h), // bordered input — grows with content (capped)
        ])
        .split(area);

    render_header(f, app, chunks[0]);
    render_chat(f, app, chunks[1]);
    render_input_box(f, app, chunks[2]);

    // Overlays rendered last so they float on top
    render_palette_popup(f, app, chunks[1]);
    render_confirm_popup(f, app, chunks[1]);
    render_wizard_popup(f, app, chunks[1]);
    render_doc_popup(f, app, chunks[1]);
    render_approvals_popup(f, app, chunks[1]);
}

// ─── Header (5-row info panel, top-right column used for per-GPU status) ──────

fn render_header(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    use ratatui::layout::Rect;

    // ── Row 0: brand • stage • streaming indicator • context badge ──
    let stage_text = if app.first_run || app.stage == WorkflowStage::Unconfigured {
        "UNCONFIGURED — /config or Ctrl+S".to_string()
    } else {
        match app.stage {
            WorkflowStage::Talk => {
                "TALK — build with the coder; it writes plan.md, then /lock-plan".to_string()
            }
            WorkflowStage::PlanReviewsComplete => {
                "PLAN REVIEWED (R1+R2) — /accept-plan to quench".to_string()
            }
            WorkflowStage::PlanAccepted => {
                "PLAN ACCEPTED — build phases; /accept-phase then /ship-phase".to_string()
            }
            WorkflowStage::Unconfigured => "UNCONFIGURED".to_string(),
        }
    };
    let stage_color = if app.first_run || app.stage == WorkflowStage::Unconfigured {
        Color::Red
    } else {
        match app.stage {
            WorkflowStage::Talk => Color::Yellow,
            WorkflowStage::PlanReviewsComplete => Color::Magenta,
            WorkflowStage::PlanAccepted => Color::LightGreen,
            WorkflowStage::Unconfigured => Color::Red,
        }
    };

    // Row 0 (top of header): stage on the left. The live "forging…/ready"
    // indicator now lives on the chat box's bottom-left border (render_chat),
    // right where the user is looking, so it's impossible to miss.
    let row0: Vec<Span<'static>> = vec![Span::styled(
        stage_text,
        Style::default()
            .fg(stage_color)
            .add_modifier(Modifier::BOLD),
    )];

    // ── Row 1: coder / R1 / R2 model labels ──
    let row1: Vec<Span<'static>> = if let Some(cfg) = &app.cfg {
        let coder = header_model_label(cfg, "coder");
        let r1 = header_model_label(cfg, "reviewer-a");
        let r2 = header_model_label(cfg, "reviewer-b");
        vec![
            Span::styled(
                " 🔨 CODER ".to_string(),
                Style::default().fg(ROLE_CODER).add_modifier(Modifier::BOLD),
            ),
            Span::styled(coder, Style::default().fg(Color::White)),
            Span::styled(
                "   🛡️ R1 ".to_string(),
                Style::default().fg(ROLE_R1).add_modifier(Modifier::BOLD),
            ),
            Span::styled(r1, Style::default().fg(Color::White)),
            Span::styled(
                "   ⚖️ R2 ".to_string(),
                Style::default().fg(ROLE_R2).add_modifier(Modifier::BOLD),
            ),
            Span::styled(r2, Style::default().fg(Color::White)),
        ]
    } else {
        vec![Span::styled(
            " Run /config or Ctrl+S for quick setup (Ollama if available)".to_string(),
            Style::default().fg(Color::Yellow),
        )]
    };

    // ── Row 2: project name + phase progress ──
    // Show the actual project directory name (e.g. "Anvil"), not a bare ".".
    let proj = project_display_name(&app.root);
    let phases = build_phase_progress(app);

    // Color any checkmarks inside the phases progress string green (e.g. P0✓).
    let phase_spans: Vec<Span<'static>> = if phases.contains('✓') {
        let mut spans: Vec<Span<'static>> = vec![];
        let base = Style::default().fg(Color::Gray);
        let green_check = Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD);
        let mut rest = phases.as_str();
        while let Some(idx) = rest.find('✓') {
            if idx > 0 {
                spans.push(Span::styled(rest[..idx].to_string(), base));
            }
            spans.push(Span::styled("✓".to_string(), green_check));
            rest = &rest[idx + '✓'.len_utf8()..];
        }
        if !rest.is_empty() {
            spans.push(Span::styled(rest.to_string(), base));
        }
        spans
    } else {
        vec![Span::styled(phases, Style::default().fg(Color::Gray))]
    };

    let row2: Vec<Span<'static>> = vec![
        Span::styled(
            " ⬡ ".to_string(),
            Style::default()
                .fg(FORGE_MOLTEN)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(proj, Style::default().fg(Color::Rgb(170, 200, 255))),
        Span::styled("  │  ".to_string(), Style::default().fg(Color::DarkGray)),
    ];
    // Append the (possibly multi-span) phase progress
    let mut row2 = row2;
    row2.extend(phase_spans);

    // Live review-gate status, just right of the phase progress. Clears itself
    // when the gate finishes (gate_header_status returns None). A forge spinner
    // animates while a step is active; a ⏸ shows while paused for the user.
    if let Some(status) = app.gate_header_status() {
        let style = Style::default()
            .fg(Color::Rgb(255, 170, 60))
            .add_modifier(Modifier::BOLD);
        let glyph = if app.gate_paused() {
            "⏸ ".to_string()
        } else {
            let sp = FORGE_SPINNER[(app.anim_tick as usize / 2) % FORGE_SPINNER.len()];
            format!("{} ", sp)
        };
        row2.push(Span::styled(
            "    ".to_string(),
            Style::default().fg(Color::DarkGray),
        ));
        row2.push(Span::styled(glyph, style));
        row2.push(Span::styled(status, style));
    }

    // Draw bordered block then overlay rows inside it. The border glows along
    // the forge heat scale (cold steel when idle → amber while forging).
    let mut block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(heat_color(app.forge_heat)))
        .title(Span::styled(
            format!(
                "Anvil v{}  🔥  forge: {} ",
                env!("CARGO_PKG_VERSION"),
                heat_name(app.forge_heat)
            ),
            Style::default()
                .fg(FORGE_MOLTEN)
                .add_modifier(Modifier::BOLD),
        ));

    // Self-update indicator, pinned to the header's bottom-right border. While an
    // update is being applied it reads "updating…"; otherwise, when the boot check
    // found a newer release, it pulses through warm colors to draw the eye.
    if app.update_in_progress {
        block = block.title_bottom(
            Line::from(Span::styled(
                " ⬇ updating… ".to_string(),
                Style::default()
                    .fg(FORGE_MOLTEN)
                    .add_modifier(Modifier::BOLD),
            ))
            .right_aligned(),
        );
    } else if let Some(v) = &app.update_available {
        let palette = [
            Color::Rgb(255, 90, 20),
            Color::Rgb(255, 140, 40),
            Color::Rgb(255, 195, 80),
            Color::Rgb(255, 140, 40),
        ];
        let c = palette[(app.anim_tick as usize / 3) % palette.len()];
        block = block.title_bottom(
            Line::from(Span::styled(
                format!(" ⬆ UPDATE v{} — /update to apply ", v),
                Style::default().fg(c).add_modifier(Modifier::BOLD),
            ))
            .right_aligned(),
        );
    }

    let inner = block.inner(area);
    f.render_widget(block, area);

    // When GPUs are present we carve out a right column (one GPU per line).
    // Otherwise left content uses the full inner width (no wasted space).
    let has_gpus = !app.gpu_stats.is_empty();
    let right_width: u16 = if has_gpus { 30 } else { 0 };
    let left_width = inner.width.saturating_sub(right_width);

    // Space the three info rows with a blank line between each (inner rows 0, 2, 4)
    // so the stage / role-labels / project lines don't read as one jumbled block.
    for (i, spans) in [row0, row1, row2].into_iter().enumerate() {
        let y = inner.y + (i as u16) * 2;
        if y >= inner.y + inner.height {
            break;
        }
        let row_area = Rect {
            x: inner.x,
            y,
            width: left_width,
            height: 1,
        };
        f.render_widget(Paragraph::new(Line::from(spans)), row_area);
    }

    // Live GPU list — each GPU gets its own dedicated line in the top-right column.
    // With the header expanded to 5 rows we have plenty of vertical room.
    // Refreshes live ~every 2s via nvidia-smi.
    if has_gpus {
        for (i, g) in app.gpu_stats.iter().enumerate() {
            if i >= 5 {
                break;
            }
            let y = inner.y + i as u16;
            if y >= inner.y + inner.height {
                break;
            }
            let gpu_spans = render_gpu_line(g, i);
            let gpu_area = Rect {
                x: inner.x + left_width,
                y,
                width: right_width,
                height: 1,
            };
            f.render_widget(Paragraph::new(Line::from(gpu_spans)), gpu_area);
        }
    }
}

/// Short model label for splash screen.
fn splash_model_label(cfg: &crate::config::AnvilConfig, role: &str) -> String {
    if let Ok((_, binding, _)) = cfg.resolve_role_full(role) {
        let m = &binding.model;
        if m.len() > 18 {
            m[..18].to_string()
        } else {
            m.clone()
        }
    } else {
        "—".to_string()
    }
}

/// Model label for the header row (coder / R1 / R2). Only the model name.
fn header_model_label(cfg: &crate::config::AnvilConfig, role: &str) -> String {
    if let Ok((_, binding, _)) = cfg.resolve_role_full(role) {
        let m = &binding.model;
        if m.len() > 22 {
            m[..22].to_string()
        } else {
            m.clone()
        }
    } else {
        "not configured".to_string()
    }
}

/// Inline phase progress: `P0✓ P1→ P2○ P3○`
fn build_phase_progress(app: &App) -> String {
    let state = load_state(&app.root);
    let plan_path = active_plan_path(&app.root);

    if !plan_path.exists() {
        return if state.accepted_plan_hash.is_some() {
            "plan: accepted".to_string()
        } else {
            "no plan yet — /plan to generate".to_string()
        };
    }

    let plan = std::fs::read_to_string(&plan_path).unwrap_or_default();
    let phase_ids = crate::phase::plan_phase_ids(&plan);

    if phase_ids.is_empty() {
        return "phases: (none in plan.md)".to_string();
    }

    let parts: Vec<String> = phase_ids
        .iter()
        .map(|id| {
            if state.shipped_phases.contains(id) {
                format!("{}✓", id)
            } else if state.current_phase.as_deref() == Some(id.as_str()) {
                format!("{}→", id)
            } else {
                format!("{}○", id)
            }
        })
        .collect();

    format!("phases: {}", parts.join(" "))
}

/// Build the spans for a single GPU's line in the right column (one GPU per line).
/// Format example: "│ 0:8000  12% 41.5/48.0G"
/// GPU util % color-coded (green/yellow/red).
/// VRAM shows driver-used/total (from nvidia-smi). High usage is colored to highlight "full" cards.
/// (Ollama /loaded reports the actual weights+cache it thinks it has resident; the two numbers
/// commonly differ by a few GB due to CUDA overhead, contexts, and KV cache.)
fn render_gpu_line(stat: &GpuStat, idx: usize) -> Vec<Span<'static>> {
    let mut out: Vec<Span<'static>> = vec![];

    // Subtle column separator so the right GPU list is visually distinct from left content.
    out.push(Span::styled(
        "│ ",
        Style::default().fg(Color::Rgb(60, 60, 80)),
    ));

    let short = short_gpu_name(&stat.name);
    out.push(Span::styled(
        format!("{}:{}", idx, short),
        Style::default().fg(Color::DarkGray),
    ));

    let util_col = if stat.util >= 85 {
        Color::Red
    } else if stat.util >= 50 {
        Color::Rgb(255, 200, 80)
    } else {
        Color::Rgb(90, 200, 130)
    };
    out.push(Span::styled(
        format!("  {}%", stat.util),
        Style::default().fg(util_col).add_modifier(Modifier::BOLD),
    ));

    let used = stat.mem_used as f32 / 1024.0;
    let tot = stat.mem_total as f32 / 1024.0;
    let mem_pct = if stat.mem_total > 0 {
        (stat.mem_used as f32 / stat.mem_total as f32 * 100.0) as u8
    } else {
        0
    };
    let mem_col = if mem_pct >= 90 {
        Color::Red
    } else if mem_pct >= 70 {
        Color::Rgb(255, 200, 80)
    } else {
        Color::Gray
    };
    out.push(Span::styled(
        format!(" {:.1}/{:.1}G", used, tot),
        Style::default().fg(mem_col),
    ));

    out
}

fn short_gpu_name(name: &str) -> String {
    let tokens: Vec<&str> = name.split_whitespace().collect();
    // Prefer a token containing digits or common accelerator prefixes (last-to-first).
    for &t in tokens.iter().rev() {
        let tu = t.to_ascii_uppercase();
        if tu.chars().any(|c| c.is_ascii_digit())
            || tu.starts_with('A')
            || tu.starts_with("MI")
            || tu.starts_with('H')
            || tu.len() <= 8
        {
            let mut s = t.to_string();
            if s.len() > 10 {
                s.truncate(10);
            }
            return s;
        }
    }
    tokens
        .last()
        .map(|s| {
            let mut x = s.to_string();
            if x.len() > 10 {
                x.truncate(10);
            }
            x
        })
        .unwrap_or_else(|| "GPU".to_string())
}

// ─── Chat area ───────────────────────────────────────────────────────────────

fn render_chat(f: &mut Frame, app: &mut App, area: ratatui::layout::Rect) {
    let chat_title = match app.stage {
        WorkflowStage::PlanReviewsComplete => {
            " Plan R1+R2 done (sequential via /lock-plan) — /accept-plan (↑↓ scroll) "
        }
        WorkflowStage::PlanAccepted => {
            " Plan accepted — /phase-start Px ; coder writes review docs on phase done (↑↓ / cmds) "
        }
        _ => "Chat Log (↑↓ scroll)",
    };

    let border_color = if app.first_run || app.stage == WorkflowStage::Unconfigured {
        Color::Rgb(120, 80, 0)
    } else {
        match app.stage {
            WorkflowStage::PlanAccepted => Color::Rgb(40, 120, 40),
            WorkflowStage::PlanReviewsComplete => Color::Rgb(100, 40, 120),
            _ => Color::Rgb(50, 50, 70),
        }
    };

    // Live forge status pinned to the bottom-left of the chat box border, in the
    // current heat color — "forging…" while a tool is actually running (hands on
    // the metal), "smithing…" while the agent is thinking, "ready" (dim) when idle.
    // This is the at-a-glance "is it doing something?" signal, right where the user looks.
    let is_streaming = app.llm_rx.is_some() || app.gate_rx.is_some();
    let status_line: Line = if is_streaming {
        let ember = heat_color(app.forge_heat);
        let verb = if app.tool_active {
            "forging "
        } else {
            "smithing "
        };
        let mut spans = vec![Span::styled(
            format!(" {}", verb),
            Style::default().fg(ember).add_modifier(Modifier::BOLD),
        )];
        // The blade grows while the smith works, glowing in the live heat color.
        spans.extend(forge_sword_spans(app.anim_tick, app.forge_heat, false));
        Line::from(spans)
    } else {
        // Idle: "Ready" + the finished, cooled sword in its true colors.
        let mut spans = vec![Span::styled(
            "Ready. ",
            Style::default()
                .fg(FORGE_MOLTEN)
                .add_modifier(Modifier::BOLD),
        )];
        spans.extend(forge_sword_spans(app.anim_tick, app.forge_heat, true));
        Line::from(spans)
    };

    let chat_block = Block::default()
        .title(Span::styled(
            chat_title,
            Style::default().fg(Color::DarkGray),
        ))
        .title_bottom(status_line.left_aligned())
        .borders(Borders::TOP | Borders::BOTTOM)
        .border_style(Style::default().fg(border_color));

    let chat = if app.messages.is_empty() {
        Paragraph::new(vec![Line::from(Span::styled(
            "(no messages yet — start typing or try /help)",
            Style::default().fg(Color::DarkGray),
        ))])
        .block(chat_block)
        .wrap(Wrap { trim: false })
    } else {
        // Build the full transcript lines every frame (cheap for normal chat lengths).
        // When follow_bottom, compute a scroll offset that places the tail of the content
        // so the newest text appears anchored toward the bottom of the chat area as the
        // conversation (and live stream) grows. This eliminates the "jumps to top on Enter"
        // and gives natural downward scroll/progress.
        let all_lines: Vec<Line> = app
            .messages
            .iter()
            .flat_map(|m| App::render_message_as_lines(m))
            .collect();

        // Scroll must be measured in WRAPPED rows, not logical lines: long messages
        // wrap, and Paragraph::scroll skips wrapped rows. Counting logical lines made
        // follow-bottom under-scroll, hiding the newest (live) line below the viewport.
        let inner_w = area.width.max(1); // full width — no left/right borders
        let h = area.height.saturating_sub(2).max(1); // visible rows inside top/bottom borders
        let para = Paragraph::new(all_lines).wrap(Wrap { trim: false });
        let total_rows = para.line_count(inner_w) as u16;
        let max_scroll = total_rows.saturating_sub(h);

        // Cache for the key handlers so manual scroll can clamp + re-engage follow.
        app.last_max_scroll = max_scroll;

        let scroll_y = if app.follow_bottom {
            max_scroll
        } else {
            (app.view_offset as u16).min(max_scroll)
        };
        para.block(chat_block).scroll((scroll_y, 0))
    };
    f.render_widget(chat, area);
}

// ─── Input box ───────────────────────────────────────────────────────────────

fn render_input_box(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let (prompt, title) = if let Some(wizard) = &app.config_wizard {
        let p = match &wizard.step {
            WizardStep::ProviderName => "provider name> ",
            WizardStep::BaseUrl => "base url> ",
            WizardStep::EnvVarName => "env var name> ",
            WizardStep::ApiKeySecret => "api key (hidden)> ",
            WizardStep::ModelName => "model id> ",
            WizardStep::BindingNote => "note (optional)> ",
            _ => "config> ",
        };
        let t = if wizard.list_title.is_empty() {
            " Config wizard — type answer + Enter (Esc=back) ".to_string()
        } else {
            format!(" {} ", wizard.list_title)
        };
        (p, t)
    } else {
        (
            "> ",
            "Input (Enter=send, Shift+Enter=newline, /=commands, /q=quit)".to_string(),
        )
    };

    let _ = prompt; // prompt is now part of input_full_text(); title is still used below

    let full_text = app.input_full_text();

    let border_color = if app.config_wizard.is_some() {
        Color::Yellow
    } else {
        Color::Rgb(60, 80, 100)
    };

    // Map the edit cursor (a byte offset into app.input) onto a (line, column) in
    // the displayed text so we can draw it where the user is actually editing —
    // not just at the end. The prompt prefix shifts every column right by its
    // char width; secret-mode bullets are 1-per-char so the column still lines up.
    let cursor_byte = {
        let mut cb = app.input_cursor.min(app.input.len());
        while cb > 0 && !app.input.is_char_boundary(cb) {
            cb -= 1;
        }
        cb
    };
    let cursor_global_col =
        app.input_prompt().chars().count() + app.input[..cursor_byte].chars().count();

    let display_lines: Vec<&str> = full_text.split('\n').collect();
    let (cur_line, cur_col) = {
        let mut rem = cursor_global_col;
        let mut found = (display_lines.len().saturating_sub(1), 0usize);
        for (li, line) in display_lines.iter().enumerate() {
            let len = line.chars().count();
            if rem <= len {
                found = (li, rem);
                break;
            }
            rem -= len + 1; // +1 for the '\n' that split() consumed between lines
        }
        found
    };

    // Forge cursor: a molten highlight that pulses between hot ember and cooled
    // iron. Mid-text it reverses onto the character under it (block cursor); at
    // end-of-line it's a trailing bar. Only the cursor blinks; text stays steady.
    let cursor_on = (app.anim_tick / 7).is_multiple_of(2);
    let white = Style::default().fg(Color::White);
    let bar_style = if cursor_on {
        Style::default()
            .fg(Color::Rgb(255, 140, 40))
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Rgb(90, 35, 20))
    };
    let block_style = if cursor_on {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Rgb(255, 140, 40))
            .add_modifier(Modifier::BOLD)
    } else {
        white
    };

    let mut input_lines: Vec<Line<'static>> = display_lines
        .iter()
        .enumerate()
        .map(|(li, line)| {
            if li != cur_line {
                return Line::from(Span::styled((*line).to_string(), white));
            }
            let chars: Vec<char> = line.chars().collect();
            if cur_col >= chars.len() {
                // Cursor past the last char of this line: text then a blinking bar.
                Line::from(vec![
                    Span::styled((*line).to_string(), white),
                    Span::styled("▌", bar_style),
                ])
            } else {
                // Cursor over a char: reverse-highlight just that one cell.
                let before: String = chars[..cur_col].iter().collect();
                let at: String = chars[cur_col..cur_col + 1].iter().collect();
                let after: String = chars[cur_col + 1..].iter().collect();
                Line::from(vec![
                    Span::styled(before, white),
                    Span::styled(at, block_style),
                    Span::styled(after, white),
                ])
            }
        })
        .collect();
    if input_lines.is_empty() {
        input_lines.push(Line::from(Span::styled("▌".to_string(), bar_style)));
    }
    let input_text = Text::from(input_lines);

    // Scroll in WRAPPED rows so the cursor line (bottom) is always visible once
    // the input grows past the box cap — same wrapped-row math as the chat log.
    let inner_w = area.width.max(1); // full width — no left/right borders
    let inner_h = area.height.saturating_sub(2).max(1);
    let para = Paragraph::new(input_text).wrap(Wrap { trim: false });
    let total_rows = Paragraph::new(format!("{}▌", full_text))
        .wrap(Wrap { trim: false })
        .line_count(inner_w) as u16;
    let scroll_y = total_rows.saturating_sub(inner_h);

    let input_widget = para
        .block(
            Block::default()
                .borders(Borders::TOP | Borders::BOTTOM)
                .border_style(Style::default().fg(border_color))
                .title(Span::styled(title, Style::default().fg(Color::DarkGray))),
        )
        .scroll((scroll_y, 0));
    f.render_widget(input_widget, area);
}

// ─── Floating overlays ────────────────────────────────────────────────────────

fn render_palette_popup(f: &mut Frame, app: &App, chat_area: ratatui::layout::Rect) {
    if !app.showing_command_palette {
        return;
    }
    let filtered = app.filtered_commands();
    if filtered.is_empty() {
        return;
    }

    // Give the palette more vertical room than before (was hard-capped at 12 → ~10 visible).
    // When there are more commands than fit, we use a ListState + render_stateful_widget
    // so that the current selection is always scrolled into the visible window (no more
    // selectable-but-invisible items).
    let available = chat_area.height.saturating_sub(2).max(5);
    let max_h = available.min(18);
    let needed = (filtered.len() as u16) + 2;
    let h = needed.min(max_h).max(3);
    let popup = ratatui::layout::Rect {
        x: chat_area.x + 2,
        y: chat_area.y + chat_area.height.saturating_sub(h),
        width: chat_area.width.saturating_sub(4),
        height: h,
    };

    f.render_widget(Clear, popup);

    let selected = app.command_selected.min(filtered.len().saturating_sub(1));
    let items: Vec<ListItem> = filtered
        .iter()
        .enumerate()
        .map(|(i, &(cmd, desc))| {
            if i == selected {
                ListItem::new(Line::from(vec![
                    Span::styled(
                        format!(" {} ", cmd),
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("  {}", desc),
                        Style::default().fg(Color::Black).bg(Color::Cyan),
                    ),
                ]))
            } else {
                ListItem::new(Line::from(vec![
                    Span::styled(format!(" {} ", cmd), Style::default().fg(Color::Cyan)),
                    Span::styled(format!("  {}", desc), Style::default().fg(Color::DarkGray)),
                ]))
            }
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title(Span::styled(
                " Commands (↑↓ pick, Enter run, Esc close, type to filter) ",
                Style::default()
                    .fg(FORGE_MOLTEN)
                    .add_modifier(Modifier::BOLD),
            )),
    );

    let mut state = ListState::default();
    state.select(Some(selected));
    f.render_stateful_widget(list, popup, &mut state);
}

/// The selectable run_command approval prompt (↑/↓ + Enter), floated over chat.
fn render_confirm_popup(f: &mut Frame, app: &App, chat_area: ratatui::layout::Rect) {
    let Some(cmd) = &app.awaiting_confirm else {
        return;
    };
    let prog = program_of(cmd);
    let options = [
        "Yes — run it once".to_string(),
        format!("Yes — and allow all `{}` commands this session", prog),
        "No — don't run it".to_string(),
    ];
    let selected = app.confirm_selected.min(options.len() - 1);

    let h = (options.len() as u16) + 3; // command line + options + borders
    let popup = ratatui::layout::Rect {
        x: chat_area.x + 2,
        y: chat_area.y + chat_area.height.saturating_sub(h),
        width: chat_area.width.saturating_sub(4),
        height: h,
    };
    f.render_widget(Clear, popup);

    let mut lines: Vec<Line> = vec![Line::from(vec![
        Span::styled("  $ ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            cmd.clone(),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
    ])];
    for (i, opt) in options.iter().enumerate() {
        let marker = if i == selected { " ▶ " } else { "   " };
        let style = if i == selected {
            let bg = if i == options.len() - 1 {
                Color::Rgb(120, 35, 35)
            } else {
                Color::Rgb(30, 90, 45)
            };
            Style::default()
                .fg(Color::White)
                .bg(bg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };
        lines.push(Line::from(Span::styled(
            format!("{}{}", marker, opt),
            style,
        )));
    }

    let para = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(255, 170, 60)))
            .title(Span::styled(
                " Run command? (↑↓ choose · Enter confirm · Esc = No) ",
                Style::default()
                    .fg(FORGE_MOLTEN)
                    .add_modifier(Modifier::BOLD),
            )),
    );
    f.render_widget(para, popup);
}

fn render_wizard_popup(f: &mut Frame, app: &App, chat_area: ratatui::layout::Rect) {
    let wizard = match &app.config_wizard {
        Some(w) if !w.list_items.is_empty() => w,
        _ => return,
    };

    // Use most of the available height so long lists (providers, model IDs) all fit.
    let available = chat_area.height.saturating_sub(4).max(4);
    let needed = (wizard.list_items.len() as u16).saturating_add(2);
    let h = needed.min(available);

    let popup = ratatui::layout::Rect {
        x: chat_area.x + 2,
        y: chat_area.y + chat_area.height.saturating_sub(h),
        width: chat_area.width.saturating_sub(4),
        height: h,
    };

    f.render_widget(Clear, popup);

    let items: Vec<ListItem> = wizard
        .list_items
        .iter()
        .map(|item| {
            let display = item.as_str();
            if matches!(wizard.step, WizardStep::RoleAssignment { .. }) {
                // Color each row according to the provider that offers the model.
                // Choices are either "model  [prov]" (from the new per-provider discovery)
                // or plain binding names (looked up for their provider color).
                // Models from the same provider appear consecutively (grouped in build fn)
                // and share the same color.
                let prov = app.extract_provider_for_choice(display);
                let col = app.color_for_provider(&prov);
                ListItem::new(Line::from(Span::styled(
                    format!("  {}  ", display),
                    Style::default().fg(col),
                )))
            } else {
                let has_check = if matches!(wizard.step, WizardStep::ProviderType) {
                    if let Some(p) = PROVIDER_PRESETS.iter().find(|p| p.0 == display) {
                        let (dname, sname, ptyp, _, _) = *p;
                        app.is_provider_preset_configured(dname, sname, ptyp)
                    } else {
                        false
                    }
                } else {
                    false
                };
                if has_check {
                    // Green checkmark for already-configured providers. Names left-aligned within this list.
                    ListItem::new(Line::from(vec![
                        Span::raw("  "),
                        Span::styled(
                            "✓ ",
                            Style::default()
                                .fg(Color::Green)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(format!("{}  ", display)),
                    ]))
                } else if matches!(wizard.step, WizardStep::ProviderType) {
                    // Extra indent so that unchecked provider names align with "✓ Name" ones.
                    ListItem::new(format!("    {}  ", display))
                } else {
                    ListItem::new(format!("  {}  ", display))
                }
            }
        })
        .collect();

    // Color the wizard popup border for role assignment (and quick Ollama picks)
    // so CODER/R1/R2 identity stays visible (blue / purple / lime). Every other
    // step gets the wizard's signature amethyst border to match the 🪄 title.
    let wiz_border = match &wizard.step {
        WizardStep::QuickOllamaModelPick { role } if role == "coder" => ROLE_CODER,
        WizardStep::QuickOllamaModelPick { role } if role == "reviewer_a" => ROLE_R1,
        WizardStep::QuickOllamaModelPick { role } if role == "reviewer_b" => ROLE_R2,
        WizardStep::RoleAssignment { role } if role == "coder" => ROLE_CODER,
        WizardStep::RoleAssignment { role } if role == "reviewer_a" => ROLE_R1,
        WizardStep::RoleAssignment { role } if role == "reviewer_b" => ROLE_R2,
        _ => WIZARD_PURPLE,
    };

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(wiz_border))
                .title(Span::styled(
                    format!(" 🪄 {} ", wizard.list_title),
                    Style::default()
                        .fg(WIZARD_PURPLE)
                        .add_modifier(Modifier::BOLD),
                )),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(" ▶ ");

    let mut state = ListState::default();
    state.select(Some(wizard.list_selected));
    f.render_stateful_widget(list, popup, &mut state);
}

fn render_doc_popup(f: &mut Frame, app: &mut App, chat_area: ratatui::layout::Rect) {
    // Pull out the title and pre-render the body lines (owned, 'static) so the
    // immutable borrow of `viewing_doc` ends before we clamp `doc_scroll` below.
    let (title, lines) = match &app.viewing_doc {
        Some((title, content)) => {
            let lines: Vec<Line> = content
                .lines()
                .flat_map(|l| App::render_message_as_lines(&format!("[doc] {}", l)))
                .collect();
            (title.clone(), lines)
        }
        None => return,
    };

    let h = (chat_area.height.saturating_sub(4)).clamp(8, 30);
    let popup = ratatui::layout::Rect {
        x: chat_area.x + 3,
        y: chat_area.y + 2,
        width: chat_area.width.saturating_sub(6),
        height: h,
    };

    f.render_widget(Clear, popup);

    let para = Paragraph::new(lines).wrap(Wrap { trim: false });

    // Clamp the scroll offset to the actual wrapped content so PgDn/↓ can't run
    // off the end of the document (and ↑ brings the top back into view).
    let inner_w = popup.width.saturating_sub(2).max(1);
    let inner_h = popup.height.saturating_sub(2);
    let total = para.line_count(inner_w) as u16;
    app.doc_scroll = app.doc_scroll.min(total.saturating_sub(inner_h));

    let viewer = para
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Magenta))
                .title(Span::styled(
                    format!(" {} (↑/↓/PgUp/PgDn · Esc to close) ", title),
                    Style::default()
                        .fg(FORGE_MOLTEN)
                        .add_modifier(Modifier::BOLD),
                )),
        )
        .scroll((app.doc_scroll, 0));
    f.render_widget(viewer, popup);
}

fn render_approvals_popup(f: &mut Frame, app: &App, chat_area: ratatui::layout::Rect) {
    let Some(ed) = &app.approvals_editor else {
        return;
    };

    let available = chat_area.height.saturating_sub(4).max(6);
    let needed = (ed.items.len() as u16).saturating_add(3);
    let h = needed.min(available);
    let popup = ratatui::layout::Rect {
        x: chat_area.x + 2,
        y: chat_area.y + chat_area.height.saturating_sub(h),
        width: chat_area.width.saturating_sub(4),
        height: h,
    };
    f.render_widget(Clear, popup);

    let items: Vec<ListItem> = ed
        .items
        .iter()
        .map(|it| {
            let (mark, mark_col) = if it.approved {
                ("[x] ", Color::Green)
            } else {
                ("[ ] ", Color::DarkGray)
            };
            ListItem::new(Line::from(vec![
                Span::raw("  "),
                Span::styled(mark, Style::default().fg(mark_col)),
                Span::raw(format!("{}  ", it.prefix)),
            ]))
        })
        .collect();

    // Show any in-progress custom entry in the title so the user sees what they're typing.
    let title = if app.input.trim().is_empty() {
        " Command approvals — Space toggle · type prefix + Enter to add · Esc save ".to_string()
    } else {
        format!(" Add prefix: \"{}\"  (Enter to add · Esc save) ", app.input)
    };

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .title(Span::styled(
                    title,
                    Style::default()
                        .fg(FORGE_MOLTEN)
                        .add_modifier(Modifier::BOLD),
                )),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(" ▶ ");

    let mut state = ListState::default();
    state.select(Some(ed.selected));
    f.render_stateful_widget(list, popup, &mut state);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_token_strips_usage_placeholders() {
        // Bare commands are untouched.
        assert_eq!(command_token("/review"), "/review");
        // Bracketed/angle placeholders (and everything after) are dropped, so the
        // palette inserts a runnable command rather than literal hint text.
        assert_eq!(command_token("/review [--deep] [label]"), "/review");
        assert_eq!(command_token("/ship-phase [id]"), "/ship-phase");
        assert_eq!(command_token("/phase-start <id>"), "/phase-start");
        assert_eq!(command_token("/new-plan <name>"), "/new-plan");
        assert_eq!(command_token("/unload [model]"), "/unload");
    }

    #[test]
    fn every_palette_command_is_runnable_after_token_extraction() {
        // The inserted text must start with '/' and carry no placeholder characters,
        // or the command would be un-runnable straight from the palette.
        for (display, _desc) in SLASH_COMMANDS {
            let token = command_token(display);
            assert!(token.starts_with('/'), "{display} -> {token}");
            assert!(
                !token.contains('[') && !token.contains('<'),
                "{display} -> {token} still has a placeholder"
            );
        }
    }
}
