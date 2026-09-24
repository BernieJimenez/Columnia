//! Local panic reports and poison recovery.
//!
//! A panic in a dependency (Polars, DuckDB) while a state mutex was held used to
//! poison it, so every later command failed until restart, and a release build
//! left no trace. Panics now write a minimal local report (version, time,
//! source file name and line; never the panic message, which may contain
//! data) and locks recover their inner value.

use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_REPORTS: usize = 20;

static REPORT_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Installs the panic hook once the app data directory is known; the previous
/// hook still runs so debug output is unchanged.
pub fn install(app_data_dir: &Path) {
    if REPORT_DIR.set(app_data_dir.join("crash-reports")).is_err() {
        return;
    }
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if let Some(directory) = REPORT_DIR.get() {
            let location = info
                .location()
                .map(|location| (location.file(), location.line()));
            let _ = write_report(directory, location, std::thread::current().name());
        }
        previous(info);
    }));
}

/// Writes one report and keeps only the newest `MAX_REPORTS`.
pub(crate) fn write_report(
    directory: &Path,
    location: Option<(&str, u32)>,
    thread: Option<&str>,
) -> io::Result<PathBuf> {
    fs::create_dir_all(directory)?;
    let created_at_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    // Only the file name: compile-time paths can include the user's home.
    let source = location.map(|(file, line)| {
        let name = file
            .rsplit(['/', '\\'])
            .next()
            .filter(|name| !name.is_empty())
            .unwrap_or("desconocido");
        format!("{name}:{line}")
    });
    let report = serde_json::json!({
        "contract": "columnia-panic-report",
        "schemaVersion": 1,
        "appVersion": env!("CARGO_PKG_VERSION"),
        "createdAtUnixMs": created_at_ms.to_string(),
        "source": source,
        "thread": thread.filter(|name| name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')),
    });
    let path = directory.join(format!("panic-{created_at_ms}-{}.json", std::process::id()));
    fs::write(
        &path,
        serde_json::to_vec_pretty(&report).map_err(io::Error::other)?,
    )?;
    prune(directory)?;
    Ok(path)
}

fn prune(directory: &Path) -> io::Result<()> {
    let mut reports = fs::read_dir(directory)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("panic-") && name.ends_with(".json"))
        })
        .collect::<Vec<_>>();
    if reports.len() <= MAX_REPORTS {
        return Ok(());
    }
    reports.sort();
    for path in &reports[..reports.len() - MAX_REPORTS] {
        let _ = fs::remove_file(path);
    }
    Ok(())
}

/// User-facing message for a background task that ended abnormally (a panic
/// or an aborted task). The runtime's text can hold engine internals or data,
/// so only the action and the next step are shown; the panic hook already wrote
/// a minimal local report.
pub(crate) fn task_interrupted(action: &str, _error: &impl std::fmt::Display) -> String {
    format!(
        "{action} por un error interno. Puedes reintentarlo; si se repite, en la carpeta de datos de Columnia hay un informe local en crash-reports."
    )
}

/// Locks a mutex and recovers it if a previous holder panicked. The protected
/// state is published atomically by the dataset operations, so the last
/// committed value remains usable instead of blocking the whole session.
pub(crate) trait LockRecovering<T> {
    fn lock_recovering(&self) -> MutexGuard<'_, T>;
}

impl<T> LockRecovering<T> for Mutex<T> {
    fn lock_recovering(&self) -> MutexGuard<'_, T> {
        self.lock().unwrap_or_else(|poisoned| {
            self.clear_poison();
            poisoned.into_inner()
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::{task_interrupted, write_report, LockRecovering};

    #[test]
    fn a_poisoned_lock_recovers_its_last_value() {
        let state = Arc::new(Mutex::new(41));
        let poisoner = Arc::clone(&state);
        let _ = std::thread::spawn(move || {
            let mut guard = poisoner.lock().unwrap();
            *guard = 42;
            panic!("pánico de prueba con el lock tomado");
        })
        .join();
        assert!(state.is_poisoned());
        assert_eq!(*state.lock_recovering(), 42);
        assert!(!state.is_poisoned());
    }

    #[test]
    fn reports_keep_only_minimal_non_identifying_fields() {
        let directory = tempfile::tempdir().expect("directorio de informes");
        let path = write_report(
            directory.path(),
            Some((
                r"C:\Users\ana\.cargo\registry\polars-core\src\frame\mod.rs",
                128,
            )),
            Some("tokio-runtime-worker"),
        )
        .expect("informe escrito");
        let text = std::fs::read_to_string(path).expect("informe legible");
        assert!(text.contains("\"source\": \"mod.rs:128\""), "{text}");
        assert!(!text.contains("ana"), "{text}");
        assert!(text.contains("columnia-panic-report"), "{text}");
        for _ in 0..25 {
            write_report(directory.path(), None, None).expect("informe escrito");
        }
        assert!(std::fs::read_dir(directory.path()).unwrap().count() <= 20);
    }

    #[test]
    fn interrupted_tasks_never_echo_the_runtime_message() {
        let message = task_interrupted(
            "La imputación de valores nulos se interrumpió",
            &"task 122 panicked with message \"expected equal chunks\"",
        );
        assert!(message
            .starts_with("La imputación de valores nulos se interrumpió por un error interno."));
        assert!(!message.contains("panicked"));
        assert!(!message.contains("chunks"));
    }
}
