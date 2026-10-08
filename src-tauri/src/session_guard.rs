//! Detects that the previous session ended without a normal exit (DAT-01).
//!
//! A marker file is written when Columnia starts and removed on a normal
//! exit. If it is still there on the next start, the previous session was
//! killed, crashed or lost power, and Cargar tells the person that changes
//! not saved in a project were not kept. Recovery itself stays explicit.

use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::Serialize;
use tauri::State;

const MARKER_FILE: &str = "session.active";
/// PROD-14: when the last session started, kept across normal exits.
const STARTED_FILE: &str = "session.started";
/// Without a previous start (first launch of this version), reports from the
/// last week are the recent ones.
const RECENT_REPORTS_WITHOUT_START_MS: u128 = 7 * 24 * 60 * 60 * 1000;

pub struct SessionGuard {
    marker: PathBuf,
    previous_exit_unclean: bool,
    set_aside_catalogs: Vec<String>,
    recent_crash_reports: usize,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SessionStatus {
    previous_exit_unclean: bool,
    /// ARQ-02: catalogs that could not be opened at start-up and were moved
    /// to these paths; new empty ones replaced them.
    set_aside_catalogs: Vec<String>,
    /// PROD-14: failure reports written since the previous session started.
    recent_crash_reports: usize,
}

impl SessionGuard {
    /// Reads the previous marker and writes the one for this session. A
    /// failure to write only means the next start cannot tell; it never
    /// blocks the app.
    pub fn begin(app_data_dir: &Path) -> Self {
        let marker = app_data_dir.join(MARKER_FILE);
        let previous_exit_unclean = marker.is_file();
        let started = app_data_dir.join(STARTED_FILE);
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or_default();
        let since_ms = fs::read_to_string(&started)
            .ok()
            .and_then(|text| text.trim().parse::<u128>().ok())
            .unwrap_or_else(|| now_ms.saturating_sub(RECENT_REPORTS_WITHOUT_START_MS));
        let recent_crash_reports =
            crate::crash_report::reports_since(&app_data_dir.join("crash-reports"), since_ms);
        let _ = fs::create_dir_all(app_data_dir);
        let _ = fs::write(&marker, b"columnia-session\n");
        let _ = fs::write(&started, now_ms.to_string());
        Self {
            marker,
            previous_exit_unclean,
            set_aside_catalogs: Vec::new(),
            recent_crash_reports,
        }
    }

    pub fn record_set_aside_catalogs(&mut self, paths: impl IntoIterator<Item = PathBuf>) {
        self.set_aside_catalogs
            .extend(paths.into_iter().map(|path| path.display().to_string()));
    }

    /// Called on a normal exit.
    pub fn end(&self) {
        let _ = fs::remove_file(&self.marker);
    }

    fn status(&self) -> SessionStatus {
        SessionStatus {
            previous_exit_unclean: self.previous_exit_unclean,
            set_aside_catalogs: self.set_aside_catalogs.clone(),
            recent_crash_reports: self.recent_crash_reports,
        }
    }
}

#[tauri::command]
pub fn get_session_status(guard: State<'_, SessionGuard>) -> SessionStatus {
    guard.status()
}

/// Whether the window holds changes that no project keeps (FUN-09). React
/// reports it; closing the window then asks before discarding them.
#[derive(Default)]
pub struct UnsavedWork(std::sync::atomic::AtomicBool);

impl UnsavedWork {
    pub fn is_set(&self) -> bool {
        self.0.load(std::sync::atomic::Ordering::Acquire)
    }

    fn set(&self, unsaved: bool) {
        self.0.store(unsaved, std::sync::atomic::Ordering::Release);
    }

    pub fn set_clean(&self) {
        self.set(false);
    }
}

#[tauri::command]
pub fn set_unsaved_work(state: State<'_, UnsavedWork>, unsaved: bool) -> Result<(), String> {
    state.set(unsaved);
    Ok(())
}

/// Text of the native question shown when the window closes with unsaved work.
pub const UNSAVED_WORK_CLOSE_MESSAGE: &str =
    "Hay cambios que no están guardados en un proyecto. Si cierras Columnia ahora, se perderán.";

#[cfg(test)]
mod tests {
    use super::SessionGuard;

    #[test]
    fn a_session_without_a_normal_exit_is_reported_on_the_next_start() {
        let directory = tempfile::tempdir().expect("directorio de datos");

        let first = SessionGuard::begin(directory.path());
        assert!(!first.status().previous_exit_unclean, "primer arranque");
        first.end();

        let second = SessionGuard::begin(directory.path());
        assert!(
            !second.status().previous_exit_unclean,
            "tras una salida normal"
        );
        // No end(): the process was killed.

        let third = SessionGuard::begin(directory.path());
        assert!(
            third.status().previous_exit_unclean,
            "tras un cierre forzado"
        );
        third.end();
        assert!(!directory.path().join("session.active").exists());
    }

    #[test]
    fn crash_reports_of_the_previous_session_are_reported_once() {
        // PROD-14: three panics were written and nobody saw them.
        let directory = tempfile::tempdir().expect("directorio de datos");
        let reports = directory.path().join("crash-reports");

        let first = SessionGuard::begin(directory.path());
        assert_eq!(first.status().recent_crash_reports, 0);
        crate::crash_report::write_report(&reports, None, None).expect("informe");
        crate::crash_report::write_webview_failure_report(&reports, 1).expect("informe");
        first.end();
        std::thread::sleep(std::time::Duration::from_millis(5));

        let second = SessionGuard::begin(directory.path());
        assert_eq!(second.status().recent_crash_reports, 2);
        second.end();
        std::thread::sleep(std::time::Duration::from_millis(5));

        let third = SessionGuard::begin(directory.path());
        assert_eq!(third.status().recent_crash_reports, 0, "ya avisados");
        third.end();
    }
}
