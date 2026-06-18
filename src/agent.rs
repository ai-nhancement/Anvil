//! The agent loop — what makes the coder a coder and not a chatbot.
//!
//! An `Agent` owns the conversation history and, each turn, asks the model for
//! an `AssistantTurn`. If the model requested tool calls, the agent executes
//! them (via `tools`), appends the results to the history, and loops — until the
//! model replies with plain text (a final answer) or a step limit is hit.
//!
//! Streaming + tool activity are emitted to the UI over a single
//! `UnboundedSender<String>` channel using small tagged prefixes the TUI knows
//! how to render:
//!   - plain text            → a streamed token delta
//!   - `[tool-start]<label>` → a tool is about to run
//!   - `[tool-end]<label>`   → a tool finished (with a short result summary)
//!   - `[confirm]<command>`  → a run_command is awaiting a y/n decision
//!   - `[llm-error]<msg>`    → transport error (emitted by llm.rs)
//!
//! For headless use (tests, CLI), pass `ConfirmHandle::AlwaysAllow`.

use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::Result;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::config::anvil_dir;
use crate::config::ProviderConnection;
use crate::llm::{ChatMessage, LlmClient, Role, ToolCall};
use crate::{reality, tools};

/// Where the immutable append-only conversation ledger lives for this project.
/// This is the source of truth for the entire history. It is never truncated.
/// Derived working sets (recent window, decayed memory, summaries) are built
/// from it at load time and during context assembly.
pub fn session_path(root: &Path) -> PathBuf {
    anvil_dir(root).join("session.json")
}

/// Append a batch of new messages to the immutable ledger as JSON Lines.
/// Each call adds one line per message. Safe for concurrent appends from the
/// same process (we always open with append). Old single-array format is
/// automatically migrated on first append.
fn append_to_ledger(root: &Path, new_msgs: &[ChatMessage]) {
    if new_msgs.is_empty() {
        return;
    }
    let path = session_path(root);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    // If the file currently contains a JSON array (legacy format), convert it
    // to JSONL once so we can append forever after.
    if path.exists() {
        if let Ok(raw) = std::fs::read_to_string(&path) {
            let trimmed = raw.trim();
            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                // Legacy array — rewrite as lines (best effort).
                if let Ok(old) = serde_json::from_str::<Vec<ChatMessage>>(trimmed) {
                    let mut f = std::fs::OpenOptions::new()
                        .create(true)
                        .write(true)
                        .truncate(true)
                        .open(&path)
                        .ok();
                    if let Some(f) = &mut f {
                        for m in &old {
                            if let Ok(line) = serde_json::to_string(m) {
                                let _ = writeln!(f, "{}", line);
                            }
                        }
                    }
                }
            }
        }
    }

    // Now append the new messages, one per line.
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        for m in new_msgs {
            if let Ok(line) = serde_json::to_string(m) {
                let _ = writeln!(f, "{}", line);
            }
        }
    }
}

/// Append a reset marker to the ledger so the next reload starts fresh while the
/// full record (everything before the marker) is preserved for audit.
pub fn append_reset_marker(root: &Path) {
    let path = session_path(root);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = writeln!(f, "{{\"reset\":true}}");
    }
}

/// Load the full immutable ledger. Returns every message that was ever
/// appended (in order). Drops a leading partial exchange (non-User start)
/// so the in-memory history always begins on a clean user turn.
pub fn load_session(root: &Path) -> Vec<ChatMessage> {
    let path = session_path(root);
    if !path.exists() {
        return vec![];
    }

    let mut msgs: Vec<ChatMessage> = vec![];
    if let Ok(f) = std::fs::File::open(&path) {
        let reader = std::io::BufReader::new(f);
        for line in std::io::BufRead::lines(reader) {
            if let Ok(line) = line {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                // Support both legacy single-array and the new JSONL format.
                if trimmed.starts_with('[') {
                    if let Ok(batch) = serde_json::from_str::<Vec<ChatMessage>>(trimmed) {
                        msgs.extend(batch);
                    }
                } else if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
                    // A reset marker (from /clear-memory) starts the reload fresh,
                    // without destroying the ledger's permanent record.
                    if val.get("reset").and_then(|r| r.as_bool()) == Some(true) {
                        msgs.clear();
                    } else if let Ok(m) = serde_json::from_value::<ChatMessage>(val) {
                        msgs.push(m);
                    }
                }
            }
        }
    }

    // Trim any leading partial exchange so we always start on a User turn.
    while msgs.first().map(|m| m.role != Role::User).unwrap_or(false) {
        msgs.remove(0);
    }
    msgs
}

/// Curated, user-editable medium-term memory. Survives compaction and restarts,
/// and is injected (bounded, delimited) into the agent's context each turn.
pub fn working_memory_path(root: &Path) -> PathBuf {
    anvil_dir(root).join("working-memory.md")
}

// ── Project context files ────────────────────────────────────────────────────
// A small set of legible, user-editable files the coder maintains with its own
// tools. Each has an explicit injection policy (no retrieval, no ranking, no
// hidden mutation): `decisions` + `assumptions` are injected each turn (bounded);
// `scratch` is never injected; `ARCHITECTURE.md` is read on demand.

/// Durable preferences/conventions + recorded verification commands.
pub fn decisions_path(root: &Path) -> PathBuf {
    anvil_dir(root).join("decisions.md")
}
/// Working hypotheses the coder has not yet verified (kept separate from facts).
pub fn assumptions_path(root: &Path) -> PathBuf {
    anvil_dir(root).join("assumptions.md")
}
/// Disposable scratchpad — never injected; not memory, not truth.
pub fn scratch_path(root: &Path) -> PathBuf {
    anvil_dir(root).join("scratch.md")
}
/// A small maintained map of the codebase (a real, committable project doc).
pub fn architecture_path(root: &Path) -> PathBuf {
    root.join("ARCHITECTURE.md")
}

const DECISIONS_TEMPLATE: &str = "# Decisions & Conventions\n<!-- Durable preferences, conventions, and verification commands for this project. Injected into the coder every turn. Keep it short and high-signal; the coder maintains this too. -->\n\n## Preferences\n<!-- e.g. - Prefer small edits over broad rewrites.  - Don't add dependencies unless necessary. -->\n\n## Verification commands\n<!-- commands that actually worked, e.g.  cargo test  ·  cargo fmt --check  ·  cargo clippy --all-targets -- -D warnings -->\n";
const ASSUMPTIONS_TEMPLATE: &str = "# Assumptions\n<!-- Working hypotheses the coder has NOT verified. Promote to a decision/fact when confirmed (and delete here), or delete if wrong. These are guesses, not truth. -->\n";
const SCRATCH_TEMPLATE: &str = "# Scratchpad (disposable — never injected)\n<!-- Temporary notes, investigation, alternative designs, command output. Not memory, not truth. Clear anytime. -->\n";
const ARCHITECTURE_TEMPLATE: &str = "# Architecture Map\n<!-- A small, maintained map of the codebase. Keep it current; the coder updates it as structure changes. -->\n\n<!-- e.g.\n- src/agent.rs — model/session/memory orchestration\n- src/ui.rs — TUI commands + rendering\n- src/reality.rs — disk/git reality snapshot\n-->\n";

/// Create the project context files with explanatory templates if they don't
/// exist yet, so they're discoverable. Templates contain only headers/comments,
/// so they are not injected until they have real content (see `has_body`).
pub fn ensure_context_files(root: &Path) {
    let _ = std::fs::create_dir_all(anvil_dir(root));
    for (path, template) in [
        (decisions_path(root), DECISIONS_TEMPLATE),
        (assumptions_path(root), ASSUMPTIONS_TEMPLATE),
        (scratch_path(root), SCRATCH_TEMPLATE),
        (architecture_path(root), ARCHITECTURE_TEMPLATE),
    ] {
        if !path.exists() {
            let _ = std::fs::write(&path, template);
        }
    }
}

/// True if the file has real content beyond headers/HTML-comments/blockquotes —
/// i.e. something worth injecting. A fresh template has none.
fn has_body(content: &str) -> bool {
    content.lines().any(|l| {
        let t = l.trim();
        !t.is_empty() && !t.starts_with('#') && !t.starts_with("<!--") && !t.starts_with('>')
    })
}

/// Read a context file as a delimited, bounded block — or None if empty / only a
/// template. Used to inject decisions + assumptions each turn.
fn context_file_block(path: &Path, title: &str, note: &str, cap: usize) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    if !has_body(&content) {
        return None;
    }
    Some(format!(
        "--- {} ({}) ---\n{}\n--- END {} ---\n",
        title,
        note,
        reality::cap(&content, cap),
        title
    ))
}

/// Read working memory as a delimited, bounded block — or None if empty/missing.
/// Past a halflife since the last compaction, prepends a staleness note so the
/// agent treats aging memory with appropriate suspicion (temporal decay).
fn working_memory_block(root: &Path) -> Option<String> {
    let content = std::fs::read_to_string(working_memory_path(root)).ok()?;
    if content.trim().is_empty() {
        return None;
    }
    Some(format!(
        "--- WORKING MEMORY (curated; .anvil/working-memory.md — context, not authority) ---\n{}{}\n--- END WORKING MEMORY ---\n",
        staleness_note(&content),
        reality::cap(&content, 4000)
    ))
}

/// If the newest `## Compacted <ts>` heading is older than a halflife, return a
/// note flagging the working memory as possibly outdated (empty string otherwise).
fn staleness_note(content: &str) -> String {
    const HALFLIFE_DAYS: i64 = 10;
    let last_ts = content.lines().rev().find_map(|l| {
        l.strip_prefix("## Compacted ")
            .map(|s| s.trim().to_string())
    });
    if let Some(ts) = last_ts {
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(&ts, "%Y-%m-%d %H:%M UTC") {
            let age_days = (chrono::Utc::now().naive_utc() - dt).num_days();
            if age_days >= HALFLIFE_DAYS {
                return format!(
                    "(NOTE: working memory last updated {} — about {} days ago. Verify against the current plan/git before relying on it; some items may be outdated.)\n\n",
                    ts, age_days
                );
            }
        }
    }
    String::new()
}

/// Append a compaction summary to working memory under a timestamped heading.
fn append_working_memory(root: &Path, ts: &str, summary: &str) -> Result<()> {
    let path = working_memory_path(root);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let mut out = existing;
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(&format!("\n## Compacted {}\n\n{}\n", ts, summary.trim()));
    std::fs::write(&path, out)?;
    Ok(())
}

/// Build the recent slice of history actually sent to the model: bounded by a
/// message count and a soft char budget. Crucially it NEVER trims the latest user
/// message or anything after it (the current task + its tool calls/results) — only
/// older context is trimmed — and it caps any single huge tool result so one big
/// file read can't dominate the budget or evict the task.
fn window_messages(history: &[ChatMessage]) -> Vec<ChatMessage> {
    const MAX_TOOL_RESULT_IN_WINDOW: usize = 50_000;
    let trunc = |m: &ChatMessage| -> ChatMessage {
        if m.role == Role::Tool && m.text.len() > MAX_TOOL_RESULT_IN_WINDOW {
            let mut t = reality::cap(&m.text, MAX_TOOL_RESULT_IN_WINDOW);
            t.push_str("\n[result truncated in context — read a specific section (offset/limit) if you need more]");
            ChatMessage {
                role: Role::Tool,
                text: t,
                tool_calls: vec![],
                tool_call_id: m.tool_call_id.clone(),
            }
        } else {
            m.clone()
        }
    };

    let start = history.len().saturating_sub(SEND_WINDOW);
    let slice = &history[start..];
    // The current task = the latest user message. Never trim it or anything after it.
    let last_user = slice
        .iter()
        .rposition(|m| m.role == Role::User)
        .unwrap_or(0);
    let task_block: Vec<ChatMessage> = slice[last_user..].iter().map(&trunc).collect();
    let task_chars: usize = task_block.iter().map(|m| m.text.len()).sum();

    // Fit as much older context (the prefix) as the remaining budget allows.
    let mut prefix: Vec<ChatMessage> = slice[..last_user].iter().map(&trunc).collect();
    let prefix_budget = CONTEXT_CHAR_BUDGET.saturating_sub(task_chars);
    let mut prefix_chars: usize = prefix.iter().map(|m| m.text.len()).sum();
    while prefix_chars > prefix_budget && !prefix.is_empty() {
        let removed = prefix.remove(0);
        prefix_chars = prefix_chars.saturating_sub(removed.text.len());
    }

    let mut window = prefix;
    window.extend(task_block);
    // Repair tool/tool_call pairing so the request is valid for both providers.
    let mut window = sanitize_history(&window);
    while window
        .first()
        .map(|m| m.role != Role::User)
        .unwrap_or(false)
    {
        window.remove(0);
    }
    window
}

/// Repair a message sequence so every `tool` result follows an assistant turn
/// whose `tool_calls` includes its id, and every assistant `tool_call` has a
/// matching result. Orphan tool results are dropped; unanswered tool_calls are
/// stripped (the assistant's text is kept). Guarantees a sequence both the
/// OpenAI-compatible and Anthropic APIs will accept.
fn sanitize_history(msgs: &[ChatMessage]) -> Vec<ChatMessage> {
    let mut out: Vec<ChatMessage> = Vec::with_capacity(msgs.len());
    let mut i = 0;
    while i < msgs.len() {
        let m = &msgs[i];
        match m.role {
            Role::Assistant if !m.tool_calls.is_empty() => {
                // Collect the contiguous tool results that follow this turn.
                let mut j = i + 1;
                let mut result_ids: Vec<String> = Vec::new();
                while j < msgs.len() && msgs[j].role == Role::Tool {
                    if let Some(id) = &msgs[j].tool_call_id {
                        result_ids.push(id.clone());
                    }
                    j += 1;
                }
                // Keep only tool_calls that actually have a matching result.
                let kept: Vec<ToolCall> = m
                    .tool_calls
                    .iter()
                    .filter(|tc| result_ids.iter().any(|id| id == &tc.id))
                    .cloned()
                    .collect();
                if kept.is_empty() {
                    if !m.text.trim().is_empty() {
                        out.push(ChatMessage::assistant(m.text.clone(), vec![]));
                    }
                } else {
                    out.push(ChatMessage::assistant(m.text.clone(), kept.clone()));
                    for t in &msgs[i + 1..j] {
                        if t.tool_call_id
                            .as_ref()
                            .map_or(false, |id| kept.iter().any(|tc| &tc.id == id))
                        {
                            out.push(t.clone());
                        }
                    }
                }
                i = j;
            }
            Role::Tool => {
                // Orphan tool result (no preceding assistant tool_calls) — drop.
                i += 1;
            }
            _ => {
                out.push(m.clone());
                i += 1;
            }
        }
    }
    out
}

/// Render the exact prompt sent to the model (system + assembled messages) as a
/// readable block for the session log, so a turn can be reproduced from the log.
fn render_prompt_for_log(system: &str, sent: &[ChatMessage]) -> String {
    let mut out = String::from("=== SYSTEM PROMPT ===\n");
    out.push_str(system);
    out.push_str(&format!(
        "\n\n=== MESSAGES SENT TO MODEL ({}) ===\n",
        sent.len()
    ));
    for m in sent {
        match m.role {
            Role::User => out.push_str(&format!("\n[USER]\n{}\n", m.text)),
            Role::Assistant => {
                out.push_str(&format!("\n[ASSISTANT]\n{}\n", m.text));
                for tc in &m.tool_calls {
                    out.push_str(&format!("  (tool call: {} {})\n", tc.name, tc.arguments));
                }
            }
            Role::Tool => out.push_str(&format!(
                "\n[TOOL RESULT {}]\n{}\n",
                m.tool_call_id.as_deref().unwrap_or(""),
                m.text
            )),
        }
    }
    out
}

/// Flatten history into a plain transcript for summarization, bounded to the tail
/// when very long (compaction only needs the recent arc plus what working memory
/// already holds).
fn render_history_for_summary(history: &[ChatMessage]) -> String {
    const MAX: usize = 40_000;
    let mut out = String::new();
    for m in history {
        match m.role {
            Role::User => out.push_str(&format!("\nUser: {}\n", m.text)),
            Role::Assistant => {
                if !m.text.trim().is_empty() {
                    out.push_str(&format!("\nAssistant: {}\n", m.text));
                }
                for tc in &m.tool_calls {
                    out.push_str(&format!("Assistant called tool: {}\n", tc.name));
                }
            }
            Role::Tool => {
                let first = m.text.lines().next().unwrap_or("");
                out.push_str(&format!("[tool result] {}\n", first));
            }
        }
    }
    if out.len() > MAX {
        let start = out.len() - MAX;
        let mut s = start;
        while s < out.len() && !out.is_char_boundary(s) {
            s += 1;
        }
        out = format!("…[earlier turns omitted]…\n{}", &out[s..]);
    }
    format!(
        "Summarize this coding session into working memory:\n{}",
        out
    )
}

/// System prompt for compaction — produce tight, durable working memory.
const COMPACT_SYSTEM: &str = "You compress a coding session into durable working memory. \
Produce a TIGHT, structured Markdown summary with these sections: ## Goal, ## Key decisions, \
## Open questions / risks, ## Current focus & next steps. Keep only high-signal facts a teammate \
would need to continue the work. Do not restate the whole transcript, do not include code dumps, \
no preamble or sign-off.";

/// How `run_command` confirmations are resolved.
pub enum ConfirmHandle {
    /// Auto-approve every command (headless / scripted / test use).
    #[allow(dead_code)]
    AlwaysAllow,
    /// Ask the UI: emit `[confirm]<cmd>` and await a bool reply on the channel.
    Channel(UnboundedReceiver<bool>),
}

impl ConfirmHandle {
    async fn confirm(&mut self, tx: &UnboundedSender<String>, command: &str) -> bool {
        match self {
            ConfirmHandle::AlwaysAllow => true,
            ConfirmHandle::Channel(rx) => {
                let _ = tx.send(format!("[confirm]{}", command));
                rx.recv().await.unwrap_or(false)
            }
        }
    }
}

/// An autonomous coding agent bound to one model + project root.
pub struct Agent {
    client: LlmClient,
    conn: ProviderConnection,
    model: String,
    api_key: String,
    system: String,
    root: PathBuf,
    history: Vec<ChatMessage>,
    /// How `run_command` confirmations are resolved for this agent.
    confirm: ConfirmHandle,
    /// Safety cap on tool-call iterations within a single user turn.
    max_steps: usize,
    /// Whether we've already nudged the user to /compact this session.
    nudged_compact: bool,
}

/// Recent messages sent to the model each turn (the long arc lives in working
/// memory + the reality snapshot, not the raw transcript).
const SEND_WINDOW: usize = 40;
/// Soft char budget for the sent window (~4 chars/token, so ~60k tokens).
const CONTEXT_CHAR_BUDGET: usize = 240_000;

impl Agent {
    pub fn new(
        client: LlmClient,
        conn: ProviderConnection,
        model: String,
        api_key: String,
        system: String,
        root: PathBuf,
        confirm: ConfirmHandle,
    ) -> Self {
        // Load the *entire* immutable ledger. The ledger is append-only and
        // never truncated. Temporal decay / recent-window logic lives in
        // context assembly (run_turn), not in what we persist.
        let history = load_session(&root);
        Self {
            client,
            conn,
            model,
            api_key,
            system,
            root,
            history,
            confirm,
            max_steps: 25,
            nudged_compact: false,
        }
    }

    /// The recent slice of history actually sent to the model: bounded by a
    /// message count and a soft char budget, and trimmed to start on a clean
    /// user turn. The full history stays in `self.history` (and on the ledger);
    /// the long arc is carried by working memory + the reality snapshot.
    fn context_window(&self) -> Vec<ChatMessage> {
        window_messages(&self.history)
    }

    /// True once the full history exceeds what we send each turn — the cue to
    /// suggest `/compact`.
    fn over_send_window(&self) -> bool {
        self.history.len() > SEND_WINDOW
            || self.history.iter().map(|m| m.text.len()).sum::<usize>() > CONTEXT_CHAR_BUDGET
    }

    /// Number of messages currently held in memory (for the /memory inspector).
    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    /// Approximate char count of what's sent to the model next turn (window only;
    /// callers add working memory + snapshot). For the /memory inspector.
    pub fn context_chars(&self) -> usize {
        self.context_window().iter().map(|m| m.text.len()).sum()
    }

    /// Reset the in-memory working history (used by /clear-memory). The ledger is
    /// untouched; a reset marker is written separately so reloads start fresh.
    pub fn clear_history(&mut self) {
        self.history.clear();
        self.nudged_compact = false;
    }

    /// Append the given messages to the immutable ledger (append-only JSONL).
    /// This is the only write path for conversation history.
    fn append_ledger(&self, new_msgs: &[ChatMessage]) {
        append_to_ledger(&self.root, new_msgs);
    }

    /// Compact the conversation: summarize it into durable working memory via the
    /// coder model, append that to `.anvil/working-memory.md`.
    ///
    /// IMPORTANT: the immutable ledger (`session.json`) is **never** truncated.
    /// This method only trims the *in-memory* working history (the derived set
    /// that actually goes to the model on future turns). The full history can
    /// always be rebuilt from the ledger + working memory + reality.
    ///
    /// Temporal decay / lightweight dedup will be applied when building the
    /// context for the model (in a later step), not by deleting ledger entries.
    pub async fn compact(&mut self, ts: &str) -> Result<String> {
        if self.history.is_empty() {
            anyhow::bail!("nothing to compact yet — start chatting first");
        }
        let transcript = render_history_for_summary(&self.history);
        let summary = self
            .client
            .chat(
                &self.conn,
                &self.model,
                &self.api_key,
                COMPACT_SYSTEM,
                &transcript,
            )
            .await?;

        append_working_memory(&self.root, ts, &summary)?;

        // Only trim the *derived* in-memory view. The ledger on disk stays complete.
        const KEEP: usize = 8;
        if self.history.len() > KEEP {
            let start = self.history.len() - KEEP;
            self.history = self.history.split_off(start);
            while self
                .history
                .first()
                .map(|m| m.role != Role::User)
                .unwrap_or(false)
            {
                self.history.remove(0);
            }
        }
        // We deliberately do *not* touch the ledger here.
        Ok(summary)
    }

    #[allow(dead_code)]
    pub fn history(&self) -> &[ChatMessage] {
        &self.history
    }

    /// Run one user turn to completion: stream the model's reply, execute any
    /// tools it requests, and keep going until it produces a final text answer.
    pub async fn run_turn(&mut self, user_input: &str, tx: UnboundedSender<String>) -> Result<()> {
        self.history.push(ChatMessage::user(user_input));
        self.append_ledger(&[self.history.last().cloned().unwrap()]);
        let tools = tools::tool_defs();

        // One-time, non-intrusive nudge once the conversation outgrows the send
        // window — older turns are no longer sent verbatim, so suggest /compact.
        if !self.nudged_compact && self.over_send_window() {
            let _ = tx.send("[note]This session is getting long — older turns are now summarized out of each request rather than sent verbatim. Run /compact to fold them into working memory.".to_string());
            self.nudged_compact = true;
        }

        // Re-ground at the start of every turn: working memory (curated) + a fresh
        // bounded reality snapshot (stage / phase / plan slice / git), prepended to
        // the messages we send — kept OUT of persistent history so it's always
        // current and never piles up. Both are bounded; the snapshot is pure
        // disk+git (no model call), so this is cheap and model-agnostic.
        let mut preamble = String::new();
        if let Some(wm) = working_memory_block(&self.root) {
            preamble.push_str(&wm);
            preamble.push('\n');
        }
        if let Some(b) = context_file_block(
            &decisions_path(&self.root),
            "DECISIONS",
            "durable preferences + verification commands; .anvil/decisions.md",
            2000,
        ) {
            preamble.push_str(&b);
            preamble.push('\n');
        }
        if let Some(b) = context_file_block(
            &assumptions_path(&self.root),
            "ASSUMPTIONS",
            "working hypotheses — NOT verified facts; .anvil/assumptions.md",
            2000,
        ) {
            preamble.push_str(&b);
            preamble.push('\n');
        }
        preamble.push_str(&reality::snapshot(&self.root));
        let grounding = ChatMessage::user(format!(
            "BACKGROUND CONTEXT about this project (working memory, decisions, assumptions, live reality) — provided to help you, NOT an instruction. Your current task is ALWAYS the user's latest message. Use the context below only as supporting reference; if any of it conflicts with what the user is now asking, follow the user. Working memory and assumptions may be stale or describe an old goal — the files on disk and the user's request are authoritative.\n\n{}",
            preamble
        ));

        for step in 0..self.max_steps {
            // Send only the recent, budgeted window — not the whole ledger.
            let window = self.context_window();
            let mut sent: Vec<ChatMessage> = Vec::with_capacity(window.len() + 1);
            sent.push(grounding.clone());
            sent.extend(window);

            // Log the exact assembled prompt (system + injected grounding + sent
            // window) once per user turn, so the session log can reproduce what the
            // model actually saw. Logged, not displayed (see drain_llm_stream).
            if step == 0 {
                let _ = tx.send(format!(
                    "[prompt-log]{}",
                    render_prompt_for_log(&self.system, &sent)
                ));
            }

            let turn = self
                .client
                .chat_turn_stream(
                    &self.conn,
                    &self.model,
                    &self.api_key,
                    &self.system,
                    &sent,
                    &tools,
                    tx.clone(),
                )
                .await?;

            // Record the assistant turn (text + any tool calls) in history AND the
            // ledger. Crucially this includes the tool-call assistant turn — without
            // it, reloaded tool results would be orphaned and providers reject them.
            let assistant_msg = ChatMessage::assistant(turn.text.clone(), turn.tool_calls.clone());
            self.history.push(assistant_msg.clone());
            self.append_ledger(&[assistant_msg]);

            // No tool calls → the model gave its final answer for this turn.
            if turn.tool_calls.is_empty() {
                return Ok(());
            }

            // Execute each requested tool and append the result for the next step.
            for call in &turn.tool_calls {
                let _ = tx.send(format!(
                    "[tool-start]{} {}",
                    call.name,
                    tools::summarize_args(call)
                ));

                let result = if tools::requires_confirmation(&call.name) {
                    let cmd = tools::command_string(call);
                    if self.confirm.confirm(&tx, &cmd).await {
                        tools::execute(call, &self.root)
                    } else {
                        "ERROR: command was declined by the user".to_string()
                    }
                } else {
                    tools::execute(call, &self.root)
                };

                let _ = tx.send(format!(
                    "[tool-end]{} {}",
                    call.name,
                    tools::result_summary(&call.name, &result)
                ));
                let tr = ChatMessage::tool_result(call.id.clone(), result);
                self.history.push(tr.clone());
                self.append_ledger(&[tr]);
            }
        }

        let _ = tx.send(format!(
            "\n[agent] stopped after {} tool steps this turn (safety cap). Ask me to continue if needed.",
            self.max_steps
        ));
        // Final assistant message (if any) was already appended when it was produced.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::ChatMessage;

    #[test]
    fn load_session_drops_leading_partial_exchange() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        // Persisted history that begins mid-exchange (an orphan tool result +
        // a dangling assistant turn) — the front must be trimmed to a clean user turn.
        let history = vec![
            ChatMessage::tool_result("x", "orphan"),
            ChatMessage::assistant("dangling", vec![]),
            ChatMessage::user("hello"),
            ChatMessage::assistant("hi", vec![]),
        ];
        let path = session_path(root);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, serde_json::to_string(&history).unwrap()).unwrap();

        let loaded = load_session(root);
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].role, Role::User);
        assert_eq!(loaded[0].text, "hello");
    }

    #[test]
    fn load_session_empty_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        assert!(load_session(dir.path()).is_empty());
    }

    #[test]
    fn sanitize_drops_orphan_tool_results() {
        // The corruption pattern: a tool result with no preceding assistant tool_call
        // (the assistant-tool_calls message was missing from the ledger).
        let msgs = vec![
            ChatMessage::user("hi"),
            ChatMessage::tool_result("orphan", "stray result"),
            ChatMessage::assistant("answer", vec![]),
        ];
        let clean = sanitize_history(&msgs);
        assert_eq!(clean.len(), 2);
        assert_eq!(clean[0].role, Role::User);
        assert_eq!(clean[1].role, Role::Assistant);
    }

    #[test]
    fn sanitize_keeps_valid_tool_pair_and_strips_unanswered() {
        let call_ok = ToolCall {
            id: "a".into(),
            name: "read_file".into(),
            arguments: serde_json::json!({}),
        };
        let call_missing = ToolCall {
            id: "b".into(),
            name: "grep".into(),
            arguments: serde_json::json!({}),
        };
        let msgs = vec![
            ChatMessage::user("go"),
            ChatMessage::assistant("", vec![call_ok, call_missing]),
            ChatMessage::tool_result("a", "ok"), // only 'a' answered; 'b' has no result
        ];
        let clean = sanitize_history(&msgs);
        // assistant keeps only the answered call; the matching result is kept.
        let asst = &clean[1];
        assert_eq!(asst.role, Role::Assistant);
        assert_eq!(asst.tool_calls.len(), 1);
        assert_eq!(asst.tool_calls[0].id, "a");
        assert_eq!(clean[2].role, Role::Tool);
        assert_eq!(clean[2].tool_call_id.as_deref(), Some("a"));
    }

    #[test]
    fn template_files_not_injected_until_they_have_body() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        ensure_context_files(root);
        // Fresh templates are headers/comments only → not injected.
        assert!(context_file_block(&decisions_path(root), "DECISIONS", "n", 2000).is_none());
        assert!(context_file_block(&assumptions_path(root), "ASSUMPTIONS", "n", 2000).is_none());
        // Real content → injected as a delimited block.
        std::fs::write(decisions_path(root), "# Decisions\n- Prefer small edits.\n").unwrap();
        let block = context_file_block(&decisions_path(root), "DECISIONS", "n", 2000).unwrap();
        assert!(block.starts_with("--- DECISIONS"));
        assert!(block.contains("Prefer small edits"));
    }

    #[test]
    fn window_preserves_task_under_huge_tool_result() {
        // The task, then a tool call + a >budget tool result (a 200KB file read).
        let big = "x".repeat(300_000);
        let history = vec![
            ChatMessage::user("add a forge cursor to the input window"),
            ChatMessage::assistant(
                "",
                vec![ToolCall {
                    id: "r".into(),
                    name: "read_file".into(),
                    arguments: serde_json::json!({"path":"src/ui.rs"}),
                }],
            ),
            ChatMessage::tool_result("r", &big),
        ];
        let window = window_messages(&history);
        // The task message must survive (not evicted by the giant read)...
        assert!(
            window
                .iter()
                .any(|m| m.role == Role::User && m.text.contains("forge cursor")),
            "task message was evicted by the large tool result"
        );
        // ...and the huge tool result must be capped in the window.
        let tool = window.iter().find(|m| m.role == Role::Tool).unwrap();
        assert!(
            tool.text.len() < 60_000,
            "tool result not capped: {} bytes",
            tool.text.len()
        );
    }

    #[test]
    fn reset_marker_starts_reload_fresh() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        append_to_ledger(
            root,
            &[
                ChatMessage::user("old"),
                ChatMessage::assistant("a1", vec![]),
            ],
        );
        append_reset_marker(root);
        append_to_ledger(
            root,
            &[
                ChatMessage::user("fresh"),
                ChatMessage::assistant("a2", vec![]),
            ],
        );
        let loaded = load_session(root);
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].text, "fresh");
        assert_eq!(loaded[1].text, "a2");
    }
}
