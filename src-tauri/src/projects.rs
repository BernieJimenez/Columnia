use std::{
    collections::HashSet,
    fs::{self},
    io::Read,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
};

use chrono::{SecondsFormat, Utc};
use polars::prelude::{DataFrame, ParquetWriter};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::dataset::{
    validate_import_profile, validate_project_profile_with_row_count, validate_project_workspace,
    DatasetPreview, DatasetProfile, DatasetState, ImportProfile, ProjectHistoryCapture,
    ProjectHistoryRestore, ProjectHistoryRestoreEntry, QualityRule, StoredTransformRecipe,
    OPERATION_CANCELLED_MESSAGE,
};
use sha2::{Digest, Sha256};

#[cfg(test)]
#[path = "projects/import_profile_tests.rs"]
mod import_profile_tests;

const SCHEMA_VERSION: i64 = 15;
const ID_LENGTH: usize = 32;
const MAX_PROJECT_VERSIONS: usize = 5;
const AUTOSAVE_QUOTA_BYTES: u64 = 512 * 1024 * 1024;
const PROJECT_VERSION_FORMAT: u8 = 1;
const MAX_SQL_QUERY_HISTORY_ENTRIES: usize = 5;
const MAX_SQL_QUERY_DURATION_MS: u64 = 24 * 60 * 60 * 1000;
const MAX_COMPARISON_KEY_COLUMNS: usize = 16;
const MAX_COMPARISON_KEY_COLUMN_CHARS: usize = 256;
const PREVIEW_PAGE_SIZE: usize = 50;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSummary {
    pub id: String,
    pub name: String,
    pub dataset_file_name: String,
    pub row_count: usize,
    pub column_count: usize,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_bytes: Option<u64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCatalogSnapshot {
    pub projects: Vec<ProjectSummary>,
    pub recovery_candidate: Option<ProjectSummary>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectVersionSummary {
    pub id: i64,
    pub created_at: String,
    pub dataset_file_name: String,
    pub row_count: usize,
    pub column_count: usize,
    pub storage_bytes: u64,
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sql_history: Vec<SqlQueryHistoryEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review_tab: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview_offset: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_phase: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query_engine: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub analysis_sample_rows: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub performance_profile: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub export_format: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub privacy_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub comparison_key_columns: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub join_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub import_profile: Option<ImportProfile>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SqlQueryHistoryEntry {
    pub id: u64,
    pub outcome: String,
    pub duration_ms: u64,
    pub row_count: Option<usize>,
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
    pub(crate) dataset_state: Option<DatasetState>,
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

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct StoredProject {
    summary: ProjectSummary,
    snapshot_name: String,
    quality_rules_json: String,
    recipe_draft_json: Option<String>,
    generation_name: Option<String>,
    history_manifest_json: Option<String>,
    profile_json: Option<String>,
    profile_cache_sha256: Option<String>,
    sql_history_json: String,
    review_tab: String,
    preview_offset: i64,
    active_phase: String,
    query_engine: Option<String>,
    analysis_sample_rows: Option<i64>,
    performance_profile: Option<String>,
    export_format: Option<String>,
    privacy_mode: Option<String>,
    comparison_key_columns_json: String,
    join_type: Option<String>,
    import_profile_json: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredProjectVersion {
    format_version: u8,
    project: StoredProject,
}

struct ValidatedProject {
    stored: StoredProject,
    workspace: ProjectWorkspace,
    profile: Option<DatasetProfile>,
    candidate: crate::dataset::ProjectDatasetCandidate,
}

struct PreparedProjectRestore {
    current: StoredProject,
    current_storage_bytes: u64,
    validated: ValidatedProject,
}

struct StagedGenerationGuard {
    path: PathBuf,
    keep: bool,
}

impl StagedGenerationGuard {
    fn new(path: PathBuf) -> Self {
        Self { path, keep: false }
    }

    fn keep(&mut self) {
        self.keep = true;
    }
}

impl Drop for StagedGenerationGuard {
    fn drop(&mut self) {
        if !self.keep {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
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
    #[serde(default)]
    id: String,
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
                        "SELECT generation_name FROM projects WHERE generation_name IS NOT NULL
                         UNION SELECT generation_name FROM project_versions
                         WHERE generation_name IS NOT NULL",
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
        let migration = match version {
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
                           profile_json TEXT,
                           profile_cache_sha256 TEXT,
                           sql_history_json TEXT NOT NULL DEFAULT '[]',
                           review_tab TEXT NOT NULL DEFAULT 'diagnosis',
                           preview_offset INTEGER NOT NULL DEFAULT 0,
                           active_phase TEXT NOT NULL DEFAULT 'review',
                           query_engine TEXT,
                           analysis_sample_rows INTEGER,
                           performance_profile TEXT,
                           export_format TEXT,
                           privacy_mode TEXT,
                           comparison_key_columns_json TEXT NOT NULL DEFAULT '[]',
                           join_type TEXT
                         );
                          CREATE INDEX projects_updated_at ON projects(updated_at DESC, id ASC);
                          ALTER TABLE projects ADD COLUMN import_profile_json TEXT;
                           PRAGMA user_version = 13;",
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
                         ALTER TABLE projects ADD COLUMN profile_cache_sha256 TEXT;
                          ALTER TABLE projects ADD COLUMN sql_history_json TEXT NOT NULL DEFAULT '[]';
                           ALTER TABLE projects ADD COLUMN review_tab TEXT NOT NULL DEFAULT 'diagnosis';
                           ALTER TABLE projects ADD COLUMN preview_offset INTEGER NOT NULL DEFAULT 0;
                           ALTER TABLE projects ADD COLUMN active_phase TEXT NOT NULL DEFAULT 'review';
                           ALTER TABLE projects ADD COLUMN query_engine TEXT;
                         ALTER TABLE projects ADD COLUMN analysis_sample_rows INTEGER;
                           ALTER TABLE projects ADD COLUMN performance_profile TEXT;
                           ALTER TABLE projects ADD COLUMN export_format TEXT;
                           ALTER TABLE projects ADD COLUMN privacy_mode TEXT;
                           ALTER TABLE projects ADD COLUMN comparison_key_columns_json TEXT NOT NULL DEFAULT '[]';
                           ALTER TABLE projects ADD COLUMN join_type TEXT;
                           ALTER TABLE projects ADD COLUMN import_profile_json TEXT;
                           PRAGMA user_version = 13;",
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
                         ALTER TABLE projects ADD COLUMN profile_cache_sha256 TEXT;
                          ALTER TABLE projects ADD COLUMN sql_history_json TEXT NOT NULL DEFAULT '[]';
                           ALTER TABLE projects ADD COLUMN review_tab TEXT NOT NULL DEFAULT 'diagnosis';
                           ALTER TABLE projects ADD COLUMN preview_offset INTEGER NOT NULL DEFAULT 0;
                           ALTER TABLE projects ADD COLUMN active_phase TEXT NOT NULL DEFAULT 'review';
                           ALTER TABLE projects ADD COLUMN query_engine TEXT;
                           ALTER TABLE projects ADD COLUMN analysis_sample_rows INTEGER;
                           ALTER TABLE projects ADD COLUMN performance_profile TEXT;
                           ALTER TABLE projects ADD COLUMN export_format TEXT;
                           ALTER TABLE projects ADD COLUMN privacy_mode TEXT;
                           ALTER TABLE projects ADD COLUMN comparison_key_columns_json TEXT NOT NULL DEFAULT '[]';
                           ALTER TABLE projects ADD COLUMN join_type TEXT;
                           ALTER TABLE projects ADD COLUMN import_profile_json TEXT;
                           PRAGMA user_version = 13;",
                    )
                    .map_err(|_| storage_error())?;
                transaction.commit().map_err(|_| storage_error())
            }
            3 => {
                let transaction = connection
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .map_err(|_| storage_error())?;
                transaction
                    .execute_batch(
                        "ALTER TABLE projects ADD COLUMN sql_history_json TEXT NOT NULL DEFAULT '[]';
                         ALTER TABLE projects ADD COLUMN profile_cache_sha256 TEXT;
                         ALTER TABLE projects ADD COLUMN review_tab TEXT NOT NULL DEFAULT 'diagnosis';
                         ALTER TABLE projects ADD COLUMN preview_offset INTEGER NOT NULL DEFAULT 0;
                         ALTER TABLE projects ADD COLUMN active_phase TEXT NOT NULL DEFAULT 'review';
                         ALTER TABLE projects ADD COLUMN query_engine TEXT;
                         ALTER TABLE projects ADD COLUMN analysis_sample_rows INTEGER;
                           ALTER TABLE projects ADD COLUMN performance_profile TEXT;
                           ALTER TABLE projects ADD COLUMN export_format TEXT;
                           ALTER TABLE projects ADD COLUMN privacy_mode TEXT;
                           ALTER TABLE projects ADD COLUMN comparison_key_columns_json TEXT NOT NULL DEFAULT '[]';
                           ALTER TABLE projects ADD COLUMN join_type TEXT;
                           ALTER TABLE projects ADD COLUMN import_profile_json TEXT;
                           PRAGMA user_version = 13;",
                    )
                    .map_err(|_| storage_error())?;
                transaction.commit().map_err(|_| storage_error())
            }
            4 => {
                let transaction = connection
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .map_err(|_| storage_error())?;
                transaction
                    .execute_batch(
                        "ALTER TABLE projects ADD COLUMN profile_cache_sha256 TEXT;
                          ALTER TABLE projects ADD COLUMN review_tab TEXT NOT NULL DEFAULT 'diagnosis';
                          ALTER TABLE projects ADD COLUMN preview_offset INTEGER NOT NULL DEFAULT 0;
                          ALTER TABLE projects ADD COLUMN active_phase TEXT NOT NULL DEFAULT 'review';
                          ALTER TABLE projects ADD COLUMN query_engine TEXT;
                          ALTER TABLE projects ADD COLUMN analysis_sample_rows INTEGER;
                           ALTER TABLE projects ADD COLUMN performance_profile TEXT;
                           ALTER TABLE projects ADD COLUMN export_format TEXT;
                           ALTER TABLE projects ADD COLUMN privacy_mode TEXT;
                           ALTER TABLE projects ADD COLUMN comparison_key_columns_json TEXT NOT NULL DEFAULT '[]';
                           ALTER TABLE projects ADD COLUMN join_type TEXT;
                           ALTER TABLE projects ADD COLUMN import_profile_json TEXT;
                           PRAGMA user_version = 13;",
                    )
                    .map_err(|_| storage_error())?;
                transaction.commit().map_err(|_| storage_error())
            }
            5 => {
                let transaction = connection
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .map_err(|_| storage_error())?;
                transaction
                    .execute_batch(
                        "ALTER TABLE projects ADD COLUMN review_tab TEXT NOT NULL DEFAULT 'diagnosis';
                         ALTER TABLE projects ADD COLUMN preview_offset INTEGER NOT NULL DEFAULT 0;
                         ALTER TABLE projects ADD COLUMN active_phase TEXT NOT NULL DEFAULT 'review';
                         ALTER TABLE projects ADD COLUMN query_engine TEXT;
                         ALTER TABLE projects ADD COLUMN analysis_sample_rows INTEGER;
                           ALTER TABLE projects ADD COLUMN performance_profile TEXT;
                           ALTER TABLE projects ADD COLUMN export_format TEXT;
                           ALTER TABLE projects ADD COLUMN privacy_mode TEXT;
                           ALTER TABLE projects ADD COLUMN comparison_key_columns_json TEXT NOT NULL DEFAULT '[]';
                           ALTER TABLE projects ADD COLUMN join_type TEXT;
                           ALTER TABLE projects ADD COLUMN import_profile_json TEXT;
                           PRAGMA user_version = 13;",
                    )
                    .map_err(|_| storage_error())?;
                transaction.commit().map_err(|_| storage_error())
            }
            6 => {
                let transaction = connection
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .map_err(|_| storage_error())?;
                transaction
                    .execute_batch(
                        "ALTER TABLE projects ADD COLUMN preview_offset INTEGER NOT NULL DEFAULT 0;
                         ALTER TABLE projects ADD COLUMN active_phase TEXT NOT NULL DEFAULT 'review';
                         ALTER TABLE projects ADD COLUMN query_engine TEXT;
                         ALTER TABLE projects ADD COLUMN analysis_sample_rows INTEGER;
                           ALTER TABLE projects ADD COLUMN performance_profile TEXT;
                           ALTER TABLE projects ADD COLUMN export_format TEXT;
                           ALTER TABLE projects ADD COLUMN privacy_mode TEXT;
                           ALTER TABLE projects ADD COLUMN comparison_key_columns_json TEXT NOT NULL DEFAULT '[]';
                           ALTER TABLE projects ADD COLUMN join_type TEXT;
                           ALTER TABLE projects ADD COLUMN import_profile_json TEXT;
                           PRAGMA user_version = 13;",
                    )
                    .map_err(|_| storage_error())?;
                transaction.commit().map_err(|_| storage_error())
            }
            7 => {
                let transaction = connection
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .map_err(|_| storage_error())?;
                transaction
                    .execute_batch(
                        "ALTER TABLE projects ADD COLUMN active_phase TEXT NOT NULL DEFAULT 'review';
                         ALTER TABLE projects ADD COLUMN query_engine TEXT;
                         ALTER TABLE projects ADD COLUMN analysis_sample_rows INTEGER;
                           ALTER TABLE projects ADD COLUMN performance_profile TEXT;
                           ALTER TABLE projects ADD COLUMN export_format TEXT;
                           ALTER TABLE projects ADD COLUMN privacy_mode TEXT;
                           ALTER TABLE projects ADD COLUMN comparison_key_columns_json TEXT NOT NULL DEFAULT '[]';
                           ALTER TABLE projects ADD COLUMN join_type TEXT;
                           ALTER TABLE projects ADD COLUMN import_profile_json TEXT;
                           PRAGMA user_version = 13;",
                    )
                    .map_err(|_| storage_error())?;
                transaction.commit().map_err(|_| storage_error())
            }
            8 => {
                let transaction = connection
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .map_err(|_| storage_error())?;
                transaction
                    .execute_batch(
                        "ALTER TABLE projects ADD COLUMN analysis_sample_rows INTEGER;
                         ALTER TABLE projects ADD COLUMN query_engine TEXT;
                           ALTER TABLE projects ADD COLUMN performance_profile TEXT;
                           ALTER TABLE projects ADD COLUMN export_format TEXT;
                           ALTER TABLE projects ADD COLUMN privacy_mode TEXT;
                           ALTER TABLE projects ADD COLUMN comparison_key_columns_json TEXT NOT NULL DEFAULT '[]';
                           ALTER TABLE projects ADD COLUMN join_type TEXT;
                           ALTER TABLE projects ADD COLUMN import_profile_json TEXT;
                           PRAGMA user_version = 13;",
                    )
                    .map_err(|_| storage_error())?;
                transaction.commit().map_err(|_| storage_error())
            }
            9 => {
                let transaction = connection
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .map_err(|_| storage_error())?;
                transaction
                    .execute_batch(
                        "ALTER TABLE projects ADD COLUMN query_engine TEXT;
                           ALTER TABLE projects ADD COLUMN performance_profile TEXT;
                           ALTER TABLE projects ADD COLUMN export_format TEXT;
                           ALTER TABLE projects ADD COLUMN privacy_mode TEXT;
                           ALTER TABLE projects ADD COLUMN comparison_key_columns_json TEXT NOT NULL DEFAULT '[]';
                           ALTER TABLE projects ADD COLUMN join_type TEXT;
                           ALTER TABLE projects ADD COLUMN import_profile_json TEXT;
                           PRAGMA user_version = 13;",
                    )
                    .map_err(|_| storage_error())?;
                transaction.commit().map_err(|_| storage_error())
            }
            10 => {
                let transaction = connection
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .map_err(|_| storage_error())?;
                transaction
                    .execute_batch(
                        "ALTER TABLE projects ADD COLUMN performance_profile TEXT;
                         ALTER TABLE projects ADD COLUMN export_format TEXT;
                         ALTER TABLE projects ADD COLUMN privacy_mode TEXT;
                         ALTER TABLE projects ADD COLUMN comparison_key_columns_json TEXT NOT NULL DEFAULT '[]';
                         ALTER TABLE projects ADD COLUMN join_type TEXT;
                         ALTER TABLE projects ADD COLUMN import_profile_json TEXT;
                           PRAGMA user_version = 13;",
                    )
                    .map_err(|_| storage_error())?;
                transaction.commit().map_err(|_| storage_error())
            }
            11 => {
                let transaction = connection
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .map_err(|_| storage_error())?;
                transaction
                    .execute_batch(
                        "ALTER TABLE projects ADD COLUMN export_format TEXT;
                         ALTER TABLE projects ADD COLUMN privacy_mode TEXT;
                         ALTER TABLE projects ADD COLUMN comparison_key_columns_json TEXT NOT NULL DEFAULT '[]';
                         ALTER TABLE projects ADD COLUMN join_type TEXT;
                         ALTER TABLE projects ADD COLUMN import_profile_json TEXT;
                           PRAGMA user_version = 13;",
                    )
                    .map_err(|_| storage_error())?;
                transaction.commit().map_err(|_| storage_error())
            }
            12 => {
                let transaction = connection
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .map_err(|_| storage_error())?;
                transaction
                    .execute_batch(
                        "ALTER TABLE projects ADD COLUMN import_profile_json TEXT;
                         PRAGMA user_version = 13;",
                    )
                    .map_err(|_| storage_error())?;
                transaction.commit().map_err(|_| storage_error())
            }
            13 => {
                let transaction = connection
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .map_err(|_| storage_error())?;
                transaction
                    .execute_batch(
                        "CREATE TABLE project_versions (
                           id INTEGER PRIMARY KEY AUTOINCREMENT,
                           project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                           created_at TEXT NOT NULL,
                           payload_json TEXT NOT NULL,
                           generation_name TEXT,
                           snapshot_name TEXT NOT NULL,
                           storage_bytes INTEGER NOT NULL CHECK (storage_bytes >= 0)
                         );
                         CREATE INDEX project_versions_project
                           ON project_versions(project_id, id DESC);
                         PRAGMA user_version = 14;",
                    )
                    .map_err(|_| storage_error())?;
                transaction.commit().map_err(|_| storage_error())
            }
            14 => {
                let transaction = connection
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .map_err(|_| storage_error())?;
                transaction
                    .execute_batch(
                        "CREATE INDEX IF NOT EXISTS projects_recovery_candidate
                           ON projects(last_opened_at DESC, id ASC)
                           WHERE last_opened_at IS NOT NULL;
                         PRAGMA user_version = 15;",
                    )
                    .map_err(|_| storage_error())?;
                transaction.commit().map_err(|_| storage_error())
            }
            _ => Err(storage_error()),
        };
        if migration.is_ok() && version < SCHEMA_VERSION {
            return self.migrate();
        }
        migration
    }

    fn list(&self) -> Result<Vec<ProjectSummary>, String> {
        self.list_with_cancel(|| false)
    }

    fn list_with_cancel<C>(&self, is_cancelled: C) -> Result<Vec<ProjectSummary>, String>
    where
        C: Fn() -> bool + Sync,
    {
        ensure_project_operation_not_cancelled(is_cancelled())?;
        self.ensure_initialized()?;
        ensure_project_operation_not_cancelled(is_cancelled())?;
        let connection = self.connection()?;
        self.list_from_connection(&connection, &is_cancelled)
    }

    fn list_catalog_with_cancel<C>(&self, is_cancelled: C) -> Result<ProjectCatalogSnapshot, String>
    where
        C: Fn() -> bool + Sync,
    {
        ensure_project_operation_not_cancelled(is_cancelled())?;
        self.ensure_initialized()?;
        ensure_project_operation_not_cancelled(is_cancelled())?;
        let mut connection = self.connection()?;
        let transaction = connection.transaction().map_err(|_| storage_error())?;
        let projects = self.list_from_connection(&transaction, &is_cancelled)?;
        ensure_project_operation_not_cancelled(is_cancelled())?;
        let candidate_id = transaction
            .query_row(
                "SELECT id FROM projects WHERE last_opened_at IS NOT NULL
                 ORDER BY last_opened_at DESC, id ASC LIMIT 1",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|_| storage_error())?;
        ensure_project_operation_not_cancelled(is_cancelled())?;
        let recovery_candidate =
            candidate_id.and_then(|id| projects.iter().find(|project| project.id == id).cloned());
        ensure_project_operation_not_cancelled(is_cancelled())?;
        transaction.commit().map_err(|_| storage_error())?;
        Ok(ProjectCatalogSnapshot {
            projects,
            recovery_candidate,
        })
    }

    fn list_from_connection<C>(
        &self,
        connection: &Connection,
        is_cancelled: &C,
    ) -> Result<Vec<ProjectSummary>, String>
    where
        C: Fn() -> bool + Sync,
    {
        ensure_project_operation_not_cancelled(is_cancelled())?;
        let mut statement = connection
            .prepare(
                "SELECT id, name, dataset_file_name, row_count, column_count, created_at, updated_at
                 FROM projects ORDER BY updated_at DESC, id ASC",
            )
            .map_err(|_| storage_error())?;
        let rows = statement
            .query_map([], summary_from_row)
            .map_err(|_| storage_error())?;
        let mut summaries = Vec::new();
        for summary in rows {
            ensure_project_operation_not_cancelled(is_cancelled())?;
            summaries.push(summary.map_err(|_| storage_error())?);
        }
        drop(statement);
        let mut projects = Vec::with_capacity(summaries.len());
        for summary in summaries {
            ensure_project_operation_not_cancelled(is_cancelled())?;
            projects.push(self.with_storage_usage_with_cancel(
                &connection,
                summary,
                is_cancelled,
            )?);
        }
        ensure_project_operation_not_cancelled(is_cancelled())?;
        Ok(projects)
    }

    fn recovery_candidate(&self) -> Result<Option<ProjectSummary>, String> {
        self.ensure_initialized()?;
        let connection = self.connection()?;
        let summary = connection
            .query_row(
                "SELECT id, name, dataset_file_name, row_count, column_count, created_at, updated_at
                 FROM projects WHERE last_opened_at IS NOT NULL
                 ORDER BY last_opened_at DESC, id ASC LIMIT 1",
                [],
                summary_from_row,
            )
            .optional()
            .map_err(|_| storage_error())?;
        summary
            .map(|summary| self.with_storage_usage(&connection, summary))
            .transpose()
    }

    fn list_versions(&self, project_id: &str) -> Result<Vec<ProjectVersionSummary>, String> {
        self.ensure_initialized()?;
        validate_id(project_id)?;
        let connection = self.connection()?;
        if self.stored_project(&connection, project_id)?.is_none() {
            return Err("El proyecto solicitado no existe.".to_owned());
        }
        let mut statement = connection
            .prepare(
                "SELECT id, created_at, payload_json, storage_bytes
                 FROM project_versions WHERE project_id = ?1 ORDER BY id DESC",
            )
            .map_err(|_| storage_error())?;
        let rows = statement
            .query_map(params![project_id], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            })
            .map_err(|_| storage_error())?;
        let mut versions = Vec::new();
        for row in rows {
            let (id, created_at, payload_json, storage_bytes) = row.map_err(|_| storage_error())?;
            let stored = decode_project_version(&payload_json)?;
            if stored.summary.id != project_id {
                return Err(storage_error());
            }
            versions.push(ProjectVersionSummary {
                id,
                created_at,
                dataset_file_name: stored.summary.dataset_file_name,
                row_count: stored.summary.row_count,
                column_count: stored.summary.column_count,
                storage_bytes: u64::try_from(storage_bytes).unwrap_or(0),
            });
        }
        Ok(versions)
    }

    fn prepare_project_restore<C>(
        &self,
        project_id: &str,
        version_id: i64,
        is_cancelled: C,
    ) -> Result<PreparedProjectRestore, String>
    where
        C: Fn() -> bool + Send + Sync + Clone + 'static,
    {
        ensure_project_open_not_cancelled(is_cancelled())?;
        self.ensure_initialized()?;
        validate_id(project_id)?;
        if version_id <= 0 {
            return Err("La versión del proyecto no es válida.".to_owned());
        }
        let connection = self.connection()?;
        let current = self
            .stored_project(&connection, project_id)?
            .ok_or_else(|| "El proyecto solicitado no existe.".to_owned())?;
        ensure_project_open_not_cancelled(is_cancelled())?;
        let payload_json: String = connection
            .query_row(
                "SELECT payload_json FROM project_versions WHERE project_id = ?1 AND id = ?2",
                params![project_id, version_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|_| storage_error())?
            .ok_or_else(|| "La versión seleccionada ya no está disponible.".to_owned())?;
        let restored = decode_project_version(&payload_json)?;
        if restored.summary.id != project_id {
            return Err(storage_error());
        }
        ensure_project_open_not_cancelled(is_cancelled())?;
        // Prepare and validate the complete candidate before changing the catalog pointer.
        let validated = self.validate_stored_project_with_cancel(restored, is_cancelled.clone())?;
        ensure_project_open_not_cancelled(is_cancelled())?;
        let current_storage_bytes = self
            .stored_project_storage_bytes(&current)
            .unwrap_or_default();
        ensure_project_open_not_cancelled(is_cancelled())?;
        Ok(PreparedProjectRestore {
            current,
            current_storage_bytes,
            validated,
        })
    }

    #[cfg(test)]
    fn restore_version(&self, project_id: &str, version_id: i64) -> Result<ProjectSummary, String> {
        let PreparedProjectRestore {
            current,
            current_storage_bytes,
            validated,
        } = self.prepare_project_restore(project_id, version_id, || false)?;
        let ValidatedProject { stored, .. } = validated;
        self.commit_project_restore(current, current_storage_bytes, stored)
    }

    fn restore_version_cancellable(
        &self,
        dataset_state: &DatasetState,
        project_id: &str,
        version_id: i64,
        generation: u64,
    ) -> Result<ProjectOpenResult, String> {
        let cancelled = dataset_state.project_open_cancellation(generation);
        let prepare_cancelled = cancelled.clone();
        let PreparedProjectRestore {
            current,
            current_storage_bytes,
            validated,
        } = self.prepare_project_restore(project_id, version_id, move || prepare_cancelled())?;
        let ValidatedProject {
            stored,
            workspace,
            profile,
            candidate,
        } = validated;
        let (project, dataset) =
            dataset_state.commit_project_open_with_candidate(generation, candidate, move || {
                self.commit_project_restore(current, current_storage_bytes, stored)
            })?;
        Ok(ProjectOpenResult {
            project,
            dataset,
            workspace,
            profile,
        })
    }

    fn commit_project_restore(
        &self,
        current: StoredProject,
        current_storage_bytes: u64,
        mut restored: StoredProject,
    ) -> Result<ProjectSummary, String> {
        let project_id = restored.summary.id.clone();
        let mut connection = self.connection()?;
        let timestamp = now_utc();
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| storage_error())?;
        insert_project_version(&transaction, &current, current_storage_bytes)?;
        let created_at = current.summary.created_at.clone();
        let update = transaction.execute(
            "UPDATE projects SET name = ?1, dataset_file_name = ?2, row_count = ?3,
             column_count = ?4, snapshot_name = ?5, updated_at = ?6, last_opened_at = ?6,
             quality_rules_json = ?7, recipe_draft_json = ?8, generation_name = ?9,
             history_manifest_json = ?10, profile_json = ?11, profile_cache_sha256 = ?12,
             sql_history_json = ?13, review_tab = ?14, preview_offset = ?15,
             active_phase = ?16, query_engine = ?17, analysis_sample_rows = ?18,
             performance_profile = ?19, export_format = ?20, privacy_mode = ?21,
             comparison_key_columns_json = ?22, join_type = ?23,
             import_profile_json = ?24 WHERE id = ?25",
            params![
                restored.summary.name,
                restored.summary.dataset_file_name,
                usize_to_i64(restored.summary.row_count)?,
                usize_to_i64(restored.summary.column_count)?,
                restored.snapshot_name,
                timestamp,
                restored.quality_rules_json,
                restored.recipe_draft_json,
                restored.generation_name,
                restored.history_manifest_json,
                restored.profile_json,
                restored.profile_cache_sha256,
                restored.sql_history_json,
                restored.review_tab,
                restored.preview_offset,
                restored.active_phase,
                restored.query_engine,
                restored.analysis_sample_rows,
                restored.performance_profile,
                restored.export_format,
                restored.privacy_mode,
                restored.comparison_key_columns_json,
                restored.join_type,
                restored.import_profile_json,
                project_id,
            ],
        );
        if !matches!(update, Ok(1)) {
            return Err(storage_error());
        }
        let pruned = self.prune_versions_in_transaction(&transaction, &project_id)?;
        transaction.commit().map_err(|_| storage_error())?;
        restored.summary.created_at = created_at;
        restored.summary.updated_at = timestamp;
        restored.summary.storage_bytes = self.stored_project_storage_bytes(&restored).ok();
        self.cleanup_pruned_version_files(&project_id, pruned);
        Ok(restored.summary)
    }

    fn prune_versions_in_transaction(
        &self,
        transaction: &rusqlite::Transaction<'_>,
        project_id: &str,
    ) -> Result<Vec<(Option<String>, String)>, String> {
        let mut statement = transaction
            .prepare(
                "SELECT id, generation_name, snapshot_name, storage_bytes
                 FROM project_versions WHERE project_id = ?1 ORDER BY id DESC",
            )
            .map_err(|_| storage_error())?;
        let versions = statement
            .query_map(params![project_id], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            })
            .map_err(|_| storage_error())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| storage_error())?;
        drop(statement);
        let mut kept_count = 0usize;
        let mut kept_bytes = 0u64;
        let mut remove = Vec::new();
        for version in versions {
            let bytes = u64::try_from(version.3).unwrap_or(0);
            let is_safety_version = kept_count == 0;
            if is_safety_version
                || (kept_count < MAX_PROJECT_VERSIONS
                    && kept_bytes.saturating_add(bytes) <= AUTOSAVE_QUOTA_BYTES)
            {
                kept_count += 1;
                kept_bytes = kept_bytes.saturating_add(bytes);
            } else {
                remove.push(version);
            }
        }
        if remove.is_empty() {
            return Ok(Vec::new());
        }
        let mut pruned = Vec::with_capacity(remove.len());
        for version in &remove {
            let deleted = transaction
                .execute(
                    "DELETE FROM project_versions WHERE id = ?1 AND project_id = ?2",
                    params![version.0, project_id],
                )
                .map_err(|_| storage_error())?;
            if deleted != 1 {
                return Err(storage_error());
            }
            pruned.push((version.1.clone(), version.2.clone()));
        }
        Ok(pruned)
    }

    fn cleanup_pruned_version_files(
        &self,
        project_id: &str,
        pruned: Vec<(Option<String>, String)>,
    ) {
        for (generation_name, snapshot_name) in pruned {
            if let Some(name) = generation_name {
                let Ok(connection) = self.connection() else {
                    continue;
                };
                let still_referenced: Result<bool, _> = connection.query_row(
                    "SELECT EXISTS(SELECT 1 FROM projects WHERE generation_name = ?1)
                         OR EXISTS(SELECT 1 FROM project_versions WHERE generation_name = ?1)",
                    params![name],
                    |row| row.get(0),
                );
                let Ok(still_referenced) = still_referenced else {
                    continue;
                };
                if !still_referenced {
                    if let Ok(path) = self.generation_path(project_id, &name) {
                        let _ = fs::remove_dir_all(path);
                    }
                }
            } else {
                let Ok(connection) = self.connection() else {
                    continue;
                };
                let still_referenced: Result<bool, _> = connection.query_row(
                    "SELECT EXISTS(SELECT 1 FROM projects WHERE snapshot_name = ?1)
                         OR EXISTS(SELECT 1 FROM project_versions WHERE snapshot_name = ?1)",
                    params![snapshot_name],
                    |row| row.get(0),
                );
                let Ok(still_referenced) = still_referenced else {
                    continue;
                };
                if !still_referenced {
                    if let Ok(path) = self.managed_snapshot_path(project_id, &snapshot_name) {
                        let _ = fs::remove_file(path);
                    }
                }
            }
        }
    }

    #[cfg(test)]
    fn prune_versions(&self, project_id: &str) -> Result<(), String> {
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| storage_error())?;
        let pruned = self.prune_versions_in_transaction(&transaction, project_id)?;
        transaction.commit().map_err(|_| storage_error())?;
        self.cleanup_pruned_version_files(project_id, pruned);
        Ok(())
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

    fn save_cancellable(
        &self,
        dataset_state: &DatasetState,
        project_id: Option<String>,
        name: String,
        workspace: ProjectWorkspace,
        generation: u64,
    ) -> Result<ProjectSummary, String> {
        let cancellation = dataset_state.project_save_cancellation(generation);
        self.save_inner_with_cancellation(
            dataset_state,
            project_id,
            name,
            workspace,
            false,
            Some(generation),
            move || cancellation(),
        )
    }

    fn autosave(
        &self,
        dataset_state: &DatasetState,
        project_id: String,
        name: String,
        workspace: ProjectWorkspace,
    ) -> Result<ProjectSummary, String> {
        self.save_inner_with_options(dataset_state, Some(project_id), name, workspace, false)
    }

    fn autosave_cancellable(
        &self,
        dataset_state: &DatasetState,
        project_id: String,
        name: String,
        workspace: ProjectWorkspace,
        generation: u64,
    ) -> Result<ProjectSummary, String> {
        let cancellation = dataset_state.project_save_cancellation(generation);
        self.save_inner_with_cancellation(
            dataset_state,
            Some(project_id),
            name,
            workspace,
            false,
            Some(generation),
            move || cancellation(),
        )
    }

    fn save_inner(
        &self,
        dataset_state: &DatasetState,
        project_id: Option<String>,
        name: String,
        workspace: ProjectWorkspace,
        fail_before_database: bool,
    ) -> Result<ProjectSummary, String> {
        self.save_inner_with_options(
            dataset_state,
            project_id,
            name,
            workspace,
            fail_before_database,
        )
    }

    fn save_inner_with_options(
        &self,
        dataset_state: &DatasetState,
        project_id: Option<String>,
        name: String,
        workspace: ProjectWorkspace,
        fail_before_database: bool,
    ) -> Result<ProjectSummary, String> {
        self.save_inner_with_cancellation(
            dataset_state,
            project_id,
            name,
            workspace,
            fail_before_database,
            None,
            || false,
        )
    }

    fn save_inner_with_cancellation<C>(
        &self,
        dataset_state: &DatasetState,
        project_id: Option<String>,
        name: String,
        workspace: ProjectWorkspace,
        fail_before_database: bool,
        project_save_generation: Option<u64>,
        is_cancelled: C,
    ) -> Result<ProjectSummary, String>
    where
        C: Fn() -> bool + Clone + Send + Sync + 'static,
    {
        ensure_project_save_not_cancelled(is_cancelled())?;
        self.ensure_initialized()?;
        let name = validate_name(name)?;
        if let Some(id) = project_id.as_deref() {
            validate_id(id)?;
        }
        ensure_project_save_not_cancelled(is_cancelled())?;
        let mut active = dataset_state.active_project_snapshot_with_cancel(is_cancelled.clone())?;
        ensure_project_save_not_cancelled(is_cancelled())?;
        validate_project_workspace(
            &active.frame,
            &workspace.quality_rules,
            workspace.recipe_draft.as_ref(),
        )?;
        validate_sql_query_history(&workspace.sql_history)?;
        if let Some(profile) = active.profile.as_ref() {
            validate_project_profile_with_row_count(&active.frame, active.row_count, profile)?;
        }
        let quality_rules_json = serde_json::to_string(&workspace.quality_rules)
            .map_err(|_| "No se pudo validar la configuración del proyecto.".to_owned())?;
        let recipe_draft_json = workspace
            .recipe_draft
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|_| "No se pudo validar la configuración del proyecto.".to_owned())?;
        let sql_history_json = serde_json::to_string(&workspace.sql_history)
            .map_err(|_| "No se pudo validar la actividad SQL del proyecto.".to_owned())?;
        let review_tab = review_tab_label(workspace.review_tab.as_deref())?.to_owned();
        let preview_offset = usize_to_i64(validate_preview_offset(
            workspace.preview_offset,
            active.row_count,
        )?)?;
        let active_phase = active_phase_label(workspace.active_phase.as_deref())?.to_owned();
        let query_engine =
            validate_query_engine(workspace.query_engine.as_deref())?.map(str::to_owned);
        let analysis_sample_rows = validate_analysis_sample_rows(workspace.analysis_sample_rows)?
            .map(usize_to_i64)
            .transpose()?;
        let performance_profile =
            validate_performance_profile(workspace.performance_profile.as_deref())?
                .map(str::to_owned);
        let export_format =
            validate_export_format(workspace.export_format.as_deref())?.map(str::to_owned);
        let privacy_mode =
            validate_privacy_mode(workspace.privacy_mode.as_deref())?.map(str::to_owned);
        let comparison_key_columns = validate_comparison_key_columns_for_frame(
            &active.frame,
            &workspace.comparison_key_columns,
        )?;
        let comparison_key_columns_json = serde_json::to_string(&comparison_key_columns)
            .map_err(|_| "No se pudo validar la configuración del proyecto.".to_owned())?;
        let join_type = validate_join_type(workspace.join_type.as_deref())?.map(str::to_owned);
        if let Some(profile) = workspace.import_profile.as_ref() {
            validate_import_profile(profile)?;
        }
        let import_profile_json = workspace
            .import_profile
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|_| "No se pudo validar el perfil de importación del proyecto.".to_owned())?;
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
        let mut generation_guard = StagedGenerationGuard::new(generation_path.clone());
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
        ensure_project_save_not_cancelled(is_cancelled())?;
        let frame = std::mem::replace(&mut active.frame, DataFrame::empty());
        write_generation_with_cancel(
            frame,
            active.current_snapshot_path.as_deref(),
            &active.history,
            &generation_path,
            is_cancelled.clone(),
        )?;
        ensure_project_save_not_cancelled(is_cancelled())?;
        let storage_bytes = self
            .generation_storage_bytes(&generation_path, Some(&history_manifest))
            .ok();
        ensure_project_save_not_cancelled(is_cancelled())?;
        let profile_cache_sha256 = active
            .profile
            .as_ref()
            .map(|_| {
                hash_file_sha256_with_cancel(&generation_path.join("current.parquet"), || {
                    is_cancelled()
                })
            })
            .transpose()?;
        ensure_project_save_not_cancelled(is_cancelled())?;

        if fail_before_database {
            return Err(storage_error());
        }

        let timestamp = now_utc();
        let old_storage_bytes = existing
            .as_ref()
            .and_then(|project| self.stored_project_storage_bytes(project).ok())
            .unwrap_or(0);
        let created_at = existing
            .as_ref()
            .map(|project| project.summary.created_at.clone())
            .unwrap_or_else(|| timestamp.clone());
        ensure_project_save_not_cancelled(is_cancelled())?;
        let mut commit_catalog = || -> Result<Vec<(Option<String>, String)>, String> {
            let transaction = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(|_| storage_error())?;
            if let Some(previous) = existing.as_ref() {
                insert_project_version(&transaction, previous, old_storage_bytes)
                    .map_err(|_| storage_error())?;
            }
            let database_result = if existing.is_some() {
                transaction.execute(
                    "UPDATE projects SET name = ?1, dataset_file_name = ?2, row_count = ?3,
                 column_count = ?4, snapshot_name = ?5, updated_at = ?6, last_opened_at = ?6,
                 quality_rules_json = ?7, recipe_draft_json = ?8, generation_name = ?9,
                  history_manifest_json = ?10, profile_json = ?11, profile_cache_sha256 = ?12,
                 sql_history_json = ?13, review_tab = ?14, preview_offset = ?15,
                 active_phase = ?16, query_engine = ?17, analysis_sample_rows = ?18,
                 performance_profile = ?19, export_format = ?20, privacy_mode = ?21,
                 comparison_key_columns_json = ?22, join_type = ?23,
                 import_profile_json = ?24 WHERE id = ?25",
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
                        profile_cache_sha256,
                        sql_history_json,
                        review_tab,
                        preview_offset,
                        active_phase,
                        query_engine,
                        analysis_sample_rows,
                        performance_profile,
                        export_format,
                        privacy_mode,
                        comparison_key_columns_json,
                        join_type,
                        import_profile_json,
                        id
                    ],
                )
            } else {
                transaction.execute(
                    "INSERT INTO projects
                 (id, name, dataset_file_name, row_count, column_count, snapshot_name,
                  created_at, updated_at, last_opened_at, quality_rules_json, recipe_draft_json,
                   generation_name, history_manifest_json, profile_json, profile_cache_sha256,
                    sql_history_json, review_tab, preview_offset, active_phase, query_engine,
                    analysis_sample_rows, performance_profile, export_format, privacy_mode,
                    comparison_key_columns_json, join_type, import_profile_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25)",
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
                    profile_json,
                    profile_cache_sha256,
                    sql_history_json,
                    review_tab,
                    preview_offset,
                    active_phase,
                    query_engine,
                    analysis_sample_rows,
                    performance_profile,
                    export_format,
                    privacy_mode,
                    comparison_key_columns_json,
                    join_type,
                    import_profile_json,
                    ],
                )
            };
            if !matches!(database_result, Ok(1)) {
                return Err(storage_error());
            }
            let pruned = self.prune_versions_in_transaction(&transaction, &id)?;
            transaction.commit().map_err(|_| storage_error())?;
            generation_guard.keep();
            Ok(pruned)
        };
        let pruned = match project_save_generation {
            Some(generation) => dataset_state.commit_project_save(generation, commit_catalog)?,
            None => commit_catalog()?,
        };
        self.cleanup_pruned_version_files(&id, pruned);
        Ok(ProjectSummary {
            id,
            name,
            dataset_file_name: active.file_name,
            row_count: active.row_count,
            column_count: active.column_count,
            created_at,
            updated_at: timestamp,
            storage_bytes,
        })
    }

    fn load_validated(&self, project_id: &str) -> Result<ValidatedProject, String> {
        self.load_validated_with_cancel(project_id, || false)
    }

    fn load_validated_with_cancel<C>(
        &self,
        project_id: &str,
        is_cancelled: C,
    ) -> Result<ValidatedProject, String>
    where
        C: Fn() -> bool + Send + Sync + Clone + 'static,
    {
        ensure_project_open_not_cancelled(is_cancelled())?;
        self.ensure_initialized()?;
        validate_id(project_id)?;
        let connection = self.connection()?;
        let stored = self
            .stored_project(&connection, project_id)?
            .ok_or_else(|| "El proyecto solicitado no existe.".to_owned())?;
        self.validate_stored_project_with_cancel(stored, is_cancelled)
    }

    fn validate_stored_project_with_cancel<C>(
        &self,
        mut stored: StoredProject,
        is_cancelled: C,
    ) -> Result<ValidatedProject, String>
    where
        C: Fn() -> bool + Send + Sync + Clone + 'static,
    {
        ensure_project_open_not_cancelled(is_cancelled())?;
        stored.summary.storage_bytes = self.stored_project_storage_bytes(&stored).ok();
        ensure_project_open_not_cancelled(is_cancelled())?;
        let workspace = decode_workspace(&stored)?;
        let mut profile = decode_profile(&stored)?;
        ensure_project_open_not_cancelled(is_cancelled())?;
        let candidate = if let Some(generation_name) = stored.generation_name.as_deref() {
            let generation = self.generation_path(&stored.summary.id, generation_name)?;
            let current = self.generation_file(&generation, "current.parquet")?;
            if profile.is_some()
                && !profile_cache_matches_with_cancel(&stored, &current, || is_cancelled())?
            {
                // A cache from a different generation is not fatal; Review can
                // recompute the derived profile for the verified dataset.
                profile = None;
            }
            ensure_project_open_not_cancelled(is_cancelled())?;
            let history = self.decode_history(&stored, &generation)?;
            DatasetState::prepare_durable_project_candidate_with_cancel(
                current,
                stored.summary.dataset_file_name.clone(),
                profile.clone(),
                Some(history),
                Arc::new(is_cancelled.clone()),
            )?
        } else {
            if stored.history_manifest_json.is_some()
                || profile.is_some()
                || stored.profile_cache_sha256.is_some()
            {
                return Err("El estado persistente del proyecto no es consistente.".to_owned());
            }
            let snapshot_path = self.snapshot_path(&stored.summary.id, &stored.snapshot_name)?;
            DatasetState::prepare_durable_project_candidate_with_cancel(
                snapshot_path,
                stored.summary.dataset_file_name.clone(),
                None,
                None,
                Arc::new(is_cancelled.clone()),
            )?
        };
        ensure_project_open_not_cancelled(is_cancelled())?;
        validate_project_workspace(
            candidate.frame(),
            &workspace.quality_rules,
            workspace.recipe_draft.as_ref(),
        )
        .map_err(|_| "La configuración guardada del proyecto no es válida.".to_owned())?;
        if candidate.dimensions() != (stored.summary.row_count, stored.summary.column_count) {
            return Err("El snapshot del proyecto no coincide con su catálogo.".to_owned());
        }
        ensure_project_open_not_cancelled(is_cancelled())?;
        Ok(ValidatedProject {
            stored,
            workspace,
            profile,
            candidate,
        })
    }

    #[cfg(test)]
    fn open(
        &self,
        dataset_state: &DatasetState,
        project_id: String,
    ) -> Result<ProjectOpenResult, String> {
        self.open_with_cancellation(dataset_state, project_id, None)
    }

    fn open_cancellable(
        &self,
        dataset_state: &DatasetState,
        project_id: String,
        generation: u64,
    ) -> Result<ProjectOpenResult, String> {
        self.open_with_cancellation(dataset_state, project_id, Some(generation))
    }

    fn open_with_cancellation(
        &self,
        dataset_state: &DatasetState,
        project_id: String,
        generation: Option<u64>,
    ) -> Result<ProjectOpenResult, String> {
        let is_cancelled: Arc<dyn Fn() -> bool + Send + Sync> = generation
            .map(|generation| dataset_state.project_open_cancellation(generation))
            .unwrap_or_else(|| Arc::new(|| false));
        let cancellation = Arc::clone(&is_cancelled);
        let validated = self.load_validated_with_cancel(&project_id, move || cancellation())?;
        let publish = move || {
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
        };
        match generation {
            Some(generation) => dataset_state.commit_project_open(generation, publish),
            None => publish(),
        }
    }

    fn delete(&self, project_id: String) -> Result<(), String> {
        self.delete_with_cancellation(project_id, None, || false)
    }

    fn delete_cancellable(
        &self,
        dataset_state: &DatasetState,
        project_id: String,
        generation: u64,
    ) -> Result<(), String> {
        let cancellation = dataset_state.project_delete_cancellation(generation);
        self.delete_with_cancellation(project_id, Some((dataset_state, generation)), move || {
            cancellation()
        })
    }

    fn delete_with_cancellation<C>(
        &self,
        project_id: String,
        dataset_state: Option<(&DatasetState, u64)>,
        is_cancelled: C,
    ) -> Result<(), String>
    where
        C: Fn() -> bool + Sync,
    {
        ensure_project_operation_not_cancelled(is_cancelled())?;
        self.ensure_initialized()?;
        validate_id(&project_id)?;
        let mut connection = self.connection()?;
        let stored = self
            .stored_project(&connection, &project_id)?
            .ok_or_else(|| "El proyecto solicitado no existe.".to_owned())?;
        let mut generations = HashSet::new();
        let mut snapshots = HashSet::new();
        collect_project_storage_paths_with_cancel(
            self,
            &stored,
            &mut generations,
            &mut snapshots,
            &is_cancelled,
        )?;
        ensure_project_operation_not_cancelled(is_cancelled())?;
        let mut statement = connection
            .prepare("SELECT payload_json FROM project_versions WHERE project_id = ?1")
            .map_err(|_| storage_error())?;
        let backups = statement
            .query_map(params![project_id], |row| row.get::<_, String>(0))
            .map_err(|_| storage_error())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| storage_error())?;
        drop(statement);
        for payload in backups {
            ensure_project_operation_not_cancelled(is_cancelled())?;
            let backup = decode_project_version(&payload)?;
            if backup.summary.id != project_id {
                return Err(storage_error());
            }
            collect_project_storage_paths_with_cancel(
                self,
                &backup,
                &mut generations,
                &mut snapshots,
                &is_cancelled,
            )?;
        }
        ensure_project_operation_not_cancelled(is_cancelled())?;
        let delete_catalog = || {
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
            // Once the catalog commit wins, finish cleanup under the same gate
            // so a late cancel cannot be acknowledged while deletion continues.
            for name in generations {
                let path = self.generation_path(&project_id, &name)?;
                match fs::remove_dir_all(path) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(_) => {
                        return Err(
                            "El proyecto se eliminó, pero no se pudo limpiar una versión."
                                .to_owned(),
                        )
                    }
                }
            }
            for name in &snapshots {
                let path = self.managed_snapshot_path(&project_id, name)?;
                if path.exists() {
                    reject_link_or_reparse(&path)?;
                }
            }
            for name in snapshots {
                let path = self.managed_snapshot_path(&project_id, &name)?;
                match fs::remove_file(path) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(_) => {
                        return Err(
                            "El proyecto se eliminó, pero no se pudo limpiar un snapshot."
                                .to_owned(),
                        )
                    }
                }
            }
            Ok(())
        };
        match dataset_state {
            Some((dataset_state, generation)) => {
                dataset_state.commit_project_delete(generation, delete_catalog)?;
            }
            None => delete_catalog()?,
        }
        Ok(())
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
                        generation_name, history_manifest_json, profile_json,
                         profile_cache_sha256, sql_history_json, review_tab, preview_offset,
                         active_phase, query_engine, analysis_sample_rows, performance_profile,
                         export_format, privacy_mode, comparison_key_columns_json, join_type,
                         import_profile_json
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
                        profile_cache_sha256: row.get(13)?,
                        sql_history_json: row.get(14)?,
                        review_tab: row.get(15)?,
                        preview_offset: row.get(16)?,
                        active_phase: row.get(17)?,
                        query_engine: row.get(18)?,
                        analysis_sample_rows: row.get(19)?,
                        performance_profile: row.get(20)?,
                        export_format: row.get(21)?,
                        privacy_mode: row.get(22)?,
                        comparison_key_columns_json: row.get(23)?,
                        join_type: row.get(24)?,
                        import_profile_json: row.get(25)?,
                    })
                },
            )
            .optional()
            .map_err(|_| storage_error())
    }

    fn with_storage_usage(
        &self,
        connection: &Connection,
        summary: ProjectSummary,
    ) -> Result<ProjectSummary, String> {
        self.with_storage_usage_with_cancel(connection, summary, &|| false)
    }

    fn with_storage_usage_with_cancel<C>(
        &self,
        connection: &Connection,
        mut summary: ProjectSummary,
        is_cancelled: &C,
    ) -> Result<ProjectSummary, String>
    where
        C: Fn() -> bool + Sync,
    {
        ensure_project_operation_not_cancelled(is_cancelled())?;
        let stored = self
            .stored_project(connection, &summary.id)?
            .ok_or_else(|| "El proyecto solicitado no existe.".to_owned())?;
        summary.storage_bytes =
            match self.stored_project_storage_bytes_with_cancel(&stored, is_cancelled) {
                Ok(bytes) => Some(bytes),
                Err(error) if error == OPERATION_CANCELLED_MESSAGE => return Err(error),
                Err(_) => None,
            };
        ensure_project_operation_not_cancelled(is_cancelled())?;
        Ok(summary)
    }

    fn stored_project_storage_bytes(&self, stored: &StoredProject) -> Result<u64, String> {
        self.stored_project_storage_bytes_with_cancel(stored, &|| false)
    }

    fn stored_project_storage_bytes_with_cancel<C>(
        &self,
        stored: &StoredProject,
        is_cancelled: &C,
    ) -> Result<u64, String>
    where
        C: Fn() -> bool + Sync,
    {
        ensure_project_operation_not_cancelled(is_cancelled())?;
        if let Some(generation_name) = stored.generation_name.as_deref() {
            let generation = self.generation_path(&stored.summary.id, generation_name)?;
            let history = stored
                .history_manifest_json
                .as_deref()
                .map(|encoded| {
                    serde_json::from_str::<DurableHistoryManifest>(encoded)
                        .map_err(|_| "El manifiesto del historial no es válido.".to_owned())
                })
                .transpose()?;
            return self.generation_storage_bytes_with_cancel(
                &generation,
                history.as_ref(),
                is_cancelled,
            );
        }

        let snapshot = self.snapshot_path(&stored.summary.id, &stored.snapshot_name)?;
        let bytes = fs::metadata(snapshot)
            .map(|metadata| metadata.len())
            .map_err(|_| storage_error())?;
        ensure_project_operation_not_cancelled(is_cancelled())?;
        Ok(bytes)
    }

    fn generation_storage_bytes(
        &self,
        generation: &Path,
        history: Option<&DurableHistoryManifest>,
    ) -> Result<u64, String> {
        self.generation_storage_bytes_with_cancel(generation, history, &|| false)
    }

    fn generation_storage_bytes_with_cancel<C>(
        &self,
        generation: &Path,
        history: Option<&DurableHistoryManifest>,
        is_cancelled: &C,
    ) -> Result<u64, String>
    where
        C: Fn() -> bool + Sync,
    {
        ensure_project_operation_not_cancelled(is_cancelled())?;
        let current = self.generation_file(generation, "current.parquet")?;
        let mut total = fs::metadata(current).map_err(|_| storage_error())?.len();
        if let Some(history) = history {
            if history.version != 1 {
                return Err("La versión del manifiesto del historial no es compatible.".to_owned());
            }
            for (index, entry) in history.entries.iter().enumerate() {
                ensure_project_operation_not_cancelled(is_cancelled())?;
                if entry.file_name != format!("history-{index:03}.parquet") {
                    return Err("El manifiesto del historial no tiene un orden válido.".to_owned());
                }
                let snapshot = self.generation_file(generation, &entry.file_name)?;
                let bytes = fs::metadata(snapshot).map_err(|_| storage_error())?.len();
                if bytes != entry.bytes {
                    return Err(
                        "El historial del proyecto no coincide con su manifiesto.".to_owned()
                    );
                }
                total = total.checked_add(bytes).ok_or_else(storage_error)?;
            }
        }
        ensure_project_operation_not_cancelled(is_cancelled())?;
        Ok(total)
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
                    id: entry.id,
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
    let source_backed = validated.candidate.is_source_backed();
    let candidate = validated.candidate;
    let (frame, dataset_state) = if source_backed {
        (DataFrame::empty(), Some(candidate.into_dataset_state()))
    } else {
        (candidate.into_frame(), None)
    };
    Ok(AutomationOpenedProject {
        frame,
        dataset_state,
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
        storage_bytes: None,
    })
}

fn insert_project_version(
    transaction: &rusqlite::Transaction<'_>,
    stored: &StoredProject,
    storage_bytes: u64,
) -> Result<i64, String> {
    let payload_json = serde_json::to_string(&StoredProjectVersion {
        format_version: PROJECT_VERSION_FORMAT,
        project: stored.clone(),
    })
    .map_err(|_| storage_error())?;
    transaction
        .execute(
            "INSERT INTO project_versions
             (project_id, created_at, payload_json, generation_name, snapshot_name, storage_bytes)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                stored.summary.id,
                stored.summary.updated_at,
                payload_json,
                stored.generation_name,
                stored.snapshot_name,
                i64::try_from(storage_bytes).unwrap_or(i64::MAX)
            ],
        )
        .map_err(|_| storage_error())?;
    Ok(transaction.last_insert_rowid())
}

fn decode_project_version(payload_json: &str) -> Result<StoredProject, String> {
    let payload: StoredProjectVersion =
        serde_json::from_str(payload_json).map_err(|_| storage_error())?;
    if payload.format_version != PROJECT_VERSION_FORMAT {
        return Err("La versión del respaldo del proyecto no es compatible.".to_owned());
    }
    Ok(payload.project)
}

fn collect_project_storage_paths_with_cancel<C>(
    store: &ProjectStore,
    stored: &StoredProject,
    generations: &mut HashSet<String>,
    snapshots: &mut HashSet<String>,
    is_cancelled: &C,
) -> Result<(), String>
where
    C: Fn() -> bool + Sync,
{
    ensure_project_operation_not_cancelled(is_cancelled())?;
    if let Some(name) = stored.generation_name.as_deref() {
        // Resolve now to reject unsafe names or reparse points before deleting the catalog row.
        store.generation_path(&stored.summary.id, name)?;
        generations.insert(name.to_owned());
    } else {
        let path = store.managed_snapshot_path(&stored.summary.id, &stored.snapshot_name)?;
        if path.exists() {
            reject_link_or_reparse(&path)?;
        }
        snapshots.insert(stored.snapshot_name.clone());
    }
    ensure_project_operation_not_cancelled(is_cancelled())?;
    Ok(())
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
    let sql_history: Vec<SqlQueryHistoryEntry> = serde_json::from_str(&stored.sql_history_json)
        .map_err(|_| "La actividad SQL guardada del proyecto no es válida.".to_owned())?;
    validate_sql_query_history(&sql_history)?;
    let review_tab = parse_review_tab(&stored.review_tab)?;
    let preview_offset = parse_preview_offset(stored.preview_offset, stored.summary.row_count);
    let active_phase = parse_active_phase(&stored.active_phase)?;
    let query_engine = parse_query_engine(stored.query_engine.clone())?;
    let analysis_sample_rows = parse_analysis_sample_rows(stored.analysis_sample_rows)?;
    let performance_profile = parse_performance_profile(stored.performance_profile.clone())?;
    let export_format = parse_export_format(stored.export_format.clone())?;
    let privacy_mode = parse_privacy_mode(stored.privacy_mode.clone())?;
    let comparison_key_columns = parse_comparison_key_columns(&stored.comparison_key_columns_json)?;
    let join_type = parse_join_type(stored.join_type.clone())?;
    let import_profile = stored
        .import_profile_json
        .as_deref()
        .map(serde_json::from_str)
        .transpose()
        .map_err(|_| "El perfil de importación guardado no es válido.".to_owned())?;
    if let Some(profile) = import_profile.as_ref() {
        validate_import_profile(profile)
            .map_err(|_| "El perfil de importación guardado no es válido.".to_owned())?;
    }
    Ok(ProjectWorkspace {
        quality_rules,
        recipe_draft,
        sql_history,
        review_tab,
        preview_offset,
        active_phase,
        query_engine,
        analysis_sample_rows,
        performance_profile,
        export_format,
        privacy_mode,
        comparison_key_columns,
        join_type,
        import_profile,
    })
}

fn review_tab_label(tab: Option<&str>) -> Result<&'static str, String> {
    match tab {
        Some("preview") => Ok("preview"),
        Some("diagnosis") | None => Ok("diagnosis"),
        Some(_) => Err("La vista guardada del proyecto no es válida.".to_owned()),
    }
}

fn parse_review_tab(value: &str) -> Result<Option<String>, String> {
    let label = review_tab_label(Some(value))?;
    Ok((label == "preview").then(|| label.to_owned()))
}

fn active_phase_label(phase: Option<&str>) -> Result<&'static str, String> {
    match phase {
        Some("load") => Ok("load"),
        Some("prepare") => Ok("prepare"),
        Some("deliver") => Ok("deliver"),
        Some("review") | None => Ok("review"),
        Some(_) => Err("La etapa guardada del proyecto no es válida.".to_owned()),
    }
}

fn parse_active_phase(value: &str) -> Result<Option<String>, String> {
    let label = active_phase_label(Some(value))?;
    Ok((label != "review").then(|| label.to_owned()))
}

fn validate_query_engine(value: Option<&str>) -> Result<Option<&str>, String> {
    match value {
        None => Ok(None),
        Some("polars" | "duckdb") => Ok(value),
        Some(_) => Err("El motor de consulta guardado no es válido.".to_owned()),
    }
}

fn parse_query_engine(value: Option<String>) -> Result<Option<String>, String> {
    validate_query_engine(value.as_deref())?;
    Ok(value)
}

fn validate_analysis_sample_rows(value: Option<usize>) -> Result<Option<usize>, String> {
    match value {
        None => Ok(None),
        Some(value) if matches!(value, 10_000 | 50_000 | 100_000) => Ok(Some(value)),
        Some(_) => Err("La cobertura de correlaciones guardada no es válida.".to_owned()),
    }
}

fn parse_analysis_sample_rows(value: Option<i64>) -> Result<Option<usize>, String> {
    value
        .map(|value| {
            usize::try_from(value)
                .map_err(|_| "La cobertura de correlaciones guardada no es válida.".to_owned())
        })
        .transpose()
        .and_then(validate_analysis_sample_rows)
}

fn validate_performance_profile(value: Option<&str>) -> Result<Option<&str>, String> {
    match value {
        None => Ok(None),
        Some("conservative" | "balanced" | "maximum") => Ok(value),
        Some(_) => Err("El perfil de rendimiento guardado no es válido.".to_owned()),
    }
}

fn parse_performance_profile(value: Option<String>) -> Result<Option<String>, String> {
    validate_performance_profile(value.as_deref())?;
    Ok(value)
}

fn validate_export_format(value: Option<&str>) -> Result<Option<&str>, String> {
    match value {
        None => Ok(None),
        Some("csv" | "json" | "parquet" | "sql" | "excel" | "sqlite" | "bundle") => Ok(value),
        Some(_) => Err("El formato de exportación guardado no es válido.".to_owned()),
    }
}

fn parse_export_format(value: Option<String>) -> Result<Option<String>, String> {
    validate_export_format(value.as_deref())?;
    Ok(value)
}

fn validate_privacy_mode(value: Option<&str>) -> Result<Option<&str>, String> {
    match value {
        None => Ok(None),
        Some("none" | "mask" | "hash") => Ok(value),
        Some(_) => Err("La protección de datos guardada no es válida.".to_owned()),
    }
}

fn parse_privacy_mode(value: Option<String>) -> Result<Option<String>, String> {
    validate_privacy_mode(value.as_deref())?;
    Ok(value)
}

fn validate_join_type(value: Option<&str>) -> Result<Option<&str>, String> {
    match value {
        None => Ok(None),
        Some("inner" | "left" | "full") => Ok(value),
        Some(_) => Err("El tipo de JOIN guardado no es válido.".to_owned()),
    }
}

fn parse_join_type(value: Option<String>) -> Result<Option<String>, String> {
    validate_join_type(value.as_deref())?;
    Ok(value)
}

fn validate_comparison_key_columns_shape(columns: &[String]) -> Result<(), String> {
    if columns.len() > MAX_COMPARISON_KEY_COLUMNS {
        return Err(format!(
            "La comparación admite como máximo {MAX_COMPARISON_KEY_COLUMNS} columnas clave."
        ));
    }
    let mut seen = HashSet::with_capacity(columns.len());
    for column in columns {
        if column.is_empty() || column.chars().count() > MAX_COMPARISON_KEY_COLUMN_CHARS {
            return Err("Una columna clave guardada no es válida.".to_owned());
        }
        if !seen.insert(column) {
            return Err("Las columnas clave guardadas no pueden repetirse.".to_owned());
        }
    }
    Ok(())
}

fn validate_comparison_key_columns_for_frame(
    frame: &DataFrame,
    columns: &[String],
) -> Result<Vec<String>, String> {
    validate_comparison_key_columns_shape(columns)?;
    for column in columns {
        if !frame
            .get_column_names()
            .iter()
            .any(|name| name.as_str() == column)
        {
            return Err(format!(
                "La columna clave '{column}' no existe en el dataset activo."
            ));
        }
    }
    Ok(columns.to_vec())
}

fn parse_comparison_key_columns(value: &str) -> Result<Vec<String>, String> {
    let columns: Vec<String> = serde_json::from_str(value)
        .map_err(|_| "Las columnas clave guardadas no son válidas.".to_owned())?;
    validate_comparison_key_columns_shape(&columns)?;
    Ok(columns)
}

fn validate_preview_offset(offset: Option<usize>, row_count: usize) -> Result<usize, String> {
    let offset = offset.unwrap_or(0);
    if offset == 0 {
        return Ok(0);
    }
    if row_count == 0 || !offset.is_multiple_of(PREVIEW_PAGE_SIZE) || offset >= row_count {
        return Err("La página guardada de la vista previa no es válida.".to_owned());
    }
    Ok(offset)
}

fn parse_preview_offset(value: i64, row_count: usize) -> Option<usize> {
    let offset = usize::try_from(value).ok()?;
    validate_preview_offset(Some(offset), row_count)
        .ok()
        .filter(|offset| *offset > 0)
}

fn validate_sql_query_history(entries: &[SqlQueryHistoryEntry]) -> Result<(), String> {
    if entries.len() > MAX_SQL_QUERY_HISTORY_ENTRIES {
        return Err(format!(
            "La actividad SQL del proyecto supera el límite de {MAX_SQL_QUERY_HISTORY_ENTRIES} entradas."
        ));
    }

    let mut ids = HashSet::with_capacity(entries.len());
    for entry in entries {
        if entry.id == 0 || !ids.insert(entry.id) {
            return Err(
                "La actividad SQL del proyecto contiene identificadores no válidos.".to_owned(),
            );
        }
        if !matches!(entry.outcome.as_str(), "success" | "error" | "cancelled") {
            return Err("La actividad SQL del proyecto contiene un estado no válido.".to_owned());
        }
        if entry.duration_ms > MAX_SQL_QUERY_DURATION_MS {
            return Err("La duración de una consulta SQL guardada no es válida.".to_owned());
        }
    }
    Ok(())
}

fn decode_profile(stored: &StoredProject) -> Result<Option<DatasetProfile>, String> {
    stored
        .profile_json
        .as_deref()
        .map(serde_json::from_str)
        .transpose()
        .map_err(|_| "El perfil guardado del proyecto no es válido.".to_owned())
}

fn profile_cache_matches_with_cancel<C>(
    stored: &StoredProject,
    current: &Path,
    is_cancelled: C,
) -> Result<bool, String>
where
    C: Fn() -> bool + Sync,
{
    ensure_project_open_not_cancelled(is_cancelled())?;
    let Some(expected) = stored.profile_cache_sha256.as_deref() else {
        // Catalogs created before the cache fingerprint existed remain
        // readable; the next save upgrades them to the verified contract.
        return Ok(true);
    };
    if expected.len() != 64 || !expected.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Ok(false);
    }
    let actual = hash_file_sha256_with_cancel(current, || is_cancelled())?;
    ensure_project_open_not_cancelled(is_cancelled())?;
    Ok(actual.eq_ignore_ascii_case(expected))
}

fn hash_file_sha256(path: &Path) -> Result<String, String> {
    hash_file_sha256_with_cancel(path, || false)
}

fn hash_file_sha256_with_cancel<C>(path: &Path, is_cancelled: C) -> Result<String, String>
where
    C: Fn() -> bool,
{
    ensure_project_open_not_cancelled(is_cancelled())?;
    let mut file = fs::File::open(path).map_err(|_| storage_error())?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        ensure_project_open_not_cancelled(is_cancelled())?;
        let read = file.read(&mut buffer).map_err(|_| storage_error())?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    ensure_project_open_not_cancelled(is_cancelled())?;
    Ok(hex::encode(hasher.finalize()))
}

fn ensure_project_open_not_cancelled(is_cancelled: bool) -> Result<(), String> {
    if is_cancelled {
        Err(OPERATION_CANCELLED_MESSAGE.to_owned())
    } else {
        Ok(())
    }
}

fn ensure_project_save_not_cancelled(is_cancelled: bool) -> Result<(), String> {
    ensure_project_open_not_cancelled(is_cancelled)
}

fn ensure_project_operation_not_cancelled(is_cancelled: bool) -> Result<(), String> {
    ensure_project_open_not_cancelled(is_cancelled)
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
                id: entry.id.clone(),
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

#[cfg(test)]
fn write_snapshot(frame: &DataFrame, destination: &Path) -> Result<(), String> {
    write_snapshot_owned(frame.clone(), destination)
}

fn write_snapshot_owned(frame: DataFrame, destination: &Path) -> Result<(), String> {
    write_snapshot_owned_with_cancel(frame, destination, || false)
}

fn write_snapshot_owned_with_cancel<C>(
    mut frame: DataFrame,
    destination: &Path,
    is_cancelled: C,
) -> Result<(), String>
where
    C: Fn() -> bool + Sync,
{
    ensure_project_operation_not_cancelled(is_cancelled())?;
    if destination.exists() {
        return Err(storage_error());
    }
    let parent = destination.parent().ok_or_else(storage_error)?;
    let temporary = tempfile::NamedTempFile::new_in(parent).map_err(|_| storage_error())?;
    ParquetWriter::new(temporary.as_file())
        .finish(&mut frame)
        .map_err(|_| storage_error())?;
    ensure_project_operation_not_cancelled(is_cancelled())?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|_| storage_error())?;
    ensure_project_operation_not_cancelled(is_cancelled())?;
    temporary
        .persist_noclobber(destination)
        .map_err(|_| storage_error())?;
    ensure_project_operation_not_cancelled(is_cancelled())?;
    if sync_directory(parent).is_err() {
        let _ = fs::remove_file(destination);
        return Err(storage_error());
    }
    Ok(())
}

fn write_generation(
    frame: DataFrame,
    current_snapshot_path: Option<&Path>,
    history: &ProjectHistoryCapture,
    destination: &Path,
) -> Result<(), String> {
    write_generation_with_cancel(frame, current_snapshot_path, history, destination, || false)
}

fn write_generation_with_cancel<C>(
    frame: DataFrame,
    current_snapshot_path: Option<&Path>,
    history: &ProjectHistoryCapture,
    destination: &Path,
    is_cancelled: C,
) -> Result<(), String>
where
    C: Fn() -> bool + Clone + Sync,
{
    ensure_project_operation_not_cancelled(is_cancelled())?;
    if destination.exists() {
        return Err(storage_error());
    }
    let parent = destination.parent().ok_or_else(storage_error)?;
    let staging = tempfile::tempdir_in(parent).map_err(|_| storage_error())?;
    let current_target = staging.path().join("current.parquet");
    if let Some(source) = current_snapshot_path {
        copy_snapshot_with_cancel(source, &current_target, is_cancelled.clone())?;
    } else {
        write_snapshot_owned_with_cancel(frame, &current_target, is_cancelled.clone())?;
    }
    for (index, entry) in history.entries.iter().enumerate() {
        ensure_project_operation_not_cancelled(is_cancelled())?;
        let target = staging.path().join(format!("history-{index:03}.parquet"));
        let mut destination_file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
            .map_err(|_| storage_error())?;
        let copied = crate::dataset::copy_file_with_cancel(
            &entry.path,
            &mut destination_file,
            is_cancelled.clone(),
        )?;
        if copied != entry.bytes {
            return Err(storage_error());
        }
        destination_file.sync_all().map_err(|_| storage_error())?;
        ensure_project_operation_not_cancelled(is_cancelled())?;
    }
    ensure_project_operation_not_cancelled(is_cancelled())?;
    sync_directory(staging.path())?;
    ensure_project_operation_not_cancelled(is_cancelled())?;
    let staging_path = staging.keep();
    if fs::rename(&staging_path, destination).is_err() {
        let _ = fs::remove_dir_all(staging_path);
        return Err(storage_error());
    }
    if sync_directory(parent).is_err() {
        let _ = fs::remove_dir_all(destination);
        return Err(storage_error());
    }
    ensure_project_operation_not_cancelled(is_cancelled())?;
    Ok(())
}

fn copy_snapshot(source: &Path, destination: &Path) -> Result<(), String> {
    copy_snapshot_with_cancel(source, destination, || false)
}

fn copy_snapshot_with_cancel<C>(
    source: &Path,
    destination: &Path,
    is_cancelled: C,
) -> Result<(), String>
where
    C: Fn() -> bool + Clone,
{
    ensure_project_operation_not_cancelled(is_cancelled())?;
    let metadata = fs::symlink_metadata(source).map_err(|_| storage_error())?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
        return Err(storage_error());
    }
    if destination.exists() {
        return Err(storage_error());
    }
    let mut destination_file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(|_| storage_error())?;
    let copied =
        crate::dataset::copy_file_with_cancel(source, &mut destination_file, is_cancelled.clone())
            .map_err(|error| {
                if error == OPERATION_CANCELLED_MESSAGE {
                    error
                } else {
                    storage_error()
                }
            })?;
    if copied != metadata.len() {
        return Err(storage_error());
    }
    destination_file.sync_all().map_err(|_| storage_error())?;
    ensure_project_operation_not_cancelled(is_cancelled())?;
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
pub async fn list_projects(app: AppHandle) -> Result<ProjectCatalogSnapshot, String> {
    let generation = app.state::<DatasetState>().begin_project_catalog()?;
    let cancellation = app
        .state::<DatasetState>()
        .project_catalog_cancellation(generation);
    run_project_operation(app, move |store, _| {
        store.list_catalog_with_cancel(move || cancellation())
    })
    .await
}

#[tauri::command]
pub async fn list_project_versions(
    app: AppHandle,
    project_id: String,
) -> Result<Vec<ProjectVersionSummary>, String> {
    run_project_operation(app, move |store, _| store.list_versions(&project_id)).await
}

#[tauri::command]
pub async fn save_project(
    app: AppHandle,
    project_id: Option<String>,
    name: String,
    workspace: ProjectWorkspace,
) -> Result<ProjectSummary, String> {
    let generation = app.state::<DatasetState>().begin_project_save()?;
    run_project_operation(app, move |store, dataset| {
        store.save_cancellable(dataset, project_id, name, workspace, generation)
    })
    .await
}

#[tauri::command]
pub async fn autosave_project(
    app: AppHandle,
    project_id: String,
    name: String,
    workspace: ProjectWorkspace,
) -> Result<ProjectSummary, String> {
    let generation = app.state::<DatasetState>().begin_project_save()?;
    run_project_operation(app, move |store, dataset| {
        store.autosave_cancellable(dataset, project_id, name, workspace, generation)
    })
    .await
}

#[tauri::command]
pub async fn restore_project_version(
    app: AppHandle,
    project_id: String,
    version_id: i64,
) -> Result<ProjectOpenResult, String> {
    let generation = app.state::<DatasetState>().begin_project_open()?;
    run_project_operation(app, move |store, dataset| {
        store.restore_version_cancellable(dataset, &project_id, version_id, generation)
    })
    .await
}

#[tauri::command]
pub async fn open_project(app: AppHandle, project_id: String) -> Result<ProjectOpenResult, String> {
    let generation = app.state::<DatasetState>().begin_project_open()?;
    run_project_operation(app, move |store, dataset| {
        store.open_cancellable(dataset, project_id, generation)
    })
    .await
}

#[tauri::command]
pub async fn delete_project(app: AppHandle, project_id: String) -> Result<(), String> {
    let generation = app.state::<DatasetState>().begin_project_delete()?;
    run_project_operation(app, move |store, dataset| {
        store.delete_cancellable(dataset, project_id, generation)
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

// Los proyectos v4 persisten el historial y la actividad SQL agregada, pero cada apertura lo copia a un TempDir nuevo:
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
            },
            "sqlHistory": [{
                "id": 7,
                "outcome": "success",
                "durationMs": 42,
                "rowCount": 2
            }],
            "reviewTab": "preview",
            "activePhase": "prepare",
            "queryEngine": "duckdb",
            "analysisSampleRows": 50000,
            "performanceProfile": "maximum",
            "exportFormat": "parquet",
            "privacyMode": "mask",
            "comparisonKeyColumns": ["value"],
            "joinType": "full"
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
    fn generation_copies_a_streamed_current_snapshot_without_reencoding() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("current-source.parquet");
        let destination = directory.path().join("generation");
        write_snapshot(&frame(&[7, 8]), &source).expect("se debe escribir la fuente Parquet");
        let (state, active_source) = active_state(directory.path(), &[7, 8], "input.parquet");
        let active = state
            .active_project_snapshot()
            .expect("se debe capturar el dataset activo");

        write_generation(
            DataFrame::empty(),
            Some(&source),
            &active.history,
            &destination,
        )
        .expect("la generación debe copiar el snapshot source-backed");

        let copied = fs::read(destination.join("current.parquet"))
            .expect("el snapshot copiado debe existir");
        let original = fs::read(source).expect("la fuente debe seguir disponible");
        assert_eq!(copied, original);
        fs::remove_file(active_source).unwrap();
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
        ProjectStore::initialize(root).expect("reabrir v8 debe ser idempotente");
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
        assert!(columns.contains(&"profile_cache_sha256".to_owned()));
        assert!(columns.contains(&"sql_history_json".to_owned()));
        assert!(columns.contains(&"review_tab".to_owned()));
        assert!(columns.contains(&"preview_offset".to_owned()));
        assert!(columns.contains(&"active_phase".to_owned()));
        assert!(columns.contains(&"query_engine".to_owned()));
        assert!(columns.contains(&"analysis_sample_rows".to_owned()));
        assert!(columns.contains(&"performance_profile".to_owned()));
    }

    #[test]
    fn migration_from_v6_adds_session_fields_without_requiring_dataset_data() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("data-v6");
        fs::create_dir_all(&root).unwrap();
        Connection::open(root.join("projects.sqlite3"))
            .unwrap()
            .execute_batch(
                "CREATE TABLE projects (id TEXT PRIMARY KEY NOT NULL);
                 PRAGMA user_version = 6;",
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
        assert!(columns.contains(&"preview_offset".to_owned()));
        assert!(columns.contains(&"active_phase".to_owned()));
        assert!(columns.contains(&"query_engine".to_owned()));
        assert!(columns.contains(&"analysis_sample_rows".to_owned()));
        assert!(columns.contains(&"performance_profile".to_owned()));
    }

    #[test]
    fn migration_from_v7_adds_active_phase_without_requiring_dataset_data() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("data-v7");
        fs::create_dir_all(&root).unwrap();
        Connection::open(root.join("projects.sqlite3"))
            .unwrap()
            .execute_batch(
                "CREATE TABLE projects (id TEXT PRIMARY KEY NOT NULL);
                 PRAGMA user_version = 7;",
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
        assert!(columns.contains(&"active_phase".to_owned()));
        assert!(columns.contains(&"query_engine".to_owned()));
        assert!(columns.contains(&"analysis_sample_rows".to_owned()));
        assert!(columns.contains(&"performance_profile".to_owned()));
    }

    #[test]
    fn migration_from_v8_adds_analysis_sample_rows_without_requiring_dataset_data() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("data-v8");
        fs::create_dir_all(&root).unwrap();
        Connection::open(root.join("projects.sqlite3"))
            .unwrap()
            .execute_batch(
                "CREATE TABLE projects (id TEXT PRIMARY KEY NOT NULL);
                 PRAGMA user_version = 8;",
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
        assert!(columns.contains(&"analysis_sample_rows".to_owned()));
        assert!(columns.contains(&"query_engine".to_owned()));
        assert!(columns.contains(&"performance_profile".to_owned()));
    }

    #[test]
    fn migration_from_v9_adds_query_engine_without_requiring_dataset_data() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("data-v9");
        fs::create_dir_all(&root).unwrap();
        Connection::open(root.join("projects.sqlite3"))
            .unwrap()
            .execute_batch(
                "CREATE TABLE projects (id TEXT PRIMARY KEY NOT NULL);
                 PRAGMA user_version = 9;",
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
        assert!(columns.contains(&"query_engine".to_owned()));
        assert!(columns.contains(&"performance_profile".to_owned()));
    }

    #[test]
    fn migration_fails_closed_for_future_catalog_versions_without_leaking_paths() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("future-private-data");
        fs::create_dir_all(&root).unwrap();
        Connection::open(root.join("projects.sqlite3"))
            .unwrap()
            .execute_batch("PRAGMA user_version = 15;")
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
        assert_eq!(version, SCHEMA_VERSION + 1);
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
    fn migration_from_v10_adds_performance_profile_without_requiring_dataset_data() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("data-v10");
        fs::create_dir_all(&root).unwrap();
        Connection::open(root.join("projects.sqlite3"))
            .unwrap()
            .execute_batch(
                "CREATE TABLE projects (id TEXT PRIMARY KEY NOT NULL);
                 PRAGMA user_version = 10;",
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
        assert!(columns.contains(&"performance_profile".to_owned()));
    }

    #[test]
    fn migration_from_v11_adds_session_preferences_without_requiring_dataset_data() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("data-v11");
        fs::create_dir_all(&root).unwrap();
        Connection::open(root.join("projects.sqlite3"))
            .unwrap()
            .execute_batch(
                "CREATE TABLE projects (id TEXT PRIMARY KEY NOT NULL);
                 PRAGMA user_version = 11;",
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
        assert!(columns.contains(&"export_format".to_owned()));
        assert!(columns.contains(&"privacy_mode".to_owned()));
        assert!(columns.contains(&"comparison_key_columns_json".to_owned()));
        assert!(columns.contains(&"join_type".to_owned()));
    }

    #[test]
    fn migration_from_v13_adds_versioned_project_backups() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("data-v13");
        fs::create_dir_all(&root).unwrap();
        Connection::open(root.join("projects.sqlite3"))
            .unwrap()
            .execute_batch(
                "CREATE TABLE projects (id TEXT PRIMARY KEY NOT NULL);
                 PRAGMA user_version = 13;",
            )
            .unwrap();

        let store = ProjectStore::initialize(root).unwrap();
        let connection = store.connection().unwrap();
        let version: i64 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION);
        let tables = connection
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table'")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert!(tables.contains(&"project_versions".to_owned()));
    }

    #[test]
    fn preview_page_offset_roundtrips_only_when_it_matches_a_real_page() {
        let directory = tempfile::tempdir().unwrap();
        let store = ProjectStore::initialize(directory.path().join("data")).unwrap();
        let values = (0..100).collect::<Vec<_>>();
        let (state, _) = active_state(directory.path(), &values, "paged.csv");
        let project = store
            .save(
                &state,
                None,
                "Dataset paginado".to_owned(),
                ProjectWorkspace {
                    preview_offset: Some(50),
                    ..ProjectWorkspace::default()
                },
            )
            .unwrap();

        let opened = store
            .open(&DatasetState::default(), project.id)
            .expect("el proyecto paginado debe reabrirse");
        assert_eq!(opened.workspace.preview_offset, Some(50));
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
        let ids_before_save = state
            .active_project_snapshot()
            .unwrap()
            .history
            .entries
            .into_iter()
            .map(|entry| entry.id)
            .collect::<Vec<_>>();
        let expected_profile = state.project_test_cache_profile().unwrap();
        let project = store
            .save(
                &state,
                None,
                "Con historia".to_owned(),
                ProjectWorkspace::default(),
            )
            .unwrap();
        assert!(project.storage_bytes.is_some_and(|bytes| bytes > 0));
        drop(store);

        let project_id = project.id.clone();
        let reopened = ProjectStore::initialize(root.clone()).unwrap();
        let restored = DatasetState::default();
        let result = reopened.open(&restored, project_id.clone()).unwrap();
        assert_eq!(result.profile, Some(expected_profile));
        assert_eq!(result.project.storage_bytes, project.storage_bytes);
        assert_eq!(
            reopened.list().unwrap().first().unwrap().storage_bytes,
            project.storage_bytes
        );
        let stored_project = reopened
            .stored_project(&reopened.connection().unwrap(), &project_id)
            .unwrap()
            .unwrap();
        let initial_generation = reopened
            .generation_path(
                &project_id,
                stored_project.generation_name.as_deref().unwrap(),
            )
            .unwrap();
        let stored_history: DurableHistoryManifest =
            serde_json::from_str(stored_project.history_manifest_json.as_deref().unwrap()).unwrap();
        let expected_storage_bytes = fs::metadata(initial_generation.join("current.parquet"))
            .unwrap()
            .len()
            + stored_history
                .entries
                .iter()
                .map(|entry| entry.bytes)
                .sum::<u64>();
        assert_eq!(project.storage_bytes, Some(expected_storage_bytes));
        let ids_after_reopen = restored
            .active_project_snapshot()
            .unwrap()
            .history
            .entries
            .into_iter()
            .map(|entry| entry.id)
            .collect::<Vec<_>>();
        assert_eq!(ids_after_reopen, ids_before_save);
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

        let old_generation_name = reopened
            .stored_project(&reopened.connection().unwrap(), &project_id)
            .unwrap()
            .unwrap()
            .generation_name
            .unwrap();
        let old_generation = reopened
            .generation_path(&project_id, &old_generation_name)
            .unwrap();
        let updated = reopened
            .save(
                &restored,
                Some(project_id.clone()),
                "Con historia actualizada".to_owned(),
                ProjectWorkspace::default(),
            )
            .unwrap();
        assert!(updated.storage_bytes.is_some_and(|bytes| bytes > 0));
        assert!(old_generation.exists());
        assert_eq!(reopened.list_versions(&project_id).unwrap().len(), 1);

        let reopened = ProjectStore::initialize(root).unwrap();
        let updated_state = DatasetState::default();
        let updated_result = reopened.open(&updated_state, project_id.clone()).unwrap();
        let updated_history_ids = updated_state
            .active_project_snapshot()
            .unwrap()
            .history
            .entries
            .into_iter()
            .map(|entry| entry.id)
            .collect::<Vec<_>>();
        assert_eq!(updated_history_ids, ids_before_save);
        assert_eq!(updated_result.project.storage_bytes, updated.storage_bytes);
        assert_eq!(
            reopened.recovery_candidate().unwrap().unwrap().id,
            project_id
        );
    }

    #[test]
    fn stale_profile_cache_is_invalidated_when_current_snapshot_changes() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("data");
        let store = ProjectStore::initialize(root).unwrap();
        let (state, _) = active_state(directory.path(), &[1, 2], "profile.csv");
        state.project_test_cache_profile().unwrap();
        state.project_test_degrade_history().unwrap();
        let project = store
            .save(
                &state,
                None,
                "Perfil verificable".to_owned(),
                ProjectWorkspace::default(),
            )
            .unwrap();

        let stored = store
            .stored_project(&store.connection().unwrap(), &project.id)
            .unwrap()
            .unwrap();
        let cache_hash = stored
            .profile_cache_sha256
            .as_deref()
            .expect("un perfil guardado debe tener huella");
        assert_eq!(cache_hash.len(), 64);
        let generation = store
            .generation_path(&project.id, stored.generation_name.as_deref().unwrap())
            .unwrap();
        let current = store
            .generation_file(&generation, "current.parquet")
            .unwrap();
        fs::remove_file(&current).unwrap();
        write_snapshot(&frame(&[9, 10]), &current).unwrap();

        let reopened = ProjectStore::initialize(directory.path().join("data")).unwrap();
        let result = reopened
            .open(&DatasetState::default(), project.id)
            .expect("un snapshot cambiado debe seguir abriendo el proyecto");
        assert!(result.profile.is_none());
        assert_eq!(result.dataset.row_count, 2);
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
        assert_eq!(snapshot_count(&store), 2, "la versión anterior se conserva");
        assert!(first_generation.exists());
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

        let restored_state = DatasetState::default();
        let reopened = store.open(&restored_state, created.id.clone()).unwrap();
        assert_eq!(reopened.project, created);
        assert_eq!(reopened.workspace, workspace());
        let reopened_snapshot = restored_state.active_project_snapshot().unwrap();
        assert!(reopened_snapshot.frame.equals_missing(&frame(&[1])));
    }

    #[test]
    fn project_updates_retain_and_restore_a_validated_version_transactionally() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("data");
        let store = ProjectStore::initialize(root.clone()).unwrap();
        let (first_state, _) = active_state(directory.path(), &[1, 2], "version-one.csv");
        let first = store
            .save(
                &first_state,
                None,
                "Versión uno".to_owned(),
                ProjectWorkspace::default(),
            )
            .unwrap();
        let first_generation = store
            .stored_project(&store.connection().unwrap(), &first.id)
            .unwrap()
            .unwrap()
            .generation_name
            .unwrap();

        let (second_state, _) = active_state(directory.path(), &[9], "version-two.csv");
        let second = store
            .save(
                &second_state,
                Some(first.id.clone()),
                "Versión dos".to_owned(),
                ProjectWorkspace::default(),
            )
            .unwrap();
        let versions = store.list_versions(&first.id).unwrap();
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0].row_count, 2);
        assert!(store
            .generation_path(&first.id, &first_generation)
            .unwrap()
            .exists());

        let restored = store.restore_version(&first.id, versions[0].id).unwrap();
        assert_eq!(restored.name, "Versión uno");
        assert_eq!(restored.row_count, 2);
        assert!(restored.updated_at >= second.updated_at);
        assert_eq!(store.list_versions(&first.id).unwrap().len(), 2);

        let active_state = DatasetState::default();
        let opened = store.open(&active_state, first.id.clone()).unwrap();
        assert_eq!(opened.project.name, "Versión uno");
        assert_eq!(opened.dataset.file_name, "version-one.csv");
        assert!(active_state
            .active_project_snapshot()
            .unwrap()
            .frame
            .equals_missing(&frame(&[1, 2])));
    }

    #[test]
    fn version_retention_obeys_count_quota_but_keeps_the_last_valid_copy() {
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
        for value in 2..=9 {
            let (updated, _) = active_state(directory.path(), &[value], "input.csv");
            store
                .autosave(
                    &updated,
                    project.id.clone(),
                    "Proyecto".to_owned(),
                    ProjectWorkspace::default(),
                )
                .unwrap();
        }
        let retained = store.list_versions(&project.id).unwrap();
        assert_eq!(retained.len(), MAX_PROJECT_VERSIONS);

        store
            .connection()
            .unwrap()
            .execute(
                "UPDATE project_versions SET storage_bytes = ?1 WHERE project_id = ?2",
                params![i64::try_from(AUTOSAVE_QUOTA_BYTES).unwrap(), project.id],
            )
            .unwrap();
        store.prune_versions(&project.id).unwrap();
        assert_eq!(store.list_versions(&project.id).unwrap().len(), 1);
        let remaining = store.list_versions(&project.id).unwrap()[0].id;
        store.restore_version(&project.id, remaining).unwrap();
        let opened = store.open(&DatasetState::default(), project.id).unwrap();
        assert_eq!(opened.dataset.file_name, "input.csv");
    }

    #[test]
    fn failed_restore_rolls_back_catalog_pointer_and_backup_insert() {
        let directory = tempfile::tempdir().unwrap();
        let store = ProjectStore::initialize(directory.path().join("data")).unwrap();
        let (first_state, _) = active_state(directory.path(), &[1, 2], "version-one.csv");
        let first = store
            .save(
                &first_state,
                None,
                "Versión uno".to_owned(),
                ProjectWorkspace::default(),
            )
            .unwrap();
        let (second_state, _) = active_state(directory.path(), &[9], "version-two.csv");
        store
            .save(
                &second_state,
                Some(first.id.clone()),
                "Versión dos".to_owned(),
                ProjectWorkspace::default(),
            )
            .unwrap();
        let version_id = store.list_versions(&first.id).unwrap()[0].id;
        store
            .connection()
            .unwrap()
            .execute_batch(
                "CREATE TRIGGER prevent_project_restore
                 BEFORE UPDATE ON projects
                 BEGIN SELECT RAISE(ABORT, 'fixture failure'); END;",
            )
            .unwrap();

        assert!(store.restore_version(&first.id, version_id).is_err());
        let active = store
            .stored_project(&store.connection().unwrap(), &first.id)
            .unwrap()
            .unwrap();
        assert_eq!(active.summary.name, "Versión dos");
        assert_eq!(store.list_versions(&first.id).unwrap().len(), 1);
        store
            .connection()
            .unwrap()
            .execute_batch("DROP TRIGGER prevent_project_restore;")
            .unwrap();
        let opened = store.open(&DatasetState::default(), first.id).unwrap();
        assert_eq!(opened.dataset.file_name, "version-two.csv");
        assert_eq!(opened.project.name, "Versión dos");
    }

    #[test]
    fn failed_version_pruning_rolls_back_save_and_restore_publication() {
        let directory = tempfile::tempdir().unwrap();
        let store = ProjectStore::initialize(directory.path().join("data")).unwrap();
        let (initial_state, _) = active_state(directory.path(), &[1], "input.csv");
        let project = store
            .save(
                &initial_state,
                None,
                "Proyecto estable".to_owned(),
                ProjectWorkspace::default(),
            )
            .unwrap();

        for value in 2..=6 {
            let (updated, _) = active_state(directory.path(), &[value], "input.csv");
            store
                .autosave(
                    &updated,
                    project.id.clone(),
                    "Proyecto estable".to_owned(),
                    ProjectWorkspace::default(),
                )
                .unwrap();
        }
        let versions_before = store.list_versions(&project.id).unwrap();
        assert_eq!(versions_before.len(), MAX_PROJECT_VERSIONS);
        let active_before = store
            .stored_project(&store.connection().unwrap(), &project.id)
            .unwrap()
            .unwrap();

        store
            .connection()
            .unwrap()
            .execute_batch(
                "CREATE TRIGGER prevent_version_pruning
                 BEFORE DELETE ON project_versions
                 BEGIN SELECT RAISE(ABORT, 'injected prune failure'); END;",
            )
            .unwrap();

        let (unsaved, _) = active_state(directory.path(), &[7], "input.csv");
        assert!(store
            .autosave(
                &unsaved,
                project.id.clone(),
                "No publicado".to_owned(),
                ProjectWorkspace::default(),
            )
            .is_err());

        let after_failed_save = store
            .stored_project(&store.connection().unwrap(), &project.id)
            .unwrap()
            .unwrap();
        assert_eq!(after_failed_save.summary.name, active_before.summary.name);
        assert_eq!(
            after_failed_save.generation_name,
            active_before.generation_name
        );
        assert_eq!(store.list_versions(&project.id).unwrap(), versions_before);

        let restore_target = versions_before[0].id;
        assert!(store.restore_version(&project.id, restore_target).is_err());
        let after_failed_restore = store
            .stored_project(&store.connection().unwrap(), &project.id)
            .unwrap()
            .unwrap();
        assert_eq!(
            after_failed_restore.summary.name,
            active_before.summary.name
        );
        assert_eq!(
            after_failed_restore.generation_name,
            active_before.generation_name
        );
        assert_eq!(store.list_versions(&project.id).unwrap(), versions_before);

        store
            .connection()
            .unwrap()
            .execute_batch("DROP TRIGGER prevent_version_pruning;")
            .unwrap();
        store.restore_version(&project.id, restore_target).unwrap();
        let restored_state = DatasetState::default();
        store.open(&restored_state, project.id).unwrap();
        assert!(restored_state
            .active_project_snapshot()
            .unwrap()
            .frame
            .equals_missing(&frame(&[5])));
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
                "recipe": {},
                "sourceSchema": [{ "name": "", "dataType": "String" }]
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
        let oversized_sql_history = ProjectWorkspace {
            quality_rules: Vec::new(),
            recipe_draft: None,
            sql_history: (1..=6)
                .map(|id| SqlQueryHistoryEntry {
                    id,
                    outcome: "success".to_owned(),
                    duration_ms: 0,
                    row_count: Some(0),
                })
                .collect(),
            review_tab: Default::default(),
            preview_offset: Default::default(),
            active_phase: Default::default(),
            query_engine: Default::default(),
            analysis_sample_rows: Default::default(),
            performance_profile: Default::default(),
            export_format: Default::default(),
            privacy_mode: Default::default(),
            comparison_key_columns: Default::default(),
            join_type: Default::default(),
            import_profile: None,
        };
        assert!(store
            .save(
                &state,
                Some(created.id.clone()),
                "No publicado".to_owned(),
                oversized_sql_history,
            )
            .is_err());
        let invalid_review_tab: ProjectWorkspace = serde_json::from_value(serde_json::json!({
            "qualityRules": [],
            "recipeDraft": null,
            "reviewTab": "unsupported"
        }))
        .unwrap();
        assert!(store
            .save(
                &state,
                Some(created.id.clone()),
                "No publicado".to_owned(),
                invalid_review_tab,
            )
            .is_err());
        let invalid_preview_offset = ProjectWorkspace {
            preview_offset: Some(50),
            ..ProjectWorkspace::default()
        };
        assert!(store
            .save(
                &state,
                Some(created.id.clone()),
                "No publicado".to_owned(),
                invalid_preview_offset,
            )
            .is_err());
        let invalid_active_phase: ProjectWorkspace = serde_json::from_value(serde_json::json!({
            "qualityRules": [],
            "recipeDraft": null,
            "activePhase": "unsupported"
        }))
        .unwrap();
        assert!(store
            .save(
                &state,
                Some(created.id.clone()),
                "No publicado".to_owned(),
                invalid_active_phase,
            )
            .is_err());
        let invalid_analysis_sample_rows = ProjectWorkspace {
            analysis_sample_rows: Some(123),
            ..ProjectWorkspace::default()
        };
        assert!(store
            .save(
                &state,
                Some(created.id.clone()),
                "No publicado".to_owned(),
                invalid_analysis_sample_rows,
            )
            .is_err());
        let invalid_query_engine = ProjectWorkspace {
            query_engine: Some("sqlite".to_owned()),
            ..ProjectWorkspace::default()
        };
        assert!(store
            .save(
                &state,
                Some(created.id.clone()),
                "No publicado".to_owned(),
                invalid_query_engine,
            )
            .is_err());
        let invalid_performance_profile = ProjectWorkspace {
            performance_profile: Some("turbo".to_owned()),
            ..ProjectWorkspace::default()
        };
        assert!(store
            .save(
                &state,
                Some(created.id.clone()),
                "No publicado".to_owned(),
                invalid_performance_profile,
            )
            .is_err());
        let invalid_export_format = ProjectWorkspace {
            export_format: Some("xml".to_owned()),
            ..ProjectWorkspace::default()
        };
        assert!(store
            .save(
                &state,
                Some(created.id.clone()),
                "No publicado".to_owned(),
                invalid_export_format,
            )
            .is_err());
        let invalid_privacy_mode = ProjectWorkspace {
            privacy_mode: Some("encrypt".to_owned()),
            ..ProjectWorkspace::default()
        };
        assert!(store
            .save(
                &state,
                Some(created.id.clone()),
                "No publicado".to_owned(),
                invalid_privacy_mode,
            )
            .is_err());
        let invalid_join_type = ProjectWorkspace {
            join_type: Some("outer".to_owned()),
            ..ProjectWorkspace::default()
        };
        assert!(store
            .save(
                &state,
                Some(created.id.clone()),
                "No publicado".to_owned(),
                invalid_join_type,
            )
            .is_err());
        let invalid_key_columns = ProjectWorkspace {
            comparison_key_columns: vec!["value".to_owned(), "value".to_owned()],
            ..ProjectWorkspace::default()
        };
        assert!(store
            .save(
                &state,
                Some(created.id.clone()),
                "No publicado".to_owned(),
                invalid_key_columns,
            )
            .is_err());
        let missing_key_column = ProjectWorkspace {
            comparison_key_columns: vec!["missing".to_owned()],
            ..ProjectWorkspace::default()
        };
        assert!(store
            .save(
                &state,
                Some(created.id.clone()),
                "No publicado".to_owned(),
                missing_key_column,
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
                    r#"{"version":2,"name":"Futura","savedAt":"2026-08-21T12:00:00Z","recipe":{},"sourceSchema":[{"name":"","dataType":"String"}]}"#,
                    project.id
                ],
            )
            .unwrap();
        assert!(store.open(&target_state, project.id.clone()).is_err());
        assert_eq!(
            target_state.active_project_snapshot().unwrap().file_name,
            "active.csv"
        );

        connection
            .execute(
                "UPDATE projects SET quality_rules_json = '[]', recipe_draft_json = NULL,
                 sql_history_json = ?1 WHERE id = ?2",
                params!["{not-json", project.id],
            )
            .unwrap();
        assert!(store.open(&target_state, project.id.clone()).is_err());
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
