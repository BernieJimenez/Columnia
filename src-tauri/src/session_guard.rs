//! Detects that the previous session ended without a normal exit (DAT-01).
//!
//! A marker file is written when Columnia starts and removed on a normal
//! exit. If it is still there on the next start, the previous session was
//! killed, crashed or lost power, and Cargar tells the person that changes
//! not saved in a project were not kept. Recovery itself stays explicit.

use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::Serialize;
use tauri::State;

const MARKER_FILE: &str = "session.active";

pub struct SessionGuard {
    marker: PathBuf,
    previous_exit_unclean: bool,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SessionStatus {
    previous_exit_unclean: bool,
}

impl SessionGuard {
    /// Reads the previous marker and writes the one for this session. A
    /// failure to write only means the next start cannot tell; it never
    /// blocks the app.
    pub fn begin(app_data_dir: &Path) -> Self {
        let marker = app_data_dir.join(MARKER_FILE);
        let previous_exit_unclean = marker.is_file();
        let _ = fs::create_dir_all(app_data_dir);
        let _ = fs::write(&marker, b"columnia-session\n");
        Self {
            marker,
            previous_exit_unclean,
        }
    }

    /// Called on a normal exit.
    pub fn end(&self) {
        let _ = fs::remove_file(&self.marker);
    }

    fn status(&self) -> SessionStatus {
        SessionStatus {
            previous_exit_unclean: self.previous_exit_unclean,
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
}
