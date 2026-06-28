//! Model-agnostic LLM client.
//!
//! Core design: one unified interface. Heavy emphasis on "openai_compat" because it covers
//! the vast majority of real-world usage:
//!   - Ollama (http://localhost:11434/v1)
//!   - Groq, Together, Fireworks, OpenRouter, DeepSeek, etc.
//!   - Azure OpenAI (when using the /openai/deployments/... path + api-key header or query param)
//!   - Any vLLM / LocalAI / llama.cpp server
//!
//! Special-cased adapters for Anthropic (Messages API) and Google (Gemini).
//! AWS (Bedrock), Vertex, Gradient etc. work via openai_compat gateways for now.
//!
//! Exactly two reviews from different providers is a *workflow* concern, not a client concern.

use std::collections::BTreeMap;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::io::{stdout, AsyncWriteExt};
use tokio::sync::mpsc::UnboundedSender;

use crate::config::{CredentialRef, ProviderConnection};

// ──────────────────────────────────────────────────────────────────────────
// Agentic tool-calling types
//
// These power the real agent loop (see `agent.rs`): the model can request tool
// calls, we execute them, append the results, and loop until it answers in text.
// Both the OpenAI-compatible (`tools` / `tool_calls`) and Anthropic
// (`tool_use` / `tool_result`) wire formats are supported.
// ──────────────────────────────────────────────────────────────────────────

/// Who authored a turn in the conversation history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    User,
    Assistant,
    /// A tool result being fed back to the model.
    Tool,
}

/// One turn of conversation history. An assistant turn may carry `tool_calls`;
/// a `Tool` turn carries the result for a prior call (`tool_call_id`).
/// Serializable so the session can be persisted to `.anvil/session.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: Role,
    pub text: String,
    pub tool_calls: Vec<ToolCall>,
    pub tool_call_id: Option<String>,
}

impl ChatMessage {
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            text: text.into(),
            tool_calls: vec![],
            tool_call_id: None,
        }
    }
    pub fn assistant(text: impl Into<String>, tool_calls: Vec<ToolCall>) -> Self {
        Self {
            role: Role::Assistant,
            text: text.into(),
            tool_calls,
            tool_call_id: None,
        }
    }
    pub fn tool_result(tool_call_id: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            role: Role::Tool,
            text: text.into(),
            tool_calls: vec![],
            tool_call_id: Some(tool_call_id.into()),
        }
    }
}

/// A tool the model is allowed to call. `input_schema` is a JSON Schema object.
#[derive(Debug, Clone)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

/// A single tool invocation requested by the model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

/// The outcome of one assistant turn: any streamed text plus any tool calls
/// the model wants executed. Empty `tool_calls` means the model is done.
#[derive(Debug, Clone, Default)]
pub struct AssistantTurn {
    pub text: String,
    pub tool_calls: Vec<ToolCall>,
}

/// High-level client. Cheap to clone (Arc under the hood).
#[derive(Clone)]
pub struct LlmClient {
    pub(crate) http: Arc<Client>,
}

impl Default for LlmClient {
    fn default() -> Self {
        Self::new()
    }
}

impl LlmClient {
    pub fn new() -> Self {
        let http = Client::builder()
            .user_agent("anvil/0.1 (https://github.com/ai-nhancement/Anvil)")
            .build()
            .expect("reqwest client");
        Self {
            http: Arc::new(http),
        }
    }

    /// Convenience helper so the synchronous CLI command functions (talk/plan/phase)
    /// can easily drive the async chat methods without pulling in the full `futures` crate.
    /// Uses a fresh current-thread runtime per call (fine for this CLI; calls are infrequent).
    pub fn block_on<F: std::future::Future>(fut: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to build tokio runtime for blocking call")
            .block_on(fut)
    }

    /// Retrieve the API key / credential for a connection at call time.
    pub fn get_credential(&self, conn_name: &str, conn: &ProviderConnection) -> Result<String> {
        match &conn.credential {
            CredentialRef::Keyring => {
                let entry_name = format!("provider:{}", conn_name);
                let entry = keyring::Entry::new("anvil", &entry_name)
                    .map_err(|e| anyhow!("keyring entry error for {}: {}", conn_name, e))?;
                entry
                    .get_password()
                    // Trim: pasted keys often carry a trailing newline/space, which
                    // sends a bad Authorization header and reads as "incorrect API key".
                    .map(|k| k.trim().to_string())
                    .map_err(|e| anyhow!("failed to read keyring for {}: {}", conn_name, e))
            }
            CredentialRef::Env { var_name } => {
                if let Ok(val) = std::env::var(var_name) {
                    if !val.trim().is_empty() {
                        return Ok(val.trim().to_string());
                    }
                }
                // Graceful fallback for local Ollama (and similar): the quick setup and docs
                // have long said "any non-empty string works (or omit)". We now make the env
                // truly optional for the conventional OLLAMA_API_KEY case so first-run "just works".
                if var_name == "OLLAMA_API_KEY" || var_name.to_uppercase().contains("OLLAMA") {
                    return Ok("ollama".to_string());
                }
                anyhow::bail!(
                    "environment variable {} not set (for provider {})",
                    var_name,
                    conn_name
                )
            }
            CredentialRef::None => {
                // Local / unauthenticated endpoint. "ollama" is a conventional harmless placeholder.
                Ok("ollama".to_string())
            }
        }
    }

    /// Quick reachability check for default Ollama (http://localhost:11434).
    /// Short timeout so first-boot / menu decisions aren't slow.
    pub async fn probe_ollama(&self) -> bool {
        let url = "http://localhost:11434/api/version";
        match self
            .http
            .get(url)
            .timeout(std::time::Duration::from_millis(700))
            .send()
            .await
        {
            Ok(resp) => resp.status().is_success(),
            Err(_) => false,
        }
    }

    /// Live list of models from local Ollama.
    /// Prefers the OpenAI-compat /v1/models (exact IDs for chat calls).
    /// Falls back to native /api/tags if needed.
    pub async fn list_ollama_models(&self) -> Result<Vec<String>> {
        // Compat path via the general helper (exact IDs for /v1/chat/completions).
        // Uses the conventional placeholder key that unauthenticated Ollama accepts.
        if let Ok(ids) = self
            .list_openai_compat_models("http://localhost:11434/v1", "ollama")
            .await
        {
            if !ids.is_empty() {
                return Ok(ids);
            }
        }

        // Native Ollama fallback
        let url = "http://localhost:11434/api/tags";
        #[derive(Deserialize, Debug)]
        struct Tag {
            name: String,
        }
        #[derive(Deserialize, Debug)]
        struct Tags {
            models: Vec<Tag>,
        }
        let resp = self
            .http
            .get(url)
            .timeout(std::time::Duration::from_secs(3))
            .send()
            .await
            .context("failed to reach Ollama /api/tags")?;
        if !resp.status().is_success() {
            anyhow::bail!("Ollama returned non-success for model list");
        }
        let t: Tags = resp.json().await?;
        Ok(t.models.into_iter().map(|x| x.name).collect())
    }

    /// Live list of models for OpenAI-compatible providers (xAI, Groq, OpenAI, Together, Fireworks,
    /// Ollama's compat endpoint, custom vLLM servers, many gateways, and Azure in compat mode).
    /// Calls the standard GET {base}/models and parses the common { "data": [ {"id": "..." }, ... ] } shape.
    /// Returns Ok([]) on network error, non-2xx, or empty/unparseable response so callers can
    /// silently fall back to static suggestions.
    /// Sends Bearer token; for Azure bases also sends api-key header (both are harmless together).
    pub async fn list_openai_compat_models(
        &self,
        base_url: &str,
        api_key: &str,
    ) -> Result<Vec<String>> {
        let base = base_url.trim_end_matches('/');
        if base.is_empty() {
            return Ok(vec![]);
        }
        let url = format!("{}/models", base);

        #[derive(Deserialize, Debug)]
        struct M {
            id: String,
        }
        #[derive(Deserialize, Debug)]
        struct L {
            data: Vec<M>,
        }

        let mut rb = self
            .http
            .get(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .timeout(std::time::Duration::from_secs(4));

        if base.contains("azure") {
            rb = rb.header("api-key", api_key);
        }

        let resp = match rb.send().await {
            Ok(r) => r,
            Err(_) => return Ok(vec![]),
        };

        if !resp.status().is_success() {
            return Ok(vec![]);
        }

        match resp.json::<L>().await {
            Ok(list) => {
                let ids: Vec<String> = list.data.into_iter().map(|m| m.id).collect();
                Ok(ids)
            }
            Err(_) => Ok(vec![]),
        }
    }

    /// Non-streaming chat. Returns the full assistant message.
    pub async fn chat(
        &self,
        conn: &ProviderConnection,
        model: &str,
        api_key: &str,
        system: &str,
        user: &str,
    ) -> Result<String> {
        match conn.r#type.as_str() {
            "openai_compat" | "openai" | "azure_openai" => {
                self.chat_openai_compat(conn, model, api_key, system, user, false)
                    .await
            }
            "anthropic" => {
                self.chat_anthropic(conn, model, api_key, system, user, false)
                    .await
            }
            "google" | "google_ai_studio" | "gemini" => {
                self.chat_google(conn, model, api_key, system, user).await
            }
            other => {
                // Future: "aws_bedrock", "vertex" etc. For now, give a helpful message.
                anyhow::bail!(
                    "provider type '{}' is not yet implemented as a native adapter.\n\
                     Use type = \"openai_compat\" with an appropriate base_url (many gateways and Azure in compat mode work this way).\n\
                     Or route through OpenRouter / Together / Fireworks which speak OpenAI compat.",
                    other
                )
            }
        }
    }

    /// Streaming chat. Prints tokens as they arrive (to stdout) and returns the full text.
    /// The caller is responsible for any "Anvil:" prefix etc.
    pub async fn chat_stream(
        &self,
        conn: &ProviderConnection,
        model: &str,
        api_key: &str,
        system: &str,
        user: &str,
    ) -> Result<String> {
        match conn.r#type.as_str() {
            "openai_compat" | "openai" | "azure_openai" => {
                self.chat_openai_compat(conn, model, api_key, system, user, true)
                    .await
            }
            "anthropic" => {
                self.chat_anthropic(conn, model, api_key, system, user, true)
                    .await
            }
            "google" | "google_ai_studio" | "gemini" => {
                // Gemini streaming is possible but more complex; fall back to non-stream for now.
                let full = self.chat_google(conn, model, api_key, system, user).await?;
                // Best-effort "stream" the whole thing
                let mut out = stdout();
                out.write_all(full.as_bytes()).await.ok();
                out.flush().await.ok();
                Ok(full)
            }
            other => {
                anyhow::bail!(
                    "provider type '{}' does not support streaming yet (or is not implemented)",
                    other
                )
            }
        }
    }

    /// Channel-based *plain* (no-tools) streaming chat. Superseded in the TUI by the
    /// agent loop (`chat_turn_stream`); retained as a simple non-agent streaming path
    /// for potential headless/CLI consumers.
    #[allow(dead_code)]
    pub async fn chat_stream_to_channel(
        &self,
        conn: &ProviderConnection,
        model: &str,
        api_key: &str,
        system: &str,
        user: &str,
        token_tx: UnboundedSender<String>,
    ) -> Result<String> {
        match conn.r#type.as_str() {
            "openai_compat" | "openai" | "azure_openai" => {
                self.chat_openai_compat_to_channel(conn, model, api_key, system, user, token_tx)
                    .await
            }
            "anthropic" => {
                self.chat_anthropic_to_channel(conn, model, api_key, system, user, token_tx)
                    .await
            }
            "google" | "google_ai_studio" | "gemini" => {
                // Gemini streaming is more involved; send the whole response as a single chunk.
                match self.chat_google(conn, model, api_key, system, user).await {
                    Ok(full) => {
                        let _ = token_tx.send(full.clone());
                        Ok(full)
                    }
                    Err(e) => {
                        let _ = token_tx.send(format!("\n[llm-error] {}", e));
                        Err(e)
                    }
                }
            }
            other => {
                let msg = format!(
                    "provider type '{}' does not support streaming yet (or is not implemented)",
                    other
                );
                let _ = token_tx.send(format!("\n[llm-error] {}", msg));
                anyhow::bail!("{}", msg)
            }
        }
    }

    // ──────────────────────────────────────────────────────────────────────────
    // OpenAI-compatible (Chat Completions) — the workhorse for Ollama + 80% of others
    // ──────────────────────────────────────────────────────────────────────────

    /// Resolve the base URL for an OpenAI-compatible provider.
    ///
    /// We deliberately do NOT default to OpenAI. A missing base_url on a non-OpenAI
    /// gateway (Gradient, Groq, local, ...) used to silently send the user's key to
    /// api.openai.com — a confusing 401 and a credential leak. Now it errors clearly.
    fn openai_compat_base(conn: &ProviderConnection) -> Result<String> {
        match conn
            .base_url
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            Some(b) => Ok(b.trim_end_matches('/').to_string()),
            None => Err(anyhow!(
                "provider has no base_url set. OpenAI-compatible providers need an explicit base URL \
                 (e.g. https://inference.do-ai.run/v1 for Gradient, https://api.x.ai/v1 for xAI, \
                 https://api.openai.com/v1 for OpenAI). Run /config and set this provider's base URL."
            )),
        }
    }

    async fn chat_openai_compat(
        &self,
        conn: &ProviderConnection,
        model: &str,
        api_key: &str,
        system: &str,
        user: &str,
        stream: bool,
    ) -> Result<String> {
        let base = Self::openai_compat_base(conn)?;

        let url = format!("{}/chat/completions", base);

        #[derive(Serialize)]
        struct Req<'a> {
            model: &'a str,
            messages: Vec<Message<'a>>,
            stream: bool,
            #[serde(skip_serializing_if = "Option::is_none")]
            temperature: Option<f32>,
            #[serde(skip_serializing_if = "Option::is_none")]
            keep_alive: Option<String>,
        }
        #[derive(Serialize)]
        struct Message<'a> {
            role: &'a str,
            content: &'a str,
        }

        let base_for_detect = conn.base_url.as_deref().unwrap_or("");
        let is_ollama =
            base_for_detect.contains("11434") || base_for_detect.to_lowercase().contains("ollama");
        let keep_alive = if let Some(k) = &conn.keep_alive {
            Some(k.clone())
        } else if is_ollama {
            Some("30s".to_string())
        } else {
            None
        };

        let req = Req {
            model,
            messages: vec![
                Message {
                    role: "system",
                    content: system,
                },
                Message {
                    role: "user",
                    content: user,
                },
            ],
            stream,
            temperature: None,
            keep_alive,
        };

        let mut request = self
            .http
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json");

        // Azure sometimes wants api-key header instead of Bearer.
        if conn.r#type == "azure_openai" || base.contains("azure.com") {
            request = request.header("api-key", api_key);
        }

        if stream {
            request = request.header("Accept", "text/event-stream");
        }

        let resp = request
            .json(&req)
            .send()
            .await
            .with_context(|| format!("POST {} failed", url))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("{} error ({}): {}", conn.r#type, status, body);
        }

        if stream {
            return self.handle_openai_sse_stream(resp).await;
        }

        #[derive(Deserialize)]
        struct Resp {
            choices: Vec<Choice>,
        }
        #[derive(Deserialize)]
        struct Choice {
            message: Msg,
        }
        #[derive(Deserialize)]
        struct Msg {
            content: String,
        }

        let body: Resp = resp.json().await?;
        let content = body
            .choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .unwrap_or_default();
        Ok(content)
    }

    async fn handle_openai_sse_stream(&self, resp: reqwest::Response) -> Result<String> {
        use tokio::io::AsyncWriteExt;

        let mut stream = resp.bytes_stream();
        let mut full = String::new();
        let mut out = stdout();

        // Very small SSE parser for OpenAI's delta stream.
        let mut buffer = String::new();

        while let Some(chunk) = stream.next().await {
            let bytes = chunk?;
            buffer.push_str(&String::from_utf8_lossy(&bytes));

            // Process complete lines
            while let Some(pos) = buffer.find('\n') {
                let line = buffer[..pos].trim_end_matches('\r').to_string();
                buffer = buffer[pos + 1..].to_string();

                if line.is_empty() {
                    continue;
                }
                if let Some(data) = line.strip_prefix("data: ") {
                    if data == "[DONE]" {
                        out.write_all(b"\n").await.ok();
                        out.flush().await.ok();
                        return Ok(full);
                    }
                    // Try to parse the json chunk
                    if let Ok(chunk) = serde_json::from_str::<OpenAiStreamChunk>(data) {
                        if let Some(delta) = chunk
                            .choices
                            .into_iter()
                            .next()
                            .and_then(|c| c.delta.content)
                        {
                            if !delta.is_empty() {
                                full.push_str(&delta);
                                // Print live
                                let _ = out.write_all(delta.as_bytes()).await;
                                let _ = out.flush().await;
                            }
                        }
                    }
                }
            }
        }

        // If we fell out without [DONE], still return what we have.
        if !full.ends_with('\n') {
            let _ = out.write_all(b"\n").await;
        }
        Ok(full)
    }

    async fn chat_openai_compat_to_channel(
        &self,
        conn: &ProviderConnection,
        model: &str,
        api_key: &str,
        system: &str,
        user: &str,
        token_tx: UnboundedSender<String>,
    ) -> Result<String> {
        let base = Self::openai_compat_base(conn)?;

        let url = format!("{}/chat/completions", base);

        #[derive(Serialize)]
        struct Req<'a> {
            model: &'a str,
            messages: Vec<Message<'a>>,
            stream: bool,
            #[serde(skip_serializing_if = "Option::is_none")]
            temperature: Option<f32>,
            #[serde(skip_serializing_if = "Option::is_none")]
            keep_alive: Option<String>,
        }
        #[derive(Serialize)]
        struct Message<'a> {
            role: &'a str,
            content: &'a str,
        }

        let base_for_detect = conn.base_url.as_deref().unwrap_or("");
        let is_ollama =
            base_for_detect.contains("11434") || base_for_detect.to_lowercase().contains("ollama");
        let keep_alive = if let Some(k) = &conn.keep_alive {
            Some(k.clone())
        } else if is_ollama {
            Some("30s".to_string())
        } else {
            None
        };

        let req = Req {
            model,
            messages: vec![
                Message {
                    role: "system",
                    content: system,
                },
                Message {
                    role: "user",
                    content: user,
                },
            ],
            stream: true,
            temperature: None,
            keep_alive,
        };

        let mut request = self
            .http
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json");

        if conn.r#type == "azure_openai" || base.contains("azure.com") {
            request = request.header("api-key", api_key);
        }
        request = request.header("Accept", "text/event-stream");

        let resp = match request
            .json(&req)
            .send()
            .await
            .with_context(|| format!("POST {} failed", url))
        {
            Ok(r) => r,
            Err(e) => {
                let _ = token_tx.send(format!("\n[llm-error] {}", e));
                return Err(e);
            }
        };

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            let err_msg = format!("{} error ({}): {}", conn.r#type, status, body);
            let _ = token_tx.send(format!("\n[llm-error] {}", err_msg));
            anyhow::bail!("{}", err_msg);
        }

        self.handle_openai_sse_to_channel(resp, token_tx).await
    }

    async fn handle_openai_sse_to_channel(
        &self,
        resp: reqwest::Response,
        token_tx: UnboundedSender<String>,
    ) -> Result<String> {
        let mut stream = resp.bytes_stream();
        let mut full = String::new();
        let mut buffer = String::new();

        while let Some(chunk) = stream.next().await {
            let bytes = chunk?;
            buffer.push_str(&String::from_utf8_lossy(&bytes));

            while let Some(pos) = buffer.find('\n') {
                let line = buffer[..pos].trim_end_matches('\r').to_string();
                buffer = buffer[pos + 1..].to_string();

                if line.is_empty() {
                    continue;
                }
                if let Some(data) = line.strip_prefix("data: ") {
                    if data == "[DONE]" {
                        return Ok(full);
                    }
                    if let Ok(chunk) = serde_json::from_str::<OpenAiStreamChunk>(data) {
                        if let Some(delta) = chunk
                            .choices
                            .into_iter()
                            .next()
                            .and_then(|c| c.delta.content)
                        {
                            if !delta.is_empty() {
                                full.push_str(&delta);
                                let _ = token_tx.send(delta);
                            }
                        }
                    }
                }
            }
        }

        // Flush remnant: the final chunk(s) often lack a trailing '\n' so the last "data: {...}"
        // (or partial) can be left in the buffer. Without this, tail tokens of a response are
        // silently dropped (classic source of "chopped" replies) even though [llm-full-wire] path
        // still captures the authoritative full for logging.
        if !buffer.trim().is_empty() {
            let line = buffer.trim_end_matches('\r').to_string();
            if let Some(data) = line.strip_prefix("data: ") {
                if data != "[DONE]" {
                    if let Ok(chunk) = serde_json::from_str::<OpenAiStreamChunk>(data) {
                        if let Some(delta) = chunk
                            .choices
                            .into_iter()
                            .next()
                            .and_then(|c| c.delta.content)
                        {
                            if !delta.is_empty() {
                                full.push_str(&delta);
                                let _ = token_tx.send(delta);
                            }
                        }
                    }
                }
            }
        }

        Ok(full)
    }

    // ──────────────────────────────────────────────────────────────────────────
    // Anthropic Messages API (streaming + non-streaming)
    // ──────────────────────────────────────────────────────────────────────────

    async fn chat_anthropic(
        &self,
        conn: &ProviderConnection,
        model: &str,
        api_key: &str,
        system: &str,
        user: &str,
        stream: bool,
    ) -> Result<String> {
        let base = conn
            .base_url
            .as_deref()
            .unwrap_or("https://api.anthropic.com")
            .trim_end_matches('/');

        let url = format!("{}/v1/messages", base);

        #[derive(Serialize)]
        struct Req<'a> {
            model: &'a str,
            max_tokens: u32,
            system: &'a str,
            messages: Vec<AnthMessage<'a>>,
            #[serde(skip_serializing_if = "std::ops::Not::not")]
            stream: bool,
        }
        #[derive(Serialize)]
        struct AnthMessage<'a> {
            role: &'a str,
            content: &'a str,
        }

        let req = Req {
            model,
            max_tokens: 8192,
            system,
            messages: vec![AnthMessage {
                role: "user",
                content: user,
            }],
            stream,
        };

        let mut request = self
            .http
            .post(&url)
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json");

        if stream {
            request = request.header("anthropic-beta", "messages-2023-12-15"); // for streaming
        }

        let resp = request.json(&req).send().await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("anthropic error ({}): {}", status, body);
        }

        if stream {
            return self.handle_anthropic_stream(resp).await;
        }

        #[derive(Deserialize)]
        struct AnthResp {
            content: Vec<AnthContent>,
        }
        #[derive(Deserialize)]
        struct AnthContent {
            text: String,
        }

        let body: AnthResp = resp.json().await?;
        Ok(body
            .content
            .into_iter()
            .next()
            .map(|c| c.text)
            .unwrap_or_default())
    }

    async fn handle_anthropic_stream(&self, resp: reqwest::Response) -> Result<String> {
        use tokio::io::AsyncWriteExt;

        let mut stream = resp.bytes_stream();
        let mut full = String::new();
        let mut out = stdout();
        let mut buffer = String::new();

        while let Some(chunk) = stream.next().await {
            let bytes = chunk?;
            buffer.push_str(&String::from_utf8_lossy(&bytes));

            while let Some(pos) = buffer.find('\n') {
                let line = buffer[..pos].trim_end_matches('\r').to_string();
                buffer = buffer[pos + 1..].to_string();

                if let Some(data) = line.strip_prefix("data: ") {
                    if let Ok(evt) = serde_json::from_str::<AnthStreamEvent>(data) {
                        if let Some(delta) = evt.delta.and_then(|d| d.text) {
                            if !delta.is_empty() {
                                full.push_str(&delta);
                                let _ = out.write_all(delta.as_bytes()).await;
                                let _ = out.flush().await;
                            }
                        }
                        if evt.r#type == "message_stop" {
                            let _ = out.write_all(b"\n").await;
                            return Ok(full);
                        }
                    }
                }
            }
        }
        if !full.ends_with('\n') {
            let _ = out.write_all(b"\n").await;
        }
        Ok(full)
    }

    async fn chat_anthropic_to_channel(
        &self,
        conn: &ProviderConnection,
        model: &str,
        api_key: &str,
        system: &str,
        user: &str,
        token_tx: UnboundedSender<String>,
    ) -> Result<String> {
        let base = conn
            .base_url
            .as_deref()
            .unwrap_or("https://api.anthropic.com")
            .trim_end_matches('/');

        let url = format!("{}/v1/messages", base);

        #[derive(Serialize)]
        struct Req<'a> {
            model: &'a str,
            max_tokens: u32,
            system: &'a str,
            messages: Vec<AnthMessage<'a>>,
            #[serde(skip_serializing_if = "std::ops::Not::not")]
            stream: bool,
        }
        #[derive(Serialize)]
        struct AnthMessage<'a> {
            role: &'a str,
            content: &'a str,
        }

        let req = Req {
            model,
            max_tokens: 8192,
            system,
            messages: vec![AnthMessage {
                role: "user",
                content: user,
            }],
            stream: true,
        };

        let request = self
            .http
            .post(&url)
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .header("anthropic-beta", "messages-2023-12-15");

        let resp = match request.json(&req).send().await {
            Ok(r) => r,
            Err(e) => {
                let _ = token_tx.send(format!("\n[llm-error] {}", e));
                return Err(e.into());
            }
        };

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            let err_msg = format!("anthropic error ({}): {}", status, body);
            let _ = token_tx.send(format!("\n[llm-error] {}", err_msg));
            anyhow::bail!("{}", err_msg);
        }

        self.handle_anthropic_stream_to_channel(resp, token_tx)
            .await
    }

    async fn handle_anthropic_stream_to_channel(
        &self,
        resp: reqwest::Response,
        token_tx: UnboundedSender<String>,
    ) -> Result<String> {
        let mut stream = resp.bytes_stream();
        let mut full = String::new();
        let mut buffer = String::new();

        while let Some(chunk) = stream.next().await {
            let bytes = chunk?;
            buffer.push_str(&String::from_utf8_lossy(&bytes));

            while let Some(pos) = buffer.find('\n') {
                let line = buffer[..pos].trim_end_matches('\r').to_string();
                buffer = buffer[pos + 1..].to_string();

                if let Some(data) = line.strip_prefix("data: ") {
                    if let Ok(evt) = serde_json::from_str::<AnthStreamEvent>(data) {
                        if let Some(delta) = evt.delta.and_then(|d| d.text) {
                            if !delta.is_empty() {
                                full.push_str(&delta);
                                let _ = token_tx.send(delta);
                            }
                        }
                        if evt.r#type == "message_stop" {
                            return Ok(full);
                        }
                    }
                }
            }
        }
        Ok(full)
    }

    // ──────────────────────────────────────────────────────────────────────────
    // Google Gemini (simple non-stream for v0)
    // ──────────────────────────────────────────────────────────────────────────

    async fn chat_google(
        &self,
        conn: &ProviderConnection,
        model: &str,
        api_key: &str,
        system: &str,
        user: &str,
    ) -> Result<String> {
        let base = conn
            .base_url
            .as_deref()
            .unwrap_or("https://generativelanguage.googleapis.com")
            .trim_end_matches('/');

        // Gemini uses {model}:generateContent?key=...
        let url = format!(
            "{}/v1beta/models/{}:generateContent?key={}",
            base, model, api_key
        );

        #[derive(Serialize)]
        struct Req {
            system_instruction: Option<SystemInst>,
            contents: Vec<GemContent>,
        }
        #[derive(Serialize)]
        struct SystemInst {
            parts: Vec<Part>,
        }
        #[derive(Serialize, Deserialize)]
        struct Part {
            text: String,
        }
        #[derive(Serialize)]
        struct GemContent {
            parts: Vec<Part>,
            role: Option<String>,
        }

        let req = Req {
            system_instruction: if system.trim().is_empty() {
                None
            } else {
                Some(SystemInst {
                    parts: vec![Part {
                        text: system.to_string(),
                    }],
                })
            },
            contents: vec![GemContent {
                parts: vec![Part {
                    text: user.to_string(),
                }],
                role: Some("user".to_string()),
            }],
        };

        let resp = self.http.post(&url).json(&req).send().await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("google error ({}): {}", status, body);
        }

        #[derive(Deserialize)]
        struct GemResp {
            candidates: Vec<Candidate>,
        }
        #[derive(Deserialize)]
        struct Candidate {
            content: Option<GemContentResp>,
        }
        #[derive(Deserialize)]
        struct GemContentResp {
            parts: Vec<Part>,
        }

        let body: GemResp = resp.json().await?;
        let text = body
            .candidates
            .into_iter()
            .flat_map(|c| c.content.into_iter().flat_map(|co| co.parts))
            .map(|p| p.text)
            .collect::<Vec<_>>()
            .join("");
        Ok(text)
    }

    /// Query Ollama's /api/ps to see which models are currently resident (and how much VRAM they occupy).
    pub async fn list_ollama_ps(&self) -> Result<Vec<OllamaPsModel>> {
        let url = "http://localhost:11434/api/ps";
        #[derive(Deserialize)]
        struct Ps {
            #[serde(default)]
            models: Vec<OllamaPsModel>,
        }
        if let Ok(resp) = self
            .http
            .get(url)
            .timeout(std::time::Duration::from_secs(2))
            .send()
            .await
        {
            if resp.status().is_success() {
                if let Ok(ps) = resp.json::<Ps>().await {
                    return Ok(ps.models);
                }
            }
        }
        Ok(vec![])
    }

    /// Ask Ollama to unload a specific model immediately (by sending a generate request with keep_alive=0).
    /// This is the supported way to drop a model from VRAM without restarting the server.
    pub async fn ollama_unload(&self, model: &str) -> Result<()> {
        let model = model.trim();
        if model.is_empty() {
            return Ok(());
        }
        let url = "http://localhost:11434/api/generate";
        let body = serde_json::json!({
            "model": model,
            "prompt": "",
            "stream": false,
            "keep_alive": 0
        });
        // Best-effort; timeouts or errors are non-fatal (model may have already been evicted).
        let _ = self
            .http
            .post(url)
            .timeout(std::time::Duration::from_secs(8))
            .json(&body)
            .send()
            .await;
        Ok(())
    }

    // ──────────────────────────────────────────────────────────────────────
    // Agentic tool-calling turn (streams text, returns any requested tool calls)
    // ──────────────────────────────────────────────────────────────────────

    /// Drive one assistant turn for the agent loop: send the full conversation
    /// `history` plus the available `tools`, stream text deltas over `token_tx`,
    /// and return the assistant's text together with any tool calls it requested.
    ///
    /// Provider support:
    /// - openai_compat / openai / azure_openai → OpenAI `tools` + `tool_calls`
    /// - anthropic → Messages API `tool_use` / `tool_result`
    /// - google / gemini → text-only fallback (no tool calls); the agent loop
    ///   then simply treats the reply as a final answer.
    #[allow(clippy::too_many_arguments)] // provider call: conn + model + history + tools + sinks
    pub async fn chat_turn_stream(
        &self,
        conn: &ProviderConnection,
        model: &str,
        api_key: &str,
        system: &str,
        history: &[ChatMessage],
        tools: &[ToolDef],
        token_tx: UnboundedSender<String>,
    ) -> Result<AssistantTurn> {
        match conn.r#type.as_str() {
            "openai_compat" | "openai" | "azure_openai" => {
                self.openai_turn_stream(conn, model, api_key, system, history, tools, token_tx)
                    .await
            }
            "anthropic" => {
                self.anthropic_turn_stream(conn, model, api_key, system, history, tools, token_tx)
                    .await
            }
            "google" | "google_ai_studio" | "gemini" => {
                self.google_turn_stream(conn, model, api_key, system, history, tools, token_tx)
                    .await
            }
            other => {
                let msg = format!("provider type '{}' does not support agent turns yet", other);
                let _ = token_tx.send(format!("\n[llm-error] {}", msg));
                anyhow::bail!("{}", msg)
            }
        }
    }

    #[allow(clippy::too_many_arguments)] // provider call: conn + model + history + tools + sinks
    async fn openai_turn_stream(
        &self,
        conn: &ProviderConnection,
        model: &str,
        api_key: &str,
        system: &str,
        history: &[ChatMessage],
        tools: &[ToolDef],
        token_tx: UnboundedSender<String>,
    ) -> Result<AssistantTurn> {
        let base = Self::openai_compat_base(conn)?;
        let url = format!("{}/chat/completions", base);

        // Build the messages array (system + history) in OpenAI wire form.
        let messages = build_openai_messages(system, history);

        let base_for_detect = conn.base_url.as_deref().unwrap_or("");
        let is_ollama =
            base_for_detect.contains("11434") || base_for_detect.to_lowercase().contains("ollama");
        let keep_alive = conn.keep_alive.clone().or_else(|| {
            if is_ollama {
                Some("30s".to_string())
            } else {
                None
            }
        });

        let mut body = json!({
            "model": model,
            "messages": messages,
            "stream": true,
        });
        if !tools.is_empty() {
            let tools_json: Vec<Value> = tools
                .iter()
                .map(|t| {
                    json!({
                        "type": "function",
                        "function": {
                            "name": t.name,
                            "description": t.description,
                            "parameters": t.input_schema,
                        }
                    })
                })
                .collect();
            body["tools"] = json!(tools_json);
        }
        if let Some(k) = keep_alive {
            body["keep_alive"] = json!(k);
        }

        // Bounded retry on transient failures BEFORE any tokens stream (so there's
        // no risk of duplicated output): network/connection errors, 408/429, 5xx,
        // and the ambiguous 400 some busy providers (e.g. xAI) throw under load.
        // Clear auth/not-found errors (401/403/404) fail fast — retrying won't help.
        const MAX_ATTEMPTS: u32 = 3;
        let mut attempt: u32 = 0;
        let resp = loop {
            attempt += 1;
            let mut request = self
                .http
                .post(&url)
                .header("Authorization", format!("Bearer {}", api_key))
                .header("Content-Type", "application/json")
                .header("Accept", "text/event-stream");
            if conn.r#type == "azure_openai" || base.contains("azure.com") {
                request = request.header("api-key", api_key);
            }

            match request.json(&body).send().await {
                Ok(r) if r.status().is_success() => break r,
                Ok(r) => {
                    let status = r.status();
                    let retryable = status.is_server_error()
                        || status.as_u16() == 400
                        || status.as_u16() == 408
                        || status.as_u16() == 429;
                    if retryable && attempt < MAX_ATTEMPTS {
                        let _ = token_tx.send(format!(
                            "[note]{} returned {} — retrying ({}/{})…",
                            conn.r#type, status, attempt, MAX_ATTEMPTS
                        ));
                        tokio::time::sleep(std::time::Duration::from_millis(500 * attempt as u64))
                            .await;
                        continue;
                    }
                    // Give up: capture the rejected request (→ .anvil/last-llm-error.json;
                    // no secret — the key is an auth header, not in `body`) and surface it.
                    let text = r.text().await.unwrap_or_default();
                    let msg = format!("{} error ({}): {}", conn.r#type, status, text);
                    let diag = json!({
                        "url": url,
                        "status": status.as_u16(),
                        "response": text,
                        "request": body,
                    });
                    let _ = token_tx.send(format!(
                        "[error-request]{}",
                        serde_json::to_string_pretty(&diag).unwrap_or_default()
                    ));
                    let _ = token_tx.send(format!("\n[llm-error] {}", msg));
                    anyhow::bail!("{}", msg);
                }
                Err(e) => {
                    if attempt < MAX_ATTEMPTS {
                        let _ = token_tx.send(format!(
                            "[note]request error ({}) — retrying ({}/{})…",
                            e, attempt, MAX_ATTEMPTS
                        ));
                        tokio::time::sleep(std::time::Duration::from_millis(500 * attempt as u64))
                            .await;
                        continue;
                    }
                    let _ = token_tx.send(format!("\n[llm-error] {}", e));
                    return Err(e.into());
                }
            }
        };

        self.handle_openai_tool_stream(resp, token_tx).await
    }

    async fn handle_openai_tool_stream(
        &self,
        resp: reqwest::Response,
        token_tx: UnboundedSender<String>,
    ) -> Result<AssistantTurn> {
        let mut stream = resp.bytes_stream();
        let mut text = String::new();
        let mut buffer = String::new();
        // index -> (id, name, accumulated-arguments-json-string)
        let mut tc_acc: BTreeMap<usize, (String, String, String)> = BTreeMap::new();

        let handle_line = |line: &str,
                           text: &mut String,
                           tc_acc: &mut BTreeMap<usize, (String, String, String)>|
         -> bool {
            // returns true if [DONE] seen
            if let Some(data) = line.strip_prefix("data: ") {
                if data == "[DONE]" {
                    return true;
                }
                if let Ok(chunk) = serde_json::from_str::<OpenAiStreamChunk>(data) {
                    if let Some(choice) = chunk.choices.into_iter().next() {
                        if let Some(delta) = choice.delta.content {
                            if !delta.is_empty() {
                                text.push_str(&delta);
                                let _ = token_tx.send(delta);
                            }
                        }
                        if let Some(calls) = choice.delta.tool_calls {
                            for d in calls {
                                let entry = tc_acc.entry(d.index).or_default();
                                if let Some(id) = d.id {
                                    if !id.is_empty() {
                                        entry.0 = id;
                                    }
                                }
                                if let Some(f) = d.function {
                                    if let Some(n) = f.name {
                                        if !n.is_empty() {
                                            entry.1 = n;
                                        }
                                    }
                                    if let Some(a) = f.arguments {
                                        entry.2.push_str(&a);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            false
        };

        let mut done = false;
        while let Some(chunk) = stream.next().await {
            let bytes = chunk?;
            buffer.push_str(&String::from_utf8_lossy(&bytes));
            while let Some(pos) = buffer.find('\n') {
                let line = buffer[..pos].trim_end_matches('\r').to_string();
                buffer = buffer[pos + 1..].to_string();
                if line.is_empty() {
                    continue;
                }
                if handle_line(&line, &mut text, &mut tc_acc) {
                    done = true;
                    break;
                }
            }
            if done {
                break;
            }
        }
        // Flush any trailing partial line (last chunk often lacks a newline).
        if !done && !buffer.trim().is_empty() {
            let line = buffer.trim_end_matches('\r').to_string();
            handle_line(&line, &mut text, &mut tc_acc);
        }

        Ok(AssistantTurn {
            text,
            tool_calls: finalize_tool_calls(tc_acc),
        })
    }

    #[allow(clippy::too_many_arguments)] // provider call: conn + model + history + tools + sinks
    async fn anthropic_turn_stream(
        &self,
        conn: &ProviderConnection,
        model: &str,
        api_key: &str,
        system: &str,
        history: &[ChatMessage],
        tools: &[ToolDef],
        token_tx: UnboundedSender<String>,
    ) -> Result<AssistantTurn> {
        let base = conn
            .base_url
            .as_deref()
            .unwrap_or("https://api.anthropic.com")
            .trim_end_matches('/');
        let url = format!("{}/v1/messages", base);

        // Map history to Anthropic messages. Consecutive tool results must be
        // collapsed into a single user message of tool_result blocks.
        let messages = build_anthropic_messages(history);

        let mut body = json!({
            "model": model,
            "max_tokens": 8192,
            "system": system,
            "messages": messages,
            "stream": true,
        });
        if !tools.is_empty() {
            let tools_json: Vec<Value> = tools
                .iter()
                .map(|t| {
                    json!({
                        "name": t.name,
                        "description": t.description,
                        "input_schema": t.input_schema,
                    })
                })
                .collect();
            body["tools"] = json!(tools_json);
        }

        let request = self
            .http
            .post(&url)
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json");

        let resp = match request.json(&body).send().await {
            Ok(r) => r,
            Err(e) => {
                let _ = token_tx.send(format!("\n[llm-error] {}", e));
                return Err(e.into());
            }
        };
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            let msg = format!("anthropic error ({}): {}", status, text);
            let _ = token_tx.send(format!("\n[llm-error] {}", msg));
            anyhow::bail!("{}", msg);
        }

        self.handle_anthropic_tool_stream(resp, token_tx).await
    }

    async fn handle_anthropic_tool_stream(
        &self,
        resp: reqwest::Response,
        token_tx: UnboundedSender<String>,
    ) -> Result<AssistantTurn> {
        let mut stream = resp.bytes_stream();
        let mut text = String::new();
        let mut buffer = String::new();
        // index -> (id, name, accumulated-input-json-string)
        let mut blocks: BTreeMap<usize, (String, String, String)> = BTreeMap::new();
        let mut is_tool: BTreeMap<usize, bool> = BTreeMap::new();

        while let Some(chunk) = stream.next().await {
            let bytes = chunk?;
            buffer.push_str(&String::from_utf8_lossy(&bytes));
            while let Some(pos) = buffer.find('\n') {
                let line = buffer[..pos].trim_end_matches('\r').to_string();
                buffer = buffer[pos + 1..].to_string();
                let data = match line.strip_prefix("data: ") {
                    Some(d) => d,
                    None => continue,
                };
                let evt: AnthEvt = match serde_json::from_str(data) {
                    Ok(e) => e,
                    Err(_) => continue,
                };
                match evt.r#type.as_str() {
                    "content_block_start" => {
                        let idx = evt.index.unwrap_or(0);
                        if let Some(cb) = evt.content_block {
                            let tool = cb.r#type == "tool_use";
                            is_tool.insert(idx, tool);
                            blocks.insert(
                                idx,
                                (
                                    cb.id.unwrap_or_default(),
                                    cb.name.unwrap_or_default(),
                                    String::new(),
                                ),
                            );
                        }
                    }
                    "content_block_delta" => {
                        let idx = evt.index.unwrap_or(0);
                        if let Some(d) = evt.delta {
                            if let Some(t) = d.text {
                                if !t.is_empty() {
                                    text.push_str(&t);
                                    let _ = token_tx.send(t);
                                }
                            }
                            if let Some(pj) = d.partial_json {
                                blocks.entry(idx).or_default().2.push_str(&pj);
                            }
                        }
                    }
                    "message_stop" => {
                        return Ok(self.build_anthropic_turn(text, blocks, is_tool));
                    }
                    _ => {}
                }
            }
        }
        Ok(self.build_anthropic_turn(text, blocks, is_tool))
    }

    fn build_anthropic_turn(
        &self,
        text: String,
        blocks: BTreeMap<usize, (String, String, String)>,
        is_tool: BTreeMap<usize, bool>,
    ) -> AssistantTurn {
        let mut tool_calls = vec![];
        for (idx, (id, name, json_str)) in blocks {
            if *is_tool.get(&idx).unwrap_or(&false) {
                let arguments = if json_str.trim().is_empty() {
                    json!({})
                } else {
                    serde_json::from_str(&json_str).unwrap_or_else(|_| json!({}))
                };
                tool_calls.push(ToolCall {
                    id,
                    name,
                    arguments,
                });
            }
        }
        AssistantTurn { text, tool_calls }
    }
}

/// Build the OpenAI Chat Completions `messages` array (system + history),
/// mapping assistant `tool_calls` and `tool` results to the wire format.
/// Pure (no I/O) so the mapping can be unit-tested offline.
fn build_openai_messages(system: &str, history: &[ChatMessage]) -> Vec<Value> {
    let mut messages: Vec<Value> = vec![json!({"role": "system", "content": system})];
    for m in history {
        match m.role {
            Role::User => messages.push(json!({"role": "user", "content": m.text})),
            Role::Assistant => {
                let mut obj = serde_json::Map::new();
                obj.insert("role".into(), json!("assistant"));
                // For a purely-tool-call turn the text is empty. OpenAI allows a
                // null content here, but some OpenAI-compatible servers (notably
                // Ollama: "invalid message content type: <nil>") reject null and
                // require a string — so always send a string ("" when empty).
                obj.insert("content".into(), json!(m.text));
                if !m.tool_calls.is_empty() {
                    let tcs: Vec<Value> = m
                        .tool_calls
                        .iter()
                        .map(|tc| {
                            json!({
                                "id": tc.id,
                                "type": "function",
                                "function": {
                                    "name": tc.name,
                                    "arguments": tc.arguments.to_string(),
                                }
                            })
                        })
                        .collect();
                    obj.insert("tool_calls".into(), json!(tcs));
                }
                messages.push(Value::Object(obj));
            }
            Role::Tool => messages.push(json!({
                "role": "tool",
                "tool_call_id": m.tool_call_id.clone().unwrap_or_default(),
                "content": m.text,
            })),
        }
    }
    messages
}

/// Build the Anthropic Messages `messages` array, mapping assistant tool calls
/// to `tool_use` blocks and collapsing consecutive tool results into a single
/// user message of `tool_result` blocks. Pure (no I/O) for unit-testing.
fn build_anthropic_messages(history: &[ChatMessage]) -> Vec<Value> {
    let mut messages: Vec<Value> = vec![];
    let mut i = 0;
    while i < history.len() {
        let m = &history[i];
        match m.role {
            Role::User => {
                messages.push(json!({
                    "role": "user",
                    "content": [{"type": "text", "text": m.text}],
                }));
                i += 1;
            }
            Role::Assistant => {
                let mut blocks: Vec<Value> = vec![];
                if !m.text.is_empty() {
                    blocks.push(json!({"type": "text", "text": m.text}));
                }
                for tc in &m.tool_calls {
                    blocks.push(json!({
                        "type": "tool_use",
                        "id": tc.id,
                        "name": tc.name,
                        "input": tc.arguments,
                    }));
                }
                messages.push(json!({"role": "assistant", "content": blocks}));
                i += 1;
            }
            Role::Tool => {
                let mut blocks: Vec<Value> = vec![];
                while i < history.len() && history[i].role == Role::Tool {
                    let t = &history[i];
                    blocks.push(json!({
                        "type": "tool_result",
                        "tool_use_id": t.tool_call_id.clone().unwrap_or_default(),
                        "content": t.text,
                    }));
                    i += 1;
                }
                messages.push(json!({"role": "user", "content": blocks}));
            }
        }
    }
    messages
}

/// Collapse history into a single text blob (used for providers without a
/// native multi-turn/tool format, e.g. the Gemini fallback).
#[allow(dead_code)]
fn flatten_history_to_text(history: &[ChatMessage]) -> String {
    let mut out = String::new();
    for m in history {
        match m.role {
            Role::User => out.push_str(&format!("\nUser: {}\n", m.text)),
            Role::Assistant => {
                if !m.text.is_empty() {
                    out.push_str(&format!("\nAssistant: {}\n", m.text));
                }
            }
            Role::Tool => out.push_str(&format!("\n[tool result] {}\n", m.text)),
        }
    }
    out
}

/// Build final `ToolCall`s from accumulated OpenAI streaming deltas, parsing the
/// argument strings into JSON (falling back to `{}` if a model emits invalid JSON).
fn finalize_tool_calls(acc: BTreeMap<usize, (String, String, String)>) -> Vec<ToolCall> {
    acc.into_iter()
        .filter(|(_, (_, name, _))| !name.is_empty())
        .map(|(_, (id, name, args))| {
            let arguments = if args.trim().is_empty() {
                json!({})
            } else {
                serde_json::from_str(&args).unwrap_or_else(|_| json!({}))
            };
            ToolCall {
                id,
                name,
                arguments,
            }
        })
        .collect()
}

// ── Stream event shapes (best effort, we only care about the text deltas) ─────

#[derive(Deserialize)]
struct OpenAiStreamChunk {
    choices: Vec<OpenAiStreamChoice>,
}

#[derive(Deserialize)]
struct OpenAiStreamChoice {
    delta: OpenAiDelta,
}

#[derive(Deserialize)]
struct OpenAiDelta {
    #[serde(default)]
    content: Option<String>,
    /// Present on agent turns; absent (None) for plain text streams.
    #[serde(default)]
    tool_calls: Option<Vec<OpenAiToolCallDelta>>,
}

/// One element of a streaming `delta.tool_calls` array. The `index` correlates
/// fragments of the same call across chunks; `id`/`name` arrive early and
/// `arguments` accumulates as a partial JSON string.
#[derive(Deserialize)]
struct OpenAiToolCallDelta {
    #[serde(default)]
    index: usize,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    function: Option<OpenAiFnDelta>,
}

#[derive(Deserialize)]
struct OpenAiFnDelta {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
}

#[derive(Deserialize)]
struct AnthStreamEvent {
    #[serde(rename = "type")]
    r#type: String,
    delta: Option<AnthDelta>,
}

#[derive(Deserialize)]
struct AnthDelta {
    text: Option<String>,
}

// Richer Anthropic stream event used by the tool-aware agent parser.
#[derive(Deserialize)]
struct AnthEvt {
    #[serde(rename = "type")]
    r#type: String,
    #[serde(default)]
    index: Option<usize>,
    #[serde(default)]
    content_block: Option<AnthBlock>,
    #[serde(default)]
    delta: Option<AnthEvtDelta>,
}

#[derive(Deserialize)]
struct AnthBlock {
    #[serde(rename = "type")]
    r#type: String,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: Option<String>,
}

#[derive(Deserialize)]
struct AnthEvtDelta {
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    partial_json: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A realistic history: user → assistant(tool_call) → tool_result → user.
    fn sample_history() -> Vec<ChatMessage> {
        vec![
            ChatMessage::user("read the readme"),
            ChatMessage::assistant(
                "",
                vec![ToolCall {
                    id: "call_1".into(),
                    name: "read_file".into(),
                    arguments: json!({"path": "README.md"}),
                }],
            ),
            ChatMessage::tool_result("call_1", "# Anvil\nhello"),
            ChatMessage::user("now summarize it"),
        ]
    }

    #[test]
    fn openai_messages_map_tool_calls_and_results() {
        let msgs = build_openai_messages("SYS", &sample_history());
        assert_eq!(msgs[0]["role"], "system");
        assert_eq!(msgs[0]["content"], "SYS");
        // assistant turn carries a tool_calls array; content is an empty string
        // (not null — Ollama's OpenAI-compat layer rejects a null content).
        assert_eq!(msgs[2]["role"], "assistant");
        assert_eq!(msgs[2]["content"], "");
        let tc = &msgs[2]["tool_calls"][0];
        assert_eq!(tc["id"], "call_1");
        assert_eq!(tc["type"], "function");
        assert_eq!(tc["function"]["name"], "read_file");
        // arguments are serialized as a JSON *string* per OpenAI spec
        assert_eq!(tc["function"]["arguments"], "{\"path\":\"README.md\"}");
        // tool result becomes a role:"tool" message keyed by tool_call_id
        assert_eq!(msgs[3]["role"], "tool");
        assert_eq!(msgs[3]["tool_call_id"], "call_1");
        assert_eq!(msgs[3]["content"], "# Anvil\nhello");
    }

    #[test]
    fn anthropic_messages_map_tool_use_and_result_blocks() {
        let msgs = build_anthropic_messages(&sample_history());
        // user, assistant(tool_use), user(tool_result), user
        assert_eq!(msgs.len(), 4);
        let use_block = &msgs[1]["content"][0];
        assert_eq!(msgs[1]["role"], "assistant");
        assert_eq!(use_block["type"], "tool_use");
        assert_eq!(use_block["id"], "call_1");
        assert_eq!(use_block["name"], "read_file");
        assert_eq!(use_block["input"]["path"], "README.md");
        // tool result is a user message with a tool_result block (not a JSON string)
        let res_block = &msgs[2]["content"][0];
        assert_eq!(msgs[2]["role"], "user");
        assert_eq!(res_block["type"], "tool_result");
        assert_eq!(res_block["tool_use_id"], "call_1");
        assert_eq!(res_block["content"], "# Anvil\nhello");
    }

    #[test]
    fn anthropic_collapses_consecutive_tool_results() {
        let history = vec![
            ChatMessage::assistant(
                "",
                vec![
                    ToolCall {
                        id: "a".into(),
                        name: "read_file".into(),
                        arguments: json!({}),
                    },
                    ToolCall {
                        id: "b".into(),
                        name: "list_dir".into(),
                        arguments: json!({}),
                    },
                ],
            ),
            ChatMessage::tool_result("a", "file a"),
            ChatMessage::tool_result("b", "dir b"),
        ];
        let msgs = build_anthropic_messages(&history);
        // The two tool results collapse into ONE user message with two blocks.
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[1]["role"], "user");
        assert_eq!(msgs[1]["content"].as_array().unwrap().len(), 2);
        assert_eq!(msgs[1]["content"][0]["tool_use_id"], "a");
        assert_eq!(msgs[1]["content"][1]["tool_use_id"], "b");
    }

    #[test]
    fn chat_messages_round_trip_json() {
        let history = sample_history();
        let json = serde_json::to_string(&history).unwrap();
        let back: Vec<ChatMessage> = serde_json::from_str(&json).unwrap();
        assert_eq!(back.len(), history.len());
        assert_eq!(back[0].role, Role::User);
        assert_eq!(back[1].role, Role::Assistant);
        assert_eq!(back[1].tool_calls[0].name, "read_file");
        assert_eq!(back[1].tool_calls[0].arguments["path"], "README.md");
        assert_eq!(back[2].role, Role::Tool);
        assert_eq!(back[2].tool_call_id.as_deref(), Some("call_1"));
    }

    #[test]
    fn finalize_skips_nameless_and_parses_args() {
        let mut acc: BTreeMap<usize, (String, String, String)> = BTreeMap::new();
        acc.insert(
            0,
            ("id0".into(), "write_file".into(), "{\"path\":\"x\"}".into()),
        );
        acc.insert(1, ("id1".into(), String::new(), "garbage".into())); // no name → dropped
        let calls = finalize_tool_calls(acc);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "write_file");
        assert_eq!(calls[0].arguments["path"], "x");
    }
}

/// Public so the TUI can display loaded models + VRAM usage from `ollama ps`.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct OllamaPsModel {
    pub name: String,
    #[serde(default)]
    pub size: u64,
    #[serde(default, rename = "size_vram")]
    pub size_vram: u64,
}
