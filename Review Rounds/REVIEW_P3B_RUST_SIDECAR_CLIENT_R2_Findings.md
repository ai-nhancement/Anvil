# Anvil — P3b Rust Sidecar Client R2 Findings

**Source review doc:** `Review Rounds/REVIEW_P3B_RUST_SIDECAR_CLIENT_R2.md`  
**Review date:** 2026-05-25  
**Reviewer instruction:** Critical review only. Do not write code. Findings are ordered most critical first.

## Validation Performed

- `cargo test --workspace` — **passes**
- `cargo clippy --workspace -- -D warnings` — **passes**
- `cargo test -p anvil-sidecar-client -- --nocapture` — **passes**

The R2 fix itself is accurate: `InvokeStream::inner` is now private. However, reviewing the full P3b implementation against the Plan and contract surfaces several unresolved issues that are more significant than the low visibility fix.

---

## 1. High — `handshake()` does not validate the negotiated protocol version

**Location:**

- `crates/anvil-sidecar-client/src/client.rs`
- `proto/README.md`
- `Anvil Plan/ANVIL_PLAN.md`

**Problem:**

`AnvilSidecarClient::handshake()` sends:

```rust
core_protocol_version: "v1"
supported_versions: vec!["v1"]
```

Then it accepts any successful `HandshakeResponse` and marks the client as handshaked:

```rust
let resp = self.inner.handshake(req).await?.into_inner();
self.handshaked = true;
Ok(resp)
```

It does not verify:

- `resp.negotiated_version == "v1"`
- `resp.negotiated_version` is in the supported set
- the response is semantically valid
- no-overlap behavior is correctly surfaced

**Impact:**

A buggy or incompatible sidecar can return a successful handshake with an unsupported negotiated version, and the client will proceed. This violates the P3b requirement that the client refuse to operate if there is no protocol-version overlap.

**Suggested fix:**

- Validate `negotiated_version` before setting `handshaked = true`.
- Return a typed client error if the negotiated version is unsupported.
- Add a hinge/unit test for “successful transport response but unsupported negotiated version is rejected.”
- Keep `handshaked = false` on all failed/invalid handshake outcomes.

---

## 2. High — Config-epoch mismatch flow is not implemented in the client wrapper

**Location:**

- `crates/anvil-sidecar-client/src/client.rs`
- `proto/README.md`
- `Anvil Plan/ANVIL_PLAN.md`

**Problem:**

The contract says the client must compare:

- `vault_config_epoch`
- `sidecar_config_epoch`

On mismatch, the Vault should call `ReloadConfig` or restart the sidecar.

Current `handshake()` simply returns the response and leaves all interpretation to callers. There is no client-level helper or enforcement for config-epoch validation.

**Impact:**

- The P3b client can mark itself handshaked even when the sidecar is stale.
- Subsequent `invoke()` calls may route through stale provider config.
- This weakens the split-brain state-drift fix introduced in the Plan.

**Suggested fix:**

- Either implement config-epoch validation in `handshake()` or provide an explicit higher-level `handshake_and_validate_config(...)` API.
- Do not let callers accidentally proceed after a mismatch without making an intentional reload/restart decision.
- Add a test or documented invariant that config-epoch mismatch is not silently accepted.

---

## 3. High — The client generates idempotency keys but does not expose them, making `Cancel` hard to use correctly

**Location:**

- `crates/anvil-sidecar-client/src/client.rs`

**Problem:**

`invoke()` and `invoke_streaming()` overwrite the caller’s `idempotency_key`:

```rust
request.idempotency_key = new_idempotency_key();
```

But the generated key is not returned to the caller before the RPC begins.

`cancel()` requires:

```rust
CancelRequest { idempotency_key }
```

For streaming or long-running requests, the caller needs the generated key to cancel the in-flight operation. The current API hides it.

**Impact:**

- Callers cannot reliably cancel requests initiated through this wrapper.
- The client owns the correlation key but does not provide a way to observe it.
- This undermines the `Cancel` RPC contract.

**Suggested fix:**

- Return a request handle containing the generated idempotency key.
- Or require the caller to provide the idempotency key and validate it as UUIDv7 instead of overwriting it.
- For streaming, expose the generated key alongside `InvokeStream`.

---

## 4. High — Unary `InvokeResponse.error` is not converted into a typed client error

**Location:**

- `crates/anvil-sidecar-client/src/client.rs`
- `proto/anvil/v1/sidecar.proto`

**Problem:**

The proto defines:

```proto
message InvokeResponse {
  string idempotency_key = 1;
  oneof result {
    ChatResponse chat = 2;
    EmbedResponse embed = 3;
    AnvilError error = 4;
  }
}
```

But `invoke()` returns raw `proto::InvokeResponse`:

```rust
Ok(self.inner.invoke(request).await?.into_inner())
```

It does not inspect whether the response contains `result = Error`.

**Impact:**

- A caller can accidentally treat an `InvokeResponse` containing `AnvilError` as a successful transport result.
- This weakens the “typed error” boundary.
- The wrapper is described as contract-enforcing, but it does not enforce the unary error envelope.

**Suggested fix:**

- Convert `InvokeResponse.result = Error` into a `ClientError::Anvil(proto::AnvilError)` or similar.
- Or provide typed helpers that return `Result<ChatResponse, ClientError>` / `Result<EmbedResponse, ClientError>`.
- Add tests for unary provider/schema/adapter errors.

---

## 5. High — Response idempotency-key echo is not validated

**Location:**

- `crates/anvil-sidecar-client/src/client.rs`

**Problem:**

The contract says the idempotency key is echoed back in response envelopes.

The client generates a key and sends it, but does not verify:

- `InvokeResponse.idempotency_key == generated_key`
- each `InvokeStreamEvent.idempotency_key == generated_key`
- `CancelResponse` correlation if applicable

**Impact:**

- Cross-request response mixups could pass unnoticed.
- A buggy sidecar can return a response for a different invocation.
- Audit and cancellation correlation can become unreliable.

**Suggested fix:**

- Store the generated idempotency key for the RPC.
- Validate every unary/stream response envelope against it.
- Treat mismatch as `SCHEMA_VIOLATION` or `ADAPTER_BUG`, depending on intended taxonomy.

---

## 6. High / Medium — `InvokeStream::collect()` returns on the first `FinalResult` without verifying terminal stream closure

**Location:**

- `crates/anvil-sidecar-client/src/client.rs`

**Problem:**

The streaming contract says exactly one `FinalResult` or exactly one `Error` terminates the stream.

Current `collect()` returns immediately on `FinalResult`:

```rust
Some(proto::invoke_stream_event::Event::FinalResult(r)) => {
    return Ok(r);
}
```

It does not verify that the stream actually closes after the final result. If a buggy sidecar emits:

```text
Token → FinalResult → Error
```

the client returns `Ok(FinalResult)` and drops the stream before observing the later error.

**Impact:**

- The client can commit a result from a malformed stream.
- This conflicts with the broader invariant: no commit on invalid sidecar output.
- It trusts the sidecar’s state machine instead of enforcing it.

**Suggested fix:**

- After receiving `FinalResult`, continue reading one more event and require stream closure.
- If any event appears after `FinalResult`, return an adapter/schema error instead of success.
- Also detect multiple final results and final-result-then-error.

---

## 7. Medium — Token visibility was deferred to P3c, but the Plan assigns streaming client behavior to P3b

**Location:**

- `Review Rounds/REVIEW_P3B_RUST_SIDECAR_CLIENT_R2.md`
- `crates/anvil-sidecar-client/src/client.rs`
- `Anvil Plan/ANVIL_PLAN.md`

**Problem:**

R2 says token forwarding is a P3c concern. But the Plan’s P3b section describes a Vault-side client with:

```text
invoke_streaming(request) -> impl Stream<Item = InvokeStreamEvent>
```

Current P3b provides only `InvokeStream::collect()`, which discards tokens and heartbeats.

**Impact:**

- The client cannot support live terminal display.
- Later phases that need streaming UX have no P3b API path.
- The disposition may contradict the phase’s documented scope.

**Suggested fix:**

- Either update the Plan/disposition to explicitly defer token-observable streaming out of P3b, or add a safe streaming API now.
- A good compromise is two APIs:
  - `collect()` for commit-safe final-result-only behavior
  - `events()` or `raw_stream()` for display-only streaming with clear documentation that tokens must not feed commit paths

---

## 8. Medium — No request schema validation before sending RPCs

**Location:**

- `crates/anvil-sidecar-client/src/client.rs`
- `proto/README.md`

**Problem:**

`proto/README.md` lists required fields:

- `model_id`
- `provider_connection_id`
- `credentials`
- `payload`
- UUIDv7 idempotency key
- etc.

The client validates none of these before dispatch, except it overwrites `idempotency_key`.

**Impact:**

- Invalid requests go over the wire and depend on sidecar validation.
- The Plan says the client has a contract-enforcement layer and schema-validation behavior.
- Bugs are detected later and less locally.

**Suggested fix:**

- Add client-side validation for semantically required fields.
- Return `ClientError::SchemaViolation` or equivalent before transport.
- Add unit tests for missing payload, missing credentials, empty model ID, and empty provider connection.

---

## 9. Medium — No retry/backoff implementation despite P3b Plan requirement

**Location:**

- `crates/anvil-sidecar-client/src/client.rs`
- `Anvil Plan/ANVIL_PLAN.md`

**Problem:**

The Plan’s P3b action list includes:

```text
Retry/backoff for Transport failures (exponential + jitter, configurable max).
```

Current code converts `tonic::Status` into `ClientError::Transport`, but does not retry any transport failures.

**Impact:**

- P3b does not satisfy the planned transport resilience requirement.
- Transient failures surface immediately.
- Later phases may assume retry semantics exist when they do not.

**Suggested fix:**

- Either implement retry/backoff now or formally document deferral.
- If deferred, update the review doc and Plan alignment notes.
- Add a future hinge test for retry behavior using a mock/test server.

---

## 10. Medium — `reload_config()` is gated behind `handshaked`, but reload is part of handshake mismatch recovery

**Location:**

- `crates/anvil-sidecar-client/src/client.rs`
- `proto/README.md`

**Problem:**

`reload_config()` returns `HandshakeRequired` unless `handshaked` is true.

This works only if `handshake()` marks the client as handshaked even when the config epochs mismatch. The current implementation does exactly that. But once handshake validation is tightened, care is needed: a config-epoch mismatch should not make the connection unusable for `ReloadConfig`.

**Impact:**

- The handshake/reload state machine is underspecified.
- A future fix to “do not mark handshaked on invalid handshake” could accidentally block `ReloadConfig`.
- The client needs a distinct state for “protocol handshake succeeded, config reload required.”

**Suggested fix:**

- Replace boolean `handshaked` with a small state machine:
  - `Disconnected` / `ConnectedNoHandshake`
  - `ProtocolReady`
  - `ConfigMismatch`
  - `Ready`
- Permit `ReloadConfig` after successful protocol negotiation even if config epoch mismatches.
- Only permit `Invoke` once config is ready.

---

## 11. Medium — `ClientError` does not represent the contract’s error taxonomy cleanly

**Location:**

- `crates/anvil-sidecar-client/src/client.rs`

**Problem:**

Current variants:

```rust
HandshakeRequired
Transport(tonic::Status)
Stream(Option<proto::AnvilError>)
NoFinalResult
```

Missing or ambiguous cases include:

- unary `AnvilError`
- schema violation before dispatch
- protocol version mismatch
- config epoch mismatch
- response idempotency mismatch
- stream event after terminal event
- no-overlap handshake failure

**Impact:**

- Contract-level failures are forced into transport or generic stream buckets.
- Callers cannot reliably distinguish provider refusal, schema violation, adapter bug, timeout, etc.
- This weakens the typed error boundary that P3a established.

**Suggested fix:**

- Add a canonical `ClientError::Anvil(proto::AnvilError)` or equivalent.
- Add targeted variants for client-side protocol failures.
- Ensure conversion from `tonic::Status` preserves enough detail to map transport vs schema failures.

---

## 12. Low / Medium — `proto/README.md` still has stale P3b handoff text

**Location:**

- `proto/README.md`
- `crates/anvil-sidecar-client/build.rs`

**Problem:**

`build.rs` currently has:

```rust
.build_client(true)
```

But `proto/README.md` still says:

```text
P3b must change this to build_client(true)
```

That handoff is now stale.

**Impact:**

- Minor documentation drift.
- Future maintainers may think P3b has not yet enabled client generation.

**Suggested fix:**

- Update README to state that P3b enabled `build_client(true)`.
- Keep a note that actual regeneration still requires `protoc`.

---

## 13. Low — R2 claims “all five R1 findings are resolved or formally accepted,” but accepted limitations are not tracked in code/docs

**Location:**

- `Review Rounds/REVIEW_P3B_RUST_SIDECAR_CLIENT_R2.md`
- `crates/anvil-sidecar-client/src/client.rs`
- `proto/README.md`

**Problem:**

R2 accepts several limitations:

- no token-observable streaming API
- missing `with_origin`
- no handshake guard behavioral test
- missing `# Errors` docs

But not all are documented in code or user-facing docs.

**Impact:**

- Accepted limitations may be forgotten.
- P3c implementers may not know what must be revisited.
- “Accepted” can become accidental permanence.

**Suggested fix:**

- Add comments or TODOs with phase references where deferred work is expected.
- Track token-observable streaming and behavioral handshake tests in the next phase’s review checklist.

---

## Overall Assessment

The R2 fix itself is valid: `InvokeStream::inner` was made private, and validation passes.

However, I would **not approve P3b as complete** based only on R2. The implementation is a useful skeleton, but it falls short of the Plan’s P3b client responsibilities in several important areas:

- handshake response validation
- config-epoch mismatch handling
- idempotency-key echo validation and cancellation usability
- typed unary error handling
- response schema validation
- retry/backoff
- stream state-machine enforcement

Minimum recommended before approval:

1. Validate `HandshakeResponse.negotiated_version`.
2. Define and implement the client-side config-epoch state flow.
3. Expose or preserve generated idempotency keys so `Cancel` can work.
4. Convert unary `InvokeResponse.error` into typed client errors.
5. Validate response idempotency-key echoes.
6. Either implement or explicitly defer retry/backoff and token-observable streaming with Plan updates.

As-is, R2 resolves the one low-severity R1 code-cleanup item, but it does not address broader P3b completeness concerns.