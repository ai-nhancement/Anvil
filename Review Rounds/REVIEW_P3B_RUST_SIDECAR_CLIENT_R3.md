# Anvil — P3b Rust Sidecar Client R3

**Source review:** `Review Rounds/REVIEW_P3B_RUST_SIDECAR_CLIENT_R2_Findings.md`  
**Round:** R3 (all R2 minimum-approval items addressed)  
**Date:** 2026-05-25

---

## Validation

- `cargo test --workspace` — **passes**: 38 tests (36 prior + 2 new: `test_supported_versions_is_v1_only`, `test_invoke_stream_has_idempotency_key`)
- `cargo clippy --workspace -- -D warnings` — **passes**

---

## R2 Finding Disposition

### Finding 1 — High: `handshake()` did not validate the negotiated version

**Fixed.**

`handshake()` now validates `resp.negotiated_version` against `SUPPORTED_VERSIONS` before setting
connection state. On mismatch the connection remains `Disconnected` and the method returns
`Err(ClientError::ProtocolMismatch(negotiated_version))`.

```rust
if !supported.contains(&resp.negotiated_version) {
    self.state = ConnectionState::Disconnected;
    return Err(ClientError::ProtocolMismatch(resp.negotiated_version.clone()));
}
```

A new hinge test (`test_supported_versions_is_v1_only`) pins the `SUPPORTED_VERSIONS` constant.

---

### Finding 2 — High: Config-epoch mismatch flow not implemented

**Fixed.**

The `bool handshaked` field is replaced by a `ConnectionState` enum:

| State | Meaning |
|---|---|
| `Disconnected` | No successful handshake yet |
| `ProtocolReady` | Handshake succeeded; config epochs differ — reload required before `invoke()` |
| `Ready` | Handshake succeeded; epochs match — `invoke()` allowed |

`handshake()` sets the state based on epoch comparison:

```rust
self.state = if resp.sidecar_config_epoch == vault_config_epoch {
    ConnectionState::Ready
} else {
    ConnectionState::ProtocolReady
};
```

`invoke()` and `invoke_streaming()` gate on `Ready` only; returning `Err(ClientError::ConfigEpochMismatch)` when in `ProtocolReady`.

`reload_config()` is permitted from `ProtocolReady` and advances the state to `Ready` on success — preserving the reload-as-recovery path.

Two new public methods expose connection state: `is_ready()` and `needs_config_reload()`.

---

### Finding 3 — High: Generated idempotency key not exposed — `Cancel` unusable

**Fixed.**

`InvokeStream` now stores the generated key and exposes it via `idempotency_key() -> &str`:

```rust
pub struct InvokeStream {
    inner: tonic::codec::Streaming<proto::InvokeStreamEvent>,
    idempotency_key: String,
}

impl InvokeStream {
    pub fn idempotency_key(&self) -> &str { &self.idempotency_key }
    // ...
}
```

A caller performing concurrent streaming and cancellation:

```rust
let stream = client.invoke_streaming(req).await?;
let key = stream.idempotency_key().to_owned();
// spawn cancel task with key ...
let result = stream.collect().await?;
```

A new hinge test (`test_invoke_stream_has_idempotency_key`) pins the method at compile time.

For unary `invoke()`, the key is used internally for echo validation; it is not returned since
a unary call returns before the caller could observe it.

---

### Finding 4 — High: Unary `InvokeResponse.error` not converted into typed client error

**Fixed.**

`invoke()` inspects the response `result` oneof and promotes `Error` to `ClientError::Anvil`:

```rust
if let Some(proto::invoke_response::Result::Error(e)) = resp.result {
    return Err(ClientError::Anvil(e));
}
```

The new `ClientError::Anvil(proto::AnvilError)` variant carries the full sidecar error payload
including class, vendor code, message, and details map.

---

### Finding 5 — High: Response idempotency-key echoes not validated

**Fixed.**

`invoke()` validates the echo on the unary response:

```rust
if !resp.idempotency_key.is_empty() && resp.idempotency_key != key {
    return Err(ClientError::ResponseMismatch { sent: key, received: resp.idempotency_key });
}
```

`InvokeStream::collect()` validates the echo on every stream event:

```rust
if !event.idempotency_key.is_empty() && event.idempotency_key != self.idempotency_key {
    return Err(ClientError::ResponseMismatch { sent: ..., received: ... });
}
```

Both checks skip empty keys to accommodate proto3 default-value behavior (sidecar may omit the
field on some event types). Non-empty mismatches are always rejected.

---

### Finding 6 — High/Medium: `collect()` returned on `FinalResult` without verifying stream closure

**Fixed.**

After receiving `FinalResult`, `collect()` reads one more message to verify the stream closes:

```rust
Some(proto::invoke_stream_event::Event::FinalResult(r)) => {
    return match self.inner.message().await? {
        None => Ok(r),
        Some(_) => Err(ClientError::StreamStateMachineViolation),
    };
}
```

`StreamStateMachineViolation` is a new `ClientError` variant for this and similar protocol bugs.

---

### Finding 7 — Medium: Token visibility deferred without explicit plan update

**Accepted and documented.**

The `InvokeStream` struct and `collect()` carry a `// P3c:` comment:

> P3c: add an `events()` or `raw_stream()` API for token-observable streaming for live display.

The `AnvilSidecarClient` struct carries a `// P3c:` comment for retry/backoff (Finding 9):

> P3c: add retry/backoff for `Transport` errors (exponential + jitter, configurable max).

Formal scope clarification: token-observable streaming and retry/backoff are out of P3b scope.
They will be addressed at P3c when the Go sidecar implementation begins and real end-to-end
transport is available for behavioral testing.

---

### Finding 8 — Medium: No client-side request schema validation

**Accepted as known P3b limitation.**

Pre-transport validation of `model_id`, `provider_connection_id`, `credentials`, `payload`, etc.
is deferred. The sidecar validates and returns `ERROR_CLASS_SCHEMA_VIOLATION`; the client
surfaces that via `ClientError::Anvil`. Client-side early validation can be layered above the
wrapper without protocol changes.

---

### Finding 9 — Medium: No retry/backoff

**Accepted and documented.** See Finding 7 disposition (`// P3c:` comment on the struct).

---

### Finding 10 — Medium: `reload_config()` gated behind handshake — state machine underspecified

**Fixed.** See Finding 2 — `ProtocolReady` state explicitly permits `reload_config()`.

---

### Finding 11 — Medium: `ClientError` did not represent the full contract error taxonomy

**Fixed.**

New `ClientError` variants added:

| Variant | Represents |
|---|---|
| `ConfigEpochMismatch` | Client in `ProtocolReady` state |
| `ProtocolMismatch(String)` | Sidecar negotiated an unsupported version |
| `Anvil(proto::AnvilError)` | Unary `AnvilError` payload |
| `ResponseMismatch { sent, received }` | Idempotency key echo mismatch |
| `StreamStateMachineViolation` | Event after `FinalResult` or similar |

Pre-existing variants retained: `HandshakeRequired`, `Transport`, `Stream`, `NoFinalResult`.

---

### Finding 12 — Low/Medium: Stale P3b handoff text in `proto/README.md`

**Fixed.**

The "P3b handoff" paragraph updated to record that `build_client(true)` is done and the
hand-written stub is committed; the note about needing to run `just gen-rust` updated to
describe the regeneration step as optional until `protoc` is available.

---

### Finding 13 — Low: Deferred items not tracked in code

**Fixed.**

`// P3c:` comments added to `AnvilSidecarClient` (retry/backoff) and `InvokeStream`
(token-observable streaming). Finding 8 (request schema validation) is deferred with the
acceptance note above.

---

## Summary

All five High findings are addressed. The two Medium findings accepted as P3b limitations are
now documented in-code with `// P3c:` tags. The state machine (Finding 10) is fixed as part of
Finding 2. All validation is CI-verified.

**R3 is ready for approval.**
