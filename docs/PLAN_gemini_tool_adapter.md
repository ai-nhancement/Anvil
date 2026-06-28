# DESIGN PLAN: Native Gemini Tool-Calling Adapter Module

## 1. Overview & Architectural Goals

The goal of this module is to enable **native Google Gemini tool-calling** directly within Anvil without requiring external translation proxies like LiteLLM or OneAPI.

By implementing an in-memory, modular translation adapter, we maintain a zero-dependency local runtime while giving users the full power of Gemini's advanced reasoning and tool-execution capabilities.

### Key Architectural Benefits:
*   **Zero-Dependency Setup:** No external Python runtimes, Node.js servers, or Docker containers are required to translate schemas.
*   **Encapsulation:** All Gemini-specific JSON structures, stream-handling protocols, and role-alternation workarounds remain isolated in a single module (`src/google_compat.rs`), keeping `src/llm.rs` clean and maintainable.
*   **High Performance:** Translation happens directly in compiled Rust in-memory, introducing zero networking or serialization overhead.

---

## 2. Integration Interface in `src/llm.rs`

The module will integrate cleanly into Anvil's router function `chat_turn_stream` inside `src/llm.rs`. Instead of using a simple text-only fallback, the router will delegate to the new `google_compat` module:

```rust
// In src/llm.rs
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
        // Delegate to the specialized, native Google tool-calling stream adapter
        self.google_turn_stream(conn, model, api_key, system, history, tools, token_tx)
            .await
    }
    _ => bail!("Unknown provider connection type"),
}
```

---

## 3. The Gemini Tool-Calling API Protocol

Google Gemini (via the Vertex AI or Google AI Studio endpoints) uses a distinct JSON format compared to OpenAI and Anthropic. To build a robust adapter, our module must adhere strictly to these schemas.

### A. Endpoints and Authentication
*   **URL Structure:** `/v1beta/models/{model}:streamGenerateContent?key={api_key}`
*   **Default Base URL:** `https://generativelanguage.googleapis.com`

---

### B. Request Schema Layout
The JSON payload sent to Gemini must follow this structure:

```json
{
  "systemInstruction": {
    "parts": [{ "text": "System prompt instructions go here..." }]
  },
  "contents": [
    {
      "role": "user",
      "parts": [{ "text": "First user message" }]
    },
    {
      "role": "model",
      "parts": [{ "text": "First assistant response" }]
    },
    {
      "role": "user",
      "parts": [
        {
          "functionResponse": {
            "name": "read_file",
            "response": {
              "content": "File contents here..."
            }
          }
        }
      ]
    }
  ],
  "tools": [
    {
      "functionDeclarations": [
        {
          "name": "read_file",
          "description": "Reads a file from the workspace",
          "parameters": {
            "type": "OBJECT",
            "properties": {
              "path": {
                "type": "STRING",
                "description": "The path to read"
              }
            },
            "required": ["path"]
          }
        }
      ]
    }
  ],
  "toolConfig": {
    "functionCallingConfig": {
      "mode": "AUTO"
    }
  }
}
```

---

### C. Streaming Response Chunk Schema
When streaming from `streamGenerateContent`, Gemini sends a JSON array or Server-Sent Events (SSE) stream where each chunk contains a candidate. If the model invokes a tool, the chunk carries `functionCalls`:

```json
{
  "candidates": [
    {
      "content": {
        "role": "model",
        "parts": [
          {
            "functionCall": {
              "name": "read_file",
              "args": {
                "path": "src/main.rs"
              }
            }
          }
        ]
      },
      "finishReason": "STOP"
    }
  ]
}
```

---

## 4. Bi-directional Translation Logic

### A. Tool Definitions (`Anvil` -> `Gemini`)
Anvil's `ToolDef` has an `input_schema` representing a standard JSON Schema. Gemini's `functionDeclarations` accept an OpenAPI 3.0 schema object, which is structurally identical but case-sensitive on types.
*   **Conversion Rule:** Convert JSON Schema lowercase type strings (e.g., `"string"`, `"object"`, `"array"`, `"integer"`) into uppercase counterparts (e.g., `"STRING"`, `"OBJECT"`, `"ARRAY"`, `"INTEGER"`) to comply with Gemini's API.

---

### B. Message History Translation & Strict Alternation (Crucial)
Gemini's API enforces two extremely strict validation rules:
1.  **Strict Alternating Roles:** Roles in the `contents` array **must** strictly alternate between `user` and `model`. You can never have two consecutive `user` turns or two consecutive `model` turns.
2.  **Valid Role Values:** The only valid roles are `"user"` and `"model"`. Tool execution results must be submitted with the role `"user"`.

#### The Mapping Blueprint:
*   **Anvil `Role::User`:** Maps directly to Gemini `role: "user"`.
*   **Anvil `Role::Assistant`:** Maps to Gemini `role: "model"`. If the turn includes `tool_calls`, translate them into Gemini `functionCall` part blocks.
*   **Anvil `Role::Tool`:** Since tool results must have the `role: "user"` in Gemini, they cannot sit consecutively next to another user turn. 
    *   **The Translation Pattern:** Group all contiguous `Tool` responses following a `model` turn into a single `"user"` turn containing multiple `functionResponse` part blocks.

#### Alternation Flow Example:
```
[Anvil History]
1. User: "Help me find a bug"
2. Assistant (calls grep, read_file): tool_call_1, tool_call_2
3. Tool Result 1 (grep): "matches found..."
4. Tool Result 2 (read_file): "file content..."

     ▼ Translated into Gemini Contents Array:

1. User (role: "user")
   parts: [{ "text": "Help me find a bug" }]

2. Model (role: "model")
   parts: [
     { "functionCall": { "name": "grep", "args": { ... } } },
     { "functionCall": { "name": "read_file", "args": { ... } } }
   ]

3. User (role: "user") — COMBINED CONTIGUOUS TOOL RESULTS
   parts: [
     { "functionResponse": { "name": "grep", "response": { "output": "matches..." } } },
     { "functionResponse": { "name": "read_file", "response": { "output": "file content..." } } }
   ]
```

---

## 5. Streaming & Accumulation Logic

Since tool calls can be streamed in chunks, our stream parser must safely buffer and parse the SSE (Server-Sent Events) chunks or JSON chunk blocks.

### Step-by-Step Chunk Processing:
1.  Read bytes from the connection stream chunk-by-chunk.
2.  Parse the stream as a standard JSON array stream or raw text. (Gemini AI Studio streams can emit raw JSON chunks containing candidate arrays).
3.  Deserialize each chunk into a helper struct `GeminiStreamChunk`.
4.  For text parts, write them directly to the `token_tx` channel so the user sees live streaming in the CLI.
5.  For `functionCall` parts, accumulate them into a thread-safe list of `ToolCall` objects.
6.  Once the stream finishes, package the accumulated text and translated `ToolCall` elements into a final `AssistantTurn`.

---

## 6. Implementation Rust Structs

These are the precise serializable structs we will define in our new module:

```rust
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiRequest {
    pub system_instruction: Option<GeminiSystemInstruction>,
    pub contents: Vec<GeminiContent>,
    pub tools: Vec<GeminiTool>,
    pub tool_config: Option<GeminiToolConfig>,
}

#[derive(Serialize)]
pub struct GeminiSystemInstruction {
    pub parts: Vec<GeminiPart>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GeminiContent {
    pub role: String, // "user" or "model"
    pub parts: Vec<GeminiPart>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum GeminiPart {
    Text {
        text: String,
    },
    FunctionCall {
        function_call: GeminiFunctionCall,
    },
    FunctionResponse {
        #[serde(rename = "functionResponse")]
        function_response: GeminiFunctionResponse,
    },
}

#[derive(Serialize, Deserialize, Clone)]
pub struct GeminiFunctionCall {
    pub name: String,
    pub args: Value,
}

#[derive(Serialize, Deserialize, Clone)]
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
```

---

## 7. Step-by-Step Engineering Roadmap

*   [ ] **Step 1: Module Creation**
    Create `src/google_compat.rs` and paste the serializable struct definitions. Link the module in `src/main.rs` (or `src/lib.rs` / `src/agent.rs` as appropriate).

*   [ ] **Step 2: Bi-directional Map functions**
    Write the `Anvil` <-> `Gemini` schema translations inside `src/google_compat.rs`:
    *   `fn translate_tool_definition(def: &ToolDef) -> GeminiFunctionDeclaration`
    *   `fn translate_chat_history(history: &[ChatMessage]) -> Vec<GeminiContent>`

*   [ ] **Step 3: Implement streaming fetch & translation router**
    Implement the async `google_turn_stream` function inside `src/google_compat.rs` using `reqwest` to call the Gemini API endpoint and process the stream chunks.

*   [ ] **Step 4: Integrate into router layer**
    In `src/llm.rs`, update `chat_turn_stream` to delegate all `google` / `gemini` calls to the newly built module.

*   [ ] **Step 5: Verify and Bench**
    Ensure no compilation warnings or clippy errors are raised (`cargo clippy`). Benchmark a Gemini model over tool-calling to verify complete compatibility.
