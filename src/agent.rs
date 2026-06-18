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
use crate::llm::{ChatMessage, LlmClient, Role};
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
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
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
    let last_ts = content
        .lines()
        .rev()
        .find_map(|l| l.strip_prefix("## Compacted ").map(|s| s.trim().to_string()));
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
    format!("Summarize this coding session into working memory:\n{}", out)
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
        let start = self.history.len().saturating_sub(SEND_WINDOW);
        let mut window: Vec<ChatMessage> = self.history[start..].to_vec();
        let mut total: usize = window.iter().map(|m| m.text.len()).sum();
        while total > CONTEXT_CHAR_BUDGET && window.len() > 2 {
            let removed = window.remove(0);
            total = total.saturating_sub(removed.text.len());
        }
        while window.first().map(|m| m.role != Role::User).unwrap_or(false) {
            window.remove(0);
        }
        window
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
            .chat(&self.conn, &self.model, &self.api_key, COMPACT_SYSTEM, &transcript)
            .await?;

        append_working_memory(&self.root, ts, &summary)?;

        // Only trim the *derived* in-memory view. The ledger on disk stays complete.
        const KEEP: usize = 8;
        if self.history.len() > KEEP {
            let start = self.history.len() - KEEP;
            self.history = self.history.split_off(start);
            while self.history.first().map(|m| m.role != Role::User).unwrap_or(false) {
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
    pub async fn run_turn(
        &mut self,
        user_input: &str,
        tx: UnboundedSender<String>,
    ) -> Result<()> {
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
        preamble.push_str(&reality::snapshot(&self.root));
        let grounding = ChatMessage::user(format!(
            "Current project context (working memory + live reality). Treat it as where we are; the files on disk remain authoritative.\n\n{}",
            preamble
        ));

        for _ in 0..self.max_steps {
            // Send only the recent, budgeted window — not the whole ledger.
            let window = self.context_window();
            let mut sent: Vec<ChatMessage> = Vec::with_capacity(window.len() + 1);
            sent.push(grounding.clone());
            sent.extend(window);

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

            // Record the assistant turn (text + any tool calls) in history.
            self.history
                .push(ChatMessage::assistant(turn.text.clone(), turn.tool_calls.clone()));

            // No tool calls → the model gave its final answer for this turn.
            if turn.tool_calls.is_empty() {
                self.append_ledger(&[self.history.last().cloned().unwrap()]);
                return Ok(());
            }

            // Execute each requested tool and append the result for the next step.
            for call in &turn.tool_calls {
                let _ = tx.send(format!("[tool-start]{} {}", call.name, tools::summarize_args(call)));

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

                let _ = tx.send(format!("[tool-end]{} {}", call.name, tools::result_summary(&call.name, &result)));
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
    fn reset_marker_starts_reload_fresh() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        append_to_ledger(root, &[
            ChatMessage::user("old"),
            ChatMessage::assistant("a1", vec![]),
        ]);
        append_reset_marker(root);
        append_to_ledger(root, &[
            ChatMessage::user("fresh"),
            ChatMessage::assistant("a2", vec![]),
        ]);
        let loaded = load_session(root);
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].text, "fresh");
        assert_eq!(loaded[1].text, "a2");
    }
}
