//! Recovery-only filesystem maintenance for durable project generations.
//!
//! This module deliberately knows nothing about SQLite or datasets. The
//! catalog layer supplies the set of referenced generation names, and this
//! module only removes stale, unreferenced generation directories.

use std::{
    collections::HashSet,
    fs,
    path::Path,
    time::{Duration, SystemTime},
};

const GENERATION_ID_LENGTH: usize = 32;

#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct ReconciliationReport {
    pub(crate) removed: usize,
    pub(crate) failures: usize,
}

fn is_generation_name(name: &str) -> bool {
    let Some((project_id, generation_id)) = name.split_once('-') else {
        return false;
    };
    project_id.len() == GENERATION_ID_LENGTH
        && generation_id.len() == GENERATION_ID_LENGTH
        && project_id
            .bytes()
            .chain(generation_id.bytes())
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

#[cfg(windows)]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_metadata: &fs::Metadata) -> bool {
    false
}

/// Removes only generation directories that are old enough and absent from
/// the catalog. Failures are counted without returning filesystem paths.
#[cfg(test)]
pub(crate) fn reconcile_orphan_generations(
    snapshots: &Path,
    active_generations: &HashSet<String>,
    now: SystemTime,
    grace: Duration,
) -> ReconciliationReport {
    reconcile_orphan_generations_with_cancel(snapshots, active_generations, now, grace, || false)
        .unwrap_or_default()
}

pub(crate) fn reconcile_orphan_generations_with_cancel<C>(
    snapshots: &Path,
    active_generations: &HashSet<String>,
    now: SystemTime,
    grace: Duration,
    is_cancelled: C,
) -> Result<ReconciliationReport, ()>
where
    C: Fn() -> bool,
{
    let mut report = ReconciliationReport::default();
    if is_cancelled() {
        return Err(());
    }
    let entries = match fs::read_dir(snapshots) {
        Ok(entries) => entries,
        Err(_) if is_cancelled() => return Err(()),
        Err(_) => return Ok(report),
    };
    if is_cancelled() {
        return Err(());
    }

    for entry in entries {
        if is_cancelled() {
            return Err(());
        }
        let Ok(entry) = entry else {
            report.failures += 1;
            continue;
        };
        let name = entry.file_name().to_string_lossy().into_owned();
        if !is_generation_name(&name) || active_generations.contains(&name) {
            continue;
        }
        let metadata = match fs::symlink_metadata(entry.path()) {
            Ok(metadata) => metadata,
            Err(_) if is_cancelled() => return Err(()),
            Err(_) => {
                report.failures += 1;
                continue;
            }
        };
        if is_cancelled() {
            return Err(());
        }
        if !metadata.is_dir() || metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
            report.failures += 1;
            continue;
        }
        let old_enough = metadata
            .modified()
            .ok()
            .and_then(|modified| now.duration_since(modified).ok())
            .is_some_and(|age| age >= grace);
        if !old_enough {
            continue;
        }
        if is_cancelled() {
            return Err(());
        }
        // Keep recursive removal in the standard library. A manual traversal
        // by path could race with a symlink or reparse-point replacement.
        // The operation itself is synchronous, so cancellation is observed
        // immediately before and after this call.
        match fs::remove_dir_all(entry.path()) {
            Ok(()) => report.removed += 1,
            Err(_) => report.failures += 1,
        }
        if is_cancelled() {
            return Err(());
        }
    }
    if is_cancelled() {
        return Err(());
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_only_stale_unreferenced_generations() {
        let root = tempfile::tempdir().unwrap();
        let active_name = "0123456789abcdef0123456789abcdef-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let orphan_name = "0123456789abcdef0123456789abcdef-bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
        let staging_name = "staging-not-a-generation";
        for name in [active_name, orphan_name, staging_name] {
            fs::create_dir(root.path().join(name)).unwrap();
        }
        let active = HashSet::from([active_name.to_owned()]);
        let report =
            reconcile_orphan_generations(root.path(), &active, SystemTime::now(), Duration::ZERO);
        assert_eq!(
            report,
            ReconciliationReport {
                removed: 1,
                failures: 0
            }
        );
        assert!(root.path().join(active_name).is_dir());
        assert!(!root.path().join(orphan_name).exists());
        assert!(root.path().join(staging_name).is_dir());
    }
}
