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

/// Where a panic happened, without the folders that can reveal the user's home
/// (CODE-01). A dependency keeps its `crate-version/...` path, the standard
/// library its `library/...` path and Columnia its relative `src/...` path; any
/// other absolute path is reduced to the file name.
fn report_source(file: &str, line: u32) -> String {
    let segments = file
        .split(['/', '\\'])
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    let is_versioned_crate = |segment: &str| {
        segment.rsplit_once('-').is_some_and(|(name, version)| {
            !name.is_empty()
                && version.split('.').count() >= 3
                && version
                    .split('.')
                    .next()
                    .is_some_and(|major| major.chars().all(|character| character.is_ascii_digit()))
        })
    };
    let absolute = file.starts_with('/')
        || file
            .get(1..3)
            .is_some_and(|prefix| prefix == ":\\" || prefix == ":/");
    let start = segments
        .iter()
        .rposition(|segment| is_versioned_crate(segment))
        .or_else(|| segments.iter().position(|segment| *segment == "library"))
        .or((!absolute && !segments.is_empty()).then_some(0))
        .unwrap_or(segments.len().saturating_sub(1));
    let path = segments
        .get(start..)
        .map(|kept| kept.join("/"))
        .unwrap_or_default();
    let path = if path.is_empty() {
        "desconocido".to_owned()
    } else {
        path
    };
    format!("{path}:{line}")
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
    let source = location.map(|(file, line)| report_source(file, line));
    let report = serde_json::json!({
        "contract": "columnia-panic-report",
        "schemaVersion": 1,
        "appVersion": env!("CARGO_PKG_VERSION"),
        "createdAtUnixMs": created_at_ms.to_string(),
        "source": source,
        "thread": thread.filter(|name| name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')),
    });
    write_json_report(directory, "panic", created_at_ms, &report)
}

/// ARQ-06: what the app does when one of its WebView2 processes fails.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WebviewFailureAction {
    /// The browser process is gone: no window can come back, so the instance
    /// exits and a new launch is not blocked by a windowless one.
    Exit,
    /// The page's renderer died or hangs: reloading brings the interface back
    /// while the engine keeps its state.
    Reload,
    /// WebView2 restarts helper, GPU and frame processes by itself.
    ReportOnly,
}

/// Maps `COREWEBVIEW2_PROCESS_FAILED_KIND` to the recovery.
pub(crate) fn webview_failure_action(kind: i32) -> WebviewFailureAction {
    match kind {
        0 => WebviewFailureAction::Exit,
        1 | 2 => WebviewFailureAction::Reload,
        _ => WebviewFailureAction::ReportOnly,
    }
}

fn webview_failure_kind_name(kind: i32) -> &'static str {
    match kind {
        0 => "browser-process-exited",
        1 => "render-process-exited",
        2 => "render-process-unresponsive",
        3 => "frame-render-process-exited",
        4 => "utility-process-exited",
        5 => "sandbox-helper-process-exited",
        6 => "gpu-process-exited",
        7 => "ppapi-plugin-process-exited",
        8 => "ppapi-broker-process-exited",
        _ => "unknown-process-exited",
    }
}

/// ARQ-06: a local report of a failed WebView2 process; like a panic report it
/// holds only the version, the time, the kind of failure and the recovery.
pub(crate) fn write_webview_failure_report(directory: &Path, kind: i32) -> io::Result<PathBuf> {
    let created_at_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    let action = match webview_failure_action(kind) {
        WebviewFailureAction::Exit => "exit",
        WebviewFailureAction::Reload => "reload",
        WebviewFailureAction::ReportOnly => "none",
    };
    let report = serde_json::json!({
        "contract": "columnia-webview-failure-report",
        "schemaVersion": 1,
        "appVersion": env!("CARGO_PKG_VERSION"),
        "createdAtUnixMs": created_at_ms.to_string(),
        "processFailedKind": webview_failure_kind_name(kind),
        "action": action,
    });
    write_json_report(directory, "webview", created_at_ms, &report)
}

fn write_json_report(
    directory: &Path,
    prefix: &str,
    created_at_ms: u128,
    report: &serde_json::Value,
) -> io::Result<PathBuf> {
    fs::create_dir_all(directory)?;
    let path = directory.join(format!(
        "{prefix}-{created_at_ms}-{}.json",
        std::process::id()
    ));
    fs::write(
        &path,
        serde_json::to_vec_pretty(report).map_err(io::Error::other)?,
    )?;
    prune(directory)?;
    Ok(path)
}

/// The panic and WebView2 reports in `directory`, with their creation time.
fn reports(directory: &Path) -> io::Result<Vec<(PathBuf, u128)>> {
    Ok(fs::read_dir(directory)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter_map(|path| {
            let name = path.file_name()?.to_str()?;
            if !(name.starts_with("panic-") || name.starts_with("webview-"))
                || !name.ends_with(".json")
            {
                return None;
            }
            let created_at_ms = name.split('-').nth(1)?.parse::<u128>().unwrap_or_default();
            Some((path, created_at_ms))
        })
        .collect())
}

/// PROD-14: how many reports were written from `since_ms` on, so Cargar can
/// say that Columnia failed instead of leaving them unread.
pub(crate) fn reports_since(directory: &Path, since_ms: u128) -> usize {
    reports(directory).map_or(0, |reports| {
        reports
            .iter()
            .filter(|(_, created_at_ms)| *created_at_ms >= since_ms)
            .count()
    })
}

fn prune(directory: &Path) -> io::Result<()> {
    let mut reports = reports(directory)?;
    if reports.len() <= MAX_REPORTS {
        return Ok(());
    }
    // Oldest first by creation time, whatever the kind of report.
    reports.sort_by_key(|(_, created_at_ms)| *created_at_ms);
    let reports = reports
        .into_iter()
        .map(|(path, _)| path)
        .collect::<Vec<_>>();
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

    use super::{report_source, task_interrupted, write_report, LockRecovering};

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
                r"C:\Users\ana\.cargo\registry\src\index.crates.io-1949cf8c6b5b557f\polars-core-0.55.2\src\frame\mod.rs",
                128,
            )),
            Some("tokio-runtime-worker"),
        )
        .expect("informe escrito");
        let text = std::fs::read_to_string(path).expect("informe legible");
        // CODE-01: «mod.rs:2136» alone could not say which crate failed.
        assert!(
            text.contains("\"source\": \"polars-core-0.55.2/src/frame/mod.rs:128\""),
            "{text}"
        );
        assert!(!text.contains("ana"), "{text}");
        assert!(text.contains("columnia-panic-report"), "{text}");
        for _ in 0..25 {
            write_report(directory.path(), None, None).expect("informe escrito");
        }
        assert!(std::fs::read_dir(directory.path()).unwrap().count() <= 20);
    }

    #[test]
    fn report_sources_name_the_crate_without_private_folders() {
        assert_eq!(
            report_source(r"src\dataset\history.rs", 603),
            "src/dataset/history.rs:603"
        );
        assert_eq!(
            report_source("/rustc/1a2b3c/library/core/src/option.rs", 9),
            "library/core/src/option.rs:9"
        );
        assert_eq!(
            report_source(r"C:\Users\ana\proyecto\privado\mod.rs", 1),
            "mod.rs:1"
        );
        assert_eq!(report_source("", 5), "desconocido:5");
    }

    #[test]
    fn a_failed_webview_process_leaves_a_report_and_decides_how_to_recover() {
        use super::{webview_failure_action, write_webview_failure_report, WebviewFailureAction};
        // ARQ-06: the browser process gone means no window can come back, so
        // the instance exits instead of holding the single-instance lock.
        assert_eq!(webview_failure_action(0), WebviewFailureAction::Exit);
        assert_eq!(webview_failure_action(1), WebviewFailureAction::Reload);
        assert_eq!(webview_failure_action(2), WebviewFailureAction::Reload);
        assert_eq!(webview_failure_action(6), WebviewFailureAction::ReportOnly);
        assert_eq!(webview_failure_action(99), WebviewFailureAction::ReportOnly);

        let directory = tempfile::tempdir().expect("directorio de informes");
        let path = write_webview_failure_report(directory.path(), 1).expect("informe escrito");
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        assert!(
            name.starts_with("webview-") && name.ends_with(".json"),
            "{name}"
        );
        let report: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(report["contract"], "columnia-webview-failure-report");
        assert_eq!(report["processFailedKind"], "render-process-exited");
        assert_eq!(report["action"], "reload");
        for _ in 0..25 {
            write_webview_failure_report(directory.path(), 2).expect("informe escrito");
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
