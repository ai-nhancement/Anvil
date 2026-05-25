# Anvil — P3c Go Sidecar R1 Findings

**Source review doc:** `Review Rounds/REVIEW_P3C_GO_SIDECAR_R1.md`  
**Review date:** 2026-05-25  
**Reviewer instruction:** Critical review only. Do not write code. Findings are ordered most critical first.

## Validation Performed

- `go build ./...` from `C:\Anvil\sidecar` — **passes**
- `go test ./... -v` from `C:\Anvil\sidecar` — **passes**
- `go vet ./...` from `C:\Anvil\sidecar` — **passes**
- `cargo test -p anvil-sidecar-client` — **passes**: 11 tests

The implementation is meaningfully further along than prior P3 rounds: Go protobuf bindings are now real generated code, the sidecar server implements the full service interface, provider adapters exist, and validation passes. However, the R1 review under-rates several issues that affect P3c’s stated scope: gRPC server behavior, daemon lifecycle, cancellation, streaming terminal guarantees, and credential safety.

---

## 1. High — Daemon lifecycle does not match the Plan’s workspace/global registry contract

**Location:**

- `sidecar/cmd/anvil-sidecar/main.go`
- `sidecar/internal/daemon/daemon.go`
- `Anvil Plan/ANVIL_PLAN.md`

**Problem:**

The Plan’s P3c daemon lifecycle says the sidecar should:

- bind to loopback
- write PID and port to `.anvil/run/sidecar.pid` and `.anvil/run/sidecar.port`
- also register globally in `~/.anvil/global-registry.json`
- include active daemon metadata such as workspace path, pid, port, started time, and `last_seen_at`
- update `last_seen_at` periodically
- support global-aware sidecar management later via the registry

The implementation instead writes:

```go
~/.anvil/<pid>.pid
~/.anvil/<pid>.port
~/.anvil/registry.json
```

and the registry entry contains:

```go
type RegistryEntry struct {
    PID         int    `json:"pid"`
    Port        int    `json:"port"`
    ConfigEpoch string `json:"config_epoch"`
    ConfigPath  string `json:"config_path"`
    StartedAt   string `json:"started_at"`
}
```

There is no `last_seen_at`, no periodic heartbeat update, no workspace-scoped `.anvil/run` path, and the registry path/name differs from the Plan.

**Impact:**

- P3c does not fully deliver the daemon lifecycle described by the Plan.
- Later `anvil sidecar status --all` and stale-daemon cleanup work will not have the required registry data.
- PID/port discovery by the Rust Vault side may look in `.anvil/run`, while the sidecar writes to user-home pid-specific files.
- Multiple workspaces/configs may be harder to reason about because the registry is keyed by config path rather than explicit workspace identity.

**Suggested fix:**

- Align file locations with the Plan, or update the Plan if the intended contract has changed.
- Add `last_seen_at` and periodic heartbeat updates to the global registry.
- Use the documented registry path `~/.anvil/global-registry.json`, or explicitly revise the documented path.
- Add lifecycle tests for PID/port file creation, registry registration, heartbeat update, and unregister-on-clean-exit.

---

## 2. High — `Cancel` is not handshake-guarded despite the connect-time contract

**Location:**

- `sidecar/internal/server/server.go`
- `proto/README.md`
- `proto/anvil/v1/sidecar.proto`
- `Review Rounds/REVIEW_P3C_GO_SIDECAR_R1.md`

**Problem:**

R1 summary says:

> Handshake-first enforcement is correct: ... `requireHandshake`/`requireReady` guards on all non-Health RPCs.

But `Cancel` does not call `requireHandshake` or `requireReady`:

```go
func (s *AnvilServer) Cancel(ctx context.Context, req *contract.CancelRequest) (*contract.CancelResponse, error) {
    cancelled := false
    if v, ok := s.cancels.Load(req.IdempotencyKey); ok {
        v.(context.CancelFunc)()
        cancelled = true
    }
    return &contract.CancelResponse{Cancelled: cancelled}, nil
}
```

`Health` being exempt is reasonable for liveness probes, but `Cancel` is an application RPC and the proto contract says `Handshake` must be the first RPC on every connection.

**Impact:**

- A client can call `Cancel` before handshake and receive a successful response.
- The implementation contradicts both the review summary and the connect-time contract.
- Per-connection protocol state is not consistently enforced.

**Suggested fix:**

- Guard `Cancel` with `requireHandshake`.
- Decide whether `Cancel` should require full config readiness or only protocol handshake. Usually protocol handshake is enough.
- Add a server test for `Cancel` before handshake returning `FailedPrecondition`.

---

## 3. High — `Cancel` only works for streaming calls, not unary `Invoke`

**Location:**

- `sidecar/internal/server/server.go`
- `proto/anvil/v1/sidecar.proto`
- `Anvil Plan/ANVIL_PLAN.md`

**Problem:**

The `Cancel` RPC is described generically as cancellation of an in-flight call by idempotency key. The Plan also says cancellation should propagate.

Current implementation registers cancellation only in `InvokeStreaming`:

```go
ctx, cancel := context.WithCancel(ctx)
s.cancels.Store(key, cancel)
defer s.cancels.Delete(key)
```

Unary `Invoke` never stores a cancel function in `s.cancels`, so `Cancel` cannot cancel a long-running unary request.

**Impact:**

- `Cancel` behaves differently for unary and streaming calls without contract documentation.
- A caller cannot cancel long-running unary model calls even though the RPC suggests it can.
- Ctrl-C cancellation behavior may be incomplete if the Vault uses unary `Invoke` for non-streaming calls.

**Suggested fix:**

- Register cancellation for unary `Invoke` as well as `InvokeStreaming`.
- Or explicitly document that `Cancel` only applies to streaming calls and adjust the contract/comments accordingly.
- Add tests for both unary and streaming cancellation registration.

---

## 4. High — Generic streaming transport errors can terminate the gRPC stream without a terminal `Error` event

**Location:**

- `sidecar/internal/server/server.go`
- `sidecar/internal/adapters/adapters.go`
- `proto/README.md`
- `Anvil Plan/ANVIL_PLAN.md`

**Problem:**

The streaming state machine requires exactly one terminal event: `FinalResult` or `Error`.

However, the adapter interface explicitly allows transport failure to return a non-nil error without sending a terminal event:

```go
// On transport failure, returns a non-nil error without sending a terminal event.
InvokeStreaming(...) error
```

The server only converts timeout/cancel cases specially. For other errors it returns the error directly:

```go
err := adapter.InvokeStreaming(ctx, conn, req, send)
if err != nil {
    if ctx.Err() == context.DeadlineExceeded { ... send StreamError ... }
    if ctx.Err() == context.Canceled { return status.Error(codes.Canceled, "streaming canceled") }
    return err
}
```

That means generic HTTP/read/SSE errors terminate the gRPC stream via status error, not via an `InvokeStreamEvent_Error` event.

**Impact:**

- The stream can close without a terminal contract event.
- This violates the documented P3a/P3c streaming invariant.
- The Vault-side client may see transport failure rather than the typed `AnvilError` stream event expected by the contract.

**Suggested fix:**

- Convert all adapter streaming errors into a terminal `StreamError` event where possible, then return nil.
- Reserve gRPC status errors for true transport/session failures outside the contract envelope.
- Add tests for adapter transport error → exactly one `StreamError` event and no continuation.

---

## 5. High — Google API key is placed in the URL, risking credential leakage in error surfaces

**Location:**

- `sidecar/internal/adapters/google.go`
- `sidecar/internal/adapters/adapters.go`
- `sidecar/internal/server/server.go`

**Problem:**

Google requests embed the API key in the URL:

```go
url := fmt.Sprintf("%s/v1beta/models/%s:generateContent?key=%s", endpoint, req.ModelId, ak)
url := fmt.Sprintf("%s/v1beta/models/%s:streamGenerateContent?key=%s&alt=sse", endpoint, req.ModelId, ak)
```

If `http.Client.Do` returns a network or URL error, Go error strings commonly include the request URL. Those errors are wrapped:

```go
return nil, fmt.Errorf("google HTTP: %w", err)
```

and then can be returned to the Vault as an `AnvilError` message or gRPC status text.

**Impact:**

- Google API keys can leak into error messages, logs, or audit-visible surfaces.
- This conflicts with the sidecar trust-boundary invariant that secrets are consumed per-call and not persisted/logged.
- Network failures are exactly when verbose error text is most likely to be surfaced.

**Suggested fix:**

- Prefer sending the API key via a header if supported, such as `x-goog-api-key`.
- If query parameter authentication must be used, redact `key=` from all error strings before returning or logging them.
- Add a test that simulates an HTTP error and asserts the API key is absent from returned errors.

---

## 6. Medium / High — Config format mismatch is deferred, but it can break the P3b/P3c reload handshake

**Location:**

- `sidecar/internal/config/config.go`
- `proto/README.md`
- `Anvil Plan/ANVIL_PLAN.md`
- `Review Rounds/REVIEW_P3C_GO_SIDECAR_R1.md`

**Problem:**

R1 correctly notes that the sidecar config parser expects JSON while prior contract text discusses TOML/provider-config bytes from `anvil.toml`.

The implementation comment says:

```go
// Format is JSON (not TOML as noted in the proto README). Canonicalize before P4a ships.
```

But config epoch comparison and `ReloadConfig` are part of the P3b/P3c split-brain fix, not merely a P4 concern.

**Impact:**

- The Vault and sidecar may compute different config epochs for the same logical config.
- `ReloadConfig` may fail if the Vault sends TOML-derived bytes and the sidecar expects JSON.
- The P3c server can appear correct in isolation while failing when wired to the Rust Vault client.

**Suggested fix:**

- Pin the provider-config wire/storage format before approving the P3b/P3c boundary.
- If JSON is the intended sidecar provider-config format, update proto docs and Vault-side handoff docs now.
- Add a cross-language fixture: known config bytes → known SHA-256 epoch in Rust and Go.

---

## 7. Medium — The server does not validate UUIDv7 idempotency keys

**Location:**

- `sidecar/internal/server/server.go`
- `proto/README.md`

**Problem:**

`proto/README.md` says:

```text
InvokeRequest.idempotency_key | Must be a non-empty UUIDv7 string
```

The server checks only non-empty:

```go
if key == "" {
    return invokeErrResp(key, schemaErr("idempotency_key is required")), nil
}
```

The streaming path has the same non-empty-only check.

**Impact:**

- Invalid idempotency keys are accepted.
- Cancel correlation and audit correlation can rely on malformed keys.
- The implementation does not enforce the documented schema-validation rule.

**Suggested fix:**

- Validate UUID syntax and version 7 specifically.
- Return `ERROR_CLASS_SCHEMA_VIOLATION` for malformed keys.
- Add unary and streaming tests for invalid idempotency keys.

---

## 8. Medium — Timeout bounds from the contract are not enforced

**Location:**

- `sidecar/internal/server/server.go`
- `proto/README.md`

**Problem:**

`proto/README.md` says oversized timeout values must be rejected with `ERROR_CLASS_SCHEMA_VIOLATION` and silent clamping is not permitted.

The server accepts any `uint64` timeout and converts it directly:

```go
ctx, cancel = context.WithTimeout(ctx, time.Duration(req.Timeout.Millis)*time.Millisecond)
```

Very large values can overflow `time.Duration` or produce surprising behavior. There is no configured maximum check.

**Impact:**

- The server does not enforce the documented timeout contract.
- Oversized values may behave incorrectly rather than returning a schema violation.
- This undermines deterministic validation behavior between Vault and sidecar.

**Suggested fix:**

- Define a sidecar maximum timeout.
- Reject timeout values above that maximum with `ERROR_CLASS_SCHEMA_VIOLATION`.
- Guard `uint64` → `time.Duration` conversion against overflow.
- Add tests for zero, normal, oversized, and overflow-range timeout values.

---

## 9. Medium — `server.New` still accepts nil config; only the test footgun was removed

**Location:**

- `sidecar/internal/server/server.go`
- `sidecar/internal/server/server_test.go`
- `Review Rounds/REVIEW_P3C_GO_SIDECAR_R1.md`

**Problem:**

R1 says the nil-config issue was resolved in-round by changing the interface test to use:

```go
var _ contract.SidecarServer = (*server.AnvilServer)(nil)
```

That avoids constructing a nil-config server in the test, but `server.New` still accepts nil:

```go
func New(cfg *config.Config, cfgPath, version string, touch func()) *AnvilServer {
    return &AnvilServer{cfg: cfg, cfgPath: cfgPath, version: version, touch: touch}
}
```

and `Handshake` still dereferences `s.cfg`:

```go
epoch := s.cfg.Epoch()
```

**Impact:**

- The actual runtime footgun remains.
- A future caller can still create a server that panics on first handshake.
- The disposition overstates the fix: it fixed the test setup, not the constructor contract.

**Suggested fix:**

- Add an explicit nil guard in `New` or return `(*AnvilServer, error)`.
- If panic-on-programmer-error is preferred, panic immediately in `New`, not during `Handshake`.
- Add a unit test for nil config behavior.

---

## 10. Medium — No integration test exercises the server over real gRPC

**Location:**

- `sidecar/internal/server`
- `sidecar/internal/contract`
- `sidecar/cmd/anvil-sidecar/main.go`

**Problem:**

The current tests are mostly compile-time or pure unit checks:

- adapter registry and error mapping
- service interface conformance
- contract shape
- supported version list

There is no test that starts a `grpc.Server`, registers `AnvilServer`, connects a generated client, and exercises:

- `Health`
- `Handshake`
- pre-handshake rejection
- config mismatch → `ReloadConfig` → ready
- schema violation responses

**Impact:**

- Runtime gRPC serialization, stats-handler connection state, and service registration are not tested end-to-end.
- The generated bindings are real now, but the actual network path is not proven by tests.
- Bugs in connection-scoped handshake state could pass unit tests.

**Suggested fix:**

- Add a loopback/bufconn gRPC integration test for the server.
- Specifically test the stats handler path because connection identity is central to handshake enforcement.
- Include both same-connection and new-connection behavior.

---

## 11. Medium — Handshake-first connection state depends on `stats.Handler`; server registration does not enforce it by itself

**Location:**

- `sidecar/internal/server/server.go`
- `sidecar/cmd/anvil-sidecar/main.go`

**Problem:**

Connection state is created only through `NewStatsHandler`:

```go
gs := grpc.NewServer(grpc.StatsHandler(sh))
```

If a future test, embedder, or alternate server setup registers `AnvilServer` without the stats handler, every guarded RPC returns:

```go
connection state not found
```

This is not necessarily wrong, but the dependency is implicit and untested over a real gRPC server.

**Impact:**

- The server is easy to miswire.
- `Register` alone is insufficient to produce a working server.
- Future daemon lifecycle or test code can break handshake behavior without a compile-time signal.

**Suggested fix:**

- Provide a helper that constructs the fully wired `grpc.Server` with stats handler and registration.
- Document that `NewStatsHandler` is mandatory.
- Add an integration test that fails if the stats handler is omitted or if connection state is unavailable.

---

## 12. Medium — Provider response-shape validation is weak and can turn malformed provider output into transport/adapter errors inconsistently

**Location:**

- `sidecar/internal/adapters/anthropic.go`
- `sidecar/internal/adapters/openai.go`
- `sidecar/internal/adapters/google.go`

**Problem:**

Provider response parsing generally trusts partially populated JSON. Examples:

- Anthropic unary concatenates text blocks but does not fail on empty/missing content.
- OpenAI unary fails empty choices as a transport-style Go error later wrapped as `TRANSPORT` by the server.
- Streaming parsers skip malformed chunks silently:

```go
if err := json.Unmarshal([]byte(value), &chunk); err != nil {
    return nil // skip malformed chunks
}
```

**Impact:**

- Malformed provider responses may become empty `FinalResult` events instead of `ADAPTER_BUG`.
- Different adapters classify parse/shape failures inconsistently.
- The Plan’s provider-diversity stress can fail later due to silently malformed outputs.

**Suggested fix:**

- Define adapter response-shape validation rules.
- Treat malformed provider JSON/chunks and impossible empty successful responses as `ERROR_CLASS_ADAPTER_BUG` unless there is a clear provider refusal/error payload.
- Add fixture tests for malformed/empty responses per adapter.

---

## 13. Low / Medium — `PROVIDER_REFUSAL` is defined but effectively unused by adapter mappings

**Location:**

- `sidecar/internal/errors/errors.go`
- `sidecar/internal/adapters/adapters_test.go`
- `proto/anvil/v1/sidecar.proto`

**Problem:**

`ErrorClass` includes:

```proto
ERROR_CLASS_PROVIDER_REFUSAL
```

But current adapter mappings classify authentication, permission, rate limit, overload, and many provider-side failures as `TRANSPORT` or `ADAPTER_BUG`. The tests pin these choices.

**Impact:**

- The `PROVIDER_REFUSAL` taxonomy value may never be exercised.
- Callers cannot distinguish provider refusal from transport failure.
- Retry/backoff policy may behave incorrectly if rate limits or provider refusals are treated as transport.

**Suggested fix:**

- Revisit the semantic mapping of vendor errors to `ErrorClass`.
- Define when `PROVIDER_REFUSAL` should be used.
- Adjust hinge tests if the intended taxonomy differs.

---

## 14. Low — `Health` handshake exemption is intentional but should be codified in the proto docs

**Location:**

- `sidecar/internal/server/server.go`
- `proto/README.md`
- `proto/anvil/v1/sidecar.proto`

**Problem:**

R1 accepts that `Health` is exempt from handshake-first enforcement. That is reasonable for liveness probes. But the proto service comment still says:

```text
Handshake must be the first RPC called on every connection. The sidecar rejects all other RPCs until a successful Handshake.
```

Without an explicit exception, the implementation contradicts the contract text.

**Impact:**

- Future implementers may treat `Health` differently.
- Contract conformance tests may later flag the sidecar as violating the literal text.

**Suggested fix:**

- Update proto comments and README to state: `Health` is exempt from handshake-first for liveness/readiness probing.
- Keep all other application RPCs, including `Cancel`, guarded.

---

## 15. Low — Config epoch hinge test is still missing

**Location:**

- `sidecar/internal/config/config.go`

**Problem:**

R1 identifies this and accepts it as a P4a limitation. There is still no test pinning SHA-256 epoch computation for known input bytes.

**Impact:**

- Epoch algorithm changes can happen silently.
- Cross-language config-epoch drift remains easier to introduce.

**Suggested fix:**

- Add `TestConfigEpochComputation` with a known byte fixture and expected SHA-256.
- Mirror the same fixture in Rust once the Vault computes provider-config epochs.

---

## Overall Assessment

P3c is a substantial implementation milestone and the basic package-level validation passes. The generated Go protobuf files are now real runtime bindings, the server implements the full `Sidecar` interface, and the three required provider adapters are present.

However, I would **not approve P3c yet** as fully satisfying the Plan. The R1 document marks only the nil-config test issue as blocking, but several larger issues remain:

- daemon lifecycle paths/registry do not match the Plan
- `Cancel` is not handshake guarded
- `Cancel` does not work for unary calls
- streaming transport failures can close without a terminal `Error` event
- Google API keys can leak through URL-bearing error strings
- config format/epoch mismatch can break Vault/sidecar reload interoperability

Minimum recommended before approval:

1. Align daemon PID/port/registry behavior with the Plan or update the Plan explicitly.
2. Guard `Cancel` with handshake and clarify readiness requirements.
3. Register unary `Invoke` calls in the cancellation map or document `Cancel` as streaming-only.
4. Convert streaming adapter transport errors into terminal `StreamError` events.
5. Remove or redact Google API keys from URLs/error surfaces.
6. Pin provider-config format and epoch computation before relying on `ReloadConfig` across Rust/Go.
7. Add at least one real gRPC integration test for handshake-first and reload-ready behavior.

The remaining items can reasonably defer if explicitly tracked, but the above issues affect core P3c contract behavior and daemon operation.