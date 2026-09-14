use std::{collections::HashSet, fs, path::PathBuf, sync::Mutex};

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::dataset::{
    validate_import_profile, validate_reusable_quality_rules, validate_reusable_recipe,
    ImportProfile, ImportProfileColumn, ImportProfileTypeChange, QualityRule,
    StoredTransformRecipe,
};

const TASK_SCHEMA_VERSION: i64 = 1;
const TASK_DOCUMENT_VERSION: u8 = 1;
const TASK_ID_LENGTH: usize = 32;
const MAX_TASK_DOCUMENT_BYTES: usize = 2 * 1024 * 1024;

/// Configuration local reusable between files. It intentionally has no source path,
/// database target, credentials, table policy, or overwrite authorization.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReusableTask {
    pub version: u8,
    pub name: String,
    pub import_profile: ImportProfile,
    pub recipe: Option<StoredTransformRecipe>,
    pub quality_rules: Vec<QualityRule>,
    pub output_format: String,
    pub privacy_mode: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReusableTaskSummary {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub updated_at: String,
    pub input_column_count: usize,
    pub has_recipe: bool,
    pub quality_rule_count: usize,
    pub output_format: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReusableTaskCompatibilityStatus {
    Ready,
    ReviewRequired,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReusableTaskSchemaCompatibility {
    pub status: ReusableTaskCompatibilityStatus,
    pub missing_columns: Vec<String>,
    pub added_columns: Vec<String>,
    pub changed_types: Vec<ImportProfileTypeChange>,
    pub order_changed: bool,
}

pub struct ReusableTaskState {
    store: TaskStore,
    operation: Mutex<()>,
}

impl ReusableTaskState {
    pub fn initialize(app_data_dir: PathBuf) -> Result<Self, String> {
        Ok(Self {
            store: TaskStore::initialize(app_data_dir)?,
            operation: Mutex::new(()),
        })
    }
}

struct TaskStore {
    catalog: PathBuf,
}

impl TaskStore {
    fn initialize(app_data_dir: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(&app_data_dir).map_err(|_| storage_error())?;
        let store = Self {
            catalog: app_data_dir.join("reusable-tasks.sqlite3"),
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
        if version > TASK_SCHEMA_VERSION {
            return Err("El catálogo de tareas pertenece a una versión más reciente.".to_owned());
        }
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| storage_error())?;
        transaction
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS reusable_tasks (
                   id TEXT PRIMARY KEY NOT NULL CHECK (length(id) = 32),
                   name TEXT NOT NULL,
                   document_json TEXT NOT NULL,
                   created_at TEXT NOT NULL,
                   updated_at TEXT NOT NULL
                 );
                 CREATE INDEX IF NOT EXISTS reusable_tasks_updated_at
                   ON reusable_tasks(updated_at DESC, id ASC);
                 PRAGMA user_version = 1;",
            )
            .map_err(|_| storage_error())?;
        transaction.commit().map_err(|_| storage_error())
    }

    fn list(&self) -> Result<Vec<ReusableTaskSummary>, String> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare(
                "SELECT id, name, document_json, created_at, updated_at
                 FROM reusable_tasks ORDER BY updated_at DESC, id ASC",
            )
            .map_err(|_| storage_error())?;
        let rows = statement
            .query_map([], task_summary_from_row)
            .map_err(|_| storage_error())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|_| storage_error())
    }

    fn open(&self, task_id: String) -> Result<ReusableTask, String> {
        validate_task_id(&task_id)?;
        let connection = self.connection()?;
        let encoded = connection
            .query_row(
                "SELECT document_json FROM reusable_tasks WHERE id = ?1",
                params![task_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|_| storage_error())?
            .ok_or_else(|| "La tarea local solicitada no existe.".to_owned())?;
        let task: ReusableTask = serde_json::from_str(&encoded)
            .map_err(|_| "La tarea local guardada no tiene un formato válido.".to_owned())?;
        validate_reusable_task(task)
    }

    fn save(
        &self,
        task_id: Option<String>,
        task: ReusableTask,
    ) -> Result<ReusableTaskSummary, String> {
        let task = validate_reusable_task(task)?;
        let encoded = serde_json::to_string(&task)
            .map_err(|_| "No se pudo guardar la tarea local.".to_owned())?;
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| storage_error())?;
        let now = now_utc();
        let (id, created_at) = if let Some(id) = task_id {
            validate_task_id(&id)?;
            let created_at = transaction
                .query_row(
                    "SELECT created_at FROM reusable_tasks WHERE id = ?1",
                    params![id],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(|_| storage_error())?
                .ok_or_else(|| "La tarea local que intentas actualizar no existe.".to_owned())?;
            let updated = transaction
                .execute(
                    "UPDATE reusable_tasks SET name = ?1, document_json = ?2, updated_at = ?3
                     WHERE id = ?4",
                    params![task.name, encoded, now, id],
                )
                .map_err(|_| storage_error())?;
            if updated != 1 {
                return Err(storage_error());
            }
            (id, created_at)
        } else {
            let id = unused_task_id(&transaction)?;
            transaction
                .execute(
                    "INSERT INTO reusable_tasks (id, name, document_json, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?4)",
                    params![id, task.name, encoded, now],
                )
                .map_err(|_| storage_error())?;
            (id, now.clone())
        };
        transaction.commit().map_err(|_| storage_error())?;
        Ok(task_summary(&id, &task, created_at, now))
    }

    fn delete(&self, task_id: String) -> Result<(), String> {
        validate_task_id(&task_id)?;
        let connection = self.connection()?;
        let deleted = connection
            .execute("DELETE FROM reusable_tasks WHERE id = ?1", params![task_id])
            .map_err(|_| storage_error())?;
        if deleted != 1 {
            return Err("La tarea local solicitada no existe.".to_owned());
        }
        Ok(())
    }

    fn check_schema(
        &self,
        task_id: String,
        schema: Vec<ImportProfileColumn>,
    ) -> Result<ReusableTaskSchemaCompatibility, String> {
        let task = self.open(task_id)?;
        let candidate = ImportProfile {
            version: 1,
            format: task.import_profile.format.clone(),
            sheet_name: task.import_profile.sheet_name.clone(),
            header_mode: task.import_profile.header_mode,
            date_convention: task.import_profile.date_convention,
            number_convention: task.import_profile.number_convention,
            schema,
        };
        validate_import_profile(&candidate)?;
        Ok(compare_schema(
            &task.import_profile.schema,
            &candidate.schema,
        ))
    }
}

fn validate_reusable_task(mut task: ReusableTask) -> Result<ReusableTask, String> {
    if task.version != TASK_DOCUMENT_VERSION {
        return Err("La versión de la tarea reutilizable no es compatible.".to_owned());
    }
    task.name = task.name.trim().to_owned();
    if task.name.is_empty()
        || task.name.chars().count() > 128
        || task.name.chars().any(char::is_control)
    {
        return Err("El nombre de la tarea debe tener entre 1 y 128 caracteres.".to_owned());
    }
    validate_import_profile(&task.import_profile)?;
    validate_reusable_quality_rules(&task.quality_rules)?;
    if let Some(recipe) = task.recipe.as_ref() {
        validate_reusable_recipe(recipe)?;
    }
    if !matches!(
        task.output_format.as_str(),
        "csv" | "json" | "parquet" | "sql" | "excel" | "sqlite" | "bundle"
    ) {
        return Err("La tarea solo puede guardar un formato de archivo local.".to_owned());
    }
    if !matches!(task.privacy_mode.as_str(), "none" | "mask" | "hash") {
        return Err("La protección de datos de la tarea no es válida.".to_owned());
    }
    let encoded =
        serde_json::to_vec(&task).map_err(|_| "No se pudo validar la tarea local.".to_owned())?;
    if encoded.len() > MAX_TASK_DOCUMENT_BYTES {
        return Err("La tarea reutilizable supera el límite de 2 MiB.".to_owned());
    }
    Ok(task)
}

fn compare_schema(
    expected: &[ImportProfileColumn],
    actual: &[ImportProfileColumn],
) -> ReusableTaskSchemaCompatibility {
    let actual_by_name = actual
        .iter()
        .map(|column| (column.name.as_str(), column.data_type.as_str()))
        .collect::<std::collections::HashMap<_, _>>();
    let expected_names = expected
        .iter()
        .map(|column| column.name.as_str())
        .collect::<HashSet<_>>();
    let missing_columns = expected
        .iter()
        .filter(|column| !actual_by_name.contains_key(column.name.as_str()))
        .map(|column| column.name.clone())
        .collect::<Vec<_>>();
    let added_columns = actual
        .iter()
        .filter(|column| !expected_names.contains(column.name.as_str()))
        .map(|column| column.name.clone())
        .collect::<Vec<_>>();
    let changed_types = expected
        .iter()
        .filter_map(|column| {
            let actual_type = actual_by_name.get(column.name.as_str())?;
            (column.data_type != *actual_type).then(|| ImportProfileTypeChange {
                column: column.name.clone(),
                expected: column.data_type.clone(),
                actual: (*actual_type).to_owned(),
            })
        })
        .collect::<Vec<_>>();
    let order_changed = expected
        .iter()
        .map(|column| &column.name)
        .collect::<Vec<_>>()
        != actual.iter().map(|column| &column.name).collect::<Vec<_>>();
    let requires_review = !missing_columns.is_empty()
        || !added_columns.is_empty()
        || !changed_types.is_empty()
        || order_changed;
    ReusableTaskSchemaCompatibility {
        status: if requires_review {
            ReusableTaskCompatibilityStatus::ReviewRequired
        } else {
            ReusableTaskCompatibilityStatus::Ready
        },
        missing_columns,
        added_columns,
        changed_types,
        order_changed,
    }
}

fn validate_task_id(id: &str) -> Result<(), String> {
    if id.len() != TASK_ID_LENGTH
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err("El identificador de la tarea local no es válido.".to_owned());
    }
    Ok(())
}

fn unused_task_id(connection: &Connection) -> Result<String, String> {
    for _ in 0..8 {
        let id = connection
            .query_row("SELECT lower(hex(randomblob(16)))", [], |row| {
                row.get::<_, String>(0)
            })
            .map_err(|_| storage_error())?;
        let exists: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM reusable_tasks WHERE id = ?1)",
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

fn task_summary_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ReusableTaskSummary> {
    let id: String = row.get(0)?;
    let name: String = row.get(1)?;
    let encoded: String = row.get(2)?;
    let created_at: String = row.get(3)?;
    let updated_at: String = row.get(4)?;
    let task: ReusableTask = serde_json::from_str(&encoded).map_err(|_| {
        rusqlite::Error::FromSqlConversionFailure(
            2,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::other("invalid task document")),
        )
    })?;
    if task.name != name {
        return Err(rusqlite::Error::FromSqlConversionFailure(
            2,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::other("task catalog name mismatch")),
        ));
    }
    Ok(task_summary(&id, &task, created_at, updated_at))
}

fn task_summary(
    id: &str,
    task: &ReusableTask,
    created_at: String,
    updated_at: String,
) -> ReusableTaskSummary {
    ReusableTaskSummary {
        id: id.to_owned(),
        name: task.name.clone(),
        created_at,
        updated_at,
        input_column_count: task.import_profile.schema.len(),
        has_recipe: task.recipe.is_some(),
        quality_rule_count: task.quality_rules.len(),
        output_format: task.output_format.clone(),
    }
}

fn now_utc() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn storage_error() -> String {
    "No se pudo acceder al catálogo local de tareas.".to_owned()
}

async fn run_task_operation<T, F>(app: AppHandle, operation: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(&TaskStore) -> Result<T, String> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<ReusableTaskState>();
        let _guard = state
            .operation
            .lock()
            .map_err(|_| "El catálogo local de tareas no está disponible.".to_owned())?;
        operation(&state.store)
    })
    .await
    .map_err(|_| "La operación de tareas se interrumpió.".to_owned())?
}

#[tauri::command]
pub async fn list_reusable_tasks(app: AppHandle) -> Result<Vec<ReusableTaskSummary>, String> {
    run_task_operation(app, |store| store.list()).await
}

#[tauri::command]
pub async fn save_reusable_task(
    app: AppHandle,
    task_id: Option<String>,
    task: ReusableTask,
) -> Result<ReusableTaskSummary, String> {
    run_task_operation(app, move |store| store.save(task_id, task)).await
}

#[tauri::command]
pub async fn open_reusable_task(app: AppHandle, task_id: String) -> Result<ReusableTask, String> {
    run_task_operation(app, move |store| store.open(task_id)).await
}

#[tauri::command]
pub async fn delete_reusable_task(app: AppHandle, task_id: String) -> Result<(), String> {
    run_task_operation(app, move |store| store.delete(task_id)).await
}

#[tauri::command]
pub async fn check_reusable_task_schema(
    app: AppHandle,
    task_id: String,
    schema: Vec<ImportProfileColumn>,
) -> Result<ReusableTaskSchemaCompatibility, String> {
    run_task_operation(app, move |store| store.check_schema(task_id, schema)).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataset::{ImportDateConvention, ImportNumberConvention};
    use serde_json::json;

    fn task() -> ReusableTask {
        ReusableTask {
            version: 1,
            name: "Cierre mensual".to_owned(),
            import_profile: ImportProfile {
                version: 1,
                format: "csv".to_owned(),
                sheet_name: None,
                header_mode: None,
                date_convention: Some(ImportDateConvention::Dmy),
                number_convention: Some(ImportNumberConvention::DotDecimalCommaGrouping),
                schema: vec![
                    ImportProfileColumn {
                        name: "id".to_owned(),
                        data_type: "Int64".to_owned(),
                    },
                    ImportProfileColumn {
                        name: "amount".to_owned(),
                        data_type: "Float64".to_owned(),
                    },
                ],
            },
            recipe: None,
            quality_rules: Vec::new(),
            output_format: "csv".to_owned(),
            privacy_mode: "mask".to_owned(),
        }
    }

    #[test]
    fn task_contract_rejects_credentials_and_overwrite_permissions() {
        let encoded = serde_json::to_value(task()).unwrap();
        let mut with_secret = encoded.clone();
        with_secret["databaseCredentials"] = json!({ "password": "must-not-persist" });
        with_secret["allowOverwrite"] = json!(true);
        assert!(serde_json::from_value::<ReusableTask>(with_secret).is_err());

        let mut with_path = encoded;
        with_path["importProfile"]["sourcePath"] = json!("C:/private/input.csv");
        assert!(serde_json::from_value::<ReusableTask>(with_path).is_err());
        assert!(validate_reusable_task(task()).is_ok());
    }

    #[test]
    fn task_only_accepts_local_file_delivery_formats() {
        let mut task = task();
        task.output_format = "odbc".to_owned();
        assert!(validate_reusable_task(task).is_err());
    }

    #[test]
    fn exact_schema_is_ready_and_any_schema_difference_requires_review() {
        let task = task();
        let same = compare_schema(&task.import_profile.schema, &task.import_profile.schema);
        assert_eq!(same.status, ReusableTaskCompatibilityStatus::Ready);

        let changed = vec![
            ImportProfileColumn {
                name: "id".to_owned(),
                data_type: "String".to_owned(),
            },
            ImportProfileColumn {
                name: "total".to_owned(),
                data_type: "Float64".to_owned(),
            },
        ];
        let result = compare_schema(&task.import_profile.schema, &changed);
        assert_eq!(
            result.status,
            ReusableTaskCompatibilityStatus::ReviewRequired
        );
        assert_eq!(result.missing_columns, vec!["amount"]);
        assert_eq!(result.added_columns, vec!["total"]);
        assert_eq!(result.changed_types[0].column, "id");
        assert!(result.order_changed);
    }

    #[test]
    fn task_catalog_round_trips_and_survives_reopening() {
        let directory = tempfile::tempdir().unwrap();
        let store = TaskStore::initialize(directory.path().join("data")).unwrap();
        let created = store.save(None, task()).unwrap();
        assert_eq!(created.name, "Cierre mensual");
        assert_eq!(created.input_column_count, 2);
        assert_eq!(store.open(created.id.clone()).unwrap(), task());
        assert_eq!(store.list().unwrap(), vec![created.clone()]);

        let reopened = TaskStore::initialize(directory.path().join("data")).unwrap();
        assert_eq!(reopened.open(created.id.clone()).unwrap(), task());
        reopened.delete(created.id.clone()).unwrap();
        assert!(reopened.open(created.id).is_err());
    }

    #[test]
    fn newer_task_catalog_schema_is_not_opened() {
        let directory = tempfile::tempdir().unwrap();
        let app_data_dir = directory.path().join("data");
        fs::create_dir_all(&app_data_dir).unwrap();
        let connection = Connection::open(app_data_dir.join("reusable-tasks.sqlite3")).unwrap();
        connection
            .execute_batch("PRAGMA user_version = 2;")
            .unwrap();
        drop(connection);
        assert!(TaskStore::initialize(app_data_dir).is_err());
    }
}
