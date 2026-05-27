# Anvil Sidecar Contract Reference

**Version:** anvil.v1  
**Protocol:** gRPC over local loopback TCP  
**Schema:** `sidecar/proto/anvil/v1/sidecar.proto`

This document covers the sidecar RPC contract used by the `anvil` CLI. It is a reference for contributors extending the CLI or writing alternative clients.

---

## Overview

The `anvil-sidecar` is a workspace-scoped Go daemon that wraps AI provider APIs. The `anvil` CLI (Rust) communicates with it over a versioned gRPC contract. The sidecar is stateless between calls; all state lives in the CLI and audit store.

```
anvil CLI (Rust)
    │
    │ gRPC over loopback TCP
    │ (port discovered from .anvil/run/sidecar.port)
    ▼
anvil-sidecar (Go daemon)
    │
    │ HTTPS
    ▼
AI Provider APIs (Anthropic, OpenAI, Google, etc.)
```

---

## Protocol Version

Every RPC request includes a `client_version` field. The sidecar checks this against its own `server_version`. If the versions are incompatible (major version mismatch), the sidecar returns `FAILED_PRECONDITION` with a descriptive error message.

**Current protocol version:** `anvil.v1`

The version is pinned by:
```
// hinge_test: pins=anvil.v1, intended=test_proto_package_version, phase=P3a
```

---

## Service: `SidecarService`

### `Health`

Health check. The CLI probes this on startup to verify the sidecar is ready.

```protobuf
rpc Health(HealthRequest) returns (HealthResponse);

message HealthRequest {
  string client_version = 1;
}

message HealthResponse {
  string server_version = 1;
  bool ready = 2;
}
```

### `Chat`

Single-turn AI chat. Used by `anvil charter review`, `anvil plan invoke`, `anvil phase build`, `anvil phase review`, and related commands.

```protobuf
rpc Chat(ChatRequest) returns (ChatResponse);

message ChatRequest {
  string client_version = 1;
  string provider_connection_id = 2;  // matches anvil.toml [provider_connections] entry
  string model = 3;
  string system_prompt = 4;
  string user_message = 5;
  repeated Message history = 6;
  ChatOptions options = 7;
}

message ChatResponse {
  string content = 1;
  Usage usage = 2;
}

message Message {
  string role = 1;   // "user" | "assistant"
  string content = 2;
}

message ChatOptions {
  float temperature = 1;
  int32 max_tokens = 2;
}

message Usage {
  int64 input_tokens = 1;
  int64 output_tokens = 2;
  double estimated_cost_usd = 3;
}
```

### `ChatStream`

Streaming variant of Chat. Returns a stream of `ChatStreamChunk` messages followed by a final chunk with `done = true`. Used when `--verbose` streaming is enabled.

```protobuf
rpc ChatStream(ChatRequest) returns (stream ChatStreamChunk);

message ChatStreamChunk {
  string delta = 1;
  bool done = 2;
  Usage usage = 3;  // populated only when done = true
}
```

---

## Error Classes

The sidecar maps provider errors to six canonical classes:

| Class | gRPC code | Description |
|---|---|---|
| `AuthError` | `UNAUTHENTICATED` | Invalid or missing API key |
| `RateLimitError` | `RESOURCE_EXHAUSTED` | Rate limit or quota exceeded |
| `ProviderError` | `UNAVAILABLE` | Provider API error (5xx) |
| `SchemaError` | `INVALID_ARGUMENT` | Malformed request or response schema |
| `NetworkError` | `UNAVAILABLE` | Network connectivity failure |
| `InternalError` | `INTERNAL` | Unexpected sidecar failure |

The mapping is provider-specific and pinned by:
```
// hinge_test: pins=6, intended=test_error_class_count, phase=P3a
```

---

## Sidecar Lifecycle

The sidecar is managed by the CLI, not by the user directly.

**Startup:** the CLI writes `.anvil/run/sidecar.pid` and `.anvil/run/sidecar.port` on launch. The port is a random available loopback port.

**Idle timeout:** after `sidecar.idle_timeout_secs` (default 30 minutes) with no incoming requests, the sidecar auto-exits.

**Shutdown:** `SIGTERM` is honored; the sidecar finishes in-flight requests and exits cleanly. `SIGKILL` is sent after a 5-second grace period if needed.

**Multi-workspace:** each workspace directory spawns an independent sidecar. Daemons do not coordinate. Running `anvil` from multiple directories simultaneously is supported-but-uncoordinated.

---

## Provider Connections

Each provider connection in `anvil.toml` maps a `connection_id` to a provider type and model:

```toml
[[provider_connections]]
id = "claude-coder"
provider = "anthropic"
model = "claude-sonnet-4-6"

[[provider_connections]]
id = "gpt-reviewer"
provider = "openai"
model = "gpt-4o"

[[provider_connections]]
id = "gemini-reviewer"
provider = "google"
model = "gemini-2.5-pro"
```

The sidecar resolves credentials at call time from the OS keychain (key: `anvil/<connection-id>`) or from `ANVIL_API_KEY_<PROVIDER>` environment variables.

---

## Streaming Invariants

- If a streaming call produces an error mid-stream, the sidecar sends a final chunk with `done = true` and no further content. Partial output is **discarded** by the CLI; it is not written to the audit store.
- The CLI never retries a partial stream. The full call is retried from scratch if needed.

Pinned by:
```
// hinge_test: pins=discard-partial, intended=test_partial_output_discarded_on_streaming_error, phase=P3b
// hinge_test: pins=no-continuation, intended=test_streaming_aborts_on_error_no_continuation, phase=P3c
```

---

## Handshake Required Fields

All `ChatRequest` messages must include non-empty:
- `client_version`
- `provider_connection_id`
- `model`
- `system_prompt` or `user_message` (at least one)

Missing required fields return `FAILED_PRECONDITION`.

Pinned by:
```
// hinge_test: pins=required-fields, intended=test_handshake_required_fields, phase=P3a
```

---

## Wire Compatibility

The package is `anvil.v1`. Breaking changes (removing fields, renaming types, changing semantics) require a package version bump to `anvil.v2`. Additive changes (new optional fields) are permitted within `anvil.v1`.

The CLI checks the server's returned `server_version` on every `Health` call. If the major version differs, the CLI surfaces a hard error with an upgrade prompt.
