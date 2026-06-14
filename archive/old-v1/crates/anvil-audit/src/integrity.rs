/// Whether the integrity check allows proceeding to ship.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntegrityStatus {
    /// All indexed records are physically present; no cross-reference gaps.
    Pass,
    /// Non-blocking issues detected (reserved for future warning-level checks).
    Warn,
    /// One or more indexed records are missing from disk; ship is blocked.
    BlockShip,
}

/// How severe a single integrity violation is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViolationSeverity {
    /// Prevents shipping — a required record is missing or corrupted.
    BlockShip,
    /// Non-blocking — an unindexed orphan file or other advisory issue.
    Warn,
}

/// A single integrity violation found during a completeness check.
#[derive(Debug, Clone)]
pub struct IntegrityViolation {
    pub id: String,
    /// Path relative to the project root where the file was expected.
    pub path: String,
    pub reason: String,
    pub severity: ViolationSeverity,
}

/// Result of [`crate::store::AuditStore::check_integrity`].
#[derive(Debug, Clone)]
pub struct IntegrityReport {
    pub status: IntegrityStatus,
    pub violations: Vec<IntegrityViolation>,
}

impl IntegrityReport {
    /// Returns `true` if the report blocks shipping.
    #[must_use]
    pub fn is_blocking(&self) -> bool {
        self.status == IntegrityStatus::BlockShip
    }
}
