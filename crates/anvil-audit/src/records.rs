use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// All 13 audit record types (11 Charter-required + 2 Plan-extensions).
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
            _ => None,
        }
    }
}

/// All 13 record types (Charter-required + Plan-extensions).
pub const ALL_RECORD_TYPES: [RecordType; 13] = [
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
// type-specific fields. Type-specific fields will be extended in the phases
// that produce the corresponding workflow artifacts (P5–P8).

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewerFindingPacket {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub cross_references: Vec<String>,
    pub phase_id: String,
    pub reviewer_id: String,
    pub finding_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifierResult {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub cross_references: Vec<String>,
    pub phase_id: String,
    pub verifier_id: String,
    pub passed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotationLog {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub cross_references: Vec<String>,
    pub rotated_from: String,
    pub rotated_to: String,
    pub reason: String,
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
    pub finding_id: String,
    pub resolution: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SidecarReload {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub cross_references: Vec<String>,
    pub config_epoch: String,
    pub reason: String,
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
impl_audit_record!(ArbiterFindingResolution, RecordType::ArbiterFindingResolution);
impl_audit_record!(SidecarReload, RecordType::SidecarReload);

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
}
