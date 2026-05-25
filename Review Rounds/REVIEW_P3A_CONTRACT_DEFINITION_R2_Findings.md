# Anvil — P3a Contract Definition R2 Findings

**Source review doc:** `Review Rounds/REVIEW_P3A_CONTRACT_DEFINITION_R2.md`  
**Review date:** 2026-05-25  
**Reviewer instruction:** Critical review only. Do not write code. Findings are ordered most critical first.

## Validation Performed

- `cargo test --workspace` — **passes**: 30 tests
- `cargo clippy --workspace -- -D warnings` — **passes**
- `go test ./...` from `C:\Anvil\sidecar` — **passes**
- `go build ./...` from `C:\Anvil\sidecar` — **passes**

Important caveat: Go build/tests pass because the Go side still compiles against the old P0 generated contract, not because it validates the new P3a contract.

---

## 1. Critical — Go generated bindings are stale and still define only the P0 `Ping` contract

**Location:**

- `proto/anvil/v1/sidecar.proto`
- `sidecar/internal/contract/sidecar.pb.go`
- `sidecar/internal/contract/sidecar_grpc.pb.go`

**Problem:**

The proto file now defines the full P3a contract:

- `Handshake`
- `Invoke`
- `InvokeStreaming`
- `Cancel`
- `Health`
- `ReloadConfig`

But the committed Go generated files still contain only the old P0 placeholder:

```go
PingRequest
PingResponse
Sidecar.Ping
```

The generated Go service comments even still say:

```go
This is the P0 placeholder; the full service definition ships in P3a.
```

This directly contradicts the P3a goal of defining a cross-language Vault/sidecar contract. The Rust side has hand-written bindings for the new schema, but the Go sidecar still exposes the old generated API.

**Impact:**

- Rust and Go disagree on the `anvil.v1` contract.
- P3c cannot implement the advertised full sidecar server using the current committed Go bindings.
- The R2 statement “complete `anvil.v1` schema ready for approval” is not true for the Go side.
- Current Go tests do not catch this because they do not assert the presence of `Handshake`, `Invoke`, etc.

**Suggested fix:**

- Regenerate and commit the Go bindings from `proto/anvil/v1/sidecar.proto`.
- Add Go-side hinge tests or compile-time assertions for:
  - `HandshakeRequest`
  - `InvokeRequest`
  - `ErrorClass`
  - `SidecarServer` methods for all six RPCs
- Add a drift check ensuring the Go generated files correspond to the current proto, not the old P0 schema.

---

## 2. High — The accepted “hand-written generated Rust artifact” limitation is already causing real cross-language drift

**Location:**

- `crates/anvil-sidecar-client/src/gen/anvil.v1.rs`
- `sidecar/internal/contract/sidecar.pb.go`
- `proto/anvil/v1/sidecar.proto`
- `crates/anvil-sidecar-client/build.rs`

**Problem:**

R2 accepts the committed-bindings approach as a known P3a limitation. But this is no longer just theoretical. The repo currently has three different contract realities:

1. The canonical proto file contains the full P3a contract.
2. The Rust committed binding mirrors the new message schema but is hand-written.
3. The Go committed binding is still the old P0 `Ping` schema.

This is exactly the drift risk the R1 finding warned about.

**Impact:**

- The system can pass Rust and Go tests while the wire contract is inconsistent.
- Developers may assume `just test` validates the full contract, but it does not.
- P3b/P3c implementation work can begin from mismatched generated artifacts.

**Suggested fix:**

- Treat generated-binding drift as blocking for P3a approval.
- Add a `just gen` run requirement before P3a acceptance.
- Add a drift-detection command even before full CI exists. It can be a local `just check-proto-drift` recipe that:
  - Regenerates into a temporary directory.
  - Diffs generated output against committed generated files.
  - Fails if they differ.
- At minimum, add tests that inspect both Rust and Go generated artifacts for the same expected RPC/message names.

---

## 3. High — P3a no longer has a real package-version hinge despite the Plan still requiring one

**Location:**

- `crates/anvil-sidecar-client/src/lib.rs`
- `proto/anvil/v1/sidecar.proto`
- `Anvil Plan/ANVIL_PLAN.md`
- `Review Rounds/REVIEW_P3A_CONTRACT_DEFINITION_R2.md`

**Problem:**

R2 says:

```text
test_proto_package_version renamed to test_error_class_unspecified_name
```

The renamed test is more accurately named, but it no longer pins the package version. It checks only:

```rust
ErrorClass::Unspecified.as_str_name() == "ERROR_CLASS_UNSPECIFIED"
```

That confirms enum naming, not the package name `anvil.v1`.

However, the Plan still lists:

```text
test_proto_package_version
```

as a P3a build hinge, and the schema-versioning policy depends on `anvil.v1` stability.

**Impact:**

- A change from `package anvil.v1` to something else would not be caught by the renamed test.
- The Plan and implementation disagree about the hinge-test surface.
- The most important contract-version invariant is not actually pinned.

**Suggested fix:**

- Restore a true package-version hinge.
- It should assert the canonical package is `anvil.v1`, ideally by checking generated descriptor metadata or parsing the proto.
- Keep `test_error_class_unspecified_name` if useful, but do not treat it as a replacement for `test_proto_package_version`.
- Update the R2 disposition to distinguish:
  - enum-name stability test
  - package-version stability test

---

## 4. High — Rust bindings suppress generated gRPC client/server stubs, which may block P3b

**Location:**

- `crates/anvil-sidecar-client/build.rs`
- `crates/anvil-sidecar-client/src/gen/anvil.v1.rs`
- `Review Rounds/REVIEW_P3A_CONTRACT_DEFINITION_R1.md`

**Problem:**

The build script explicitly uses:

```rust
.build_server(false)
.build_client(false)
```

R1 documents this as intentional:

```text
No client or server code
```

But P3b’s purpose is a Vault-side gRPC client. With client generation disabled, the Rust crate has message types but no generated `SidecarClient` gRPC stub.

This may be intentional for P3a, but it creates a likely handoff problem: P3b either has to reverse this setting or manually implement tonic calls.

**Impact:**

- P3a may not actually deliver the Rust-side contract surface P3b needs.
- The crate name `anvil-sidecar-client` is misleading if it contains no generated client API.
- The contract is only partially compiled on the Rust side: messages yes, service client no.

**Suggested fix:**

- Decide whether P3a should generate the Rust client stub now.
- If P3b is expected to use tonic-generated client code, enable client generation during P3a.
- If client generation is intentionally deferred, document the exact P3b change required and add an explicit deferred item.
- Add a P3a or P3b acceptance test that proves the generated `SidecarClient` type exists.

---

## 5. Medium — P3a hinge tests lack the structured `hinge_test` annotations used elsewhere

**Location:**

- `crates/anvil-sidecar-client/src/lib.rs`

**Problem:**

Other phases use structured hinge metadata comments such as:

```rust
// hinge_test: pins=..., intended=..., phase=P2
```

But the P3a tests use informal comments:

```rust
// Hinge test — pins=6
```

The Plan says hinge tests are ordinary unit tests with structured comment annotations from P0 onward, and P10b will later auto-discover/register them.

**Impact:**

- P10b discovery may miss these tests.
- P3a hinge metadata is less precise than P2’s.
- Cross-language consensus checks later depend on stable hinge names, pins, intended values, and phases.

**Suggested fix:**

- Convert P3a hinge comments to the same structured format used in P2.
- Include:
  - `pins`
  - `intended`
  - `phase=P3a`
  - whether the hinge is cross-language where relevant
- Ensure names align with the Plan registry.

---

## 6. Medium — No Go-side contract hinge tests despite Plan expecting Rust + Go coverage

**Location:**

- `sidecar/internal/contract`
- `Anvil Plan/ANVIL_PLAN.md`

**Problem:**

The Plan lists these as Rust + Go hinges:

```text
test_error_class_count
test_handshake_required_fields
```

But I found these only on the Rust side. The Go side has no equivalent test asserting:

- error class count
- package version
- handshake fields
- full service methods
- oneof payload structure

**Impact:**

- Cross-language schema drift is not detected.
- The stale Go bindings problem survived because no Go contract hinge exists.
- Future P10b consensus checks will have missing/asymmetric hinge data.

**Suggested fix:**

- Add Go tests under `sidecar/internal/contract`.
- Mirror the Rust hinge names where the Plan expects cross-language consensus.
- At minimum test:
  - package name or descriptor full name
  - `ErrorClass` non-unspecified count
  - `HandshakeRequest` fields
  - `InvokeRequest` payload oneof
  - all six service RPC names

---

## 7. Medium — `Credentials` schema is likely incomplete for the stated provider-secret model

**Location:**

- `proto/anvil/v1/sidecar.proto`

Current schema:

```proto
message Credentials {
  oneof credential {
    string api_key = 1;
    string bearer_token = 2;
  }
}
```

**Problem:**

The Plan says per-call credentials include API keys, SigV4 credentials, OAuth tokens, and similar secret material. The current proto supports only:

- API key string
- bearer token string

It does not model structured credentials such as AWS SigV4:

- access key ID
- secret access key
- session token
- signing region/service if needed

OAuth bearer tokens may fit under `bearer_token`, but SigV4 does not fit cleanly.

**Impact:**

- The P3a contract may be too narrow for later provider adapters.
- Adding structured credential variants later may be non-breaking if new oneof fields are added, but P3a claims to define the complete contract.
- P3c adapter implementation may need to overload `api_key` or `bearer_token` in provider-specific ways, which weakens the contract.

**Suggested fix:**

- Decide whether v1 supports only API-key/bearer-token providers.
- If SigV4 or other structured credentials are in v1 scope, add explicit credential messages now.
- At minimum document credential extensibility and provider compatibility limits in `proto/README.md`.

---

## 8. Medium — Version negotiation semantics are underspecified for string versions

**Location:**

- `proto/anvil/v1/sidecar.proto`
- `proto/README.md`

**Problem:**

The schema says:

```proto
string core_protocol_version = 1;
repeated string supported_versions = 2;
```

and comments say the negotiated version is:

```text
the highest version supported by both sides
```

But protocol versions are strings such as `v1`, `v2`, `v10`. “Highest” is ambiguous unless a numeric ordering rule is specified.

**Impact:**

- Lexicographic ordering would rank `v10` before or after `v2` incorrectly depending on implementation.
- Rust and Go implementations could choose different negotiation behavior.
- This is exactly the kind of boundary rule that should be nailed down in the contract phase.

**Suggested fix:**

- Define negotiation as preference-order based rather than “highest”.
  - Example: sidecar picks the first version in the Vault’s `supported_versions` list that it also supports.
- Or define a strict numeric parse rule for `vN`.
- Add a future P3b test for no-overlap and multi-version negotiation ordering.

---

## 9. Medium — Several “required” semantic fields are not enforceable and lack validation guidance

**Location:**

- `proto/anvil/v1/sidecar.proto`
- `proto/README.md`

**Problem:**

The proto comments and tests refer to required fields, such as:

- `core_protocol_version`
- `supported_versions`
- `idempotency_key`
- `model_id`
- `provider_connection_id`
- `credentials`
- `payload`

But proto3 does not enforce required scalar fields. Empty strings and absent message fields are valid wire values.

This is especially important for:

```proto
Credentials credentials = 4;
oneof payload { ... }
```

In Rust, `credentials` becomes `Option<Credentials>`, so it can be absent despite the contract treating it as required.

**Impact:**

- Sidecar/client implementations may differ in validation strictness.
- Missing credentials or payload could be interpreted as default/empty rather than schema violation.
- `ErrorClass::SCHEMA_VIOLATION` exists, but there is no validation matrix saying which missing/empty fields trigger it.

**Suggested fix:**

- Add a validation section to `proto/README.md`.
- Explicitly state which fields must be non-empty or present.
- Define expected error class for invalid request envelopes.
- Add P3b/P3c conformance tests for missing payload, empty model ID, missing credentials, empty supported version list, etc.

---

## 10. Medium — Streaming terminal-event invariants are documented but not structurally constrained

**Location:**

- `proto/anvil/v1/sidecar.proto`

**Problem:**

The proto comment says:

```text
Exactly one FinalResult or exactly one Error terminates the stream.
```

But the schema does not encode this constraint. That is normal for streaming protocols, but P3a currently has no conformance test or machine-readable contract to preserve this behavior.

**Impact:**

A sidecar implementation could legally emit:

- tokens after an error
- multiple final results
- final result then error
- no terminal event
- heartbeat forever

The proto alone will not prevent this.

**Suggested fix:**

- Document the stream state machine in `proto/README.md`.
- Add P3b/P3c contract-conformance tests later.
- Consider adding an explicit terminal marker or sequence semantics if needed.
- Define behavior when the stream ends without `FinalResult` or `Error`.

---

## 11. Low / Medium — R2 test says “fully-populated `InvokeRequest`” but leaves optional fields unset

**Location:**

- `Review Rounds/REVIEW_P3A_CONTRACT_DEFINITION_R2.md`
- `crates/anvil-sidecar-client/src/lib.rs`

**Problem:**

R2 says the new test constructs a “fully-populated `InvokeRequest`,” but the test sets:

```rust
timeout: None
temperature: None
```

This is minor, but the wording is inaccurate.

**Impact:**

- The test is still useful.
- But it does not pin `Timeout` shape.
- It does not fully exercise optional field presence.

**Suggested fix:**

- Either change the review wording to “representative populated `InvokeRequest`,” or set optional fields to `Some`.
- Consider adding a small assertion for `Timeout.millis` if timeout shape matters to the contract.

---

## 12. Low / Medium — Chat roles are free-form strings rather than a contract enum

**Location:**

- `proto/anvil/v1/sidecar.proto`

Current schema:

```proto
message Message {
  string role = 1;
  string content = 2;
}
```

**Problem:**

`role` is a free-form string. Chat APIs usually distinguish a small set of roles such as:

- system
- user
- assistant
- tool

The current `ChatRequest` also has a separate `system_prompt` field, so it is unclear whether a `Message` may also have role `system`.

**Impact:**

- Adapters may interpret roles differently.
- Contract does not specify valid role values.
- Schema violations around invalid roles are undefined.

**Suggested fix:**

- Define `MessageRole` enum if roles are intended to be portable across providers.
- Or explicitly document accepted role strings and validation behavior.
- Clarify interaction between `system_prompt` and system-role messages.

---

## 13. Low — Timeout and numeric fields lack bounds guidance

**Location:**

- `proto/anvil/v1/sidecar.proto`

Examples:

```proto
optional int32 max_tokens = 3;
optional float temperature = 4;
uint64 millis = 1;
```

**Problem:**

The schema allows:

- `max_tokens <= 0`
- negative or NaN-like temperature representations depending on language handling
- huge timeout values
- zero timeout

**Impact:**

- Implementations may disagree on validation.
- Provider adapters may pass through invalid values and get provider-specific failures instead of `SCHEMA_VIOLATION`.

**Suggested fix:**

- Define validation ranges in README.
- State whether unset means provider/default config.
- Add future conformance tests.

---

## Overall Assessment

The R2 fixes to the Rust-side test coverage are real, and Rust validation passes. However, I would **not approve P3a yet** because the cross-language contract is currently inconsistent:

1. The canonical proto is full P3a.
2. The Rust hand-written binding approximates full P3a messages.
3. The Go generated binding is still P0 `Ping` only.

That is a blocking issue for a “contract definition” phase. The minimum approval path should be:

- Regenerate and commit Go bindings.
- Restore or add a true `anvil.v1` package-version hinge.
- Add Go-side contract hinge tests.
- Add at least a local generated-bindings drift check.

Once those are addressed, the remaining issues are mostly schema-hardening and validation-clarity items that can be handled in P3b/P3c if explicitly deferred.