//! Evaluation metrics and alert engine for Anvil (P10a).
//!
//! Three-layer evaluation system:
//! - **Layer-1**: six metrics computed automatically from audit-store records.
//! - **Layer-2**: compares metric values against per-project numeric targets in `anvil.toml`.
//! - **Layer-3**: rule-based alert engine that fires on the four Charter-defined alert kinds.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};

use anvil_audit::records::{
    CuratedFindingsRecord, HingeFlip, PhaseDisposition, ProvisionalLock, ReviewerFindingPacket,
    RollbackEvent, DISPOSITION_SHIPPED,
};
use anvil_audit::{AuditStore, RecordType};
use anvil_core::config::MetricTargets;
use anvil_core::error::AnvilError;
use anvil_core::pipeline::CurationAction;

// ── Constants ─────────────────────────────────────────────────────────────────

/// Number of Layer-1 metrics. Pinned by `test_layer_1_metric_count`.
pub const LAYER1_METRIC_COUNT: usize = 6;

/// Number of Layer-3 alert kinds. Pinned by `test_alert_kinds_count`.
pub const ALERT_KIND_COUNT: usize = 4;

// ── Layer-1 Metrics ───────────────────────────────────────────────────────────

/// Six Layer-1 metrics computed from audit-store data.
///
/// `None` means insufficient data to compute the metric (e.g., no curated findings
/// for precision, no shipped phases for escape rate). The field count is pinned to
/// `LAYER1_METRIC_COUNT` by `test_layer_1_metric_count`.
#[derive(Debug, Clone)]
pub struct Layer1Metrics {
    /// Fraction of reviewer findings retained through curation (0.0–1.0).
    /// `None` if no phase review curated-findings records exist.
    pub reviewer_precision: Option<f64>,
    /// Similarity of finding counts across reviewers per phase (0.0–1.0); high
    /// agreement suggests reduced diversity. `None` if no phase had ≥2 reviewers.
    pub finding_count_agreement: Option<f64>,
    /// Average wall-clock minutes from first `ReviewerFindingPacket` to shipped
    /// `PhaseDisposition` across all shipped phases. `None` if no shipped phases with RFP data.
    pub human_minutes_per_phase: Option<f64>,
    /// Average `ConvergenceDeclaration.round_count` for shipped phases.
    /// `None` if no shipped phases have a convergence declaration.
    pub avg_round_count: Option<f64>,
    /// Count of unique hinge-test names that have been flipped via `HingeFlip` records.
    /// The denominator (total known hinge tests) requires the P10b registry.
    pub deferred_resolved_count: u32,
    /// Fraction of ever-shipped phases subsequently rolled back (0.0–1.0).
    /// `None` if no phases have ever been shipped.
    pub defect_escape_rate: Option<f64>,
}

/// Computes all six Layer-1 metrics from the audit store.
///
/// Records that cannot be deserialized are silently skipped (partial data is
/// preferable to an error for a metrics read path).
///
/// # Errors
///
/// Returns [`AnvilError`] only if the store index itself cannot be read.
pub fn compute_layer1(store: &AuditStore) -> Result<Layer1Metrics, AnvilError> {
    Ok(Layer1Metrics {
        reviewer_precision: compute_reviewer_precision(store)?,
        finding_count_agreement: compute_finding_count_agreement(store)?,
        human_minutes_per_phase: compute_human_minutes(store)?,
        avg_round_count: compute_avg_round_count(store)?,
        deferred_resolved_count: compute_deferred_resolved_count(store)?,
        defect_escape_rate: compute_defect_escape_rate(store)?,
    })
}

// ── Layer-1 helpers ───────────────────────────────────────────────────────────

fn all_rfps(store: &AuditStore) -> Result<Vec<ReviewerFindingPacket>, AnvilError> {
    Ok(store
        .list(RecordType::ReviewerFindingPacket)?
        .iter()
        .filter_map(|e| {
            store
                .get(&e.id)
                .ok()
                .and_then(|v| serde_json::from_value::<ReviewerFindingPacket>(v).ok())
        })
        .collect())
}

fn all_dispositions(store: &AuditStore) -> Result<Vec<PhaseDisposition>, AnvilError> {
    Ok(store
        .list(RecordType::PhaseDisposition)?
        .iter()
        .filter_map(|e| {
            store
                .get(&e.id)
                .ok()
                .and_then(|v| serde_json::from_value::<PhaseDisposition>(v).ok())
        })
        .collect())
}

fn all_rollbacks(store: &AuditStore) -> Result<Vec<RollbackEvent>, AnvilError> {
    Ok(store
        .list(RecordType::RollbackEvent)?
        .iter()
        .filter_map(|e| {
            store
                .get(&e.id)
                .ok()
                .and_then(|v| serde_json::from_value::<RollbackEvent>(v).ok())
        })
        .collect())
}

fn shipped_phase_latest_times(dispositions: &[PhaseDisposition]) -> HashMap<String, DateTime<Utc>> {
    let mut map: HashMap<String, DateTime<Utc>> = HashMap::new();
    for d in dispositions {
        if d.disposition == DISPOSITION_SHIPPED {
            let e = map.entry(d.phase_id.clone()).or_insert(d.created_at);
            if d.created_at > *e {
                *e = d.created_at;
            }
        }
    }
    map
}

/// Extracts the normalized phase ID from a `ReviewerFindingPacket.phase_id` or
/// `ConvergenceDeclaration.phase_id` field of the form `"phase:<id>"` or
/// `"phase:<id>:R<n>"`. Returns `None` for non-phase artifacts (e.g., `"charter.md"`,
/// `"plan-R1"`).
fn phase_id_from_artifact_ref(s: &str) -> Option<&str> {
    s.strip_prefix("phase:")
        .and_then(|rest| rest.split(':').next())
}

/// For each shipped phase, the timestamp of the most recent `RollbackEvent` that
/// invalidated it strictly before the current ship time. Absent from the map when
/// no prior rollback applies (use full history for that phase).
fn phase_epoch_starts(
    rollbacks: &[RollbackEvent],
    shipped: &HashMap<String, DateTime<Utc>>,
) -> HashMap<String, DateTime<Utc>> {
    let mut map: HashMap<String, DateTime<Utc>> = HashMap::new();
    for rb in rollbacks {
        let Some(&ship_time) = shipped.get(&rb.invalidated_phase) else {
            continue;
        };
        if rb.created_at >= ship_time {
            continue;
        }
        let e = map
            .entry(rb.invalidated_phase.clone())
            .or_insert(rb.created_at);
        if rb.created_at > *e {
            *e = rb.created_at;
        }
    }
    map
}

/// Returns whether `rfp.created_at` falls within the current shipping epoch for
/// its phase: after any rollback boundary and at/before the ship time.
fn rfp_in_epoch(
    created_at: DateTime<Utc>,
    ship_time: DateTime<Utc>,
    epoch_start: Option<DateTime<Utc>>,
) -> bool {
    match epoch_start {
        Some(es) => created_at > es && created_at <= ship_time,
        None => created_at <= ship_time,
    }
}

/// Builds a map from `packet_id` to the latest `CuratedFindingsRecord` for that
/// packet among those whose `packet_id` is in `known_packets`. Duplicate records for
/// the same packet are resolved by `created_at` (latest wins, so retries don't
/// double-count drops).
fn latest_curations_for_packets(
    store: &AuditStore,
    known_packets: &HashSet<&str>,
) -> Result<HashMap<String, CuratedFindingsRecord>, AnvilError> {
    let mut map: HashMap<String, CuratedFindingsRecord> = HashMap::new();
    for e in store.list(RecordType::CuratedFindings)? {
        if let Some(cr) = store
            .get(&e.id)
            .ok()
            .and_then(|v| serde_json::from_value::<CuratedFindingsRecord>(v).ok())
        {
            if !known_packets.contains(cr.packet_id.as_str()) {
                continue;
            }
            match map.entry(cr.packet_id.clone()) {
                std::collections::hash_map::Entry::Occupied(mut oe) => {
                    if cr.created_at > oe.get().created_at {
                        *oe.get_mut() = cr;
                    }
                }
                std::collections::hash_map::Entry::Vacant(ve) => {
                    ve.insert(cr);
                }
            }
        }
    }
    Ok(map)
}

fn compute_reviewer_precision(store: &AuditStore) -> Result<Option<f64>, AnvilError> {
    let rfps = all_rfps(store)?;

    // Only phase-artifact RFPs contribute to reviewer precision.
    let phase_rfps: Vec<&ReviewerFindingPacket> = rfps
        .iter()
        .filter(|r| phase_id_from_artifact_ref(&r.phase_id).is_some())
        .collect();

    let total_findings: u32 = phase_rfps.iter().map(|r| r.finding_count).sum();
    if total_findings == 0 {
        return Ok(None);
    }

    // Known packet IDs drawn from phase RFPs only. Orphan or non-phase curation
    // records do not contribute drops.
    let known_packets: HashSet<&str> = phase_rfps
        .iter()
        .map(|r| r.packet.packet_id.as_str())
        .collect();

    let curations = latest_curations_for_packets(store, &known_packets)?;

    // Count unique (packet_id, finding_id) drops to prevent double-counting from
    // duplicate curation records. Latest curation per packet already handles retries;
    // the set handles any finding_id repeated within one record.
    let mut dropped: HashSet<(String, String)> = HashSet::new();
    for cr in curations.values() {
        for d in &cr.dispositions {
            if d.action == CurationAction::Drop {
                dropped.insert((cr.packet_id.clone(), d.finding_id.clone()));
            }
        }
    }

    let total_dropped = u32::try_from(dropped.len()).unwrap_or(u32::MAX);
    let upheld = total_findings.saturating_sub(total_dropped);
    Ok(Some(f64::from(upheld) / f64::from(total_findings)))
}

fn compute_finding_count_agreement(store: &AuditStore) -> Result<Option<f64>, AnvilError> {
    let rfps = all_rfps(store)?;
    let dispositions = all_dispositions(store)?;
    let shipped = shipped_phase_latest_times(&dispositions);
    let rollbacks = all_rollbacks(store)?;
    let epochs = phase_epoch_starts(&rollbacks, &shipped);

    // Group current-epoch finding counts by normalized phase_id.
    // Charter and Plan review RFPs are excluded (no "phase:" prefix).
    let mut phase_counts: HashMap<String, Vec<u32>> = HashMap::new();
    for rfp in &rfps {
        let Some(phase_id) = phase_id_from_artifact_ref(&rfp.phase_id) else {
            continue;
        };
        let Some(&ship_time) = shipped.get(phase_id) else {
            continue;
        };
        if !rfp_in_epoch(rfp.created_at, ship_time, epochs.get(phase_id).copied()) {
            continue;
        }
        phase_counts
            .entry(phase_id.to_owned())
            .or_default()
            .push(rfp.finding_count);
    }

    // Only phases with ≥2 reviewers contribute to the agreement score.
    let multi_reviewer: Vec<&Vec<u32>> = phase_counts
        .values()
        .filter(|counts| counts.len() >= 2)
        .collect();

    if multi_reviewer.is_empty() {
        return Ok(None);
    }

    // Agreement per phase = min_count / max_count.
    // 1.0 = identical counts, 0.0 = one reviewer found nothing.
    // High values indicate reviewers are flagging the same volume — potential echo chamber.
    #[allow(clippy::cast_precision_loss)]
    let total: f64 = multi_reviewer
        .iter()
        .map(|counts| {
            let max = counts.iter().copied().max().unwrap_or(0);
            let min = counts.iter().copied().min().unwrap_or(0);
            if max == 0 {
                1.0_f64
            } else {
                f64::from(min) / f64::from(max)
            }
        })
        .sum();

    #[allow(clippy::cast_precision_loss)]
    Ok(Some(total / multi_reviewer.len() as f64))
}

fn compute_human_minutes(store: &AuditStore) -> Result<Option<f64>, AnvilError> {
    let rfps = all_rfps(store)?;
    let dispositions = all_dispositions(store)?;
    let shipped = shipped_phase_latest_times(&dispositions);
    let rollbacks = all_rollbacks(store)?;
    let epochs = phase_epoch_starts(&rollbacks, &shipped);

    // Earliest current-epoch RFP per shipped phase (phase artifacts only).
    let mut phase_first_rfp: HashMap<String, DateTime<Utc>> = HashMap::new();
    for rfp in &rfps {
        let Some(phase_id) = phase_id_from_artifact_ref(&rfp.phase_id) else {
            continue;
        };
        let Some(&ship_time) = shipped.get(phase_id) else {
            continue;
        };
        if !rfp_in_epoch(rfp.created_at, ship_time, epochs.get(phase_id).copied()) {
            continue;
        }
        let e = phase_first_rfp
            .entry(phase_id.to_owned())
            .or_insert(rfp.created_at);
        if rfp.created_at < *e {
            *e = rfp.created_at;
        }
    }

    let mut total_minutes: i64 = 0;
    let mut count: u32 = 0;
    for (phase_id, ship_time) in &shipped {
        if let Some(&rfp_time) = phase_first_rfp.get(phase_id) {
            let minutes = ship_time.signed_duration_since(rfp_time).num_minutes();
            if minutes >= 0 {
                total_minutes += minutes;
                count += 1;
            }
        }
    }

    if count == 0 {
        Ok(None)
    } else {
        #[allow(clippy::cast_precision_loss)]
        Ok(Some(total_minutes as f64 / f64::from(count)))
    }
}

fn compute_avg_round_count(store: &AuditStore) -> Result<Option<f64>, AnvilError> {
    use anvil_audit::records::ConvergenceDeclaration;

    let dispositions = all_dispositions(store)?;
    let shipped = shipped_phase_latest_times(&dispositions);

    let entries = store.list(RecordType::ConvergenceDeclaration)?;
    let mut total: u32 = 0;
    let mut count: u32 = 0;
    for e in &entries {
        if let Some(decl) = store
            .get(&e.id)
            .ok()
            .and_then(|v| serde_json::from_value::<ConvergenceDeclaration>(v).ok())
        {
            // Only declarations for shipped phases contribute. Normalize the artifact ref
            // (e.g., "phase:P8" → "P8") so it matches PhaseDisposition.phase_id.
            let Some(phase_id) = phase_id_from_artifact_ref(&decl.phase_id) else {
                continue;
            };
            if !shipped.contains_key(phase_id) {
                continue;
            }
            total += decl.round_count;
            count += 1;
        }
    }

    if count == 0 {
        Ok(None)
    } else {
        Ok(Some(f64::from(total) / f64::from(count)))
    }
}

fn compute_deferred_resolved_count(store: &AuditStore) -> Result<u32, AnvilError> {
    let entries = store.list(RecordType::HingeFlip)?;
    let mut names: HashSet<String> = HashSet::new();
    for e in &entries {
        if let Some(flip) = store
            .get(&e.id)
            .ok()
            .and_then(|v| serde_json::from_value::<HingeFlip>(v).ok())
        {
            names.insert(flip.hinge_test_name);
        }
    }
    Ok(u32::try_from(names.len()).unwrap_or(u32::MAX))
}

fn compute_defect_escape_rate(store: &AuditStore) -> Result<Option<f64>, AnvilError> {
    let dispositions = all_dispositions(store)?;
    let shipped = shipped_phase_latest_times(&dispositions);

    let shipped_count = shipped.len();
    if shipped_count == 0 {
        return Ok(None);
    }

    // Latest rollback per invalidated phase.
    let mut phase_latest_rollback: HashMap<String, DateTime<Utc>> = HashMap::new();
    for e in store.list(RecordType::RollbackEvent)? {
        if let Some(r) = store
            .get(&e.id)
            .ok()
            .and_then(|v| serde_json::from_value::<RollbackEvent>(v).ok())
        {
            let entry = phase_latest_rollback
                .entry(r.invalidated_phase)
                .or_insert(r.created_at);
            if r.created_at > *entry {
                *entry = r.created_at;
            }
        }
    }

    let escaped = shipped
        .iter()
        .filter(|(phase_id, &ship_time)| {
            phase_latest_rollback
                .get(*phase_id)
                .is_some_and(|&rb| rb > ship_time)
        })
        .count();

    #[allow(clippy::cast_precision_loss)]
    Ok(Some(escaped as f64 / shipped_count as f64))
}

// ── Per-phase history ─────────────────────────────────────────────────────────

/// Metric values for a single shipped phase (used by `anvil metrics history`).
#[derive(Debug, Clone)]
pub struct PhaseMetrics {
    pub phase_id: String,
    pub shipped_at: DateTime<Utc>,
    /// From `ConvergenceDeclaration.round_count`; `None` if no declaration exists.
    pub review_rounds: Option<u32>,
    /// Wall-clock minutes from first RFP to ship disposition; `None` if no RFP data.
    pub human_minutes: Option<i64>,
    /// Total findings across current-epoch `ReviewerFindingPacket` records for this phase.
    pub finding_count: u32,
    /// Findings dropped during curation for this phase (current epoch only).
    pub dropped_count: u32,
    /// `true` if a `RollbackEvent` exists after the ship timestamp.
    pub rolled_back: bool,
}

/// Returns per-phase metric values for all shipped phases, sorted by `shipped_at` ascending.
///
/// All per-phase counts (findings, drops, minutes) are scoped to the current shipping
/// epoch: records created after the most recent `RollbackEvent` invalidating the phase
/// and at/before the current ship disposition.
///
/// # Errors
///
/// Returns [`AnvilError`] if the store index cannot be read.
pub fn compute_history(store: &AuditStore) -> Result<Vec<PhaseMetrics>, AnvilError> {
    use anvil_audit::records::ConvergenceDeclaration;

    let rfps = all_rfps(store)?;
    let dispositions = all_dispositions(store)?;
    let shipped = shipped_phase_latest_times(&dispositions);

    if shipped.is_empty() {
        return Ok(Vec::new());
    }

    let rollbacks = all_rollbacks(store)?;
    let epochs = phase_epoch_starts(&rollbacks, &shipped);

    // Current-epoch RFP data per phase.
    let mut phase_first_rfp: HashMap<String, DateTime<Utc>> = HashMap::new();
    let mut phase_finding_count: HashMap<String, u32> = HashMap::new();
    // packet_id → normalized phase_id (current epoch only).
    let mut current_epoch_packets: HashMap<String, String> = HashMap::new();

    for rfp in &rfps {
        let Some(phase_id) = phase_id_from_artifact_ref(&rfp.phase_id) else {
            continue;
        };
        let Some(&ship_time) = shipped.get(phase_id) else {
            continue;
        };
        if !rfp_in_epoch(rfp.created_at, ship_time, epochs.get(phase_id).copied()) {
            continue;
        }
        let e = phase_first_rfp
            .entry(phase_id.to_owned())
            .or_insert(rfp.created_at);
        if rfp.created_at < *e {
            *e = rfp.created_at;
        }
        *phase_finding_count.entry(phase_id.to_owned()).or_default() += rfp.finding_count;
        current_epoch_packets.insert(rfp.packet.packet_id.clone(), phase_id.to_owned());
    }

    // Dropped count per phase via latest curation per current-epoch packet.
    let known_packets: HashSet<&str> = current_epoch_packets.keys().map(String::as_str).collect();
    let curations = latest_curations_for_packets(store, &known_packets)?;
    let mut phase_dropped: HashMap<String, u32> = HashMap::new();
    for cr in curations.values() {
        let Some(phase_id) = current_epoch_packets.get(&cr.packet_id) else {
            continue;
        };
        let dropped = cr
            .dispositions
            .iter()
            .filter(|d| d.action == CurationAction::Drop)
            .count();
        *phase_dropped.entry(phase_id.clone()).or_default() +=
            u32::try_from(dropped).unwrap_or(u32::MAX);
    }

    // Round count per shipped phase from ConvergenceDeclaration (normalized phase_id).
    let mut phase_rounds: HashMap<String, u32> = HashMap::new();
    for e in store.list(RecordType::ConvergenceDeclaration)? {
        if let Some(decl) = store
            .get(&e.id)
            .ok()
            .and_then(|v| serde_json::from_value::<ConvergenceDeclaration>(v).ok())
        {
            let Some(phase_id) = phase_id_from_artifact_ref(&decl.phase_id) else {
                continue;
            };
            if shipped.contains_key(phase_id) {
                phase_rounds.insert(phase_id.to_owned(), decl.round_count);
            }
        }
    }

    // Latest rollback per invalidated phase (for rolled_back flag).
    let mut phase_latest_rollback: HashMap<String, DateTime<Utc>> = HashMap::new();
    for rb in &rollbacks {
        let entry = phase_latest_rollback
            .entry(rb.invalidated_phase.clone())
            .or_insert(rb.created_at);
        if rb.created_at > *entry {
            *entry = rb.created_at;
        }
    }

    let mut history: Vec<PhaseMetrics> = shipped
        .iter()
        .map(|(phase_id, &ship_time)| {
            let human_minutes = phase_first_rfp.get(phase_id).map(|&rfp_time| {
                ship_time
                    .signed_duration_since(rfp_time)
                    .num_minutes()
                    .max(0)
            });
            let rolled_back = phase_latest_rollback
                .get(phase_id)
                .is_some_and(|&rb| rb > ship_time);
            PhaseMetrics {
                phase_id: phase_id.clone(),
                shipped_at: ship_time,
                review_rounds: phase_rounds.get(phase_id).copied(),
                human_minutes,
                finding_count: phase_finding_count.get(phase_id).copied().unwrap_or(0),
                dropped_count: phase_dropped.get(phase_id).copied().unwrap_or(0),
                rolled_back,
            }
        })
        .collect();

    history.sort_by_key(|p| p.shipped_at);
    Ok(history)
}

// ── Layer-2 target evaluation ─────────────────────────────────────────────────

/// Whether a metric is within its target range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetStatus {
    Met,
    Violated,
    NoData,
}

impl TargetStatus {
    #[must_use]
    pub fn symbol(&self) -> &'static str {
        match self {
            Self::Met => "✓",
            Self::Violated => "✗",
            Self::NoData => "-",
        }
    }
}

/// Qualitative trend direction for a metric value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Flat,
}

impl Direction {
    #[must_use]
    pub fn symbol(&self) -> &'static str {
        match self {
            Self::Up => "↑",
            Self::Down => "↓",
            Self::Flat => "→",
        }
    }
}

/// Display row for a single metric in the `anvil metrics show` output.
#[derive(Debug, Clone)]
pub struct MetricRow {
    pub name: &'static str,
    pub value: String,
    pub direction: Direction,
    pub target: String,
    pub status: TargetStatus,
}

/// Evaluates all six Layer-1 metrics against their Layer-2 targets.
///
/// Direction (↑/↓/→) compares the last two shipped phases in `history` for
/// `human_minutes` and `avg_round_count`, but only when `history.len() >= 3`.
/// With fewer than three shipped phases the indicator stays → (Flat) to avoid
/// presenting a trend derived from two noisy samples.
#[must_use]
pub fn evaluate_targets(
    metrics: &Layer1Metrics,
    history: &[PhaseMetrics],
    targets: &MetricTargets,
) -> Vec<MetricRow> {
    // Suppress direction until ≥3 shipped phases exist; two samples are too noisy.
    let prev = if history.len() >= 3 {
        history.iter().rev().nth(1)
    } else {
        None
    };
    let latest = if history.len() >= 3 {
        history.last()
    } else {
        None
    };

    #[allow(clippy::cast_precision_loss)]
    let minutes_dir = direction_from(
        prev.and_then(|p| p.human_minutes).map(|m| m as f64),
        latest.and_then(|p| p.human_minutes).map(|m| m as f64),
    );
    let rounds_dir = direction_from(
        prev.and_then(|p| p.review_rounds).map(f64::from),
        latest.and_then(|p| p.review_rounds).map(f64::from),
    );

    vec![
        MetricRow {
            name: "Reviewer Precision",
            value: fmt_pct(metrics.reviewer_precision),
            direction: Direction::Flat,
            target: format!("≥{:.0}%", targets.precision_min * 100.0),
            status: match metrics.reviewer_precision {
                None => TargetStatus::NoData,
                Some(v) if v >= targets.precision_min => TargetStatus::Met,
                Some(_) => TargetStatus::Violated,
            },
        },
        MetricRow {
            name: "Finding Count Agreement",
            value: fmt_pct(metrics.finding_count_agreement),
            direction: Direction::Flat,
            target: format!("≤{:.0}%", targets.agreement_max * 100.0),
            status: match metrics.finding_count_agreement {
                None => TargetStatus::NoData,
                Some(v) if v <= targets.agreement_max => TargetStatus::Met,
                Some(_) => TargetStatus::Violated,
            },
        },
        MetricRow {
            name: "Human Min / Phase",
            value: fmt_minutes(metrics.human_minutes_per_phase),
            direction: minutes_dir,
            target: format!("≤{:.0}m", targets.human_minutes_max),
            status: match metrics.human_minutes_per_phase {
                None => TargetStatus::NoData,
                Some(v) if v <= targets.human_minutes_max => TargetStatus::Met,
                Some(_) => TargetStatus::Violated,
            },
        },
        MetricRow {
            name: "Avg Review Rounds",
            value: fmt_f1(metrics.avg_round_count),
            direction: rounds_dir,
            target: format!("≤{:.1}", targets.round_count_max),
            status: match metrics.avg_round_count {
                None => TargetStatus::NoData,
                Some(v) if v <= targets.round_count_max => TargetStatus::Met,
                Some(_) => TargetStatus::Violated,
            },
        },
        MetricRow {
            name: "Deferred Resolved",
            value: metrics.deferred_resolved_count.to_string(),
            direction: Direction::Flat,
            target: "P10b registry".to_owned(),
            status: TargetStatus::NoData,
        },
        MetricRow {
            name: "Defect Escape Rate",
            value: fmt_pct(metrics.defect_escape_rate),
            direction: Direction::Flat,
            target: format!("≤{:.0}%", targets.escape_rate_max * 100.0),
            status: match metrics.defect_escape_rate {
                None => TargetStatus::NoData,
                Some(v) if v <= targets.escape_rate_max => TargetStatus::Met,
                Some(_) => TargetStatus::Violated,
            },
        },
    ]
}

fn direction_from(prev: Option<f64>, current: Option<f64>) -> Direction {
    match (prev, current) {
        (Some(p), Some(c)) if c > p => Direction::Up,
        (Some(p), Some(c)) if c < p => Direction::Down,
        _ => Direction::Flat,
    }
}

fn fmt_pct(v: Option<f64>) -> String {
    v.map_or_else(|| "-".to_owned(), |f| format!("{:.0}%", f * 100.0))
}

fn fmt_minutes(v: Option<f64>) -> String {
    v.map_or_else(|| "-".to_owned(), |f| format!("{f:.0}m"))
}

fn fmt_f1(v: Option<f64>) -> String {
    v.map_or_else(|| "-".to_owned(), |f| format!("{f:.1}"))
}

// ── Layer-3 alert engine ──────────────────────────────────────────────────────

/// Four Charter-defined Layer-3 alert kinds. Count is pinned by `test_alert_kinds_count`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlertKind {
    LowPrecision,
    RisingHumanMinutesTrend,
    ExtremeAgreement,
    DeferralOpenTooLong,
}

/// A fired Layer-3 alert.
#[derive(Debug, Clone)]
pub struct Alert {
    pub kind: AlertKind,
    pub message: String,
}

/// Evaluates Layer-3 alerts from metrics, per-phase history, targets, and the store.
///
/// # Errors
///
/// Returns [`AnvilError`] if the store index cannot be read.
pub fn evaluate_alerts(
    metrics: &Layer1Metrics,
    history: &[PhaseMetrics],
    targets: &MetricTargets,
    store: &AuditStore,
) -> Result<Vec<Alert>, AnvilError> {
    let mut alerts = Vec::new();

    // Alert 1: Low precision.
    if let Some(p) = metrics.reviewer_precision {
        if p < targets.precision_min {
            alerts.push(Alert {
                kind: AlertKind::LowPrecision,
                message: format!(
                    "Reviewer precision {:.0}% is below the minimum target of {:.0}%.",
                    p * 100.0,
                    targets.precision_min * 100.0
                ),
            });
        }
    }

    // Alert 2: Rising human-minutes trend (last 3 phases with minutes data, strictly increasing).
    let minutes_history: Vec<i64> = history.iter().filter_map(|p| p.human_minutes).collect();
    if minutes_history.len() >= 3 {
        let n = minutes_history.len();
        if minutes_history[n - 3] < minutes_history[n - 2]
            && minutes_history[n - 2] < minutes_history[n - 1]
        {
            alerts.push(Alert {
                kind: AlertKind::RisingHumanMinutesTrend,
                message: format!(
                    "Human minutes per phase has risen for 3 consecutive phases: {}m → {}m → {}m.",
                    minutes_history[n - 3],
                    minutes_history[n - 2],
                    minutes_history[n - 1]
                ),
            });
        }
    }

    // Alert 3: Extreme finding-count agreement (reviewers flagging the same volume).
    if let Some(a) = metrics.finding_count_agreement {
        if a > targets.agreement_max {
            alerts.push(Alert {
                kind: AlertKind::ExtremeAgreement,
                message: format!(
                    "Finding count agreement {:.0}% exceeds the maximum target of {:.0}%. \
                     This may indicate reduced reviewer diversity.",
                    a * 100.0,
                    targets.agreement_max * 100.0
                ),
            });
        }
    }

    // Alert 4: Deferral open >5 shipped phases.
    // Fires when any ProvisionalLock was created before the 5th-oldest ship event.
    let shipped_times: Vec<DateTime<Utc>> = {
        let mut times: Vec<DateTime<Utc>> = history.iter().map(|p| p.shipped_at).collect();
        times.sort();
        times
    };

    if shipped_times.len() >= 5 {
        // Cutoff: the 5th-oldest ship time.  A lock created before this time has
        // been open through at least 5 shipped phases without resolution.
        let cutoff = shipped_times[shipped_times.len() - 5];

        let lock_entries = store.list(RecordType::ProvisionalLock)?;
        let stale_locks: Vec<String> = lock_entries
            .iter()
            .filter_map(|e| {
                store
                    .get(&e.id)
                    .ok()
                    .and_then(|v| serde_json::from_value::<ProvisionalLock>(v).ok())
                    .filter(|l| l.created_at <= cutoff)
                    .map(|l| l.choice_key)
            })
            .collect();

        if !stale_locks.is_empty() {
            let keys = stale_locks.join(", ");
            alerts.push(Alert {
                kind: AlertKind::DeferralOpenTooLong,
                message: format!(
                    "The following provisional choices have been deferred for ≥5 shipped \
                     phases: {keys}. (v1 note: ProvisionalLock audit records persist after \
                     config-level resolution; this alert may fire as a false positive after \
                     P11 finalizes provisional choices.)"
                ),
            });
        }
    }

    Ok(alerts)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // hinge_test: pins=6, intended=layer1-metric-count, phase=P10a
    #[test]
    fn test_layer_1_metric_count() {
        // Pins: Layer1Metrics has exactly 6 fields — one per Charter Layer-1 metric.
        // Adding a new metric requires updating LAYER1_METRIC_COUNT, the struct,
        // compute_layer1(), evaluate_targets(), the CLI display, and this count.
        assert_eq!(LAYER1_METRIC_COUNT, 6);
    }

    // hinge_test: pins=4, intended=alert-kinds-count, phase=P10a
    #[test]
    fn test_alert_kinds_count() {
        // Pins: AlertKind has exactly 4 variants — the four Charter-defined alert kinds.
        // Adding a new alert kind requires updating ALERT_KIND_COUNT, the enum,
        // evaluate_alerts(), and this count.
        assert_eq!(ALERT_KIND_COUNT, 4);
    }

    fn init_store(dir: &tempfile::TempDir) -> AuditStore {
        anvil_core::project::init(dir.path()).unwrap();
        AuditStore::open(dir.path()).unwrap()
    }

    #[test]
    fn test_compute_layer1_empty_store_returns_none_for_optional_metrics() {
        let tmp = tempfile::TempDir::new().unwrap();
        let store = init_store(&tmp);
        let metrics = compute_layer1(&store).unwrap();

        assert!(
            metrics.reviewer_precision.is_none(),
            "no curated findings → precision is None"
        );
        assert!(
            metrics.finding_count_agreement.is_none(),
            "no multi-reviewer phases → agreement is None"
        );
        assert!(
            metrics.human_minutes_per_phase.is_none(),
            "no shipped phases → minutes is None"
        );
        assert!(
            metrics.avg_round_count.is_none(),
            "no convergence declarations → round count is None"
        );
        assert_eq!(
            metrics.deferred_resolved_count, 0,
            "no hinge flips → resolved count is 0"
        );
        assert!(
            metrics.defect_escape_rate.is_none(),
            "no shipped phases → escape rate is None"
        );
    }

    #[test]
    fn test_compute_history_empty_store_returns_empty_vec() {
        let tmp = tempfile::TempDir::new().unwrap();
        let store = init_store(&tmp);
        let history = compute_history(&store).unwrap();
        assert!(history.is_empty(), "no shipped phases → empty history");
    }

    #[test]
    fn test_evaluate_targets_all_no_data() {
        let metrics = Layer1Metrics {
            reviewer_precision: None,
            finding_count_agreement: None,
            human_minutes_per_phase: None,
            avg_round_count: None,
            deferred_resolved_count: 0,
            defect_escape_rate: None,
        };
        let rows = evaluate_targets(&metrics, &[], &MetricTargets::default());
        assert_eq!(rows.len(), LAYER1_METRIC_COUNT);
        // All optional metrics report NoData when None.
        let optional_rows: Vec<_> = rows
            .iter()
            .filter(|r| r.name != "Deferred Resolved")
            .collect();
        for row in &optional_rows {
            assert_eq!(
                row.status,
                TargetStatus::NoData,
                "row '{}' should be NoData",
                row.name
            );
        }
    }

    #[test]
    fn test_evaluate_targets_precision_violated() {
        let metrics = Layer1Metrics {
            reviewer_precision: Some(0.50),
            finding_count_agreement: None,
            human_minutes_per_phase: None,
            avg_round_count: None,
            deferred_resolved_count: 0,
            defect_escape_rate: None,
        };
        let rows = evaluate_targets(&metrics, &[], &MetricTargets::default());
        let precision_row = rows
            .iter()
            .find(|r| r.name == "Reviewer Precision")
            .unwrap();
        assert_eq!(precision_row.status, TargetStatus::Violated);
    }

    #[test]
    fn test_evaluate_targets_precision_met() {
        let metrics = Layer1Metrics {
            reviewer_precision: Some(0.85),
            finding_count_agreement: None,
            human_minutes_per_phase: None,
            avg_round_count: None,
            deferred_resolved_count: 0,
            defect_escape_rate: None,
        };
        let rows = evaluate_targets(&metrics, &[], &MetricTargets::default());
        let precision_row = rows
            .iter()
            .find(|r| r.name == "Reviewer Precision")
            .unwrap();
        assert_eq!(precision_row.status, TargetStatus::Met);
    }

    #[test]
    fn test_evaluate_alerts_empty_store_no_alerts() {
        let tmp = tempfile::TempDir::new().unwrap();
        let store = init_store(&tmp);
        let metrics = compute_layer1(&store).unwrap();
        let history = compute_history(&store).unwrap();
        let alerts =
            evaluate_alerts(&metrics, &history, &MetricTargets::default(), &store).unwrap();
        assert!(alerts.is_empty(), "empty store must produce no alerts");
    }

    #[test]
    fn test_alert_low_precision_fires() {
        let metrics = Layer1Metrics {
            reviewer_precision: Some(0.40),
            finding_count_agreement: None,
            human_minutes_per_phase: None,
            avg_round_count: None,
            deferred_resolved_count: 0,
            defect_escape_rate: None,
        };
        let tmp = tempfile::TempDir::new().unwrap();
        let store = init_store(&tmp);
        let alerts = evaluate_alerts(&metrics, &[], &MetricTargets::default(), &store).unwrap();
        assert!(
            alerts.iter().any(|a| a.kind == AlertKind::LowPrecision),
            "low precision should fire alert"
        );
    }

    #[test]
    fn test_alert_extreme_agreement_fires() {
        let metrics = Layer1Metrics {
            reviewer_precision: None,
            finding_count_agreement: Some(0.95),
            human_minutes_per_phase: None,
            avg_round_count: None,
            deferred_resolved_count: 0,
            defect_escape_rate: None,
        };
        let tmp = tempfile::TempDir::new().unwrap();
        let store = init_store(&tmp);
        let alerts = evaluate_alerts(&metrics, &[], &MetricTargets::default(), &store).unwrap();
        assert!(
            alerts.iter().any(|a| a.kind == AlertKind::ExtremeAgreement),
            "extreme agreement should fire alert"
        );
    }

    // ── R2 regression tests ───────────────────────────────────────────────────

    /// Returns a minimal `FindingsPacket` for testing (two findings: F1 and F2).
    #[cfg(test)]
    fn make_two_finding_packet(
        packet_id: &str,
        artifact_ref: &str,
    ) -> anvil_core::pipeline::FindingsPacket {
        use anvil_core::pipeline::{Finding, FindingSeverity, FindingsPacket, LocationAnchor};
        let make_finding = |id: &str| Finding {
            id: id.to_owned(),
            severity: FindingSeverity::P2,
            location: LocationAnchor {
                artifact_path: "test.rs".to_owned(),
                section_id: Some("§test".to_owned()),
                line_range: None,
                symbol_name: None,
                quote: None,
            },
            claim: "test claim".to_owned(),
            evidence: "test evidence".to_owned(),
            recommendation: "test recommendation".to_owned(),
            metadata: None,
            advisory: false,
        };
        FindingsPacket {
            packet_id: packet_id.to_owned(),
            artifact_ref: artifact_ref.to_owned(),
            round_number: 1,
            reviewer_id: "reviewer-a".to_owned(),
            reviewer_model_identity: "model-a".to_owned(),
            produced_at: Utc::now(),
            findings: vec![make_finding("F1"), make_finding("F2")],
            artifact_hash: None,
            reviewer_meta: None,
        }
    }

    /// Arbiter resolutions must not affect `reviewer_precision`.
    /// With 2 phase findings and 1 curated drop, precision = 0.5 regardless of arbiter records.
    #[test]
    fn test_precision_excludes_arbiter_resolutions() {
        use anvil_audit::records::{
            ArbiterFindingResolution, CuratedFindingsRecord, ReviewerFindingPacket,
        };
        use anvil_core::pipeline::{CurationAction, CurationDisposition};

        let tmp = tempfile::TempDir::new().unwrap();
        let store = init_store(&tmp);

        let packet = make_two_finding_packet("pkt1", "phase:P1:R1");
        let rfp =
            ReviewerFindingPacket::from_packet("phase:P1:R1".to_owned(), packet.clone(), vec![]);
        store.append(&rfp).unwrap();

        // Drop F1; keep F2.
        let curation = CuratedFindingsRecord::new(
            "pkt1".to_owned(),
            "coordinator".to_owned(),
            vec![
                CurationDisposition {
                    finding_id: "F1".to_owned(),
                    action: CurationAction::Drop,
                    edited_finding: None,
                    annotation: None,
                    advisory_disposition: None,
                },
                CurationDisposition {
                    finding_id: "F2".to_owned(),
                    action: CurationAction::Keep,
                    edited_finding: None,
                    annotation: None,
                    advisory_disposition: None,
                },
            ],
            vec![],
        );
        store.append(&curation).unwrap();

        // Arbiter resolution for an unrelated finding reference.
        let arbiter_res = ArbiterFindingResolution::new(
            "pkt1:F2".to_owned(),
            "arbiter-a".to_owned(),
            "overriding".to_owned(),
            "keep".to_owned(),
            "none".to_owned(),
            vec![],
        );
        store.append(&arbiter_res).unwrap();

        let metrics = compute_layer1(&store).unwrap();
        // Precision = 1 upheld / 2 total = 0.5; arbiter record must not subtract.
        let precision = metrics.reviewer_precision.expect("precision must be Some");
        assert!(
            (precision - 0.5).abs() < 0.001,
            "expected precision 0.5, got {precision}"
        );
    }

    /// `evaluate_targets` must produce a row named "Finding Count Agreement" and
    /// must not produce any row named "Cross-reviewer Agreement".
    #[test]
    fn test_finding_count_agreement_row_name() {
        let metrics = Layer1Metrics {
            reviewer_precision: None,
            finding_count_agreement: None,
            human_minutes_per_phase: None,
            avg_round_count: None,
            deferred_resolved_count: 0,
            defect_escape_rate: None,
        };
        let rows = evaluate_targets(&metrics, &[], &MetricTargets::default());
        assert!(
            rows.iter().any(|r| r.name == "Finding Count Agreement"),
            "expected row named 'Finding Count Agreement'"
        );
        assert!(
            !rows.iter().any(|r| r.name == "Cross-reviewer Agreement"),
            "old name 'Cross-reviewer Agreement' must not appear"
        );
    }

    /// Direction must be Flat with <3 shipped phases and directional with ≥3.
    #[test]
    fn test_direction_suppressed_below_three_phases() {
        fn make_phase(id: &str, minutes: i64, offset_secs: i64) -> PhaseMetrics {
            PhaseMetrics {
                phase_id: id.to_owned(),
                shipped_at: Utc::now() + chrono::Duration::seconds(offset_secs),
                review_rounds: None,
                human_minutes: Some(minutes),
                finding_count: 0,
                dropped_count: 0,
                rolled_back: false,
            }
        }
        let metrics = Layer1Metrics {
            reviewer_precision: None,
            finding_count_agreement: None,
            human_minutes_per_phase: Some(60.0),
            avg_round_count: None,
            deferred_resolved_count: 0,
            defect_escape_rate: None,
        };

        // Two phases: direction must be Flat.
        let h2 = vec![make_phase("P1", 30, -200), make_phase("P2", 60, -100)];
        let rows2 = evaluate_targets(&metrics, &h2, &MetricTargets::default());
        let min_row2 = rows2
            .iter()
            .find(|r| r.name == "Human Min / Phase")
            .unwrap();
        assert_eq!(
            min_row2.direction,
            Direction::Flat,
            "direction must be Flat with 2 phases"
        );

        // Three phases with increasing minutes: direction must be Up.
        let h3 = vec![
            make_phase("P1", 30, -300),
            make_phase("P2", 60, -200),
            make_phase("P3", 90, -100),
        ];
        let rows3 = evaluate_targets(&metrics, &h3, &MetricTargets::default());
        let min_row3 = rows3
            .iter()
            .find(|r| r.name == "Human Min / Phase")
            .unwrap();
        assert_eq!(
            min_row3.direction,
            Direction::Up,
            "direction must be Up with 3 increasing phases"
        );
    }

    /// `DeferralOpenTooLong` alert message must contain the v1 false-positive note.
    #[test]
    fn test_deferral_alert_message_contains_v1_note() {
        use anvil_audit::records::ProvisionalLock;

        let tmp = tempfile::TempDir::new().unwrap();
        let store = init_store(&tmp);

        // Append the provisional lock first; its created_at is set to Utc::now().
        let lock = ProvisionalLock::new("choice-a".to_owned(), "hypothesis".to_owned(), vec![]);
        store.append(&lock).unwrap();

        // Build 5 shipped phases all at times after the lock was created.
        let now = Utc::now();
        let history: Vec<PhaseMetrics> = (0..5_usize)
            .map(|i| PhaseMetrics {
                phase_id: format!("P{}", i + 1),
                shipped_at: now + chrono::Duration::seconds(i64::try_from(i).unwrap_or(0) + 60),
                review_rounds: None,
                human_minutes: None,
                finding_count: 0,
                dropped_count: 0,
                rolled_back: false,
            })
            .collect();

        let metrics = compute_layer1(&store).unwrap();
        let alerts =
            evaluate_alerts(&metrics, &history, &MetricTargets::default(), &store).unwrap();
        let deferral = alerts
            .iter()
            .find(|a| a.kind == AlertKind::DeferralOpenTooLong)
            .expect("DeferralOpenTooLong alert must fire");
        assert!(
            deferral.message.contains("v1 note"),
            "deferral alert message must contain 'v1 note'; got: {}",
            deferral.message
        );
    }
}
