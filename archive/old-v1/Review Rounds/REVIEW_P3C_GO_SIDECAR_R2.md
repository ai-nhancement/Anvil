# Anvil — P3c Go Sidecar R2

**Phase:** P3c — Go sidecar gRPC server + vendor adapters + daemon lifecycle  
**Round:** R2  
**Date:** 2026-05-25  
**Addresses:** REVIEW_P3C_GO_SIDECAR_R1_Findings.md (external review, 15 findings)

---

## Validation

- `go build ./...` — **passes** (clean)
- `go test ./... -v` — **passes**: 23 tests across 5 packages
- `go vet ./...` — **passes** (clean)

Test breakdown:
| Package | Tests | New in R2 |
|---|---|---|
| `cmd/anvil-sidecar` | 2 | 0 (unchanged) |
| `internal/adapters` | 6 | 0 (1 new case added to existing test) |
| `internal/config` | 2 | 2 |
| `internal/contract` | 6 | 0 (P3a hinge tests retained) |
| `internal/server` | 7 | 5 (bufconn integration tests) |

---

## R1 Findings Resolution

### Finding 1 — Daemon lifecycle: wrong paths, missing fields, no heartbeat — **FIXED**

**Files:** `cmd/anvil-sidecar/main.go`, `internal/daemon/daemon.go`

R1 wrote PID/port to `~/.anvil/<pid>.{pid,port}` and used `~/.anvil/registry.json`. R2 corrects to the plan spec:

- PID: `{workspace}/.anvil/run/sidecar.pid`
- Port: `{workspace}/.anvil/run/sidecar.port`
- Global registry: `~/.anvil/global-registry.json`
- `--workspace` flag added (default `.`; resolved to absolute path before use)
- `RegistryEntry` now includes `WorkspacePath` and `LastSeenAt`
- `daemon.StartHeartbeat(workspaceAbs, reg, 60*time.Second)` wired; channel closed on shutdown
- Registry key is the absolute workspace path (not `configPath`)

---

### Finding 2 — Cancel not guarded by Handshake — **FIXED** (in R2 server.go)

`Cancel` now calls `requireHandshake` before processing. A `Cancel` on a connection that has not completed `Handshake` returns `FailedPrecondition`. Verified by `TestCancelNoHandshake` integration test.

---

### Finding 3 — Unary Invoke cancel not registered — **FIXED** (in R2 server.go)

`Invoke` now wraps its context with `context.WithCancel` and stores the cancel function in `s.cancels` before calling the adapter. The cancel is removed via `defer s.cancels.Delete(key)`. Both unary and streaming calls are now cancellable by the `Cancel` RPC.

---

### Finding 4 — Streaming transport errors close stream without terminal event — **FIXED** (in R2 server.go)

`InvokeStreaming` no longer propagates adapter transport errors as bare gRPC errors. All non-context error paths now send a `StreamError` event (TRANSPORT or TIMEOUT class) and return `nil`, upholding the stream state machine invariant. Only `context.Canceled` (client disconnect / Cancel RPC) returns a gRPC `Canceled` status, since `stream.Send` would fail anyway.

---

### Finding 5 — Google API key embedded in URL — **FIXED**

**File:** `internal/adapters/google.go`

`googleHeaders()` now takes `ak string` and returns `x-goog-api-key: {key}` as a header instead of embedding the key in the URL query string. Both `Invoke` and `InvokeStreaming` use this. The key no longer appears in URL-derived error strings, log output, or HTTP client traces.

Before:
```
url := fmt.Sprintf("%s/v1beta/models/%s:generateContent?key=%s", endpoint, req.ModelId, ak)
resp, err := doHTTP(ctx, http.MethodPost, url, googleHeaders(), body)
```
After:
```
url := fmt.Sprintf("%s/v1beta/models/%s:generateContent", endpoint, req.ModelId)
resp, err := doHTTP(ctx, http.MethodPost, url, googleHeaders(ak), body)
```

---

### Finding 6 — Config format not pinned in proto README — **FIXED**

**File:** `proto/README.md`

Added a **Config format** paragraph to the Contract invariants section specifying:
- Format is JSON (not TOML)
- Epoch = lowercase hex SHA-256 of the raw JSON bytes as written to disk
- Both sides must hash the identical byte sequence

---

### Finding 9 — nil config footgun partially fixed — **FIXED** (in R2 server.go)

`server.New` panics with a descriptive message if `cfg == nil`. Combined with R1 in-round fix (test uses typed nil pointer), the footgun is fully closed.

---

### Finding 10 — No test proves per-connection handshake isolation — **FIXED**

**File:** `internal/server/server_integration_test.go`

`TestTwoConnectionsIndependentHandshakeState` opens two gRPC connections to the same server. conn1 performs a Handshake; conn2 does not. The test verifies that conn2's `Cancel` returns `FailedPrecondition` despite conn1's successful handshake, proving connection state is not shared.

---

### Finding 11 — NewGRPCServer helper missing — **FIXED** (in R2 server.go)

`server.NewGRPCServer(srv, opts...)` creates the gRPC server with the stats handler wired and the service registered in a single call. `main.go` now uses `server.NewGRPCServer(srv)` instead of the three-step manual wiring pattern.

---

### Finding 13 — OpenAI `content_policy_violation` not mapped to PROVIDER_REFUSAL — **FIXED**

**Files:** `internal/errors/errors.go`, `internal/adapters/adapters_test.go`

Added `"content_policy_violation" → ERROR_CLASS_PROVIDER_REFUSAL` to `OpenAIErrorClass`. Test case added to `TestErrorClassMappingOpenAI`.

---

### Finding 14 — Health handshake exemption not documented — **FIXED**

**File:** `proto/README.md`

Connect-time invariant now explicitly states: `Health` is exempt from the Handshake-first requirement to support liveness and readiness probes before any client session is established.

---

### Finding 15 — No hinge test pins config epoch algorithm — **FIXED**

**File:** `internal/config/config_test.go` (new)

Two new tests:
- `TestConfigEpochComputation`: pins the exact SHA-256 hex value (`978741969065f5be40b642a7a4eba801218a64516989f30e16a7f1c28f257138`) for the fixture `{"version":1,"connections":[]}`. A change to the epoch algorithm breaks this test.
- `TestConfigEpochIsLowercaseHex`: verifies epoch is 64 lowercase hex characters.

---

### Finding 7 — No real gRPC integration test — **FIXED**

**File:** `internal/server/server_integration_test.go` (new)

Five bufconn integration tests covering the minimum approval requirement:
- `TestHealthNoHandshake` — Health exempt from handshake
- `TestInvokeNoHandshake` — Invoke returns FailedPrecondition without handshake
- `TestCancelNoHandshake` — Cancel returns FailedPrecondition without handshake (Finding 2)
- `TestHandshakeSuccessEpochMatch` — full Handshake round-trip with matching epoch
- `TestTwoConnectionsIndependentHandshakeState` — per-connection state isolation (Finding 10)

---

## Findings Deferred to P4a (unchanged from R1 + external review)

| Finding | Description |
|---|---|
| R1 Finding 1 | Config epoch canonicalization — JSON format now documented; byte-level canonicalization deferred |
| R1 Finding 5 | HTTP client 120s global timeout vs per-request context deadline |
| R1 Finding 6 | OpenAI: sends FinalResult on unexpected EOF (truncated stream) |
| R1 Finding 7 | Google AI Studio: non-SSE fallback not detected |
| External Finding 8 | `ReloadConfig` UUIDv7 validation not applied to `new_config_epoch` |
| External Finding 12 | Provider response shape validation (empty candidates, empty content) |
| Token-observable streaming | `InvokeStream` `events()` API (P3b deferred item) |
| Retry/backoff on Transport errors | (P3b deferred item) |

---

## Summary

R2 resolves all minimum-approval findings from the external review:

1. **Daemon lifecycle** corrected to plan spec: workspace-scoped PID/port files, global registry at `~/.anvil/global-registry.json`, `WorkspacePath`/`LastSeenAt` in entries, 60s heartbeat.
2. **Security**: Google API key moved from URL to `x-goog-api-key` header.
3. **Cancel guarded** by `requireHandshake`; unary `Invoke` cancel now registered.
4. **Streaming transport errors** converted to terminal `StreamError` events.
5. **`NewGRPCServer` helper** enforces correct wiring.
6. **5 bufconn integration tests** validate handshake enforcement and connection isolation.
7. **2 config epoch hinge tests** pin the SHA-256 algorithm.
8. **OpenAI `content_policy_violation`** maps to `ERROR_CLASS_PROVIDER_REFUSAL`.
9. **proto/README.md** documents Health exemption and JSON config format.

**R2 is ready for approval.**
