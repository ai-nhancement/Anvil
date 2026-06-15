//! ratatui TUI for Anvil (Phase 2 complete: real LLM streaming chat).
//!
//! Default launch target: `anvil` (no subcommand) or `cargo run --`.
//! Persistent chat-centric UI. All legacy headless subcommands remain fully functional.
//!
//! Phase 2: normal typing now resolves the planner (or coder) role, calls the real LlmClient
//! via the new chat_stream_to_channel path, and appends tokens live using mpsc from a
//! background multi-thread tokio runtime. No stdout writes from LLM code while in TUI.
//! Headless paths (plan/phase/talk + their block_on + prints) untouched.

use std::io::stdout;
use std::path::{Path, PathBuf};

use anyhow::Result;
use tokio::sync::mpsc;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame, Terminal,
};

use crate::config::{
    ensure_anvil_dir, load_config, save_config, AnvilConfig, CredentialRef, ModelBinding,
    ProviderConnection, Roles,
};
use crate::llm::LlmClient;
use crate::state::{load_state, reviews_dir, save_state};

/// Workflow stage machine for the TUI (reconciled from disk artifacts + state on every relevant action).
/// This makes the "source of truth is the files" contract visible and enforces the gates.
#[derive(Clone, Debug, Default, PartialEq)]
enum WorkflowStage {
    #[default]
    Talk,
    PlanReviewsComplete, // R1 + R2 done for the plan, awaiting explicit accept
    PlanAccepted,        // hash recorded, ready for phases (phase 4 will add InPhase etc.)
    Unconfigured,
}

/// Slash commands shown in the interactive palette (triggered by typing `/`).
/// Descriptions appear in the popup to help users discover the flow.
const SLASH_COMMANDS: &[(&str, &str)] = &[
    ("/plan", "Generate (or refresh) the plan, then run exactly R1 + R2 reviews"),
    ("/accept-plan", "Record that R1+R2 findings were addressed; unlocks phases"),
    ("/config", "Configure providers, model bindings, roles & API keys (full setup)"),
    ("/setup", "Alias for /config — providers, models, keys"),
    ("/status", "Show reviewers, config state, and current gate progress"),
    ("/help", "Show key bindings and available commands"),
    ("/quit", "Exit the TUI"),
    ("/phase-done", "Phase gates (R1+R2 + accept) — coming in phase 4"),
    ("/include <path>", "Include a project file's content as context for the model (enables grounded suggestions)"),
    ("/context", "List files currently included as context"),
    ("/clear-context", "Remove all included context files"),
    ("/view-plan", "Open the current plan.md in a focused review popup (Cline-style card)"),
    ("/view-reviews", "Open REVIEW_plan_R1 + R2 in a focused review popup — inspect the two independent reviews before /accept-plan"),
];

const SPLASH_DURATION: u8 = 1; // any nonzero value; splash waits for keypress, not a timer

const SPINNER: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

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

/// Steps in the in-TUI configuration wizard (launched via /config or /setup).
#[derive(Clone, Debug, PartialEq)]
enum WizardStep {
    MainMenu,
    // Provider connection flow
    ProviderType,
    ProviderName,
    BaseUrl,
    CredentialKind,
    EnvVarName,
    ApiKeySecret,
    // Model binding flow
    BindingProvider,
    BindingName,
    ModelName,
    BindingNote,
    // Role assignment
    RoleAssignment { role: String },
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

    binding_provider: Option<String>,
    binding_name: Option<String>,
    model: Option<String>,
    note: Option<String>,

    // Which role we are currently assigning (for RoleAssignment step)
    current_role: Option<String>,
}

/// Entry point called from main when no subcommand (or `anvil ui`) is given.
pub fn run_ui(root: &Path) -> Result<()> {
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

    // Setup terminal (raw mode + alternate screen). We must restore on any exit path.
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run the app loop; capture result so we can always restore the terminal.
    let run_result = run_app_loop(&mut terminal, &mut app);

    // Restore terminal state (critical on Windows and for users who ^C).
    let _ = disable_raw_mode();
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let _ = terminal.show_cursor();

    run_result
}

fn is_unconfigured(root: &Path) -> bool {
    if !root.join("anvil.toml").exists() {
        return true;
    }
    match load_config(root) {
        Ok(cfg) => {
            // Consider unconfigured unless the two critical reviewers are both set.
            cfg.roles.reviewer_a.is_none() || cfg.roles.reviewer_b.is_none()
        }
        Err(_) => true,
    }
}

struct App {
    root: PathBuf,
    messages: Vec<String>,
    input: String,
    view_offset: usize, // simple scroll control (index of first visible message from top)
    should_quit: bool,
    first_run: bool,
    status_line: String,

    // For real LLM chat (phase 2+)
    runtime: Option<tokio::runtime::Runtime>,
    llm: LlmClient,
    cfg: Option<AnvilConfig>,
    llm_rx: Option<mpsc::UnboundedReceiver<String>>,

    // Workflow + plan gate (phase 3)
    stage: WorkflowStage,
    gate_rx: Option<mpsc::UnboundedReceiver<String>>, // signals from spawn_blocking plan gate

    // Slash command palette (opened by pressing / ; supports arrows + live filter)
    showing_command_palette: bool,
    command_selected: usize,

    // In-TUI configuration wizard (/config). When Some, normal chat is suspended
    // and the UI drives a step-by-step provider / binding / role + key flow.
    config_wizard: Option<ConfigWizard>,

    // Animation state (frame counter + splash countdown)
    splash_ticks: u8, // nonzero = showing splash; cleared on first keypress
    anim_tick: u64,   // increments every frame, drives spinner + cursor blink

    // When true the input characters are masked in the UI (for API keys)
    input_secret: bool,

    // Files whose contents are sent as additional context with chat turns (via /include).
    // First step toward real agentic/grounded assistance *behind the gates* (post PlanAccepted).
    // The model sees the real file text; human still decides what to keep or edit.
    active_context: Vec<(PathBuf, String)>,

    // Lightweight document viewer popup (for /view-plan, /view-reviews etc.).
    // Gives a focused "card" experience for inspecting gate artifacts (plan + the two reviews)
    // before the explicit accept step — inspired by deliberate plan/approve flows.
    viewing_doc: Option<(String, String)>, // (title, full_content)
}

impl App {
    fn new(root: PathBuf) -> Self {
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
            view_offset: 0,
            should_quit: false,
            first_run: false,
            status_line: String::new(),
            runtime,
            llm: LlmClient::new(),
            cfg: None,
            llm_rx: None,
            stage: WorkflowStage::Talk,
            gate_rx: None,
            showing_command_palette: false,
            command_selected: 0,
            config_wizard: None,
            splash_ticks: SPLASH_DURATION,
            anim_tick: 0,
            input_secret: false,
            active_context: vec![],
            viewing_doc: None,
        };

        // Best-effort load of existing config so we can do real chat immediately if roles are set.
        app.cfg = load_config(&app.root).ok();
        let has_reviewers = app
            .cfg
            .as_ref()
            .map_or(false, |c| c.roles.reviewer_a.is_some() && c.roles.reviewer_b.is_some());

        app.push_system("Welcome to Anvil TUI.");
        app.push_system("Type to chat with the planner (or coder). Real streaming to your configured model. /plan /phase-done /status /help /quit");
        if !has_reviewers {
            app.first_run = true;
            // The smooth first-time experience auto-launches the config wizard in run_ui().
            // We keep one gentle note here; the wizard itself will guide the user.
            app.push_system("First run detected — the setup wizard will open so you can connect a model and assign roles in under a minute.");
        } else {
            app.push_system("Configuration loaded. Reviewers are distinct — gates will enforce R1 then R2.");
        }
        app.reconcile_stage_from_disk();
        app.update_status();
        app
    }

    fn push(&mut self, line: String) {
        self.messages.push(line);
        // Auto-scroll to bottom by resetting offset (simple strategy for skeleton).
        if self.messages.len() > 20 {
            self.view_offset = self.messages.len() - 20;
        } else {
            self.view_offset = 0;
        }
    }

    fn push_system(&mut self, text: &str) {
        self.push(format!("[system] {}", text));
    }

    fn update_status(&mut self) {
        let proj = self
            .root
            .file_name()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| self.root.display().to_string());
        let stage = if self.first_run || self.stage == WorkflowStage::Unconfigured {
            "UNCONFIGURED — press s for quick setup"
        } else {
            match self.stage {
                WorkflowStage::Talk => "TALK (chat with planner/coder; /plan for gate)",
                WorkflowStage::PlanReviewsComplete => "PLAN (R1+R2 done — /accept-plan to proceed)",
                WorkflowStage::PlanAccepted => "PLAN ACCEPTED — /start <id> for phases (phase gates in next iteration)",
                _ => "TALK",
            }
        };
        let ctx = if !self.active_context.is_empty() {
            format!("  |  ctx:{}", self.active_context.len())
        } else {
            String::new()
        };
        self.status_line = format!("Anvil — {}  |  {}{}", proj, stage, ctx);
    }

    /// Include a file (relative to project root) into the active context.
    /// The full (truncated) contents will be appended to the user message on the *next*
    /// chat turn so the model can give grounded, code-aware answers.
    fn include_file(&mut self, path_str: &str) {
        let candidate = self.root.join(path_str);
        let p = if candidate.exists() {
            candidate
        } else {
            // Also accept absolute or already-rooted paths
            PathBuf::from(path_str)
        };
        if !p.exists() || !p.is_file() {
            self.push_system(&format!("File not found or not a regular file: {}", path_str));
            return;
        }
        match std::fs::read_to_string(&p) {
            Ok(content) => {
                let rel = p.strip_prefix(&self.root).unwrap_or(&p).to_path_buf();
                // De-duplicate
                self.active_context.retain(|(rp, _)| rp != &rel);
                self.active_context.push((rel.clone(), content.clone()));
                let note = if self.stage != WorkflowStage::PlanAccepted {
                    " (most useful after you /accept-plan)"
                } else {
                    ""
                };
                self.push_system(&format!(
                    "✓ Context added: {} ({} chars){}",
                    rel.display(),
                    content.len(),
                    note
                ));
                self.update_status();
            }
            Err(e) => {
                self.push_system(&format!("Could not read {}: {}", path_str, e));
            }
        }
    }

    fn show_context(&mut self) {
        if self.active_context.is_empty() {
            self.push_system("No active context. Use /include <relative-path> to give the model real file contents.");
            return;
        }
        self.push_system("Active context files (contents sent with your next messages):");
        // Snapshot to avoid holding borrow on active_context while calling push_system (mut self).
        let snapshots: Vec<(String, usize, String)> = self
            .active_context
            .iter()
            .map(|(p, c)| {
                let preview = c.lines().next().unwrap_or("").chars().take(60).collect::<String>();
                (p.display().to_string(), c.len(), preview)
            })
            .collect();
        for (pstr, len, preview) in snapshots {
            self.push_system(&format!("  • {} ({} chars)  e.g. {}", pstr, len, preview));
        }
        self.push_system("Use /clear-context to remove them. The model sees these files until cleared.");
    }

    fn open_doc_viewer(&mut self, title: &str, path: &Path) {
        match std::fs::read_to_string(path) {
            Ok(content) => {
                self.viewing_doc = Some((title.to_string(), content));
                self.push_system(&format!("Opened '{}' — Esc to close the card. (Content also available in your editor for deep review.)", path.display()));
            }
            Err(e) => {
                self.push_system(&format!("Could not open {}: {}", path.display(), e));
            }
        }
    }

    /// Production-quality message renderer for the main chat log.
    /// - Respects [you], [system], [review R*], [assistant via ...] prefixes with distinct colors.
    /// - Parses ```lang ... ``` fences anywhere in the message (including inside the big REVIEW_*.md dumps
    ///   and context-augmented LLM replies) and renders them as visual "code cards" using box-drawing
    ///   characters + muted style. This gives a Cline-like richer reading experience for code suggestions
    ///   and the structured review findings.
    /// - Crude but effective markdown-ish treatment for # headers and **bold** lines (common in plan/reviews).
    /// - Properly splits on embedded \n (so one big [review R1]\n<full md with its own code> becomes many clean Lines).
    /// Used by both the main chat Paragraph and the document viewer popups.
    fn render_message_as_lines(m: &str) -> Vec<Line<'static>> {
        let (base_style, body) = if m.starts_with("[system]") {
            (Style::default().fg(Color::Yellow), m.strip_prefix("[system] ").unwrap_or(m))
        } else if m.starts_with("[you]") {
            (Style::default().fg(Color::Green), m.strip_prefix("[you] ").unwrap_or(m))
        } else if m.starts_with("[demo]") {
            (Style::default().fg(Color::Magenta), m.strip_prefix("[demo] ").unwrap_or(m))
        } else if m.starts_with("[review") {
            // Prominent treatment for the gate reviews (the heart of the "exactly two" contract).
            (Style::default().fg(Color::Cyan).bold(), m)
        } else if m.starts_with("[") && m.contains(" via ") {
            // LLM responses, e.g. [planner via llama3.2]
            (Style::default().fg(Color::LightBlue), m)
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
                    out.push(Line::from(Span::styled(header, Style::default().fg(Color::Blue).bold())));
                } else {
                    in_code = false;
                    out.push(Line::from(Span::styled("└─── end code ", Style::default().fg(Color::Blue))));
                }
                continue;
            }

            let style = if in_code {
                // Code inside the visual card — muted so it doesn't fight the surrounding text.
                Style::default().fg(Color::Gray)
            } else if line.starts_with('#') || line.starts_with("**") || line.starts_with("Reviewer:") || line.starts_with("Date:") {
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

            out.push(Line::from(Span::styled(displayed, style)));
        }

        // If the original had a review prefix, make the very first line a strong banner for production feel.
        if m.starts_with("[review") && !out.is_empty() {
            // Prepend a clear separator banner (the first real content line will follow).
            let banner = Line::from(Span::styled(
                "════════════════════════════════════════════════════════════",
                Style::default().fg(Color::Cyan),
            ));
            out.insert(0, banner);
        }

        if out.is_empty() {
            out.push(Line::from(Span::styled(m.to_string(), base_style)));
        }
        out
    }

    fn handle_input(&mut self) {
        let input = std::mem::take(&mut self.input);
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
        let cmd = trimmed.to_lowercase();

        // Built-in slash commands for the skeleton (real gates added in later phases)
        if cmd == "/quit" || cmd == "/q" || cmd == ":q" {
            self.should_quit = true;
            return;
        }

        if cmd == "/status" {
            let configured = !self.first_run;
            self.push_system(&format!(
                "root={}  configured={}  messages={}",
                self.root.display(),
                configured,
                self.messages.len()
            ));
            return;
        }

        if cmd == "/plan" {
            if !self.is_configured() {
                self.push_system("Cannot run plan gate: no reviewers configured. The first-run wizard should have started (or press 's' / use /config).");
                return;
            }
            self.push_system("=== PLAN GATE (R1 + R2) ===");
            self.push_system("Spawning exact `anvil plan --fresh` equivalent (reuses plan.rs generation + run_single_review + header + state hash).");
            self.push_system("Progress will appear when complete; identical REVIEW_plan_R*.md + plan.md will be written.");

            let (tx, rx) = mpsc::unbounded_channel::<String>();
            self.gate_rx = Some(rx);

            if let Some(rt) = &self.runtime {
                let root = self.root.clone();
                rt.spawn(async move {
                    // spawn_blocking so the sync run_plan (with its block_on calls) runs off the UI thread.
                    // All side effects (plan.md write, REVIEW_*.md writes with canonical headers, state.json hash) are identical to CLI.
                    let res = tokio::task::spawn_blocking(move || {
                        crate::plan::run_plan(&root, /*fresh=*/ true, /*context=*/ None)
                    })
                    .await;

                    let signal = match res {
                        Ok(Ok(())) => "GATE_DONE".to_string(),
                        Ok(Err(e)) => format!("GATE_ERROR: {}", e),
                        Err(e) => format!("GATE_ERROR: {}", e),
                    };
                    let _ = tx.send(signal);
                });
            } else {
                self.push_system("No runtime available for gate task.");
            }
            return;
        }

        if cmd == "/accept-plan" || cmd == "/accept plan" {
            let plan_path = self.root.join("plan.md");
            let rev_dir = reviews_dir(&self.root);
            let r1 = rev_dir.join("REVIEW_plan_R1.md");
            let r2 = rev_dir.join("REVIEW_plan_R2.md");

            if !plan_path.exists() || !r1.exists() || !r2.exists() {
                self.push_system("Both R1 and R2 review files (and plan.md) must exist before accept. Run /plan first.");
                return;
            }

            // Reuse the exact same hash computation as the CLI path.
            if let Ok(plan_txt) = std::fs::read_to_string(&plan_path) {
                let hash = crate::plan::simple_hash(&plan_txt);
                let mut st = load_state(&self.root);
                st.accepted_plan_hash = Some(hash);
                if let Err(e) = save_state(&self.root, &st) {
                    self.push_system(&format!("Warning: could not persist accept hash: {}", e));
                }
            }

            self.stage = WorkflowStage::PlanAccepted;
            self.reconcile_stage_from_disk(); // will pick up the hash we just wrote
            self.update_status();

            self.push_system("✓ Plan accepted. accepted_plan_hash recorded in .anvil/state.json (same as future `anvil plan --accept`).");
            self.push_system("Next steps: implement phases in your editor. Use /start <id> (e.g. P0) when ready (phase gates wired in phase 4).");
            return;
        }

        if cmd == "/config" || cmd == "/setup" {
            self.start_config_wizard();
            return;
        }

        if cmd == "/phase-done" || cmd == "/done" {
            self.push_system("Phase review (exactly R1 + R2) + hard accept guard coming in phase 4. For now use /plan + /accept-plan for the plan gate.");
            return;
        }

        if cmd.starts_with("/include ") {
            // Use original trimmed (not lowercased cmd) so paths keep correct case on all OSes.
            let path_str = trimmed.split_once(' ').map(|(_, rest)| rest.trim()).unwrap_or("");
            if !path_str.is_empty() {
                self.include_file(path_str);
            } else {
                self.push_system("Usage: /include <path>   (path relative to project root)");
            }
            return;
        }
        if cmd == "/context" || cmd == "/ctx" {
            self.show_context();
            return;
        }
        if cmd == "/clear-context" || cmd == "/clearcontext" || cmd == "/nocontext" {
            self.active_context.clear();
            self.update_status();
            self.push_system("Context cleared. Future messages will no longer include file contents.");
            return;
        }

        if cmd == "/view-plan" {
            let plan_path = self.root.join("plan.md");
            self.open_doc_viewer("Plan (read before accept)", &plan_path);
            return;
        }
        if cmd == "/view-reviews" {
            let rev_dir = reviews_dir(&self.root);
            let r1 = rev_dir.join("REVIEW_plan_R1.md");
            let r2 = rev_dir.join("REVIEW_plan_R2.md");
            // Build a combined document for the popup card (keeps the "exactly two different reviewers" visible).
            let mut combined = String::new();
            combined.push_str("=== REVIEW R1 (from plan gate — first independent reviewer) ===\n\n");
            if let Ok(c) = std::fs::read_to_string(&r1) {
                combined.push_str(&c);
            } else {
                combined.push_str("(R1 file not found — run /plan first)\n");
            }
            combined.push_str("\n\n=== REVIEW R2 (from plan gate — second independent reviewer, different binding) ===\n\n");
            if let Ok(c) = std::fs::read_to_string(&r2) {
                combined.push_str(&c);
            } else {
                combined.push_str("(R2 file not found)\n");
            }
            combined.push_str("\n\n--- End of reviews. Address findings, then use /accept-plan to record the gate. ---\n");
            self.viewing_doc = Some(("Plan Reviews — R1 + R2 (Esc to close, scroll chat if needed)".to_string(), combined));
            self.push_system("Opened focused review card for the two mandatory independent reviews. Close with Esc. Then /accept-plan when ready.");
            return;
        }

        if cmd == "/help" || cmd == "?" {
            self.push_system("Keys: Enter=chat (streams), Esc/Ctrl-C/q=quit, s=quick-setup, ↑/↓ scroll chat (or command list), / for palette (filter + arrows + Enter to pick), Backspace");
            self.push_system("Context: /include <path>  •  /context  •  /clear-context   (gives the model real file contents for better suggestions after gates)");
            return;
        }

        // Real LLM chat (planner or coder role) with live streaming via mpsc.
        if self.is_configured() {
            self.start_real_chat(trimmed);
        } else {
            self.push_system("Not configured yet — the wizard should have opened automatically (or press 's' for instant local Ollama, or /config).");
            if trimmed.len() > 3 {
                self.push("[system] (demo) Understood. After setup your messages will stream from the real model.".to_string());
            }
        }
    }

    /// Quick local-Ollama first-run setup. Reuses the exact same config types + save_config
    /// that cmd_init / cmd_setup use, so anvil.toml is 100% compatible.
    ///
    /// Uses two different model families on the *same* local-ollama service for the two
    /// reviewers. Ollama hosts many distinct models, so this still satisfies the R1/R2
    /// cross-family diversity requirement without emitting a "same provider" warning.
    fn do_quick_ollama_setup(&mut self) -> Result<()> {
        ensure_anvil_dir(&self.root)?;

        let mut cfg = load_config(&self.root).unwrap_or_default();

        // Seed the same local-ollama provider (idempotent if already present).
        // Use CredentialRef::None because default Ollama (and many local openai-compat servers)
        // require no key. We still send a conventional placeholder so the HTTP layer is uniform.
        cfg.providers.insert(
            "local-ollama".to_string(),
            ProviderConnection {
                r#type: "openai_compat".to_string(),
                base_url: Some("http://localhost:11434/v1".to_string()),
                credential: CredentialRef::None,
                extra: Default::default(),
            },
        );

        // Two distinct bindings using different model families on the single local-ollama connection.
        // This gives reviewer_a and reviewer_b the required diversity for the anti-drift gates.
        // (Remove any prior single-binding quick-start name so re-running 's' after the old behavior leaves a clean file.)
        cfg.model_bindings.remove("local-default");
        cfg.model_bindings.insert(
            "local-llama".to_string(),
            ModelBinding {
                provider: "local-ollama".to_string(),
                model: "llama3.2".to_string(),
                note: Some("quick-start".to_string()),
            },
        );
        cfg.model_bindings.insert(
            "local-qwen".to_string(),
            ModelBinding {
                provider: "local-ollama".to_string(),
                model: "qwen2.5".to_string(),
                note: Some("quick-start (diverse reviewer)".to_string()),
            },
        );

        // Coder + planner on one; the two reviewers on different bindings so R1 vs R2 has model diversity.
        cfg.roles = Roles {
            coder: Some("local-llama".to_string()),
            planner: Some("local-llama".to_string()),
            reviewer_a: Some("local-llama".to_string()),
            reviewer_b: Some("local-qwen".to_string()),
        };

        save_config(&self.root, &cfg)?;

        self.first_run = false;
        self.cfg = load_config(&self.root).ok();
        self.reconcile_stage_from_disk();
        self.update_status();

        self.push_system("Quick setup complete: local-ollama provider + two bindings (llama3.2 + qwen2.5).");
        self.push_system("reviewer-a uses local-llama; reviewer-b uses local-qwen (different models on same Ollama service).");
        self.push_system("This satisfies the R1 + R2 diversity requirement for the plan/phase gates.");
        self.push_system("You can now type to chat and run /plan for the full structured workflow. (Ollama must be running on :11434 for real calls.)");

        Ok(())
    }

    fn is_configured(&self) -> bool {
        self.cfg
            .as_ref()
            .map_or(false, |c| c.roles.reviewer_a.is_some() && c.roles.reviewer_b.is_some())
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

        // Prefer planner for normal conversation; fall back to coder.
        let role = if cfg.roles.planner.is_some() {
            "planner"
        } else if cfg.roles.coder.is_some() {
            "coder"
        } else {
            self.push_system("No planner or coder role configured. Use 's' or `anvil setup` to assign bindings.");
            return;
        };

        let (binding_name, binding, provider) = match cfg.resolve_role_full(role) {
            Ok(triple) => triple,
            Err(e) => {
                self.push_system(&format!("Role resolution failed for '{}': {}", role, e));
                return;
            }
        };

        let api_key = match self.llm.get_credential(binding_name, provider) {
            Ok(k) => k,
            Err(e) => {
                self.push_system(&format!("Credential error for {}: {}", binding_name, e));
                self.push_system("For local providers (Ollama etc.) use the quick setup or /config and pick 'No authentication' / CredentialRef::None. Real providers need a key in the keyring or a valid env var.");
                return;
            }
        };

        // Clone what we need for the async task + the UI prefix *before* any mutable calls on self.
        // This releases the immutable borrow on self.cfg / binding / provider.
        let model_for_ui_and_task = binding.model.clone();
        let conn_for_task = provider.clone();
        let key_for_task = api_key.clone();

        // Create the (unbounded) channel. The receiver lives in the App; the sender is moved into the spawned task.
        let (tx, rx) = mpsc::unbounded_channel::<String>();
        self.llm_rx = Some(rx);

        // Start the visible streaming line with a nice role prefix (tokens will be appended to this entry).
        let prefix = format!("[{} via {}] ", role, model_for_ui_and_task);
        self.push(prefix);

        // Spawn the actual streaming work on our runtime so the TUI loop is not blocked.
        if let Some(rt) = &self.runtime {
            let llm = self.llm.clone();
            // Practical chat system prompt (lighter than the strict reviewer prompts used in gates).
            let system = "You are a thoughtful technical thought partner helping with vibe-driven coding. \
                          Keep answers practical and concrete. When suggesting code, be precise and include \
                          only the relevant snippet or file context. The user is working inside Anvil's \
                          Talk → Plan (R1+R2) → phased build flow.".to_string();

            // Inject active file context (from /include) so the model has real code to work with.
            // This is the entry point for "agentic awareness" while the hard gates (exactly two diverse reviews,
            // disk truth, explicit accept) remain fully enforced by the rest of the system.
            let mut user = text.to_string();
            if !self.active_context.is_empty() {
                user.push_str("\n\n--- BEGIN PROJECT CONTEXT (files you asked to include) ---\n");
                let mut budget: usize = 12_000; // soft total budget to keep prompts reasonable on first cut
                for (rel, content) in &self.active_context {
                    if budget == 0 {
                        break;
                    }
                    let mut to_send = content.clone();
                    if to_send.len() > budget {
                        to_send.truncate(budget);
                        to_send.push_str("\n... [truncated for prompt size]");
                    }
                    user.push_str(&format!(
                        "\n--- {} ---\n```\n{}\n```\n",
                        rel.display(),
                        to_send
                    ));
                    budget = budget.saturating_sub(to_send.len());
                }
                user.push_str("--- END PROJECT CONTEXT ---\n");
                user.push_str("Use the above file contents to give accurate, grounded answers and concrete code suggestions.");
            }

            rt.spawn(async move {
                // The tx is consumed by the call; when the future ends the sender is dropped and
                // the receiver in the UI loop will observe disconnect (stream finished).
                let _ = llm
                    .chat_stream_to_channel(&conn_for_task, &model_for_ui_and_task, &key_for_task, &system, &user, tx)
                    .await;
            });
        } else {
            self.push_system("(internal) no runtime available for LLM task");
        }
    }

    /// Drain any pending token deltas from the current LLM stream and append them
    /// to the last message (the active streaming assistant response). Called frequently
    /// from the event loop so text appears live without blocking crossterm poll.
    fn drain_llm_stream(&mut self) -> bool {
        let mut changed = false;
        if let Some(rx) = &mut self.llm_rx {
            loop {
                match rx.try_recv() {
                    Ok(delta) => {
                        if let Some(last) = self.messages.last_mut() {
                            last.push_str(&delta);
                        }
                        changed = true;
                    }
                    Err(mpsc::error::TryRecvError::Empty) => break,
                    Err(mpsc::error::TryRecvError::Disconnected) => {
                        // Stream finished for this response. Leave the accumulated text as-is.
                        // Clear the receiver so we don't keep checking a dead channel.
                        self.llm_rx = None;
                        // Ensure the line ends neatly if the model didn't send a trailing newline.
                        if let Some(last) = self.messages.last_mut() {
                            if !last.ends_with('\n') {
                                last.push('\n');
                            }
                        }
                        changed = true;
                        break;
                    }
                }
            }
        }
        changed
    }

    /// Inspect on-disk artifacts (plan.md + REVIEW_plan_R*.md + .anvil/state.json) and derive
    /// the authoritative WorkflowStage. Called on startup, after quick setup, and after gates.
    /// This is what makes the TUI "source of truth = files" and prevents bypassing the two-review rule.
    fn reconcile_stage_from_disk(&mut self) {
        if !self.is_configured() {
            self.stage = WorkflowStage::Unconfigured;
            return;
        }

        let plan_path = self.root.join("plan.md");
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

    /// Drain one-shot gate signals (from the spawn_blocking plan run).
    /// On completion we surface the *exact* on-disk review files into the chat so the user sees R1/R2
    /// with their canonical headers, exactly as written by the shared plan.rs logic.
    fn drain_gate_events(&mut self) -> bool {
        let mut changed = false;
        if let Some(rx) = &mut self.gate_rx {
            while let Ok(msg) = rx.try_recv() {
                if msg == "GATE_DONE" {
                    self.push_system("✓ Plan gate complete. plan.md + REVIEW_plan_R1.md + R2.md written (bit-identical to `anvil plan`).");
                    // Surface the real artifacts (headers + findings) for the user to read in context.
                    // With the new rich renderer these will display with code cards, bold headers, etc. (Cline-quality reading).
                    let rev_dir = reviews_dir(&self.root);
                    for round in ["R1", "R2"] {
                        let p = rev_dir.join(format!("REVIEW_plan_{}.md", round));
                        if let Ok(content) = std::fs::read_to_string(&p) {
                            self.push(format!("[review {}]\n{}", round, content));
                        }
                    }
                    self.reconcile_stage_from_disk();
                    self.update_status();
                    self.push_system("Use /view-reviews (or /view-plan) for a focused card view of the two independent reviews. Address findings, then /accept-plan.");
                    changed = true;
                } else if msg.starts_with("GATE_ERROR") {
                    self.push_system(&format!("Plan gate failed: {}", msg));
                    self.reconcile_stage_from_disk();
                    self.update_status();
                    changed = true;
                }
                self.gate_rx = None;
                break;
            }
        }
        changed
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
            binding_provider: None,
            binding_name: None,
            model: None,
            note: None,
            current_role: None,
        };

        self.push_system("=== CONFIGURATION WIZARD ===");
        self.push_system("Scroll lists with ↑/↓, Enter to pick, Esc to go back/cancel, or type answers for text fields.");
        self.push_system("All changes are saved to anvil.toml + keyring (when you choose keyring).");

        if self.first_run {
            self.push_system("Welcome! A 60-second setup gets you chatting with a real model and using the full Talk → /plan (R1 + R2) workflow.");
            self.push_system("Tip: the top menu choice is the fastest on-ramp (local Ollama, zero secrets). Arrow to it and hit Enter.");
        }

        self.config_wizard = Some(w);
        self.populate_main_menu();
        self.update_status();
    }

    fn populate_main_menu(&mut self) {
        if let Some(w) = &mut self.config_wizard {
            w.step = WizardStep::MainMenu;
            if self.first_run {
                // Prominent fast path for "someone off the street" — one choice and they are done.
                w.list_items = vec![
                    "1. Quick local Ollama setup (recommended first try — no keys, ~10 seconds)".to_string(),
                    "2. Add / update a provider connection (OpenAI, Anthropic, Azure, AWS, Groq, Ollama, ... )".to_string(),
                    "3. Add a model binding".to_string(),
                    "4. Assign roles (coder, planner, reviewer-a, reviewer-b — keep the two reviewers on different bindings)".to_string(),
                    "5. Show current configuration".to_string(),
                    "6. Finish & return to chat".to_string(),
                ];
                w.list_selected = 0;
                w.list_title = "First-time setup — ↑↓ to the top item then Enter (or just type 1). This is the simplest on-ramp.".to_string();
            } else {
                w.list_items = vec![
                    "Add / update a provider connection".to_string(),
                    "Add a model binding".to_string(),
                    "Assign roles (coder, planner, reviewer-a, reviewer-b)".to_string(),
                    "Show current configuration".to_string(),
                    "Finish & return to chat".to_string(),
                ];
                w.list_selected = 0;
                w.list_title = "What would you like to do? (↑↓ Enter)".to_string();
            }
        }
        if self.first_run {
            self.push_system("Main menu — first-run mode. Pick 1 for instant local setup or arrow + Enter.");
        } else {
            self.push_system("Main menu — use arrows then Enter, or type a number 1-5.");
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
                    | WizardStep::RoleAssignment { .. }
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
                // First-run has a prominent Quick Ollama choice as #1. Handle it specially
                // and auto-finish the wizard so the user lands straight back in chat ready to code.
                if self.first_run && (s == "1" || s.contains("quick") || s.contains("ollama")) {
                    if let Err(e) = self.do_quick_ollama_setup() {
                        self.push_system(&format!("Quick setup error: {}", e));
                    }
                    // do_quick_ollama_setup already sets first_run=false and pushes success details.
                    self.finish_config_wizard();
                } else if (s == "1" || s == "2") || s.contains("provider") {
                    // Covers normal (provider=1) and first-run (provider=2) layouts.
                    self.start_add_provider();
                } else if (s == "2" || s == "3") || s.contains("binding") || s.contains("model") {
                    self.start_add_binding();
                } else if (s == "3" || s == "4") || s.contains("role") || s.contains("assign") {
                    self.start_role_assignment();
                } else if (s == "4" || s == "5") || s.contains("show") || s.contains("current") {
                    self.show_current_config();
                    self.populate_main_menu();
                } else if (s == "5" || s == "6") || s.contains("finish") || s.contains("return") || s.contains("done") {
                    self.finish_config_wizard();
                } else {
                    self.push_system("Please choose a number or use the arrow keys + Enter on the list.");
                }
            }

            Some(WizardStep::ProviderType) => {
                let ptype = effective.trim();
                if ptype.is_empty() {
                    return;
                }
                if let Some(w) = &mut self.config_wizard {
                    w.provider_type = Some(ptype.to_string());
                    w.step = WizardStep::ProviderName;
                    w.list_items.clear();
                    w.list_title.clear();
                }
                self.push_system(&format!("Provider type set to '{}'.", ptype));
                self.push_system("Enter a short name for this connection (e.g. local-ollama, my-anthropic, azure-east):");
            }

            Some(WizardStep::ProviderName) => {
                let name = effective.trim();
                if name.is_empty() {
                    return;
                }
                let default = if let Some(w) = &self.config_wizard {
                    match w.provider_type.as_deref() {
                        Some("openai_compat") | Some("azure_openai") => "https://api.openai.com/v1",
                        _ => "",
                    }
                } else {
                    ""
                }
                .to_string();

                if let Some(w) = &mut self.config_wizard {
                    w.provider_name = Some(name.to_string());
                    w.base_url = if default.is_empty() { None } else { Some(default.clone()) };
                    w.step = WizardStep::BaseUrl;
                    w.list_items.clear();
                    w.list_title.clear();
                }
                self.push_system(&format!("Connection name: {}", name));
                if !default.is_empty() {
                    self.push_system(&format!("Enter base URL (default: {}) — press Enter to accept:", default));
                } else {
                    self.push_system("Enter base URL (or leave empty for provider default):");
                }
            }

            Some(WizardStep::BaseUrl) => {
                let url = effective.trim();
                if let Some(w) = &mut self.config_wizard {
                    if !url.is_empty() {
                        w.base_url = Some(url.to_string());
                    }
                }
                self.start_credential_list();
            }

            Some(WizardStep::CredentialKind) => {
                let kind = effective.to_lowercase();
                if kind.contains("keyring") || kind == "1" {
                    if let Some(w) = &mut self.config_wizard {
                        w.cred_kind = Some("keyring".to_string());
                        w.step = WizardStep::ApiKeySecret;
                        w.list_items.clear();
                        w.list_title.clear();
                    }
                    self.input_secret = true;
                    self.push_system("Using OS keyring (recommended).");
                    self.push_system("Paste or type the API key / token now (input will be hidden):");
                } else if kind.contains("no auth") || kind.contains("none") || kind == "3" {
                    if let Some(w) = &mut self.config_wizard {
                        w.cred_kind = Some("none".to_string());
                        w.list_items.clear();
                        w.list_title.clear();
                    }
                    self.push_system("No authentication required for this provider.");
                    self.finish_add_provider();
                } else {
                    if let Some(w) = &mut self.config_wizard {
                        w.cred_kind = Some("env".to_string());
                        w.step = WizardStep::EnvVarName;
                        w.list_items.clear();
                        w.list_title.clear();
                    }
                    self.push_system("Using environment variable.");
                    self.push_system("Enter the environment variable name (e.g. ANTHROPIC_API_KEY):");
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
                if let Some(w) = &mut self.config_wizard {
                    w.binding_provider = Some(prov.to_string());
                    w.step = WizardStep::BindingName;
                    w.list_items.clear();
                    w.list_title.clear();
                }
                self.push_system(&format!("Using provider connection '{}'.", prov));
                self.push_system("Enter a logical name for this binding (e.g. coder-claude, llama3-reviewer, gpt4-writer):");
            }

            Some(WizardStep::BindingName) => {
                let bname = effective.trim();
                if bname.is_empty() {
                    return;
                }
                if let Some(w) = &mut self.config_wizard {
                    w.binding_name = Some(bname.to_string());
                    w.step = WizardStep::ModelName;
                    w.list_items.clear();
                    w.list_title.clear();
                }
                self.push_system(&format!("Binding name: {}", bname));
                self.push_system("Enter the exact model identifier the provider expects (e.g. llama3.2, claude-3-5-sonnet-20241022, gpt-4o):");
            }

            Some(WizardStep::ModelName) => {
                let model = effective.trim();
                if model.is_empty() {
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
                let binding = effective.trim();
                if binding.is_empty() {
                    return;
                }

                if let Some(cfg) = &mut self.cfg {
                    match role.as_str() {
                        "coder" => cfg.roles.coder = Some(binding.to_string()),
                        "planner" => cfg.roles.planner = Some(binding.to_string()),
                        "reviewer_a" => cfg.roles.reviewer_a = Some(binding.to_string()),
                        "reviewer_b" => cfg.roles.reviewer_b = Some(binding.to_string()),
                        _ => {}
                    }
                }
                self.push_system(&format!("Set {} → {}", role, binding));

                let next_role = match role.as_str() {
                    "coder" => Some("planner".to_string()),
                    "planner" => Some("reviewer_a".to_string()),
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
                            self.push_system("You can now type to chat with the planner (or coder).");
                            self.push_system("Run /plan to generate a plan, then automatically get exactly R1 + R2 reviews from two different model bindings.");
                            self.push_system("This is the simple structured workflow that keeps vibe coding from drifting — valuable for beginners and hardcore users alike.");
                        }
                    }
                    self.populate_main_menu();
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
            w.list_items = vec![
                "openai_compat  (Ollama, Groq, Together, Fireworks, OpenRouter, Azure compat, vLLM, ...)".to_string(),
                "anthropic".to_string(),
                "google".to_string(),
                "azure_openai (native Azure OpenAI)".to_string(),
                "aws_bedrock    (via Bedrock or gateway)".to_string(),
                "other (enter any provider type string)".to_string(),
            ];
            w.list_selected = 0;
            w.list_title = "Choose provider type (↑↓ Enter or type)".to_string();
        }
        self.push_system("Adding a provider connection.");
        self.push_system("Select the type of provider (this determines how we call the API).");
    }

    fn start_credential_list(&mut self) {
        if let Some(w) = &mut self.config_wizard {
            w.step = WizardStep::CredentialKind;
            w.list_items = vec![
                "1. Store in OS keyring (recommended — secure, works everywhere)".to_string(),
                "2. Environment variable (you will set the var yourself)".to_string(),
                "3. No authentication required (local Ollama, unauthenticated self-hosted, etc.)".to_string(),
            ];
            w.list_selected = 0;
            w.list_title = "How will the API key be provided?".to_string();
        }
        self.push_system("Choose how the credential will be supplied for this provider.");
    }

    fn finish_add_provider(&mut self) {
        let (ptype, name, base, cred_kind, api_key, env_var) = if let Some(w) = &self.config_wizard {
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

        let cred = if cred_kind.as_deref() == Some("keyring") {
            if let Some(key) = &api_key {
                let entry_name = format!("provider:{}", name);
                match keyring::Entry::new("anvil", &entry_name) {
                    Ok(entry) => {
                        if let Err(e) = entry.set_password(key) {
                            self.push_system(&format!("Warning: could not store key in keyring: {}", e));
                        } else {
                            self.push_system("✓ Key stored securely in OS keyring.");
                        }
                    }
                    Err(e) => {
                        self.push_system(&format!("Warning: keyring unavailable ({}). Falling back to env var.", e));
                    }
                }
            }
            CredentialRef::Keyring
        } else if cred_kind.as_deref() == Some("none") {
            CredentialRef::None
        } else {
            CredentialRef::Env {
                var_name: env_var.unwrap_or_else(|| "API_KEY".to_string()),
            }
        };

        let cfg = self.cfg.get_or_insert_with(AnvilConfig::default);

        let normalized_type = if ptype.starts_with("openai_compat") {
            "openai_compat".to_string()
        } else if ptype.starts_with("azure") {
            "azure_openai".to_string()
        } else if ptype.starts_with("aws") {
            "aws_bedrock".to_string()
        } else {
            ptype.clone()
        };

        cfg.providers.insert(
            name.clone(),
            ProviderConnection {
                r#type: normalized_type,
                base_url: base,
                credential: cred,
                extra: Default::default(),
            },
        );

        self.save_current_config();
        self.push_system(&format!("✓ Provider '{}' saved (type={}).", name, ptype));

        // Immediately offer to create a binding for the new provider
        if let Some(w) = &mut self.config_wizard {
            w.binding_provider = Some(name);
        }
        self.start_add_binding();
    }

    fn start_add_binding(&mut self) {
        let cfg = self.cfg.get_or_insert_with(AnvilConfig::default);
        let prov_names: Vec<String> = cfg.providers.keys().cloned().collect();

        if prov_names.is_empty() {
            self.push_system("No providers configured yet. Add a provider first.");
            self.populate_main_menu();
            return;
        }

        if let Some(w) = &mut self.config_wizard {
            w.step = WizardStep::BindingProvider;
            w.list_items = prov_names;
            w.list_selected = 0;
            w.list_title = "Which provider connection should this binding use?".to_string();
            w.binding_name = None;
            w.model = None;
            w.note = None;
        }
        self.push_system("Adding a model binding (logical name → specific model on a provider).");
    }

    fn finish_add_binding(&mut self) {
        let (bname, prov, model, note) = if let Some(w) = &self.config_wizard {
            (
                w.binding_name.clone().unwrap_or_default(),
                w.binding_provider.clone().unwrap_or_default(),
                w.model.clone().unwrap_or_default(),
                w.note.clone(),
            )
        } else {
            return;
        };

        let cfg = self.cfg.get_or_insert_with(AnvilConfig::default);

        if bname.is_empty() || model.is_empty() || prov.is_empty() {
            self.push_system("Binding incomplete — cancelling.");
            self.populate_main_menu();
            return;
        }

        cfg.model_bindings.insert(
            bname.clone(),
            ModelBinding {
                provider: prov.clone(),
                model: model.clone(),
                note,
            },
        );

        self.save_current_config();
        self.push_system(&format!("✓ Binding '{}' → {} via {} saved.", bname, model, prov));

        self.populate_main_menu();
    }

    fn start_role_assignment(&mut self) {
        let cfg = self.cfg.get_or_insert_with(AnvilConfig::default);
        if cfg.model_bindings.is_empty() {
            self.push_system("No model bindings yet. Add at least one binding before assigning roles.");
            self.populate_main_menu();
            return;
        }
        self.start_role_list("coder");
    }

    fn start_role_list(&mut self, role: &str) {
        let cfg = self.cfg.get_or_insert_with(AnvilConfig::default);
        let binding_names: Vec<String> = cfg.model_bindings.keys().cloned().collect();

        if let Some(w) = &mut self.config_wizard {
            w.step = WizardStep::RoleAssignment { role: role.to_string() };
            w.list_items = binding_names;
            w.list_selected = 0;
            w.current_role = Some(role.to_string());
            w.list_title = format!(
                "Choose binding for role '{}' (make reviewer_a and reviewer_b different)",
                role
            );
        }

        self.push_system(&format!("Assigning role: {}", role));
        self.push_system("Select a model binding from the list (↑↓ then Enter).");
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
            WizardStep::BindingName => WizardStep::BindingProvider,
            WizardStep::ModelName => WizardStep::BindingName,
            WizardStep::BindingNote => WizardStep::ModelName,
            WizardStep::RoleAssignment { role } => {
                match role.as_str() {
                    "planner" => WizardStep::RoleAssignment { role: "coder".to_string() },
                    "reviewer_a" => WizardStep::RoleAssignment { role: "planner".to_string() },
                    "reviewer_b" => WizardStep::RoleAssignment { role: "reviewer_a".to_string() },
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
            _ => {
                self.populate_main_menu();
                self.push_system("(back)");
                return;
            }
        };

        // Now take a short-lived mutable borrow to apply the back step + update lists/input state.
        if let Some(w) = &mut self.config_wizard {
            w.step = prev;

            // Rebuild the list (if any) for the step we just moved to. We do this silently
            // (no "Adding ..." progress messages that the start_* helpers emit on forward entry).
            match &w.step {
                WizardStep::ProviderType => {
                    w.list_items = vec![
                        "openai_compat  (Ollama, Groq, Together, Fireworks, OpenRouter, Azure compat, vLLM, ...)".to_string(),
                        "anthropic".to_string(),
                        "google".to_string(),
                        "azure_openai (native Azure OpenAI)".to_string(),
                        "aws_bedrock    (via Bedrock or gateway)".to_string(),
                        "other (enter any provider type string)".to_string(),
                    ];
                    w.list_selected = 0;
                    w.list_title = "Choose provider type (↑↓ Enter or type)".to_string();
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
                WizardStep::RoleAssignment { role } => {
                    let cfg = self.cfg.get_or_insert_with(AnvilConfig::default);
                    w.list_items = cfg.model_bindings.keys().cloned().collect();
                    w.list_selected = 0;
                    w.current_role = Some(role.clone());
                    w.list_title = format!(
                        "Choose binding for role '{}' (make reviewer_a and reviewer_b different)",
                        role
                    );
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
                WizardStep::BindingName => {
                    self.input = w.binding_name.clone().unwrap_or_default();
                }
                WizardStep::ModelName => {
                    self.input = w.model.clone().unwrap_or_default();
                }
                WizardStep::BindingNote => {
                    self.input = w.note.clone().unwrap_or_default();
                }
                _ => {}
            }

            // Best-effort: when landing on a list step, highlight the item that corresponds to a
            // previously made choice (if any) instead of always starting at index 0.
            match &w.step {
                WizardStep::ProviderType => {
                    if let Some(pt) = &w.provider_type {
                        if let Some(idx) = w
                            .list_items
                            .iter()
                            .position(|s| s.to_lowercase().starts_with(&pt.to_lowercase()) || s.contains(pt))
                        {
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
                            if let Some(idx) = w.list_items.iter().position(|s| s == name) {
                                w.list_selected = idx;
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        self.push_system("(back)");
    }

    fn show_current_config(&mut self) {
        let lines = if let Some(cfg) = &self.cfg {
            let mut out = vec![
                "--- Current Configuration ---".to_string(),
                format!(
                    "Roles: coder={} planner={} reviewer_a={} reviewer_b={}",
                    cfg.roles.coder.as_deref().unwrap_or("(none)"),
                    cfg.roles.planner.as_deref().unwrap_or("(none)"),
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
                out.push(format!("  {} (type={}, base={}, {})", name, p.r#type, base, auth));
            }
            out.push("Model Bindings:".to_string());
            for (name, b) in &cfg.model_bindings {
                let note = b.note.as_deref().map(|n| format!(" ({})", n)).unwrap_or_default();
                out.push(format!("  {} → {} via {}{}", name, b.model, b.provider, note));
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
                self.push_system("Setup complete! You can now type to chat with the planner/coder.");
                self.push_system("Use /plan to run the Talk → plan + R1 review + R2 review gate (exactly two diverse reviewers).");
                self.push_system("The workflow is deliberately simple to start yet powerful enough for serious use: structure that prevents drift without killing velocity.");
            } else {
                self.push_system("Configuration wizard finished. Changes saved to anvil.toml (and keyring where used).");
                self.push_system("You can now chat with the planner/coder and run /plan for the gate.");
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
        if let Some(cfg) = &self.cfg {
            if let Err(e) = save_config(&self.root, cfg) {
                self.push_system(&format!("Warning: could not save config: {}", e));
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
        let _chat = app.drain_llm_stream();
        let _gate = app.drain_gate_events();
        app.anim_tick = app.anim_tick.wrapping_add(1);

        terminal.draw(|f| render_ui(f, app))?;

        if event::poll(std::time::Duration::from_millis(80))? {
            if let Event::Key(key) = event::read()? {
                // Only act on real presses and OS-generated repeats.
                // Ignore Release events (crossterm 0.28+ on Windows commonly emits both
                // Press + Release for the same physical key, which was causing every
                // character to be inserted twice into the input buffer).
                if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
                    if handle_key(app, key)? {
                        break;
                    }
                }
            }
        }

        if app.should_quit {
            break;
        }
    }
    Ok(())
}

fn handle_key(app: &mut App, key: event::KeyEvent) -> Result<bool> {
    // Any keypress dismisses the splash screen.
    if app.splash_ticks > 0 {
        app.splash_ticks = 0;
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
                    app.input = (*cmd).to_string();
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
                | WizardStep::RoleAssignment { .. }
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
        if key.code == KeyCode::Esc {
            app.viewing_doc = None;
            return Ok(false);
        }
        // For now the viewer is read-only display (content is also in the main chat log with rich code blocks).
        // Future: internal scroll offset per viewer if needed.
        return Ok(false);
    }

    match key.code {
        KeyCode::Esc if key.modifiers.is_empty() && app.config_wizard.is_none() => {
            // Only quit on Esc at the top level. When the config wizard (or other modal) is open,
            // Esc is handled above to go back one menu or exit the config menu.
            app.should_quit = true;
            return Ok(true);
        }
        KeyCode::Char('q') if key.modifiers.is_empty() => {
            // 'q' is always a hard quit (even from inside the config wizard).
            app.should_quit = true;
            return Ok(true);
        }
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.should_quit = true;
            return Ok(true);
        }

        KeyCode::Char('s') if app.first_run && key.modifiers.is_empty() => {
            // Quick first-run setup (addresses the review comment at plan.md:183-185)
            // Works even if the wizard is currently open — great escape hatch for the "just get me going" path.
            if let Err(e) = app.do_quick_ollama_setup() {
                app.push_system(&format!("Quick setup error: {}", e));
            }
            app.showing_command_palette = false;
            if app.config_wizard.is_some() {
                app.config_wizard = None;
                app.input_secret = false;
            }
            return Ok(false);
        }

        KeyCode::Enter => {
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                // Shift+Enter inserts a newline for multi-line input (the input box is several lines tall and auto-tails).
                app.input.push('\n');
                // Keep palette closed unless this is starting a command (unlikely with shift).
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
                app.input.push(ch);
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
            app.input.pop();
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

        KeyCode::Up => {
            if app.view_offset > 0 {
                app.view_offset -= 1;
            }
            return Ok(false);
        }
        KeyCode::Down => {
            if app.view_offset + 1 < app.messages.len() {
                app.view_offset += 1;
            }
            return Ok(false);
        }

        KeyCode::PageUp => {
            app.view_offset = app.view_offset.saturating_sub(10);
            return Ok(false);
        }
        KeyCode::PageDown => {
            let max = app.messages.len().saturating_sub(1);
            app.view_offset = (app.view_offset + 10).min(max);
            return Ok(false);
        }

        _ => {}
    }
    Ok(false)
}

fn render_ui(f: &mut Frame, app: &App) {
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
    let img_max_cols = area.width.saturating_sub(6);
    let img_max_rows = area.height.saturating_sub(10).max(8);
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
        Style::default().fg(Color::White).add_modifier(Modifier::ITALIC),
    )));
    lines.push(Line::from(Span::styled(
        "  Talk  →  Plan (R1+R2)  →  Build  →  Ship  ".to_string(),
        Style::default().fg(Color::Rgb(150, 200, 255)),
    )));

    lines.push(Line::from(Span::raw("".to_string())));

    let ver_line = format!("  v{}  —  model-agnostic, cross-provider  ", env!("CARGO_PKG_VERSION"));
    lines.push(Line::from(Span::styled(
        ver_line,
        Style::default().fg(Color::DarkGray),
    )));

    if let Some(cfg) = &app.cfg {
        let coder = splash_model_label(cfg, "coder");
        let r1    = splash_model_label(cfg, "reviewer-a");
        let r2    = splash_model_label(cfg, "reviewer-b");
        if coder != "—" || r1 != "—" {
            lines.push(Line::from(vec![
                Span::styled("  CODER ".to_string(), Style::default().fg(Color::Cyan)),
                Span::styled(coder, Style::default().fg(Color::White)),
                Span::styled("   R1 ".to_string(), Style::default().fg(Color::Magenta)),
                Span::styled(r1, Style::default().fg(Color::White)),
                Span::styled("   R2 ".to_string(), Style::default().fg(Color::Magenta)),
                Span::styled(r2, Style::default().fg(Color::White)),
                Span::raw("  ".to_string()),
            ]));
        }
    }

    lines.push(Line::from(Span::raw("".to_string())));

    // Pulsing dismiss hint
    let hint_color = if (app.anim_tick / 6) % 2 == 0 { Color::DarkGray } else { Color::Gray };
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
        let line_w = line.spans.iter().map(|s| s.content.chars().count()).sum::<usize>() as u16;
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

fn render_main(f: &mut Frame, app: &App) {
    let area = f.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // bordered header — 3 info rows
            Constraint::Min(4),    // chat log
            Constraint::Length(4), // bordered input — 2 content rows
        ])
        .split(area);

    render_header(f, app, chunks[0]);
    render_chat(f, app, chunks[1]);
    render_input_box(f, app, chunks[2]);

    // Overlays rendered last so they float on top
    render_palette_popup(f, app, chunks[1]);
    render_wizard_popup(f, app, chunks[1]);
    render_doc_popup(f, app, chunks[1]);
}

// ─── Header (3-row info panel) ────────────────────────────────────────────────

fn render_header(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    use ratatui::layout::Rect;

    // ── Row 0: brand • stage • streaming indicator • context badge ──
    let stage_text = if app.first_run || app.stage == WorkflowStage::Unconfigured {
        "UNCONFIGURED — /config or press s".to_string()
    } else {
        match app.stage {
            WorkflowStage::Talk                => "TALK — chat freely, then /plan".to_string(),
            WorkflowStage::PlanReviewsComplete => "REVIEWS DONE — address findings, then /accept-plan".to_string(),
            WorkflowStage::PlanAccepted        => "PLAN ACCEPTED — build phases".to_string(),
            WorkflowStage::Unconfigured        => "UNCONFIGURED".to_string(),
        }
    };
    let stage_color = if app.first_run || app.stage == WorkflowStage::Unconfigured {
        Color::Red
    } else {
        match app.stage {
            WorkflowStage::Talk                => Color::Yellow,
            WorkflowStage::PlanReviewsComplete => Color::Magenta,
            WorkflowStage::PlanAccepted        => Color::LightGreen,
            WorkflowStage::Unconfigured        => Color::Red,
        }
    };

    let is_streaming = app.llm_rx.is_some() || app.gate_rx.is_some();
    let stream_spans: Vec<Span<'static>> = if is_streaming {
        let sp = SPINNER[(app.anim_tick as usize / 2) % SPINNER.len()];
        vec![
            Span::raw("  ".to_string()),
            Span::styled(sp.to_string(), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled(" thinking…".to_string(), Style::default().fg(Color::Cyan)),
        ]
    } else {
        vec![
            Span::raw("  ".to_string()),
            Span::styled("ready".to_string(), Style::default().fg(Color::DarkGray)),
        ]
    };

    let mut row0: Vec<Span<'static>> = vec![
        Span::styled(
            " ANVIL ".to_string(),
            Style::default().fg(Color::Rgb(255, 180, 0)).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("v{}  ", env!("CARGO_PKG_VERSION")),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled("│ ".to_string(), Style::default().fg(Color::Rgb(60, 60, 80))),
        Span::styled(
            stage_text,
            Style::default().fg(stage_color).add_modifier(Modifier::BOLD),
        ),
    ];
    row0.extend(stream_spans);
    if !app.active_context.is_empty() {
        row0.push(Span::styled(
            format!("  [ctx:{}]", app.active_context.len()),
            Style::default().fg(Color::Rgb(100, 200, 255)),
        ));
    }

    // ── Row 1: coder / R1 / R2 model labels ──
    let row1: Vec<Span<'static>> = if let Some(cfg) = &app.cfg {
        let coder = header_model_label(cfg, "coder");
        let r1    = header_model_label(cfg, "reviewer-a");
        let r2    = header_model_label(cfg, "reviewer-b");
        vec![
            Span::styled(" CODER ".to_string(), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled(coder, Style::default().fg(Color::White)),
            Span::styled("  │  R1 ".to_string(), Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
            Span::styled(r1, Style::default().fg(Color::White)),
            Span::styled("  │  R2 ".to_string(), Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
            Span::styled(r2, Style::default().fg(Color::White)),
        ]
    } else {
        vec![Span::styled(
            " Run /config or press s for quick Ollama setup".to_string(),
            Style::default().fg(Color::Yellow),
        )]
    };

    // ── Row 2: project name + phase progress ──
    let proj = app.root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(".")
        .to_string();
    let phases = build_phase_progress(app);

    let row2: Vec<Span<'static>> = vec![
        Span::styled(" project ".to_string(), Style::default().fg(Color::DarkGray)),
        Span::styled(proj, Style::default().fg(Color::Gray)),
        Span::styled("  │  ".to_string(), Style::default().fg(Color::DarkGray)),
        Span::styled(phases, Style::default().fg(Color::Gray)),
    ];

    // Draw bordered block then overlay rows inside it
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(60, 60, 80)))
        .title(Span::styled(
            " ⬡ anvil ",
            Style::default().fg(Color::Rgb(255, 180, 0)).add_modifier(Modifier::BOLD),
        ));

    let inner = block.inner(area);
    f.render_widget(block, area);

    for (i, spans) in [row0, row1, row2].into_iter().enumerate() {
        let y = inner.y + i as u16;
        if y >= inner.y + inner.height {
            break;
        }
        let row_area = Rect { x: inner.x, y, width: inner.width, height: 1 };
        f.render_widget(Paragraph::new(Line::from(spans)), row_area);
    }
}

/// Short model label for splash screen.
fn splash_model_label(cfg: &crate::config::AnvilConfig, role: &str) -> String {
    if let Ok((name, binding, _)) = cfg.resolve_role_full(role) {
        let m = &binding.model;
        let m = if m.len() > 18 { &m[..18] } else { m };
        format!("{} ({})", name, m)
    } else {
        "—".to_string()
    }
}

/// Full model label for the header row.
fn header_model_label(cfg: &crate::config::AnvilConfig, role: &str) -> String {
    if let Ok((name, binding, provider)) = cfg.resolve_role_full(role) {
        let m = &binding.model;
        let m = if m.len() > 20 { &m[..20] } else { m };
        format!("{} ({} / {})", name, m, provider.r#type)
    } else {
        "not configured".to_string()
    }
}

/// Inline phase progress: `P0✓ P1→ P2○ P3○`
fn build_phase_progress(app: &App) -> String {
    let state = load_state(&app.root);
    let plan_path = app.root.join("plan.md");

    if !plan_path.exists() {
        return if state.accepted_plan_hash.is_some() {
            "plan: accepted".to_string()
        } else {
            "no plan yet — /plan to generate".to_string()
        };
    }

    let plan = std::fs::read_to_string(&plan_path).unwrap_or_default();
    let mut seen = std::collections::HashSet::new();
    let phase_ids: Vec<String> = plan
        .lines()
        .filter_map(|line| {
            let s = line.trim_start_matches('#').trim();
            if let Some(rest) = s.strip_prefix('P') {
                let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
                if !digits.is_empty() {
                    let id = format!("P{}", digits);
                    if seen.insert(id.clone()) { Some(id) } else { None }
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();

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

// ─── Chat area ───────────────────────────────────────────────────────────────

fn render_chat(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let chat_title = match app.stage {
        WorkflowStage::PlanReviewsComplete =>
            " Reviews ready — /view-reviews then /accept-plan (↑↓ scroll) ",
        WorkflowStage::PlanAccepted =>
            " Plan accepted — build phases (↑↓ / for commands) ",
        _ =>
            " Chat log (↑↓ scroll, Enter=send, Shift+Enter=newline, / for commands) ",
    };

    let border_color = if app.first_run || app.stage == WorkflowStage::Unconfigured {
        Color::Rgb(120, 80, 0)
    } else {
        match app.stage {
            WorkflowStage::PlanAccepted        => Color::Rgb(40, 120, 40),
            WorkflowStage::PlanReviewsComplete => Color::Rgb(100, 40, 120),
            _                                  => Color::Rgb(50, 50, 70),
        }
    };

    let chat_block = Block::default()
        .title(Span::styled(chat_title, Style::default().fg(Color::DarkGray)))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let visible: Vec<Line> = if app.messages.is_empty() {
        vec![Line::from(Span::styled(
            "(no messages yet — start typing or try /help)",
            Style::default().fg(Color::DarkGray),
        ))]
    } else {
        let start = app.view_offset.min(app.messages.len().saturating_sub(1));
        app.messages[start..]
            .iter()
            .flat_map(|m| App::render_message_as_lines(m))
            .collect()
    };

    let chat = Paragraph::new(visible)
        .block(chat_block)
        .wrap(Wrap { trim: false });
    f.render_widget(chat, area);
}

// ─── Input box ───────────────────────────────────────────────────────────────

fn render_input_box(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let (prompt, title) = if let Some(wizard) = &app.config_wizard {
        let p = match &wizard.step {
            WizardStep::ProviderName  => "provider name> ",
            WizardStep::BaseUrl       => "base url> ",
            WizardStep::EnvVarName    => "env var name> ",
            WizardStep::ApiKeySecret  => "api key (hidden)> ",
            WizardStep::BindingName   => "binding name> ",
            WizardStep::ModelName     => "model id> ",
            WizardStep::BindingNote   => "note (optional)> ",
            _                         => "config> ",
        };
        let t = if wizard.list_title.is_empty() {
            " Config wizard — type answer + Enter (Esc=back) ".to_string()
        } else {
            format!(" {} ", wizard.list_title)
        };
        (p, t)
    } else {
        ("> ", " Input (Enter=send, Shift+Enter=newline, /=commands, Esc/q=quit) ".to_string())
    };

    let display = if app.input_secret {
        "•".repeat(app.input.len())
    } else {
        app.input.clone()
    };

    let full_text = format!("{}{}", prompt, display);
    let inner_h = (area.height as usize).saturating_sub(2).max(1);
    let all_lines: Vec<&str> = full_text.lines().collect();
    let start = if all_lines.len() > inner_h { all_lines.len() - inner_h } else { 0 };
    let visible_text = if start > 0 { all_lines[start..].join("\n") } else { full_text };

    let border_color = if app.config_wizard.is_some() {
        Color::Yellow
    } else {
        Color::Rgb(60, 80, 100)
    };

    // Cursor blink: bright when on, dim when off
    let cursor_on = (app.anim_tick / 7) % 2 == 0;
    let text_style = if cursor_on && app.config_wizard.is_none() {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::White)
    };

    let input_widget = Paragraph::new(visible_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color))
                .title(Span::styled(title, Style::default().fg(Color::DarkGray))),
        )
        .style(text_style)
        .wrap(Wrap { trim: false });
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

    let max_h: u16 = 12;
    let needed = (filtered.len() as u16) + 2;
    let h = needed.min(max_h).min(chat_area.height.saturating_sub(1)).max(3);
    let popup = ratatui::layout::Rect {
        x: chat_area.x + 2,
        y: chat_area.y + chat_area.height.saturating_sub(h),
        width: chat_area.width.saturating_sub(4),
        height: h,
    };

    f.render_widget(Clear, popup);

    let items: Vec<ListItem> = filtered
        .iter()
        .enumerate()
        .map(|(i, &(cmd, desc))| {
            if i == app.command_selected {
                ListItem::new(Line::from(vec![
                    Span::styled(
                        format!(" {} ", cmd),
                        Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD),
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
                Style::default().fg(Color::DarkGray),
            )),
    );
    f.render_widget(list, popup);
}

fn render_wizard_popup(f: &mut Frame, app: &App, chat_area: ratatui::layout::Rect) {
    let wizard = match &app.config_wizard {
        Some(w) if !w.list_items.is_empty() => w,
        _ => return,
    };

    let max_h: u16 = 12;
    let needed = (wizard.list_items.len() as u16) + 2;
    let h = needed.min(max_h).min(chat_area.height.saturating_sub(1)).max(3);
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
        .enumerate()
        .map(|(i, item)| {
            if i == wizard.list_selected {
                ListItem::new(Line::from(Span::styled(
                    format!(" ▶ {} ", item),
                    Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD),
                )))
            } else {
                ListItem::new(Line::from(Span::styled(
                    format!("   {} ", item),
                    Style::default().fg(Color::Yellow),
                )))
            }
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow))
            .title(Span::styled(
                format!(" {} ", wizard.list_title),
                Style::default().fg(Color::DarkGray),
            )),
    );
    f.render_widget(list, popup);
}

fn render_doc_popup(f: &mut Frame, app: &App, chat_area: ratatui::layout::Rect) {
    let (title, content) = match &app.viewing_doc {
        Some(pair) => pair,
        None => return,
    };

    let h = (chat_area.height.saturating_sub(4)).max(8).min(30);
    let popup = ratatui::layout::Rect {
        x: chat_area.x + 3,
        y: chat_area.y + 2,
        width: chat_area.width.saturating_sub(6),
        height: h,
    };

    f.render_widget(Clear, popup);

    let lines: Vec<Line> = content
        .lines()
        .flat_map(|l| App::render_message_as_lines(&format!("[doc] {}", l)))
        .collect();

    let viewer = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Magenta))
                .title(Span::styled(
                    format!(" {} (Esc to close) ", title),
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                )),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(viewer, popup);
}