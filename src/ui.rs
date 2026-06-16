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

use anyhow::Result;
use chrono::Utc;
use tokio::sync::mpsc;
use uuid::Uuid;
use crossterm::{
    event::{self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};

use crate::config::{
    ensure_anvil_dir, load_config, load_local_env, save_config, set_local_env_var, AnvilConfig,
    CredentialRef, ModelBinding, ProviderConnection,
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
    ("/loaded", "/ps /ollama-ps — list Ollama models currently in VRAM + sizes (cross-check vs live GPU header)"),
    ("/unload [model]", "Force immediate unload (keep_alive=0) of one or all loaded models; header GPU stats refresh"),
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

/// Role-specific colors used consistently for labels, headers, splash, chat prefixes,
/// and during quick-setup model picking. Coder=blue, R1=purple (magenta), R2=lime (bright green).
const ROLE_CODER: Color = Color::LightBlue;
const ROLE_R1: Color = Color::Magenta;
const ROLE_R2: Color = Color::Rgb(50, 255, 127);

/// Known providers: (display_name, suggested_connection_name, provider_type, base_url, needs_api_key)
/// base_url = "" means the client uses the provider's SDK default (anthropic, google).
const PROVIDER_PRESETS: &[(&str, &str, &str, &str, bool)] = &[
    ("Anthropic",          "anthropic",  "anthropic",     "",                                          true),
    ("OpenAI",             "openai",     "openai_compat", "https://api.openai.com/v1",                 true),
    ("xAI",                "xai",        "openai_compat", "https://api.x.ai/v1",                       true),
    ("Google",             "google",     "google",        "",                                          true),
    ("Groq",               "groq",       "openai_compat", "https://api.groq.com/openai/v1",            true),
    ("Mistral",            "mistral",    "openai_compat", "https://api.mistral.ai/v1",                 true),
    ("Together AI",        "together",   "openai_compat", "https://api.together.xyz/v1",               true),
    ("OpenRouter",         "openrouter", "openai_compat", "https://openrouter.ai/api/v1",              true),
    ("Fireworks",          "fireworks",  "openai_compat", "https://api.fireworks.ai/inference/v1",     true),
    ("Perplexity",         "perplexity", "openai_compat", "https://api.perplexity.ai",                 true),
    ("DeepSeek",           "deepseek",   "openai_compat", "https://api.deepseek.com",                  true),
    ("Cohere",             "cohere",     "openai_compat", "https://api.cohere.com/v2",                 true),
    ("Azure",              "azure",      "azure_openai",  "",                                          true),
    ("AWS",                "aws",        "openai_compat", "",                                          true),
    ("Vertex AI",          "vertex",     "openai_compat", "",                                          true),
    ("Gradient",           "gradient",   "openai_compat", "",                                          true),
    ("Ollama (local)",     "ollama",     "openai_compat", "http://localhost:11434/v1",                 false),
    ("LM Studio (local)",  "lmstudio",   "openai_compat", "http://localhost:1234/v1",                  false),
    ("Other / custom",     "custom",     "openai_compat", "",                                          true),
];

/// Suggest a conventional environment variable name for a provider connection.
/// Used so that when the user pastes a key we can auto `std::env::set_var` it (current process),
/// store CredentialRef::Env, and give the user exact `setx` / profile instructions.
/// Prioritizes well-known names (XAI_API_KEY etc.) so tools and user scripts stay compatible.
fn suggest_env_var_name(conn_name: &str, base_url: Option<&str>) -> String {
    let n = conn_name.to_lowercase();
    if n == "xai" || n.contains("xai") { return "XAI_API_KEY".to_string(); }
    if n == "openai" || n.contains("openai") { return "OPENAI_API_KEY".to_string(); }
    if n.contains("groq") { return "GROQ_API_KEY".to_string(); }
    if n.contains("anthropic") { return "ANTHROPIC_API_KEY".to_string(); }
    if n.contains("mistral") { return "MISTRAL_API_KEY".to_string(); }
    if n.contains("together") { return "TOGETHER_API_KEY".to_string(); }
    if n.contains("fireworks") { return "FIREWORKS_API_KEY".to_string(); }
    if n.contains("perplexity") { return "PERPLEXITY_API_KEY".to_string(); }
    if n.contains("deepseek") { return "DEEPSEEK_API_KEY".to_string(); }
    if n.contains("cohere") { return "COHERE_API_KEY".to_string(); }
    if n.contains("azure") { return "AZURE_OPENAI_API_KEY".to_string(); }
    if n.contains("google") || n.contains("gemini") || n.contains("vertex") { return "GOOGLE_API_KEY".to_string(); }
    if n.contains("aws") { return "AWS_BEDROCK_API_KEY".to_string(); }
    if n.contains("ollama") { return "OLLAMA_API_KEY".to_string(); }
    if n.contains("lmstudio") || n.contains("lm-studio") { return "LMSTUDIO_API_KEY".to_string(); }

    // Generic fallback based on the connection name the user chose (e.g. "my-xai" -> MY_XAI_API_KEY)
    let base = if let Some(u) = base_url { u } else { "" };
    let mut stem = conn_name.to_uppercase();
    stem = stem.replace(|c: char| !c.is_alphanumeric(), "_");
    stem = stem.trim_matches('_').to_string();
    if stem.is_empty() {
        stem = "ANVIL".to_string();
    }
    if base.contains("x.ai") && !stem.contains("XAI") { stem = "XAI".to_string(); }
    format!("{}_API_KEY", stem)
}

fn models_for_connection(provider_type: &str, base_url: Option<&str>) -> &'static [&'static str] {
    let url = base_url.unwrap_or("");
    if url.contains("x.ai") {
        // Static suggestions as last-resort fallback only. The live /v1/models path (when the
        // provider connection + key are valid) should return the current catalog for the key.
        return &["grok-3", "grok-3-fast", "grok-3-mini", "grok-3-mini-fast", "grok-2-1212", "grok-beta", "grok-4.3", "grok-4.2", "grok-build-0.1"];
    }
    if url.contains("groq.com") {
        return &["llama-3.3-70b-versatile", "llama-3.1-70b-versatile", "llama-3.1-8b-instant", "mixtral-8x7b-32768", "gemma2-9b-it", "llama-guard-3-8b"];
    }
    if url.contains("mistral.ai") {
        return &["mistral-large-latest", "mistral-medium-latest", "mistral-small-latest", "codestral-latest", "open-mistral-nemo", "open-codestral-mamba"];
    }
    if url.contains("together.xyz") {
        return &["meta-llama/Llama-3.3-70B-Instruct-Turbo", "meta-llama/Meta-Llama-3.1-8B-Instruct-Turbo", "Qwen/Qwen2.5-72B-Instruct-Turbo", "deepseek-ai/DeepSeek-R1", "mistralai/Mixtral-8x7B-Instruct-v0.1"];
    }
    if url.contains("openrouter.ai") {
        return &["anthropic/claude-opus-4-8", "anthropic/claude-sonnet-4-6", "openai/gpt-4o", "google/gemini-2.5-pro-preview", "meta-llama/llama-3.3-70b-instruct", "deepseek/deepseek-r1"];
    }
    if url.contains("fireworks.ai") {
        return &["accounts/fireworks/models/llama-v3p3-70b-instruct", "accounts/fireworks/models/llama-v3p1-405b-instruct", "accounts/fireworks/models/mixtral-8x7b-instruct", "accounts/fireworks/models/qwen2p5-72b-instruct"];
    }
    if url.contains("perplexity.ai") {
        return &["llama-3.1-sonar-large-128k-online", "llama-3.1-sonar-small-128k-online", "llama-3.1-sonar-huge-128k-online"];
    }
    if url.contains("deepseek.com") {
        return &["deepseek-chat", "deepseek-coder", "deepseek-reasoner"];
    }
    if url.contains("cohere.com") {
        return &["command-r-plus", "command-r", "command-light", "command"];
    }
    if url.contains("openai.com") {
        return &["gpt-4o", "gpt-4o-mini", "gpt-4-turbo", "gpt-4", "gpt-3.5-turbo", "o1-preview", "o1-mini", "o3-mini"];
    }
    match provider_type {
        "anthropic" => &["claude-opus-4-8", "claude-sonnet-4-6", "claude-haiku-4-5-20251001", "claude-3-5-sonnet-20241022", "claude-3-5-haiku-20241022", "claude-3-opus-20240229"],
        "google" => &["gemini-2.5-pro-preview-06-05", "gemini-2.5-flash-preview-05-20", "gemini-2.0-flash", "gemini-1.5-pro", "gemini-1.5-flash"],
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
    RoleAssignment { role: String },
    // Special first-run quick Ollama path: after auto-adding the local provider,
    // user scrolls the *live* fetched model list and picks (no more hardcoded defaults).
    QuickOllamaModelPick { role: String },
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
    no_auth: bool,   // true for local providers — skips credential steps
    model_options: Vec<String>,

    binding_provider: Option<String>,
    model: Option<String>,
    note: Option<String>,

    // Which role we are currently assigning (for RoleAssignment step)
    current_role: Option<String>,

    // Populated only for the Quick Ollama model picker flow so we can present
    // the real models the user has `ollama pull`'ed (no baked-in llama3.2 etc).
    ollama_model_list: Vec<String>,
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
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableBracketedPaste);
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
    view_offset: usize, // line offset into full transcript (used for manual scroll when !follow_bottom)
    follow_bottom: bool, // when true, render auto-scrolls so newest content is visible at bottom of chat area
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

    // Cached result of the Ollama probe (localhost:11434). Decides whether the
    // "Quick local Ollama setup" option is offered on first boot / in the wizard.
    // None = not yet probed. Populated lazily by is_ollama_available().
    ollama_available_cached: Option<bool>,

    // Files whose contents are sent as additional context with chat turns (via /include).
    // First step toward real agentic/grounded assistance *behind the gates* (post PlanAccepted).
    // The model sees the real file text; human still decides what to keep or edit.
    active_context: Vec<(PathBuf, String)>,

    // Lightweight document viewer popup (for /view-plan, /view-reviews etc.).
    // Gives a focused "card" experience for inspecting gate artifacts (plan + the two reviews)
    // before the explicit accept step — inspired by deliberate plan/approve flows.
    viewing_doc: Option<(String, String)>, // (title, full_content)

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
            view_offset: 0,
            follow_bottom: true,
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
            ollama_available_cached: None,
            active_context: vec![],
            viewing_doc: None,
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
        if let Ok(dir) = ensure_anvil_dir(&app.root) {
            let now = Utc::now();
            // Use milliseconds and replace '.' so the filename is clean on all filesystems.
            let ts = now.format("%Y-%m-%d-%H-%M-%S%.3f").to_string().replace('.', "-");
            let filename = format!("chat-{}.jsonl", ts);
            app.session_chat_log = Some(dir.join(&filename));

            // Write a self-describing header record for this session (handy when you have many old session logs).
            let start_rec = serde_json::json!({
                "ts": now.to_rfc3339(),
                "event": "session_start",
                "root": app.root.display().to_string(),
                "version": env!("CARGO_PKG_VERSION"),
            });
            if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(app.session_chat_log.as_ref().unwrap()) {
                let _ = writeln!(f, "{}", start_rec);
            }
        }

        // Best-effort load of existing config so we can do real chat immediately if roles are set.
        app.cfg = load_config(&app.root).ok();
        let has_reviewers = app
            .cfg
            .as_ref()
            .map_or(false, |c| c.roles.reviewer_a.is_some() && c.roles.reviewer_b.is_some());

        app.push_system("Welcome to Anvil TUI.");
        app.push_system("Type to chat with your coder. Real streaming to your configured model. /plan /phase-done /status /help /quit");
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

    /// Write one JSON event line into this session's dedicated chat log file.
    /// The file is created on first write for the session (timestamped name chosen in App::new).
    /// All events for one launch of the TUI go into exactly one file so you can inspect or delete per-session.
    fn log_chat_event(&self, event: &str, turn_id: Option<&str>, role: Option<&str>, binding: Option<&str>, model: Option<&str>, content: &str) {
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
        let proj = self
            .root
            .file_name()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| self.root.display().to_string());
        let stage = if self.first_run || self.stage == WorkflowStage::Unconfigured {
            "UNCONFIGURED — press s for quick setup (when Ollama present)"
        } else {
            match self.stage {
                WorkflowStage::Talk => "TALK (chat with coder; /plan for gate)",
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
    /// - Respects [you], [system], [review R*], [coder], [planner], [reviewer*] (assistant) prefixes with distinct colors.
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
        } else if m.starts_with("[coder]") || m.starts_with("[planner]") || m.starts_with("[assistant") {
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
        self.follow_bottom = true;
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

            // Snapshot to avoid long-lived & borrow of self while calling push_system (mut).
            let gpu_snap: Vec<(usize, f32, f32, u8)> = self
                .gpu_stats
                .iter()
                .enumerate()
                .map(|(i, g)| (i, g.mem_used as f32 / 1024.0, g.mem_total as f32 / 1024.0, g.util))
                .collect();

            // Quick VRAM/GPU snapshot in /status for convenience when debugging "full" cards.
            if !gpu_snap.is_empty() {
                self.push_system("GPUs (nvidia-smi):");
                for (i, used_g, tot_g, util) in &gpu_snap {
                    self.push_system(&format!("  {}: {:.1}/{:.1}G used @ {}% util", i, used_g, tot_g, util));
                }
            }

            // Also surface how many models Ollama currently claims are loaded.
            // Snapshot the summary first (block_on borrow of runtime).
            let ollama_info: Option<(usize, f64)> = if let Some(rt) = &self.runtime {
                match rt.block_on(self.llm.list_ollama_ps()) {
                    Ok(models) if !models.is_empty() => {
                        let total_vram: f64 = models.iter().map(|m| (m.size_vram.max(m.size)) as f64 / 1e9).sum();
                        Some((models.len(), total_vram))
                    }
                    _ => None,
                }
            } else {
                None
            };
            if let Some((cnt, vram)) = ollama_info {
                self.push_system(&format!("Ollama /ps: {} model(s) resident, ~{:.1} GB claimed VRAM", cnt, vram));
            }
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
            self.push_system("Not configured yet — the wizard should have opened automatically (or press 's' for instant local Ollama, or /config).");
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
        save_config(&self.root, &cfg)?;
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
                self.push_system(&format!("Requested unload for '{}'. (Ollama will drop it from VRAM.)", specific));
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
                    self.push_system(&format!("Unloaded {} model(s). VRAM should be freeing up.", models.len()));
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
            self.push_system("Install from https://ollama.com, run `ollama serve` (or launch the app), pull a couple models, then press 's' or choose Quick setup again.");
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
                    self.push_system(&format!("Could not list Ollama models: {}. (Is Ollama still running?)", e));
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
            w.step = WizardStep::QuickOllamaModelPick { role: "coder".to_string() };
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
            w.step = WizardStep::QuickOllamaModelPick { role: next_role.to_string() };
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
    fn live_or_static_models_for_provider(&mut self, prov_name: &str, ptype: &str, base_url: Option<&str>) -> Vec<String> {
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
    fn is_provider_preset_configured(&self, display_name: &str, suggested_name: &str, ptype: &str) -> bool {
        let Some(cfg) = &self.cfg else { return false; };
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
            if n == d_lower || n.contains(&d_lower.replace(" (local)", "").replace(" (", "").replace(")", "").replace(" ", "")) {
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
        let (provider_infos, binding_keys): (Vec<(String, String, Option<String>)>, Vec<String>) =
            if let Some(cfg) = &self.cfg {
                let provs = cfg.providers.iter()
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
                    let mods = self.live_or_static_models_for_provider(
                        &name,
                        &ptype,
                        base.as_deref(),
                    );
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
            let already = choices.iter().any(|c| {
                c == &bname || c.starts_with(&format!("{}  [", bname))
            });
            if !already {
                choices.push(bname);
            }
        }

        choices
    }

    fn is_configured(&self) -> bool {
        self.cfg
            .as_ref()
            .map_or(false, |c| c.roles.reviewer_a.is_some() && c.roles.reviewer_b.is_some())
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
                self.push_system(&format!("Credential error for binding '{}' (provider '{}'): {}", binding_name, binding.provider, e));
                self.push_system("For local providers (Ollama etc.) use the quick setup or /config and pick 'No authentication' / CredentialRef::None. Real providers need a key in the keyring or a valid env var.");
                return;
            }
        };

        // Clone what we need for the async task + the UI prefix *before* any mutable calls on self.
        // This releases the immutable borrow on self.cfg / binding / provider.
        let binding_name = binding_name.to_string();
        let model = binding.model.clone();
        let conn_for_task = provider.clone();
        let key_for_task = api_key.clone();

        // Per-turn correlation id for the jsonl log. All user / deltas / wire-full / ui-final for this exchange share it.
        let turn_id = Uuid::new_v4().to_string();
        self.current_turn_id = Some(turn_id.clone());
        self.current_role = Some(role.to_string());
        self.current_binding = Some(binding_name.clone());
        self.current_model = Some(model.clone());

        // Log the human input that started the turn (raw, as typed). Context augmentation is logged separately below as user_sent.
        self.log_chat_event("user", Some(&turn_id), Some(role), Some(&binding_name), Some(&model), text);

        // Create the (unbounded) channel. The receiver lives in the App; the sender is moved into the spawned task.
        let (tx, rx) = mpsc::unbounded_channel::<String>();
        let tx_for_full = tx.clone();
        self.llm_rx = Some(rx);

        // Start the visible streaming line with the role prefix (tokens appended live to this entry).
        // Model name is not shown here (kept only in the top header bar for CODER/R1/R2).
        let prefix = format!("[{}] ", role);
        self.push(prefix.clone());
        self.follow_bottom = true;
        self.log_chat_event("assistant_begin", Some(&turn_id), Some(role), Some(&binding_name), Some(&model), &prefix);

        // Spawn the actual streaming work on our runtime so the TUI loop is not blocked.
        if let Some(rt) = &self.runtime {
            let llm = self.llm.clone();
            // Practical chat system prompt (lighter than the strict reviewer prompts used in gates).
            // Explicitly teaches the coder its full environment, the slash-command "tools" the user can invoke,
            // exactly how /include context is delivered, the hard R1+R2 gates, and how to drive the workflow
            // without drift. This ensures the model has everything it needs to guide perfectly.
            let system = "You are a thoughtful technical thought partner helping with vibe-driven coding inside Anvil.\n\
Keep answers practical, concrete and short. When suggesting code changes, be precise: always name the exact file and show only the minimal relevant snippet or diff.\n\n\
The user follows a strict Talk → Plan (R1+R2) → phased build (each phase also R1+R2) discipline. Source of truth is always disk files (plan.md, REVIEW_*.md, .anvil/state.json). You have no direct FS or execution access — you guide the human.\n\n\
Available commands the user can type (suggest these by name when the moment is right):\n\
- /plan — generate/refresh plan.md then automatically run exactly two reviews (R1 then R2 from different configured models/bindings). After reviews, user addresses findings then /accept-plan.\n\
- /include <relative/path> — include a real project file's full content for your next turn (you will see it in a tagged block below). Tell the user the exact files to include when you need ground truth.\n\
- /context — list currently included files; /clear-context — drop them all.\n\
- /status — show reviewers, GPU/VRAM live, Ollama /ps summary, gate progress.\n\
- /loaded or /ps — detailed list of models currently in Ollama VRAM + sizes (cross-check the header GPU stats).\n\
- /unload [model] — immediately free VRAM (keep_alive=0); omit model to unload all.\n\
- /view-plan and /view-reviews — open focused cards for plan.md + the two REVIEW_plan_R*.md before accepting.\n\
- /accept-plan — record that R1+R2 findings were addressed (writes accepted hash); unlocks phases.\n\
- /config or /setup — reconfigure providers, bindings or roles.\n\
- /help — show keybindings and command list.\n\n\
When you need project information, say exactly: \"Please run /include src/foo.rs (or the paths you want) then ask me again.\"  Context from /include appears in the next user message after a '--- BEGIN PROJECT CONTEXT (files you asked to include) ---' header, with per-file --- path --- ```content``` blocks, ended by '--- END PROJECT CONTEXT ---'. Use those contents for accurate answers.\n\n\
You may suggest the user emit structured artifacts inside <artifact name=\"charter\"> or <artifact name=\"plan-draft\"> tags during exploration (the headless `anvil talk` CLI can save them).\n\n\
Be direct, surface assumptions and risks, ask clarifying questions. Always keep the two independent diverse reviews and explicit accept steps in mind — never pretend a change is done until the gates are passed on disk.".to_string();

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

            // Log the exact system + (augmented) user that will be sent over the wire. Useful for retrieval and to diagnose prompt issues.
            self.log_chat_event("system", Some(&turn_id), Some(role), Some(&binding_name), Some(&model), &system);
            self.log_chat_event("user_sent", Some(&turn_id), Some(role), Some(&binding_name), Some(&model), &user);

            rt.spawn(async move {
                // The tx is consumed by the call; when the future ends the sender is dropped and
                // the receiver in the UI loop will observe disconnect (stream finished).
                let res = llm
                    .chat_stream_to_channel(&conn_for_task, &model, &key_for_task, &system, &user, tx)
                    .await;
                if let Ok(full) = res {
                    // Send the *exact* full string returned by the SSE handler (authoritative wire response).
                    // Drain will log it under "assistant_full_wire" for chopping diagnosis (compare to concat of deltas and to final_ui)
                    // but will NOT append this to the visible message (the incremental deltas already built it).
                    let _ = tx_for_full.send(format!("[llm-full-wire]{}", full));
                }
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

        for delta in deltas {
            // Special handling for errors injected by the streaming layer (so the user
            // sees *why* there was no reply, e.g. Ollama not running, model not pulled,
            // bad endpoint, auth, etc.). We remove the placeholder assistant line we
            // started with the role prefix and surface a clean system message instead.
            if delta.contains("[llm-error]") {
                // Drop the "[coder] " (or equivalent) starter that has no useful content.
                if let Some(last) = self.messages.last() {
                    if last.starts_with('[') {
                        let _ = self.messages.pop();
                    }
                }
                let clean = delta
                    .trim_start_matches('\n')
                    .trim_start_matches("[llm-error]")
                    .trim_start_matches(": ")
                    .trim_start_matches(' ')
                    .to_string();
                self.log_chat_event("error", turn.as_deref(), role.as_deref(), binding.as_deref(), model.as_deref(), &clean);
                self.push_system(&format!("model error: {}", clean));
                changed = true;
                continue;
            }

            if let Some(wire) = delta.strip_prefix("[llm-full-wire]") {
                // Authoritative full text returned by the llm layer (concat of everything it parsed + sent over the channel).
                // Logged for comparison against the individual deltas (to detect SSE buffer remnant loss etc) and against final_ui.
                // Never appended to the visible chat line (deltas already produced the same content).
                self.log_chat_event("assistant_full_wire", turn.as_deref(), role.as_deref(), binding.as_deref(), model.as_deref(), wire);
                continue;
            }

            // Raw chunk as it arrived from the provider (via SSE parse in llm.rs). Timestamped in the jsonl.
            self.log_chat_event("assistant_delta", turn.as_deref(), role.as_deref(), binding.as_deref(), model.as_deref(), &delta);

            if let Some(last) = self.messages.last_mut() {
                last.push_str(&delta);
            }
            changed = true;
        }

        if stream_finished {
            // Stream finished for this response. Leave the accumulated text as-is.
            // Clear the receiver so we don't keep checking a dead channel.
            self.llm_rx = None;
            // Ensure the line ends neatly if the model didn't send a trailing newline.
            // Ensure trailing newline on the stored message (original behavior).
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
                self.log_chat_event("assistant_final_ui", turn.as_deref(), role.as_deref(), binding.as_deref(), model.as_deref(), last);
            }
            // Clear turn correlation so next chat gets a fresh id.
            self.current_turn_id = None;
            self.current_role = None;
            self.current_binding = None;
            self.current_model = None;
            changed = true;
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
            no_auth: false,
            model_options: vec![],
            binding_provider: None,
            model: None,
            note: None,
            current_role: None,
            ollama_model_list: vec![],
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
                }.to_string();
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
                } else if (s == "1" || s == "2") || s.contains("add / update a provider") || s.contains("provider connection") || s.contains("provider") {
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
                } else if (s == "4" || s == "5") || s.contains("finish") || s.contains("return") || s.contains("done") {
                    self.finish_config_wizard();
                } else {
                    self.push_system("Please choose a number or use the arrow keys + Enter on the list.");
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
                    w.base_url = if url.is_empty() { None } else { Some(url.to_string()) };
                    w.no_auth = !needs_key;
                    w.step = WizardStep::ProviderName;
                    w.list_items.clear();
                    w.list_title.clear();
                }
                // Pre-fill input with the suggested connection name so user can just press Enter
                self.input = suggested.to_string();

                let url_note = if url.is_empty() { "provider default".to_string() } else { url.to_string() };
                self.push_system(&format!("Provider: {}  (type={}, url={})", selected, ptype, url_note));
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
                self.input = current_url.clone();

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
                if kind.contains("keyring") || kind == "3" {
                    // Keyring is last / advanced because it has been unreliable for some users on Windows.
                    if let Some(w) = &mut self.config_wizard {
                        w.cred_kind = Some("keyring".to_string());
                        w.step = WizardStep::ApiKeySecret;
                        w.list_items.clear();
                        w.list_title.clear();
                    }
                    self.input_secret = true;
                    self.push_system("Using OS keyring (may not be readable on all Windows setups).");
                    self.push_system("Paste or type the API key / token now (input will be hidden):");
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
                    self.push_system("Using environment variable (auto-captured from the key you paste).");
                    self.push_system("Paste or type the API key / token now (input will be hidden):");
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
                    cfg.providers.get(prov)
                        .map(|c| (c.r#type.clone(), c.base_url.clone()))
                        .unwrap_or_default()
                } else { (String::new(), None) };
                let model_opts: Vec<String> = if !prov.is_empty() && !ptype_for_live.is_empty() {
                    self.live_or_static_models_for_provider(prov, &ptype_for_live, base_for_live.as_deref())
                } else { vec![] };
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
                if self.config_wizard.as_ref().map(|w| !w.list_items.is_empty()).unwrap_or(false) {
                    self.push_system("Select the model ID from the list, or choose 'Other / type manually':");
                } else {
                    self.push_system("Enter the model ID (e.g. grok-3, claude-sonnet-4-6, gpt-4o):");
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
                    self.push_system("Type the model ID (e.g. claude-sonnet-4-6, gpt-4o, llama3.1:8b):");
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
                let picked = effective.trim();
                if picked.is_empty() {
                    return;
                }

                let (binding_name, prov, model) = self.parse_role_choice(picked);

                let mut did_auto_register = false;
                if let Some(cfg) = &mut self.cfg {
                    if !cfg.model_bindings.contains_key(&binding_name) {
                        // Auto-create a binding for a model chosen directly from a provider's
                        // available list (now supports all providers, not just local-ollama).
                        // The choice string may encode "model [prov]" so we use the parsed prov.
                        // (Also keeps the old local-ollama auto-register path working for plain picks.)
                        if !cfg.providers.contains_key(&prov) {
                            // As a safety net, ensure a plausible local-ollama entry exists
                            // (mirrors prior behavior for the very first quick-ollama case).
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
                                note: Some("from role assignment (provider models list)".to_string()),
                            },
                        );
                        did_auto_register = true;
                    }

                    match role.as_str() {
                        "coder" => cfg.roles.coder = Some(binding_name.clone()),
                        "reviewer_a" => cfg.roles.reviewer_a = Some(binding_name.clone()),
                        "reviewer_b" => cfg.roles.reviewer_b = Some(binding_name.clone()),
                        _ => {}
                    }
                }

                // Borrows on cfg have ended; safe to call other &mut self methods now.
                self.save_current_config();
                if did_auto_register {
                    self.push_system(&format!("✓ Auto-registered model binding '{}' via {}.", binding_name, prov));
                }
                let display_role = match role.as_str() {
                    "coder" => "coder",
                    "reviewer_a" => "reviewer-R1",
                    "reviewer_b" => "reviewer-R2",
                    _ => role,
                };
                self.push_system(&format!("Set {} → {}", display_role, binding_name));

                let next_role = match role.as_str() {
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
                            self.push_system("You can now type to chat with the planner (or coder).");
                            self.push_system("Run /plan to generate a plan, then automatically get exactly R1 + R2 reviews from two different model bindings.");
                            self.push_system("This is the simple structured workflow that keeps vibe coding from drifting — valuable for beginners and hardcore users alike.");
                        }
                    }
                    self.populate_main_menu();
                }
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
                        },
                    );
                    match role.as_str() {
                        "coder" => cfg.roles.coder = Some(binding_name.clone()),
                        "reviewer_a" => cfg.roles.reviewer_a = Some(binding_name.clone()),
                        "reviewer_b" => cfg.roles.reviewer_b = Some(binding_name.clone()),
                        _ => {}
                    }
                }

                save_config(&self.root, self.cfg.as_ref().unwrap()).ok();
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
                    self.push_system("Type to chat, or run /plan for the full R1+R2 gated workflow.");
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
        if let Some(w) = &mut self.config_wizard {
            w.step = WizardStep::CredentialKind;
            w.list_items = vec![
                "1. Environment variable (recommended — paste the key once; we auto-set e.g. XAI_API_KEY for this session + print persistence steps)".to_string(),
                "2. No authentication required (local Ollama, unauthenticated self-hosted, etc.)".to_string(),
                "3. OS keyring (advanced; known to be unreliable on some Windows Credential Manager setups — you may see 'No matching entry')".to_string(),
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
            self.push_system(&format!("✓ Key captured as environment variable {} (current session).", var_name));
            self.push_system("  We also wrote it to .anvil/.env (plain text — keep the .anvil directory private).");
            self.push_system("  Any future `anvil` run from this project directory will auto-load it (no shell config required).");
            self.push_system("");
            self.push_system("  How this works everywhere (PowerShell, bash, Docker, CI, WSL, macOS, Linux servers...):");
            self.push_system("    • The *runtime* (std::env::var + set_var) is the same on every OS and shell.");
            self.push_system("    • .anvil/.env is loaded automatically by anvil on every start (TUI + all CLI commands).");
            self.push_system("    • For global use or when running anvil from other directories, set the variable");
            self.push_system("      in your normal environment:");
            self.push_system(&format!("        Windows (PowerShell):  $env:{} = \"<key>\"     (or use setx)", var_name));
            self.push_system(&format!("        Windows (cmd):         set {}=\"<key>\"", var_name));
            self.push_system(&format!("        Linux / macOS / WSL / Git Bash:   export {}=\"<key>\"", var_name));
            self.push_system(&format!("        fish:                  set -x {} \"<key>\"", var_name));
            self.push_system("    • CI / Docker / scripts / systemd: just make sure the variable is present in the");
            self.push_system("      environment of the process that executes `anvil` (GitHub secrets, -e flags, etc.).");
            self.push_system("    • The exact same variable names (XAI_API_KEY, OPENAI_API_KEY, ...) are used by");
            self.push_system("      many other tools, so you can often reuse existing secrets.");

            (CredentialRef::Env { var_name: var_name.clone() }, Some(var_name))
        } else if cred_kind.as_deref() == Some("keyring") {
            if let Some(key) = &api_key {
                let entry_name = format!("provider:{}", name);
                match keyring::Entry::new("anvil", &entry_name) {
                    Ok(entry) => {
                        if let Err(e) = entry.set_password(key) {
                            self.push_system(&format!("Warning: could not store key in keyring: {}", e));
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
            (CredentialRef::Env {
                var_name: env_var.unwrap_or_else(|| "API_KEY".to_string()),
            }, None)
        };

        let cred = cred;  // for use below
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
        let is_remote_compat = normalized_type == "openai_compat" || normalized_type == "openai" || normalized_type.starts_with("azure");

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
            let probe_key: Option<String> = if cred_kind.as_deref() == Some("keyring") || cred_kind.as_deref() == Some("env") {
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
                } else { String::new() }
            } else if let Some(c) = &self.cfg {
                // No just-entered key available (e.g. pure env var flow); fall back to normal credential lookup.
                if let Some(conn) = c.providers.get(&name) {
                    let bb = conn.base_url.as_deref().unwrap_or(&b).trim().to_string();
                    if !bb.is_empty() {
                        match self.llm.get_credential(&name, conn) {
                            Ok(key) => {
                                if let Some(rt) = &self.runtime {
                                    match rt.block_on(self.llm.list_openai_compat_models(&bb, &key)) {
                                        Ok(models) if !models.is_empty() => {
                                            let preview: Vec<String> = models.iter().take(3).cloned().collect();
                                            format!("✓ Live model list for '{}': {} models. Examples: {}", name, models.len(), preview.join(", "))
                                        }
                                        Ok(_) => {
                                            format!("[models] '{}' live /models returned no results (or auth issue). Role/model pickers will use built-in suggestions.", name)
                                        }
                                        Err(e) => {
                                            format!("[models] Error fetching live models for '{}': {} (using suggestions)", name, e)
                                        }
                                    }
                                } else { String::new() }
                            }
                            Err(e) => {
                                format!("[models] Could not read credential for '{}' after add ({}). Live models unavailable.", name, e)
                            }
                        }
                    } else { String::new() }
                } else { String::new() }
            } else { String::new() };
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
        if self.cfg.as_ref().map_or(true, |c| c.providers.is_empty()) {
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
                cfg.providers.get(&prov)
                    .map(|c| (c.r#type.clone(), c.base_url.clone()))
                    .unwrap_or_default()
            } else { (String::new(), None) };
            let model_opts: Vec<String> = if !prov.is_empty() && !ptype_for_live.is_empty() {
                self.live_or_static_models_for_provider(&prov, &ptype_for_live, base_for_live.as_deref())
            } else { vec![] };

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
            if self.config_wizard.as_ref().map(|w| !w.list_items.is_empty()).unwrap_or(false) {
                self.push_system("Select the model ID from the list, or choose 'Other / type manually':");
            } else {
                self.push_system("Enter the model ID (e.g. grok-3, claude-sonnet-4-6, gpt-4o):");
            }
        } else {
            // Show the provider list for the user to choose from.
            let prov_names: Vec<String> = if let Some(cfg) = &self.cfg {
                cfg.providers.keys().cloned().collect()
            } else { vec![] };
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
            },
        );

        self.save_current_config();
        self.push_system(&format!("✓ Model '{}' saved via provider '{}'.", model, prov));
        self.push_system("Use 'Assign roles' from the menu to assign it to coder / reviewer-R1 / reviewer-R2.");

        self.populate_main_menu();
    }

    fn start_role_assignment(&mut self) {
        // Delegate to start_role_list. It now pulls live/static models from *all* configured
        // providers (grouped + color-coded in the UI) and falls back gracefully with a message
        // if nothing is available yet.
        self.start_role_list("coder");
    }

    fn start_role_list(&mut self, role: &str) {
        let binding_names = self.build_available_bindings_for_roles();

        if binding_names.is_empty() {
            self.push_system("No models available from configured providers yet — add a provider via Config / 'Assign roles', or use Quick local Ollama setup first.");
            self.populate_main_menu();
            return;
        }

        if let Some(w) = &mut self.config_wizard {
            w.step = WizardStep::RoleAssignment { role: role.to_string() };
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
        self.push_system("Select a binding or live Ollama tag from the list (↑↓ then Enter):");
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
            WizardStep::ModelName => WizardStep::BindingProvider,
            WizardStep::BindingNote => WizardStep::ModelName,
            WizardStep::RoleAssignment { role } => {
                match role.as_str() {
                    "reviewer_a" => WizardStep::RoleAssignment { role: "coder".to_string() },
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
            WizardStep::QuickOllamaModelPick { role } => {
                match role.as_str() {
                    "reviewer_b" => WizardStep::QuickOllamaModelPick { role: "reviewer_a".to_string() },
                    "reviewer_a" => WizardStep::QuickOllamaModelPick { role: "coder".to_string() },
                    _ => {
                        self.populate_main_menu();
                        self.push_system("(back)");
                        return;
                    }
                }
            }
            _ => {
                self.populate_main_menu();
                self.push_system("(back)");
                return;
            }
        };

        // Snapshot role list (with live models) *before* taking the long &mut borrow on .config_wizard.
        // The build now performs &mut self live fetches (for the per-provider /models calls), so
        // we do the snapshot early while no wizard state is mutably borrowed.
        let role_list_items: Option<Vec<String>> = if matches!(prev, WizardStep::RoleAssignment { .. }) {
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
                        w.list_title = "Select a model ID (↑↓ then Enter, or choose 'Other' to type):".to_string();
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
                let ka = p.keep_alive.as_deref().map(|k| format!(" keep_alive={}", k)).unwrap_or_default();
                out.push(format!("  {} (type={}, base={}, {}{})", name, p.r#type, base, auth, ka));
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
                self.push_system("Setup complete! You can now type to chat with the coder.");
                self.push_system("Use /plan to run the Talk → plan + R1 review + R2 review gate (exactly two diverse reviewers).");
                self.push_system("The workflow is deliberately simple to start yet powerful enough for serious use: structure that prevents drift without killing velocity.");
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

        // Live GPU stats ~every 2 seconds (80ms * 25). Cheap and useful for local models.
        if app.anim_tick % 25 == 0 {
            app.refresh_gpu_stats();
        }

        terminal.draw(|f| render_ui(f, app))?;

        if event::poll(std::time::Duration::from_millis(80))? {
            match event::read()? {
                Event::Key(key) if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) => {
                    if handle_key(app, key)? {
                        break;
                    }
                }
                Event::Paste(text) => {
                    // Paste arrives as a single string — append directly without per-char processing.
                    // This avoids crashes from escape sequences inside bracketed paste streams.
                    app.input.push_str(&text);
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
                | WizardStep::ModelName
                | WizardStep::RoleAssignment { .. }
                | WizardStep::QuickOllamaModelPick { .. }
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
        KeyCode::Char('q') if key.modifiers.is_empty() && app.config_wizard.is_none() && app.input.is_empty() => {
            // Only quit on 'q' when idle — not while the wizard is open or input contains text,
            // so pasting an API key that contains 'q' doesn't eject the user.
            app.should_quit = true;
            return Ok(true);
        }
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.should_quit = true;
            return Ok(true);
        }

        KeyCode::Char('s') if key.modifiers.is_empty() && app.config_wizard.is_none() && app.input.is_empty() => {
            // Quick local Ollama setup / re-setup. Always available (when Ollama reachable) so users
            // can easily change the models assigned to CODER / R1 / R2 later by re-picking from live tags.
            // Guarded by config_wizard.is_none() (like the 'q' hotkey) so that:
            // - Pasting or typing an API key that starts with 's' (sk-... for OpenAI, many others) during
            //   the provider key entry step does not hijack into Quick Ollama.
            // - Accidental 's' while inside any part of /config or first-time setup is ignored.
            app.showing_command_palette = false;
            app.start_quick_ollama_setup();
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
            app.follow_bottom = false;
            if app.view_offset > 0 {
                app.view_offset -= 1;
            }
            return Ok(false);
        }
        KeyCode::Down => {
            app.follow_bottom = false;
            app.view_offset = app.view_offset.saturating_add(1);
            return Ok(false);
        }

        KeyCode::PageUp => {
            app.follow_bottom = false;
            app.view_offset = app.view_offset.saturating_sub(10);
            return Ok(false);
        }
        KeyCode::PageDown => {
            app.follow_bottom = false;
            app.view_offset = app.view_offset.saturating_add(10);
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
            Constraint::Length(7), // bordered header — 5 info rows (taller for GPUs on right + breathing room)
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

// ─── Header (5-row info panel, top-right column used for per-GPU status) ──────

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

    // Row 0 (top of header): stage + streaming + ctx on left.
    // When GPUs present: right column (top rows) shows 1 GPU per line; left is narrowed.
    // (The prominent "Anvil vX" with version lives in the orange block title on the border.)
    let mut row0: Vec<Span<'static>> = vec![Span::styled(
        stage_text,
        Style::default().fg(stage_color).add_modifier(Modifier::BOLD),
    )];
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
            Span::styled(" CODER ".to_string(), Style::default().fg(ROLE_CODER).add_modifier(Modifier::BOLD)),
            Span::styled(coder, Style::default().fg(Color::White)),
            Span::styled("  │  R1 ".to_string(), Style::default().fg(ROLE_R1).add_modifier(Modifier::BOLD)),
            Span::styled(r1, Style::default().fg(Color::White)),
            Span::styled("  │  R2 ".to_string(), Style::default().fg(ROLE_R2).add_modifier(Modifier::BOLD)),
            Span::styled(r2, Style::default().fg(Color::White)),
        ]
    } else {
        vec![Span::styled(
            " Run /config or press s for quick setup (Ollama if available)".to_string(),
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
            format!(" ⬡ Anvil v{} ", env!("CARGO_PKG_VERSION")),
            Style::default().fg(Color::Rgb(255, 180, 0)).add_modifier(Modifier::BOLD),
        ));

    let inner = block.inner(area);
    f.render_widget(block, area);

    // When GPUs are present we carve out a right column (one GPU per line).
    // Otherwise left content uses the full inner width (no wasted space).
    let has_gpus = !app.gpu_stats.is_empty();
    let right_width: u16 = if has_gpus { 30 } else { 0 };
    let left_width = inner.width.saturating_sub(right_width);

    for (i, spans) in [row0, row1, row2].into_iter().enumerate() {
        let y = inner.y + i as u16;
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
        if m.len() > 18 { m[..18].to_string() } else { m.clone() }
    } else {
        "—".to_string()
    }
}

/// Model label for the header row (coder / R1 / R2). Only the model name.
fn header_model_label(cfg: &crate::config::AnvilConfig, role: &str) -> String {
    if let Ok((_, binding, _)) = cfg.resolve_role_full(role) {
        let m = &binding.model;
        if m.len() > 22 { m[..22].to_string() } else { m.clone() }
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

/// Build the spans for a single GPU's line in the right column (one GPU per line).
/// Format example: "│ 0:8000  12% 41.5/48.0G"
/// GPU util % color-coded (green/yellow/red).
/// VRAM shows driver-used/total (from nvidia-smi). High usage is colored to highlight "full" cards.
/// (Ollama /loaded reports the actual weights+cache it thinks it has resident; the two numbers
/// commonly differ by a few GB due to CUDA overhead, contexts, and KV cache.)
fn render_gpu_line(stat: &GpuStat, idx: usize) -> Vec<Span<'static>> {
    let mut out: Vec<Span<'static>> = vec![];

    // Subtle column separator so the right GPU list is visually distinct from left content.
    out.push(Span::styled("│ ", Style::default().fg(Color::Rgb(60, 60, 80))));

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
        let all_lines: Vec<Line> = app.messages
            .iter()
            .flat_map(|m| App::render_message_as_lines(m))
            .collect();
        let total = all_lines.len() as u16;
        let h = area.height.saturating_sub(2).max(1);
        let scroll_y = if app.follow_bottom {
            total.saturating_sub(h)
        } else {
            // Manual scroll position (now line-granular). Clamp to valid range.
            (app.view_offset as u16).min(total.saturating_sub(1))
        };
        Paragraph::new(all_lines)
            .block(chat_block)
            .wrap(Wrap { trim: false })
            .scroll((scroll_y, 0))
    };
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

    let mut state = ListState::default();
    state.select(Some(selected));
    f.render_stateful_widget(list, popup, &mut state);
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
                        Span::styled("✓ ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
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

    // Color the wizard popup border + title for role assignment (and quick Ollama picks)
    // so CODER/R1/R2 identity is visible (blue / purple / lime).
    let (wiz_border, wiz_title_fg) = match &wizard.step {
        WizardStep::QuickOllamaModelPick { role } if role == "coder" => (ROLE_CODER, ROLE_CODER),
        WizardStep::QuickOllamaModelPick { role } if role == "reviewer_a" => (ROLE_R1, ROLE_R1),
        WizardStep::QuickOllamaModelPick { role } if role == "reviewer_b" => (ROLE_R2, ROLE_R2),
        WizardStep::RoleAssignment { role } if role == "coder" => (ROLE_CODER, ROLE_CODER),
        WizardStep::RoleAssignment { role } if role == "reviewer_a" => (ROLE_R1, ROLE_R1),
        WizardStep::RoleAssignment { role } if role == "reviewer_b" => (ROLE_R2, ROLE_R2),
        _ => (Color::Yellow, Color::DarkGray),
    };

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(wiz_border))
                .title(Span::styled(
                    format!(" {} ", wizard.list_title),
                    Style::default().fg(wiz_title_fg),
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