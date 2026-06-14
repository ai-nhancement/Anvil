//! Rendering functions for Charter and Plan Stage Pipeline artifacts (P5/P7).
//!
//! Produces `charter.md`, Plan documents, Disposition documents
//! (`REVIEW_<artifact>_R<N>.md`), and hardening-history appends.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use crate::error::AnvilError;
use crate::pipeline::{
    AdvisoryDispositionType, CharterPacket, CurationAction, DispositionLabel, VerifiedFinding,
};
use crate::plan::PlannerContract;

// ── Charter rendering ──────────────────────────────────────────────────────────

/// Renders a `CharterPacket` into a `charter.md` string.
///
/// Produces a structured markdown document with the standard Charter sections.
#[must_use]
pub fn render_charter_md(packet: &CharterPacket) -> String {
    let mut out = String::new();

    write!(out, "# {}\n\n", packet.title).ok();
    write!(
        out,
        "**Status:** Draft 1  \n**Produced:** {}\n\n---\n\n",
        packet.produced_at.format("%Y-%m-%d")
    )
    .ok();

    out.push_str("## Goals\n\n");
    for goal in &packet.goals {
        writeln!(out, "- {goal}").ok();
    }
    out.push('\n');

    out.push_str("## Scope\n\n");
    out.push_str(&packet.scope);
    out.push_str("\n\n");

    if !packet.out_of_scope.is_empty() {
        out.push_str("## Out of Scope\n\n");
        for item in &packet.out_of_scope {
            writeln!(out, "- {item}").ok();
        }
        out.push('\n');
    }

    if !packet.required_choices.is_empty() {
        out.push_str("## Required Choices\n\n");
        for choice in &packet.required_choices {
            writeln!(out, "- {choice}").ok();
        }
        out.push('\n');
    }

    out.push_str("## Success Criteria\n\n");
    for (i, criterion) in packet.success_criteria.iter().enumerate() {
        writeln!(out, "{}. {criterion}", i + 1).ok();
    }
    out.push('\n');

    if !packet.stakeholders.is_empty() {
        out.push_str("## Stakeholders\n\n");
        for stakeholder in &packet.stakeholders {
            writeln!(out, "- {stakeholder}").ok();
        }
        out.push('\n');
    }

    if let Some(ref notes) = packet.additional_notes {
        if !notes.trim().is_empty() {
            out.push_str("## Additional Notes\n\n");
            out.push_str(notes);
            out.push_str("\n\n");
        }
    }

    out
}

// ── Disposition document rendering ────────────────────────────────────────────

/// A file entry for the "Files Changed" section of a Disposition document.
pub struct FileChanged {
    pub file: String,
    pub action: String,
    pub purpose: String,
}

/// Input bundle for `render_disposition_doc`.
pub struct DispositionInput<'a> {
    /// Artifact short name (e.g., "charter").
    pub artifact_name: &'a str,
    pub round_number: u32,
    pub reviewer_id: &'a str,
    pub date: &'a str,
    /// All verified findings from the `ReviewerFindingPacket` + `VerifierResult` records.
    pub verified_findings: &'a [VerifiedFinding],
    /// Disposition label assigned per `finding_id` (only for findings with action Keep).
    pub disposition_map: &'a BTreeMap<String, DispositionLabel>,
    pub files_changed: &'a [FileChanged],
    /// Prose narrative of what changed in this round.
    pub narrative_summary: &'a str,
    /// Corrections to the prior round's narrative, if any.
    pub corrections: &'a str,
    /// Notes on findings deferred to a future phase or round.
    pub residual_notes: &'a str,
    /// Shell commands a reviewer can run to verify the live state.
    pub reproducibility_commands: &'a str,
    /// One-to-two sentence summary for reviewers who read only this section.
    pub bottom_line: &'a str,
    /// `CurationAction` per `finding_id`.
    pub curation_actions: &'a BTreeMap<String, CurationAction>,
    /// Advisory disposition type and annotation text per advisory `finding_id` (P6).
    /// Used to render explicit labels (`Accept-Advisory`, `Drop-Advisory: <reason>`, etc.)
    /// instead of `—` for advisory findings.
    pub advisory_dispositions: &'a BTreeMap<String, (AdvisoryDispositionType, Option<String>)>,
}

/// Renders a full Disposition document per the Artifact Specifications template.
#[allow(clippy::too_many_lines)]
///
/// All 9 sections are always present:
/// 1. Header block
/// 2. What Changed in R`<N>`
/// 3. Verification of R`<N>` Claims
/// 4. Disposition of R`<N>` Findings
/// 5. Files Changed Since R`<N-1>`
/// 6. Corrections to R`<N-1>` Narrative
/// 7. Residual / Deferred
/// 8. Reproducibility
/// 9. Bottom Line
#[must_use]
pub fn render_disposition_doc(input: &DispositionInput<'_>) -> String {
    let r = input.round_number;
    let prev = r.saturating_sub(1);
    let art = input.artifact_name;
    let mut out = String::new();

    // Header block
    write!(out, "# {art} — R{r} Disposition\n\n").ok();
    write!(
        out,
        "**Date:** {}  \n**Artifact:** {}  \n**Round:** R{}  \n**Reviewer:** {}\n\n---\n\n",
        input.date, art, r, input.reviewer_id
    )
    .ok();

    // 2. What Changed
    write!(out, "## What Changed in R{r}\n\n").ok();
    if input.narrative_summary.trim().is_empty() {
        out.push_str("_(no narrative provided)_\n\n");
    } else {
        out.push_str(input.narrative_summary);
        out.push_str("\n\n");
    }

    // 3. Verification of R<N> Claims
    write!(out, "## Verification of R{r} Claims\n\n").ok();
    out.push_str("| Finding | Verifiable Claim | Verified? | Notes |\n");
    out.push_str("|---|---|---|---|\n");
    for vf in input.verified_findings {
        let claim = escape_md_table(&vf.finding.claim);
        let outcome = vf.outcome.as_str();
        let note = escape_md_table(&vf.evidence_note);
        writeln!(out, "| {} | {claim} | {outcome} | {note} |", vf.finding.id).ok();
    }
    out.push('\n');

    // 4. Disposition of R<N> Findings
    write!(out, "## Disposition of R{r} Findings\n\n").ok();
    out.push_str("| # | Severity | Finding | Disposition |\n");
    out.push_str("|---|---|---|---|\n");
    for vf in input.verified_findings {
        let sev = vf.finding.severity.as_str();
        let claim = escape_md_table(&vf.finding.claim);
        let label = if vf.finding.advisory {
            match input.advisory_dispositions.get(&vf.finding.id) {
                Some((AdvisoryDispositionType::AcceptAdvisory, _)) => "Accept-Advisory".to_owned(),
                Some((AdvisoryDispositionType::DropAdvisory, note)) => {
                    if let Some(n) = note {
                        format!("Drop-Advisory: {}", escape_md_table(n))
                    } else {
                        "Drop-Advisory".to_owned()
                    }
                }
                Some((AdvisoryDispositionType::DeferAdvisory, note)) => {
                    if let Some(n) = note {
                        format!("Defer-Advisory: {}", escape_md_table(n))
                    } else {
                        "Defer-Advisory".to_owned()
                    }
                }
                None => "—".to_owned(),
            }
        } else {
            let action = input
                .curation_actions
                .get(&vf.finding.id)
                .map_or("keep", CurationAction::as_str);
            if action == "drop" {
                "Dropped".to_owned()
            } else {
                input
                    .disposition_map
                    .get(&vf.finding.id)
                    .map_or_else(|| "—".to_owned(), |l| l.as_str().to_owned())
            }
        };
        writeln!(out, "| {} | {sev} | {claim} | {label} |", vf.finding.id).ok();
    }
    out.push('\n');

    // 5. Files Changed Since R<prev>
    write!(out, "## Files Changed Since R{prev}\n\n").ok();
    if input.files_changed.is_empty() {
        out.push_str("_(not collected in this round)_\n\n");
    } else {
        out.push_str("| File | Action | Purpose |\n");
        out.push_str("|---|---|---|\n");
        for fc in input.files_changed {
            writeln!(
                out,
                "| `{}` | {} | {} |",
                fc.file,
                fc.action,
                escape_md_table(&fc.purpose)
            )
            .ok();
        }
        out.push('\n');
    }

    // 6. Corrections to R<prev> Narrative
    write!(out, "## Corrections to R{prev} Narrative\n\n").ok();
    if input.corrections.trim().is_empty() {
        out.push_str("_(none)_\n\n");
    } else {
        out.push_str(input.corrections);
        out.push_str("\n\n");
    }

    // 7. Residual / Deferred
    out.push_str("## Residual / Deferred\n\n");
    if input.residual_notes.trim().is_empty() {
        out.push_str("_(none)_\n\n");
    } else {
        out.push_str(input.residual_notes);
        out.push_str("\n\n");
    }

    // 8. Reproducibility
    out.push_str("## Reproducibility\n\n");
    if input.reproducibility_commands.trim().is_empty() {
        out.push_str("_(no commands provided)_\n\n");
    } else {
        out.push_str("```sh\n");
        out.push_str(input.reproducibility_commands);
        out.push_str("\n```\n\n");
    }

    // 9. Bottom Line
    out.push_str("## Bottom Line\n\n");
    if input.bottom_line.trim().is_empty() {
        out.push_str("_(no bottom line provided)_\n");
    } else {
        out.push_str(input.bottom_line);
        out.push('\n');
    }

    out
}

fn escape_md_table(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

// ── Hardening history append ───────────────────────────────────────────────────

/// Appends a hardening-history entry to `<project_root>/CHARTER_HARDENING_HISTORY.md`.
///
/// Each entry is a dated paragraph; the Charter body is never modified.
///
/// # Errors
///
/// Returns [`AnvilError::Io`] on filesystem failure.
pub fn append_charter_hardening_history(
    project_root: &Path,
    round_number: u32,
    reviewer_id: &str,
    date: &str,
    summary: &str,
) -> Result<(), AnvilError> {
    use std::io::Write as IoWrite;
    let path = project_root.join("CHARTER_HARDENING_HISTORY.md");
    let entry = format!("\n## R{round_number} — {date} (reviewer: {reviewer_id})\n\n{summary}\n");
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(&path)?;
    file.write_all(entry.as_bytes())?;
    Ok(())
}

// ── Plan rendering (P7) ────────────────────────────────────────────────────────

/// Renders a `PlannerContract` into a Plan document with all required sections.
///
/// Produces a structured markdown document matching the Plan Template in
/// `ARTIFACT_SPECIFICATIONS.md`.
#[must_use]
pub fn render_plan_doc(contract: &PlannerContract, date: &str) -> String {
    let mut out = String::new();

    writeln!(
        out,
        "# Anvil Plan — v{}\n\n\
         **Date:** {date}  \n\
         **Status:** Draft  \n\
         **Charter ref:** {}  \n\
         **Planner-Contract compliance:** all nine required per-phase fields validated.\n",
        contract.plan_version, contract.charter_ref
    )
    .ok();

    writeln!(out, "## Executive Summary\n").ok();
    writeln!(
        out,
        "This Plan decomposes the project into {} phase(s), derived from the approved Charter.\n",
        contract.phases.len()
    )
    .ok();

    writeln!(out, "## Phase Decomposition\n").ok();
    for phase in &contract.phases {
        render_phase_section(&mut out, phase);
    }

    render_phase_dep_graph_section(&mut out, &contract.phases);
    render_deferred_decision_section(&mut out, &contract.phases);

    writeln!(out, "## Plan-Level Acceptance Criteria\n").ok();
    for (i, phase) in contract.phases.iter().enumerate() {
        writeln!(
            out,
            "{}. {} ({}) ships per its acceptance criteria.",
            i + 1,
            phase.name,
            phase.phase_id
        )
        .ok();
    }
    writeln!(out).ok();

    writeln!(out, "## Bottom Line\n").ok();
    writeln!(
        out,
        "Plan v{} covers {} phase(s). All phases validated against the Planner Contract.",
        contract.plan_version,
        contract.phases.len()
    )
    .ok();

    out
}

fn render_phase_section(out: &mut String, phase: &crate::plan::PlannerPhase) {
    writeln!(out, "### {} — {}\n", phase.phase_id, phase.name).ok();
    writeln!(out, "- **Goal.** {}", phase.goal).ok();
    writeln!(out, "- **Deliverable.** {}", phase.deliverable).ok();
    writeln!(out, "- **Action list.**").ok();
    for action in &phase.action_list {
        writeln!(out, "  - {action}").ok();
    }
    writeln!(out, "- **Acceptance criteria.**").ok();
    for (i, ac) in phase.acceptance_criteria.iter().enumerate() {
        writeln!(out, "  {}. {ac}", i + 1).ok();
    }
    if phase.dependencies.is_empty() {
        writeln!(out, "- **Dependencies.** (none)").ok();
    } else {
        writeln!(out, "- **Dependencies.** {}", phase.dependencies.join(", ")).ok();
    }
    if phase.hinge_tests.is_empty() {
        writeln!(out, "- **Hinge-test list.** (none)").ok();
    } else {
        writeln!(out, "- **Hinge-test list.**").ok();
        for ht in &phase.hinge_tests {
            writeln!(out, "  - `{ht}`").ok();
        }
    }
    writeln!(
        out,
        "- **Evaluation-metric impact.** {}\n",
        phase.evaluation_metric_impact
    )
    .ok();
    if let Some(r) = phase.estimated_rounds {
        writeln!(out, "- **Estimated rounds-to-convergence.** {r}\n").ok();
    }
    writeln!(out, "---\n").ok();
}

fn render_phase_dep_graph_section(out: &mut String, phases: &[crate::plan::PlannerPhase]) {
    writeln!(out, "## Phase Dependency Graph\n").ok();
    for phase in phases {
        if phase.dependencies.is_empty() {
            writeln!(out, "- `{}` (no deps)", phase.phase_id).ok();
        } else {
            writeln!(
                out,
                "- `{}` → depends on: {}",
                phase.phase_id,
                phase.dependencies.join(", ")
            )
            .ok();
        }
    }
    writeln!(out).ok();
}

fn render_deferred_decision_section(out: &mut String, phases: &[crate::plan::PlannerPhase]) {
    writeln!(out, "## Deferred-Decision Registry\n").ok();
    let all_hinge_tests: Vec<(&str, &str)> = phases
        .iter()
        .flat_map(|p| {
            p.hinge_tests
                .iter()
                .map(|ht| (ht.as_str(), p.phase_id.as_str()))
        })
        .collect();
    if all_hinge_tests.is_empty() {
        writeln!(out, "(none)\n").ok();
    } else {
        writeln!(out, "| Test | Phase |").ok();
        writeln!(out, "|---|---|").ok();
        for (ht, pid) in &all_hinge_tests {
            writeln!(out, "| `{ht}` | {pid} |").ok();
        }
        writeln!(out).ok();
    }
}

/// Appends a round entry to `PLAN_HARDENING_HISTORY.md` in the project root (P7).
///
/// Creates the file if it does not exist.
///
/// # Errors
///
/// Returns [`AnvilError`] on I/O failure.
pub fn append_plan_hardening_history(
    project_root: &Path,
    round_number: u32,
    reviewer_id: &str,
    date: &str,
    summary: &str,
) -> Result<(), AnvilError> {
    use std::io::Write as IoWrite;
    let path = project_root.join("PLAN_HARDENING_HISTORY.md");
    let entry = format!("\n## R{round_number} — {date} (reviewer: {reviewer_id})\n\n{summary}\n");
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(&path)?;
    file.write_all(entry.as_bytes())?;
    Ok(())
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::pipeline::{
        CurationAction, DispositionLabel, Finding, FindingSeverity, LocationAnchor,
        VerificationOutcome, VerifiedFinding,
    };

    fn make_verified_finding(id: &str, claim: &str) -> VerifiedFinding {
        VerifiedFinding {
            finding: Finding {
                id: id.to_owned(),
                severity: FindingSeverity::P2,
                location: LocationAnchor {
                    artifact_path: "charter.md".to_owned(),
                    section_id: Some("Goals".to_owned()),
                    line_range: None,
                    symbol_name: None,
                    quote: None,
                },
                claim: claim.to_owned(),
                evidence: "See Goals section".to_owned(),
                recommendation: "Add more detail".to_owned(),
                metadata: None,
                advisory: false,
            },
            outcome: VerificationOutcome::Grounded,
            evidence_note: "Section 'Goals' found.".to_owned(),
        }
    }

    fn make_test_input<'a>(
        vfs: &'a [VerifiedFinding],
        dmap: &'a BTreeMap<String, DispositionLabel>,
        cmap: &'a BTreeMap<String, CurationAction>,
        adv: &'a BTreeMap<String, (AdvisoryDispositionType, Option<String>)>,
    ) -> DispositionInput<'a> {
        DispositionInput {
            artifact_name: "charter",
            round_number: 1,
            reviewer_id: "reviewer-1",
            date: "2026-05-26",
            verified_findings: vfs,
            disposition_map: dmap,
            files_changed: &[],
            narrative_summary: "Updated Goals section.",
            corrections: "",
            residual_notes: "",
            reproducibility_commands: "",
            bottom_line: "All findings resolved.",
            curation_actions: cmap,
            advisory_dispositions: adv,
        }
    }

    // hinge_test: pins=disposition_doc_required_sections, intended=disposition-schema, phase=P5
    #[test]
    fn test_disposition_doc_required_sections() {
        // Pins: rendered disposition docs always contain all required section headings
        // per the Artifact Specifications Disposition Document Template.
        // Sections 5 and 6 use concrete round numbers (R0 for R1 doc, etc.), not placeholders.
        // Flipping requires updating the spec AND this test together.
        let vf = make_verified_finding("F1", "Missing detail in Goals section");
        let mut dmap = BTreeMap::new();
        dmap.insert("F1".to_owned(), DispositionLabel::Fixed);
        let mut cmap = BTreeMap::new();
        cmap.insert("F1".to_owned(), CurationAction::Keep);
        let adv = BTreeMap::new();

        let doc = render_disposition_doc(&make_test_input(&[vf], &dmap, &cmap, &adv));

        assert!(
            doc.contains("## Verification of R1 Claims"),
            "missing Verification section"
        );
        assert!(
            doc.contains("## Disposition of R1 Findings"),
            "missing Disposition section"
        );
        // Round 1 → prior round is R0
        assert!(
            doc.contains("## Files Changed Since R0"),
            "missing Files Changed section"
        );
        assert!(
            doc.contains("## Corrections to R0 Narrative"),
            "missing Corrections section"
        );
        assert!(
            doc.contains("## Residual / Deferred"),
            "missing Residual section"
        );
        assert!(
            doc.contains("## Reproducibility"),
            "missing Reproducibility section"
        );
        assert!(
            doc.contains("## Bottom Line"),
            "missing Bottom Line section"
        );
    }

    #[test]
    fn test_disposition_doc_round2_headings() {
        // Round 2 → prior round is R1; sections must not say R0 or R<N-1>.
        let vf = make_verified_finding("F1", "claim");
        let mut dmap = BTreeMap::new();
        dmap.insert("F1".to_owned(), DispositionLabel::Fixed);
        let mut cmap = BTreeMap::new();
        cmap.insert("F1".to_owned(), CurationAction::Keep);
        let adv = BTreeMap::new();

        let vfs = [vf];
        let input = DispositionInput {
            round_number: 2,
            narrative_summary: "Round 2 changes.",
            corrections: "Prior narrative had a typo.",
            ..make_test_input(&vfs, &dmap, &cmap, &adv)
        };
        let doc = render_disposition_doc(&input);
        assert!(
            doc.contains("## Files Changed Since R1"),
            "R2 doc must reference R1"
        );
        assert!(
            doc.contains("## Corrections to R1 Narrative"),
            "R2 doc must reference R1"
        );
        assert!(
            !doc.contains("R<N-1>"),
            "no placeholder text in rendered doc"
        );
        assert!(
            doc.contains("Prior narrative had a typo."),
            "corrections text must appear"
        );
    }

    #[test]
    fn test_render_charter_md_required_sections() {
        use chrono::Utc;
        let packet = CharterPacket {
            title: "Test Charter".to_owned(),
            produced_at: Utc::now(),
            goals: vec!["Build X".to_owned(), "Ship Y".to_owned()],
            scope: "Everything needed to build X and Y.".to_owned(),
            out_of_scope: vec!["Z".to_owned()],
            required_choices: vec!["language".to_owned()],
            success_criteria: vec!["X ships".to_owned()],
            stakeholders: vec!["Alice".to_owned()],
            additional_notes: None,
        };
        let md = render_charter_md(&packet);
        assert!(md.contains("# Test Charter"));
        assert!(md.contains("## Goals"));
        assert!(md.contains("## Scope"));
        assert!(md.contains("## Success Criteria"));
        assert!(md.contains("- Build X"));
        assert!(md.contains("1. X ships"));
    }

    #[test]
    fn test_render_plan_doc_required_sections() {
        use crate::plan::{PlannerContract, PlannerPhase};
        let phase = PlannerPhase {
            phase_id: "P0".to_owned(),
            name: "Bootstrap".to_owned(),
            goal: "Initialize the workspace.".to_owned(),
            action_list: vec!["Create dirs.".to_owned()],
            deliverable: "Scaffold.".to_owned(),
            acceptance_criteria: vec!["anvil init succeeds.".to_owned()],
            dependencies: vec![],
            hinge_tests: vec!["test_init_idempotent".to_owned()],
            evaluation_metric_impact: "None.".to_owned(),
            estimated_rounds: Some(1),
        };
        let contract = PlannerContract {
            plan_version: "1.0.0".to_owned(),
            charter_ref: "charter.md:v1".to_owned(),
            phases: vec![phase],
        };
        let doc = render_plan_doc(&contract, "2026-05-26");
        assert!(
            doc.contains("## Executive Summary"),
            "must have Executive Summary"
        );
        assert!(
            doc.contains("## Phase Decomposition"),
            "must have Phase Decomposition"
        );
        assert!(
            doc.contains("## Phase Dependency Graph"),
            "must have Phase Dependency Graph"
        );
        assert!(
            doc.contains("## Plan-Level Acceptance Criteria"),
            "must have Acceptance Criteria"
        );
        assert!(doc.contains("### P0 —"), "must include phase section");
        assert!(doc.contains("Bootstrap"), "must include phase name");
    }

    #[test]
    fn test_append_plan_hardening_history() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path();
        std::fs::write(root.join("PLAN_HARDENING_HISTORY.md"), b"").unwrap();

        append_plan_hardening_history(root, 1, "reviewer-1", "2026-05-26", "Fixed P0 criteria.")
            .expect("append should succeed");

        let content = std::fs::read_to_string(root.join("PLAN_HARDENING_HISTORY.md")).unwrap();
        assert!(content.contains("## R1 — 2026-05-26"));
        assert!(content.contains("Fixed P0 criteria."));
    }

    #[test]
    fn test_append_charter_hardening_history() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path();
        std::fs::write(root.join("CHARTER_HARDENING_HISTORY.md"), b"").unwrap();

        append_charter_hardening_history(root, 1, "reviewer-1", "2026-05-26", "Fixed Goals.")
            .expect("append should succeed");

        let content = std::fs::read_to_string(root.join("CHARTER_HARDENING_HISTORY.md")).unwrap();
        assert!(content.contains("## R1 — 2026-05-26"));
        assert!(content.contains("Fixed Goals."));

        append_charter_hardening_history(root, 2, "reviewer-1", "2026-05-27", "Second round.")
            .expect("second append");
        let content2 = std::fs::read_to_string(root.join("CHARTER_HARDENING_HISTORY.md")).unwrap();
        assert!(content2.contains("## R1 — 2026-05-26"));
        assert!(content2.contains("## R2 — 2026-05-27"));
    }
}
