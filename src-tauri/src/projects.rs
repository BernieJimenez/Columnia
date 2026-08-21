use std::{
    fs::{self},
    path::{Path, PathBuf},
    sync::Mutex,
};

use chrono::{SecondsFormat, Utc};
use polars::prelude::{DataFrame, ParquetWriter};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::dataset::{DatasetPreview, DatasetState};

const SCHEMA_VERSION: i64 = 1;
const ID_LENGTH: usize = 32;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSummary {
    pub id: String,
    pub name: String,
    pub dataset_file_name: String,
    pub row_count: usize,
    pub column_count: usize,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectOpenResult {
    pub project: ProjectSummary,
    pub dataset: DatasetPreview,
}

pub struct ProjectState {
    store: ProjectStore,
    operation: Mutex<()>,
}

impl ProjectState {
    pub fn initialize(app_data_dir: PathBuf) -> Result<Self, String> {
        Ok(Self {
            store: ProjectStore::initialize(app_data_dir)?,
            operation: Mutex::new(()),
        })
    }
}

#[derive(Clone)]
struct ProjectStore {
    root: PathBuf,
    snapshots: PathBuf,
    catalog: PathBuf,
}

#[derive(Debug)]
struct StoredProject {
    summary: ProjectSummary,
    snapshot_name: String,
}

impl ProjectStore {
    fn initialize(root: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(&root).map_err(|_| storage_error())?;
        let root = fs::canonicalize(&root).map_err(|_| storage_error())?;
        let snapshots = root.join("project-snapshots");
        fs::create_dir_all(&snapshots).map_err(|_| storage_error())?;
        let snapshots = fs::canonicalize(&snapshots).map_err(|_| storage_error())?;
        if snapshots.parent() != Some(root.as_path()) {
            return Err(storage_error());
        }
        let store = Self {
            catalog: root.join("projects.sqlite3"),
            root,
            snapshots,
        };
        store.migrate()?;
        Ok(store)
    }

    fn connection(&self) -> Result<Connection, String> {
        let connection = Connection::open(&self.catalog).map_err(|_| storage_error())?;
        connection
            .execute_batch(
                "PRAGMA foreign_keys = ON;
                 PRAGMA journal_mode = WAL;
                 PRAGMA synchronous = FULL;
                 PRAGMA busy_timeout = 5000;",
            )
            .map_err(|_| storage_error())?;
        Ok(connection)
    }

    fn migrate(&self) -> Result<(), String> {
        let mut connection = self.connection()?;
        let version: i64 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(|_| storage_error())?;
        if version > SCHEMA_VERSION {
            return Err(
                "El catálogo de proyectos pertenece a una versión más reciente.".to_owned(),
            );
        }
        if version == 0 {
            let transaction = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(|_| storage_error())?;
            transaction
                .execute_batch(
                    "CREATE TABLE projects (
                       id TEXT PRIMARY KEY NOT NULL,
                       name TEXT NOT NULL,
                       dataset_file_name TEXT NOT NULL,
                       row_count INTEGER NOT NULL CHECK (row_count >= 0),
                       column_count INTEGER NOT NULL CHECK (column_count >= 0),
                       snapshot_name TEXT NOT NULL UNIQUE,
                       created_at TEXT NOT NULL,
                       updated_at TEXT NOT NULL,
                       last_opened_at TEXT
                     );
                     CREATE INDEX projects_updated_at ON projects(updated_at DESC, id ASC);
                     PRAGMA user_version = 1;",
                )
                .map_err(|_| storage_error())?;
            transaction.commit().map_err(|_| storage_error())?;
        }
        Ok(())
    }

    fn list(&self) -> Result<Vec<ProjectSummary>, String> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare(
                "SELECT id, name, dataset_file_name, row_count, column_count, created_at, updated_at
                 FROM projects ORDER BY updated_at DESC, id ASC",
            )
            .map_err(|_| storage_error())?;
        let rows = statement
            .query_map([], summary_from_row)
            .map_err(|_| storage_error())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|_| storage_error())
    }

    fn recovery_candidate(&self) -> Result<Option<ProjectSummary>, String> {
        let connection = self.connection()?;
        connection
            .query_row(
                "SELECT id, name, dataset_file_name, row_count, column_count, created_at, updated_at
                 FROM projects WHERE last_opened_at IS NOT NULL
                 ORDER BY last_opened_at DESC, id ASC LIMIT 1",
                [],
                summary_from_row,
            )
            .optional()
            .map_err(|_| storage_error())
    }

    fn save(
        &self,
        dataset_state: &DatasetState,
        project_id: Option<String>,
        name: String,
    ) -> Result<ProjectSummary, String> {
        self.save_inner(dataset_state, project_id, name, false)
    }

    fn save_inner(
        &self,
        dataset_state: &DatasetState,
        project_id: Option<String>,
        name: String,
        fail_before_database: bool,
    ) -> Result<ProjectSummary, String> {
        let name = validate_name(name)?;
        if let Some(id) = project_id.as_deref() {
            validate_id(id)?;
        }
        let active = dataset_state.active_project_snapshot()?;
        let mut connection = self.connection()?;
        let updating = project_id.is_some();
        let id = match project_id {
            Some(id) => id,
            None => unused_random_id(&connection)?,
        };
        let existing = self.stored_project(&connection, &id)?;
        if existing.is_none() && updating {
            return Err("El proyecto solicitado no existe.".to_owned());
        }
        let generation = random_id(&connection)?;
        let snapshot_name = format!("{id}-{generation}.parquet");
        let snapshot_path = self.snapshots.join(&snapshot_name);
        let row_count = usize_to_i64(active.row_count)?;
        let column_count = usize_to_i64(active.column_count)?;
        write_snapshot(&active.frame, &snapshot_path)?;

        if fail_before_database {
            let _ = fs::remove_file(&snapshot_path);
            return Err(storage_error());
        }

        let timestamp = now_utc();
        let old_snapshot = existing
            .as_ref()
            .map(|project| project.snapshot_name.clone());
        let created_at = existing
            .as_ref()
            .map(|project| project.summary.created_at.clone())
            .unwrap_or_else(|| timestamp.clone());
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| {
                let _ = fs::remove_file(&snapshot_path);
                storage_error()
            })?;
        let database_result = if existing.is_some() {
            transaction.execute(
                "UPDATE projects SET name = ?1, dataset_file_name = ?2, row_count = ?3,
                 column_count = ?4, snapshot_name = ?5, updated_at = ?6, last_opened_at = ?6
                 WHERE id = ?7",
                params![
                    name,
                    active.file_name,
                    row_count,
                    column_count,
                    snapshot_name,
                    timestamp,
                    id
                ],
            )
        } else {
            transaction.execute(
                "INSERT INTO projects
                 (id, name, dataset_file_name, row_count, column_count, snapshot_name,
                  created_at, updated_at, last_opened_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7, ?7)",
                params![
                    id,
                    name,
                    active.file_name,
                    row_count,
                    column_count,
                    snapshot_name,
                    timestamp
                ],
            )
        };
        if !matches!(database_result, Ok(1)) || transaction.commit().is_err() {
            let _ = fs::remove_file(&snapshot_path);
            return Err(storage_error());
        }
        if let Some(old_snapshot) = old_snapshot {
            if let Ok(old_path) = self.snapshot_path(&id, &old_snapshot) {
                let _ = fs::remove_file(old_path);
            }
        }
        Ok(ProjectSummary {
            id,
            name,
            dataset_file_name: active.file_name,
            row_count: active.row_count,
            column_count: active.column_count,
            created_at,
            updated_at: timestamp,
        })
    }

    fn open(
        &self,
        dataset_state: &DatasetState,
        project_id: String,
    ) -> Result<ProjectOpenResult, String> {
        validate_id(&project_id)?;
        let mut connection = self.connection()?;
        let stored = self
            .stored_project(&connection, &project_id)?
            .ok_or_else(|| "El proyecto solicitado no existe.".to_owned())?;
        let snapshot_path = self.snapshot_path(&project_id, &stored.snapshot_name)?;
        let candidate = DatasetState::prepare_project_candidate(
            snapshot_path,
            stored.summary.dataset_file_name.clone(),
        )?;
        if candidate.dimensions() != (stored.summary.row_count, stored.summary.column_count) {
            return Err("El snapshot del proyecto no coincide con su catálogo.".to_owned());
        }
        let timestamp = now_utc();
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| storage_error())?;
        if transaction
            .execute(
                "UPDATE projects SET last_opened_at = ?1 WHERE id = ?2",
                params![timestamp, project_id],
            )
            .map_err(|_| storage_error())?
            != 1
        {
            return Err("El proyecto solicitado no existe.".to_owned());
        }
        transaction.commit().map_err(|_| storage_error())?;
        let dataset = dataset_state.activate_project_candidate(candidate)?;
        Ok(ProjectOpenResult {
            project: stored.summary,
            dataset,
        })
    }

    fn delete(&self, project_id: String) -> Result<(), String> {
        validate_id(&project_id)?;
        let mut connection = self.connection()?;
        let stored = self
            .stored_project(&connection, &project_id)?
            .ok_or_else(|| "El proyecto solicitado no existe.".to_owned())?;
        let snapshot_path = self.managed_snapshot_path(&project_id, &stored.snapshot_name)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| storage_error())?;
        if transaction
            .execute("DELETE FROM projects WHERE id = ?1", params![project_id])
            .map_err(|_| storage_error())?
            != 1
        {
            return Err("El proyecto solicitado no existe.".to_owned());
        }
        transaction.commit().map_err(|_| storage_error())?;
        if snapshot_path.exists() {
            reject_link_or_reparse(&snapshot_path)?;
        }
        match fs::remove_file(snapshot_path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => {
                Err("El proyecto se eliminó, pero no se pudo limpiar su snapshot.".to_owned())
            }
        }
    }

    fn stored_project(
        &self,
        connection: &Connection,
        id: &str,
    ) -> Result<Option<StoredProject>, String> {
        connection
            .query_row(
                "SELECT id, name, dataset_file_name, row_count, column_count, created_at,
                        updated_at, snapshot_name FROM projects WHERE id = ?1",
                params![id],
                |row| {
                    Ok(StoredProject {
                        summary: summary_from_row(row)?,
                        snapshot_name: row.get(7)?,
                    })
                },
            )
            .optional()
            .map_err(|_| storage_error())
    }

    fn snapshot_path(&self, id: &str, snapshot_name: &str) -> Result<PathBuf, String> {
        let candidate = self.managed_snapshot_path(id, snapshot_name)?;
        reject_link_or_reparse(&candidate)?;
        let canonical = fs::canonicalize(&candidate)
            .map_err(|_| "El snapshot del proyecto no está disponible.".to_owned())?;
        if canonical.parent() != Some(self.snapshots.as_path())
            || !canonical.starts_with(&self.root)
            || !canonical.is_file()
        {
            return Err(storage_error());
        }
        Ok(canonical)
    }

    fn managed_snapshot_path(&self, id: &str, snapshot_name: &str) -> Result<PathBuf, String> {
        validate_id(id)?;
        let prefix = format!("{id}-");
        let generation = snapshot_name
            .strip_prefix(&prefix)
            .and_then(|name| name.strip_suffix(".parquet"))
            .ok_or_else(storage_error)?;
        validate_id(generation)?;
        Ok(self.snapshots.join(snapshot_name))
    }
}

fn summary_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProjectSummary> {
    let row_count: i64 = row.get(3)?;
    let column_count: i64 = row.get(4)?;
    Ok(ProjectSummary {
        id: row.get(0)?,
        name: row.get(1)?,
        dataset_file_name: row.get(2)?,
        row_count: usize::try_from(row_count)
            .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(3, row_count))?,
        column_count: usize::try_from(column_count)
            .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(4, column_count))?,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

fn validate_name(name: String) -> Result<String, String> {
    let name = name.trim().to_owned();
    let length = name.chars().count();
    if !(1..=128).contains(&length) {
        return Err("El nombre del proyecto debe tener entre 1 y 128 caracteres.".to_owned());
    }
    Ok(name)
}

fn validate_id(id: &str) -> Result<(), String> {
    if id.len() != ID_LENGTH
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err("El identificador del proyecto no es válido.".to_owned());
    }
    Ok(())
}

fn random_id(connection: &Connection) -> Result<String, String> {
    connection
        .query_row("SELECT lower(hex(randomblob(16)))", [], |row| row.get(0))
        .map_err(|_| storage_error())
}

fn unused_random_id(connection: &Connection) -> Result<String, String> {
    for _ in 0..8 {
        let id = random_id(connection)?;
        let exists: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM projects WHERE id = ?1)",
                params![id],
                |row| row.get(0),
            )
            .map_err(|_| storage_error())?;
        if !exists {
            return Ok(id);
        }
    }
    Err(storage_error())
}

fn now_utc() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn usize_to_i64(value: usize) -> Result<i64, String> {
    i64::try_from(value).map_err(|_| storage_error())
}

fn storage_error() -> String {
    "No se pudo acceder al almacenamiento privado de proyectos.".to_owned()
}

fn write_snapshot(frame: &DataFrame, destination: &Path) -> Result<(), String> {
    if destination.exists() {
        return Err(storage_error());
    }
    let parent = destination.parent().ok_or_else(storage_error)?;
    let temporary = tempfile::NamedTempFile::new_in(parent).map_err(|_| storage_error())?;
    let mut snapshot = frame.clone();
    ParquetWriter::new(temporary.as_file())
        .finish(&mut snapshot)
        .map_err(|_| storage_error())?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|_| storage_error())?;
    temporary
        .persist_noclobber(destination)
        .map_err(|_| storage_error())?;
    if sync_directory(parent).is_err() {
        let _ = fs::remove_file(destination);
        return Err(storage_error());
    }
    Ok(())
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<(), String> {
    fs::File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| storage_error())
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> Result<(), String> {
    Ok(())
}

fn reject_link_or_reparse(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| "El snapshot del proyecto no está disponible.".to_owned())?;
    if metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
        return Err("El snapshot del proyecto no es un archivo administrado válido.".to_owned());
    }
    Ok(())
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

async fn run_project_operation<T, F>(app: AppHandle, operation: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(&ProjectStore, &DatasetState) -> Result<T, String> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(move || {
        let project_state = app.state::<ProjectState>();
        let _guard = project_state
            .operation
            .lock()
            .map_err(|_| "El catálogo de proyectos no está disponible.".to_owned())?;
        let dataset_state = app.state::<DatasetState>();
        operation(&project_state.store, &dataset_state)
    })
    .await
    .map_err(|_| "La operación de proyectos se interrumpió.".to_owned())?
}

#[tauri::command]
pub async fn list_projects(app: AppHandle) -> Result<Vec<ProjectSummary>, String> {
    run_project_operation(app, |store, _| store.list()).await
}

#[tauri::command]
pub async fn get_recovery_candidate(app: AppHandle) -> Result<Option<ProjectSummary>, String> {
    run_project_operation(app, |store, _| store.recovery_candidate()).await
}

#[tauri::command]
pub async fn save_project(
    app: AppHandle,
    project_id: Option<String>,
    name: String,
) -> Result<ProjectSummary, String> {
    run_project_operation(app, move |store, dataset| {
        store.save(dataset, project_id, name)
    })
    .await
}

#[tauri::command]
pub async fn open_project(app: AppHandle, project_id: String) -> Result<ProjectOpenResult, String> {
    run_project_operation(app, move |store, dataset| store.open(dataset, project_id)).await
}

#[tauri::command]
pub async fn delete_project(app: AppHandle, project_id: String) -> Result<(), String> {
    run_project_operation(app, move |store, _| store.delete(project_id)).await
}

// El historial durable queda deliberadamente fuera de proyectos v1. Cada apertura crea
// un HistoryManager temporal nuevo a partir del snapshot persistente actual.

#[cfg(test)]
mod tests {
    use super::*;
    use polars::prelude::{DataFrame, IntoColumn, NamedFrom, Series};

    fn frame(values: &[i64]) -> DataFrame {
        DataFrame::new(
            values.len(),
            vec![
                Series::new("value".into(), values).into_column(),
                Series::new(
                    "label".into(),
                    values
                        .iter()
                        .map(|value| format!("row-{value}"))
                        .collect::<Vec<_>>(),
                )
                .into_column(),
            ],
        )
        .expect("el frame de prueba debe ser válido")
    }

    fn active_state(directory: &Path, values: &[i64], file_name: &str) -> (DatasetState, PathBuf) {
        let source = directory.join("source.parquet");
        if source.exists() {
            fs::remove_file(&source).expect("se debe reemplazar la fuente de prueba");
        }
        write_snapshot(&frame(values), &source).expect("se debe escribir la fuente");
        let candidate =
            DatasetState::prepare_project_candidate(source.clone(), file_name.to_owned())
                .expect("se debe preparar el dataset activo");
        let state = DatasetState::default();
        state
            .activate_project_candidate(candidate)
            .expect("se debe activar el dataset");
        (state, source)
    }

    fn snapshot_count(store: &ProjectStore) -> usize {
        fs::read_dir(&store.snapshots)
            .expect("se debe leer la carpeta de snapshots")
            .filter_map(Result::ok)
            .count()
    }

    #[test]
    fn migration_is_versioned_and_idempotent() {
        let directory = tempfile::tempdir().unwrap();
        let first = ProjectStore::initialize(directory.path().join("data")).unwrap();
        let connection = first.connection().unwrap();
        let version: i64 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION);
        drop(connection);

        let second = ProjectStore::initialize(directory.path().join("data")).unwrap();
        assert!(second.list().unwrap().is_empty());
    }

    #[test]
    fn create_update_list_recovery_and_delete_preserve_active_dataset() {
        let directory = tempfile::tempdir().unwrap();
        let store = ProjectStore::initialize(directory.path().join("data")).unwrap();
        let (state, _) = active_state(directory.path(), &[1, 2, 3], "entrada original.csv");

        let created = store
            .save(&state, None, "  Proyecto ágil  ".to_owned())
            .unwrap();
        assert_eq!(created.name, "Proyecto ágil");
        assert_eq!(created.dataset_file_name, "entrada original.csv");
        assert_eq!((created.row_count, created.column_count), (3, 2));
        assert_eq!(created.id.len(), ID_LENGTH);
        assert!(created.created_at.ends_with('Z'));
        assert_eq!(store.list().unwrap(), vec![created.clone()]);
        assert_eq!(store.recovery_candidate().unwrap(), Some(created.clone()));
        assert_eq!(snapshot_count(&store), 1);

        let updated = store
            .save(&state, Some(created.id.clone()), "Renombrado".to_owned())
            .unwrap();
        assert_eq!(updated.id, created.id);
        assert_eq!(updated.created_at, created.created_at);
        assert_eq!(updated.name, "Renombrado");
        assert_eq!(snapshot_count(&store), 1, "el snapshot anterior se limpia");

        store.delete(created.id).unwrap();
        assert!(store.list().unwrap().is_empty());
        assert_eq!(snapshot_count(&store), 0);
        assert_eq!(state.active_project_snapshot().unwrap().row_count, 3);
    }

    #[test]
    fn snapshot_survives_source_deletion_and_real_catalog_restart() {
        let directory = tempfile::tempdir().unwrap();
        let data_root = directory.path().join("data");
        let store = ProjectStore::initialize(data_root.clone()).unwrap();
        let (state, source) = active_state(directory.path(), &[7, 8], "ventas.csv");
        let project = store.save(&state, None, "Ventas".to_owned()).unwrap();
        fs::remove_file(source).unwrap();
        drop(store);

        let reopened = ProjectStore::initialize(data_root).unwrap();
        let restored_state = DatasetState::default();
        let result = reopened.open(&restored_state, project.id.clone()).unwrap();
        assert_eq!(result.dataset.file_name, "ventas.csv");
        assert_eq!(result.project, project);
        let restored = restored_state.active_project_snapshot().unwrap();
        assert_eq!(restored.file_name, "ventas.csv");
        assert!(restored.frame.equals_missing(&frame(&[7, 8])));
        assert_eq!(reopened.recovery_candidate().unwrap(), Some(project));
    }

    #[test]
    fn database_failure_removes_the_new_snapshot_and_keeps_catalog_unchanged() {
        let directory = tempfile::tempdir().unwrap();
        let store = ProjectStore::initialize(directory.path().join("data")).unwrap();
        let (state, _) = active_state(directory.path(), &[1], "input.csv");

        let error = store
            .save_inner(&state, None, "Proyecto".to_owned(), true)
            .unwrap_err();
        assert_eq!(error, storage_error());
        assert!(store.list().unwrap().is_empty());
        assert_eq!(snapshot_count(&store), 0);

        let created = store.save(&state, None, "Original".to_owned()).unwrap();
        let error = store
            .save_inner(
                &state,
                Some(created.id.clone()),
                "No publicado".to_owned(),
                true,
            )
            .unwrap_err();
        assert_eq!(error, storage_error());
        assert_eq!(store.list().unwrap(), vec![created]);
        assert_eq!(snapshot_count(&store), 1);
    }

    #[test]
    fn invalid_names_and_ids_are_rejected_without_creating_files() {
        let directory = tempfile::tempdir().unwrap();
        let store = ProjectStore::initialize(directory.path().join("data")).unwrap();
        let (state, _) = active_state(directory.path(), &[1], "input.csv");

        for name in ["   ".to_owned(), "x".repeat(129)] {
            assert!(store.save(&state, None, name).is_err());
        }
        for id in ["../catalog", "ABCDEF0123456789ABCDEF0123456789", "abc"] {
            assert!(store.open(&DatasetState::default(), id.to_owned()).is_err());
            assert!(store.delete(id.to_owned()).is_err());
            assert!(store
                .save(&state, Some(id.to_owned()), "Proyecto".to_owned())
                .is_err());
        }
        let missing = "0".repeat(ID_LENGTH);
        assert!(store
            .save(&state, Some(missing.clone()), "Proyecto".to_owned())
            .is_err());
        assert!(store
            .open(&DatasetState::default(), missing.clone())
            .is_err());
        assert!(store.delete(missing).is_err());
        assert_eq!(snapshot_count(&store), 0);
    }

    #[test]
    fn serialized_contracts_and_errors_never_expose_managed_paths() {
        let directory = tempfile::tempdir().unwrap();
        let store = ProjectStore::initialize(directory.path().join("private-root")).unwrap();
        let (state, _) = active_state(directory.path(), &[1], "safe.csv");
        let project = store.save(&state, None, "Seguro".to_owned()).unwrap();
        let json = serde_json::to_string(&project).unwrap();
        assert!(!json.contains(directory.path().to_string_lossy().as_ref()));
        assert!(!json.contains("snapshot"));

        store
            .connection()
            .unwrap()
            .execute(
                "UPDATE projects SET snapshot_name = ?1 WHERE id = ?2",
                params!["../escape.parquet", project.id],
            )
            .unwrap();
        let error = store
            .open(&DatasetState::default(), project.id)
            .unwrap_err();
        assert!(!error.contains(directory.path().to_string_lossy().as_ref()));
    }

    #[test]
    fn open_rejects_catalog_dimension_tampering_without_replacing_active_dataset() {
        let directory = tempfile::tempdir().unwrap();
        let store = ProjectStore::initialize(directory.path().join("data")).unwrap();
        let (source_state, _) = active_state(directory.path(), &[1, 2], "input.csv");
        let project = store
            .save(&source_state, None, "Proyecto".to_owned())
            .unwrap();
        store
            .connection()
            .unwrap()
            .execute(
                "UPDATE projects SET row_count = 99 WHERE id = ?1",
                params![project.id],
            )
            .unwrap();
        let (target_state, _) = active_state(directory.path(), &[42], "existing.csv");

        assert!(store.open(&target_state, project.id).is_err());
        let active = target_state.active_project_snapshot().unwrap();
        assert_eq!(active.file_name, "existing.csv");
        assert_eq!(active.row_count, 1);
    }

    #[cfg(unix)]
    #[test]
    fn open_rejects_symbolic_snapshot_links() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let store = ProjectStore::initialize(directory.path().join("data")).unwrap();
        let (state, _) = active_state(directory.path(), &[1], "input.csv");
        let project = store.save(&state, None, "Proyecto".to_owned()).unwrap();
        let stored = store
            .stored_project(&store.connection().unwrap(), &project.id)
            .unwrap()
            .unwrap();
        let snapshot = store
            .managed_snapshot_path(&project.id, &stored.snapshot_name)
            .unwrap();
        let outside = directory.path().join("outside.parquet");
        fs::rename(&snapshot, &outside).unwrap();
        symlink(&outside, &snapshot).unwrap();

        assert!(store.open(&DatasetState::default(), project.id).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn open_rejects_windows_snapshot_reparse_points_when_supported() {
        use std::{io::ErrorKind, os::windows::fs::symlink_file};

        let directory = tempfile::tempdir().unwrap();
        let store = ProjectStore::initialize(directory.path().join("data")).unwrap();
        let (state, _) = active_state(directory.path(), &[1], "input.csv");
        let project = store.save(&state, None, "Proyecto".to_owned()).unwrap();
        let stored = store
            .stored_project(&store.connection().unwrap(), &project.id)
            .unwrap()
            .unwrap();
        let snapshot = store
            .managed_snapshot_path(&project.id, &stored.snapshot_name)
            .unwrap();
        let outside = directory.path().join("outside.parquet");
        fs::rename(&snapshot, &outside).unwrap();
        match symlink_file(&outside, &snapshot) {
            Ok(()) => assert!(store.open(&DatasetState::default(), project.id).is_err()),
            Err(error) if error.kind() == ErrorKind::PermissionDenied => {}
            Err(error) => panic!("no se pudo crear el reparse point de prueba: {error}"),
        }
    }
}
