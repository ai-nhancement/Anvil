# Anvil — P3a Contract Definition R1

**Phase:** P3a — Protobuf Contract Definition  
**Date:** 2026-05-25

---

## Validation

- `cargo test --workspace` — **passes**: 29 tests (up from 26 in P2 R3)
- `cargo clippy --workspace -- -D warnings` — **passes**

---

## Scope

P3a defines the complete wire contract between the Vault (Rust) and the sidecar (Go) as a versioned protobuf schema. It does not implement the gRPC client (P3b) or the Go sidecar server (P3c).

---

## Deliverables

### `proto/anvil/v1/sidecar.proto`

Full `Sidecar` service with 6 RPCs:

| RPC | Kind | Purpose |
|---|---|---|
| `Handshake` | unary | Version negotiation + config-epoch comparison |
| `Invoke` | unary | Single-shot chat or embed invocation |
| `InvokeStreaming` | server-stream | Streaming invocation with Token/FinalResult/Error/Heartbeat events |
| `Cancel` | unary | Cancel in-flight call by idempotency key |
| `Health` | unary | Liveness/readiness probe |
| `ReloadConfig` | unary | Atomic provider-config swap on epoch mismatch |

Key design decisions:

- **Connect-time contract**: Handshake must precede all other RPCs. Documented as a service-level comment.
- **No-commit-on-partial-output invariant**: Only `FinalResult` is authoritative for commit; mid-stream `Error` requires full state discard. Documented as `Plan-Level Trust-Boundary Invariant #1`.
- **Config-epoch**: SHA-256 hash of provider-config content; compared at every Handshake; mismatch triggers `ReloadConfig` or restart.
- **ErrorClass**: Six non-unspecified values (`Transport`, `ProviderRefusal`, `SchemaViolation`, `AdapterBug`, `Timeout`, `Cancelled`) plus `Unspecified` sentinel. Pinned by hinge test.
- **UUIDv7 idempotency key**: `InvokeRequest.idempotency_key` is echoed in all response envelopes for correlation and cancel routing.
- **Oneof payload**: `InvokeRequest` and `InvokeResponse` use oneof to carry either `ChatRequest`/`ChatResponse` or `EmbedRequest`/`EmbedResponse`.

### `crates/anvil-sidecar-client/src/gen/anvil.v1.rs`

Hand-written bootstrap matching prost 0.13.5 + tonic-build 0.12.3 output format:
- All 24 message types with correct prost field annotations
- `ErrorClass` enum with `as_str_name()` and `from_str_name()` impl block
- 5 oneof modules (`invoke_request`, `invoke_response`, `invoke_stream_event`, `final_result`, `credentials`)
- No client or server code (`build_client(false)`, `build_server(false)`)

### `crates/anvil-sidecar-client/build.rs`

Committed-bindings pattern:
- Normal builds copy `src/gen/anvil.v1.rs` → `OUT_DIR/anvil.v1.rs` without invoking protoc.
- `ANVIL_REGEN_PROTO=1` (set by `just gen-rust`) triggers tonic-build regeneration to `src/gen/`.
- Meaningful error message if `src/gen/anvil.v1.rs` is absent.

### `crates/anvil-sidecar-client/src/lib.rs`

Three hinge tests:

| Test | Pins |
|---|---|
| `test_proto_package_version` | `as_str_name` of `ErrorClass::Unspecified` = `"ERROR_CLASS_UNSPECIFIED"` |
| `test_error_class_count` | Exactly 6 non-unspecified error classes |
| `test_handshake_required_fields` | `core_protocol_version` and `supported_versions` fields exist and are non-empty |

### `proto/README.md`

Updated with: RPC table, contract invariants (connect-time, no-commit-on-partial-output, config-epoch), versioning policy, and committed-bindings regeneration instructions. P0 state section removed.

### `justfile`

`gen-rust` recipe updated to set `ANVIL_REGEN_PROTO=1`.

---

## Known Limitations / Deferred to Later Phases

| Item | Phase |
|---|---|
| gRPC client implementation (`SidecarClient`) | P3b |
| Go sidecar server implementation | P3c |
| `FILE_DESCRIPTOR_SET` / reflection support | Not planned for P3 |
| Protoc availability on CI (currently no CI) | Post-P0 |

---

## R1 is ready for approval.
