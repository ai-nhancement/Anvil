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

use std::path::PathBuf;

use anyhow::Result;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::config::ProviderConnection;
use crate::llm::{ChatMessage, LlmClient};
use crate::tools;

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
        Self {
            client,
            conn,
            model,
            api_key,
            system,
            root,
            history: vec![],
            confirm,
            max_steps: 25,
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

        for _ in 0..self.max_steps {
            let turn = self
                .client
                .chat_turn_stream(
                    &self.conn,
                    &self.model,
                    &self.api_key,
                    &self.system,
                    &self.history,
                    &tools,
                    tx.clone(),
                )
                .await?;

            // Record the assistant turn (text + any tool calls) in history.
            self.history
                .push(ChatMessage::assistant(turn.text.clone(), turn.tool_calls.clone()));

            // No tool calls → the model gave its final answer for this turn.
            if turn.tool_calls.is_empty() {
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

                let _ = tx.send(format!("[tool-end]{} {}", call.name, tools::result_summary(&result)));
                self.history
                    .push(ChatMessage::tool_result(call.id.clone(), result));
            }
        }

        let _ = tx.send(format!(
            "\n[agent] stopped after {} tool steps this turn (safety cap). Ask me to continue if needed.",
            self.max_steps
        ));
        Ok(())
    }
}
