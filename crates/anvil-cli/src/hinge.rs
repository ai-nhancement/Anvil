use std::path::Path;

use anvil_audit::records::HingeFlip;
use anvil_audit::{AuditStore, RecordType};
use anvil_core::error::AnvilError;
use anvil_hinge::{scan_workspace, HingeSource};

pub fn run_hinge_list(project: &Path, strict: bool, count_only: bool) -> Result<(), AnvilError> {
    let registry = scan_workspace(project)?;

    let total = registry.entries.len() + registry.alternatives.len();

    if count_only {
        println!("{total}");
        return Ok(());
    }

    // Read flip history to determine which entries have been flipped.
    let store = AuditStore::open(project)?;
    let flipped: std::collections::HashSet<String> = store
        .list(RecordType::HingeFlip)?
        .iter()
        .filter_map(|e| {
            store
                .get(&e.id)
                .ok()
                .and_then(|v| serde_json::from_value::<HingeFlip>(v).ok())
        })
        .map(|f| f.hinge_test_name)
        .collect();

    if total == 0 {
        println!("No hinge entries found.");
        return Ok(());
    }

    println!(
        "  {:<40}  {:<24}  {:<6}  {:<4}  STATUS",
        "INTENDED", "PINS", "PHASE", "LANG"
    );
    println!("{}", "─".repeat(88));

    for entry in &registry.entries {
        let lang = match entry.source {
            HingeSource::Rust => "Rust",
            HingeSource::Go => "Go",
        };
        let status = if flipped.contains(&entry.intended) {
            "FLIPPED"
        } else {
            "OPEN"
        };
        println!(
            "  {:<40}  {:<24}  {:<6}  {:<4}  {}",
            entry.intended, entry.pins, entry.phase, lang, status
        );
    }

    for alt in &registry.alternatives {
        println!(
            "  {:<40}  {:<24}  {:<6}  {:<4}  OPEN (alt: {})",
            alt.intended, alt.pins, alt.phase, "ALT", alt.mechanism
        );
    }

    if strict {
        let violations = registry.consensus_violations();
        if !violations.is_empty() {
            eprintln!();
            eprintln!(
                "[BLOCK-SHIP] Cross-language consensus violations ({}):",
                violations.len()
            );
            for v in &violations {
                eprintln!("  {} — {}", v.intended, v.reason);
            }
            std::process::exit(1);
        }
    }

    Ok(())
}

pub fn run_hinge_flip(
    project: &Path,
    id: &str,
    new_value: &str,
    reason: &str,
) -> Result<(), AnvilError> {
    if reason.trim().is_empty() {
        return Err(AnvilError::EmptyReasoning {
            command: "hinge flip",
        });
    }

    let registry = scan_workspace(project)?;

    let (old_value, hinge_test_name) = registry
        .entries
        .iter()
        .find(|e| e.intended == id)
        .map(|e| (e.pins.clone(), e.intended.clone()))
        .or_else(|| {
            registry
                .alternatives
                .iter()
                .find(|a| a.intended == id)
                .map(|a| (a.pins.clone(), a.intended.clone()))
        })
        .ok_or_else(|| AnvilError::RecordNotFound { id: id.to_owned() })?;

    let record = HingeFlip::new(
        hinge_test_name,
        old_value.clone(),
        new_value.to_owned(),
        Vec::new(),
    );
    let store = AuditStore::open(project)?;
    store.append(&record)?;

    println!("Flipped '{id}': {old_value} → {new_value}");
    println!("Reason: {reason}");
    Ok(())
}
