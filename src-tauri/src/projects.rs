use std::{
    collections::HashSet,
    fs::{self},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
};

use chrono::{SecondsFormat, Utc};
use polars::prelude::{DataFrame, ParquetWriter};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;

use crate::dataset::{
    self, validate_project_profile, validate_project_workspace, DatasetPreview, DatasetProfile,
    DatasetState, ProjectHistoryCapture, ProjectHistoryRestore, ProjectHistoryRestoreEntry,
    QualityRule, SpreadsheetHeaderMode, StoredTransformRecipe,
};
use serde_json::{Map as JsonMap, Value as JsonValue};

const SCHEMA_VERSION: i64 = 3;
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
    pub workspace: ProjectWorkspace,
    pub profile: Option<DatasetProfile>,
}

#[cfg(debug_assertions)]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeProjectReopen {
    pub project: ProjectSummary,
    pub dataset_file_name: String,
    pub row_count: usize,
    pub column_count: usize,
    pub quality_rule_count: usize,
    pub recipe_draft_present: bool,
    pub recovery_candidate_present: bool,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectWorkspace {
    pub quality_rules: Vec<QualityRule>,
    pub recipe_draft: Option<StoredTransformRecipe>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AutomationHistorySummary {
    pub(crate) entry_count: usize,
    pub(crate) current_index: usize,
    pub(crate) can_undo: bool,
    pub(crate) can_redo: bool,
    pub(crate) snapshots_enabled: bool,
    pub(crate) degraded: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AutomationProjectInspection {
    pub(crate) project: ProjectSummary,
    pub(crate) profile_cached: bool,
    pub(crate) quality_rule_count: usize,
    pub(crate) recipe_draft_present: bool,
    pub(crate) history: AutomationHistorySummary,
}

pub(crate) struct AutomationOpenedProject {
    pub(crate) frame: DataFrame,
    pub(crate) workspace: ProjectWorkspace,
}

pub struct ProjectState {
    store: ProjectStore,
    operation: Mutex<()>,
}

impl ProjectState {
    pub fn initialize(app_data_dir: PathBuf) -> Result<Self, String> {
        Ok(Self {
            store: ProjectStore::initialize_deferred(app_data_dir)?,
            operation: Mutex::new(()),
        })
    }
}

#[derive(Clone)]
struct ProjectStore {
    root: PathBuf,
    snapshots: PathBuf,
    catalog: PathBuf,
    initialized: Arc<Mutex<bool>>,
}

#[derive(Debug)]
struct StoredProject {
    summary: ProjectSummary,
    snapshot_name: String,
    quality_rules_json: String,
    recipe_draft_json: Option<String>,
    generation_name: Option<String>,
    history_manifest_json: Option<String>,
    profile_json: Option<String>,
}

struct ValidatedProject {
    stored: StoredProject,
    workspace: ProjectWorkspace,
    profile: Option<DatasetProfile>,
    candidate: crate::dataset::ProjectDatasetCandidate,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DurableHistoryManifest {
    version: u32,
    entries: Vec<DurableHistoryEntry>,
    cursor: usize,
    snapshots_enabled: bool,
    degraded_reason: Option<String>,
    current_label: String,
    max_entries: usize,
    disk_budget_bytes: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DurableHistoryEntry {
    label: String,
    file_name: String,
    bytes: u64,
}

impl ProjectStore {
    fn initialize(root: PathBuf) -> Result<Self, String> {
        let store = Self::initialize_deferred(root)?;
        store.ensure_initialized()?;
        Ok(store)
    }

    fn initialize_deferred(root: PathBuf) -> Result<Self, String> {
        let root = prepare_store_directory(&root)?;
        let snapshots = prepare_store_directory(&root.join("project-snapshots"))?;
        if snapshots.parent() != Some(root.as_path()) {
            return Err(storage_error());
        }
        let store = Self {
            catalog: root.join("projects.sqlite3"),
            root,
            snapshots,
            initialized: Arc::new(Mutex::new(false)),
        };
        Ok(store)
    }

    fn ensure_initialized(&self) -> Result<(), String> {
        let mut initialized = self
            .initialized
            .lock()
            .map_err(|_| "El catálogo de proyectos no está disponible.".to_owned())?;
        if *initialized {
            return Ok(());
        }
        self.migrate()?;
        self.reconcile_orphan_generations();
        *initialized = true;
        Ok(())
    }

    fn reconcile_orphan_generations(&self) {
        let active = self
            .connection()
            .ok()
            .and_then(|connection| {
                let mut statement = connection
                    .prepare(
                        "SELECT generation_name FROM projects WHERE generation_name IS NOT NULL",
                    )
                    .ok()?;
                let rows = statement
                    .query_map([], |row| row.get::<_, String>(0))
                    .ok()?;
                Some(rows.filter_map(Result::ok).collect::<HashSet<_>>())
            })
            .unwrap_or_default();
        let _ = crate::project_recovery::reconcile_orphan_generations(
            &self.snapshots,
            &active,
            SystemTime::now(),
            Duration::from_secs(60 * 60),
        );
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
        match version {
            SCHEMA_VERSION => Ok(()),
            0 => {
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
                           last_opened_at TEXT,
                           quality_rules_json TEXT NOT NULL DEFAULT '[]',
                           recipe_draft_json TEXT,
                           generation_name TEXT,
                           history_manifest_json TEXT,
                           profile_json TEXT
                         );
                         CREATE INDEX projects_updated_at ON projects(updated_at DESC, id ASC);
                         PRAGMA user_version = 3;",
                    )
                    .map_err(|_| storage_error())?;
                transaction.commit().map_err(|_| storage_error())
            }
            1 => {
                let transaction = connection
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .map_err(|_| storage_error())?;
                transaction
                    .execute_batch(
                        "ALTER TABLE projects
                           ADD COLUMN quality_rules_json TEXT NOT NULL DEFAULT '[]';
                         ALTER TABLE projects ADD COLUMN recipe_draft_json TEXT;
                         ALTER TABLE projects ADD COLUMN generation_name TEXT;
                         ALTER TABLE projects ADD COLUMN history_manifest_json TEXT;
                         ALTER TABLE projects ADD COLUMN profile_json TEXT;
                         PRAGMA user_version = 3;",
                    )
                    .map_err(|_| storage_error())?;
                transaction.commit().map_err(|_| storage_error())
            }
            2 => {
                let transaction = connection
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .map_err(|_| storage_error())?;
                transaction
                    .execute_batch(
                        "ALTER TABLE projects ADD COLUMN generation_name TEXT;
                         ALTER TABLE projects ADD COLUMN history_manifest_json TEXT;
                         ALTER TABLE projects ADD COLUMN profile_json TEXT;
                         PRAGMA user_version = 3;",
                    )
                    .map_err(|_| storage_error())?;
                transaction.commit().map_err(|_| storage_error())
            }
            _ => Err(storage_error()),
        }
    }

    fn list(&self) -> Result<Vec<ProjectSummary>, String> {
        self.ensure_initialized()?;
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
        self.ensure_initialized()?;
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
        workspace: ProjectWorkspace,
    ) -> Result<ProjectSummary, String> {
        self.save_inner(dataset_state, project_id, name, workspace, false)
    }

    fn save_inner(
        &self,
        dataset_state: &DatasetState,
        project_id: Option<String>,
        name: String,
        workspace: ProjectWorkspace,
        fail_before_database: bool,
    ) -> Result<ProjectSummary, String> {
        self.ensure_initialized()?;
        let name = validate_name(name)?;
        if let Some(id) = project_id.as_deref() {
            validate_id(id)?;
        }
        let active = dataset_state.active_project_snapshot()?;
        validate_project_workspace(
            &active.frame,
            &workspace.quality_rules,
            workspace.recipe_draft.as_ref(),
        )?;
        if let Some(profile) = active.profile.as_ref() {
            validate_project_profile(&active.frame, profile)?;
        }
        let quality_rules_json = serde_json::to_string(&workspace.quality_rules)
            .map_err(|_| "No se pudo validar la configuración del proyecto.".to_owned())?;
        let recipe_draft_json = workspace
            .recipe_draft
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|_| "No se pudo validar la configuración del proyecto.".to_owned())?;
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
        let generation_name = format!("{id}-{generation}");
        let generation_path = self.snapshots.join(&generation_name);
        let row_count = usize_to_i64(active.row_count)?;
        let column_count = usize_to_i64(active.column_count)?;
        let history_manifest = history_manifest(&active.history);
        let history_manifest_json = serde_json::to_string(&history_manifest)
            .map_err(|_| "No se pudo validar el historial del proyecto.".to_owned())?;
        let profile_json = active
            .profile
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|_| "No se pudo validar el perfil del proyecto.".to_owned())?;
        write_generation(&active.frame, &active.history, &generation_path)?;

        if fail_before_database {
            let _ = fs::remove_dir_all(&generation_path);
            return Err(storage_error());
        }

        let timestamp = now_utc();
        let old_snapshot = existing
            .as_ref()
            .map(|project| project.snapshot_name.clone());
        let old_generation = existing
            .as_ref()
            .and_then(|project| project.generation_name.clone());
        let created_at = existing
            .as_ref()
            .map(|project| project.summary.created_at.clone())
            .unwrap_or_else(|| timestamp.clone());
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| {
                let _ = fs::remove_dir_all(&generation_path);
                storage_error()
            })?;
        let database_result = if existing.is_some() {
            transaction.execute(
                "UPDATE projects SET name = ?1, dataset_file_name = ?2, row_count = ?3,
                 column_count = ?4, snapshot_name = ?5, updated_at = ?6, last_opened_at = ?6,
                 quality_rules_json = ?7, recipe_draft_json = ?8, generation_name = ?9,
                 history_manifest_json = ?10, profile_json = ?11 WHERE id = ?12",
                params![
                    name,
                    active.file_name,
                    row_count,
                    column_count,
                    snapshot_name,
                    timestamp,
                    quality_rules_json,
                    recipe_draft_json,
                    generation_name,
                    history_manifest_json,
                    profile_json,
                    id
                ],
            )
        } else {
            transaction.execute(
                "INSERT INTO projects
                 (id, name, dataset_file_name, row_count, column_count, snapshot_name,
                  created_at, updated_at, last_opened_at, quality_rules_json, recipe_draft_json,
                  generation_name, history_manifest_json, profile_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![
                    id,
                    name,
                    active.file_name,
                    row_count,
                    column_count,
                    snapshot_name,
                    timestamp,
                    quality_rules_json,
                    recipe_draft_json,
                    generation_name,
                    history_manifest_json,
                    profile_json
                ],
            )
        };
        if !matches!(database_result, Ok(1)) || transaction.commit().is_err() {
            let _ = fs::remove_dir_all(&generation_path);
            return Err(storage_error());
        }
        if let Some(old_generation) = old_generation {
            if let Ok(path) = self.generation_path(&id, &old_generation) {
                let _ = fs::remove_dir_all(path);
            }
        } else if let Some(old_snapshot) = old_snapshot {
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

    fn load_validated(&self, project_id: &str) -> Result<ValidatedProject, String> {
        self.ensure_initialized()?;
        validate_id(project_id)?;
        let connection = self.connection()?;
        let stored = self
            .stored_project(&connection, project_id)?
            .ok_or_else(|| "El proyecto solicitado no existe.".to_owned())?;
        let workspace = decode_workspace(&stored)?;
        let profile = decode_profile(&stored)?;
        let candidate = if let Some(generation_name) = stored.generation_name.as_deref() {
            let generation = self.generation_path(project_id, generation_name)?;
            let current = self.generation_file(&generation, "current.parquet")?;
            let history = self.decode_history(&stored, &generation)?;
            DatasetState::prepare_durable_project_candidate(
                current,
                stored.summary.dataset_file_name.clone(),
                profile.clone(),
                Some(history),
            )?
        } else {
            if stored.history_manifest_json.is_some() || profile.is_some() {
                return Err("El estado persistente del proyecto no es consistente.".to_owned());
            }
            let snapshot_path = self.snapshot_path(project_id, &stored.snapshot_name)?;
            DatasetState::prepare_project_candidate(
                snapshot_path,
                stored.summary.dataset_file_name.clone(),
            )?
        };
        validate_project_workspace(
            candidate.frame(),
            &workspace.quality_rules,
            workspace.recipe_draft.as_ref(),
        )
        .map_err(|_| "La configuración guardada del proyecto no es válida.".to_owned())?;
        if candidate.dimensions() != (stored.summary.row_count, stored.summary.column_count) {
            return Err("El snapshot del proyecto no coincide con su catálogo.".to_owned());
        }
        Ok(ValidatedProject {
            stored,
            workspace,
            profile,
            candidate,
        })
    }

    fn open(
        &self,
        dataset_state: &DatasetState,
        project_id: String,
    ) -> Result<ProjectOpenResult, String> {
        let validated = self.load_validated(&project_id)?;
        let mut connection = self.connection()?;
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
        let dataset = dataset_state.activate_project_candidate(validated.candidate)?;
        Ok(ProjectOpenResult {
            project: validated.stored.summary,
            dataset,
            workspace: validated.workspace,
            profile: validated.profile,
        })
    }

    fn delete(&self, project_id: String) -> Result<(), String> {
        self.ensure_initialized()?;
        validate_id(&project_id)?;
        let mut connection = self.connection()?;
        let stored = self
            .stored_project(&connection, &project_id)?
            .ok_or_else(|| "El proyecto solicitado no existe.".to_owned())?;
        let generation_path = stored
            .generation_name
            .as_deref()
            .map(|name| self.generation_path(&project_id, name))
            .transpose()?;
        let snapshot_path = if generation_path.is_none() {
            Some(self.managed_snapshot_path(&project_id, &stored.snapshot_name)?)
        } else {
            None
        };
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
        if let Some(generation_path) = generation_path {
            return fs::remove_dir_all(generation_path).map_err(|_| {
                "El proyecto se eliminó, pero no se pudo limpiar su generación.".to_owned()
            });
        }
        let snapshot_path = snapshot_path.expect("la ruta legacy se calculó sin generación");
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
                        updated_at, snapshot_name, quality_rules_json, recipe_draft_json,
                        generation_name, history_manifest_json, profile_json
                 FROM projects WHERE id = ?1",
                params![id],
                |row| {
                    Ok(StoredProject {
                        summary: summary_from_row(row)?,
                        snapshot_name: row.get(7)?,
                        quality_rules_json: row.get(8)?,
                        recipe_draft_json: row.get(9)?,
                        generation_name: row.get(10)?,
                        history_manifest_json: row.get(11)?,
                        profile_json: row.get(12)?,
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

    fn generation_path(&self, id: &str, generation_name: &str) -> Result<PathBuf, String> {
        validate_id(id)?;
        let generation = generation_name
            .strip_prefix(&format!("{id}-"))
            .ok_or_else(storage_error)?;
        validate_id(generation)?;
        let candidate = self.snapshots.join(generation_name);
        reject_link_or_reparse(&candidate)?;
        let canonical = fs::canonicalize(&candidate)
            .map_err(|_| "La generación del proyecto no está disponible.".to_owned())?;
        if canonical.parent() != Some(self.snapshots.as_path())
            || !canonical.starts_with(&self.root)
            || !canonical.is_dir()
        {
            return Err(storage_error());
        }
        Ok(canonical)
    }

    fn generation_file(&self, generation: &Path, file_name: &str) -> Result<PathBuf, String> {
        let valid_name = file_name == "current.parquet"
            || file_name
                .strip_prefix("history-")
                .and_then(|value| value.strip_suffix(".parquet"))
                .is_some_and(|value| {
                    value.len() == 3 && value.bytes().all(|byte| byte.is_ascii_digit())
                });
        if !valid_name {
            return Err("El manifiesto del historial contiene un archivo no válido.".to_owned());
        }
        let candidate = generation.join(file_name);
        reject_link_or_reparse(&candidate)?;
        let canonical = fs::canonicalize(&candidate)
            .map_err(|_| "Un archivo de la generación no está disponible.".to_owned())?;
        if canonical.parent() != Some(generation) || !canonical.is_file() {
            return Err(storage_error());
        }
        Ok(canonical)
    }

    fn decode_history(
        &self,
        stored: &StoredProject,
        generation: &Path,
    ) -> Result<ProjectHistoryRestore, String> {
        let encoded = stored
            .history_manifest_json
            .as_deref()
            .ok_or_else(|| "El proyecto no incluye un manifiesto de historial.".to_owned())?;
        let manifest: DurableHistoryManifest = serde_json::from_str(encoded)
            .map_err(|_| "El manifiesto del historial no es válido.".to_owned())?;
        if manifest.version != 1 {
            return Err("La versión del manifiesto del historial no es compatible.".to_owned());
        }
        let entries = manifest
            .entries
            .into_iter()
            .enumerate()
            .map(|(index, entry)| {
                if entry.file_name != format!("history-{index:03}.parquet") {
                    return Err("El manifiesto del historial no tiene un orden válido.".to_owned());
                }
                Ok(ProjectHistoryRestoreEntry {
                    label: entry.label,
                    path: self.generation_file(generation, &entry.file_name)?,
                    bytes: entry.bytes,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ProjectHistoryRestore {
            entries,
            cursor: manifest.cursor,
            snapshots_enabled: manifest.snapshots_enabled,
            degraded_reason: manifest.degraded_reason,
            current_label: manifest.current_label,
            max_entries: manifest.max_entries,
            disk_budget_bytes: manifest.disk_budget_bytes,
        })
    }
}

fn automation_inspection(validated: &ValidatedProject) -> AutomationProjectInspection {
    let history = validated.candidate.history_summary();
    AutomationProjectInspection {
        project: validated.stored.summary.clone(),
        profile_cached: validated.profile.is_some(),
        quality_rule_count: validated.workspace.quality_rules.len(),
        recipe_draft_present: validated.workspace.recipe_draft.is_some(),
        history: AutomationHistorySummary {
            entry_count: history.entry_count,
            current_index: history.current_index,
            can_undo: history.can_undo,
            can_redo: history.can_redo,
            snapshots_enabled: history.snapshots_enabled,
            degraded: history.degraded,
        },
    }
}

pub(crate) fn automation_list_projects(root: &Path) -> Result<Vec<ProjectSummary>, String> {
    ProjectStore::initialize(root.to_path_buf())?.list()
}

pub(crate) fn automation_import_project(
    root: &Path,
    dataset: &DatasetState,
    project_id: Option<String>,
    name: String,
    workspace: ProjectWorkspace,
) -> Result<ProjectSummary, String> {
    ProjectStore::initialize(root.to_path_buf())?.save(dataset, project_id, name, workspace)
}

pub(crate) fn automation_import_dataprep_session_project(
    root: &Path,
    session_path: &Path,
    name: Option<String>,
) -> Result<ProjectSummary, String> {
    let store = ProjectStore::initialize(root.to_path_buf())?;
    import_dataprep_session_project_from_path(&store, session_path, name, None, None)
}

pub(crate) fn automation_inspect_project(
    root: &Path,
    project_id: &str,
) -> Result<AutomationProjectInspection, String> {
    let store = ProjectStore::initialize(root.to_path_buf())?;
    let validated = store.load_validated(project_id)?;
    Ok(automation_inspection(&validated))
}

pub(crate) fn automation_open_project(
    root: &Path,
    project_id: &str,
) -> Result<AutomationOpenedProject, String> {
    let store = ProjectStore::initialize(root.to_path_buf())?;
    let validated = store.load_validated(project_id)?;
    Ok(AutomationOpenedProject {
        frame: validated.candidate.into_frame(),
        workspace: validated.workspace,
    })
}

pub(crate) fn automation_delete_project(root: &Path, project_id: &str) -> Result<(), String> {
    ProjectStore::initialize(root.to_path_buf())?.delete(project_id.to_owned())
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

fn decode_workspace(stored: &StoredProject) -> Result<ProjectWorkspace, String> {
    let quality_rules = serde_json::from_str(&stored.quality_rules_json)
        .map_err(|_| "La configuración guardada del proyecto no es válida.".to_owned())?;
    let recipe_draft = stored
        .recipe_draft_json
        .as_deref()
        .map(serde_json::from_str)
        .transpose()
        .map_err(|_| "La configuración guardada del proyecto no es válida.".to_owned())?;
    Ok(ProjectWorkspace {
        quality_rules,
        recipe_draft,
    })
}

fn decode_profile(stored: &StoredProject) -> Result<Option<DatasetProfile>, String> {
    stored
        .profile_json
        .as_deref()
        .map(serde_json::from_str)
        .transpose()
        .map_err(|_| "El perfil guardado del proyecto no es válido.".to_owned())
}

fn prepare_store_directory(requested: &Path) -> Result<PathBuf, String> {
    if requested.as_os_str().is_empty() {
        return Err("La raíz del catálogo no es válida.".to_owned());
    }
    let absolute = if requested.is_absolute() {
        requested.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|_| storage_error())?
            .join(requested)
    };
    if absolute
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err("La raíz del catálogo no admite segmentos relativos.".to_owned());
    }

    let mut current = PathBuf::new();
    for component in absolute.components() {
        if matches!(component, std::path::Component::CurDir) {
            continue;
        }
        current.push(component.as_os_str());
        if matches!(
            component,
            std::path::Component::Prefix(_) | std::path::Component::RootDir
        ) {
            continue;
        }
        match fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if !metadata.is_dir()
                    || metadata.file_type().is_symlink()
                    || is_reparse_point(&metadata)
                {
                    return Err("La raíz del catálogo no es un directorio seguro.".to_owned());
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                match fs::create_dir(&current) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(_) => return Err(storage_error()),
                }
                let metadata = fs::symlink_metadata(&current).map_err(|_| storage_error())?;
                if !metadata.is_dir()
                    || metadata.file_type().is_symlink()
                    || is_reparse_point(&metadata)
                {
                    return Err("La raíz del catálogo no es un directorio seguro.".to_owned());
                }
            }
            Err(_) => return Err(storage_error()),
        }
    }
    let canonical = fs::canonicalize(&current).map_err(|_| storage_error())?;
    if canonical.parent().is_none() {
        return Err("La raíz del catálogo debe ser un directorio dedicado.".to_owned());
    }
    Ok(canonical)
}

fn history_manifest(history: &ProjectHistoryCapture) -> DurableHistoryManifest {
    DurableHistoryManifest {
        version: 1,
        entries: history
            .entries
            .iter()
            .enumerate()
            .map(|(index, entry)| DurableHistoryEntry {
                label: entry.label.clone(),
                file_name: format!("history-{index:03}.parquet"),
                bytes: entry.bytes,
            })
            .collect(),
        cursor: history.cursor,
        snapshots_enabled: history.snapshots_enabled,
        degraded_reason: history.degraded_reason.clone(),
        current_label: history.current_label.clone(),
        max_entries: history.max_entries,
        disk_budget_bytes: history.disk_budget_bytes,
    }
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

fn session_reference_value<'a>(
    root: &'a JsonMap<String, JsonValue>,
    keys: &[&str],
) -> Option<&'a JsonValue> {
    let session = root.get("session").and_then(JsonValue::as_object);
    keys.iter().find_map(|key| {
        root.get(*key).filter(|value| !value.is_null()).or_else(|| {
            session
                .and_then(|map| map.get(*key))
                .filter(|value| !value.is_null())
        })
    })
}

fn resolve_dataprep_reference(
    session_path: &Path,
    keys: &[&str],
    missing_message: &str,
) -> Result<PathBuf, String> {
    let bytes = fs::read(session_path).map_err(|_| {
        "No se pudo leer la sesión DataPrep para localizar su referencia.".to_owned()
    })?;
    let root = serde_json::from_slice::<JsonValue>(&bytes)
        .ok()
        .and_then(|value| value.as_object().cloned())
        .ok_or_else(|| "La sesión DataPrep no contiene una fuente utilizable.".to_owned())?;
    let reference = session_reference_value(&root, keys)
        .and_then(JsonValue::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty() && !value.contains("://"))
        .ok_or_else(|| missing_message.to_owned())?;
    let candidate = Path::new(reference);
    let candidate = if candidate.is_absolute() {
        candidate.to_owned()
    } else {
        session_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(candidate)
    };
    dataset::canonicalize_file_for_automation(&candidate).map_err(|_| missing_message.to_owned())
}

fn resolve_dataprep_source(session_path: &Path) -> Result<PathBuf, String> {
    resolve_dataprep_reference(
        session_path,
        &["source_path", "sourcePath", "file_name", "fileName"],
        "La fuente de la sesión DataPrep no está disponible.",
    )
}

fn resolve_dataprep_snapshot(session_path: &Path) -> Result<PathBuf, String> {
    resolve_dataprep_reference(
        session_path,
        &["snapshot_path", "snapshotPath"],
        "El snapshot de la sesión DataPrep no está disponible.",
    )
}

fn import_dataprep_session_project_from_path(
    store: &ProjectStore,
    session_path: &Path,
    name: Option<String>,
    sheet_name: Option<String>,
    header_mode: Option<SpreadsheetHeaderMode>,
) -> Result<ProjectSummary, String> {
    let session_path = dataset::canonicalize_file_for_automation(session_path)
        .map_err(|_| "La sesión DataPrep seleccionada no está disponible.".to_owned())?;
    let plan = dataset::load_dataprep_session_migration_plan(&session_path)?;
    if !plan.can_create_project {
        let mut reasons = Vec::new();
        if !plan.missing_references.is_empty() {
            reasons.push("faltan archivos vinculados".to_owned());
        }
        if !plan.collisions.is_empty() {
            reasons.push("hay operaciones con nombres en conflicto".to_owned());
        }
        return Err(format!(
            "La sesión no se puede importar: {}.",
            reasons.join(" y ")
        ));
    }
    // A materialized DataPrep snapshot represents the exact current session
    // state, including cleaning operations that are only recorded as
    // ``applied_ops`` metadata. Prefer it whenever it is available; replaying
    // the recipe over the source cannot reproduce those operations without
    // their original options. If no snapshot exists, the source remains a
    // safe, reproducible fallback and the structural recipe is applied there.
    // Resolved paths stay private to this native operation and never cross the
    // bridge.
    let (input_path, apply_recipe) = if matches!(
        plan.snapshot_status,
        dataset::SessionReferenceStatus::Available
    ) {
        (resolve_dataprep_snapshot(&session_path)?, false)
    } else if matches!(
        plan.source_status,
        dataset::SessionReferenceStatus::Available
    ) {
        (resolve_dataprep_source(&session_path)?, true)
    } else {
        return Err("La sesión no contiene una fuente o snapshot disponible.".to_owned());
    };
    let extension = input_path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    let is_spreadsheet = matches!(extension.as_str(), "xlsx" | "xls" | "xlsb" | "ods");
    let explicit_sheet = sheet_name.is_some();
    let requested_sheet = sheet_name.or_else(|| plan.sheet_name.clone());
    let (requested_sheet, requested_header) = if is_spreadsheet {
        let sheet = requested_sheet
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| "La sesión no indica una hoja válida para el libro.".to_owned())?;
        if let (Some(expected), Some(actual)) = (plan.sheet_name.as_deref(), Some(sheet.as_str())) {
            if expected != actual {
                return Err(
                    "La hoja elegida no coincide con la hoja registrada en la sesión.".to_owned(),
                );
            }
        }
        (
            Some(sheet),
            Some(header_mode.unwrap_or(SpreadsheetHeaderMode::FirstRow)),
        )
    } else {
        if explicit_sheet || header_mode.is_some() {
            return Err(
                "La sesión no puede asignar una hoja a este formato de archivo.".to_owned(),
            );
        }
        (None, None)
    };
    let (frame, preview) = dataset::load_dataset_for_automation(
        &input_path,
        requested_sheet.as_deref(),
        requested_header,
    )
    .map_err(|_| {
        "El artefacto de la sesión no se puede leer con el esquema registrado.".to_owned()
    })?;
    let imported = DatasetState::for_project_import(frame, preview.file_name.clone())?;
    if apply_recipe {
        imported
            .apply_project_import_recipe(&plan.recipe.recipe)
            .map_err(|_| {
                "La receta de la sesión no coincide con el esquema de la fuente.".to_owned()
            })?;
    }
    store.save(
        &imported,
        None,
        name.unwrap_or(plan.name),
        ProjectWorkspace {
            quality_rules: plan.quality_rules,
            recipe_draft: Some(plan.recipe),
        },
    )
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

fn write_generation(
    frame: &DataFrame,
    history: &ProjectHistoryCapture,
    destination: &Path,
) -> Result<(), String> {
    if destination.exists() {
        return Err(storage_error());
    }
    let parent = destination.parent().ok_or_else(storage_error)?;
    let staging = tempfile::tempdir_in(parent).map_err(|_| storage_error())?;
    write_snapshot(frame, &staging.path().join("current.parquet"))?;
    for (index, entry) in history.entries.iter().enumerate() {
        let target = staging.path().join(format!("history-{index:03}.parquet"));
        let copied = fs::copy(&entry.path, &target).map_err(|_| storage_error())?;
        if copied != entry.bytes {
            return Err(storage_error());
        }
        fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&target)
            .and_then(|file| file.sync_all())
            .map_err(|_| storage_error())?;
    }
    sync_directory(staging.path())?;
    let staging_path = staging.keep();
    if fs::rename(&staging_path, destination).is_err() {
        let _ = fs::remove_dir_all(staging_path);
        return Err(storage_error());
    }
    if sync_directory(parent).is_err() {
        let _ = fs::remove_dir_all(destination);
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
        project_state.store.ensure_initialized()?;
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
    workspace: ProjectWorkspace,
) -> Result<ProjectSummary, String> {
    run_project_operation(app, move |store, dataset| {
        store.save(dataset, project_id, name, workspace)
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

/// Maps a DataPrep session into a new catalog project without touching the active dataset.
/// All references and schema-dependent operations are verified before the catalog is written.
#[tauri::command]
pub async fn import_dataprep_session_project(
    app: AppHandle,
    name: Option<String>,
    sheet_name: Option<String>,
    header_mode: Option<SpreadsheetHeaderMode>,
) -> Result<ProjectSummary, String> {
    let selection = app
        .dialog()
        .file()
        .add_filter("Sesión DataPrep", &["json"])
        .blocking_pick_file();
    let Some(selection) = selection else {
        return Err("No se seleccionó una sesión DataPrep.".to_owned());
    };
    let session_path = selection
        .into_path()
        .map_err(|_| "No se pudo resolver la sesión DataPrep seleccionada.".to_owned())?;
    run_project_operation(app, move |store, _| {
        import_dataprep_session_project_from_path(
            store,
            &session_path,
            name,
            sheet_name,
            header_mode,
        )
    })
    .await
}

#[cfg(debug_assertions)]
#[tauri::command]
pub async fn probe_reopen_project(
    app: AppHandle,
    project_id: String,
) -> Result<NativeProjectReopen, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("No se pudo resolver el almacén de proyectos: {error}"))?;
    tauri::async_runtime::spawn_blocking(move || {
        let store = ProjectStore::initialize(app_data_dir)?;
        let recovery_candidate_present = store.recovery_candidate()?.is_some();
        let validated = store.load_validated(&project_id)?;
        let (row_count, column_count) = validated.candidate.dimensions();
        let project = validated.stored.summary.clone();
        Ok(NativeProjectReopen {
            dataset_file_name: project.dataset_file_name.clone(),
            row_count,
            column_count,
            quality_rule_count: validated.workspace.quality_rules.len(),
            recipe_draft_present: validated.workspace.recipe_draft.is_some(),
            recovery_candidate_present,
            project,
        })
    })
    .await
    .map_err(|error| format!("La reapertura nativa del proyecto se interrumpió: {error}"))?
}

// Los proyectos v3 persisten el historial, pero cada apertura lo copia a un TempDir nuevo:
// undo/redo posteriores nunca modifican la generación durable hasta el próximo guardado.

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

    fn workspace() -> ProjectWorkspace {
        serde_json::from_value(serde_json::json!({
            "qualityRules": [{
                "column": "value",
                "kind": "not_null",
                "maxInvalid": 0
            }],
            "recipeDraft": {
                "version": 1,
                "name": "Borrador seguro",
                "savedAt": "2026-08-21T12:00:00Z",
                "recipe": {}
            }
        }))
        .expect("el workspace de prueba debe ser válido")
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
    fn deferred_store_prepares_paths_and_migrates_on_first_operation() {
        let directory = tempfile::tempdir().unwrap();
        let store = ProjectStore::initialize_deferred(directory.path().join("data")).unwrap();

        assert!(!store.catalog.exists());
        assert!(store.list().unwrap().is_empty());
        assert!(store.catalog.exists());
    }

    #[test]
    fn deferred_store_retries_after_a_transient_initialization_failure() {
        let directory = tempfile::tempdir().unwrap();
        let store =
            ProjectStore::initialize_deferred(directory.path().join("retryable-data")).unwrap();
        fs::write(&store.catalog, b"not a sqlite catalog").unwrap();

        assert!(store.list().is_err());
        fs::remove_file(&store.catalog).unwrap();

        assert!(store.list().unwrap().is_empty());
        assert!(store.catalog.exists());
    }

    #[test]
    fn migration_from_v1_adds_empty_workspace_without_changing_existing_metadata() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("data");
        fs::create_dir_all(&root).unwrap();
        let catalog = root.join("projects.sqlite3");
        let connection = Connection::open(&catalog).unwrap();
        connection
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
                 INSERT INTO projects VALUES (
                   '0123456789abcdef0123456789abcdef', 'Legado', 'input.csv', 2, 1,
                   '0123456789abcdef0123456789abcdef-abcdef0123456789abcdef0123456789.parquet',
                   '2026-08-20T00:00:00.000Z', '2026-08-20T00:00:00.000Z', NULL
                 );
                 PRAGMA user_version = 1;",
            )
            .unwrap();
        drop(connection);

        let store = ProjectStore::initialize(root.clone()).unwrap();
        let migrated = store
            .stored_project(
                &store.connection().unwrap(),
                "0123456789abcdef0123456789abcdef",
            )
            .unwrap()
            .unwrap();
        assert_eq!(migrated.summary.name, "Legado");
        assert_eq!(
            decode_workspace(&migrated).unwrap(),
            ProjectWorkspace::default()
        );
        let version: i64 = store
            .connection()
            .unwrap()
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION);
        drop(store);
        ProjectStore::initialize(root).expect("reabrir v3 debe ser idempotente");
    }

    #[test]
    fn migration_from_v2_adds_durable_state_columns_transactionally() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("data-v2");
        fs::create_dir_all(&root).unwrap();
        Connection::open(root.join("projects.sqlite3"))
            .unwrap()
            .execute_batch(
                "CREATE TABLE projects (
                   id TEXT PRIMARY KEY NOT NULL, name TEXT NOT NULL,
                   dataset_file_name TEXT NOT NULL, row_count INTEGER NOT NULL,
                   column_count INTEGER NOT NULL, snapshot_name TEXT NOT NULL UNIQUE,
                   created_at TEXT NOT NULL, updated_at TEXT NOT NULL, last_opened_at TEXT,
                   quality_rules_json TEXT NOT NULL DEFAULT '[]', recipe_draft_json TEXT
                 );
                 CREATE INDEX projects_updated_at ON projects(updated_at DESC, id ASC);
                 PRAGMA user_version = 2;",
            )
            .unwrap();

        let store = ProjectStore::initialize(root).unwrap();
        let connection = store.connection().unwrap();
        let version: i64 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION);
        let columns = connection
            .prepare("PRAGMA table_info(projects)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert!(columns.contains(&"generation_name".to_owned()));
        assert!(columns.contains(&"history_manifest_json".to_owned()));
        assert!(columns.contains(&"profile_json".to_owned()));
    }

    #[test]
    fn migration_fails_closed_for_future_catalog_versions_without_leaking_paths() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("future-private-data");
        fs::create_dir_all(&root).unwrap();
        Connection::open(root.join("projects.sqlite3"))
            .unwrap()
            .execute_batch("PRAGMA user_version = 4;")
            .unwrap();

        let error = ProjectStore::initialize(root.clone())
            .err()
            .expect("una versión futura debe rechazarse");
        assert!(error.contains("versión más reciente"));
        assert!(!error.contains(root.to_string_lossy().as_ref()));
        let version: i64 = Connection::open(root.join("projects.sqlite3"))
            .unwrap()
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, 4);
    }

    #[test]
    fn workspace_roundtrips_with_snapshot_across_restart() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("data");
        let store = ProjectStore::initialize(root.clone()).unwrap();
        let (state, source) = active_state(directory.path(), &[1, 2], "input.csv");
        let expected = workspace();
        let project = store
            .save(
                &state,
                None,
                "Con configuración".to_owned(),
                expected.clone(),
            )
            .unwrap();
        fs::remove_file(source).unwrap();
        drop(store);

        let reopened = ProjectStore::initialize(root).unwrap();
        let result = reopened.open(&DatasetState::default(), project.id).unwrap();
        assert_eq!(result.workspace, expected);
        assert_eq!(result.dataset.file_name, "input.csv");
    }

    #[test]
    fn profile_and_history_cursor_roundtrip_across_restart_with_working_undo_redo() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("data");
        let store = ProjectStore::initialize(root.clone()).unwrap();
        let (state, _) = active_state(directory.path(), &[1, 2], "history.csv");
        state
            .project_test_record(frame(&[1, 2, 3]), "Agregar fila")
            .unwrap();
        state
            .project_test_record(frame(&[9]), "Reemplazar contenido")
            .unwrap();
        assert!(state
            .project_test_undo()
            .unwrap()
            .equals_missing(&frame(&[1, 2, 3])));
        let expected_profile = state.project_test_cache_profile().unwrap();
        let project = store
            .save(
                &state,
                None,
                "Con historia".to_owned(),
                ProjectWorkspace::default(),
            )
            .unwrap();
        drop(store);

        let reopened = ProjectStore::initialize(root).unwrap();
        let restored = DatasetState::default();
        let result = reopened.open(&restored, project.id).unwrap();
        assert_eq!(result.profile, Some(expected_profile));
        assert!(restored
            .project_test_undo()
            .unwrap()
            .equals_missing(&frame(&[1, 2])));
        assert!(restored
            .project_test_redo()
            .unwrap()
            .equals_missing(&frame(&[1, 2, 3])));
        assert!(restored
            .project_test_redo()
            .unwrap()
            .equals_missing(&frame(&[9])));
    }

    #[test]
    fn degraded_history_restores_honestly_around_the_current_frame() {
        let directory = tempfile::tempdir().unwrap();
        let store = ProjectStore::initialize(directory.path().join("data")).unwrap();
        let (state, _) = active_state(directory.path(), &[5, 6], "degraded.csv");
        state.project_test_degrade_history().unwrap();
        let project = store
            .save(
                &state,
                None,
                "Degradado".to_owned(),
                ProjectWorkspace::default(),
            )
            .unwrap();
        let restored = DatasetState::default();
        store.open(&restored, project.id).unwrap();

        let active = restored.active_project_snapshot().unwrap();
        assert!(!active.history.snapshots_enabled);
        assert!(active.history.entries.is_empty());
        assert!(active.history.degraded_reason.is_some());
        assert!(active.frame.equals_missing(&frame(&[5, 6])));
        assert!(restored.project_test_undo().is_err());
    }

    #[test]
    fn create_update_list_recovery_and_delete_preserve_active_dataset() {
        let directory = tempfile::tempdir().unwrap();
        let store = ProjectStore::initialize(directory.path().join("data")).unwrap();
        let (state, _) = active_state(directory.path(), &[1, 2, 3], "entrada original.csv");

        let created = store
            .save(
                &state,
                None,
                "  Proyecto ágil  ".to_owned(),
                ProjectWorkspace::default(),
            )
            .unwrap();
        assert_eq!(created.name, "Proyecto ágil");
        assert_eq!(created.dataset_file_name, "entrada original.csv");
        assert_eq!((created.row_count, created.column_count), (3, 2));
        assert_eq!(created.id.len(), ID_LENGTH);
        assert!(created.created_at.ends_with('Z'));
        assert_eq!(store.list().unwrap(), vec![created.clone()]);
        assert_eq!(store.recovery_candidate().unwrap(), Some(created.clone()));
        assert_eq!(snapshot_count(&store), 1);
        let first_stored = store
            .stored_project(&store.connection().unwrap(), &created.id)
            .unwrap()
            .unwrap();
        let first_generation = store
            .generation_path(
                &created.id,
                first_stored.generation_name.as_deref().unwrap(),
            )
            .unwrap();
        assert!(fs::read_dir(&first_generation).unwrap().count() >= 2);

        let updated = store
            .save(
                &state,
                Some(created.id.clone()),
                "Renombrado".to_owned(),
                ProjectWorkspace::default(),
            )
            .unwrap();
        assert_eq!(updated.id, created.id);
        assert_eq!(updated.created_at, created.created_at);
        assert_eq!(updated.name, "Renombrado");
        assert_eq!(snapshot_count(&store), 1, "el snapshot anterior se limpia");
        assert!(!first_generation.exists());
        let second_stored = store
            .stored_project(&store.connection().unwrap(), &created.id)
            .unwrap()
            .unwrap();
        let second_generation = store
            .generation_path(
                &created.id,
                second_stored.generation_name.as_deref().unwrap(),
            )
            .unwrap();

        store.delete(created.id).unwrap();
        assert!(store.list().unwrap().is_empty());
        assert_eq!(snapshot_count(&store), 0);
        assert!(!second_generation.exists());
        assert_eq!(state.active_project_snapshot().unwrap().row_count, 3);
    }

    #[test]
    fn snapshot_survives_source_deletion_and_real_catalog_restart() {
        let directory = tempfile::tempdir().unwrap();
        let data_root = directory.path().join("data");
        let store = ProjectStore::initialize(data_root.clone()).unwrap();
        let (state, source) = active_state(directory.path(), &[7, 8], "ventas.csv");
        let project = store
            .save(
                &state,
                None,
                "Ventas".to_owned(),
                ProjectWorkspace::default(),
            )
            .unwrap();
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
            .save_inner(
                &state,
                None,
                "Proyecto".to_owned(),
                ProjectWorkspace::default(),
                true,
            )
            .unwrap_err();
        assert_eq!(error, storage_error());
        assert!(store.list().unwrap().is_empty());
        assert_eq!(snapshot_count(&store), 0);

        let created = store
            .save(&state, None, "Original".to_owned(), workspace())
            .unwrap();
        let before = store
            .stored_project(&store.connection().unwrap(), &created.id)
            .unwrap()
            .unwrap();
        let before_generation = before.generation_name.clone();
        let error = store
            .save_inner(
                &state,
                Some(created.id.clone()),
                "No publicado".to_owned(),
                ProjectWorkspace::default(),
                true,
            )
            .unwrap_err();
        assert_eq!(error, storage_error());
        assert_eq!(store.list().unwrap(), vec![created.clone()]);
        assert_eq!(snapshot_count(&store), 1);
        let stored = store
            .stored_project(&store.connection().unwrap(), &created.id)
            .unwrap()
            .unwrap();
        assert_eq!(decode_workspace(&stored).unwrap(), workspace());
        assert_eq!(stored.generation_name, before_generation);
    }

    #[test]
    fn invalid_workspace_is_rejected_before_writing_a_snapshot_or_metadata() {
        let directory = tempfile::tempdir().unwrap();
        let store = ProjectStore::initialize(directory.path().join("data")).unwrap();
        let (state, _) = active_state(directory.path(), &[1], "input.csv");
        let created = store
            .save(
                &state,
                None,
                "Original".to_owned(),
                ProjectWorkspace::default(),
            )
            .unwrap();
        let invalid_recipe: ProjectWorkspace = serde_json::from_value(serde_json::json!({
            "qualityRules": [],
            "recipeDraft": {
                "version": 2,
                "name": "Futura",
                "savedAt": "2026-08-21T12:00:00Z",
                "recipe": {}
            }
        }))
        .unwrap();
        assert!(store
            .save(
                &state,
                Some(created.id.clone()),
                "No publicado".to_owned(),
                invalid_recipe,
            )
            .is_err());
        let unknown_column: ProjectWorkspace = serde_json::from_value(serde_json::json!({
            "qualityRules": [{
                "column": "missing",
                "kind": "not_null",
                "maxInvalid": 0
            }],
            "recipeDraft": null
        }))
        .unwrap();
        assert!(store
            .save(
                &state,
                Some(created.id.clone()),
                "No publicado".to_owned(),
                unknown_column,
            )
            .is_err());
        assert_eq!(store.list().unwrap(), vec![created]);
        assert_eq!(snapshot_count(&store), 1);
    }

    #[test]
    fn corrupt_workspace_fails_closed_before_replacing_the_active_dataset() {
        let directory = tempfile::tempdir().unwrap();
        let store = ProjectStore::initialize(directory.path().join("data")).unwrap();
        let (source_state, _) = active_state(directory.path(), &[1, 2], "project.csv");
        let project = store
            .save(&source_state, None, "Proyecto".to_owned(), workspace())
            .unwrap();
        let (target_state, _) = active_state(directory.path(), &[42], "active.csv");
        let connection = store.connection().unwrap();
        connection
            .execute(
                "UPDATE projects SET quality_rules_json = ?1 WHERE id = ?2",
                params!["{not-json", project.id],
            )
            .unwrap();
        assert!(store.open(&target_state, project.id.clone()).is_err());
        assert_eq!(
            target_state.active_project_snapshot().unwrap().file_name,
            "active.csv"
        );

        connection
            .execute(
                "UPDATE projects SET quality_rules_json = '[]', recipe_draft_json = ?1
                 WHERE id = ?2",
                params![
                    r#"{"version":2,"name":"Futura","savedAt":"2026-08-21T12:00:00Z","recipe":{}}"#,
                    project.id
                ],
            )
            .unwrap();
        assert!(store.open(&target_state, project.id).is_err());
        assert_eq!(
            target_state.active_project_snapshot().unwrap().file_name,
            "active.csv"
        );
    }

    #[test]
    fn corrupt_non_current_history_snapshot_fails_closed() {
        let directory = tempfile::tempdir().unwrap();
        let store = ProjectStore::initialize(directory.path().join("data")).unwrap();
        let (state, _) = active_state(directory.path(), &[1, 2], "project.csv");
        state
            .project_test_record(frame(&[1, 2, 3]), "Paso 1")
            .unwrap();
        state.project_test_record(frame(&[9]), "Paso 2").unwrap();
        state.project_test_undo().unwrap();
        let project = store
            .save(
                &state,
                None,
                "Proyecto".to_owned(),
                ProjectWorkspace::default(),
            )
            .unwrap();
        let stored = store
            .stored_project(&store.connection().unwrap(), &project.id)
            .unwrap()
            .unwrap();
        let generation = store
            .generation_path(&project.id, stored.generation_name.as_deref().unwrap())
            .unwrap();
        fs::write(generation.join("history-000.parquet"), b"corrupt").unwrap();
        let (target, _) = active_state(directory.path(), &[42], "active.csv");

        assert!(store.open(&target, project.id).is_err());
        assert_eq!(
            target.active_project_snapshot().unwrap().file_name,
            "active.csv"
        );
    }

    #[test]
    fn corrupt_history_manifest_and_limits_fail_closed() {
        let directory = tempfile::tempdir().unwrap();
        let store = ProjectStore::initialize(directory.path().join("data")).unwrap();
        let (state, _) = active_state(directory.path(), &[1], "project.csv");
        let project = store
            .save(
                &state,
                None,
                "Proyecto".to_owned(),
                ProjectWorkspace::default(),
            )
            .unwrap();
        let connection = store.connection().unwrap();
        let encoded: String = connection
            .query_row(
                "SELECT history_manifest_json FROM projects WHERE id = ?1",
                params![project.id],
                |row| row.get(0),
            )
            .unwrap();
        let mut manifest: serde_json::Value = serde_json::from_str(&encoded).unwrap();
        manifest["maxEntries"] = serde_json::json!(13);
        connection
            .execute(
                "UPDATE projects SET history_manifest_json = ?1 WHERE id = ?2",
                params![serde_json::to_string(&manifest).unwrap(), project.id],
            )
            .unwrap();
        let (target, _) = active_state(directory.path(), &[42], "active.csv");
        assert!(store.open(&target, project.id.clone()).is_err());
        assert_eq!(
            target.active_project_snapshot().unwrap().file_name,
            "active.csv"
        );

        connection
            .execute(
                "UPDATE projects SET history_manifest_json = '{}' WHERE id = ?1",
                params![project.id],
            )
            .unwrap();
        assert!(store.open(&target, project.id).is_err());
    }

    #[test]
    fn tampered_profile_fails_closed_before_dataset_activation() {
        let directory = tempfile::tempdir().unwrap();
        let store = ProjectStore::initialize(directory.path().join("data")).unwrap();
        let (state, _) = active_state(directory.path(), &[1, 2], "project.csv");
        state.project_test_cache_profile().unwrap();
        let project = store
            .save(
                &state,
                None,
                "Proyecto".to_owned(),
                ProjectWorkspace::default(),
            )
            .unwrap();
        let connection = store.connection().unwrap();
        let encoded: String = connection
            .query_row(
                "SELECT profile_json FROM projects WHERE id = ?1",
                params![project.id],
                |row| row.get(0),
            )
            .unwrap();
        let mut profile: serde_json::Value = serde_json::from_str(&encoded).unwrap();
        profile["duplicatePercentage"] = serde_json::json!(200.0);
        connection
            .execute(
                "UPDATE projects SET profile_json = ?1 WHERE id = ?2",
                params![serde_json::to_string(&profile).unwrap(), project.id],
            )
            .unwrap();
        let (target, _) = active_state(directory.path(), &[42], "active.csv");

        assert!(store.open(&target, project.id).is_err());
        assert_eq!(
            target.active_project_snapshot().unwrap().file_name,
            "active.csv"
        );
    }

    #[test]
    fn invalid_names_and_ids_are_rejected_without_creating_files() {
        let directory = tempfile::tempdir().unwrap();
        let store = ProjectStore::initialize(directory.path().join("data")).unwrap();
        let (state, _) = active_state(directory.path(), &[1], "input.csv");

        for name in ["   ".to_owned(), "x".repeat(129)] {
            assert!(store
                .save(&state, None, name, ProjectWorkspace::default())
                .is_err());
        }
        for id in ["../catalog", "ABCDEF0123456789ABCDEF0123456789", "abc"] {
            assert!(store.open(&DatasetState::default(), id.to_owned()).is_err());
            assert!(store.delete(id.to_owned()).is_err());
            assert!(store
                .save(
                    &state,
                    Some(id.to_owned()),
                    "Proyecto".to_owned(),
                    ProjectWorkspace::default(),
                )
                .is_err());
        }
        let missing = "0".repeat(ID_LENGTH);
        assert!(store
            .save(
                &state,
                Some(missing.clone()),
                "Proyecto".to_owned(),
                ProjectWorkspace::default(),
            )
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
        let project = store
            .save(
                &state,
                None,
                "Seguro".to_owned(),
                ProjectWorkspace::default(),
            )
            .unwrap();
        let json = serde_json::to_string(&project).unwrap();
        assert!(!json.contains(directory.path().to_string_lossy().as_ref()));
        assert!(!json.contains("snapshot"));

        store
            .connection()
            .unwrap()
            .execute(
                "UPDATE projects SET generation_name = ?1 WHERE id = ?2",
                params!["../escape", project.id],
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
            .save(
                &source_state,
                None,
                "Proyecto".to_owned(),
                ProjectWorkspace::default(),
            )
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

    #[test]
    fn automation_store_supports_crud_restart_and_cleans_generations() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("nested").join("projects");
        assert!(automation_list_projects(&root).unwrap().is_empty());
        let dataset = DatasetState::for_project_import(frame(&[1, 2]), "input.csv".to_owned())
            .expect("el import debe crear un estado aislado");

        let saved = automation_import_project(
            &root,
            &dataset,
            None,
            " Proyecto CLI ".to_owned(),
            ProjectWorkspace::default(),
        )
        .unwrap();
        assert_eq!(saved.name, "Proyecto CLI");
        assert_eq!(
            automation_list_projects(&root).unwrap(),
            vec![saved.clone()]
        );

        let reopened = ProjectStore::initialize(root.clone()).unwrap();
        let stored = reopened
            .stored_project(&reopened.connection().unwrap(), &saved.id)
            .unwrap()
            .unwrap();
        let generation = reopened
            .generation_path(&saved.id, stored.generation_name.as_deref().unwrap())
            .unwrap();
        assert!(generation.is_dir());
        drop(reopened);

        let opened = automation_open_project(&root, &saved.id).unwrap();
        assert_eq!(opened.frame.height(), 2);
        assert_eq!(opened.workspace, ProjectWorkspace::default());

        automation_delete_project(&root, &saved.id).unwrap();
        assert!(!generation.exists());
        assert!(automation_list_projects(&root).unwrap().is_empty());
    }

    fn write_session(directory: &Path, source: &str, transform: JsonValue) -> PathBuf {
        let source_path = directory.join(source);
        fs::write(&source_path, "value,label\n1,uno\n2,dos\n").unwrap();
        let session_path = directory.join("session.json");
        fs::write(
            &session_path,
            serde_json::to_vec(&serde_json::json!({
                "version": 1,
                "name": "Sesión importada",
                "saved_at": "2026-08-24T00:00:00Z",
                "source_path": source,
                "transform": transform
            }))
            .unwrap(),
        )
        .unwrap();
        session_path
    }

    #[test]
    fn dataprep_session_mapping_validates_schema_before_creating_project() {
        let directory = tempfile::tempdir().unwrap();
        let store = ProjectStore::initialize(directory.path().join("data")).unwrap();
        let session = write_session(
            directory.path(),
            "source.csv",
            serde_json::json!({"rename_text": "missing -> renamed"}),
        );
        let error = import_dataprep_session_project_from_path(&store, &session, None, None, None)
            .unwrap_err();

        assert!(error.contains("no coincide con el esquema"));
        assert!(!error.contains(directory.path().to_string_lossy().as_ref()));
        assert!(store.list().unwrap().is_empty());
        assert_eq!(snapshot_count(&store), 0);
    }

    #[test]
    fn dataprep_session_mapping_reports_missing_source_without_replacing_catalog() {
        let directory = tempfile::tempdir().unwrap();
        let store = ProjectStore::initialize(directory.path().join("data")).unwrap();
        let (active, _) = active_state(directory.path(), &[7, 8], "existing.csv");
        let existing = store
            .save(
                &active,
                None,
                "Proyecto válido".to_owned(),
                ProjectWorkspace::default(),
            )
            .unwrap();
        let session = write_session(directory.path(), "missing.csv", serde_json::json!({}));
        fs::remove_file(directory.path().join("missing.csv")).unwrap();
        let before_snapshots = snapshot_count(&store);
        let error = import_dataprep_session_project_from_path(
            &store,
            &session,
            Some("No debe publicarse".to_owned()),
            None,
            None,
        )
        .unwrap_err();

        assert!(error.contains("faltan archivos vinculados"));
        assert!(!error.contains(directory.path().to_string_lossy().as_ref()));
        assert_eq!(store.list().unwrap(), vec![existing.clone()]);
        assert_eq!(snapshot_count(&store), before_snapshots);
        assert_eq!(
            store
                .open(&DatasetState::default(), existing.id.clone())
                .unwrap()
                .project,
            existing
        );
    }

    #[test]
    fn dataprep_session_mapping_restores_available_snapshot_when_source_is_missing() {
        let directory = tempfile::tempdir().unwrap();
        let store = ProjectStore::initialize(directory.path().join("data")).unwrap();
        let snapshot = directory.path().join("snapshot.parquet");
        write_snapshot(&frame(&[11, 12]), &snapshot).unwrap();
        let session = directory.path().join("session.json");
        fs::write(
            &session,
            serde_json::to_vec(&serde_json::json!({
                "version": 1,
                "name": "Restauración desde snapshot",
                "source_path": "missing.csv",
                "snapshot_path": "snapshot.parquet",
                // This operation references a column absent from the snapshot. A
                // restored snapshot is already materialized and must not replay it.
                "transform": {"rename_text": "missing -> renamed"}
            }))
            .unwrap(),
        )
        .unwrap();

        let plan = dataset::load_dataprep_session_migration_plan(&session).unwrap();
        assert_eq!(plan.source_status, dataset::SessionReferenceStatus::Missing);
        assert_eq!(
            plan.snapshot_status,
            dataset::SessionReferenceStatus::Available
        );
        assert!(plan.missing_references.iter().any(|item| item == "source"));
        assert!(plan.can_create_project);

        let imported =
            import_dataprep_session_project_from_path(&store, &session, None, None, None)
                .expect("un snapshot disponible debe poder restaurarse");
        assert_eq!(imported.dataset_file_name, "snapshot.parquet");
        assert_eq!((imported.row_count, imported.column_count), (2, 2));

        let opened = store
            .open(&DatasetState::default(), imported.id)
            .expect("el proyecto restaurado debe reabrirse");
        assert_eq!(opened.dataset.columns[0].name, "value");
        assert_eq!(opened.dataset.columns[1].name, "label");
        assert_eq!(snapshot_count(&store), 1);
    }

    #[test]
    fn dataprep_session_mapping_prefers_available_snapshot_over_source_replay() {
        let directory = tempfile::tempdir().unwrap();
        let store = ProjectStore::initialize(directory.path().join("data")).unwrap();
        let source = directory.path().join("source.csv");
        let snapshot = directory.path().join("snapshot.parquet");
        let session = directory.path().join("session.json");
        fs::write(&source, "value,label\n1,one\n2,two\n").unwrap();
        write_snapshot(
            &DataFrame::new(
                2,
                vec![
                    Series::new("amount".into(), vec![10_i64, 20]).into_column(),
                    Series::new("label".into(), vec!["one", "two"]).into_column(),
                ],
            )
            .unwrap(),
            &snapshot,
        )
        .unwrap();
        fs::write(
            &session,
            serde_json::to_vec(&serde_json::json!({
                "version": 1,
                "name": "Snapshot exacto",
                "source_path": "source.csv",
                "snapshot_path": "snapshot.parquet",
                // The source deliberately cannot satisfy this recipe. The
                // available snapshot already contains the materialized state.
                "transform": {"rename_text": "missing -> renamed"}
            }))
            .unwrap(),
        )
        .unwrap();

        let imported =
            import_dataprep_session_project_from_path(&store, &session, None, None, None)
                .expect("el snapshot disponible debe conservar el estado materializado");
        assert_eq!(imported.dataset_file_name, "snapshot.parquet");

        let opened = store
            .open(&DatasetState::default(), imported.id)
            .expect("el proyecto importado desde snapshot debe reabrirse");
        assert_eq!(opened.dataset.columns[0].name, "amount");
        assert_eq!(opened.dataset.columns[1].name, "label");
    }

    #[test]
    fn dataprep_session_mapping_rejects_sheet_options_for_non_workbooks() {
        let directory = tempfile::tempdir().unwrap();
        let store = ProjectStore::initialize(directory.path().join("data")).unwrap();
        let session = write_session(directory.path(), "source.csv", serde_json::json!({}));
        let error = import_dataprep_session_project_from_path(
            &store,
            &session,
            None,
            Some("Datos".to_owned()),
            None,
        )
        .unwrap_err();

        assert!(error.contains("no puede asignar una hoja"));
        assert!(store.list().unwrap().is_empty());
        assert_eq!(snapshot_count(&store), 0);
    }

    #[test]
    fn dataprep_session_roundtrips_through_review_validation_and_export() {
        let directory = tempfile::tempdir().unwrap();
        let store = ProjectStore::initialize(directory.path().join("data")).unwrap();
        let source = directory.path().join("source.csv");
        let session = directory.path().join("session.json");
        let output = directory.path().join("validated.csv");
        fs::write(&source, "value,label\n1,uno\n2,dos\n").unwrap();
        fs::write(
            &session,
            serde_json::to_vec(&serde_json::json!({
                "version": 1,
                "name": "Pipeline ventas",
                "source_path": "source.csv",
                "stage_label": "Preparar",
                "transform_config": {"rename_text": "value -> amount"},
                "quality_rules": [{
                    "column": "amount",
                    "kind": "not_null",
                    "maxInvalid": 0
                }]
            }))
            .unwrap(),
        )
        .unwrap();

        let plan = dataset::load_dataprep_session_migration_plan(&session)
            .expect("la sesión válida debe producir un plan importable");
        assert!(plan.can_create_project);
        assert_eq!(
            plan.source_status,
            dataset::SessionReferenceStatus::Available
        );
        assert_eq!(plan.source_file_name.as_deref(), Some("source.csv"));
        assert_eq!(plan.stage_label.as_deref(), Some("Preparar"));
        assert_eq!(plan.quality_rules.len(), 1);

        let imported = import_dataprep_session_project_from_path(
            &store,
            &session,
            Some("Ventas importadas".to_owned()),
            None,
            None,
        )
        .expect("la sesión debe convertirse en un proyecto");

        let reopened_state = DatasetState::default();
        let opened = store
            .open(&reopened_state, imported.id.clone())
            .expect("el proyecto importado debe poder reabrirse");
        assert_eq!(opened.project.name, "Ventas importadas");
        assert_eq!(
            (opened.dataset.row_count, opened.dataset.column_count),
            (2, 2)
        );
        assert_eq!(opened.dataset.columns[0].name, "amount");
        assert_eq!(opened.dataset.columns[0].data_type, "str");
        assert_eq!(opened.workspace.quality_rules.len(), 1);
        assert!(opened.workspace.recipe_draft.is_some());
        let active = reopened_state
            .active_project_snapshot()
            .expect("el estado reabierto debe conservar el frame activo");

        let validation = dataset::evaluate_quality_rules_for_automation(
            &active.frame,
            &opened.workspace.quality_rules,
        )
        .expect("las reglas migradas deben validarse contra el dataset reabierto");
        assert!(validation.passed);
        assert_eq!(validation.row_count, 2);
        assert_eq!(validation.total_rules, 1);

        let exported = dataset::export_frame_for_automation_with_recipe(
            &active.frame,
            &output,
            dataset::ExportFormat::Csv,
            opened.workspace.recipe_draft.as_ref(),
        )
        .expect("el dataset validado debe poder exportarse");
        assert_eq!(exported.file_name, "validated.csv");
        let exported_csv = fs::read_to_string(&output).unwrap();
        assert!(exported_csv.starts_with("amount,label"));
        assert!(exported_csv.contains("1,uno"));
        assert!(!exported_csv.contains("source.csv"));

        let inspect = store
            .stored_project(&store.connection().unwrap(), &imported.id)
            .unwrap()
            .unwrap();
        assert_eq!(inspect.summary.row_count, 2);
        assert_eq!(inspect.summary.column_count, 2);
    }

    #[test]
    fn dataprep_session_fixture_roundtrips_sheet_workspace_and_history_artifacts() {
        let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("Cargo debe vivir dentro del repositorio");
        let fixture_directory = repository.join("fixtures/migration");
        let directory = tempfile::tempdir().unwrap();
        let session = directory.path().join("session-roundtrip.json");
        let source = directory.path().join("ventas-hoja.csv");
        fs::copy(
            fixture_directory.join("dataprep-session-v1-roundtrip.json"),
            &session,
        )
        .unwrap();
        fs::copy(fixture_directory.join("ventas-hoja.csv"), &source).unwrap();

        let store = ProjectStore::initialize(directory.path().join("projects")).unwrap();
        let plan = dataset::load_dataprep_session_migration_plan(&session)
            .expect("la fixture de sesión debe producir un plan válido");
        assert!(plan.can_create_project);
        assert_eq!(
            plan.source_status,
            dataset::SessionReferenceStatus::Available
        );
        assert_eq!(plan.sheet_name.as_deref(), Some("Datos"));
        assert_eq!(plan.quality_rules.len(), 1);

        let imported =
            import_dataprep_session_project_from_path(&store, &session, None, None, None)
                .expect("la fixture debe convertirse en un proyecto durable");
        let active_state = DatasetState::default();
        let opened = store
            .open(&active_state, imported.id.clone())
            .expect("el proyecto importado debe reabrirse");
        assert_eq!(opened.dataset.file_name, "ventas-hoja.csv");
        assert_eq!(opened.dataset.columns[0].name, "new_name");
        assert_eq!(opened.dataset.columns[1].name, "amount");
        assert_eq!(opened.workspace.quality_rules.len(), 1);
        let recipe = opened
            .workspace
            .recipe_draft
            .as_ref()
            .expect("la receta migrada debe persistirse en el workspace");
        let recipe_json = serde_json::to_value(recipe)
            .expect("el artefacto de receta debe conservar metadatos de sesión");
        let session_metadata = &recipe_json["migrationReport"]["session"];
        assert_eq!(session_metadata["sheetName"], "Datos");
        assert_eq!(session_metadata["appliedOperationCount"], 2);
        assert_eq!(session_metadata["qualityRuleCount"], 1);
        assert_eq!(session_metadata["analysisCheckCount"], 2);
        assert_eq!(
            session_metadata["appliedOperations"],
            serde_json::json!(["normalize_text", "rename_text"])
        );
        assert_eq!(
            session_metadata["analysisChecks"],
            serde_json::json!(["completeness", "duplicates"])
        );

        let initial_frame = active_state
            .active_project_snapshot()
            .expect("el dataset importado debe estar activo")
            .frame;
        let revised_frame = DataFrame::new(
            3,
            vec![
                Series::new("new_name".into(), vec!["Alpha", "Beta", "Gamma"]).into_column(),
                Series::new("amount".into(), vec![10_i64, 20, 30]).into_column(),
            ],
        )
        .unwrap();
        active_state
            .project_test_record(revised_frame.clone(), "Agregar venta")
            .unwrap();
        store
            .save(
                &active_state,
                Some(imported.id.clone()),
                "Sesión round-trip de ventas".to_owned(),
                opened.workspace.clone(),
            )
            .expect("el workspace y el historial deben guardarse juntos");

        let stored = store
            .stored_project(&store.connection().unwrap(), &imported.id)
            .unwrap()
            .unwrap();
        let generation = store
            .generation_path(&imported.id, stored.generation_name.as_deref().unwrap())
            .unwrap();
        let artifacts = fs::read_dir(&generation)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert!(artifacts.iter().any(|name| name == "current.parquet"));
        assert!(artifacts.iter().any(|name| name == "history-000.parquet"));
        assert!(artifacts.iter().any(|name| name == "history-001.parquet"));
        drop(store);

        let reopened_store = ProjectStore::initialize(directory.path().join("projects")).unwrap();
        let restored_state = DatasetState::default();
        let restored = reopened_store
            .open(&restored_state, imported.id)
            .expect("los artefactos de la sesión deben sobrevivir al reinicio");
        assert_eq!(restored.dataset.columns[0].name, "new_name");
        assert_eq!(restored.dataset.row_count, 3);
        let restored_history = restored_state.active_project_snapshot().unwrap().history;
        assert_eq!(restored_history.entries.len(), 3);
        assert_eq!(restored_history.cursor, 2);
        assert!(restored_state
            .project_test_undo()
            .unwrap()
            .equals_missing(&initial_frame));
        assert!(restored_state
            .project_test_redo()
            .unwrap()
            .equals_missing(&revised_frame));
    }

    #[test]
    fn automation_inspect_is_read_only_and_reports_recipe_history_and_profile() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("projects");
        let dataset =
            DatasetState::for_project_import(frame(&[1, 2]), "source.csv".to_owned()).unwrap();
        let recipe = serde_json::from_value(serde_json::json!({
            "renames": [{ "from": "value", "to": "amount" }]
        }))
        .unwrap();
        assert!(dataset.apply_project_import_recipe(&recipe).unwrap());
        dataset.cache_project_import_profile().unwrap();
        let workspace: ProjectWorkspace = serde_json::from_value(serde_json::json!({
            "qualityRules": [{
                "column": "amount",
                "kind": "not_null",
                "maxInvalid": 0
            }],
            "recipeDraft": {
                "version": 1,
                "name": "Rename amount",
                "savedAt": "2026-08-21T12:00:00Z",
                "recipe": { "renames": [{ "from": "value", "to": "amount" }] }
            }
        }))
        .unwrap();
        let saved = automation_import_project(
            &root,
            &dataset,
            None,
            "Con estado".to_owned(),
            workspace.clone(),
        )
        .unwrap();
        let store = ProjectStore::initialize(root.clone()).unwrap();
        store
            .connection()
            .unwrap()
            .execute(
                "UPDATE projects SET last_opened_at = NULL WHERE id = ?1",
                params![saved.id],
            )
            .unwrap();
        drop(store);

        let inspection = automation_inspect_project(&root, &saved.id).unwrap();
        assert!(inspection.profile_cached);
        assert_eq!(inspection.quality_rule_count, 1);
        assert!(inspection.recipe_draft_present);
        assert_eq!(inspection.history.entry_count, 2);
        assert_eq!(inspection.history.current_index, 1);
        assert!(inspection.history.can_undo);
        assert!(!inspection.history.can_redo);
        assert!(inspection.history.snapshots_enabled);
        assert!(!inspection.history.degraded);

        let store = ProjectStore::initialize(root.clone()).unwrap();
        assert!(store.recovery_candidate().unwrap().is_none());
        drop(store);
        let opened = automation_open_project(&root, &saved.id).unwrap();
        assert_eq!(opened.workspace, workspace);
        assert_eq!(opened.frame.get_column_names(), &["amount", "label"]);
        assert!(ProjectStore::initialize(root)
            .unwrap()
            .recovery_candidate()
            .unwrap()
            .is_none());
    }

    #[test]
    fn automation_store_rejects_files_and_path_errors_are_generic() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("not-a-directory");
        fs::write(&file, b"private").unwrap();
        let error = automation_list_projects(&file).unwrap_err();
        assert!(!error.contains(file.to_string_lossy().as_ref()));

        let traversing = directory.path().join("safe").join("..").join("projects");
        let error = automation_list_projects(&traversing).unwrap_err();
        assert!(!error.contains(directory.path().to_string_lossy().as_ref()));
    }

    #[cfg(unix)]
    #[test]
    fn automation_store_rejects_symbolic_root_segments() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let outside = directory.path().join("outside");
        fs::create_dir(&outside).unwrap();
        let link = directory.path().join("linked");
        symlink(&outside, &link).unwrap();

        let error = automation_list_projects(&link.join("projects")).unwrap_err();
        assert!(!error.contains(directory.path().to_string_lossy().as_ref()));
    }

    #[cfg(unix)]
    #[test]
    fn open_rejects_symbolic_snapshot_links() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let store = ProjectStore::initialize(directory.path().join("data")).unwrap();
        let (state, _) = active_state(directory.path(), &[1], "input.csv");
        let project = store
            .save(
                &state,
                None,
                "Proyecto".to_owned(),
                ProjectWorkspace::default(),
            )
            .unwrap();
        let stored = store
            .stored_project(&store.connection().unwrap(), &project.id)
            .unwrap()
            .unwrap();
        let generation = store
            .generation_path(&project.id, stored.generation_name.as_deref().unwrap())
            .unwrap();
        let snapshot = store
            .generation_file(&generation, "current.parquet")
            .unwrap();
        let outside = directory.path().join("outside.parquet");
        fs::rename(&snapshot, &outside).unwrap();
        symlink(&outside, &snapshot).unwrap();

        assert!(store.open(&DatasetState::default(), project.id).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn automation_store_rejects_windows_root_reparse_points_when_supported() {
        use std::{io::ErrorKind, os::windows::fs::symlink_dir};

        let directory = tempfile::tempdir().unwrap();
        let outside = directory.path().join("outside");
        fs::create_dir(&outside).unwrap();
        let link = directory.path().join("linked");
        match symlink_dir(&outside, &link) {
            Ok(()) => {
                let error = automation_list_projects(&link.join("projects")).unwrap_err();
                assert!(!error.contains(directory.path().to_string_lossy().as_ref()));
            }
            Err(error) if error.kind() == ErrorKind::PermissionDenied => {}
            Err(error) => panic!("no se pudo crear el reparse point de prueba: {error}"),
        }
    }

    #[cfg(windows)]
    #[test]
    fn open_rejects_windows_snapshot_reparse_points_when_supported() {
        use std::{io::ErrorKind, os::windows::fs::symlink_file};

        let directory = tempfile::tempdir().unwrap();
        let store = ProjectStore::initialize(directory.path().join("data")).unwrap();
        let (state, _) = active_state(directory.path(), &[1], "input.csv");
        let project = store
            .save(
                &state,
                None,
                "Proyecto".to_owned(),
                ProjectWorkspace::default(),
            )
            .unwrap();
        let stored = store
            .stored_project(&store.connection().unwrap(), &project.id)
            .unwrap()
            .unwrap();
        let generation = store
            .generation_path(&project.id, stored.generation_name.as_deref().unwrap())
            .unwrap();
        let snapshot = store
            .generation_file(&generation, "current.parquet")
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
