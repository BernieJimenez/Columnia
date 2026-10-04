//! ARQ-02: a damaged secondary catalog never keeps the app from opening.
//!
//! Reusable tasks and delivery presets live in their own SQLite files. When
//! one cannot be opened (corrupt, written by a newer version, unreadable),
//! the file and its WAL/SHM companions are moved aside, never deleted, and a
//! new empty catalog takes its place; Cargar then says where the old one is.

use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

/// Runs `initialize`; if it fails while the catalog file exists, moves the
/// file aside as `<name>.unreadable-<seconds>` and tries once more. Returns
/// the state and, when the file was moved, its new path.
pub fn initialize_or_set_aside<T>(
    catalog: &Path,
    initialize: impl Fn() -> Result<T, String>,
) -> Result<(T, Option<PathBuf>), String> {
    let error = match initialize() {
        Ok(state) => return Ok((state, None)),
        Err(error) => error,
    };
    if !catalog.is_file() {
        return Err(error);
    }
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or_default();
    let aside = with_suffix(catalog, &format!(".unreadable-{stamp}"));
    fs::rename(catalog, &aside).map_err(|_| error.clone())?;
    for companion in ["-wal", "-shm"] {
        let path = with_suffix(catalog, companion);
        if path.is_file() {
            let _ = fs::rename(&path, with_suffix(&aside, companion));
        }
    }
    initialize().map(|state| (state, Some(aside)))
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = OsString::from(path.as_os_str());
    name.push(suffix);
    PathBuf::from(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn a_catalog_that_opens_is_left_in_place() {
        let directory = tempfile::tempdir().expect("temp dir");
        let catalog = directory.path().join("catalog.sqlite3");
        fs::write(&catalog, b"ok").expect("catalog");

        let (value, aside) = initialize_or_set_aside(&catalog, || Ok(7)).expect("opens");

        assert_eq!(value, 7);
        assert_eq!(aside, None);
        assert!(catalog.is_file());
    }

    #[test]
    fn an_unreadable_catalog_is_moved_aside_with_its_companions() {
        let directory = tempfile::tempdir().expect("temp dir");
        let catalog = directory.path().join("catalog.sqlite3");
        fs::write(&catalog, b"basura").expect("catalog");
        fs::write(with_suffix(&catalog, "-wal"), b"wal").expect("wal");
        let attempts = Cell::new(0);

        let (_, aside) = initialize_or_set_aside(&catalog, || {
            attempts.set(attempts.get() + 1);
            if catalog.is_file() {
                Err("dañado".to_owned())
            } else {
                Ok(())
            }
        })
        .expect("recovers");

        let aside = aside.expect("moved aside");
        assert_eq!(attempts.get(), 2);
        assert_eq!(fs::read(&aside).expect("kept"), b"basura");
        assert!(with_suffix(&aside, "-wal").is_file());
        assert!(!catalog.exists());
    }

    #[test]
    fn a_failure_without_a_catalog_file_is_reported() {
        let directory = tempfile::tempdir().expect("temp dir");
        let catalog = directory.path().join("missing.sqlite3");

        let result = initialize_or_set_aside(&catalog, || Err::<(), _>("disco".to_owned()));

        assert_eq!(result.unwrap_err(), "disco");
    }
}

#[cfg(test)]
mod catalog_tests {
    use super::*;

    #[test]
    fn a_garbage_delivery_preset_catalog_no_longer_blocks_start_up() {
        let directory = tempfile::tempdir().expect("temp dir");
        let catalog = directory.path().join("delivery-presets.sqlite3");
        fs::write(&catalog, "esto no es una base SQLite ".repeat(200)).expect("garbage");
        let root = directory.path().to_path_buf();

        let (_, aside) = initialize_or_set_aside(&catalog, || {
            crate::delivery_presets::DeliveryPresetState::initialize(root.clone())
        })
        .expect("opens with a new catalog");

        assert!(aside.expect("set aside").is_file());
        assert!(catalog.is_file());
    }

    #[test]
    fn a_garbage_reusable_task_catalog_no_longer_blocks_start_up() {
        let directory = tempfile::tempdir().expect("temp dir");
        let catalog = directory.path().join("reusable-tasks.sqlite3");
        fs::write(&catalog, "basura ".repeat(500)).expect("garbage");
        let root = directory.path().to_path_buf();

        let (_, aside) = initialize_or_set_aside(&catalog, || {
            crate::reusable_tasks::ReusableTaskState::initialize(root.clone())
        })
        .expect("opens with a new catalog");

        assert!(aside.is_some());
    }
}
