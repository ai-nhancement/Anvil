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

use std::path::{Path, PathBuf};

use anyhow::Result;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::config::anvil_dir;
use crate::config::ProviderConnection;
use crate::llm::{ChatMessage, LlmClient, Role};
use crate::{reality, tools};

/// How many trailing history messages we persist across restarts. Bounded so the
/// session file (and the reloaded context) can't grow without limit.
const MAX_PERSIST_MESSAGES: usize = 80;

/// Where the rolling conversation history is persisted for this project.
pub fn session_path(root: &Path) -> PathBuf {
    anvil_dir(root).join("session.json")
}

/// Load the persisted conversation history for `root` (empty if none / unreadable).
/// Drops any leading partial exchange so the restored history always begins on a
/// clean user turn — never an orphaned tool result or tool-call assistant turn
/// (which would violate the providers' strict message ordering on the next call).
pub fn load_session(root: &Path) -> Vec<ChatMessage> {
    let mut msgs: Vec<ChatMessage> = match std::fs::read_to_string(session_path(root)) {
        Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
        Err(_) => vec![],
    };
    while msgs.first().map(|m| m.role != Role::User).unwrap_or(false) {
        msgs.remove(0);
    }
    msgs
}

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
}

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
        // Restore any persisted history for this project so the coder picks up
        // where we left off across `anvil` restarts.
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
        }
    }

    /// Persist a bounded tail of the conversation history to `.anvil/session.json`.
    /// Called at the end of each completed turn.
    fn save_session(&self) {
        let start = self.history.len().saturating_sub(MAX_PERSIST_MESSAGES);
        let slice = &self.history[start..];
        if let Ok(json) = serde_json::to_string(slice) {
            let path = session_path(&self.root);
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(path, json);
        }
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
        let tools = tools::tool_defs();

        // Re-ground at the start of every turn: a fresh, bounded reality snapshot
        // (stage / phase / plan slice / git) prepended to the messages we send —
        // kept OUT of persistent history so it's always current and never piles up.
        // Pure disk+git, so this is cheap and model-agnostic.
        let grounding = ChatMessage::user(format!(
            "Here is the current project reality. Treat it as ground truth for where we are; the files on disk remain authoritative.\n\n{}",
            reality::snapshot(&self.root)
        ));

        for _ in 0..self.max_steps {
            let mut sent: Vec<ChatMessage> = Vec::with_capacity(self.history.len() + 1);
            sent.push(grounding.clone());
            sent.extend(self.history.iter().cloned());

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
                self.save_session();
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
                self.history
                    .push(ChatMessage::tool_result(call.id.clone(), result));
            }
        }

        let _ = tx.send(format!(
            "\n[agent] stopped after {} tool steps this turn (safety cap). Ask me to continue if needed.",
            self.max_steps
        ));
        self.save_session();
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
}
