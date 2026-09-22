use super::{
    fresh_history_entry_id, read_parquet_frame, DatasetPreview, HISTORY_DISK_BUDGET_BYTES,
    HISTORY_MAX_ENTRIES, HISTORY_SNAPSHOT_BATCH_ROWS, OPERATION_CANCELLED_MESSAGE,
};
use polars::prelude::{DataFrame, ParquetWriter};
use serde::Serialize;
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
};

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

#[derive(Debug)]
pub(super) struct HistoryManager {
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
}

impl HistoryManager {
    pub(super) fn deferred() -> Result<Self, String> {
        let directory = tempfile::tempdir()
            .map_err(|error| format!("No se pudo crear el historial temporal: {error}"))?;
        Ok(Self {
            directory,
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
        let directory = tempfile::tempdir()
            .map_err(|error| format!("No se pudo crear el historial temporal: {error}"))?;
        let mut manager = Self {
            directory,
            source_snapshot_path: None,
            entries: Vec::new(),
            cursor: 0,
            snapshots_enabled: true,
            degraded_reason: None,
            current_label: "Dataset original".to_owned(),
            next_id: 0,
            max_entries: max_entries.max(1),
            disk_budget_bytes,
        };
        manager.record(frame, "Dataset original")?;
        Ok(manager)
    }

    pub(super) fn disk_bytes(&self) -> u64 {
        self.entries.iter().map(|entry| entry.bytes).sum()
    }

    pub(super) fn state(&self) -> HistoryState {
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
        }
    }

    pub(super) fn disable_for_size(&mut self, label: &str, bytes: u64) {
        for path in self.disable_for_size_deferred(label, bytes) {
            let _ = fs::remove_file(path);
        }
    }

    pub(super) fn disable_for_size_deferred(&mut self, label: &str, bytes: u64) -> Vec<PathBuf> {
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
        self.current_label = label.to_owned();
        if !self.snapshots_enabled {
            return Ok(());
        }

        let temporary = tempfile::NamedTempFile::new_in(self.directory.path())
            .map_err(|error| format!("No se pudo preparar el snapshot del historial: {error}"))?;
        let mut snapshot = frame.clone();
        ParquetWriter::new(temporary.as_file())
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
        self.next_id = self.next_id.wrapping_add(1);

        while self.entries.len() > self.max_entries || self.disk_bytes() > self.disk_budget_bytes {
            let entry = self.entries.remove(0);
            let _ = fs::remove_file(entry.path);
            self.cursor = self.cursor.saturating_sub(1);
        }
        Ok(())
    }

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
        let mut offset = 0;
        while offset < frame.height() {
            if is_cancelled() {
                return Err(OPERATION_CANCELLED_MESSAGE.to_owned());
            }
            let row_count = HISTORY_SNAPSHOT_BATCH_ROWS.min(frame.height() - offset);
            let batch = frame.slice(offset as i64, row_count);
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

    pub(super) fn restore(&self, index: usize) -> Result<DataFrame, String> {
        let entry = self
            .entries
            .get(index)
            .ok_or_else(|| "La revisión solicitada ya no está disponible.".to_owned())?;
        read_parquet_frame(&entry.path)
            .map_err(|error| format!("No se pudo restaurar el snapshot del historial: {error}"))
    }

    pub(super) fn restore_by_id(&self, id: &str) -> Result<(DataFrame, String), String> {
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
        let frame = read_parquet_frame(&entry.path)
            .map_err(|_| "No se pudo leer una revisión del historial.".to_owned())?;
        Ok((frame, entry.label.clone()))
    }

    pub(super) fn contains_id(&self, id: &str) -> bool {
        self.snapshots_enabled && self.entries.iter().any(|entry| entry.id == id)
    }
}
