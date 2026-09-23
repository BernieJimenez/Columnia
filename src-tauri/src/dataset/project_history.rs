use super::*;

pub(crate) struct ActiveDatasetSnapshot {
    pub(crate) frame: DataFrame,
    pub(crate) current_snapshot_path: Option<PathBuf>,
    pub(super) _current_snapshot_guard: Option<tempfile::TempPath>,
    pub(crate) file_name: String,
    pub(crate) row_count: usize,
    pub(crate) column_count: usize,
    pub(crate) profile: Option<DatasetProfile>,
    pub(crate) history: ProjectHistoryCapture,
}

pub(crate) struct ProjectHistoryCaptureEntry {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) path: PathBuf,
    pub(crate) bytes: u64,
}

pub(crate) struct ProjectHistoryCapture {
    _directory: tempfile::TempDir,
    pub(crate) entries: Vec<ProjectHistoryCaptureEntry>,
    pub(crate) cursor: usize,
    pub(crate) snapshots_enabled: bool,
    pub(crate) degraded_reason: Option<String>,
    pub(crate) current_label: String,
    pub(crate) max_entries: usize,
    pub(crate) disk_budget_bytes: u64,
}

pub(crate) struct ProjectHistoryRestoreEntry {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) path: PathBuf,
    pub(crate) bytes: u64,
}

pub(crate) struct ProjectHistoryRestore {
    pub(crate) entries: Vec<ProjectHistoryRestoreEntry>,
    pub(crate) cursor: usize,
    pub(crate) snapshots_enabled: bool,
    pub(crate) degraded_reason: Option<String>,
    pub(crate) current_label: String,
    pub(crate) max_entries: usize,
    pub(crate) disk_budget_bytes: u64,
}

pub(crate) struct ProjectDatasetCandidate {
    pub(super) loaded: LoadedDataset,
    pub(super) preview: DatasetPreview,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProjectHistorySummary {
    pub(crate) entry_count: usize,
    pub(crate) current_index: usize,
    pub(crate) can_undo: bool,
    pub(crate) can_redo: bool,
    pub(crate) snapshots_enabled: bool,
    pub(crate) degraded: bool,
}

impl ProjectDatasetCandidate {
    pub(crate) fn dimensions(&self) -> (usize, usize) {
        (self.loaded.row_count, self.loaded.frame.width())
    }

    pub(crate) fn frame(&self) -> &DataFrame {
        &self.loaded.frame
    }

    pub(crate) fn history_summary(&self) -> ProjectHistorySummary {
        let state = self.loaded.history.state();
        ProjectHistorySummary {
            entry_count: state.entry_count,
            current_index: state.current_index,
            can_undo: state.can_undo,
            can_redo: state.can_redo,
            snapshots_enabled: state.snapshots_enabled,
            degraded: state.degraded_reason.is_some(),
        }
    }

    pub(crate) fn into_frame(self) -> DataFrame {
        self.loaded.frame
    }

    pub(crate) fn is_source_backed(&self) -> bool {
        self.loaded.source_backed
    }

    pub(crate) fn into_dataset_state(self) -> DatasetState {
        DatasetState {
            current: Mutex::new(Some(self.loaded)),
            ..DatasetState::default()
        }
    }
}

fn validate_history_label(label: &str) -> Result<(), String> {
    let length = label.chars().count();
    if !(1..=256).contains(&length) {
        return Err("El historial del proyecto contiene una etiqueta no válida.".to_owned());
    }
    Ok(())
}

pub(super) fn validate_history_entry_id(id: &str) -> Result<(), String> {
    if !(8..=96).contains(&id.len())
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return Err("Un ID de revisión del historial no es válido.".to_owned());
    }
    Ok(())
}

pub(super) fn capture_project_history_with_cancel<C>(
    history: &HistoryManager,
    current_frame: &DataFrame,
    is_cancelled: C,
) -> Result<ProjectHistoryCapture, String>
where
    C: Fn() -> bool + Sync,
{
    ensure_not_cancelled(is_cancelled())?;
    if history.max_entries == 0
        || history.max_entries > HISTORY_MAX_ENTRIES
        || history.disk_budget_bytes > HISTORY_DISK_BUDGET_BYTES
        || history.entries.len() > history.max_entries
    {
        return Err("El historial activo supera los límites del proyecto.".to_owned());
    }
    validate_history_label(&history.current_label)?;
    if history.snapshots_enabled {
        if history.entries.is_empty()
            || history.cursor >= history.entries.len()
            || history.degraded_reason.is_some()
        {
            return Err("El historial activo no tiene un estado consistente.".to_owned());
        }
    } else if !history.entries.is_empty()
        || history.cursor != 0
        || history
            .degraded_reason
            .as_deref()
            .is_none_or(|reason| reason.trim().is_empty())
    {
        return Err("El historial degradado no tiene un estado consistente.".to_owned());
    }

    let directory = tempfile::tempdir()
        .map_err(|_| "No se pudo preparar el historial del proyecto.".to_owned())?;
    let mut entries = Vec::with_capacity(history.entries.len());
    let mut total_bytes = 0_u64;
    let mut cursor_matches = !history.snapshots_enabled;
    let mut ids = HashSet::with_capacity(history.entries.len());
    for (index, entry) in history.entries.iter().enumerate() {
        ensure_not_cancelled(is_cancelled())?;
        validate_history_label(&entry.label)?;
        validate_history_entry_id(&entry.id)?;
        if !ids.insert(entry.id.as_str()) {
            return Err("El historial activo contiene IDs de revisión repetidos.".to_owned());
        }
        let metadata = fs::symlink_metadata(&entry.path)
            .map_err(|_| "No se pudo leer el historial activo.".to_owned())?;
        if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() != entry.bytes
        {
            return Err("El historial activo contiene un snapshot no válido.".to_owned());
        }
        total_bytes = total_bytes
            .checked_add(entry.bytes)
            .ok_or_else(|| "El historial activo supera los límites del proyecto.".to_owned())?;
        if total_bytes > history.disk_budget_bytes {
            return Err("El historial activo supera el presupuesto de disco.".to_owned());
        }
        let destination = directory.path().join(format!("entry-{index:03}.parquet"));
        let mut destination_file = File::create(&destination)
            .map_err(|_| "No se pudo preparar el historial del proyecto.".to_owned())?;
        let copied = copy_file_with_cancel(&entry.path, &mut destination_file, &is_cancelled)
            .map_err(|error| {
                if error == OPERATION_CANCELLED_MESSAGE {
                    error
                } else {
                    "No se pudo preparar el historial del proyecto.".to_owned()
                }
            })?;
        if copied != entry.bytes {
            return Err("El historial activo cambió mientras se guardaba.".to_owned());
        }
        let matches = validate_staged_history_snapshot_with_cancel(
            &destination,
            current_frame,
            index == history.cursor,
            "El historial activo contiene un snapshot corrupto.",
            &is_cancelled,
        )?;
        if index == history.cursor {
            cursor_matches = matches;
        }
        entries.push(ProjectHistoryCaptureEntry {
            id: entry.id.clone(),
            label: entry.label.clone(),
            path: destination,
            bytes: copied,
        });
    }
    if !cursor_matches {
        return Err("El cursor del historial activo no coincide con el dataset.".to_owned());
    }
    ensure_not_cancelled(is_cancelled())?;
    Ok(ProjectHistoryCapture {
        _directory: directory,
        entries,
        cursor: history.cursor,
        snapshots_enabled: history.snapshots_enabled,
        degraded_reason: history.degraded_reason.clone(),
        current_label: history.current_label.clone(),
        max_entries: history.max_entries,
        disk_budget_bytes: history.disk_budget_bytes,
    })
}

pub(super) fn restore_project_history_with_cancel<C>(
    current_frame: &DataFrame,
    history: ProjectHistoryRestore,
    is_cancelled: C,
) -> Result<HistoryManager, String>
where
    C: Fn() -> bool + Sync,
{
    ensure_not_cancelled(is_cancelled())?;
    if history.max_entries == 0
        || history.max_entries > HISTORY_MAX_ENTRIES
        || history.disk_budget_bytes > HISTORY_DISK_BUDGET_BYTES
        || history.entries.len() > history.max_entries
    {
        return Err("El historial guardado supera los límites admitidos.".to_owned());
    }
    validate_history_label(&history.current_label)?;
    if history.snapshots_enabled {
        if history.entries.is_empty()
            || history.cursor >= history.entries.len()
            || history.degraded_reason.is_some()
        {
            return Err("El historial guardado no tiene un cursor válido.".to_owned());
        }
    } else if !history.entries.is_empty()
        || history.cursor != 0
        || history
            .degraded_reason
            .as_deref()
            .is_none_or(|reason| reason.trim().is_empty() || reason.chars().count() > 1024)
    {
        return Err("El historial degradado guardado no es válido.".to_owned());
    }

    let directory = tempfile::tempdir()
        .map_err(|_| "No se pudo preparar el historial restaurado.".to_owned())?;
    let mut entries = Vec::with_capacity(history.entries.len());
    let mut cursor_matches = !history.snapshots_enabled;
    let mut total_bytes = 0_u64;
    let mut ids = HashSet::with_capacity(history.entries.len());
    for (index, entry) in history.entries.into_iter().enumerate() {
        ensure_not_cancelled(is_cancelled())?;
        validate_history_label(&entry.label)?;
        let id = if entry.id.is_empty() {
            // Manifiestos anteriores a D03 no incluían IDs estables.
            fresh_history_entry_id()
        } else {
            validate_history_entry_id(&entry.id)?;
            entry.id
        };
        if !ids.insert(id.clone()) {
            return Err("El historial guardado contiene IDs de revisión repetidos.".to_owned());
        }
        let metadata = fs::symlink_metadata(&entry.path)
            .map_err(|_| "Un snapshot del historial no está disponible.".to_owned())?;
        if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() != entry.bytes
        {
            return Err("Un snapshot del historial no es válido.".to_owned());
        }
        total_bytes = total_bytes
            .checked_add(entry.bytes)
            .ok_or_else(|| "El historial guardado supera su presupuesto.".to_owned())?;
        if total_bytes > history.disk_budget_bytes {
            return Err("El historial guardado supera su presupuesto.".to_owned());
        }
        let destination = directory
            .path()
            .join(format!("snapshot-{index:020}.parquet"));
        let mut destination_file = File::create(&destination)
            .map_err(|_| "No se pudo copiar el historial restaurado.".to_owned())?;
        let copied = copy_file_with_cancel(&entry.path, &mut destination_file, &is_cancelled)
            .map_err(|error| {
                if error == OPERATION_CANCELLED_MESSAGE {
                    error
                } else {
                    "No se pudo copiar el historial restaurado.".to_owned()
                }
            })?;
        ensure_not_cancelled(is_cancelled())?;
        destination_file
            .sync_all()
            .map_err(|_| "No se pudo sincronizar el historial restaurado.".to_owned())?;
        if copied != entry.bytes {
            return Err("Un snapshot del historial cambió durante la apertura.".to_owned());
        }
        let matches = validate_staged_history_snapshot_with_cancel(
            &destination,
            current_frame,
            index == history.cursor,
            "Un snapshot del historial no contiene un Parquet válido.",
            &is_cancelled,
        )?;
        if index == history.cursor {
            cursor_matches = matches;
        }
        entries.push(HistoryEntry {
            id,
            label: entry.label,
            path: destination,
            bytes: copied,
        });
    }
    if !cursor_matches {
        return Err("El cursor del historial no coincide con el dataset actual.".to_owned());
    }
    ensure_not_cancelled(is_cancelled())?;
    let next_id = entries.len() as u64;
    Ok(HistoryManager {
        directory,
        source_snapshot_path: None,
        entries,
        cursor: history.cursor,
        snapshots_enabled: history.snapshots_enabled,
        degraded_reason: history.degraded_reason,
        current_label: history.current_label,
        next_id,
        max_entries: history.max_entries,
        disk_budget_bytes: history.disk_budget_bytes,
    })
}
