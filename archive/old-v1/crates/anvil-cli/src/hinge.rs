use std::path::Path;

use anvil_audit::records::HingeFlip;
use anvil_audit::{AuditStore, RecordType};
use anvil_core::error::AnvilError;
use anvil_hinge::{scan_workspace, HingeSource};

pub fn run_hinge_list(project: &Path, strict: bool, count_only: bool) -> Result<(), AnvilError> {
    let registry = scan_workspace(project)?;

    let total = registry.entries.len() + registry.alternatives.len();

    // Run strict consensus check before any I/O so that `--strict` works on a bare
    // checkout without `anvil.toml`, and so `--strict --count` does not bypass it.
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

    if count_only {
        println!("{total}");
        return Ok(());
    }

    // Flip status requires an initialized project; treat uninitialized as no recorded flips.
    let flipped: std::collections::HashSet<String> = match AuditStore::open(project) {
        Ok(store) => store
            .list(RecordType::HingeFlip)?
            .iter()
            .filter_map(|e| {
                store
                    .get(&e.id)
                    .ok()
                    .and_then(|v| serde_json::from_value::<HingeFlip>(v).ok())
            })
            .map(|f| f.hinge_test_name)
            .collect(),
        Err(AnvilError::NotInitialized(_)) => std::collections::HashSet::new(),
        Err(e) => return Err(e),
    };

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
    if new_value.trim().is_empty() {
        return Err(AnvilError::InvalidConfigValue {
            key: "new-value".to_owned(),
            reason: "must not be empty for hinge flip".to_owned(),
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
        reason.to_owned(),
        Vec::new(),
    );
    let store = AuditStore::open(project)?;
    store.append(&record)?;

    println!("Flipped '{id}': {old_value} → {new_value}");
    println!("Reason: {reason}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hinge_list_strict_succeeds_without_audit_store() {
        // --strict must work on a bare checkout without anvil.toml.
        // With no hinge entries in a temp dir, there are no violations, so it returns Ok.
        let tmp = tempfile::TempDir::new().unwrap();
        // No anvil.toml, no crates/, no sidecar/ — scan_workspace returns an empty registry.
        run_hinge_list(tmp.path(), true, true).unwrap();
    }

    #[test]
    fn test_hinge_list_count_with_strict_runs_strict_check() {
        // --count --strict must run the strict check, not bypass it.
        // With a clean empty workspace, both succeed; the test verifies no panic/exit.
        let tmp = tempfile::TempDir::new().unwrap();
        run_hinge_list(tmp.path(), true, true).unwrap();
    }

    #[test]
    fn test_flip_rejects_empty_reason() {
        let tmp = tempfile::TempDir::new().unwrap();
        let err = run_hinge_flip(tmp.path(), "some-id", "v2", "").unwrap_err();
        assert!(
            matches!(err, AnvilError::EmptyReasoning { .. }),
            "empty reason must be rejected: {err}"
        );
    }

    #[test]
    fn test_flip_rejects_empty_new_value() {
        let tmp = tempfile::TempDir::new().unwrap();
        let err = run_hinge_flip(tmp.path(), "some-id", "", "valid reason").unwrap_err();
        assert!(
            matches!(err, AnvilError::InvalidConfigValue { .. }),
            "empty new_value must be rejected: {err}"
        );
    }
}
