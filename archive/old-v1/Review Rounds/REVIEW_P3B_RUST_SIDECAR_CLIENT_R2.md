# Anvil — P3b Rust Sidecar Client R2

**Source review:** `Review Rounds/REVIEW_P3B_RUST_SIDECAR_CLIENT_R1.md`  
**Round:** R2 (R1 minimum-approval item addressed)  
**Date:** 2026-05-25

---

## Validation

- `cargo test --workspace` — **passes**: 36 tests (unchanged)
- `cargo clippy --workspace -- -D warnings` — **passes**

---

## R1 Finding Disposition

### Finding 4 — Low: `pub(crate)` on `InvokeStream::inner` is unused

**Fixed.**

`InvokeStream::inner` changed from `pub(crate)` to private. No callers outside `InvokeStream::collect()` access the field; the visibility was speculative. The `test_invoke_stream_collect_type` hinge test does not construct an `InvokeStream` directly — it only verifies the return type of `collect()`.

---

### Finding 1 — Medium: Token events are fully opaque to the caller

**Accepted as known P3b limitation.**

`InvokeStream::collect()` correctly enforces NO-COMMIT-ON-PARTIAL-OUTPUT by returning only `FinalResult`. Token forwarding for live UI display is a P3c concern. Noted in review; no code change.

---

### Finding 2 — Medium: Hand-written `sidecar_client` stub missing `with_origin`

**Accepted as known P3b limitation.**

`Grpc::with_origin` in tonic 0.12 is a static factory, not an instance method; the correct hand-written form would need to extract the inner transport (not exposed). The omission is intentional. When `just gen-rust` regenerates at P3c, tonic-build will emit the correct version. The `AnvilSidecarClient` wrapper is unaffected.

---

### Finding 3 — Low: Handshake guard has no runtime behavioral test

**Accepted as known P3b limitation.**

The guard (`if !self.handshaked`) is a trivial conditional. Behavioral test requires a test gRPC server, deferred to P3c integration testing.

---

### Finding 5 — Low: Blanket `#![allow(clippy::missing_errors_doc)]`

**Accepted for P3b.**

`client.rs` is internal to the crate and not yet published. `# Errors` sections can be added when the crate approaches external consumption.

---

## Summary

All five R1 findings are resolved or formally accepted. The one required fix (Finding 4) is in place and CI-verified.

**R2 is ready for approval.**
