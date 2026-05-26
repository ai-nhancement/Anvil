use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use anvil_core::pipeline::{CurationDisposition, FindingsPacket, VerifiedFinding};

/// All 14 audit record types (11 Charter-required + 3 Plan-extensions).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordType {
    ReviewerFindingPacket,
    VerifierResult,
    RotationLog,
    CharterAmendment,
    PlanAmendment,
    PhaseDisposition,
    HingeFlip,
    GateApproval,
    ConvergenceDeclaration,
    ProvisionalLock,
    RollbackEvent,
    /// Plan-extension (P6): per-finding arbiter override.
    ArbiterFindingResolution,
    /// Plan-extension (P3b): config-epoch reload events.
    SidecarReload,
    /// Plan-extension (P5): curation decisions for a `ReviewerFindingPacket`.
    CuratedFindings,
    /// Plan-extension (P7): version snapshot when Plan is consolidated.
    PlanConsolidation,
}

impl RecordType {
    /// Subdirectory name under `audit-store/` for this record type.
    #[must_use]
    pub fn dir_name(self) -> &'static str {
        match self {
            Self::ReviewerFindingPacket => "reviewer-finding-packet",
            Self::VerifierResult => "verifier-result",
            Self::RotationLog => "rotation-log",
            Self::CharterAmendment => "charter-amendment",
            Self::PlanAmendment => "plan-amendment",
            Self::PhaseDisposition => "phase-disposition",
            Self::HingeFlip => "hinge-flip",
            Self::GateApproval => "gate-approval",
            Self::ConvergenceDeclaration => "convergence-declaration",
            Self::ProvisionalLock => "provisional-lock",
            Self::RollbackEvent => "rollback-event",
            Self::ArbiterFindingResolution => "arbiter-finding-resolution",
            Self::SidecarReload => "sidecar-reload",
            Self::CuratedFindings => "curated-findings",
            Self::PlanConsolidation => "plan-consolidation",
        }
    }

    /// Pascal-case name used in index files and CLI output (e.g., `GateApproval`).
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ReviewerFindingPacket => "ReviewerFindingPacket",
            Self::VerifierResult => "VerifierResult",
            Self::RotationLog => "RotationLog",
            Self::CharterAmendment => "CharterAmendment",
            Self::PlanAmendment => "PlanAmendment",
            Self::PhaseDisposition => "PhaseDisposition",
            Self::HingeFlip => "HingeFlip",
            Self::GateApproval => "GateApproval",
            Self::ConvergenceDeclaration => "ConvergenceDeclaration",
            Self::ProvisionalLock => "ProvisionalLock",
            Self::RollbackEvent => "RollbackEvent",
            Self::ArbiterFindingResolution => "ArbiterFindingResolution",
            Self::SidecarReload => "SidecarReload",
            Self::CuratedFindings => "CuratedFindings",
            Self::PlanConsolidation => "PlanConsolidation",
        }
    }

    /// Parses from the pascal-case `as_str()` form (e.g., `"GateApproval"`).
    #[must_use]
    pub fn from_type_name(s: &str) -> Option<Self> {
        match s {
            "ReviewerFindingPacket" => Some(Self::ReviewerFindingPacket),
            "VerifierResult" => Some(Self::VerifierResult),
            "RotationLog" => Some(Self::RotationLog),
            "CharterAmendment" => Some(Self::CharterAmendment),
            "PlanAmendment" => Some(Self::PlanAmendment),
            "PhaseDisposition" => Some(Self::PhaseDisposition),
            "HingeFlip" => Some(Self::HingeFlip),
            "GateApproval" => Some(Self::GateApproval),
            "ConvergenceDeclaration" => Some(Self::ConvergenceDeclaration),
            "ProvisionalLock" => Some(Self::ProvisionalLock),
            "RollbackEvent" => Some(Self::RollbackEvent),
            "ArbiterFindingResolution" => Some(Self::ArbiterFindingResolution),
            "SidecarReload" => Some(Self::SidecarReload),
            "CuratedFindings" => Some(Self::CuratedFindings),
            "PlanConsolidation" => Some(Self::PlanConsolidation),
            _ => None,
        }
    }

    /// Parses from the kebab-case `dir_name()` form.
    #[must_use]
    pub fn from_dir_name(s: &str) -> Option<Self> {
        match s {
            "reviewer-finding-packet" => Some(Self::ReviewerFindingPacket),
            "verifier-result" => Some(Self::VerifierResult),
            "rotation-log" => Some(Self::RotationLog),
            "charter-amendment" => Some(Self::CharterAmendment),
            "plan-amendment" => Some(Self::PlanAmendment),
            "phase-disposition" => Some(Self::PhaseDisposition),
            "hinge-flip" => Some(Self::HingeFlip),
            "gate-approval" => Some(Self::GateApproval),
            "convergence-declaration" => Some(Self::ConvergenceDeclaration),
            "provisional-lock" => Some(Self::ProvisionalLock),
            "rollback-event" => Some(Self::RollbackEvent),
            "arbiter-finding-resolution" => Some(Self::ArbiterFindingResolution),
            "sidecar-reload" => Some(Self::SidecarReload),
            "curated-findings" => Some(Self::CuratedFindings),
            "plan-consolidation" => Some(Self::PlanConsolidation),
            _ => None,
        }
    }
}

/// All 15 record types (11 Charter-required + 4 Plan-extensions).
pub const ALL_RECORD_TYPES: [RecordType; 15] = [
    RecordType::ReviewerFindingPacket,
    RecordType::VerifierResult,
    RecordType::RotationLog,
    RecordType::CharterAmendment,
    RecordType::PlanAmendment,
    RecordType::PhaseDisposition,
    RecordType::HingeFlip,
    RecordType::GateApproval,
    RecordType::ConvergenceDeclaration,
    RecordType::ProvisionalLock,
    RecordType::RollbackEvent,
    RecordType::ArbiterFindingResolution,
    RecordType::SidecarReload,
    RecordType::CuratedFindings,
    RecordType::PlanConsolidation,
];

/// The 11 Charter-required record type names (pascal-case, matching `RecordType::as_str()`).
/// Plan-level extensions are permitted under the Charter's "minimum set; Plan may extend" wording.
pub const CHARTER_REQUIRED_TYPES: [&str; 11] = [
    "ReviewerFindingPacket",
    "VerifierResult",
    "RotationLog",
    "CharterAmendment",
    "PlanAmendment",
    "PhaseDisposition",
    "HingeFlip",
    "GateApproval",
    "ConvergenceDeclaration",
    "ProvisionalLock",
    "RollbackEvent",
];

/// Shared interface for all audit record types.
pub trait AuditRecord: serde::Serialize {
    #[must_use]
    fn id(&self) -> &str;
    #[must_use]
    fn record_type(&self) -> RecordType;
    /// Cross-reference keys this record backs (format: `<artifact-path>:<section-id>:<version>`).
    #[must_use]
    fn cross_references(&self) -> &[String];
}

// ── Record structs ────────────────────────────────────────────────────────────
// Each struct contains common fields (id, created_at, cross_references) plus
// type-specific fields.

/// Audit record wrapping a full `FindingsPacket` from a review round.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewerFindingPacket {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub cross_references: Vec<String>,
    pub phase_id: String,
    pub reviewer_id: String,
    pub finding_count: u32,
    /// Full findings packet (P5+).
    pub packet: FindingsPacket,
}

impl ReviewerFindingPacket {
    /// Constructs a new record wrapping the given `FindingsPacket`.
    #[must_use]
    pub fn from_packet(
        phase_id: String,
        packet: FindingsPacket,
        cross_references: Vec<String>,
    ) -> Self {
        let finding_count = u32::try_from(packet.findings.len()).unwrap_or(u32::MAX);
        let reviewer_id = packet.reviewer_id.clone();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            created_at: Utc::now(),
            cross_references,
            phase_id,
            reviewer_id,
            finding_count,
            packet,
        }
    }
}

/// Audit record for the Finding Verifier's pass over a `ReviewerFindingPacket`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifierResult {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub cross_references: Vec<String>,
    pub phase_id: String,
    pub verifier_id: String,
    pub passed: bool,
    /// `FindingsPacket.packet_id` this result was computed for. Used by curation to
    /// verify that the latest `VerifierResult` corresponds to the latest `ReviewerFindingPacket`.
    pub source_packet_id: String,
    /// Per-finding verified results (P5+).
    pub verified_findings: Vec<VerifiedFinding>,
}

impl VerifierResult {
    /// Constructs a new verifier result record.
    #[must_use]
    pub fn from_verified(
        phase_id: String,
        verifier_id: String,
        source_packet_id: String,
        verified_findings: Vec<VerifiedFinding>,
        cross_references: Vec<String>,
    ) -> Self {
        use anvil_core::pipeline::VerificationOutcome;
        let passed = verified_findings
            .iter()
            .all(|vf| vf.outcome != VerificationOutcome::Refuted);
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            created_at: Utc::now(),
            cross_references,
            phase_id,
            verifier_id,
            passed,
            source_packet_id,
            verified_findings,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotationLog {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub cross_references: Vec<String>,
    /// The previous reviewer binding name, or `None` for the first review round.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rotated_from: Option<String>,
    pub rotated_to: String,
    pub reason: String,
    /// The review round that triggered this rotation.
    pub round_number: u32,
}

impl RotationLog {
    #[must_use]
    pub fn new(
        rotated_from: Option<String>,
        rotated_to: String,
        reason: String,
        round_number: u32,
        cross_references: Vec<String>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            created_at: Utc::now(),
            cross_references,
            rotated_from,
            rotated_to,
            reason,
            round_number,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharterAmendment {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub cross_references: Vec<String>,
    pub amendment_id: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanAmendment {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub cross_references: Vec<String>,
    pub amendment_id: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseDisposition {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub cross_references: Vec<String>,
    pub phase_id: String,
    pub disposition: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HingeFlip {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub cross_references: Vec<String>,
    pub hinge_test_name: String,
    pub old_value: String,
    pub new_value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateApproval {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub cross_references: Vec<String>,
    pub gate_name: String,
    pub approver: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvergenceDeclaration {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub cross_references: Vec<String>,
    pub phase_id: String,
    pub round_count: u32,
    /// Non-empty arbiter reasoning required to create this record (P6).
    pub reasoning: String,
    /// Count of advisory findings open at declaration time (P6).
    pub advisory_finding_count: u32,
    /// Count of arbiter-decided findings at declaration time (P6).
    pub arbiter_decided_count: u32,
}

impl ConvergenceDeclaration {
    #[must_use]
    pub fn new(
        phase_id: String,
        round_count: u32,
        reasoning: String,
        advisory_finding_count: u32,
        arbiter_decided_count: u32,
        cross_references: Vec<String>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            created_at: Utc::now(),
            cross_references,
            phase_id,
            round_count,
            reasoning,
            advisory_finding_count,
            arbiter_decided_count,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvisionalLock {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub cross_references: Vec<String>,
    pub choice_key: String,
    pub hypothesis: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackEvent {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub cross_references: Vec<String>,
    pub target_phase: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbiterFindingResolution {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub cross_references: Vec<String>,
    /// Composite finding reference: `"<packet_id>:<finding_id>"` (e.g., `"uuid:F1"`).
    pub finding_id: String,
    /// Identifier of the human arbiter making this resolution.
    pub arbiter_id: String,
    /// Non-empty reasoning required to create this record.
    pub reasoning: String,
    /// Summary of the chosen direction (e.g., "Keep approach X as designed").
    pub chosen_direction_summary: String,
    /// Which other findings or rounds this contradicts or relates to.
    pub contradiction_context: String,
}

impl ArbiterFindingResolution {
    #[must_use]
    pub fn new(
        finding_id: String,
        arbiter_id: String,
        reasoning: String,
        chosen_direction_summary: String,
        contradiction_context: String,
        cross_references: Vec<String>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            created_at: Utc::now(),
            cross_references,
            finding_id,
            arbiter_id,
            reasoning,
            chosen_direction_summary,
            contradiction_context,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SidecarReload {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub cross_references: Vec<String>,
    pub config_epoch: String,
    pub reason: String,
}

/// Audit record for the Coordinator's curation of a `ReviewerFindingPacket` (P5+).
///
/// Written as a sibling to the original packet; never modifies the original.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CuratedFindingsRecord {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub cross_references: Vec<String>,
    /// References the `ReviewerFindingPacket.packet.packet_id`.
    pub packet_id: String,
    pub curated_by: String,
    pub dispositions: Vec<CurationDisposition>,
}

impl CuratedFindingsRecord {
    #[must_use]
    pub fn new(
        packet_id: String,
        curated_by: String,
        dispositions: Vec<CurationDisposition>,
        cross_references: Vec<String>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            created_at: Utc::now(),
            cross_references,
            packet_id,
            curated_by,
            dispositions,
        }
    }
}

/// Audit record written when the Plan is consolidated (P7).
///
/// Stores the full prior Plan text as `prior_plan_snapshot` so the previous
/// version remains queryable via `anvil audit show <id>`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanConsolidationRecord {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub cross_references: Vec<String>,
    /// Semver string of the Plan version before consolidation (e.g. `"1.0.0"`).
    pub plan_version_from: String,
    /// Semver string of the Plan version after consolidation (e.g. `"1.1.0"`).
    pub plan_version_to: String,
    /// Human-readable trigger event (e.g. `"end-of-P7"`).
    pub trigger: String,
    /// Review round numbers whose hardening notes were absorbed.
    pub hardening_rounds_absorbed: Vec<u32>,
    /// Full text of the Plan file before this consolidation.
    pub prior_plan_snapshot: String,
}

// ── AuditRecord impls ─────────────────────────────────────────────────────────

macro_rules! impl_audit_record {
    ($t:ty, $variant:expr) => {
        impl AuditRecord for $t {
            fn id(&self) -> &str {
                &self.id
            }
            fn record_type(&self) -> RecordType {
                $variant
            }
            fn cross_references(&self) -> &[String] {
                &self.cross_references
            }
        }
    };
}

impl_audit_record!(ReviewerFindingPacket, RecordType::ReviewerFindingPacket);
impl_audit_record!(VerifierResult, RecordType::VerifierResult);
impl_audit_record!(RotationLog, RecordType::RotationLog);
impl_audit_record!(CharterAmendment, RecordType::CharterAmendment);
impl_audit_record!(PlanAmendment, RecordType::PlanAmendment);
impl_audit_record!(PhaseDisposition, RecordType::PhaseDisposition);
impl_audit_record!(HingeFlip, RecordType::HingeFlip);
impl_audit_record!(GateApproval, RecordType::GateApproval);
impl_audit_record!(ConvergenceDeclaration, RecordType::ConvergenceDeclaration);
impl_audit_record!(ProvisionalLock, RecordType::ProvisionalLock);
impl_audit_record!(RollbackEvent, RecordType::RollbackEvent);
impl_audit_record!(
    ArbiterFindingResolution,
    RecordType::ArbiterFindingResolution
);
impl_audit_record!(SidecarReload, RecordType::SidecarReload);
impl_audit_record!(CuratedFindingsRecord, RecordType::CuratedFindings);
impl_audit_record!(PlanConsolidationRecord, RecordType::PlanConsolidation);

// ── Constructors ──────────────────────────────────────────────────────────────

impl GateApproval {
    #[must_use]
    pub fn new(gate_name: String, approver: String, cross_references: Vec<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            created_at: Utc::now(),
            cross_references,
            gate_name,
            approver,
        }
    }
}

impl PhaseDisposition {
    #[must_use]
    pub fn new(phase_id: String, disposition: String, cross_references: Vec<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            created_at: Utc::now(),
            cross_references,
            phase_id,
            disposition,
        }
    }
}

impl HingeFlip {
    #[must_use]
    pub fn new(
        hinge_test_name: String,
        old_value: String,
        new_value: String,
        cross_references: Vec<String>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            created_at: Utc::now(),
            cross_references,
            hinge_test_name,
            old_value,
            new_value,
        }
    }
}

impl ProvisionalLock {
    #[must_use]
    pub fn new(choice_key: String, hypothesis: String, cross_references: Vec<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            created_at: Utc::now(),
            cross_references,
            choice_key,
            hypothesis,
        }
    }
}

impl SidecarReload {
    #[must_use]
    pub fn new(config_epoch: String, reason: String, cross_references: Vec<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            created_at: Utc::now(),
            cross_references,
            config_epoch,
            reason,
        }
    }
}

impl PlanConsolidationRecord {
    /// Creates a new consolidation record.
    ///
    /// `prior_plan_snapshot` is the full text of the Plan before consolidation,
    /// making the prior version queryable via the audit store (P7 AC5).
    #[must_use]
    pub fn new(
        plan_version_from: String,
        plan_version_to: String,
        trigger: String,
        hardening_rounds_absorbed: Vec<u32>,
        prior_plan_snapshot: String,
        cross_references: Vec<String>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            created_at: Utc::now(),
            cross_references,
            plan_version_from,
            plan_version_to,
            trigger,
            hardening_rounds_absorbed,
            prior_plan_snapshot,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_type_name_roundtrip() {
        for rt in ALL_RECORD_TYPES {
            let name = rt.as_str();
            assert_eq!(
                RecordType::from_type_name(name),
                Some(rt),
                "from_type_name({name:?}) must return {rt:?}"
            );
        }
    }

    #[test]
    fn test_record_type_dir_name_roundtrip() {
        for rt in ALL_RECORD_TYPES {
            let dir = rt.dir_name();
            assert_eq!(
                RecordType::from_dir_name(dir),
                Some(rt),
                "from_dir_name({dir:?}) must return {rt:?}"
            );
        }
    }

    #[test]
    fn test_record_type_parsers_reject_invalid_input() {
        assert!(RecordType::from_type_name("").is_none());
        assert!(RecordType::from_type_name("gate_approval").is_none()); // snake_case is not accepted
        assert!(RecordType::from_type_name("gateapproval").is_none());
        assert!(RecordType::from_dir_name("").is_none());
        assert!(RecordType::from_dir_name("GateApproval").is_none()); // PascalCase is not accepted
        assert!(RecordType::from_dir_name("unknown-type").is_none());
    }

    // hinge_test: pins=curated_findings_record_exists, intended=curation-audit-persistence, phase=P5
    #[test]
    fn test_curation_audit_record_required() {
        // Pins: CuratedFindingsRecord implements AuditRecord and persists as RecordType::CuratedFindings.
        // Flipping requires updating the record type, the audit-store directory, and this test together.
        use chrono::Utc;
        let cross_ref = crate::CrossRefKey::new("charter.md", "§root", "R1").to_key_string();
        let record = CuratedFindingsRecord {
            id: "test-curation-id".to_owned(),
            created_at: Utc::now(),
            cross_references: vec![cross_ref.clone()],
            packet_id: "test-packet-id".to_owned(),
            curated_by: "coordinator".to_owned(),
            dispositions: vec![],
        };
        assert_eq!(record.record_type(), RecordType::CuratedFindings);
        assert_eq!(RecordType::CuratedFindings.dir_name(), "curated-findings");
        // Full JSON round-trip
        let json = serde_json::to_string(&record).expect("serialize");
        let parsed: CuratedFindingsRecord = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.packet_id, "test-packet-id");
        assert_eq!(parsed.curated_by, "coordinator");
        assert!(parsed.dispositions.is_empty());
        // Cross-reference must be a valid three-part key
        assert!(crate::CrossRefKey::parse(&parsed.cross_references[0]).is_some());
    }

    #[test]
    fn test_verifier_result_source_packet_id_stored() {
        use chrono::Utc;
        let vr = VerifierResult {
            id: "test-vr-id".to_owned(),
            created_at: Utc::now(),
            cross_references: vec![],
            phase_id: "charter-R1".to_owned(),
            verifier_id: "local-verifier-v1".to_owned(),
            passed: true,
            source_packet_id: "pkt-abc".to_owned(),
            verified_findings: vec![],
        };
        let json = serde_json::to_string(&vr).expect("serialize");
        let parsed: VerifierResult = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.source_packet_id, "pkt-abc");
    }
}
