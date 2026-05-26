# Anvil — P8 Build Stage Pipeline R2 Findings

**Source review doc:** `Review Rounds/REVIEW_P8_BUILD_STAGE_PIPELINE_R2.md`  
**Review date:** 2026-05-26  
**Reviewer instruction:** Critical review only. Do not write code. Findings are ordered most critical first.

## Validation Performed

- `cargo fmt --all -- --check` — **Pass**
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — **Pass**
- `cargo test --workspace` — **Pass** (132 tests)

The R2 validation claim is accurate. The four R1 findings are addressed as claimed: phase review packets now carry briefing hashes, phase ship computes a current briefing hash, `BriefingStatus` constrains the status vocabulary, missing briefing sections can surface as `PhaseBriefingMissingSection`, and `status.rs::compute_hex_hash` delegates to `utils::sha256_hex`.

---

## 1. High — `run_phase_review` selects the same reviewer for R1 and R2

**Location:**

- `crates/anvil-cli/src/phase.rs:186-204`
- `crates/anvil-core/src/rotation.rs:12-25`
- `Anvil Plan/ANVIL_PLAN.md:702`

**Problem:**

`rotation_select` is documented as taking a 1-indexed round number:

```rust
pub fn rotation_select(pool: &[String], round_count: u32) -> Option<&str> {
    let idx = (round_count.saturating_sub(1) as usize) % pool.len();
    Some(&pool[idx])
}
```

But `run_phase_review` passes the existing RFP count, not the next round number:

```rust
let round_count = u32::try_from(phase_rfps.len()).unwrap_or(u32::MAX);
let round_number = round_count + 1;
...
let reviewer_name = rotation_select(&pool, round_count)
```

This means:

- First phase review: `round_count = 0`, `round_number = 1`, `rotation_select(pool, 0)` saturates to index 0.
- Second phase review: `round_count = 1`, `round_number = 2`, `rotation_select(pool, 1)` also selects index 0.
- Third phase review: `round_count = 2`, `round_number = 3`, selects index 1.

So rotation is shifted and reviewer 1 receives both R1 and R2. This violates the P8 acceptance criterion that phase review goes to “the next reviewer in rotation.”

The `prev_reviewer` calculation is similarly shifted:

```rust
let prev_reviewer: Option<String> = if round_count > 0 {
    rotation_select(&pool, round_count - 1).map(ToOwned::to_owned)
} else {
    None
};
```

For R2, this calls `rotation_select(pool, 0)`, which again returns reviewer 1 only because of saturating behavior rather than correct 1-indexed semantics.

**Impact:**

- Multi-reviewer diversity is weakened for phase reviews.
- R2 can be reviewed by the same reviewer family as R1 even when the pool has multiple reviewers.
- Rotation logs become misleading because they record the shifted reviewer sequence.
- Full-pool clean convergence may take more rounds than intended or fail to exercise reviewers in the expected order.

**Suggested fix:**

- Use `rotation_select(&pool, round_number)` for the selected reviewer.
- Use `rotation_select(&pool, round_number - 1)` for `prev_reviewer` when `round_number > 1`, matching the Charter and Plan review implementations.
- Add a regression test with a pool of at least two reviewers asserting phase R1 selects reviewer 1 and phase R2 selects reviewer 2.

---

## 2. High — `run_phase_ship` can ship an older reviewed briefing while a newer briefing exists unreviewed

**Location:**

- `crates/anvil-cli/src/phase.rs:74-78`
- `crates/anvil-cli/src/phase.rs:386-404`
- `Review Rounds/REVIEW_P8_BUILD_STAGE_PIPELINE_R2.md:30-35`

**Problem:**

R2 improves ship by hashing a briefing file, but the file chosen by `run_phase_ship` is derived from the number of RFPs, not from the latest built briefing or latest `phase-{id}-briefing-sent` gate:

```rust
let round_count = u32::try_from(phase_rfps.len()).unwrap_or(u32::MAX);
let latest_briefing = project_root
    .join("reviews")
    .join(format!("BRIEFING_{phase_id}_R{round_count}.md"));
```

A stale-ship path remains:

1. `anvil phase build P8` creates `BRIEFING_P8_R1.md`.
2. `anvil phase review P8` creates a clean RFP with hash of `BRIEFING_P8_R1.md`.
3. Before shipping, `anvil phase build P8` is run again. Because one RFP exists, it writes `BRIEFING_P8_R2.md`.
4. `anvil phase ship P8` still sets `round_count = 1` from the RFP count and hashes `BRIEFING_P8_R1.md`.
5. The clean R1 packet matches the R1 briefing hash, so ship succeeds even though `BRIEFING_P8_R2.md` is the newer phase briefing and has never been reviewed.

This means the R2 hash check verifies “latest reviewed briefing” rather than “latest built phase artifact/briefing.” The R2 disposition says the hash is computed from the latest briefing file, but the implementation’s “latest” is based on review count, not filesystem/gate state.

**Impact:**

- A phase can ship after unreviewed changes to its briefing.
- The phase-scoped termination gate is still incomplete under normal user behavior: build again after review but before ship.
- `PhaseDisposition` and `phase-{id}-ship` can be recorded for stale reviewed state rather than current phase state.

**Suggested fix:**

- Track build/briefing rounds independently from RFP count, preferably via `phase-{id}-briefing-sent` gate records or a typed briefing/build audit record.
- In `run_phase_ship`, determine the latest built briefing for the phase and require clean reviewer packets whose `artifact_hash` matches that briefing.
- If a newer briefing exists than the latest reviewed round, block ship with an explicit “latest briefing has not been reviewed” reason.
- Add a regression test: clean-review R1, create `BRIEFING_P8_R2.md`, then assert `run_phase_ship` fails until R2 is reviewed.

---

## 3. High / Medium — P8 still lacks the required findings curation, disposition rendering, and six-gate completeness check

**Location:**

- `Anvil Plan/ANVIL_PLAN.md:696,705-706`
- `Review Rounds/REVIEW_P8_BUILD_STAGE_PIPELINE_R2.md:102-105`
- `crates/anvil-cli/src/main.rs:194-215`
- `crates/anvil-cli/src/phase.rs:337-445`

**Problem:**

The P8 Plan acceptance criteria require:

```text
6. All six gate-approval audit records created per phase loop; gate check verifies completeness before ship.
7. Coder renders disposition document with all six required sections.
```

R2 explicitly defers the missing pieces:

```text
- `anvil phase findings` (curation + disposition rendering) — deferred to P9/post-ship scope
- Gate types `phase-{id}-findings-curated`, `phase-{id}-disposition-rendered`,
  `phase-{id}-next-reviewer-or-ship` — not yet wired to CLI commands
```

The current CLI only exposes:

- `anvil phase build`
- `anvil phase review`
- `anvil phase ship`

and `run_phase_ship` writes only:

- `phase-{id}-ship` `GateApproval`
- `PhaseDisposition`

It does not check that these gate records exist before ship:

- `phase-{id}-briefing-sent`
- `phase-{id}-findings-received`
- `phase-{id}-findings-curated`
- `phase-{id}-disposition-rendered`
- `phase-{id}-next-reviewer-or-ship`
- `phase-{id}-ship`

It also does not render the required phase disposition document.

Earlier review notes treated this as acceptable deferral, but it directly contradicts the P8 acceptance criteria and the R2 “ready to converge” conclusion.

**Impact:**

- P8 is not end-to-end complete as specified.
- Phase review findings can be verified and stored, but there is no phase-level equivalent of Charter/Plan curation and disposition rendering.
- Ship can happen without proving that findings were curated, a disposition document was rendered, or a human next-reviewer-or-ship decision gate was recorded.
- Audit trail completeness is weaker for Build phases than for Charter/Plan review loops.

**Suggested fix:**

- Either implement `anvil phase findings` now, mirroring Charter/Plan findings curation and rendering, or explicitly amend the P8 acceptance criteria to defer these gates to P9.
- Add a ship preflight that checks the required phase gate records for the target phase and blocks with a named list of missing gates.
- Add tests covering missing gate records and successful ship only after all required gates are present.

---

## 4. Medium — `run_phase_build` can overwrite an existing briefing for the same round after already appending a gate record

**Location:**

- `crates/anvil-cli/src/phase.rs:74-78`
- `crates/anvil-cli/src/phase.rs:141-157`

**Problem:**

`run_phase_build` determines the briefing round from existing RFP count:

```rust
let phase_rfps = filter_rfps_by_artifact(&store, &all_rfp_entries, &artifact_ref_prefix);
let round_number = u32::try_from(phase_rfps.len()).unwrap_or(u32::MAX) + 1;
```

If a user runs `anvil phase build P8` twice before any review, both invocations compute `round_number = 1` and target the same file:

```rust
reviews/BRIEFING_P8_R1.md
```

The command appends a `phase-P8-briefing-sent` gate before writing the file:

```rust
store.append(&gate)?;
std::fs::write(&briefing_path, doc.as_bytes())?;
```

Because `std::fs::write` overwrites, the second build replaces the first R1 briefing while leaving two append-only gate records that both point to `phase:P8:§briefing:R1`.

**Impact:**

- Briefing files are mutable by accident despite audit records being append-only.
- Multiple briefing-sent gates can reference the same round while only the last file contents remain.
- A write failure after the gate append also leaves a gate claiming a briefing was sent even when the file was not written.
- This contributes to stale/ambiguous ship-state behavior because build rounds are not tracked independently.

**Suggested fix:**

- Derive briefing round from briefing/gate records, not RFP records, or reject building a round if its briefing file already exists.
- Use create-new semantics for briefing files rather than overwrite semantics.
- Consider writing the file to a temporary path first, then append the gate only after the durable briefing artifact exists, or write an explicit failure/recovery record if the file write fails after gate append.
- Add a regression test that a repeated build before review cannot overwrite `BRIEFING_{id}_R1.md` or duplicate its gate.

---

## 5. Medium — Phase briefing contracts are not checked against the requested phase ID

**Location:**

- `crates/anvil-cli/src/phase.rs:60-130`
- `crates/anvil-core/src/phase_briefing.rs:83-85`

**Problem:**

`run_phase_build(project_root, phase_id, ...)` accepts a phase ID from the CLI, but after parsing the model’s `PhaseBriefingContract`, it only validates section completeness:

```rust
let briefing = parse_phase_briefing_contract(&response)?;
validate_phase_briefing_contract(&briefing)?;
```

There is no check that:

```rust
briefing.phase_id == phase_id
```

So `anvil phase build P8` can write `reviews/BRIEFING_P8_R1.md` containing a rendered header for `P9` if the model emits `"phase_id": "P9"`. The artifact reference and gate records still use the requested phase prefix `phase:P8`, while the human-facing document says it is for another phase.

**Impact:**

- Reviewers can be sent a briefing that claims to cover a different phase than the audit records indicate.
- Ship and review gates may be recorded for P8 while the briefing content is for P9.
- Audit cross-references become misleading.

**Suggested fix:**

- After parsing, require the contract phase ID to exactly match the CLI `phase_id` argument.
- Also consider validating `spec_section` against the phase ID or loaded plan contract entry.
- Add a regression test for mismatched requested phase and contract phase.

---

## 6. Medium — Reviewer binding identity is not authoritative in phase review packets

**Location:**

- `crates/anvil-cli/src/phase.rs:197-213`
- `crates/anvil-cli/src/phase.rs:252-268`
- `crates/anvil-cli/src/status.rs:252-302`

**Problem:**

`run_phase_review` invokes a specific configured reviewer binding:

```rust
let reviewer_name = rotation_select(...).to_owned();
let binding = find_model_binding(&config, &reviewer_name)?;
```

But the persisted `FindingsPacket.reviewer_id` comes from the model response:

```rust
let partial: PartialFindingsPacket = serde_json::from_str(packet_json)?;
...
let mut packet = FindingsPacket::new(
    format!("{artifact_ref_prefix}:R{round_number}"),
    round_number,
    partial.reviewer_id,
    reviewer_model_identity,
    partial.findings,
);
```

The full-pool clean check keys latest packets by `rfp.packet.reviewer_id` and compares that to configured pool binding names:

```rust
let reviewer_id = rfp.packet.reviewer_id.clone();
latest_by_reviewer.entry(reviewer_id) ...
...
let reviewer_rfp = latest_by_reviewer.get(binding_name.as_str());
```

Therefore, if the model omits `reviewer_id`, emits the default `reviewer-1`, or emits any value different from the binding name, the audit packet and full-pool clean logic are attributed to the wrong reviewer. Conversely, a bad model response can claim a different reviewer binding than the one actually invoked.

**Impact:**

- Full-pool clean can incorrectly report missing reviewers or credit the wrong reviewer.
- Rotation diversity enforcement depends on model-supplied identity rather than the Vault’s known invoked binding.
- Audit records can disagree: `GateApproval.approver` and `RotationLog.rotated_to` use `reviewer_name`, while `ReviewerFindingPacket.reviewer_id` may use model-supplied `partial.reviewer_id`.

**Suggested fix:**

- Treat the configured reviewer binding (`reviewer_name`) as authoritative for `FindingsPacket.reviewer_id`.
- Preserve model-supplied identity only in metadata if useful, or validate it matches the invoked binding before accepting the packet.
- Add a regression test where the model packet has mismatched `reviewer_id` and assert the persisted packet uses or requires the invoked binding.

---

## Overall Assessment

P8 R2 fixes the four R1 findings and the codebase remains fmt/clippy/test clean. The hash addition is a meaningful improvement over R1, and the status/section validation fixes are solid.

However, P8 should not be marked fully converged yet. The remaining issues affect core per-phase workflow correctness:

1. Phase reviewer rotation is off by one and repeats reviewer 1 for R1 and R2.
2. Phase ship can approve an older reviewed briefing while a newer briefing exists unreviewed.
3. The six-gate and disposition-document requirements in the P8 acceptance criteria are still deferred rather than implemented or formally amended.
4. Briefing artifacts can be overwritten for the same round.
5. The model-produced briefing phase ID and reviewer ID are trusted where the Vault should enforce authoritative identities.
