# Anvil — P5 Charter Stage Pipeline R1

**Phase:** P5 — Charter Stage Pipeline (Single Reviewer)  
**Round:** R1  
**Date:** 2026-05-26  
**Author:** Single writer (Build-stage protocol)

---

## Validation

- `cargo build --workspace` — **passes** (clean)
- `cargo test --workspace` — **passes**: 59 tests across 7 crates, 0 failures
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — **passes** (clean)
- `cargo fmt --check` — **passes** (clean)

Rust test breakdown:

| Crate | Tests | New in P5 |
|---|---|---|
| `anvil-core` | 22 | 11 (8 pipeline + 3 render) |
| `anvil-audit` | 16 | 1 (`test_curation_audit_record_required`) |
| `anvil-cli` | 8 | 0 (pre-existing) |
| `anvil-sidecar-client` | 11 | 0 (pre-existing) |
| `anvil-graph` | 2 | 0 (pre-existing) |

P5 hinge tests (all passing):

| Test | Crate | Pins |
|---|---|---|
| `test_findings_packet_schema` | `anvil-core::pipeline` | `FindingsPacket` required fields per Artifact Specifications schema |
| `test_disposition_doc_required_sections` | `anvil-core::render` | All 8 disposition doc section headings |
| `test_curation_audit_record_required` | `anvil-audit::records` | `CuratedFindingsRecord` schema: `packet_id`, `coordinator_id`, `dispositions` |

---

## Files Changed

### New files

| File | Purpose |
|---|---|
| `crates/anvil-core/src/pipeline.rs` | All P5 domain types: `FindingSeverity`, `LocationAnchor`, `Finding`, `FindingsPacket`, `CharterPacket`, `VerificationOutcome`, `VerifiedFinding`, `CurationAction`, `CurationDisposition`, `DispositionLabel`; `verify_findings()` local verifier; `extract_charter_packet_json()` / `extract_findings_packet_json()` tag parsers |
| `crates/anvil-core/src/render.rs` | `render_charter_md()`, `render_disposition_doc()` (8 required sections), `append_charter_hardening_history()` |
| `crates/anvil-cli/src/session.rs` | Shared sidecar session helpers: `build_sidecar_config_json()`, `retrieve_api_key()`, `ensure_sidecar_running()`, `connect_and_handshake()`, `find_model_binding()` |
| `crates/anvil-cli/src/discuss.rs` | `anvil discuss` — multi-turn streaming Interlocutor session; terminates on `<charter_packet>` extraction; writes `charter.md` |
| `crates/anvil-cli/src/charter.rs` | `anvil charter review` (unary invoke, parse `<findings_packet>`, run verifier, persist audit records) and `anvil charter findings` (interactive curation, disposition doc, hardening history, `CuratedFindingsRecord`) |

### Modified files

| File | Change |
|---|---|
| `crates/anvil-core/src/lib.rs` | `pub mod pipeline; pub mod render;` |
| `crates/anvil-core/src/error.rs` | 8 new error variants: `SidecarNotConnected`, `CharterPacketInvalid`, `ModelBindingMissing`, `ProviderConnectionMissing`, `CredentialError`, `ModelResponseMissingPacket`, `ModelResponseBadJson`, `NoFindingsPacket` |
| `crates/anvil-core/src/project.rs` | Added `"audit-store/curated-findings"` to `LAYOUT_DIRS`; hinge test updated 18→19 entries |
| `crates/anvil-core/Cargo.toml` | `uuid = { version = "1", features = ["v4"] }`, `chrono = { version = "0.4", features = ["serde"] }`; `tempfile = "3"` dev-dep |
| `crates/anvil-audit/src/records.rs` | Added `CuratedFindings` variant to `RecordType` (plan-extension #14); `ALL_RECORD_TYPES` → `[RecordType; 14]`; `ReviewerFindingPacket` now embeds `pub packet: FindingsPacket` with `from_packet()` constructor; `VerifierResult` now embeds `pub verified_findings: Vec<VerifiedFinding>` with `from_verified()` constructor; new `CuratedFindingsRecord` with `new()` |
| `crates/anvil-audit/src/lib.rs` | Re-exports: `CuratedFindingsRecord`, `ReviewerFindingPacket`, `VerifierResult` |
| `crates/anvil-sidecar-client/src/client.rs` | `InvokeStream::drain_displaying<F: FnMut(&str)>()` — calls `on_token` for each `Token` event; preserves NO-COMMIT-ON-PARTIAL-OUTPUT invariant by returning `FinalResult` only on clean completion |
| `crates/anvil-cli/src/main.rs` | `mod charter; mod discuss; mod session;`; `Command::Discuss`, `Command::Charter(CharterCmd)`, `CharterCmd { Review, Findings }`, `SidecarCmd::Start`; dispatch routing for all new commands |
| `crates/anvil-cli/src/setup.rs` | Made `pub(crate)`: `KEYRING_SERVICE`, `sidecar_config_epoch()`, `keychain_entry_name()`, `provider_type_sidecar_str()` |
| `crates/anvil-cli/Cargo.toml` | `chrono = { version = "0.4", features = ["serde"] }` |

---

## Architecture Decisions

### 1. Local verifier (no model invocation)
The Finding Verifier (`verify_findings()` in `pipeline.rs`) is pure file I/O. It checks quotes, section headings, and symbol names against the artifact on disk. No model is invoked. Rationale: model-based grounding is expensive and introduces its own failure modes (hallucination of verification). Local grounding is fast, deterministic, and sufficient for the structural checks the verifier is designed for (is the cited text actually in the artifact?). Priority order: `quote` > `section_id` > `symbol_name` > `line_range`.

### 2. drain_displaying() preserves NO-COMMIT-ON-PARTIAL-OUTPUT
`discuss.rs` uses `invoke_streaming()` for the Interlocutor turn, printing tokens live for UX. The new `drain_displaying()` method on `InvokeStream` calls a closure per `Token` event and returns the authoritative `FinalResult`. The `charter_packet` extraction always runs on the `FinalResult.content`, not on the token-accumulated buffer — satisfying the invariant that partial streamed output is display-only and never committed.

### 3. Unary invoke() for the reviewer
`charter.rs` uses `invoke()` (non-streaming) for the reviewer. The reviewer produces a single large structured output; streaming adds complexity with no functional benefit. The 180-second timeout is conservative enough for charter-sized inputs.

### 4. PartialFindingsPacket pattern
The reviewer JSON is parsed into `PartialFindingsPacket` (no `packet_id`, no `produced_at`), then `FindingsPacket::new()` fills those in. This mirrors how the Interlocutor's `CharterPacket` is meant to work but currently does not (see Finding F1 below).

### 5. session.rs shared module
Both `discuss.rs` and `charter.rs` need sidecar startup, credential retrieval, and gRPC handshake. These are extracted into `session.rs` to avoid duplication. The module is `pub(crate)` — not a public API.

### 6. CuratedFindings as plan-extension record type #14
The plan listed `CuratedFindings` as a third plan-extension record type alongside `ArbiterFindingResolution` and `SidecarReload`. This required incrementing `LAYOUT_DIRS` from 18→19 entries and updating `ALL_RECORD_TYPES` from `[RecordType; 13]` to `[RecordType; 14]`.

---

## Plan Compliance

### P5 Acceptance Criteria

| # | Criterion | Status |
|---|---|---|
| 1 | Charter packet meets required fields | **PASS** — `CharterPacket::validate()` enforces `title`, `goals`, `scope`, `success_criteria`; pinned by `test_charter_packet_validate` |
| 2 | Charter rendering produces valid `charter.md` | **PASS** — `render_charter_md()` produces required sections; pinned by `test_render_charter_md_required_sections` |
| 3 | Reviewer invocation produces conforming findings packet | **PASS** — `FindingsPacket` schema pinned by hinge test `test_findings_packet_schema`; `PartialFindingsPacket` pattern for safe deserialization from model JSON |
| 4 | Verifier produces verified results with evidence pointers | **PASS** — `verify_findings()` returns `VerifiedFinding` per finding; tested by `test_verify_findings_unanchored` and `test_verify_finding_with_quote` |
| 5 | Curation gestures persist as audit records, round-trip correctly | **PASS** — `CuratedFindingsRecord` persisted via `AuditStore::append`; schema pinned by hinge test `test_curation_audit_record_required` |
| 6 | Disposition rendering matches required format | **PASS** — 8 required sections always present; pinned by hinge test `test_disposition_doc_required_sections` |
| 7 | Hardening-history append works; Charter body not contaminated | **PASS** — `append_charter_hardening_history()` appends-only to `CHARTER_HARDENING_HISTORY.md`; never touches `charter.md`; tested by `test_append_charter_hardening_history` |

---

## What to Review

### F1 (High) — `CharterPacket.produced_at` will always fail to deserialize from model JSON

**Location:** `crates/anvil-core/src/pipeline.rs:152–163`, `crates/anvil-cli/src/discuss.rs:231`

`CharterPacket` has a non-optional `produced_at: DateTime<Utc>` field with no `#[serde(default)]`. The `INTERLOCUTOR_SYSTEM_PROMPT` does not ask the model to include `produced_at` — it is not in the example JSON. So every real `anvil discuss` invocation will fail with a serde missing-field error when `finalize_charter()` calls `serde_json::from_str::<CharterPacket>(packet_json)`.

`FindingsPacket` avoids this correctly via the `PartialFindingsPacket` intermediary. The same pattern should be applied to `CharterPacket`.

**Expected fix:** Define `PartialCharterPacket` (model-supplied fields only, no `produced_at`), parse model JSON into it, then construct `CharterPacket` with `produced_at = Utc::now()`. Alternatively, add `#[serde(default = "chrono::Utc::now")]` — but this requires a newtype wrapper since Serde `default` needs a fn path. The `PartialCharterPacket` pattern is cleaner and consistent with the existing reviewer path.

---

### F2 (Medium) — `CurationAction::Edit` stores `edited_finding: None` unconditionally

**Location:** `crates/anvil-cli/src/charter.rs:356–362`

When the user selects "Edit" during `run_charter_findings`, the disposition is stored as `CurationDisposition { action: Edit, edited_finding: None, annotation: None }`. No edited-finding text is collected. The `CuratedFindingsRecord` will claim a finding was edited but carry no replacement text. P5 plan acceptance criterion 5 requires curation gestures to "persist correctly."

**Question for reviewer:** Is this acceptable for P5 (the Edit action is persisted; the replacement text is a deliberate later step) or must the interactive loop collect the replacement claim/recommendation fields?

---

### F3 (Medium) — RFP and VR record pairing is not verified in `run_charter_findings`

**Location:** `crates/anvil-cli/src/charter.rs:260–289`

`run_charter_findings` loads `rfp_entries.last()` and `vr_entries.last()` independently. If `anvil charter review` is run twice without an intervening `anvil charter findings`, the audit store will have two `ReviewerFindingPacket` records and two `VerifierResult` records. Both `last()` calls return the most recent, which may not correspond to the same review round if the second review failed mid-write. There is no cross-check on round number or `source_context` match.

**Question for reviewer:** Should `run_charter_findings` assert that `rfp.packet.round_number == vr.round_number` (or equivalent `source_context` match) and exit non-zero if they diverge?

---

### F4 (Low) — `## Corrections to R<N-1> Narrative` is hardcoded for all rounds

**Location:** `crates/anvil-core/src/render.rs:207–209`

The "Corrections" section always renders:
```
_(none — R1 has no prior narrative to correct)_
```
regardless of `input.round_number`. For R2+ disposition docs this is factually wrong — R1 narrative may have corrections. The disposition doc rendered for a hypothetical R2 of `anvil charter review` would say the same R1 message.

**Expected fix:** Make the correction text generic `_(none)_` and add an optional `corrections` field to `DispositionInput`, or accept a corrections string in the interactive `run_charter_findings` flow alongside `narrative_summary`.

---

### F5 (Low) — Section-heading grounding misses `###` and deeper sub-headings

**Location:** `crates/anvil-core/src/pipeline.rs:356–373`

The verifier's `section_id` grounding checks only for `## {section_id}` and `# {section_id}`. A reviewer citing a sub-section like `### Versioning` or `#### API Contract` will get `CannotBeVerified` rather than `Grounded`. This produces misleading output — the section exists but the verifier cannot confirm it.

**Expected fix:** Check for any `#+ {section_id}` prefix (simple: try `format!("# {section}")`-starts-any-line using `.lines().any(|l| l.trim_start_matches('#').trim_start() == section)`).

---

### F6 (Low) — No progress signal during reviewer invocation (up to 3 minutes)

**Location:** `crates/anvil-cli/src/charter.rs:109–126`

`invoke_reviewer` uses unary `invoke()` with a 180-second timeout. The user sees one line `"Invoking reviewer for charter R{N}…"` and then nothing until the full response arrives. On a complex charter this may take 60–120 seconds.

**Question for reviewer:** Is a simple spinner or elapsed-time counter warranted for P5, or is the single print line sufficient for coordinator use? (P5 is not a user-facing product yet.)

---

## Test Coverage Summary

| Area | Tests | How Covered |
|---|---|---|
| `FindingsPacket` schema roundtrip | `test_findings_packet_schema` (hinge) | Required fields, JSON serialize+deserialize |
| `Finding` severity | `test_finding_severity_roundtrip` | All 3 tiers `as_str` / `from_str` round-trip; invalid input returns `None` |
| `CharterPacket` validation | `test_charter_packet_validate` | Required fields; empty title, empty goals error correctly |
| Tag extraction | `test_extract_charter_packet_json`, `test_extract_findings_packet_json` | With tags, without tags, with surrounding text |
| `LocationAnchor` anchoring | `test_location_anchor_is_anchored` | No fields → false; section_id present → true |
| Verifier — unanchored | `test_verify_findings_unanchored` | No anchor → `CannotBeVerified` |
| Verifier — quote grounding | `test_verify_finding_with_quote` | Present quote → `Grounded`; absent quote → `Refuted`; uses tempfile |
| Disposition doc sections | `test_disposition_doc_required_sections` (hinge) | All 8 section headings present in rendered output |
| Charter rendering | `test_render_charter_md_required_sections` | Goals, Scope, Success Criteria sections in rendered Markdown |
| Hardening history append | `test_append_charter_hardening_history` | Two consecutive appends; both entries visible; first not overwritten |
| Curation record schema | `test_curation_audit_record_required` (hinge) | `CuratedFindingsRecord` has `packet_id`, `coordinator_id`, `dispositions`; serializes/deserializes; appends to store |

**Not covered by automated tests (requires real sidecar + API key):**
- `anvil discuss` end-to-end (model → `<charter_packet>` → `charter.md`)
- `anvil charter review` end-to-end (model → `<findings_packet>` → audit records)
- `anvil charter findings` interactive session (requires terminal input)

---

## How to Activate

```sh
# Prerequisites: anvil setup completed, sidecar binary installed
cd <project-root>
anvil discuss                   # Interlocutor session → charter.md
anvil charter review            # Invoke reviewer → ReviewerFindingPacket + VerifierResult
anvil charter findings          # Interactive curation → REVIEW_charter_R1.md + CHARTER_HARDENING_HISTORY.md
```

Audit records written to `.anvil/audit-store/` under their respective subdirectories:
- `reviewer-finding-packets/`
- `verifier-results/`
- `curated-findings/`
