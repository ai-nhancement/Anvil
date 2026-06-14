# Anvil — P3a Contract Definition R4

**Source review:** `Review Rounds/REVIEW_P3A_CONTRACT_DEFINITION_R3_Findings.md`  
**Round:** R4 (all R3 minimum-approval items addressed)  
**Date:** 2026-05-25

---

## Validation

- `cargo test --workspace` — **passes**: 31 tests (unchanged)
- `cargo clippy --workspace -- -D warnings` — **passes**
- `go test ./...` from `sidecar/` — **passes**
- `go build ./...` from `sidecar/` — **passes**

---

## R3 Finding Disposition

### Finding 1 — High: Go bootstrap not runtime-usable — caveat not prominent enough

**Fixed.**

A bold "P3A SHAPE-ONLY BOOTSTRAP" block comment added at the top of `sidecar/internal/contract/sidecar.pb.go`:

```go
// P3A SHAPE-ONLY BOOTSTRAP — NOT RUNTIME-USABLE PROTOBUF CODE.
// This file defines message structs for type-level testing only. It does NOT include
// the rawDesc binary (serialized FileDescriptorProto), so proto reflection, JSON
// marshaling, and actual gRPC wire encoding will panic at runtime. Regenerate with
// `just gen-go` (requires protoc) before P3c implementation.
```

The same caveat is now the first visible block in `proto/README.md`, rendered as a callout note.

---

### Finding 2 — High: Package-version hinges are constants, not derived from the proto

**Fixed.**

Both Rust and Go hinge tests now read the actual proto file and assert the `package anvil.v1;` declaration, rather than testing a hand-written constant.

**Rust** (`test_proto_package_version`):
```rust
let proto_src = std::fs::read_to_string(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../proto/anvil/v1/sidecar.proto"
)).expect("...");
assert!(proto_src.contains("package anvil.v1;"), "...");
```

**Go** (`TestProtoFilePackageName`):
```go
content, err := os.ReadFile(filepath.Join("..", "..", "..", "proto", "anvil", "v1", "sidecar.proto"))
// ...
if !bytes.Contains(content, []byte("package anvil.v1;")) { ... }
```

The `PROTO_PACKAGE` const has been removed from the generated Rust file (`anvil.v1.rs`). The Go `ProtoPackageName` const has been moved from `sidecar.pb.go` (generated) to `doc.go` (non-generated, survives `just gen-go`).

---

### Finding 3 — High: Go service test passes even if both interface and stub drop an RPC together

**Fixed.**

`TestSidecarServiceInterface` now independently asserts the `Sidecar_ServiceDesc` descriptor, which is maintained separately from the interface:

- `ServiceName == "anvil.v1.Sidecar"`
- Unary method set is exactly `{Handshake, Invoke, Cancel, Health, ReloadConfig}` (detects both unexpected methods and missing ones)
- Exactly one stream entry, named `InvokeStreaming`, with `ServerStreams == true`

The compile-time `var _ contract.SidecarServer = &contract.UnimplementedSidecarServer{}` check is retained as a complementary guard.

---

### Finding 4 — Medium: Drift detection remains partial

**Accepted as known P3a limitation.**

The Go hinge tests (7 tests in `sidecar_contract_test.go`) serve as the P3a drift guard. Field-level drift (protobuf tags, wire numbers) is not caught without protoc-level regeneration. A `just check-proto-drift` recipe is documented in `proto/README.md` as deferred until protoc is available. This is the same scope accepted for the Rust committed-bindings pattern.

---

### Finding 5 — Medium: Version negotiation documentation internally inconsistent

**Fixed.**

The "Version negotiation" section in `proto/README.md` now states a single, unambiguous algorithm:

> **Negotiation algorithm (preference-order):** The sidecar scans the Vault's `supported_versions` list in order and selects the first version it also supports. Implementations may validate that version strings match the `vN` pattern but must not rank by string or numeric comparison — Vault preference order is authoritative.

The conflicting "parse and compare the integer N" sentence has been removed.

---

### Finding 6 — Medium: No-overlap handshake error undefined

**Fixed.**

The "Version negotiation" section now defines no-overlap failure precisely:

> **No-overlap failure:** If no version in the Vault's `supported_versions` list is supported by the sidecar, the sidecar must return gRPC status `FailedPrecondition` (code 9). The `ErrorClass` for this condition is `ERROR_CLASS_SCHEMA_VIOLATION`.

---

### Finding 7 — Medium: `PROTO_PACKAGE` constant will be overwritten by protoc

**Fixed.**

`PROTO_PACKAGE` removed from `anvil.v1.rs` (generated). `ProtoPackageName` moved from `sidecar.pb.go` (generated) to `doc.go` (non-generated, will survive `just gen-go`). Package-version tests now read the proto file directly rather than testing a constant.

---

### Finding 8 — Medium: Go tests depend on bootstrap APIs that may not match protoc output

**Accepted as known P3a limitation.**

The oneof wrapper names (`InvokeRequest_Chat`, `Credentials_ApiKey`, etc.) match standard protoc-gen-go naming conventions and should survive regeneration. `ProtoPackageName` is moved to a non-generated file. The `TestSidecarServiceInterface` descriptor assertions will continue to work after regeneration since `Sidecar_ServiceDesc` is stable output from protoc-gen-go-grpc. Tests focused on type-shape will be reviewed at P3c regeneration time.

---

### Finding 9 — Low/Medium: Error class count tests do not verify discriminant values

**Fixed.**

Both Rust and Go `test_error_class_count` / `TestErrorClassCount` now assert the discriminant integer values for all 7 classes:

| Class | Expected value |
|---|---|
| `Unspecified` | 0 |
| `Transport` | 1 |
| `ProviderRefusal` | 2 |
| `SchemaViolation` | 3 |
| `AdapterBug` | 4 |
| `Timeout` | 5 |
| `Cancelled` | 6 |

---

### Finding 10 — Low/Medium: Silent timeout clamping conflicts with auditable-workflow ethos

**Fixed.**

`proto/README.md` now states:

> **Timeout:** Oversized timeout values (above the sidecar's configured maximum) must be rejected with `ERROR_CLASS_SCHEMA_VIOLATION` — silent clamping is not permitted.

---

### Finding 11 — Low: README conflates `vN` (handshake version) with `anvil.vN` (package name)

**Fixed.**

The "Version negotiation" section now opens with:

> **Version string format:** Handshake version strings are `vN` (e.g. `"v1"`, `"v2"`). These are distinct from the protobuf package name `anvil.vN` — do not send the package name as a handshake version string.

---

## Summary

All five minimum-before-approval items from R3 are addressed. Additionally, Findings 7, 9, 10, and 11 are fixed; Findings 4 and 8 are formally accepted as known P3a limitations consistent with the committed-bindings approach.

**R4 is ready for approval.**
