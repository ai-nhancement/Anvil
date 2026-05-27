# Leaflog Charter

**Version:** R2 (final, converged)  
**Date:** 2026-05-21  
**Owner:** [friend's first name redacted]  
**Pilot coordinator:** jvcan

---

## Purpose

Leaflog is a command-line tool for tracking the care of houseplants. I have 34 plants across five rooms and currently track watering in a notebook. The notebook doesn't remind me when plants are overdue, and I can't filter by room when I'm standing in the living room wondering which ones need water today. Leaflog solves this with a terminal command I can run any time.

The scope is personal use — one person, one computer, no sync, no sharing. If it grows beyond that, that's a future problem.

---

## Core Principles

1. **Data is mine and stays local.** SQLite on my filesystem. No cloud sync, no accounts, no telemetry.
2. **No surprises from the UI.** Plain terminal output — no TUI, no color (unless I ask for it), no ncurses. I run this in tmux and I don't want anything touching my terminal state.
3. **Reminders are informational, not nagging.** `leaflog remind` tells me what's overdue; it does not send push notifications, emails, or cron spam. I run it when I want it.
4. **Destructive operations confirm before acting.** Deleting a plant or clearing its history asks first, or requires `--yes`.
5. **Small scope.** This is not a plant encyclopedia, a photo journal, or a light-meter integration. It is a watering log with reminders.

---

## Constraints

- **No network access.** The tool makes no outbound requests in v1.
- **No GUI.** Terminal-only.
- **Single user.** The data model does not support multiple users or plant sharing.
- **SQLite only.** No PostgreSQL, no flat-file alternatives. The query requirement (filter by room) needs SQL.
- **Rust CLI.** The Coordinator will implement in Rust. Not because it needs to be fast (it doesn't), but because that's what the Coordinator knows.

---

## Non-Goals (Explicit)

- Photo storage for plants
- Light meter or soil sensor integration  
- Web interface or mobile app
- Cloud backup or sync
- Multi-user or sharing features
- Species database or care-schedule templates

---

## Deliverable

A CLI binary `leaflog` that supports:

```sh
leaflog add "Monstera" --room "Living Room" --interval 7
leaflog list [--room <name>]
leaflog water <plant-id> [--note "Looks healthy"]
leaflog log <plant-id>
leaflog remind
leaflog export --format csv
leaflog export --format json
leaflog delete <plant-id> [--yes]
```

---

## Governance

Single Coordinator (the Coordinator is building this for the Owner). The Owner has final say on scope decisions. Disputes resolved by the Owner saying "no."

---

## Success Criteria

I can open a terminal, run `leaflog remind`, and immediately know which plants need water today without opening a notebook.
