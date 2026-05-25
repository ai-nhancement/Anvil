use std::sync::atomic::{AtomicU64, Ordering};

/// Layer-1 metric counters wired at the audit-store write path.
///
/// P10a collection infrastructure reads from these counters via [`StoreMetrics::snapshot`].
#[derive(Debug, Default)]
pub struct StoreMetrics {
    /// Total records appended across all types since the store was opened.
    pub total_appended: AtomicU64,
}

impl StoreMetrics {
    /// Returns the current total-appended count.
    #[must_use]
    pub fn snapshot_total_appended(&self) -> u64 {
        self.total_appended.load(Ordering::Relaxed)
    }
}
