# Anvil — P3a Contract Definition R3 Findings

**Source review doc:** `Review Rounds/REVIEW_P3A_CONTRACT_DEFINITION_R3.md`  
**Review date:** 2026-05-25  
**Reviewer instruction:** Critical review only. Do not write code. Findings are ordered most critical first.

## Validation Performed

- `cargo test --workspace` — **passes**: 31 tests
- `cargo clippy --workspace -- -D warnings` — **passes**
- `go test ./...` from `C:\Anvil\sidecar` — **passes**
- `go build ./...` from `C:\Anvil\sidecar` — **passes**

R3 substantially improves the situation from R2: the Go side now has full type-level coverage for the P3a schema and no longer exposes the stale P0-only `Ping` surface. However, there are still important risks and inconsistencies.

---

## 1. High — Go “generated” bindings are still not runtime-usable protobuf bindings

**Location:**

- `sidecar/internal/contract/sidecar.pb.go`
- `sidecar/internal/contract/sidecar_grpc.pb.go`
- `Review Rounds/REVIEW_P3A_CONTRACT_DEFINITION_R3.md`

**Problem:**

R3 explicitly acknowledges this caveat:

> Wire compatibility caveat: The bootstrap files do not include `rawDesc` ... proto reflection, JSON marshaling, and actual gRPC wire encoding will not work at runtime until the files are regenerated with protoc.

That caveat is accurate. The handwritten Go message types implement `ProtoMessage()` but do **not** implement the modern `google.golang.org/protobuf/proto.Message` interface because they lack `ProtoReflect()`.

This means the current Go contract package is a compile-time shape stub, not actual usable protobuf generated code.

**Impact:**

- `go build` and shape tests pass, but real gRPC serialization will fail later.
- `sidecar_grpc.pb.go` looks like usable generated gRPC code, which could mislead P3c implementation work.
- The R3 statement that Rust + Go agree “at the type level” is mostly true, but “bindings work” is still not true in the protobuf/gRPC sense.
- Any P3c implementer who tries to register the service and call it over gRPC before regeneration will hit runtime codec failures.

**Suggested fix:**

- Make the caveat more prominent in `proto/README.md`, not only the review doc.
- Add an explicit compile-time or runtime guard/comment near the top of `sidecar.pb.go` saying: “P3a shape-only bootstrap; not protobuf-runtime compatible.”
- Consider renaming or isolating the handwritten bootstrap to avoid presenting it as actual generated `pb.go`.
- Prefer regenerating with `protoc` before approving P3a if P3a’s acceptance criterion is interpreted as “generated Rust and Go bindings work.”

---

## 2. High — Package-version hinges are constants, not derived from the proto package

**Location:**

- `crates/anvil-sidecar-client/src/gen/anvil.v1.rs`
- `sidecar/internal/contract/sidecar.pb.go`
- `crates/anvil-sidecar-client/src/lib.rs`
- `sidecar/internal/contract/sidecar_contract_test.go`
- `proto/anvil/v1/sidecar.proto`

**Problem:**

R3 restores package-version hinges using constants:

Rust:

```rust
pub const PROTO_PACKAGE: &str = "anvil.v1";
```

Go:

```go
const ProtoPackageName = "anvil.v1"
```

Tests assert those constants equal `"anvil.v1"`.

This is better than R2, but it does not actually verify the package declared in `sidecar.proto`:

```proto
package anvil.v1;
```

If someone changes the proto package but forgets to update the constants, or vice versa, the tests can still pass while the canonical proto has drifted.

**Impact:**

- The most important schema-version invariant is still indirectly pinned.
- The test pins handwritten side constants, not the canonical proto source.
- Without descriptors, this is understandable, but still weaker than the review summary implies.

**Suggested fix:**

- Make the package-version hinge read or parse `proto/anvil/v1/sidecar.proto` and assert `package anvil.v1;`.
- Once protoc descriptors are available, use descriptor metadata instead.
- Keep the constants if useful for consumers, but do not rely on them as the only package-version proof.

---

## 3. High — Go “all 6 RPCs” hinge can pass if the interface and unimplemented stub drift together

**Location:**

- `sidecar/internal/contract/sidecar_contract_test.go`
- `sidecar/internal/contract/sidecar_grpc.pb.go`

**Problem:**

`TestSidecarServiceInterface` checks:

```go
var _ contract.SidecarServer = &contract.UnimplementedSidecarServer{}
```

This proves `UnimplementedSidecarServer` implements whatever `SidecarServer` currently requires. But it does **not** prove that `SidecarServer` still contains all six intended RPCs.

If both `SidecarServer` and `UnimplementedSidecarServer` accidentally drop `ReloadConfig`, for example, this test still passes.

**Impact:**

- The test is weaker than the R3 doc claims.
- It does not independently pin:
  - `Handshake`
  - `Invoke`
  - `InvokeStreaming`
  - `Cancel`
  - `Health`
  - `ReloadConfig`
- Service-shape drift can still slip through if the generated-like files are edited consistently but incorrectly.

**Suggested fix:**

- Add assertions against `Sidecar_ServiceDesc`:
  - `ServiceName == "anvil.v1.Sidecar"`
  - unary method names are exactly `Handshake`, `Invoke`, `Cancel`, `Health`, `ReloadConfig`
  - stream descriptors contain exactly `InvokeStreaming`
  - `InvokeStreaming.ServerStreams == true`
- Add compile-time assignments for an explicit fake implementation with all six methods if desired.
- Add equivalent Rust-side future checks once client stubs are generated.

---

## 4. Medium — Drift detection remains partial and hand-maintained

**Location:**

- `sidecar/internal/contract/sidecar.pb.go`
- `sidecar/internal/contract/sidecar_grpc.pb.go`
- `crates/anvil-sidecar-client/src/gen/anvil.v1.rs`
- `justfile`

**Problem:**

R3 says the Go hinge tests serve as the P3a drift guard. They help, but they only cover selected surface features. They do not verify:

- all field numbers
- all protobuf tags
- oneof tag correctness
- optional scalar generation semantics
- map encoding
- gRPC method descriptors fully
- generated client/server compatibility
- proto source vs committed bindings equivalence

The lack of `just check-proto-drift` remains deferred.

**Impact:**

- Manual generated-like code can still diverge from the proto in subtle ways.
- The most dangerous errors are not field-name errors but tag/wire-format errors.
- P3b/P3c can inherit mismatches that tests do not catch.

**Suggested fix:**

- Add a lightweight proto-source parser test now, even before protoc, to compare:
  - package name
  - message names
  - enum names
  - RPC names
- Add `just check-proto-drift` as soon as protoc is available.
- Treat regenerated output diff as mandatory before P3c implementation.

---

## 5. Medium — Version negotiation documentation is internally inconsistent

**Location:**

- `proto/README.md`
- `proto/anvil/v1/sidecar.proto`

**Problem:**

The README says:

```text
"Negotiated version" means the first entry in the Vault's supported_versions list that the sidecar also supports.
```

That is a preference-order algorithm.

But the same paragraph also says:

```text
Implementations must not use string comparison to rank versions — parse and compare the integer N.
```

Parsing and comparing integer `N` implies a ranking algorithm, which conflicts with “first supported entry in Vault preference order.”

**Impact:**

- Rust and Go implementations may choose different negotiation behavior.
- One side may respect Vault order while the other computes a numeric maximum.
- The original ambiguity is reduced but not fully eliminated.

**Suggested fix:**

- Pick exactly one rule:
  - **Preference-order:** sidecar scans Vault’s `supported_versions` in order and selects the first it supports. No numeric comparison is needed except optional validation of `vN` syntax.
  - **Numeric highest:** both sides parse `N` and select the highest common `N`.
- Given the R3 stated intent, use preference-order and revise the sentence to: “Implementations may validate that versions match `vN`, but must not rank by string or numeric comparison.”

---

## 6. Medium — No-overlap handshake behavior still lacks a precise transport/error contract

**Location:**

- `proto/README.md`
- `proto/anvil/v1/sidecar.proto`

**Problem:**

README says:

```text
If no overlap exists ... the sidecar must reject the Handshake with an appropriate error.
```

But `HandshakeResponse` has no error envelope field. That means rejection must happen via gRPC status, but the contract does not say:

- which gRPC status code to use
- whether an `AnvilError` is encoded in status details
- whether this maps to `ERROR_CLASS_SCHEMA_VIOLATION`, `TRANSPORT`, or another class
- what the client should surface

**Impact:**

- P3b and P3c may implement incompatible handshake failure behavior.
- No-overlap is listed as a P3b hinge, so the exact expected failure mode should be contractually defined now.

**Suggested fix:**

- Define no-overlap as a specific gRPC status, probably `FailedPrecondition` or `InvalidArgument`.
- Define whether an `AnvilError` detail is attached.
- State the intended `ErrorClass`.
- Add this to the README’s Version negotiation section.

---

## 7. Medium — P3b handoff says Rust must enable client generation, but current generated Rust file has handwritten constants that protoc will overwrite

**Location:**

- `crates/anvil-sidecar-client/build.rs`
- `crates/anvil-sidecar-client/src/gen/anvil.v1.rs`
- `proto/README.md`

**Problem:**

R3 adds a manual constant to the Rust generated file:

```rust
pub const PROTO_PACKAGE: &str = "anvil.v1";
```

But P3b handoff says to run real generation with `tonic_build`. Real `tonic_build` output will likely not preserve this custom constant unless a post-generation patch step adds it.

**Impact:**

- `test_proto_package_version` may fail immediately when P3b regenerates Rust bindings.
- P3b implementer may be forced to re-add manual edits to generated files.
- This weakens the “never edit generated files directly” policy.

**Suggested fix:**

- Do not put custom constants inside generated files unless generation explicitly emits them.
- Move `PROTO_PACKAGE` to a non-generated Rust module, or generate it via build script from the proto package.
- Make the test source-of-truth the proto file or descriptor instead.

---

## 8. Medium — Go contract tests depend on manual bootstrap APIs that may not match real protoc output

**Location:**

- `sidecar/internal/contract/sidecar_contract_test.go`
- `sidecar/internal/contract/sidecar.pb.go`

**Problem:**

The Go tests use manually defined shapes such as:

```go
contract.InvokeRequest_Chat
contract.Credentials_ApiKey
contract.ProtoPackageName
```

The oneof wrapper names are likely to match protoc output, but the custom `ProtoPackageName` constant will not. Other generated helper methods and descriptor surfaces are absent.

When real protoc output replaces the manual files, some tests may fail for reasons unrelated to the actual contract.

**Impact:**

- P3c regeneration may break R3 tests.
- Tests currently validate the bootstrap API, not necessarily real generated Go API.
- There is risk of accumulating permanent manual patches over generated code.

**Suggested fix:**

- Keep tests focused on protoc-stable generated names and descriptors.
- Avoid testing custom constants in generated files.
- Once protoc is available, replace bootstrap tests with descriptor-based tests.

---

## 9. Low / Medium — `ErrorClass` count tests still do not detect value drift

**Location:**

- `crates/anvil-sidecar-client/src/lib.rs`
- `sidecar/internal/contract/sidecar_contract_test.go`

**Problem:**

Both Rust and Go count six non-unspecified classes by constructing an array manually. This catches deletion at compile time if a named variant disappears, but it does not independently verify discriminant values.

For example, if `ERROR_CLASS_TIMEOUT` changes from `5` to `50`, the count tests still pass.

**Impact:**

- Wire compatibility can break even though count tests pass.
- Error-class numeric values are part of the protobuf contract.

**Suggested fix:**

- Assert numeric discriminants:
  - `TRANSPORT == 1`
  - `PROVIDER_REFUSAL == 2`
  - `SCHEMA_VIOLATION == 3`
  - `ADAPTER_BUG == 4`
  - `TIMEOUT == 5`
  - `CANCELLED == 6`
- Add the same check in both Rust and Go.

---

## 10. Low / Medium — `proto/README.md` says huge timeouts are “silently clamped”

**Location:**

- `proto/README.md`

**Problem:**

The validation section says:

```text
Huge values are silently clamped to the sidecar's configured maximum.
```

Silent clamping may be convenient, but it is a semantic choice that can hide caller mistakes. Elsewhere the contract emphasizes schema violations for invalid request envelopes.

**Impact:**

- A user may request an unexpectedly large timeout and not realize it was reduced.
- Rust/Go implementations may disagree on whether to clamp, warn, or reject.
- “Silently” conflicts with an auditable workflow ethos.

**Suggested fix:**

- Consider making oversized timeout a `SCHEMA_VIOLATION`.
- Or require clamping to be explicit in response metadata/logging.
- At minimum, define the configured maximum source and whether clamping must be observable.

---

## 11. Low — README says parse version integer even though accepted versions are package names like `anvil.v1` elsewhere

**Location:**

- `proto/README.md`
- `Anvil Plan/ANVIL_PLAN.md`

**Problem:**

The README version negotiation section describes versions as:

```text
vN
```

But the schema-versioning discussion elsewhere treats package versions as:

```text
anvil.v1
anvil.v2
```

The proto field examples use `"v1"`, so this may be intentional, but the distinction between protocol version string and protobuf package name should be made explicit.

**Impact:**

- Implementers may send `anvil.v1` in `supported_versions` while another side expects `v1`.
- This is minor but can become a connection failure.

**Suggested fix:**

- Explicitly state that handshake versions are `vN`, while protobuf package names are `anvil.vN`.
- Or use the package name as the handshake version consistently.

---

## Overall Assessment

R3 fixes the biggest R2 issue at the **type-shape level**: the Go side no longer exposes only the stale P0 `Ping` contract, and both Rust and Go now have better hinge coverage.

However, I would still be cautious about approving P3a as “fully ready” unless the team explicitly accepts the following boundary:

> P3a delivers a manually maintained type-shape contract, not runtime-usable protobuf/gRPC generated bindings.

If that boundary is acceptable, then R3 is much closer to approval, with the remaining issues mostly around stronger drift detection and sharper contract wording.

Minimum recommended before approval:

1. Make the Go runtime-inoperability caveat prominent in `proto/README.md`.
2. Change package-version hinges to verify the actual `.proto` package, not only manual constants.
3. Strengthen the Go service test to assert all six RPC names in `Sidecar_ServiceDesc`.
4. Clarify version negotiation as either preference-order or numeric-highest, not both.
5. Define exact no-overlap handshake error semantics.

With those addressed or formally accepted, the remaining items can reasonably defer to P3b/P3c.