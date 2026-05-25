# Anvil — P3a Contract Definition R2

**Source review:** `Review Rounds/REVIEW_P3A_CONTRACT_DEFINITION_R1.md`  
**Round:** R2 (all R1 findings addressed)  
**Date:** 2026-05-25

---

## Validation

- `cargo test --workspace` — **passes**: 30 tests (up from 29 in R1)
- `cargo clippy --workspace -- -D warnings` — **passes**

---

## R1 Finding Disposition

### Finding 1 — Medium: Hinge-test coverage too narrow for a contract definition phase

**Fixed.**

New test `test_invoke_request_chat_payload`:
- Constructs a fully-populated `InvokeRequest` with a `credentials::Credential::ApiKey` oneof and an `invoke_request::Payload::Chat` oneof.
- Asserts `model_id` field value.
- Asserts the `payload` discriminant via `matches!`.
- Descends into the `Chat` variant and asserts `messages.len()` and `messages[0].role`.

This pins the oneof module structure (`invoke_request`, `credentials`), the `ChatRequest` shape (`messages` field, `Message.role`), and the discriminant matching pattern in one test.

New test: `test_invoke_request_chat_payload`.

---

### Finding 2 — Low-Medium: Hand-written `src/gen/anvil.v1.rs` is a fragile committed artifact

**Accepted as known P3a limitation.**

The `build.rs` guard (`ANVIL_REGEN_PROTO` + meaningful error on missing file) is the current mitigation. A CI drift-detection step is deferred to when CI exists (post-P0). The committed file will be replaced by real protoc-generated output when P3b is implemented and protoc becomes available in the build environment.

The doc comment `// @generated` signals to editors and CI that the file is generated. The `proto/README.md` note ("Never edit `src/gen/anvil.v1.rs` directly") documents the policy.

---

### Finding 3 — Minor: Test naming and assertion weakness

**Fixed.**

- `test_proto_package_version` renamed to `test_error_class_unspecified_name` — name now accurately describes what the test pins.
- `test_handshake_required_fields` comment updated to label it a "structural smoke test", clarifying that its value is compile-time field-existence verification plus runtime round-trip of the two required fields.

---

## New Tests Added in R2

| Test | Crate | What it pins |
|---|---|---|
| `test_invoke_request_chat_payload` | `anvil-sidecar-client` | `invoke_request::Payload::Chat` oneof discriminant; `ChatRequest.messages` shape; `credentials::Credential::ApiKey` oneof |

---

## Summary

All R1 findings have been fixed or formally accepted with documented rationale. The committed-bindings pattern, four hinge tests, and the complete `anvil.v1` schema are ready for approval.

**R2 is ready for approval.**
