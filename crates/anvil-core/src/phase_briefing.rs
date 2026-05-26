//! Phase Review Briefing types and rendering (P8).
//!
//! Defines `PhaseBriefingContract` — the typed JSON the Coder produces inside
//! `<phase_briefing>` tags — plus validation and markdown rendering per the
//! Artifact Specifications §Phase Review Briefing Template.

use std::fmt::Write as _;

use serde::{Deserialize, Serialize};

use crate::error::AnvilError;

/// A file entry for the "What Was Built" table of a Phase Review Briefing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BriefingFileChange {
    pub file: String,
    /// One of: CREATE, MODIFY, DELETE.
    pub action: String,
    pub purpose: String,
    /// Approximate line delta (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lines: Option<i32>,
}

/// A row for the "Architecture Compliance" table of a Phase Review Briefing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BriefingComplianceItem {
    pub invariant: String,
    pub evidence: String,
}

/// A row for the "Test Coverage Summary" table of a Phase Review Briefing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BriefingTestArea {
    pub area: String,
    pub tests_added: String,
    pub coverage_status: String,
}

/// Status vocabulary for `PhaseBriefingContract`, per Artifact Specifications Standard Vocabularies.
///
/// The serde rename values match the vocabulary labels defined in the Artifact Specifications.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum BriefingStatus {
    #[serde(rename = "Draft")]
    #[default]
    Draft,
    #[serde(rename = "Awaiting Review")]
    AwaitingReview,
    #[serde(rename = "In Revision")]
    InRevision,
    #[serde(rename = "Convergent")]
    Convergent,
    #[serde(rename = "Approved")]
    Approved,
    #[serde(rename = "Superseded")]
    Superseded,
}

impl BriefingStatus {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "Draft",
            Self::AwaitingReview => "Awaiting Review",
            Self::InRevision => "In Revision",
            Self::Convergent => "Convergent",
            Self::Approved => "Approved",
            Self::Superseded => "Superseded",
        }
    }
}

/// The typed JSON contract the Coder produces inside `<phase_briefing>` tags.
///
/// Maps to the 7 required sections of the Phase Review Briefing Template
/// (Artifact Specifications §Phase Review Briefing Template).
///
/// All section fields carry `#[serde(default)]` so that absent JSON fields default to
/// empty / default values and are caught with precise `PhaseBriefingMissingSection`
/// errors by `validate_phase_briefing_contract`, rather than opaque `ModelResponseBadJson`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseBriefingContract {
    /// Phase ID (e.g. `"P8"`). Required — absence produces `ModelResponseBadJson`.
    pub phase_id: String,
    /// One-line description of what this briefing covers (section 1 — Header block).
    #[serde(default)]
    pub scope: String,
    /// Which Plan section is being implemented (e.g. `"§P8"`).
    #[serde(default)]
    pub spec_section: String,
    /// Document status per Artifact Specifications Standard Vocabularies.
    #[serde(default)]
    pub status: BriefingStatus,
    /// What Was Built — file-level changes (section 2).
    #[serde(default)]
    pub files_changed: Vec<BriefingFileChange>,
    /// Architecture Compliance — invariant → evidence mapping (section 3).
    #[serde(default)]
    pub compliance_items: Vec<BriefingComplianceItem>,
    /// What to Review — numbered questions for the reviewer (section 4).
    #[serde(default)]
    pub what_to_review: Vec<String>,
    /// Test Coverage Summary — per-area coverage table (section 5).
    #[serde(default)]
    pub test_areas: Vec<BriefingTestArea>,
    /// How to Activate for Testing — runbook instructions (section 6).
    #[serde(default)]
    pub how_to_activate: String,
    /// Next Phase — preview of what ships after this phase (section 7).
    #[serde(default)]
    pub next_phase: String,
}

/// The 7 required section keys for a `PhaseBriefingContract`.
///
/// Validation asserts that none of the mandatory string fields are empty and
/// that all list fields contain at least one entry.
pub const REQUIRED_BRIEFING_SECTIONS: [&str; 7] = [
    "scope",            // 1 — Header block
    "files_changed",    // 2 — What Was Built
    "compliance_items", // 3 — Architecture Compliance
    "what_to_review",   // 4 — What to Review
    "test_areas",       // 5 — Test Coverage Summary
    "how_to_activate",  // 6 — How to Activate for Testing
    "next_phase",       // 7 — Next Phase
];

/// Validates that all 7 required sections of a `PhaseBriefingContract` are present and non-empty.
///
/// Because all section fields carry `#[serde(default)]`, this function is the single gate
/// for section completeness — both absent-from-JSON and present-but-empty cases are caught here.
///
/// # Errors
///
/// Returns [`AnvilError::PhaseBriefingMissingSection`] for the first missing or empty section.
pub fn validate_phase_briefing_contract(
    contract: &PhaseBriefingContract,
) -> Result<(), AnvilError> {
    if contract.scope.trim().is_empty() {
        return Err(AnvilError::PhaseBriefingMissingSection {
            phase_id: contract.phase_id.clone(),
            section: "scope",
        });
    }
    if contract.files_changed.is_empty() {
        return Err(AnvilError::PhaseBriefingMissingSection {
            phase_id: contract.phase_id.clone(),
            section: "files_changed",
        });
    }
    if contract.compliance_items.is_empty() {
        return Err(AnvilError::PhaseBriefingMissingSection {
            phase_id: contract.phase_id.clone(),
            section: "compliance_items",
        });
    }
    if contract.what_to_review.is_empty() {
        return Err(AnvilError::PhaseBriefingMissingSection {
            phase_id: contract.phase_id.clone(),
            section: "what_to_review",
        });
    }
    if contract.test_areas.is_empty() {
        return Err(AnvilError::PhaseBriefingMissingSection {
            phase_id: contract.phase_id.clone(),
            section: "test_areas",
        });
    }
    if contract.how_to_activate.trim().is_empty() {
        return Err(AnvilError::PhaseBriefingMissingSection {
            phase_id: contract.phase_id.clone(),
            section: "how_to_activate",
        });
    }
    if contract.next_phase.trim().is_empty() {
        return Err(AnvilError::PhaseBriefingMissingSection {
            phase_id: contract.phase_id.clone(),
            section: "next_phase",
        });
    }
    Ok(())
}

/// Extracts a `PhaseBriefingContract` from a model response containing
/// `<phase_briefing>...</phase_briefing>` tags.
///
/// Because section fields carry `#[serde(default)]`, absent fields produce empty values
/// rather than `ModelResponseBadJson`; call `validate_phase_briefing_contract` after this
/// to enforce section completeness.
///
/// # Errors
///
/// Returns [`AnvilError::ModelResponseMissingPacket`] if the tags are absent,
/// or [`AnvilError::ModelResponseBadJson`] if the JSON is structurally invalid.
pub fn parse_phase_briefing_contract(response: &str) -> Result<PhaseBriefingContract, AnvilError> {
    let start_tag = "<phase_briefing>";
    let end_tag = "</phase_briefing>";
    let start = response
        .find(start_tag)
        .ok_or_else(|| AnvilError::ModelResponseMissingPacket("phase_briefing".to_owned()))?;
    let end = response
        .find(end_tag)
        .ok_or_else(|| AnvilError::ModelResponseMissingPacket("phase_briefing".to_owned()))?;
    let json = response[start + start_tag.len()..end].trim();
    serde_json::from_str(json).map_err(|e| AnvilError::ModelResponseBadJson {
        reason: e.to_string(),
    })
}

/// Extracts the `<phase_disposition>` markdown block from a model response.
///
/// Returns `None` if the tags are not present in the response.
#[must_use]
pub fn extract_phase_disposition_md(response: &str) -> Option<String> {
    let start_tag = "<phase_disposition>";
    let end_tag = "</phase_disposition>";
    let start = response.find(start_tag)?;
    let end = response.find(end_tag)?;
    Some(response[start + start_tag.len()..end].trim().to_owned())
}

/// Renders a `PhaseBriefingContract` into a Phase Review Briefing markdown document.
///
/// All 7 required sections are always present per the Artifact Specifications template.
#[must_use]
pub fn render_phase_briefing_doc(
    contract: &PhaseBriefingContract,
    date: &str,
    round: u32,
) -> String {
    let mut out = String::new();

    // 1. Header block
    writeln!(
        out,
        "# Phase Review Briefing — {} R{round}\n\n\
         **Date:** {date}  \n\
         **Phase:** {}  \n\
         **Scope:** {}  \n\
         **Spec Section:** {}  \n\
         **Status:** {}\n\n---\n",
        contract.phase_id,
        contract.phase_id,
        contract.scope,
        contract.spec_section,
        contract.status.as_str()
    )
    .ok();

    // 2. What Was Built
    out.push_str("## What Was Built\n\n");
    out.push_str("| File | Action | Purpose | Lines |\n");
    out.push_str("|---|---|---|---|\n");
    for fc in &contract.files_changed {
        let lines = fc.lines.map_or_else(|| "—".to_owned(), |n| n.to_string());
        writeln!(
            out,
            "| `{}` | {} | {} | {} |",
            fc.file,
            fc.action,
            escape_md_table(&fc.purpose),
            lines
        )
        .ok();
    }
    out.push('\n');

    // 3. Architecture Compliance
    out.push_str("## Architecture Compliance\n\n");
    out.push_str("| Invariant | Evidence |\n");
    out.push_str("|---|---|\n");
    for item in &contract.compliance_items {
        writeln!(
            out,
            "| {} | {} |",
            escape_md_table(&item.invariant),
            escape_md_table(&item.evidence)
        )
        .ok();
    }
    out.push('\n');

    // 4. What to Review
    out.push_str("## What to Review\n\n");
    for (i, q) in contract.what_to_review.iter().enumerate() {
        writeln!(out, "{}. {q}", i + 1).ok();
    }
    out.push('\n');

    // 5. Test Coverage Summary
    out.push_str("## Test Coverage Summary\n\n");
    out.push_str("| Area | Tests Added | Coverage Status |\n");
    out.push_str("|---|---|---|\n");
    for ta in &contract.test_areas {
        writeln!(
            out,
            "| {} | {} | {} |",
            escape_md_table(&ta.area),
            escape_md_table(&ta.tests_added),
            escape_md_table(&ta.coverage_status)
        )
        .ok();
    }
    out.push('\n');

    // 6. How to Activate for Testing
    out.push_str("## How to Activate for Testing\n\n");
    out.push_str(&contract.how_to_activate);
    out.push_str("\n\n");

    // 7. Next Phase
    out.push_str("## Next Phase\n\n");
    out.push_str(&contract.next_phase);
    out.push('\n');

    out
}

fn escape_md_table(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_contract() -> PhaseBriefingContract {
        PhaseBriefingContract {
            phase_id: "P8".to_owned(),
            scope: "Build Stage Pipeline".to_owned(),
            spec_section: "§P8".to_owned(),
            status: BriefingStatus::AwaitingReview,
            files_changed: vec![BriefingFileChange {
                file: "crates/anvil-cli/src/phase.rs".to_owned(),
                action: "CREATE".to_owned(),
                purpose: "Phase build/review/ship commands".to_owned(),
                lines: Some(200),
            }],
            compliance_items: vec![BriefingComplianceItem {
                invariant: "Gate records written before file ops".to_owned(),
                evidence: "store.append called before fs::write in run_phase_build".to_owned(),
            }],
            what_to_review: vec!["Does run_phase_ship correctly check termination?".to_owned()],
            test_areas: vec![BriefingTestArea {
                area: "Phase ship termination".to_owned(),
                tests_added: "test_phase_cannot_ship_without_termination".to_owned(),
                coverage_status: "Covered".to_owned(),
            }],
            how_to_activate: "cargo test --workspace".to_owned(),
            next_phase: "P9 — Ship + Rollback".to_owned(),
        }
    }

    // hinge_test: pins=7_briefing_sections, intended=briefing_validation, phase=P8
    #[test]
    fn test_phase_briefing_required_sections() {
        // Pins: validate_phase_briefing_contract rejects contracts missing any of the
        // 7 required sections. Flipping requires updating REQUIRED_BRIEFING_SECTIONS
        // and this test together.
        let valid = minimal_contract();
        assert!(
            validate_phase_briefing_contract(&valid).is_ok(),
            "valid contract must pass validation"
        );

        let mut c = minimal_contract();
        c.scope = String::new();
        assert!(
            matches!(
                validate_phase_briefing_contract(&c),
                Err(AnvilError::PhaseBriefingMissingSection {
                    section: "scope",
                    ..
                })
            ),
            "empty scope must be rejected"
        );

        let mut c = minimal_contract();
        c.files_changed.clear();
        assert!(
            matches!(
                validate_phase_briefing_contract(&c),
                Err(AnvilError::PhaseBriefingMissingSection {
                    section: "files_changed",
                    ..
                })
            ),
            "empty files_changed must be rejected"
        );

        let mut c = minimal_contract();
        c.compliance_items.clear();
        assert!(
            matches!(
                validate_phase_briefing_contract(&c),
                Err(AnvilError::PhaseBriefingMissingSection {
                    section: "compliance_items",
                    ..
                })
            ),
            "empty compliance_items must be rejected"
        );

        let mut c = minimal_contract();
        c.what_to_review.clear();
        assert!(
            matches!(
                validate_phase_briefing_contract(&c),
                Err(AnvilError::PhaseBriefingMissingSection {
                    section: "what_to_review",
                    ..
                })
            ),
            "empty what_to_review must be rejected"
        );

        let mut c = minimal_contract();
        c.test_areas.clear();
        assert!(
            matches!(
                validate_phase_briefing_contract(&c),
                Err(AnvilError::PhaseBriefingMissingSection {
                    section: "test_areas",
                    ..
                })
            ),
            "empty test_areas must be rejected"
        );

        let mut c = minimal_contract();
        c.how_to_activate = String::new();
        assert!(
            matches!(
                validate_phase_briefing_contract(&c),
                Err(AnvilError::PhaseBriefingMissingSection {
                    section: "how_to_activate",
                    ..
                })
            ),
            "empty how_to_activate must be rejected"
        );

        let mut c = minimal_contract();
        c.next_phase = String::new();
        assert!(
            matches!(
                validate_phase_briefing_contract(&c),
                Err(AnvilError::PhaseBriefingMissingSection {
                    section: "next_phase",
                    ..
                })
            ),
            "empty next_phase must be rejected"
        );
    }

    #[test]
    fn test_missing_section_field_produces_section_error_not_json_error() {
        // Regression: absent JSON fields (not just empty strings) must produce
        // PhaseBriefingMissingSection via validate, not opaque ModelResponseBadJson.
        let json_missing_scope = r#"{"phase_id":"P8","files_changed":[{"file":"f","action":"CREATE","purpose":"p"}],"compliance_items":[{"invariant":"i","evidence":"e"}],"what_to_review":["q"],"test_areas":[{"area":"a","tests_added":"t","coverage_status":"c"}],"how_to_activate":"h","next_phase":"n"}"#;
        let wrapped = format!("<phase_briefing>{json_missing_scope}</phase_briefing>");
        let contract = parse_phase_briefing_contract(&wrapped)
            .expect("parse must succeed (scope defaults to empty string)");
        let err = validate_phase_briefing_contract(&contract)
            .expect_err("validation must fail with missing scope");
        assert!(
            matches!(
                err,
                AnvilError::PhaseBriefingMissingSection {
                    section: "scope",
                    ..
                }
            ),
            "absent scope field must produce PhaseBriefingMissingSection, got: {err}"
        );
    }

    #[test]
    fn test_invalid_status_value_produces_json_error() {
        // BriefingStatus is an enum; unknown values must fail deserialization.
        let json = r#"{"phase_id":"P8","status":"INVALID_STATUS","scope":"s","files_changed":[{"file":"f","action":"CREATE","purpose":"p"}],"compliance_items":[{"invariant":"i","evidence":"e"}],"what_to_review":["q"],"test_areas":[{"area":"a","tests_added":"t","coverage_status":"c"}],"how_to_activate":"h","next_phase":"n"}"#;
        let wrapped = format!("<phase_briefing>{json}</phase_briefing>");
        let result = parse_phase_briefing_contract(&wrapped);
        assert!(
            matches!(result, Err(AnvilError::ModelResponseBadJson { .. })),
            "invalid status must produce ModelResponseBadJson, got: {result:?}"
        );
    }

    #[test]
    fn test_briefing_status_roundtrip() {
        for status in [
            BriefingStatus::Draft,
            BriefingStatus::AwaitingReview,
            BriefingStatus::InRevision,
            BriefingStatus::Convergent,
            BriefingStatus::Approved,
            BriefingStatus::Superseded,
        ] {
            let serialized = serde_json::to_string(&status).unwrap();
            let deserialized: BriefingStatus = serde_json::from_str(&serialized).unwrap();
            assert_eq!(status, deserialized);
            assert!(!status.as_str().is_empty());
        }
    }

    #[test]
    fn test_parse_phase_briefing_contract_roundtrip() {
        let contract = minimal_contract();
        let json = serde_json::to_string(&contract).unwrap();
        let wrapped = format!("<phase_briefing>\n{json}\n</phase_briefing>");
        let parsed = parse_phase_briefing_contract(&wrapped).unwrap();
        assert_eq!(parsed.phase_id, "P8");
        assert_eq!(parsed.scope, "Build Stage Pipeline");
        assert_eq!(parsed.status, BriefingStatus::AwaitingReview);
    }

    #[test]
    fn test_parse_phase_briefing_contract_missing_tags() {
        let result = parse_phase_briefing_contract("no tags here");
        assert!(matches!(
            result,
            Err(AnvilError::ModelResponseMissingPacket(_))
        ));
    }

    #[test]
    fn test_extract_phase_disposition_md_present() {
        let resp =
            "some text\n<phase_disposition>\n## Disposition\nfoo\n</phase_disposition>\nmore";
        let extracted = extract_phase_disposition_md(resp).unwrap();
        assert!(extracted.contains("## Disposition"));
    }

    #[test]
    fn test_extract_phase_disposition_md_absent() {
        assert!(extract_phase_disposition_md("no disposition here").is_none());
    }

    #[test]
    fn test_render_phase_briefing_doc_has_all_sections() {
        let contract = minimal_contract();
        let doc = render_phase_briefing_doc(&contract, "2026-05-26", 1);
        assert!(doc.contains("## What Was Built"));
        assert!(doc.contains("## Architecture Compliance"));
        assert!(doc.contains("## What to Review"));
        assert!(doc.contains("## Test Coverage Summary"));
        assert!(doc.contains("## How to Activate for Testing"));
        assert!(doc.contains("## Next Phase"));
        assert!(doc.contains("Awaiting Review"));
    }
}
