//! Charter Stage Pipeline domain types (P5/P6).
//!
//! Defines the typed schemas for Findings Packets, Verification outcomes,
//! and Curation records that flow through the Charter review cycle.
//! These types conform to the Artifact Specifications v1.0.0.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ── Severity Tiers ─────────────────────────────────────────────────────────────

/// Severity tier assigned by the reviewer (per Artifact Specifications Standard Vocabularies).
/// P1 blocks ship in all rounds; P2 blocks in rounds 1–5; P3 is advisory in all rounds.
/// (Advisory flags are set by `apply_severity_tiering`, not by the reviewer model.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum FindingSeverity {
    P1,
    P2,
    P3,
}

impl FindingSeverity {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::P1 => "P1",
            Self::P2 => "P2",
            Self::P3 => "P3",
        }
    }

    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "P1" => Some(Self::P1),
            "P2" => Some(Self::P2),
            "P3" => Some(Self::P3),
            _ => None,
        }
    }
}

// ── Location Anchor ────────────────────────────────────────────────────────────

/// Points to the exact location in an artifact that a finding concerns.
/// At least one of `section_id`, `line_range`, or `symbol_name` must be populated
/// for the finding to be considered anchored.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationAnchor {
    pub artifact_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub section_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_range: Option<[u32; 2]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quote: Option<String>,
}

impl LocationAnchor {
    /// Returns `true` if at least one anchoring field is populated.
    #[must_use]
    pub fn is_anchored(&self) -> bool {
        self.section_id.is_some() || self.line_range.is_some() || self.symbol_name.is_some()
    }
}

// ── Finding ────────────────────────────────────────────────────────────────────

/// Optional metadata carried by a Finding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FindingMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub related_finding_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proposed_severity_floor: Option<FindingSeverity>,
}

/// A single reviewer finding per the Artifact Specifications Finding schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    /// Local to the packet (e.g., "F1", "F2"); becomes global on persistence.
    pub id: String,
    pub severity: FindingSeverity,
    pub location: LocationAnchor,
    /// One-sentence statement of the issue.
    pub claim: String,
    /// Citation of the artifact text or code that supports the claim.
    pub evidence: String,
    /// Proposed resolution or direction.
    pub recommendation: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<FindingMetadata>,
    /// Set by the system (not the reviewer model) when the round count exceeds
    /// `ADVISORY_THRESHOLD_ROUND` and the severity is P2 or P3.
    /// Advisory findings must receive an explicit `AdvisoryDispositionType` during
    /// curation but do not block the full-pool clean convergence check.
    #[serde(default)]
    pub advisory: bool,
}

// ── Reviewer meta ──────────────────────────────────────────────────────────────

/// Optional metadata the reviewer may attach to a packet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewerMeta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub review_duration_seconds: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

// ── Findings Packet ────────────────────────────────────────────────────────────

/// Structured packet a reviewer produces during a review round.
/// Conforms to the Artifact Specifications `FindingsPacket` schema (v1.0.0).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FindingsPacket {
    pub packet_id: String,
    pub artifact_ref: String,
    pub round_number: u32,
    pub reviewer_id: String,
    pub reviewer_model_identity: String,
    pub produced_at: DateTime<Utc>,
    pub findings: Vec<Finding>,
    /// SHA-256 hex digest of the reviewed artifact content at the time of review (P6 R2).
    /// Used by the full-pool clean check to verify all reviewers reviewed the same state.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reviewer_meta: Option<ReviewerMeta>,
}

impl FindingsPacket {
    /// Constructs a new packet with a generated UUID and current timestamp.
    #[must_use]
    pub fn new(
        artifact_ref: String,
        round_number: u32,
        reviewer_id: String,
        reviewer_model_identity: String,
        findings: Vec<Finding>,
    ) -> Self {
        Self {
            packet_id: uuid::Uuid::new_v4().to_string(),
            artifact_ref,
            round_number,
            reviewer_id,
            reviewer_model_identity,
            produced_at: Utc::now(),
            findings,
            artifact_hash: None,
            reviewer_meta: None,
        }
    }
}

// ── Charter Packet ─────────────────────────────────────────────────────────────

/// Structured output from an Interlocutor discussion session.
/// This is the authoritative input to Charter rendering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharterPacket {
    pub title: String,
    pub produced_at: DateTime<Utc>,
    pub goals: Vec<String>,
    pub scope: String,
    pub out_of_scope: Vec<String>,
    pub required_choices: Vec<String>,
    pub success_criteria: Vec<String>,
    pub stakeholders: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_notes: Option<String>,
}

/// Model-supplied fields only — `produced_at` is generated locally after parsing.
#[derive(serde::Deserialize)]
struct PartialCharterPacket {
    title: String,
    #[serde(default)]
    goals: Vec<String>,
    #[serde(default)]
    scope: String,
    #[serde(default)]
    out_of_scope: Vec<String>,
    #[serde(default)]
    required_choices: Vec<String>,
    #[serde(default)]
    success_criteria: Vec<String>,
    #[serde(default)]
    stakeholders: Vec<String>,
    additional_notes: Option<String>,
}

impl CharterPacket {
    /// Required top-level fields; used by `test_charter_packet_required_fields`.
    pub const REQUIRED_FIELDS: &'static [&'static str] =
        &["title", "goals", "scope", "success_criteria"];

    /// Parses model-supplied JSON into a `CharterPacket`, filling `produced_at` locally.
    ///
    /// The model prompt does not ask for `produced_at`; this constructor handles that gap.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::AnvilError::ModelResponseBadJson`] if the JSON is malformed.
    pub fn from_model_json(json: &str) -> Result<Self, crate::error::AnvilError> {
        let partial: PartialCharterPacket = serde_json::from_str(json).map_err(|e| {
            crate::error::AnvilError::ModelResponseBadJson {
                reason: e.to_string(),
            }
        })?;
        Ok(Self {
            title: partial.title,
            produced_at: Utc::now(),
            goals: partial.goals,
            scope: partial.scope,
            out_of_scope: partial.out_of_scope,
            required_choices: partial.required_choices,
            success_criteria: partial.success_criteria,
            stakeholders: partial.stakeholders,
            additional_notes: partial.additional_notes,
        })
    }

    /// Returns `Ok(())` if all required fields are non-empty.
    ///
    /// # Errors
    ///
    /// Returns a string naming the first missing or empty required field.
    pub fn validate(&self) -> Result<(), String> {
        if self.title.trim().is_empty() {
            return Err("title".to_owned());
        }
        if self.goals.is_empty() {
            return Err("goals".to_owned());
        }
        if self.scope.trim().is_empty() {
            return Err("scope".to_owned());
        }
        if self.success_criteria.is_empty() {
            return Err("success_criteria".to_owned());
        }
        Ok(())
    }
}

// ── Verification ───────────────────────────────────────────────────────────────

/// Outcome of the Finding Verifier for a single finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerificationOutcome {
    /// The artifact contains the cited content at the cited location.
    Grounded,
    /// The artifact does not match the cited evidence.
    Refuted,
    /// No anchor provided, or the artifact file could not be read.
    CannotBeVerified,
}

impl VerificationOutcome {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Grounded => "Grounded",
            Self::Refuted => "Refuted",
            Self::CannotBeVerified => "CannotBeVerified",
        }
    }
}

/// A finding paired with its verification outcome and a human-readable evidence note.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifiedFinding {
    pub finding: Finding,
    pub outcome: VerificationOutcome,
    /// Human-readable note from the verifier explaining the outcome.
    pub evidence_note: String,
}

// ── Curation ───────────────────────────────────────────────────────────────────

/// The action the Coordinator takes on a single finding during curation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CurationAction {
    Keep,
    Drop,
    /// Reserved for future use; not available in the P5/P6 interactive CLI.
    Edit,
    Annotate,
}

impl CurationAction {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Keep => "keep",
            Self::Drop => "drop",
            Self::Edit => "edit",
            Self::Annotate => "annotate",
        }
    }
}

/// Explicit disposition type for advisory findings (P6).
///
/// Each advisory finding must receive one of these to pass the convergence gate check.
/// `Drop-Advisory` and `Defer-Advisory` carry additional context in the disposition's
/// `annotation` field (reason text or target phase, respectively).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AdvisoryDispositionType {
    /// Acknowledged; no action required; finding recorded in the disposition.
    AcceptAdvisory,
    /// Finding refuted or non-applicable; requires reason (stored in `annotation`).
    DropAdvisory,
    /// Deferred to a named future phase; requires target phase (stored in `annotation`).
    DeferAdvisory,
}

impl AdvisoryDispositionType {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AcceptAdvisory => "accept-advisory",
            Self::DropAdvisory => "drop-advisory",
            Self::DeferAdvisory => "defer-advisory",
        }
    }
}

/// Per-finding curation decision per the Artifact Specifications `CurationDisposition` schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurationDisposition {
    /// References `Finding.id` in the original packet.
    pub finding_id: String,
    pub action: CurationAction,
    /// Present only if `action == Edit`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edited_finding: Option<Finding>,
    /// Present only if `action == Annotate` or `action == Drop`, or for advisory findings
    /// where `advisory_disposition` is `DropAdvisory` (reason) or `DeferAdvisory` (target phase).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotation: Option<String>,
    /// Required for advisory findings (P6). `None` for non-advisory findings.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub advisory_disposition: Option<AdvisoryDispositionType>,
}

// ── Disposition Label ──────────────────────────────────────────────────────────

/// The label the Coder assigns to each finding in a Disposition document.
/// Uses the exhaustive vocabulary from the Artifact Specifications.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DispositionLabel {
    Fixed,
    LockedPendingPlan,
    Refuted,
    Deferred,
}

impl DispositionLabel {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Fixed => "Fixed",
            Self::LockedPendingPlan => "Locked in Charter, enforcement pending Plan",
            Self::Refuted => "Refuted",
            Self::Deferred => "Deferred",
        }
    }
}

// ── Severity tiering (P6) ─────────────────────────────────────────────────────

/// Marks findings as advisory per the Artifact Specifications severity vocabulary:
/// - P3 is advisory in **all** rounds.
/// - P2 is advisory in rounds 6+ (after `ADVISORY_THRESHOLD_ROUND`).
/// - P1 is never advisory.
///
/// Called immediately after the model response is parsed, before the packet is stored.
/// The `advisory` flag is set by the system, not the reviewer model.
pub fn apply_severity_tiering(packet: &mut FindingsPacket, round_count: u32) {
    let is_advisory_round = crate::rotation::is_advisory_round(round_count);
    for finding in &mut packet.findings {
        match finding.severity {
            FindingSeverity::P3 => finding.advisory = true,
            FindingSeverity::P2 if is_advisory_round => finding.advisory = true,
            _ => {}
        }
    }
}

/// Returns the IDs of advisory findings that fail the advisory gate:
/// - missing an `advisory_disposition`, OR
/// - `DropAdvisory` / `DeferAdvisory` with an empty annotation (reason / target phase required).
///
/// Used as a pre-commit gate in `anvil charter findings`: if the returned list is
/// non-empty, the coordinator must provide complete dispositions before the record is stored.
#[must_use]
pub fn check_advisory_gate(
    dispositions: &[CurationDisposition],
    findings: &[Finding],
) -> Vec<String> {
    findings
        .iter()
        .filter(|f| f.advisory)
        .filter_map(|f| {
            let d = dispositions.iter().find(|d| d.finding_id == f.id);
            match d {
                None => Some(f.id.clone()),
                Some(d) => match &d.advisory_disposition {
                    None => Some(f.id.clone()),
                    Some(
                        AdvisoryDispositionType::DropAdvisory
                        | AdvisoryDispositionType::DeferAdvisory,
                    ) => {
                        if d.annotation.as_deref().unwrap_or("").trim().is_empty() {
                            Some(f.id.clone())
                        } else {
                            None
                        }
                    }
                    Some(AdvisoryDispositionType::AcceptAdvisory) => None,
                },
            }
        })
        .collect()
}

// ── Verifier ───────────────────────────────────────────────────────────────────

/// Grounds each finding against the actual artifact files on disk.
///
/// Grounding rules:
/// - No anchor → `CannotBeVerified`.
/// - `quote` present → search for quote in the artifact file → `Grounded` / `Refuted`.
/// - `section_id` present (no quote) → scan lines for a heading with that text → `Grounded` / `CannotBeVerified`.
/// - `symbol_name` present (no quote, no `section_id`) → search file for the symbol name → `Grounded` / `CannotBeVerified`.
/// - `line_range` only → bounds check; no text content verified → `CannotBeVerified` if in bounds, `Refuted` if out of bounds.
/// - Anchor present but file unreadable → `CannotBeVerified`.
#[must_use]
pub fn verify_findings(
    findings: &[Finding],
    project_root: &std::path::Path,
) -> Vec<VerifiedFinding> {
    findings
        .iter()
        .map(|f| verify_one(f, project_root))
        .collect()
}

fn verify_one(finding: &Finding, project_root: &std::path::Path) -> VerifiedFinding {
    let loc = &finding.location;

    if !loc.is_anchored() && loc.quote.is_none() {
        return VerifiedFinding {
            finding: finding.clone(),
            outcome: VerificationOutcome::CannotBeVerified,
            evidence_note: "No location anchor provided.".to_owned(),
        };
    }

    let artifact_path = project_root.join(&loc.artifact_path);
    let Ok(content) = std::fs::read_to_string(&artifact_path) else {
        return VerifiedFinding {
            finding: finding.clone(),
            outcome: VerificationOutcome::CannotBeVerified,
            evidence_note: format!("Artifact '{}' could not be read.", loc.artifact_path),
        };
    };

    // Quote grounding (highest confidence).
    if let Some(ref quote) = loc.quote {
        if content.contains(quote.as_str()) {
            return VerifiedFinding {
                finding: finding.clone(),
                outcome: VerificationOutcome::Grounded,
                evidence_note: format!("Quote found in '{}'.", loc.artifact_path),
            };
        }
        return VerifiedFinding {
            finding: finding.clone(),
            outcome: VerificationOutcome::Refuted,
            evidence_note: format!(
                "Quote not found in '{}': {:?}",
                loc.artifact_path,
                truncate(quote, 80)
            ),
        };
    }

    // Section-ID grounding — matches any ATX heading level (#, ##, ###, etc.).
    // CommonMark allows up to 3 leading spaces before the '#' markers; strip them first.
    if let Some(ref section) = loc.section_id {
        let found = content.lines().any(|line| {
            let stripped = line.trim_start_matches(' ');
            let after_hashes = stripped.trim_start_matches('#');
            // Must have at least one '#', a mandatory space/tab separator, then the section name.
            after_hashes != stripped
                && after_hashes.starts_with(|c: char| c.is_whitespace())
                && after_hashes.trim_start() == section.as_str()
        });
        if found {
            return VerifiedFinding {
                finding: finding.clone(),
                outcome: VerificationOutcome::Grounded,
                evidence_note: format!("Section '{section}' found in '{}'.", loc.artifact_path),
            };
        }
        return VerifiedFinding {
            finding: finding.clone(),
            outcome: VerificationOutcome::CannotBeVerified,
            evidence_note: format!(
                "Section '{section}' heading not found in '{}' — may be nested differently.",
                loc.artifact_path
            ),
        };
    }

    // Symbol-name grounding.
    if let Some(ref sym) = loc.symbol_name {
        if content.contains(sym.as_str()) {
            return VerifiedFinding {
                finding: finding.clone(),
                outcome: VerificationOutcome::Grounded,
                evidence_note: format!("Symbol '{sym}' found in '{}'.", loc.artifact_path),
            };
        }
        return VerifiedFinding {
            finding: finding.clone(),
            outcome: VerificationOutcome::CannotBeVerified,
            evidence_note: format!("Symbol '{sym}' not found in '{}'.", loc.artifact_path),
        };
    }

    // Line-range only — bounds check. No text content is verified, so this is
    // CannotBeVerified (in-bounds) rather than Grounded; Refuted only if out-of-bounds.
    if let Some([start, end]) = loc.line_range {
        let line_count = u32::try_from(content.lines().count()).unwrap_or(u32::MAX);
        if start > end || start < 1 || end > line_count {
            return VerifiedFinding {
                finding: finding.clone(),
                outcome: VerificationOutcome::Refuted,
                evidence_note: format!(
                    "Lines {start}–{end} out of range in '{}' ({line_count} lines).",
                    loc.artifact_path
                ),
            };
        }
        return VerifiedFinding {
            finding: finding.clone(),
            outcome: VerificationOutcome::CannotBeVerified,
            evidence_note: format!(
                "Lines {start}–{end} are in bounds in '{}' ({line_count} lines) \
                 but no text anchor is available to verify content.",
                loc.artifact_path
            ),
        };
    }

    VerifiedFinding {
        finding: finding.clone(),
        outcome: VerificationOutcome::CannotBeVerified,
        evidence_note: "Anchor present but no verifiable field populated.".to_owned(),
    }
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        &s[..max]
    }
}

// ── Model response parsing ─────────────────────────────────────────────────────

/// Extracts the content between `<charter_packet>` and `</charter_packet>` tags.
#[must_use]
pub fn extract_charter_packet_json(response: &str) -> Option<&str> {
    extract_tagged(response, "charter_packet")
}

/// Extracts the content between `<findings_packet>` and `</findings_packet>` tags.
#[must_use]
pub fn extract_findings_packet_json(response: &str) -> Option<&str> {
    extract_tagged(response, "findings_packet")
}

fn extract_tagged<'a>(s: &'a str, tag: &str) -> Option<&'a str> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = s.find(open.as_str())? + open.len();
    let end = s[start..].find(close.as_str())?;
    Some(s[start..start + end].trim())
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // hinge_test: pins=findings_packet_required_fields, intended=schema-compliance, phase=P5
    #[test]
    fn test_findings_packet_schema() {
        // Pins: FindingsPacket has all required fields from the Artifact Specifications schema.
        // Flipping requires updating the schema AND the Artifact Specifications together.
        let packet = FindingsPacket {
            packet_id: "test-packet-id".to_owned(),
            artifact_ref: "charter.md:post-R0".to_owned(),
            round_number: 1,
            reviewer_id: "reviewer-1".to_owned(),
            reviewer_model_identity: "claude-sonnet-4-6".to_owned(),
            produced_at: Utc::now(),
            findings: vec![],
            artifact_hash: None,
            reviewer_meta: None,
        };
        assert_eq!(packet.round_number, 1);
        assert_eq!(packet.reviewer_id, "reviewer-1");
        assert!(packet.findings.is_empty());
        let json = serde_json::to_string(&packet).expect("serialize");
        let parsed: FindingsPacket = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.packet_id, "test-packet-id");
        assert_eq!(parsed.artifact_ref, "charter.md:post-R0");
        assert_eq!(parsed.round_number, 1);
    }

    #[test]
    fn test_finding_severity_roundtrip() {
        for sev in [
            FindingSeverity::P1,
            FindingSeverity::P2,
            FindingSeverity::P3,
        ] {
            assert_eq!(FindingSeverity::parse(sev.as_str()), Some(sev));
        }
        assert!(FindingSeverity::parse("P0").is_none());
        assert!(FindingSeverity::parse("").is_none());
    }

    #[test]
    fn test_charter_packet_validate() {
        let valid = CharterPacket {
            title: "Test Project".to_owned(),
            produced_at: Utc::now(),
            goals: vec!["Build X".to_owned()],
            scope: "Everything".to_owned(),
            out_of_scope: vec![],
            required_choices: vec![],
            success_criteria: vec!["X ships".to_owned()],
            stakeholders: vec![],
            additional_notes: None,
        };
        assert!(valid.validate().is_ok());

        let empty_title = CharterPacket {
            title: String::new(),
            ..valid.clone()
        };
        assert_eq!(empty_title.validate(), Err("title".to_owned()));

        let no_goals = CharterPacket {
            goals: vec![],
            ..valid.clone()
        };
        assert_eq!(no_goals.validate(), Err("goals".to_owned()));
    }

    #[test]
    fn test_charter_packet_from_prompt_example() {
        // Regression: the exact JSON from the Interlocutor prompt must parse without `produced_at`.
        let example_json = r#"{
            "title": "My Project",
            "goals": ["Accomplish X", "Enable Y"],
            "scope": "Everything needed to accomplish X and Y.",
            "out_of_scope": ["Z feature"],
            "required_choices": ["Primary language"],
            "success_criteria": ["X ships to production", "Y is measurable"],
            "stakeholders": ["Alice (product)", "Bob (eng)"],
            "additional_notes": null
        }"#;
        let packet =
            CharterPacket::from_model_json(example_json).expect("prompt example must parse");
        packet
            .validate()
            .expect("prompt example must pass validation");
        assert_eq!(packet.title, "My Project");
        assert_eq!(packet.goals.len(), 2);
        assert_eq!(packet.success_criteria.len(), 2);
    }

    #[test]
    fn test_extract_charter_packet_json() {
        let response =
            "Some text\n<charter_packet>\n{\"title\":\"X\"}\n</charter_packet>\nMore text";
        assert_eq!(
            extract_charter_packet_json(response),
            Some("{\"title\":\"X\"}")
        );
        assert!(extract_charter_packet_json("no tags here").is_none());
    }

    #[test]
    fn test_extract_findings_packet_json() {
        let response = "<findings_packet>{\"reviewer_id\":\"r1\"}</findings_packet>";
        assert_eq!(
            extract_findings_packet_json(response),
            Some("{\"reviewer_id\":\"r1\"}")
        );
    }

    #[test]
    fn test_location_anchor_is_anchored() {
        let unanchored = LocationAnchor {
            artifact_path: "charter.md".to_owned(),
            section_id: None,
            line_range: None,
            symbol_name: None,
            quote: None,
        };
        assert!(!unanchored.is_anchored());

        let anchored = LocationAnchor {
            section_id: Some("Goals".to_owned()),
            ..unanchored
        };
        assert!(anchored.is_anchored());
    }

    #[test]
    fn test_verify_findings_unanchored() {
        let finding = Finding {
            id: "F1".to_owned(),
            severity: FindingSeverity::P2,
            location: LocationAnchor {
                artifact_path: "charter.md".to_owned(),
                section_id: None,
                line_range: None,
                symbol_name: None,
                quote: None,
            },
            claim: "Missing section".to_owned(),
            evidence: "N/A".to_owned(),
            recommendation: "Add it".to_owned(),
            metadata: None,
            advisory: false,
        };
        let verified = verify_one(&finding, std::path::Path::new("."));
        assert_eq!(verified.outcome, VerificationOutcome::CannotBeVerified);
    }

    #[test]
    fn test_verify_finding_with_quote() {
        use std::io::Write as _;
        let dir = std::env::temp_dir();
        let path = dir.join("anvil_test_verify_charter.md");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, "## Goals\n\nBuild a great product.").unwrap();
        drop(f);

        let finding = Finding {
            id: "F1".to_owned(),
            severity: FindingSeverity::P1,
            location: LocationAnchor {
                artifact_path: "anvil_test_verify_charter.md".to_owned(),
                section_id: None,
                line_range: None,
                symbol_name: None,
                quote: Some("Build a great product.".to_owned()),
            },
            claim: "test".to_owned(),
            evidence: "test".to_owned(),
            recommendation: "test".to_owned(),
            metadata: None,
            advisory: false,
        };
        let verified = verify_one(&finding, &dir);
        assert_eq!(verified.outcome, VerificationOutcome::Grounded);

        let finding_bad_quote = Finding {
            location: LocationAnchor {
                quote: Some("This text does not exist in the file.".to_owned()),
                ..finding.location.clone()
            },
            ..finding
        };
        let verified_bad = verify_one(&finding_bad_quote, &dir);
        assert_eq!(verified_bad.outcome, VerificationOutcome::Refuted);

        std::fs::remove_file(&path).ok();
    }

    #[allow(clippy::too_many_lines)]
    #[test]
    fn test_verify_section_heading_all_levels() {
        use std::io::Write as _;
        let dir = std::env::temp_dir();
        let path = dir.join("anvil_test_headings.md");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, "# Top Level\n## Second\n### Third\n#### Fourth").unwrap();
        drop(f);

        for section in ["Top Level", "Second", "Third", "Fourth"] {
            let finding = Finding {
                id: "F1".to_owned(),
                severity: FindingSeverity::P3,
                location: LocationAnchor {
                    artifact_path: "anvil_test_headings.md".to_owned(),
                    section_id: Some(section.to_owned()),
                    line_range: None,
                    symbol_name: None,
                    quote: None,
                },
                claim: "test".to_owned(),
                evidence: "test".to_owned(),
                recommendation: "test".to_owned(),
                metadata: None,
                advisory: false,
            };
            let verified = verify_one(&finding, &dir);
            assert_eq!(
                verified.outcome,
                VerificationOutcome::Grounded,
                "section '{section}' should be Grounded"
            );
        }

        let missing = Finding {
            id: "F2".to_owned(),
            severity: FindingSeverity::P3,
            location: LocationAnchor {
                artifact_path: "anvil_test_headings.md".to_owned(),
                section_id: Some("Nonexistent Section".to_owned()),
                line_range: None,
                symbol_name: None,
                quote: None,
            },
            claim: "test".to_owned(),
            evidence: "test".to_owned(),
            recommendation: "test".to_owned(),
            metadata: None,
            advisory: false,
        };
        let verified_missing = verify_one(&missing, &dir);
        assert_eq!(
            verified_missing.outcome,
            VerificationOutcome::CannotBeVerified
        );

        // A line like `###NoSpace` (no whitespace after hashes) must NOT be grounded.
        let mut f2 = std::fs::File::create(&path).unwrap();
        writeln!(f2, "###NoSpace").unwrap();
        drop(f2);
        let nospace = Finding {
            id: "F3".to_owned(),
            severity: FindingSeverity::P3,
            location: LocationAnchor {
                artifact_path: "anvil_test_headings.md".to_owned(),
                section_id: Some("NoSpace".to_owned()),
                line_range: None,
                symbol_name: None,
                quote: None,
            },
            claim: "test".to_owned(),
            evidence: "test".to_owned(),
            recommendation: "test".to_owned(),
            metadata: None,
            advisory: false,
        };
        let verified_nospace = verify_one(&nospace, &dir);
        assert_eq!(
            verified_nospace.outcome,
            VerificationOutcome::CannotBeVerified,
            "###NoSpace (no space after hashes) must not be Grounded"
        );

        // Indented headings (up to 3 leading spaces) must be grounded.
        let mut f3 = std::fs::File::create(&path).unwrap();
        writeln!(f3, "   ### Indented").unwrap();
        drop(f3);
        let indented = Finding {
            id: "F4".to_owned(),
            severity: FindingSeverity::P3,
            location: LocationAnchor {
                artifact_path: "anvil_test_headings.md".to_owned(),
                section_id: Some("Indented".to_owned()),
                line_range: None,
                symbol_name: None,
                quote: None,
            },
            claim: "test".to_owned(),
            evidence: "test".to_owned(),
            recommendation: "test".to_owned(),
            metadata: None,
            advisory: false,
        };
        let verified_indented = verify_one(&indented, &dir);
        assert_eq!(
            verified_indented.outcome,
            VerificationOutcome::Grounded,
            "   ### Indented (3 leading spaces) must be Grounded"
        );

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_from_model_json_missing_required_fields() {
        // Missing title: serde fails because title has no #[serde(default)].
        let no_title = r#"{"goals": ["X"], "scope": "S", "success_criteria": ["Y"]}"#;
        assert!(
            CharterPacket::from_model_json(no_title).is_err(),
            "missing title should fail deserialization"
        );

        // Missing goals (defaults to []): parses OK, validate() catches it.
        let no_goals = r#"{"title": "T", "scope": "S", "success_criteria": ["Y"]}"#;
        let p = CharterPacket::from_model_json(no_goals).expect("parses with default goals");
        assert_eq!(p.validate(), Err("goals".to_owned()));

        // Missing scope (defaults to ""): parses OK, validate() catches it.
        let no_scope = r#"{"title": "T", "goals": ["X"], "success_criteria": ["Y"]}"#;
        let p2 = CharterPacket::from_model_json(no_scope).expect("parses with default scope");
        assert_eq!(p2.validate(), Err("scope".to_owned()));

        // Missing success_criteria (defaults to []): parses OK, validate() catches it.
        let no_sc = r#"{"title": "T", "goals": ["X"], "scope": "S"}"#;
        let p3 =
            CharterPacket::from_model_json(no_sc).expect("parses with default success_criteria");
        assert_eq!(p3.validate(), Err("success_criteria".to_owned()));
    }

    // hinge_test: pins=severity_tiering_p3_always_advisory, intended=advisory-semantics, phase=P6
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_apply_severity_tiering_p3_always_advisory_p2_after_round_5() {
        // Pins: P3 is advisory in ALL rounds (Artifact Spec §Standard Vocabularies).
        //       P2 is advisory only in rounds 6+ (after ADVISORY_THRESHOLD_ROUND).
        //       P1 is never advisory.
        // Flipping requires updating ARTIFACT_SPECIFICATIONS.md + this test together.
        use super::{apply_severity_tiering, check_advisory_gate, AdvisoryDispositionType};

        let make_finding = |id: &str, sev: FindingSeverity| Finding {
            id: id.to_owned(),
            severity: sev,
            location: LocationAnchor {
                artifact_path: "charter.md".to_owned(),
                section_id: None,
                line_range: None,
                symbol_name: None,
                quote: None,
            },
            claim: "test".to_owned(),
            evidence: "test".to_owned(),
            recommendation: "test".to_owned(),
            metadata: None,
            advisory: false,
        };

        let mut packet = FindingsPacket::new(
            "charter.md:R1".to_owned(),
            1,
            "reviewer-1".to_owned(),
            "model-v1".to_owned(),
            vec![
                make_finding("F1", FindingSeverity::P1),
                make_finding("F2", FindingSeverity::P2),
                make_finding("F3", FindingSeverity::P3),
            ],
        );

        // Round 5 (≤ ADVISORY_THRESHOLD_ROUND): P3 advisory, P2 still blocking, P1 never advisory.
        apply_severity_tiering(&mut packet, 5);
        assert!(!packet.findings[0].advisory, "P1 must not become advisory");
        assert!(
            !packet.findings[1].advisory,
            "P2 at round 5 must not be advisory"
        );
        assert!(
            packet.findings[2].advisory,
            "P3 must be advisory in all rounds including round 5"
        );

        // Reset to test round 6 transition cleanly.
        packet.findings[2].advisory = false;

        // Round 6 (> ADVISORY_THRESHOLD_ROUND): P2 and P3 both advisory, P1 still blocking.
        apply_severity_tiering(&mut packet, 6);
        assert!(
            !packet.findings[0].advisory,
            "P1 must never become advisory"
        );
        assert!(
            packet.findings[1].advisory,
            "P2 at round 6 must be advisory"
        );
        assert!(
            packet.findings[2].advisory,
            "P3 at round 6 must be advisory"
        );

        // Gate flags both F2 and F3 (advisory, no disposition).
        let missing = check_advisory_gate(&[], &packet.findings);
        assert_eq!(missing, vec!["F2", "F3"], "gate should flag F2 and F3");

        // Gate passes when both have AcceptAdvisory dispositions.
        let dispositions = vec![
            CurationDisposition {
                finding_id: "F2".to_owned(),
                action: CurationAction::Keep,
                edited_finding: None,
                annotation: None,
                advisory_disposition: Some(AdvisoryDispositionType::AcceptAdvisory),
            },
            CurationDisposition {
                finding_id: "F3".to_owned(),
                action: CurationAction::Keep,
                edited_finding: None,
                annotation: None,
                advisory_disposition: Some(AdvisoryDispositionType::AcceptAdvisory),
            },
        ];
        let missing_after_accept = check_advisory_gate(&dispositions, &packet.findings);
        assert!(
            missing_after_accept.is_empty(),
            "AcceptAdvisory should satisfy the gate"
        );

        // Gate fails Drop/Defer with empty annotation.
        let dispositions_empty_drop = vec![CurationDisposition {
            finding_id: "F2".to_owned(),
            action: CurationAction::Drop,
            edited_finding: None,
            annotation: None,
            advisory_disposition: Some(AdvisoryDispositionType::DropAdvisory),
        }];
        let missing_empty_drop =
            check_advisory_gate(&dispositions_empty_drop, &[packet.findings[1].clone()]);
        assert_eq!(
            missing_empty_drop,
            vec!["F2"],
            "DropAdvisory with empty annotation must fail the gate"
        );

        // Gate passes Drop/Defer with non-empty annotation.
        let dispositions_filled_drop = vec![CurationDisposition {
            finding_id: "F2".to_owned(),
            action: CurationAction::Drop,
            edited_finding: None,
            annotation: Some("not applicable in this context".to_owned()),
            advisory_disposition: Some(AdvisoryDispositionType::DropAdvisory),
        }];
        let missing_filled_drop =
            check_advisory_gate(&dispositions_filled_drop, &[packet.findings[1].clone()]);
        assert!(
            missing_filled_drop.is_empty(),
            "DropAdvisory with non-empty annotation must pass the gate"
        );

        // P1 is non-advisory: gate never flags it.
        let missing_p1 = check_advisory_gate(&[], &[packet.findings[0].clone()]);
        assert!(
            missing_p1.is_empty(),
            "non-advisory P1 must not appear in gate failures"
        );
    }

    #[test]
    fn test_verify_line_range_returns_cannot_be_verified() {
        use std::io::Write as _;
        let dir = std::env::temp_dir();
        let path = dir.join("anvil_test_linerange.md");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, "line 1\nline 2\nline 3").unwrap();
        drop(f);

        let finding = Finding {
            id: "F1".to_owned(),
            severity: FindingSeverity::P2,
            location: LocationAnchor {
                artifact_path: "anvil_test_linerange.md".to_owned(),
                section_id: None,
                line_range: Some([1, 2]),
                symbol_name: None,
                quote: None,
            },
            claim: "test".to_owned(),
            evidence: "test".to_owned(),
            recommendation: "test".to_owned(),
            metadata: None,
            advisory: false,
        };
        let verified = verify_one(&finding, &dir);
        // In-bounds line range: CannotBeVerified (bounds check only, no text verified)
        assert_eq!(verified.outcome, VerificationOutcome::CannotBeVerified);

        // Out-of-bounds: Refuted
        let out_of_bounds = Finding {
            location: LocationAnchor {
                line_range: Some([10, 20]),
                ..finding.location.clone()
            },
            ..finding
        };
        let verified_oob = verify_one(&out_of_bounds, &dir);
        assert_eq!(verified_oob.outcome, VerificationOutcome::Refuted);

        std::fs::remove_file(&path).ok();
    }
}
