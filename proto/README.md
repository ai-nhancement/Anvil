# Anvil Protobuf Schema

This directory contains the versioned protobuf contracts between the Vault (Rust) and the sidecar (Go).

## Current version: `anvil.v1`

- `anvil/v1/sidecar.proto` — full `Sidecar` service contract

---

> **P3a bootstrap note:** The committed Rust (`src/gen/anvil.v1.rs`) and Go (`sidecar/internal/contract/sidecar.pb.go`) binding files are hand-written type-shape stubs for compile-time and hinge testing **only**. They do not include the binary file descriptor (`rawDesc`) required for proto reflection, JSON marshaling, or actual gRPC wire encoding. Real gRPC calls will fail at runtime until the files are regenerated with `protoc`. Regenerate before P3c implementation (`just gen-go` + `just gen-rust`).

---

## Service RPCs

| RPC | Direction | Description |
|---|---|---|
| `Handshake` | unary | First call on every connection; negotiates version, detects config-epoch drift |
| `Invoke` | unary | Single-shot chat or embed invocation |
| `InvokeStreaming` | server-stream | Streaming invocation; `Token` events are ephemeral, `FinalResult` is authoritative |
| `Cancel` | unary | Cancel an in-flight call by idempotency key |
| `Health` | unary | Liveness/readiness probe |
| `ReloadConfig` | unary | Atomically swap provider config on config-epoch mismatch |

## Contract invariants

**Connect-time:** `Handshake` must be the first RPC on every connection. The sidecar rejects all other RPCs until Handshake succeeds.

**No-commit-on-partial-output:** `InvokeStreaming` may emit `Token` events for live terminal display. Only the `FinalResult` event is authoritative for commit purposes. On any `Error` event mid-stream, the Vault must discard all accumulated stream state — there is no best-effort commit.

**Config-epoch:** `vault_config_epoch` (SHA-256 of the Vault's provider config) is compared against `sidecar_config_epoch` at every Handshake. A mismatch triggers `ReloadConfig` or a sidecar restart.

## Version negotiation

**Version string format:** Handshake version strings are `vN` (e.g. `"v1"`, `"v2"`). These are distinct from the protobuf package name `anvil.vN` — do not send the package name as a handshake version string.

**Negotiation algorithm (preference-order):** The sidecar scans the Vault's `supported_versions` list in order and selects the first version it also supports. Implementations may validate that version strings match the `vN` pattern but must not rank by string or numeric comparison — Vault preference order is authoritative.

**No-overlap failure:** If no version in the Vault's `supported_versions` list is supported by the sidecar, the sidecar must return gRPC status `FailedPrecondition` (code 9). The `ErrorClass` for this condition is `ERROR_CLASS_SCHEMA_VIOLATION`. The sidecar must not proceed to process any further RPCs on that connection.

## Streaming state machine

`InvokeStreaming` stream events follow this state machine:

```
OPEN → (Token | Heartbeat)* → FinalResult → CLOSED
OPEN → (Token | Heartbeat)* → Error       → CLOSED
```

Exactly one `FinalResult` or exactly one `Error` terminates the stream. Implementations must not:
- Emit any event after `FinalResult` or `Error`
- Emit multiple `FinalResult` events
- Close the stream without emitting `FinalResult` or `Error`
- Emit `Token` events after `Error`

Behavior when the stream ends without a terminal event is an implementation bug and must be surfaced as `ERROR_CLASS_ADAPTER_BUG`.

## Validation

The following fields are semantically required. Senders must populate them; receivers must return `ERROR_CLASS_SCHEMA_VIOLATION` if they are absent or invalid.

| Field | Rule |
|---|---|
| `InvokeRequest.idempotency_key` | Must be a non-empty UUIDv7 string |
| `InvokeRequest.model_id` | Must be non-empty |
| `InvokeRequest.provider_connection_id` | Must be non-empty |
| `InvokeRequest.credentials` | Must be present with a populated oneof variant |
| `InvokeRequest.payload` | Must be present with a populated oneof variant |
| `HandshakeRequest.core_protocol_version` | Must be non-empty |
| `HandshakeRequest.supported_versions` | Must be non-empty |

**Optional numeric fields:** `max_tokens` and `temperature` absent (nil) means "use the provider/model default." When present: `max_tokens` must be > 0; `temperature` is provider-defined but typically in `[0.0, 2.0]`.

**Timeout:** `Timeout.millis` of 0 is treated as "no timeout override." Oversized timeout values (above the sidecar's configured maximum) must be rejected with `ERROR_CLASS_SCHEMA_VIOLATION` — silent clamping is not permitted.

## Credentials scope (P3a)

`Credentials` supports two variants for P3a:

- `api_key`: bearer API key (Anthropic, OpenAI, Groq, etc.)
- `bearer_token`: OAuth-style bearer token

AWS SigV4 and other structured credential schemes are out of scope for `anvil.v1`. They will require either a new oneof variant in a future revision or a dedicated `Credentials` extension. Adapters must not overload `api_key` or `bearer_token` for structured credential transport.

## Chat message roles

`Message.role` is a free-form string for P3a. Recognized values across adapters:

- `"user"` — turn from the human side
- `"assistant"` — turn from the model
- `"tool"` — tool-call result turn (provider-dependent)

The `ChatRequest.system_prompt` field is the canonical way to pass system context. A `Message` with `role = "system"` is provider-dependent and may be rejected by some adapters. A `MessageRole` enum may be introduced in a future version if cross-provider normalization is needed.

## Schema versioning policy

- The package is `anvil.v1`. Breaking changes require bumping to `anvil.v2` and a new directory.
- Non-breaking additions (new optional fields, new RPC methods) are permitted within a version.
- `ErrorClass` is pinned at 6 non-unspecified values (hinge test `test_error_class_count`). Adding or removing a class is a breaking contract change.

## Generating bindings

```
just gen        # regenerate both Rust and Go
just gen-go     # Go only  (requires protoc + protoc-gen-go + protoc-gen-go-grpc)
just gen-rust   # Rust only (requires protoc)
```

**Rust:** Bindings are committed to `crates/anvil-sidecar-client/src/gen/anvil.v1.rs`. Normal builds copy the committed file to `OUT_DIR`; regeneration requires `protoc` and sets `ANVIL_REGEN_PROTO=1` automatically via `just gen-rust`. Never edit `src/gen/anvil.v1.rs` directly.

**Go:** Bindings are committed to `sidecar/internal/contract/sidecar.pb.go` and `sidecar_grpc.pb.go`. Never edit them directly — regenerate with `just gen-go`. The `ProtoPackageName` constant in `doc.go` is non-generated and survives regeneration.

**P3b — done:** `build.rs` was updated to `build_client(true)` in P3b. A hand-written `pub mod sidecar_client` stub (matching tonic-build 0.12 output style) is committed to `src/gen/anvil.v1.rs`. When `protoc` is available, running `just gen-rust` replaces the hand-written stub with the fully generated version.

**Drift detection:** A `just check-proto-drift` recipe (regenerate into a temp dir and diff) is deferred until `protoc` is available. Until then, the hinge tests in `anvil-sidecar-client` and `sidecar/internal/contract` serve as the primary drift guard.
