# Anvil Sidecar Contract Reference

**Version:** anvil.v1  
**Protocol:** gRPC over local loopback TCP  
**Schema:** `proto/anvil/v1/sidecar.proto`  
**Last synced:** 2026-05-27 from `proto/anvil/v1/sidecar.proto` (manual sync)

> **Maintenance note:** This document is manually kept in sync with `proto/anvil/v1/sidecar.proto` and the generated Go bindings in `sidecar/internal/contract/`. There is no automated CI check for drift in v1. Before relying on this document for integration work, verify message names, field numbers, and enum values against the `.proto` directly. Automated drift detection is a v1.1 task.

This document covers the sidecar RPC contract used by the Anvil Vault. It is a reference for contributors extending the CLI or writing alternative clients (including the v1.1 App).

---

## Overview

The `anvil-sidecar` is a workspace-scoped Go daemon that wraps AI provider APIs. The Vault (`anvil-core`, Rust) communicates with it over a versioned gRPC contract. The sidecar is stateless between calls; all state lives in the Vault and audit store.

```
Vault / anvil CLI (Rust)
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

**Current package:** `anvil.v1`  
**Go import path:** `github.com/ai-nhancement/Anvil/sidecar/internal/contract`

The package is pinned by:
```
// hinge_test: pins=anvil.v1, intended=test_proto_package_version, phase=P3a
```

Breaking changes (removing fields, renaming types, changing semantics) require a package bump to `anvil.v2`. Additive changes (new optional fields) are permitted within `anvil.v1`.

---

## Connect-Time Contract

**Handshake is required.** `Handshake` must be the first RPC called on every connection. The sidecar rejects all other RPCs until a successful `Handshake` has completed for the current connection.

---

## Service: `Sidecar`

```protobuf
service Sidecar {
  rpc Handshake(HandshakeRequest)     returns (HandshakeResponse);
  rpc Invoke(InvokeRequest)           returns (InvokeResponse);
  rpc InvokeStreaming(InvokeRequest)  returns (stream InvokeStreamEvent);
  rpc Cancel(CancelRequest)           returns (CancelResponse);
  rpc Health(HealthRequest)           returns (HealthResponse);
  rpc ReloadConfig(ReloadConfigRequest) returns (ReloadConfigResponse);
}
```

---

### `Handshake`

Negotiates the protocol version and detects config-epoch drift between the Vault and sidecar. Called before any other RPC on each connection.

```protobuf
message HandshakeRequest {
  string core_protocol_version = 1;    // preferred version, e.g. "v1"
  repeated string supported_versions = 2; // all versions Vault supports, in preference order
  string vault_config_epoch = 3;       // SHA-256 of Vault's active provider-config
}

message HandshakeResponse {
  string negotiated_version = 1;       // first supported_versions entry the sidecar also accepts
  string sidecar_version = 2;          // sidecar binary version string
  string sidecar_build_info = 3;       // commit, timestamp, etc.
  string sidecar_config_epoch = 4;     // SHA-256 of sidecar's loaded provider-config
}
```

On config-epoch mismatch (`vault_config_epoch != sidecar_config_epoch`), the Vault calls `ReloadConfig` or restarts the sidecar before proceeding.

---

### `Invoke`

Single-turn AI request. Waits for the full response before returning.

```protobuf
message InvokeRequest {
  string idempotency_key = 1;          // UUIDv7 correlation ID; echoed in all responses
  string model_id = 2;                 // model identity string, e.g. "claude-opus-4-7"
  string provider_connection_id = 3;   // connection name from anvil.toml
  Credentials credentials = 4;         // per-call secret material
  oneof payload {
    ChatRequest chat = 5;
    EmbedRequest embed = 6;
  }
  optional Timeout timeout = 7;        // per-call timeout override
}

message InvokeResponse {
  string idempotency_key = 1;
  oneof result {
    ChatResponse chat = 2;
    EmbedResponse embed = 3;
    AnvilError error = 4;
  }
}
```

---

### `InvokeStreaming`

Streaming variant of `Invoke`. Returns a stream of `InvokeStreamEvent` messages.

**No-commit-on-partial-output invariant:** `Token` events are ephemeral and for display only. Only the `FinalResult` event is authoritative for the Vault's commit path. On any `StreamError` mid-stream, the Vault discards all accumulated stream state and surfaces only the typed error. There is no best-effort commit on partial output.

Pinned by:
```
// hinge_test: pins=discard-partial, intended=test_partial_output_discarded_on_streaming_error, phase=P3b
// hinge_test: pins=no-continuation, intended=test_streaming_aborts_on_error_no_continuation, phase=P3c
```

```protobuf
message InvokeStreamEvent {
  string idempotency_key = 1;
  oneof event {
    Token token = 2;         // ephemeral display token — NOT authoritative
    FinalResult final_result = 3; // authoritative — only this enters the commit path
    StreamError error = 4;   // terminal error — discard all prior stream state
    Heartbeat heartbeat = 5; // keepalive — no semantic content
  }
}

message Token        { string text = 1; }
message FinalResult  { oneof result { ChatResponse chat = 1; EmbedResponse embed = 2; } }
message StreamError  { AnvilError error = 1; }
message Heartbeat    { int64 timestamp_millis = 1; }
```

---

### `Cancel`

Requests cancellation of an in-flight call identified by `idempotency_key`.

```protobuf
message CancelRequest  { string idempotency_key = 1; }
message CancelResponse { bool cancelled = 1; }
```

---

### `Health`

Checks whether the sidecar daemon is alive and ready. The CLI probes this on startup to verify the sidecar is responding.

```protobuf
message HealthRequest  {}
message HealthResponse { bool healthy = 1; string version = 2; }
```

---

### `ReloadConfig`

Atomically swaps the sidecar's in-memory provider config. Called by the Vault when a config-epoch mismatch is detected at `Handshake` time.

```protobuf
message ReloadConfigRequest {
  string new_config_epoch = 1;     // SHA-256 of new_provider_config; sidecar verifies before applying
  bytes new_provider_config = 2;   // serialized TOML bytes matching the anvil.toml provider section
}

message ReloadConfigResponse {
  bool success = 1;
  AnvilError error = 2;            // set on failure; absent on success
  string active_config_epoch = 3;  // new epoch on success; old epoch on failure
}
```

---

## Payload Messages

### Chat

```protobuf
message ChatRequest {
  string system_prompt = 1;
  repeated Message messages = 2;
  optional int32 max_tokens = 3;
  optional float temperature = 4;
}

message Message {
  string role = 1;    // "user" | "assistant"
  string content = 2;
}

message ChatResponse {
  string content = 1;
  string model = 2;         // model identity echoed by the provider
  Usage usage = 3;
  string finish_reason = 4; // "stop" | "length" | "content_filter" etc.
}

message Usage {
  int32 input_tokens = 1;
  int32 output_tokens = 2;
}
```

### Embed

```protobuf
message EmbedRequest  { string text = 1; }
message EmbedResponse { repeated float embedding = 1; string model = 2; }
```

---

## Credentials and Timeout

```protobuf
message Credentials {
  oneof credential {
    string api_key = 1;
    string bearer_token = 2;
  }
}

message Timeout { uint64 millis = 1; }
```

The sidecar resolves credentials at call time from the OS keychain (key: `anvil/<connection-id>`) or from `ANVIL_API_KEY_<PROVIDER>` environment variables. The Vault passes the resolved credential in `InvokeRequest.credentials`.

---

## Error Model

### `AnvilError`

```protobuf
message AnvilError {
  ErrorClass class = 1;
  string vendor_code = 2;         // provider-specific error code, if available
  string message = 3;
  map<string, string> details = 4;
}
```

### `ErrorClass`

Six non-sentinel values. Adding or removing a class is a breaking contract change, pinned by:
```
// hinge_test: pins=6, intended=test_error_class_count, phase=P3a
```

```protobuf
enum ErrorClass {
  ERROR_CLASS_UNSPECIFIED    = 0;  // sentinel; never returned by a well-behaved sidecar
  ERROR_CLASS_TRANSPORT      = 1;  // network connectivity failure (connection refused, DNS, TLS)
  ERROR_CLASS_PROVIDER_REFUSAL = 2; // provider rejected the request (auth, rate limit, safety)
  ERROR_CLASS_SCHEMA_VIOLATION = 3; // malformed request or response schema
  ERROR_CLASS_ADAPTER_BUG    = 4;  // internal sidecar defect — file a bug
  ERROR_CLASS_TIMEOUT        = 5;  // call exceeded configured timeout
  ERROR_CLASS_CANCELLED      = 6;  // call was cancelled via Cancel RPC or client disconnect
}
```

---

## Sidecar Lifecycle

The sidecar is managed by the Vault, not by the user directly.

**Startup:** the Vault writes `.anvil/run/sidecar.pid` and `.anvil/run/sidecar.port` on launch. The port is a random available loopback port.

**Idle timeout:** after `sidecar.idle_timeout_secs` (default 30 minutes) with no incoming requests, the sidecar auto-exits.

**Shutdown:** `SIGTERM` is honored; the sidecar finishes in-flight requests and exits cleanly. `SIGKILL` is sent after a 5-second grace period if needed.

**Multi-workspace:** each workspace directory spawns an independent sidecar. Daemons do not coordinate. Running `anvil` from multiple directories simultaneously is supported-but-uncoordinated.

---

## Provider Connections

Each provider connection in `anvil.toml` maps a `connection_id` to a provider type:

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

The `connection_id` value is passed as `InvokeRequest.provider_connection_id`.
