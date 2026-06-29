use std::collections::BTreeMap;

use anyhow::Result;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::mpsc::UnboundedSender;

use crate::config::ProviderConnection;
use crate::llm::{AssistantTurn, ChatMessage, LlmClient, Role, ToolCall, ToolDef};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiRequest {
    pub system_instruction: Option<GeminiSystemInstruction>,
    pub contents: Vec<GeminiContent>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<GeminiTool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_config: Option<GeminiToolConfig>,
}

#[derive(Serialize)]
pub struct GeminiSystemInstruction {
    pub parts: Vec<GeminiPart>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GeminiContent {
    pub role: String, // "user" or "model"
    pub parts: Vec<GeminiPart>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum GeminiPart {
    Text {
        text: String,
    },
    FunctionCall {
        #[serde(rename = "functionCall")]
        function_call: GeminiFunctionCall,
    },
    FunctionResponse {
        #[serde(rename = "functionResponse")]
        function_response: GeminiFunctionResponse,
    },
    Unknown(Value),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GeminiFunctionCall {
    pub name: String,
    pub args: Value,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GeminiFunctionResponse {
    pub name: String,
    pub response: Value,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiTool {
    pub function_declarations: Vec<GeminiFunctionDeclaration>,
}

#[derive(Serialize)]
pub struct GeminiFunctionDeclaration {
    pub name: String,
    pub description: String,
    pub parameters: Value, // Uppercased JSON Schema
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiToolConfig {
    pub function_calling_config: GeminiFunctionCallingConfig,
}

#[derive(Serialize)]
pub struct GeminiFunctionCallingConfig {
    pub mode: String, // "AUTO"
}

// Response streaming chunk types
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GeminiStreamChunk {
    pub candidates: Option<Vec<GeminiCandidate>>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GeminiCandidate {
    pub content: Option<GeminiCandidateContent>,
    #[allow(dead_code)]
    pub finish_reason: Option<String>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GeminiCandidateContent {
    #[allow(dead_code)]
    pub role: Option<String>,
    pub parts: Option<Vec<GeminiPart>>,
}

struct GeminiStreamParser {
    buffer: String,
}

impl GeminiStreamParser {
    fn new() -> Self {
        Self {
            buffer: String::new(),
        }
    }

    fn push(&mut self, text: &str) {
        self.buffer.push_str(text);
    }

    fn next_object(&mut self) -> Option<Value> {
        let mut trimmed = self.buffer.trim_start();
        if trimmed.starts_with('[') {
            trimmed = &trimmed[1..];
        }
        trimmed = trimmed.trim_start();
        if trimmed.starts_with(',') {
            trimmed = &trimmed[1..];
        }
        trimmed = trimmed.trim_start();

        if trimmed.is_empty() || trimmed == "]" {
            self.buffer = trimmed.to_string();
            return None;
        }

        let mut de = serde_json::Deserializer::from_str(trimmed).into_iter::<Value>();
        if let Some(Ok(value)) = de.next() {
            let consumed = de.byte_offset();
            self.buffer = trimmed[consumed..].to_string();
            Some(value)
        } else {
            None
        }
    }
}

pub fn uppercase_types(val: &mut Value) {
    match val {
        Value::Object(map) => {
            if let Some(t) = map.get_mut("type") {
                if let Value::String(s) = t {
                    *s = s.to_ascii_uppercase();
                } else if let Value::Array(arr) = t {
                    for item in arr {
                        if let Value::String(s) = item {
                            *s = s.to_ascii_uppercase();
                        }
                    }
                }
            }
            for (_k, v) in map.iter_mut() {
                uppercase_types(v);
            }
        }
        Value::Array(arr) => {
            for item in arr {
                uppercase_types(item);
            }
        }
        _ => {}
    }
}

pub fn translate_tool_definition(def: &ToolDef) -> GeminiFunctionDeclaration {
    let mut parameters = def.input_schema.clone();
    uppercase_types(&mut parameters);

    GeminiFunctionDeclaration {
        name: def.name.clone(),
        description: def.description.clone(),
        parameters,
    }
}

pub fn translate_chat_history(history: &[ChatMessage]) -> Vec<GeminiContent> {
    let mut tool_call_id_to_name = BTreeMap::new();
    for m in history {
        if m.role == Role::Assistant {
            for tc in &m.tool_calls {
                tool_call_id_to_name.insert(tc.id.clone(), tc.name.clone());
            }
        }
    }

    let mut contents = Vec::new();
    let mut i = 0;
    while i < history.len() {
        let m = &history[i];
        let gemini_role = match m.role {
            Role::User | Role::Tool => "user",
            Role::Assistant => "model",
        };

        let mut parts = Vec::new();
        while i < history.len() {
            let next_m = &history[i];
            let next_gemini_role = match next_m.role {
                Role::User | Role::Tool => "user",
                Role::Assistant => "model",
            };
            if next_gemini_role != gemini_role {
                break;
            }

            match next_m.role {
                Role::User => {
                    if !next_m.text.is_empty() {
                        parts.push(GeminiPart::Text {
                            text: next_m.text.clone(),
                        });
                    }
                }
                Role::Assistant => {
                    if !next_m.text.is_empty() {
                        parts.push(GeminiPart::Text {
                            text: next_m.text.clone(),
                        });
                    }
                    for tc in &next_m.tool_calls {
                        parts.push(GeminiPart::FunctionCall {
                            function_call: GeminiFunctionCall {
                                name: tc.name.clone(),
                                args: tc.arguments.clone(),
                            },
                        });
                    }
                }
                Role::Tool => {
                    let tool_call_id = next_m.tool_call_id.clone().unwrap_or_default();
                    let name = tool_call_id_to_name
                        .get(&tool_call_id)
                        .cloned()
                        .unwrap_or_else(|| "tool_response".to_string());

                    let response = serde_json::from_str::<Value>(&next_m.text)
                        .ok()
                        .filter(|v| v.is_object())
                        .unwrap_or_else(|| json!({ "output": next_m.text }));

                    parts.push(GeminiPart::FunctionResponse {
                        function_response: GeminiFunctionResponse { name, response },
                    });
                }
            }
            i += 1;
        }

        if parts.is_empty() {
            parts.push(GeminiPart::Text {
                text: String::new(),
            });
        }

        contents.push(GeminiContent {
            role: gemini_role.to_string(),
            parts,
        });
    }

    contents
}

impl LlmClient {
    #[allow(clippy::too_many_arguments)]
    pub async fn google_turn_stream(
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
            .unwrap_or("https://generativelanguage.googleapis.com")
            .trim_end_matches('/');

        let model_path = if model.starts_with("models/") {
            model.to_string()
        } else {
            format!("models/{}", model)
        };

        let gemini_tools = if !tools.is_empty() {
            let decls = tools.iter().map(translate_tool_definition).collect();
            vec![GeminiTool {
                function_declarations: decls,
            }]
        } else {
            vec![]
        };

        let tool_config = if !tools.is_empty() {
            Some(GeminiToolConfig {
                function_calling_config: GeminiFunctionCallingConfig {
                    mode: "AUTO".to_string(),
                },
            })
        } else {
            None
        };

        let req = GeminiRequest {
            system_instruction: if system.trim().is_empty() {
                None
            } else {
                Some(GeminiSystemInstruction {
                    parts: vec![GeminiPart::Text {
                        text: system.to_string(),
                    }],
                })
            },
            contents: translate_chat_history(history),
            tools: gemini_tools,
            tool_config,
        };

        let endpoint = format!("{}/v1beta/{}:streamGenerateContent", base, model_path);

        let mut req_builder = self.http.post(&endpoint);

        // Prefer query key for Google AI Studio / generativelanguage bases (AIza keys).
        // This matches the legacy chat_google behavior and what works for native Gemini.
        let use_query_key = api_key.starts_with("AIza")
            || api_key.starts_with("AQ.")
            || base.contains("generativelanguage");
        if use_query_key {
            req_builder = req_builder.query(&[("key", api_key)]);
        } else {
            req_builder = req_builder.header("Authorization", format!("Bearer {}", api_key));
        }

        let resp = req_builder.json(&req).send().await.map_err(|e| {
            let msg = format!("google request error: {}", e);
            let _ = token_tx.send(format!("\n[llm-error] {}", msg));
            e
        })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            let msg = format!("google stream error ({}): {}", status, body);
            let _ = token_tx.send(format!("\n[llm-error] {}", msg));
            anyhow::bail!("{}", msg);
        }

        let mut stream = resp.bytes_stream();
        let mut parser = GeminiStreamParser::new();
        let mut text = String::new();
        let mut tool_calls = Vec::new();

        while let Some(chunk) = stream.next().await {
            let bytes = chunk?;
            let chunk_str = String::from_utf8_lossy(&bytes);
            parser.push(&chunk_str);

            while let Some(obj) = parser.next_object() {
                if let Ok(stream_chunk) = serde_json::from_value::<GeminiStreamChunk>(obj) {
                    if let Some(candidates) = stream_chunk.candidates {
                        for candidate in candidates {
                            if let Some(content) = candidate.content {
                                if let Some(parts) = content.parts {
                                    for part in parts {
                                        match part {
                                            GeminiPart::Text { text: t } if !t.is_empty() => {
                                                text.push_str(&t);
                                                let _ = token_tx.send(t);
                                            }
                                            GeminiPart::FunctionCall { function_call } => {
                                                let tc_id =
                                                    format!("call_{}", uuid::Uuid::new_v4());
                                                tool_calls.push(ToolCall {
                                                    id: tc_id,
                                                    name: function_call.name,
                                                    arguments: function_call.args,
                                                    extra_content: None,
                                                });
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(AssistantTurn { text, tool_calls })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_uppercase_types() {
        let mut schema = json!({
            "type": "object",
            "properties": {
                "name": { "type": "string" },
                "tags": { "type": ["string", "null"] }
            }
        });
        uppercase_types(&mut schema);
        assert_eq!(schema["type"], "OBJECT");
        assert_eq!(schema["properties"]["name"]["type"], "STRING");
        assert_eq!(schema["properties"]["tags"]["type"][0], "STRING");
        assert_eq!(schema["properties"]["tags"]["type"][1], "NULL");
    }

    #[test]
    fn test_translate_chat_history() {
        let history = vec![
            ChatMessage::user("hello"),
            ChatMessage::assistant(
                "thinking",
                vec![ToolCall {
                    id: "call_123".to_string(),
                    name: "test_tool".to_string(),
                    arguments: json!({"arg": 1}),
                    extra_content: None,
                }],
            ),
            ChatMessage::tool_result("call_123", "tool output here"),
            ChatMessage::user("thanks"),
        ];

        let contents = translate_chat_history(&history);

        // Strict alternating roles rule means we should have:
        // 1. user
        // 2. model (assistant text + functionCall)
        // 3. user (combines ToolResult + User turn contiguous)
        assert_eq!(contents.len(), 3);
        assert_eq!(contents[0].role, "user");
        assert_eq!(contents[0].parts.len(), 1);
        match &contents[0].parts[0] {
            GeminiPart::Text { text } => assert_eq!(text, "hello"),
            _ => panic!("Expected text part"),
        }

        assert_eq!(contents[1].role, "model");
        assert_eq!(contents[1].parts.len(), 2);
        match &contents[1].parts[0] {
            GeminiPart::Text { text } => assert_eq!(text, "thinking"),
            _ => panic!("Expected text part"),
        }
        match &contents[1].parts[1] {
            GeminiPart::FunctionCall { function_call } => {
                assert_eq!(function_call.name, "test_tool");
                assert_eq!(function_call.args["arg"], 1);
            }
            _ => panic!("Expected function call part"),
        }

        // The third turn is contiguous "user" role because both tool_result (Role::Tool)
        // and "thanks" (Role::User) map to the "user" role in Gemini. They must be merged.
        assert_eq!(contents[2].role, "user");
        assert_eq!(contents[2].parts.len(), 2);
        match &contents[2].parts[0] {
            GeminiPart::FunctionResponse { function_response } => {
                assert_eq!(function_response.name, "test_tool");
                assert_eq!(function_response.response["output"], "tool output here");
            }
            _ => panic!("Expected function response part"),
        }
        match &contents[2].parts[1] {
            GeminiPart::Text { text } => assert_eq!(text, "thanks"),
            _ => panic!("Expected text part"),
        }
    }

    #[test]
    fn test_stream_parser() {
        let mut parser = GeminiStreamParser::new();

        // Feed partial chunks representing:
        // [
        //   {"candidates": [{"content": {"parts": [{"text": "Hello"}]}}]},
        //   {"candidates": [{"content": {"parts": [{"text": " world"}]}}]}
        // ]

        parser.push("[\n");
        assert!(parser.next_object().is_none());

        parser.push("  {\"candidates\": [{\"content\": {\"parts\": [{\"text\": \"Hello\"}]}}]}");
        let obj1 = parser
            .next_object()
            .expect("Expected to parse first object");
        assert_eq!(
            obj1["candidates"][0]["content"]["parts"][0]["text"],
            "Hello"
        );

        parser.push(
            "\n  ,\n  {\"candidates\": [{\"content\": {\"parts\": [{\"text\": \" world\"}]}}]}",
        );
        let obj2 = parser
            .next_object()
            .expect("Expected to parse second object");
        assert_eq!(
            obj2["candidates"][0]["content"]["parts"][0]["text"],
            " world"
        );

        parser.push("\n]");
        assert!(parser.next_object().is_none());
    }
}
