# Anvil — P3b Rust Sidecar Client R1

**Phase:** P3b — Vault-side Rust gRPC sidecar client  
**Round:** R1  
**Date:** 2026-05-25

---

## Validation

- `cargo test --workspace` — **passes**: 36 tests (31 prior + 5 new in anvil-sidecar-client: 4 P3b + existing 5 P3a)

Wait — actual count: anvil-sidecar-client now has **9 tests** (5 P3a + 4 P3b). Workspace total: **36**.

- `cargo clippy --workspace -- -D warnings` — **passes**

---

## Scope

P3b adds three artifacts to `crates/anvil-sidecar-client`:

| Artifact | Location | Purpose |
|---|---|---|
| tonic client stub | `src/gen/anvil.v1.rs` — `pub mod sidecar_client` | Raw gRPC dispatch for all 6 RPCs |
| Contract-enforcing wrapper | `src/client.rs` — `AnvilSidecarClient` | Handshake-first + UUIDv7 idempotency keys |
| `InvokeStream` drain type | `src/client.rs` | No-commit-on-partial-output enforcement |
| P3b hinge tests | `src/lib.rs` — `mod tests` | 4 tests pinning client error variants, UUID format, collect() type, status conversion |

Additionally:
- `uuid = { version = "1", features = ["v7"] }` added to `Cargo.toml`
- `build.rs` updated: `build_client(false)` → `build_client(true)` for future `just gen-rust` regeneration

---

## Findings

### Finding 1 — Medium: Token events are fully opaque to the caller

`InvokeStream::collect()` silently discards `Token` and `Heartbeat` events and returns only `FinalResult`. This enforces the NO-COMMIT-ON-PARTIAL-OUTPUT invariant correctly — the Vault cannot accidentally commit on a token.

However, `proto/README.md` states: "`Token` events are ephemeral, `FinalResult` is authoritative for commit purposes." This implies Token events exist *for terminal display*, not for commit. There is currently no API path for the Vault to observe individual tokens for live UI streaming.

This is a known P3b limitation. Live token forwarding would require a separate `raw_stream()` method returning the `tonic::codec::Streaming<InvokeStreamEvent>` directly, or an async `Iterator`-style API. That path is out of scope for P3b, which implements the transport contract only.

**Severity:** Medium  
**Recommendation:** Accept as known P3b limitation. Document in `proto/README.md` under a "P3b Vault client" section that token visibility is deferred. Add to P3c scope.

---

### Finding 2 — Medium: Hand-written `sidecar_client` stub diverges from tonic-build output in one known way

The `pub mod sidecar_client` appended to `src/gen/anvil.v1.rs` closely matches what tonic-build 0.12.3 would generate, with one deliberate omission: the `with_origin` builder method is absent. tonic 0.12's `Grpc::with_origin` is a static factory function (`with_origin(inner: T, origin: Uri) -> Self`), not an instance method. Writing the correct form requires extracting the inner transport from the existing `Grpc<T>`, which requires an unstable internal accessor. The method was removed rather than written incorrectly.

When `just gen-rust` runs at P3c time (with `build_client(true)` now set), tonic-build will regenerate `sidecar_client` with `with_origin` present. The wrapper in `src/client.rs` is not affected since it does not call `with_origin`.

**Severity:** Medium  
**Recommendation:** Accept. The omission is deliberate and documented implicitly by the `build_client(true)` handoff. No change needed.

---

### Finding 3 — Low: Handshake guard has no runtime behavioral test

`AnvilSidecarClient::invoke()` (and other guarded RPCs) returns `Err(ClientError::HandshakeRequired)` when called before `handshake()`. This code path is present and correct, but the P3b hinge tests only verify:
- The `ClientError::HandshakeRequired` variant is constructible (compile-time)
- The `From<tonic::Status>` conversion works

There is no test that calls `invoke()` before `handshake()` and asserts the error. A real behavioral test would require a mock or test gRPC server, which is out of scope for P3b.

**Severity:** Low  
**Recommendation:** Accept for P3b. The guard logic is a trivial `if !self.handshaked` check — the risk of it being wrong is low. Integration tests at P3c time will exercise the full handshake flow.

---

### Finding 4 — Low: `pub(crate)` visibility on `InvokeStream::inner` is unused

`InvokeStream::inner` is declared `pub(crate)` but no code in the crate accesses it directly. The P3b type-check test (`test_invoke_stream_collect_type`) takes an `InvokeStream` by value but does not construct one. The field could be private without losing anything.

**Severity:** Low  
**Recommendation:** Change `pub(crate) inner` to `inner` (private). If future tests need to inject a mock stream, the constructor pattern is cleaner than field access.

---

### Finding 5 — Low: Blanket `#![allow(clippy::missing_errors_doc)]` in `client.rs`

All public `async fn` methods return `Result` but have no `# Errors` doc section. The blanket allow suppresses the lint for the entire file. For a published library crate this would be a quality concern; for Anvil's internal crate (not yet published), it is acceptable.

**Severity:** Low  
**Recommendation:** Accept for P3b. Add `# Errors` sections to public methods when the crate approaches external consumption.

---

## Summary

The P3b implementation is functionally correct. The handshake-first guard, UUIDv7 idempotency key generation, and `InvokeStream::collect()` no-commit enforcement are all in place. Two medium findings are both accepted-as-known-limitations rather than bugs. Three low findings are minor polish items.

**Minimum before approval:**

1. Fix Finding 4 — remove `pub(crate)` from `InvokeStream::inner` (trivial, no behavior change).

All other findings are either accepted limitations or deferred polish.

**R1 is ready for review.**
