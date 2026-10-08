use super::{
    collect_lazy_frame_streaming_with_cancel, dataset_preview_from_schema_and_page,
    ensure_not_cancelled, fresh_history_entry_id, loaded_dataset_preview, parquet_row_count,
    parquet_row_group_rows, parquet_scan, read_parquet_frame_with_cancel,
    read_parquet_schema_frame, validate_dataset_file, DatasetPreview, DatasetState, IdxSize,
    LoadedDataset, PrepareCancellation, HISTORY_DISK_BUDGET_BYTES, HISTORY_MAX_ENTRIES,
    OPERATION_CANCELLED_MESSAGE, PREVIEW_ROW_LIMIT,
};
use crate::crash_report::LockRecovering;
use polars::prelude::{DataFrame, ParquetWriter};
use serde::Serialize;
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
};
use tauri::{AppHandle, Manager, State};

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryResult {
    pub(super) dataset: DatasetPreview,
    pub(super) history: HistoryState,
    pub(super) message: String,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntryState {
    pub(super) id: Option<String>,
    pub(super) index: usize,
    pub(super) label: String,
    pub(super) is_current: bool,
}

/// PROD-10: what the last published change did, for one common summary.
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChangeSummary {
    pub(super) nulled_cell_count: usize,
    pub(super) removed_row_count: usize,
    pub(super) reversible: bool,
    #[serde(skip)]
    pub(super) revision: u64,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryState {
    pub(super) can_undo: bool,
    pub(super) can_redo: bool,
    pub(super) current_index: usize,
    pub(super) entry_count: usize,
    pub(super) entries: Vec<HistoryEntryState>,
    pub(super) snapshots_enabled: bool,
    pub(super) degraded_reason: Option<String>,
    pub(super) max_entries: usize,
    pub(super) disk_bytes: u64,
    pub(super) disk_budget_bytes: u64,
    /// PROD-10: only while the frame is the one that change published.
    pub(super) last_change: Option<ChangeSummary>,
}

#[derive(Debug)]
pub(super) struct HistoryEntry {
    pub(super) id: String,
    pub(super) label: String,
    pub(super) path: PathBuf,
    pub(super) bytes: u64,
}

#[derive(Debug)]
pub(super) struct PreparedHistorySnapshot {
    pub(super) temporary: tempfile::NamedTempFile,
    pub(super) bytes: u64,
}

#[derive(Debug)]
pub(super) struct HistoryCommitResult {
    pub(super) active_snapshot: Option<(PathBuf, u64)>,
    pub(super) retired_paths: Vec<PathBuf>,
}

/// Folders of the reversible history in the system temp directory start
/// with this, so that a later start can find the ones left behind (DAT-04).
const HISTORY_DIRECTORY_PREFIX: &str = "columnia-history-";
/// Held open while a session uses its folder; a folder whose lock can be
/// deleted belongs to a session that is over.
const HISTORY_LOCK_FILE: &str = "en-uso.lock";

/// A new history folder and its open lock (DAT-04).
///
/// QA-39: fields drop in declaration order, so a folder abandoned by an error
/// releases its lock first and is then deleted. As a `(lock, directory)` pair
/// bound to two locals, the folder dropped first while the lock still kept
/// it, and it stayed in %TEMP% with the snapshots already copied into it.
pub(super) struct HistoryDirectory {
    pub(super) lock: std::fs::File,
    pub(super) directory: tempfile::TempDir,
}

pub(super) fn history_directory() -> Result<HistoryDirectory, String> {
    let directory = tempfile::Builder::new()
        .prefix(HISTORY_DIRECTORY_PREFIX)
        .tempdir()
        .map_err(|error| format!("No se pudo crear el historial temporal: {error}"))?;
    let lock = open_history_lock(&directory.path().join(HISTORY_LOCK_FILE))
        .map_err(|error| format!("No se pudo crear el historial temporal: {error}"))?;
    Ok(HistoryDirectory { lock, directory })
}

#[cfg(windows)]
fn open_history_lock(path: &std::path::Path) -> std::io::Result<std::fs::File> {
    use std::os::windows::fs::OpenOptionsExt;
    // FILE_SHARE_READ only: while it is open nobody can delete it.
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .share_mode(0x1)
        .open(path)
}

#[cfg(not(windows))]
fn open_history_lock(path: &std::path::Path) -> std::io::Result<std::fs::File> {
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
}

/// Removes the history folders of sessions that are over: closed normally
/// or killed (DAT-04). A folder in use keeps its lock and is skipped. Off
/// Windows, where an open file can be deleted, only folders untouched for a
/// day are removed. Returns how many folders were removed.
pub(crate) fn purge_finished_history_directories() -> usize {
    let Ok(entries) = std::fs::read_dir(std::env::temp_dir()) else {
        return 0;
    };
    let mut removed = 0;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with(".tmp") && is_legacy_history_directory(&entry.path()) {
            if std::fs::remove_dir_all(entry.path()).is_ok() {
                removed += 1;
            }
            continue;
        }
        if !name.starts_with(HISTORY_DIRECTORY_PREFIX) {
            continue;
        }
        let Ok(metadata) = std::fs::symlink_metadata(entry.path()) else {
            continue;
        };
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            continue;
        }
        let lock = entry.path().join(HISTORY_LOCK_FILE);
        let finished = if cfg!(windows) {
            !lock.exists() || std::fs::remove_file(&lock).is_ok()
        } else {
            metadata
                .modified()
                .ok()
                .and_then(|modified| modified.elapsed().ok())
                .is_some_and(|age| age.as_secs() >= 24 * 60 * 60)
        };
        if finished && std::fs::remove_dir_all(entry.path()).is_ok() {
            removed += 1;
        }
    }
    removed
}

/// A history folder of a version before DAT-04: a plain `.tmp*` folder,
/// untouched for a day, holding only `snapshot-<20 digits>.parquet` files.
fn is_legacy_history_directory(path: &std::path::Path) -> bool {
    let Ok(metadata) = std::fs::symlink_metadata(path) else {
        return false;
    };
    let old = metadata
        .modified()
        .ok()
        .and_then(|modified| modified.elapsed().ok())
        .is_some_and(|age| age.as_secs() >= 24 * 60 * 60);
    if !metadata.is_dir() || metadata.file_type().is_symlink() || !old {
        return false;
    }
    let Ok(entries) = std::fs::read_dir(path) else {
        return false;
    };
    let mut snapshots = 0;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let is_snapshot = name
            .strip_prefix("snapshot-")
            .and_then(|rest| rest.strip_suffix(".parquet"))
            .is_some_and(|digits| {
                digits.len() == 20 && digits.bytes().all(|byte| byte.is_ascii_digit())
            });
        if !is_snapshot || !entry.file_type().is_ok_and(|kind| kind.is_file()) {
            return false;
        }
        snapshots += 1;
    }
    snapshots > 0
}

#[derive(Debug)]
pub(super) struct HistoryManager {
    /// Dropped before `directory`, so the folder can be deleted (DAT-04).
    pub(super) _lock: std::fs::File,
    pub(super) directory: tempfile::TempDir,
    pub(super) source_snapshot_path: Option<PathBuf>,
    pub(super) entries: Vec<HistoryEntry>,
    pub(super) cursor: usize,
    pub(super) snapshots_enabled: bool,
    pub(super) degraded_reason: Option<String>,
    pub(super) current_label: String,
    pub(super) next_id: u64,
    pub(super) max_entries: usize,
    pub(super) disk_budget_bytes: u64,
    /// Grows with every change of the active frame, also while snapshots are
    /// disabled, so a cache can tell two versions apart (REN-05).
    pub(super) revision: u64,
    /// The label of the active version when it was too large to keep a
    /// snapshot: the earlier versions stay and undo returns to the last one
    /// that was saved (DAT-07).
    pub(super) unsaved_current: Option<String>,
    /// PROD-10: the summary of the last change published through Preparar.
    pub(super) last_change: Option<ChangeSummary>,
}

#[cfg(test)]
thread_local! {
    /// COD-18: history folders this thread created, for tests that must not
    /// see the ones of other tests running in parallel.
    pub(super) static DEFERRED_HISTORIES_CREATED: std::cell::Cell<usize> =
        const { std::cell::Cell::new(0) };
}

impl HistoryManager {
    pub(super) fn deferred() -> Result<Self, String> {
        #[cfg(test)]
        DEFERRED_HISTORIES_CREATED.with(|count| count.set(count.get() + 1));
        let folder = history_directory()?;
        Ok(Self {
            _lock: folder.lock,
            directory: folder.directory,
            source_snapshot_path: None,
            entries: Vec::new(),
            cursor: 0,
            snapshots_enabled: false,
            degraded_reason: Some(
                "La carga source-backed conserva el historial desactivado hasta materializar una operación.".to_owned(),
            ),
            current_label: "Dataset original".to_owned(),
            next_id: 0,
            max_entries: HISTORY_MAX_ENTRIES,
            disk_budget_bytes: HISTORY_DISK_BUDGET_BYTES,
            revision: 0,
            unsaved_current: None,
            last_change: None,
        })
    }

    pub(super) fn new(frame: &DataFrame) -> Result<Self, String> {
        Self::with_limits(frame, HISTORY_MAX_ENTRIES, HISTORY_DISK_BUDGET_BYTES)
    }

    pub(super) fn with_limits(
        frame: &DataFrame,
        max_entries: usize,
        disk_budget_bytes: u64,
    ) -> Result<Self, String> {
        let folder = history_directory()?;
        let mut manager = Self {
            _lock: folder.lock,
            directory: folder.directory,
            source_snapshot_path: None,
            entries: Vec::new(),
            cursor: 0,
            snapshots_enabled: true,
            degraded_reason: None,
            current_label: "Dataset original".to_owned(),
            next_id: 0,
            max_entries: max_entries.max(1),
            disk_budget_bytes,
            revision: 0,
            unsaved_current: None,
            last_change: None,
        };
        manager.record(frame, "Dataset original")?;
        Ok(manager)
    }

    /// PROD-10: the last summary while the frame is still the one it describes.
    fn current_change(&self) -> Option<ChangeSummary> {
        self.last_change
            .clone()
            .filter(|change| change.revision == self.revision)
    }

    pub(super) fn touch(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }

    pub(super) fn disk_bytes(&self) -> u64 {
        self.entries.iter().map(|entry| entry.bytes).sum()
    }

    pub(super) fn state(&self) -> HistoryState {
        if let (true, Some(label)) = (self.snapshots_enabled, self.unsaved_current.as_ref()) {
            let mut entries = self
                .entries
                .iter()
                .enumerate()
                .map(|(index, entry)| HistoryEntryState {
                    id: Some(entry.id.clone()),
                    index,
                    label: entry.label.clone(),
                    is_current: false,
                })
                .collect::<Vec<_>>();
            entries.push(HistoryEntryState {
                id: None,
                index: entries.len(),
                label: label.clone(),
                is_current: true,
            });
            return HistoryState {
                can_undo: !self.entries.is_empty(),
                can_redo: false,
                current_index: entries.len() - 1,
                entry_count: entries.len(),
                entries,
                snapshots_enabled: true,
                degraded_reason: self.degraded_reason.clone(),
                max_entries: self.max_entries,
                disk_bytes: self.disk_bytes(),
                disk_budget_bytes: self.disk_budget_bytes,
                last_change: self.current_change(),
            };
        }
        let entries = if self.snapshots_enabled {
            self.entries
                .iter()
                .enumerate()
                .map(|(index, entry)| HistoryEntryState {
                    id: Some(entry.id.clone()),
                    index,
                    label: entry.label.clone(),
                    is_current: index == self.cursor,
                })
                .collect()
        } else {
            vec![HistoryEntryState {
                id: None,
                index: 0,
                label: self.current_label.clone(),
                is_current: true,
            }]
        };
        let entry_count = entries.len();
        HistoryState {
            can_undo: self.snapshots_enabled && self.cursor > 0,
            can_redo: self.snapshots_enabled && self.cursor + 1 < self.entries.len(),
            current_index: if self.snapshots_enabled {
                self.cursor
            } else {
                0
            },
            entry_count,
            entries,
            snapshots_enabled: self.snapshots_enabled,
            degraded_reason: self.degraded_reason.clone(),
            max_entries: self.max_entries,
            disk_bytes: self.disk_bytes(),
            disk_budget_bytes: self.disk_budget_bytes,
            last_change: self.current_change(),
        }
    }

    pub(super) fn disable_for_size(&mut self, label: &str, bytes: u64) {
        for path in self.disable_for_size_deferred(label, bytes) {
            let _ = fs::remove_file(path);
        }
    }

    pub(super) fn disable_for_size_deferred(&mut self, label: &str, bytes: u64) -> Vec<PathBuf> {
        if self.snapshots_enabled && !self.entries.is_empty() {
            // DAT-07: the earlier versions stay; only the redo branch, which
            // the new version replaces, goes.
            let branch_start = self.cursor.saturating_add(1).min(self.entries.len());
            let retired = self
                .entries
                .split_off(branch_start)
                .into_iter()
                .map(|entry| entry.path)
                .collect();
            let previous = self.entries[self.cursor].label.clone();
            self.current_label = label.to_owned();
            self.unsaved_current = Some(label.to_owned());
            self.degraded_reason = Some(format!(
                "El resultado de «{label}» ocupa {bytes} bytes y supera el límite local de {} bytes del historial, así que no se guardó como versión: deshacer vuelve a «{previous}» y este paso no se podrá rehacer. Las versiones anteriores se conservan.",
                self.disk_budget_bytes
            ));
            return retired;
        }
        let retired_paths = self.entries.drain(..).map(|entry| entry.path).collect();
        self.cursor = 0;
        self.snapshots_enabled = false;
        self.current_label = label.to_owned();
        self.degraded_reason = Some(format!(
            "El snapshot requiere {bytes} bytes y supera el límite local de {} bytes. El cambio se aplicó sin historial reversible.",
            self.disk_budget_bytes
        ));
        retired_paths
    }

    pub(super) fn record(&mut self, frame: &DataFrame, label: &str) -> Result<(), String> {
        self.touch();
        self.current_label = label.to_owned();
        if !self.snapshots_enabled {
            return Ok(());
        }

        let temporary = tempfile::NamedTempFile::new_in(self.directory.path())
            .map_err(|error| format!("No se pudo preparar el snapshot del historial: {error}"))?;
        let mut snapshot = frame.clone();
        ParquetWriter::new(temporary.as_file())
            .with_row_group_size(Some(parquet_row_group_rows(frame)))
            .finish(&mut snapshot)
            .map_err(|error| format!("No se pudo escribir el snapshot del historial: {error}"))?;
        temporary.as_file().sync_all().map_err(|error| {
            format!("No se pudo sincronizar el snapshot del historial: {error}")
        })?;
        let bytes = temporary
            .as_file()
            .metadata()
            .map_err(|error| format!("No se pudo verificar el snapshot del historial: {error}"))?
            .len();
        if bytes > self.disk_budget_bytes {
            drop(temporary);
            self.disable_for_size(label, bytes);
            return Ok(());
        }

        let destination = self
            .directory
            .path()
            .join(format!("snapshot-{:020}.parquet", self.next_id));
        temporary.persist(&destination).map_err(|error| {
            format!(
                "No se pudo publicar el snapshot del historial: {}",
                error.error
            )
        })?;

        // Solo después de publicar el snapshot se descarta una posible rama de rehacer.
        let branch_start = self.cursor.saturating_add(1).min(self.entries.len());
        let removed = self.entries.split_off(branch_start);
        for entry in removed {
            let _ = fs::remove_file(entry.path);
        }
        self.entries.push(HistoryEntry {
            id: fresh_history_entry_id(),
            label: label.to_owned(),
            path: destination,
            bytes,
        });
        self.cursor = self.entries.len() - 1;
        self.unsaved_current = None;
        self.degraded_reason = None;
        self.next_id = self.next_id.wrapping_add(1);

        while self.entries.len() > self.max_entries || self.disk_bytes() > self.disk_budget_bytes {
            let entry = self.entries.remove(0);
            let _ = fs::remove_file(entry.path);
            self.cursor = self.cursor.saturating_sub(1);
        }
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn record_parquet(&mut self, source: &Path, label: &str) -> Result<(), String> {
        self.current_label = label.to_owned();
        if !self.snapshots_enabled {
            return Ok(());
        }

        let mut temporary = tempfile::NamedTempFile::new_in(self.directory.path())
            .map_err(|error| format!("No se pudo preparar el snapshot Parquet: {error}"))?;
        let mut input = File::open(source)
            .map_err(|error| format!("No se pudo leer el snapshot Parquet: {error}"))?;
        std::io::copy(&mut input, temporary.as_file_mut()).map_err(|error| {
            format!("No se pudo copiar el snapshot Parquet al historial: {error}")
        })?;
        temporary.as_file().sync_all().map_err(|error| {
            format!("No se pudo sincronizar el snapshot Parquet del historial: {error}")
        })?;
        let bytes = temporary
            .as_file()
            .metadata()
            .map_err(|error| format!("No se pudo verificar el snapshot Parquet: {error}"))?
            .len();
        if bytes > self.disk_budget_bytes {
            drop(temporary);
            self.disable_for_size(label, bytes);
            return Ok(());
        }

        let destination = self
            .directory
            .path()
            .join(format!("snapshot-{:020}.parquet", self.next_id));
        temporary.persist(&destination).map_err(|error| {
            format!(
                "No se pudo publicar el snapshot Parquet del historial: {}",
                error.error
            )
        })?;

        let branch_start = self.cursor.saturating_add(1).min(self.entries.len());
        let removed = self.entries.split_off(branch_start);
        for entry in removed {
            let _ = fs::remove_file(entry.path);
        }
        self.entries.push(HistoryEntry {
            id: fresh_history_entry_id(),
            label: label.to_owned(),
            path: destination,
            bytes,
        });
        self.cursor = self.entries.len() - 1;
        self.unsaved_current = None;
        self.degraded_reason = None;
        self.next_id = self.next_id.wrapping_add(1);

        while self.entries.len() > self.max_entries || self.disk_bytes() > self.disk_budget_bytes {
            let entry = self.entries.remove(0);
            let _ = fs::remove_file(entry.path);
            self.cursor = self.cursor.saturating_sub(1);
        }
        Ok(())
    }

    pub(super) fn prepare_parquet_snapshot<C>(
        &self,
        source: &Path,
        mut is_cancelled: C,
    ) -> Result<PreparedHistorySnapshot, String>
    where
        C: FnMut() -> bool,
    {
        let mut temporary = tempfile::NamedTempFile::new_in(self.directory.path())
            .map_err(|error| format!("No se pudo preparar el snapshot Parquet: {error}"))?;
        let mut input = File::open(source)
            .map_err(|error| format!("No se pudo leer el snapshot Parquet: {error}"))?;
        let mut buffer = [0; 64 * 1024];
        loop {
            if is_cancelled() {
                return Err(OPERATION_CANCELLED_MESSAGE.to_owned());
            }
            let bytes_read = input
                .read(&mut buffer)
                .map_err(|error| format!("No se pudo leer el snapshot Parquet: {error}"))?;
            if bytes_read == 0 {
                break;
            }
            temporary
                .as_file_mut()
                .write_all(&buffer[..bytes_read])
                .map_err(|error| {
                    format!("No se pudo copiar el snapshot Parquet al historial: {error}")
                })?;
        }
        if is_cancelled() {
            return Err(OPERATION_CANCELLED_MESSAGE.to_owned());
        }
        temporary.as_file().sync_all().map_err(|error| {
            format!("No se pudo sincronizar el snapshot Parquet del historial: {error}")
        })?;
        if is_cancelled() {
            return Err(OPERATION_CANCELLED_MESSAGE.to_owned());
        }
        let bytes = temporary
            .as_file()
            .metadata()
            .map_err(|error| format!("No se pudo verificar el snapshot Parquet: {error}"))?
            .len();
        Ok(PreparedHistorySnapshot { temporary, bytes })
    }

    pub(super) fn prepare_frame_snapshot<C>(
        &self,
        frame: &DataFrame,
        mut is_cancelled: C,
    ) -> Result<PreparedHistorySnapshot, String>
    where
        C: FnMut() -> bool,
    {
        if is_cancelled() {
            return Err(OPERATION_CANCELLED_MESSAGE.to_owned());
        }
        let mut temporary = tempfile::NamedTempFile::new_in(self.directory.path())
            .map_err(|error| format!("No se pudo preparar el snapshot del historial: {error}"))?;
        let schema_frame = frame.slice(0, 0);
        let schema = schema_frame.schema();
        let mut writer = ParquetWriter::new(temporary.as_file_mut())
            .set_parallel(false)
            .batched(schema)
            .map_err(|error| format!("No se pudo preparar el snapshot del historial: {error}"))?;
        let batch_rows = parquet_row_group_rows(frame);
        let mut offset = 0;
        while offset < frame.height() {
            if is_cancelled() {
                return Err(OPERATION_CANCELLED_MESSAGE.to_owned());
            }
            let row_count = batch_rows.min(frame.height() - offset);
            let mut batch = frame.slice(offset as i64, row_count);
            // Mutations rebuild some columns as one chunk while streamed loads
            // keep several; the batched writer requires equal chunk layouts.
            batch.align_chunks_par();
            writer.write_batch(&batch).map_err(|error| {
                format!("No se pudo escribir el snapshot del historial: {error}")
            })?;
            offset += row_count;
        }
        if is_cancelled() {
            return Err(OPERATION_CANCELLED_MESSAGE.to_owned());
        }
        writer
            .finish()
            .map_err(|error| format!("No se pudo cerrar el snapshot del historial: {error}"))?;
        if is_cancelled() {
            return Err(OPERATION_CANCELLED_MESSAGE.to_owned());
        }
        temporary.as_file().sync_all().map_err(|error| {
            format!("No se pudo sincronizar el snapshot del historial: {error}")
        })?;
        if is_cancelled() {
            return Err(OPERATION_CANCELLED_MESSAGE.to_owned());
        }
        let bytes = temporary
            .as_file()
            .metadata()
            .map_err(|error| format!("No se pudo verificar el snapshot del historial: {error}"))?
            .len();
        Ok(PreparedHistorySnapshot { temporary, bytes })
    }

    pub(super) fn record_prepared_snapshot(
        &mut self,
        prepared: PreparedHistorySnapshot,
        label: &str,
    ) -> Result<Vec<PathBuf>, String> {
        self.record_prepared_snapshot_with_persist(prepared, label, |prepared, destination| {
            prepared
                .temporary
                .persist(destination)
                .map(|_| ())
                .map_err(|error| error.error.to_string())
        })
    }

    pub(super) fn record_prepared_snapshot_with_persist<P>(
        &mut self,
        prepared: PreparedHistorySnapshot,
        label: &str,
        mut persist: P,
    ) -> Result<Vec<PathBuf>, String>
    where
        P: FnMut(PreparedHistorySnapshot, &Path) -> Result<(), String>,
    {
        self.touch();
        if !self.snapshots_enabled {
            self.current_label = label.to_owned();
            return Ok(Vec::new());
        }
        if prepared.bytes > self.disk_budget_bytes {
            return Ok(self.disable_for_size_deferred(label, prepared.bytes));
        }

        let (id, destination) = self.next_snapshot_destination(self.next_id, &[])?;
        let bytes = prepared.bytes;
        persist(prepared, &destination)
            .map_err(|error| format!("No se pudo publicar el snapshot del historial: {error}"))?;

        self.current_label = label.to_owned();
        let branch_start = self.cursor.saturating_add(1).min(self.entries.len());
        let mut retired_paths = self
            .entries
            .split_off(branch_start)
            .into_iter()
            .map(|entry| entry.path)
            .collect::<Vec<_>>();
        self.entries.push(HistoryEntry {
            id: fresh_history_entry_id(),
            label: label.to_owned(),
            path: destination,
            bytes,
        });
        self.cursor = self.entries.len() - 1;
        self.unsaved_current = None;
        self.degraded_reason = None;
        self.next_id = id.wrapping_add(1);

        while self.entries.len() > self.max_entries || self.disk_bytes() > self.disk_budget_bytes {
            let entry = self.entries.remove(0);
            retired_paths.push(entry.path);
            self.cursor = self.cursor.saturating_sub(1);
        }
        Ok(retired_paths)
    }

    pub(super) fn next_snapshot_destination(
        &self,
        mut candidate_id: u64,
        reserved: &[PathBuf],
    ) -> Result<(u64, PathBuf), String> {
        loop {
            let candidate_path = self
                .directory
                .path()
                .join(format!("snapshot-{candidate_id:020}.parquet"));
            let referenced = self
                .entries
                .iter()
                .any(|entry| entry.path == candidate_path)
                || reserved.iter().any(|path| path == &candidate_path);
            if !referenced && !candidate_path.exists() {
                return Ok((candidate_id, candidate_path));
            }
            candidate_id = candidate_id.checked_add(1).ok_or_else(|| {
                "No se pudo reservar un identificador único para el historial.".to_owned()
            })?;
        }
    }

    #[cfg(test)]
    pub(super) fn restore(&self, index: usize) -> Result<DataFrame, String> {
        self.restore_with_cancel(index, || false)
    }

    pub(super) fn restore_with_cancel<C>(
        &self,
        index: usize,
        is_cancelled: C,
    ) -> Result<DataFrame, String>
    where
        C: Fn() -> bool + Sync,
    {
        ensure_not_cancelled(is_cancelled())?;
        let entry = self
            .entries
            .get(index)
            .ok_or_else(|| "La revisión solicitada ya no está disponible.".to_owned())?;
        read_parquet_frame_with_cancel(&entry.path, is_cancelled).map_err(|error| {
            if error == OPERATION_CANCELLED_MESSAGE {
                error
            } else {
                format!("No se pudo restaurar el snapshot del historial: {error}")
            }
        })
    }

    #[cfg(test)]
    pub(super) fn restore_by_id(&self, id: &str) -> Result<(DataFrame, String), String> {
        self.restore_by_id_with_cancel(id, || false)
    }

    pub(super) fn restore_by_id_with_cancel<C>(
        &self,
        id: &str,
        is_cancelled: C,
    ) -> Result<(DataFrame, String), String>
    where
        C: Fn() -> bool + Sync,
    {
        ensure_not_cancelled(is_cancelled())?;
        if !self.snapshots_enabled {
            return Err(self.degraded_reason.clone().unwrap_or_else(|| {
                "La comparación no está disponible porque el historial está desactivado.".to_owned()
            }));
        }
        let entry = self
            .entries
            .iter()
            .find(|entry| entry.id == id)
            .ok_or_else(|| "La revisión seleccionada ya no está disponible.".to_owned())?;
        let frame = read_parquet_frame_with_cancel(&entry.path, is_cancelled).map_err(|error| {
            if error == OPERATION_CANCELLED_MESSAGE {
                error
            } else {
                "No se pudo leer una revisión del historial.".to_owned()
            }
        })?;
        Ok((frame, entry.label.clone()))
    }

    pub(super) fn contains_id(&self, id: &str) -> bool {
        self.snapshots_enabled && self.entries.iter().any(|entry| entry.id == id)
    }
}

pub(super) async fn undo_last_change_impl(app: AppHandle) -> Result<HistoryResult, String> {
    let cancellation = PrepareCancellation::begin(&app);
    tauri::async_runtime::spawn_blocking(move || {
        cancellation.ensure()?;
        let state = app.state::<DatasetState>();
        let mut current = state.current.lock_recovering();
        let dataset = current.as_mut().ok_or_else(|| {
            "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
        })?;
        undo_dataset_with_cancellation(dataset, Some(&cancellation))
    })
    .await
    .map_err(|error| {
        crate::crash_report::task_interrupted("No se pudo deshacer el cambio", &error)
    })?
}
fn restore_source_backed_history_cursor(
    dataset: &mut LoadedDataset,
    target: usize,
    message: &str,
    cancellation: Option<&PrepareCancellation>,
) -> Result<HistoryResult, String> {
    let check_cancellation = || cancellation.map_or(Ok(()), |token| token.ensure());
    let is_cancelled = || cancellation.is_some_and(|token| token.is_cancelled());
    check_cancellation()?;
    let entry = dataset
        .history
        .entries
        .get(target)
        .ok_or_else(|| "La revisión solicitada ya no está disponible.".to_owned())?;
    let label = entry.label.clone();
    let entry_path = entry.path.clone();
    let (snapshot_path, snapshot_size, extension) = validate_dataset_file(&entry_path)?;
    check_cancellation()?;
    if extension != "parquet" {
        return Err("El snapshot del historial no es un Parquet válido.".to_owned());
    }

    // Validate and prepare every value before changing the active dataset or cursor. The
    // cursor can therefore remain usable if a snapshot is truncated or corrupted.
    let schema = read_parquet_schema_frame(&snapshot_path)?;
    check_cancellation()?;
    let row_count = parquet_row_count(&snapshot_path)?;
    check_cancellation()?;
    let page = if row_count == 0 {
        schema.slice(0, 0)
    } else {
        let page = collect_lazy_frame_streaming_with_cancel(
            parquet_scan(&snapshot_path)?.slice(0, PREVIEW_ROW_LIMIT as IdxSize),
            "No se pudo leer la vista previa del historial Parquet",
            &is_cancelled,
        )?;
        check_cancellation()?;
        let expected_rows = row_count.min(PREVIEW_ROW_LIMIT);
        if page.height() != expected_rows {
            return Err(
                "El snapshot del historial no coincide con su conteo registrado.".to_owned(),
            );
        }
        page
    };
    let was_source_backed = dataset.source_backed;
    let preview_size = if was_source_backed {
        snapshot_size
    } else {
        dataset.file_size_bytes
    };
    let preview = dataset_preview_from_schema_and_page(
        &dataset.file_name,
        preview_size,
        row_count,
        &schema,
        &page,
    )?;
    check_cancellation()?;

    let publish = || {
        dataset.history.cursor = target;
        dataset.history.unsaved_current = None;
        dataset.history.degraded_reason = None;
        dataset.history.current_label = label;
        dataset.history.source_snapshot_path = None;
        dataset.row_count = row_count;
        dataset.frame = schema;
        dataset.history.touch();
        dataset.profile = None;
        if was_source_backed {
            dataset.source_path = Some(snapshot_path);
            dataset.file_size_bytes = snapshot_size;
            dataset.source_backed = true;
        }

        Ok(HistoryResult {
            dataset: preview,
            history: dataset.history.state(),
            message: message.to_owned(),
        })
    };
    if let Some(cancellation) = cancellation {
        cancellation.commit(publish)
    } else {
        publish()
    }
}
pub(super) async fn redo_last_change_impl(app: AppHandle) -> Result<HistoryResult, String> {
    let cancellation = PrepareCancellation::begin(&app);
    tauri::async_runtime::spawn_blocking(move || {
        cancellation.ensure()?;
        let state = app.state::<DatasetState>();
        let mut current = state.current.lock_recovering();
        let dataset = current.as_mut().ok_or_else(|| {
            "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
        })?;
        redo_dataset_with_cancellation(dataset, Some(&cancellation))
    })
    .await
    .map_err(|error| {
        crate::crash_report::task_interrupted("No se pudo rehacer el cambio", &error)
    })?
}
pub(super) fn get_history_state_impl(
    state: State<'_, DatasetState>,
) -> Result<HistoryState, String> {
    let current = state.current.lock_recovering();
    let dataset = current.as_ref().ok_or_else(|| {
        "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
    })?;
    Ok(dataset.history.state())
}

#[cfg(test)]
pub(super) fn undo_dataset(dataset: &mut LoadedDataset) -> Result<HistoryResult, String> {
    undo_dataset_with_cancellation(dataset, None)
}

pub(super) fn undo_dataset_with_cancellation(
    dataset: &mut LoadedDataset,
    cancellation: Option<&PrepareCancellation>,
) -> Result<HistoryResult, String> {
    if let Some(cancellation) = cancellation {
        cancellation.ensure()?;
    }
    if !dataset.history.state().can_undo {
        return Err("No hay un cambio disponible para deshacer.".to_owned());
    }
    // DAT-07: an unsaved current version goes back to the last saved one.
    let target = if dataset.history.unsaved_current.is_some() {
        dataset.history.cursor
    } else {
        dataset.history.cursor - 1
    };
    if dataset.source_backed {
        return restore_source_backed_history_cursor(
            dataset,
            target,
            "Se deshizo el último cambio.",
            cancellation,
        );
    }
    let previous = dataset.history.restore_with_cancel(target, || {
        cancellation.is_some_and(|token| token.is_cancelled())
    })?;
    let preview = loaded_dataset_preview(dataset, &previous)?;
    let publish = || {
        dataset.row_count = previous.height();
        dataset.frame = previous;
        dataset.history.touch();
        dataset.source_backed = false;
        dataset.history.cursor = target;
        dataset.history.unsaved_current = None;
        dataset.history.degraded_reason = None;
        dataset.profile = None;
        Ok(HistoryResult {
            dataset: preview,
            history: dataset.history.state(),
            message: "Se deshizo el último cambio.".to_owned(),
        })
    };
    if let Some(cancellation) = cancellation {
        cancellation.commit(publish)
    } else {
        publish()
    }
}

#[cfg(test)]
pub(super) fn redo_dataset(dataset: &mut LoadedDataset) -> Result<HistoryResult, String> {
    redo_dataset_with_cancellation(dataset, None)
}

pub(super) fn redo_dataset_with_cancellation(
    dataset: &mut LoadedDataset,
    cancellation: Option<&PrepareCancellation>,
) -> Result<HistoryResult, String> {
    if let Some(cancellation) = cancellation {
        cancellation.ensure()?;
    }
    if !dataset.history.state().can_redo {
        return Err("No hay un cambio disponible para rehacer.".to_owned());
    }
    let target = dataset.history.cursor + 1;
    if dataset.source_backed {
        return restore_source_backed_history_cursor(
            dataset,
            target,
            "Se rehízo el último cambio.",
            cancellation,
        );
    }
    let next = dataset.history.restore_with_cancel(target, || {
        cancellation.is_some_and(|token| token.is_cancelled())
    })?;
    let preview = loaded_dataset_preview(dataset, &next)?;
    let publish = || {
        dataset.row_count = next.height();
        dataset.frame = next;
        dataset.history.touch();
        dataset.source_backed = false;
        dataset.history.cursor = target;
        dataset.profile = None;
        Ok(HistoryResult {
            dataset: preview,
            history: dataset.history.state(),
            message: "Se rehízo el último cambio.".to_owned(),
        })
    };
    if let Some(cancellation) = cancellation {
        cancellation.commit(publish)
    } else {
        publish()
    }
}
