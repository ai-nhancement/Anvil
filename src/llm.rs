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

use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncWriteExt, stdout};
use tokio::sync::mpsc::UnboundedSender;

use crate::config::{CredentialRef, ProviderConnection};

/// High-level client. Cheap to clone (Arc under the hood).
#[derive(Clone)]
pub struct LlmClient {
    http: Arc<Client>,
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
        Self { http: Arc::new(http) }
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
                    .map_err(|e| anyhow!("failed to read keyring for {}: {}", conn_name, e))
            }
            CredentialRef::Env { var_name } => {
                if let Ok(val) = std::env::var(var_name) {
                    if !val.trim().is_empty() {
                        return Ok(val);
                    }
                }
                // Graceful fallback for local Ollama (and similar): the quick setup and docs
                // have long said "any non-empty string works (or omit)". We now make the env
                // truly optional for the conventional OLLAMA_API_KEY case so first-run "just works".
                if var_name == "OLLAMA_API_KEY" || var_name.to_uppercase().contains("OLLAMA") {
                    return Ok("ollama".to_string());
                }
                anyhow::bail!("environment variable {} not set (for provider {})", var_name, conn_name)
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
        if let Ok(ids) = self.list_openai_compat_models("http://localhost:11434/v1", "ollama").await {
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
    pub async fn list_openai_compat_models(&self, base_url: &str, api_key: &str) -> Result<Vec<String>> {
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
                self.chat_openai_compat(conn, model, api_key, system, user, false).await
            }
            "anthropic" => self.chat_anthropic(conn, model, api_key, system, user, false).await,
            "google" | "google_ai_studio" | "gemini" => self.chat_google(conn, model, api_key, system, user).await,
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
                self.chat_openai_compat(conn, model, api_key, system, user, true).await
            }
            "anthropic" => self.chat_anthropic(conn, model, api_key, system, user, true).await,
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
                anyhow::bail!("provider type '{}' does not support streaming yet (or is not implemented)", other)
            }
        }
    }

    /// Channel-based streaming chat for the TUI (and other non-stdout consumers).
    ///
    /// Sends every content delta as it arrives over `token_tx` (UnboundedSender for simplicity).
    /// Returns the full concatenated text on successful completion.
    /// The provided sender is dropped when the stream ends (receiver will see disconnect).
    /// **Never writes to stdout** — required when the terminal is in raw/alternate mode.
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
                self.chat_openai_compat_to_channel(conn, model, api_key, system, user, token_tx).await
            }
            "anthropic" => {
                self.chat_anthropic_to_channel(conn, model, api_key, system, user, token_tx).await
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
                let msg = format!("provider type '{}' does not support streaming yet (or is not implemented)", other);
                let _ = token_tx.send(format!("\n[llm-error] {}", msg));
                anyhow::bail!("{}", msg)
            }
        }
    }

    // ──────────────────────────────────────────────────────────────────────────
    // OpenAI-compatible (Chat Completions) — the workhorse for Ollama + 80% of others
    // ──────────────────────────────────────────────────────────────────────────

    async fn chat_openai_compat(
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
            .unwrap_or("https://api.openai.com/v1")
            .trim_end_matches('/');

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
        let is_ollama = base_for_detect.contains("11434") || base_for_detect.to_lowercase().contains("ollama");
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
                Message { role: "system", content: system },
                Message { role: "user", content: user },
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
                        if let Some(delta) = chunk.choices.into_iter().next().and_then(|c| c.delta.content) {
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
        let base = conn
            .base_url
            .as_deref()
            .unwrap_or("https://api.openai.com/v1")
            .trim_end_matches('/');

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
        let is_ollama = base_for_detect.contains("11434") || base_for_detect.to_lowercase().contains("ollama");
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
                Message { role: "system", content: system },
                Message { role: "user", content: user },
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
                        if let Some(delta) = chunk.choices.into_iter().next().and_then(|c| c.delta.content) {
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
                        if let Some(delta) = chunk.choices.into_iter().next().and_then(|c| c.delta.content) {
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
            messages: vec![AnthMessage { role: "user", content: user }],
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
        Ok(body.content.into_iter().next().map(|c| c.text).unwrap_or_default())
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
            messages: vec![AnthMessage { role: "user", content: user }],
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

        self.handle_anthropic_stream_to_channel(resp, token_tx).await
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
        let url = format!("{}/v1beta/models/{}:generateContent?key={}", base, model, api_key);

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
                    parts: vec![Part { text: system.to_string() }],
                })
            },
            contents: vec![GemContent {
                parts: vec![Part { text: user.to_string() }],
                role: Some("user".to_string()),
            }],
        };

        let resp = self
            .http
            .post(&url)
            .json(&req)
            .send()
            .await?;

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

/// Public so the TUI can display loaded models + VRAM usage from `ollama ps`.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct OllamaPsModel {
    pub name: String,
    #[serde(default)]
    pub size: u64,
    #[serde(default, rename = "size_vram")]
    pub size_vram: u64,
}
