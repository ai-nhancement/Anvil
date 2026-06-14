use std::path::Path;

use anvil_audit::AuditStore;
use anvil_core::{config::load_config, error::AnvilError};
use anvil_eval::{
    compute_history, compute_layer1, evaluate_alerts, evaluate_targets, AlertKind, TargetStatus,
};

pub fn run_metrics_show(project: &Path) -> Result<(), AnvilError> {
    let config = load_config(project)?;
    let store = AuditStore::open(project)?;
    let metrics = compute_layer1(&store)?;
    let history = compute_history(&store)?;
    let rows = evaluate_targets(&metrics, &history, &config.metric_targets);
    let alerts = evaluate_alerts(&metrics, &history, &config.metric_targets, &store)?;

    println!("Layer-1 Metrics");
    println!("{}", "─".repeat(62));

    for row in &rows {
        let status_sym = row.status.symbol();
        let dir_sym = row.direction.symbol();
        println!(
            "  {:<28}  {:>6}  {}  {:<14}  {}",
            row.name, row.value, dir_sym, row.target, status_sym
        );
    }

    let violated: Vec<_> = rows
        .iter()
        .filter(|r| r.status == TargetStatus::Violated)
        .collect();
    if !violated.is_empty() {
        println!();
        for row in &violated {
            println!("  [WARN] {} is outside its target range.", row.name);
        }
    }

    println!();
    println!("Alerts");
    println!("{}", "─".repeat(62));

    if alerts.is_empty() {
        println!("  None.");
    } else {
        for alert in &alerts {
            let tag = match alert.kind {
                AlertKind::LowPrecision => "[LOW-PRECISION]",
                AlertKind::RisingHumanMinutesTrend => "[RISING-MINUTES]",
                AlertKind::ExtremeAgreement => "[EXTREME-AGREEMENT]",
                AlertKind::DeferralOpenTooLong => "[STALE-DEFERRAL]",
            };
            println!("  {tag} {}", alert.message);
        }
    }

    if history.is_empty() {
        println!();
        println!("  No shipped phases yet. Metrics will populate as phases ship.");
    }

    Ok(())
}

pub fn run_metrics_history(project: &Path) -> Result<(), AnvilError> {
    let store = AuditStore::open(project)?;
    let history = compute_history(&store)?;

    if history.is_empty() {
        println!("No shipped phases yet.");
        return Ok(());
    }

    println!(
        "{:<12}  {:>10}  {:>8}  {:>8}  {:>8}  Rolled Back",
        "Phase", "Shipped", "Rounds", "Minutes", "Findings"
    );
    println!("{}", "─".repeat(68));

    for phase in &history {
        let shipped = phase.shipped_at.format("%Y-%m-%d").to_string();
        let rounds = phase
            .review_rounds
            .map_or_else(|| "-".to_owned(), |r| r.to_string());
        let minutes = phase
            .human_minutes
            .map_or_else(|| "-".to_owned(), |m| format!("{m}m"));
        let rolled = if phase.rolled_back { "yes" } else { "no" };

        println!(
            "{:<12}  {:>10}  {:>8}  {:>8}  {:>8}  {}",
            phase.phase_id, shipped, rounds, minutes, phase.finding_count, rolled
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn init_project(dir: &tempfile::TempDir) -> AuditStore {
        anvil_core::project::init(dir.path()).unwrap();
        AuditStore::open(dir.path()).unwrap()
    }

    #[test]
    fn test_metrics_show_empty_project_succeeds() {
        let tmp = tempfile::TempDir::new().unwrap();
        init_project(&tmp);
        run_metrics_show(tmp.path()).unwrap();
    }

    #[test]
    fn test_metrics_history_empty_project_succeeds() {
        let tmp = tempfile::TempDir::new().unwrap();
        init_project(&tmp);
        run_metrics_history(tmp.path()).unwrap();
    }
}
