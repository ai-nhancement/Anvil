# Leaflog — Implementation Plan

**Date:** 2026-05-21  
**Status:** Approved (R1 converged; hardening applied)  
**Phases:** 4 (P0, P1, P2, P3)

---

## Phase Summary

| Phase | Name | Description | Deps |
|---|---|---|---|
| P0 | Bootstrap | Project init, plant model, SQLite store | — |
| P1 | Core Commands | `add`, `list`, `water`, `log`, `delete` | P0 |
| P2 | Reminders | `remind` command, due-date calculation | P1 |
| P3 | Export | `export --format csv\|json` | P1 |

Critical path: P0 → P1 → (P2 ∥ P3).

---

## P0 — Bootstrap

**Goal.** Cargo workspace, SQLite schema, migration runner, `leaflog --version`.

**Deliverables:**
- `Cargo.toml` workspace with `leaflog-cli` and `leaflog-db` crates
- `leaflog-db`: `plants` table (id, name, room, watering_interval_days, created_at), `events` table (id, plant_id, event_type, occurred_at, note)
- Migration runner (embedded SQL using `rusqlite_migration` or equivalent)
- `leaflog --version` exits 0

**Acceptance criteria:**
1. `cargo build --release` succeeds with zero warnings
2. `cargo test` passes
3. `leaflog --version` prints the crate version
4. `leaflog-db` migration runs idempotently on a fresh database

---

## P1 — Core Commands

**Goal.** Full CRUD lifecycle: add plants, log watering events, list plants, view event log, delete plants.

**Commands:**
- `leaflog add <name> --room <room> --interval <days>` — creates plant record, prints assigned ID
- `leaflog list [--room <room>]` — tabular output: ID, name, room, interval, last watered
- `leaflog water <id> [--note <text>]` — records watering event with UTC timestamp
- `leaflog log <id>` — lists all events for a plant in reverse chronological order
- `leaflog delete <id> [--yes]` — prompts for confirmation unless `--yes` is passed; deletes plant and all events

**Acceptance criteria:**
1. All five commands exit 0 on valid input, non-zero on invalid ID or malformed arguments
2. `leaflog list` output is readable in an 80-column terminal
3. `leaflog delete` without `--yes` prompts; with `--yes` does not prompt
4. `cargo test` passes with integration tests covering each command

---

## P2 — Reminders

**Goal.** `leaflog remind` shows plants whose last watering was more than `interval` days ago (or never watered).

**Due-date logic:**
- `overdue_days = today - last_water_date - interval`
- If `overdue_days > 0`: plant is overdue by `overdue_days` days
- If never watered: plant is overdue since `created_at`

**Output format:**
```
OVERDUE (3 days):  Monstera [Living Room]
OVERDUE (1 day):   Pothos [Bedroom]
DUE TODAY:         Spider Plant [Office]
```

Plants with upcoming due dates (within 2 days) are shown under a separate "UPCOMING" section. Plants watered on schedule are not shown.

**Acceptance criteria:**
1. `leaflog remind` exits 0 and prints nothing when all plants are current
2. Overdue calculation is correct across month and year boundaries
3. Plant never watered is treated as overdue from its creation date
4. Integration test covers: all current, some overdue, some upcoming, never-watered

---

## P3 — Export

**Goal.** `leaflog export --format csv` and `leaflog export --format json` write all plant and event data to stdout.

**CSV format:**
```
plant_id,plant_name,room,interval_days,event_type,occurred_at,note
1,Monstera,Living Room,7,water,2026-05-20T14:00:00Z,Looks healthy
```

**JSON format:**
```json
{
  "plants": [
    {
      "id": 1, "name": "Monstera", "room": "Living Room",
      "watering_interval_days": 7, "created_at": "...",
      "events": [
        { "event_type": "water", "occurred_at": "...", "note": "..." }
      ]
    }
  ]
}
```

**Acceptance criteria:**
1. CSV output is valid RFC 4180 (fields with commas or newlines are quoted)
2. JSON output is valid JSON and passes schema validation in the test suite
3. Both formats include all plants and all events, sorted by plant ID then event timestamp
4. Export of an empty database produces a valid empty document (not an error)

---

## Hinge Tests

```
// hinge_test: pins=rusqlite, intended=storage-backend, phase=P0
// hinge_test: pins=80-col, intended=list-output-width, phase=P1
// hinge_test: pins=2-day-window, intended=remind-upcoming-window, phase=P2
```

---

## Provisional Locks

None. The scope is small enough that every decision was settled at Plan time.

---

## Plan-Level Acceptance Criteria

1. All four phases shipped via `anvil phase ship`
2. `anvil ship --project .` passes all gates
3. The owner can run `leaflog remind` on a database with 10+ plants and read the output without referring to documentation
4. `leaflog export --format json` output passes `jq .` validation
