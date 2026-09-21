use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
};

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::dataset::OPERATION_CANCELLED_MESSAGE;

const PRESET_VERSION: u8 = 1;
const PRESET_ID_LENGTH: usize = 32;
const MAX_PRESETS: usize = 100;
const MAX_PRESET_BYTES: usize = 64 * 1024;
const MAX_COLUMNS: usize = 2_000;
pub(crate) const DELIVERY_PRESET_CATALOG_OPERATION: &str = "deliveryPresetCatalog";

/// Persisted delivery configuration. Credentials and overwrite authorization are
/// deliberately absent: selecting a preset must never grant a destructive write.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeliveryPreset {
    pub version: u8,
    pub name: String,
    pub format: String,
    pub selected_columns: Vec<String>,
    pub privacy_mode: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub database_target: Option<DeliveryPresetDatabaseTarget>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeliveryPresetDatabaseTarget {
    pub kind: String,
    pub schema: String,
    pub table: String,
    pub table_policy: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeliveryPresetSummary {
    pub id: String,
    pub name: String,
    pub format: String,
    pub updated_at: String,
    pub selected_column_count: usize,
    pub remote: bool,
}

pub struct DeliveryPresetState {
    store: DeliveryPresetStore,
    operation: Mutex<()>,
    catalog_generation: AtomicU64,
}

impl DeliveryPresetState {
    pub fn initialize(app_data_dir: PathBuf) -> Result<Self, String> {
        Ok(Self {
            store: DeliveryPresetStore::initialize(app_data_dir)?,
            operation: Mutex::new(()),
            catalog_generation: AtomicU64::new(0),
        })
    }

    fn begin_catalog(&self) -> u64 {
        self.catalog_generation
            .fetch_add(1, Ordering::SeqCst)
            .wrapping_add(1)
    }

    pub(crate) fn cancel_catalog(&self) {
        self.catalog_generation.fetch_add(1, Ordering::SeqCst);
    }

    fn catalog_was_cancelled(&self, generation: u64) -> bool {
        self.catalog_generation.load(Ordering::SeqCst) != generation
    }
}

struct DeliveryPresetStore {
    catalog: PathBuf,
}

impl DeliveryPresetStore {
    fn initialize(app_data_dir: PathBuf) -> Result<Self, String> {
        std::fs::create_dir_all(&app_data_dir).map_err(|_| storage_error())?;
        let store = Self {
            catalog: app_data_dir.join("delivery-presets.sqlite3"),
        };
        store.migrate()?;
        Ok(store)
    }

    fn connection(&self) -> Result<Connection, String> {
        let connection = Connection::open(&self.catalog).map_err(|_| storage_error())?;
        connection
            .execute_batch(
                "PRAGMA journal_mode = WAL;
                 PRAGMA synchronous = FULL;
                 PRAGMA busy_timeout = 5000;",
            )
            .map_err(|_| storage_error())?;
        Ok(connection)
    }

    fn migrate(&self) -> Result<(), String> {
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| storage_error())?;
        transaction
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS delivery_presets (
                   id TEXT PRIMARY KEY NOT NULL CHECK (length(id) = 32),
                   name TEXT NOT NULL COLLATE NOCASE UNIQUE,
                   document_json TEXT NOT NULL,
                   updated_at TEXT NOT NULL
                 );
                 CREATE INDEX IF NOT EXISTS delivery_presets_updated_at
                   ON delivery_presets(updated_at DESC, id ASC);",
            )
            .map_err(|_| storage_error())?;
        transaction.commit().map_err(|_| storage_error())
    }

    fn list_with_cancel(
        &self,
        is_cancelled: impl Fn() -> bool,
    ) -> Result<Vec<DeliveryPresetSummary>, String> {
        ensure_preset_catalog_not_cancelled(&is_cancelled)?;
        let connection = self.connection()?;
        ensure_preset_catalog_not_cancelled(&is_cancelled)?;
        let mut statement = connection
            .prepare(
                "SELECT id, name, document_json, updated_at
                 FROM delivery_presets ORDER BY updated_at DESC, id ASC",
            )
            .map_err(|_| storage_error())?;
        let mut rows = statement.query([]).map_err(|_| storage_error())?;
        let mut presets = Vec::new();
        loop {
            ensure_preset_catalog_not_cancelled(&is_cancelled)?;
            let Some(row) = rows.next().map_err(|_| storage_error())? else {
                break;
            };
            presets.push(preset_summary_from_row(row).map_err(|_| storage_error())?);
            ensure_preset_catalog_not_cancelled(&is_cancelled)?;
        }
        ensure_preset_catalog_not_cancelled(&is_cancelled)?;
        Ok(presets)
    }

    #[cfg(test)]
    fn list(&self) -> Result<Vec<DeliveryPresetSummary>, String> {
        self.list_with_cancel(|| false)
    }

    fn open(&self, preset_id: String) -> Result<DeliveryPreset, String> {
        validate_preset_id(&preset_id)?;
        let connection = self.connection()?;
        let encoded = connection
            .query_row(
                "SELECT document_json FROM delivery_presets WHERE id = ?1",
                params![preset_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|_| storage_error())?
            .ok_or_else(|| "El preset local solicitado no existe.".to_owned())?;
        let preset: DeliveryPreset = serde_json::from_str(&encoded)
            .map_err(|_| "El preset local guardado no tiene un formato válido.".to_owned())?;
        validate_preset(preset)
    }

    fn save(
        &self,
        preset_id: Option<String>,
        preset: DeliveryPreset,
    ) -> Result<DeliveryPresetSummary, String> {
        let preset = validate_preset(preset)?;
        let encoded = serde_json::to_string(&preset)
            .map_err(|_| "No se pudo guardar el preset de entrega.".to_owned())?;
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| storage_error())?;
        let now = now_utc();
        let id = if let Some(id) = preset_id {
            validate_preset_id(&id)?;
            let exists: bool = transaction
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM delivery_presets WHERE id = ?1)",
                    params![id],
                    |row| row.get(0),
                )
                .map_err(|_| storage_error())?;
            if !exists {
                return Err("El preset que intentas actualizar ya no existe.".to_owned());
            }
            transaction
                .execute(
                    "UPDATE delivery_presets SET name = ?1, document_json = ?2, updated_at = ?3
                     WHERE id = ?4",
                    params![preset.name, encoded, now, id],
                )
                .map_err(|_| "Ya existe un preset con ese nombre.".to_owned())?;
            id
        } else {
            let count: i64 = transaction
                .query_row("SELECT COUNT(*) FROM delivery_presets", [], |row| {
                    row.get(0)
                })
                .map_err(|_| storage_error())?;
            if count >= MAX_PRESETS as i64 {
                return Err(format!(
                    "El catálogo admite como máximo {MAX_PRESETS} presets."
                ));
            }
            let id = unused_preset_id(&transaction)?;
            transaction
                .execute(
                    "INSERT INTO delivery_presets (id, name, document_json, updated_at)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![id, preset.name, encoded, now],
                )
                .map_err(|_| "Ya existe un preset con ese nombre.".to_owned())?;
            id
        };
        transaction.commit().map_err(|_| storage_error())?;
        Ok(preset_summary(&id, &preset, now))
    }

    fn delete(&self, preset_id: String) -> Result<(), String> {
        validate_preset_id(&preset_id)?;
        let connection = self.connection()?;
        let deleted = connection
            .execute(
                "DELETE FROM delivery_presets WHERE id = ?1",
                params![preset_id],
            )
            .map_err(|_| storage_error())?;
        if deleted != 1 {
            return Err("El preset local solicitado no existe.".to_owned());
        }
        Ok(())
    }
}

fn validate_preset(mut preset: DeliveryPreset) -> Result<DeliveryPreset, String> {
    if preset.version != PRESET_VERSION {
        return Err("La versión del preset de entrega no es compatible.".to_owned());
    }
    preset.name = preset.name.trim().to_owned();
    if preset.name.is_empty()
        || preset.name.chars().count() > 80
        || preset.name.chars().any(char::is_control)
    {
        return Err("El nombre del preset debe tener entre 1 y 80 caracteres.".to_owned());
    }
    if !matches!(
        preset.format.as_str(),
        "csv"
            | "json"
            | "parquet"
            | "sql"
            | "excel"
            | "sqlite"
            | "bundle"
            | "postgresql"
            | "mysql"
            | "sqlserver"
    ) {
        return Err("El formato guardado en el preset no es compatible.".to_owned());
    }
    if !matches!(preset.privacy_mode.as_str(), "none" | "mask" | "hash") {
        return Err("La protección de datos guardada en el preset no es válida.".to_owned());
    }
    if preset.selected_columns.len() > MAX_COLUMNS
        || preset
            .selected_columns
            .iter()
            .any(|column| column.trim().is_empty() || column.chars().any(char::is_control))
    {
        return Err("La lista de columnas del preset no es válida.".to_owned());
    }
    let unique_columns = preset.selected_columns.iter().collect::<HashSet<_>>();
    if unique_columns.len() != preset.selected_columns.len() {
        return Err("El preset no puede repetir columnas de salida.".to_owned());
    }
    match (preset.format.as_str(), preset.database_target.as_ref()) {
        ("postgresql" | "mysql" | "sqlserver", Some(target)) => {
            if target.kind != preset.format {
                return Err("El motor del preset no coincide con su formato.".to_owned());
            }
            if !matches!(
                target.table_policy.as_str(),
                "append" | "create_only" | "replace"
            ) || (target.kind == "mysql" && target.table_policy == "replace")
            {
                return Err("La política guardada en el preset no es compatible.".to_owned());
            }
            validate_identifier(&target.schema, "El esquema", true)?;
            validate_identifier(&target.table, "La tabla", false)?;
        }
        ("postgresql" | "mysql" | "sqlserver", None) => {
            return Err("El preset remoto debe incluir el destino sin credenciales.".to_owned());
        }
        (_, Some(_)) => {
            return Err("Un preset de archivo local no admite destino remoto.".to_owned());
        }
        (_, None) => {}
    }
    let encoded = serde_json::to_vec(&preset)
        .map_err(|_| "No se pudo validar el preset de entrega.".to_owned())?;
    if encoded.len() > MAX_PRESET_BYTES {
        return Err("El preset de entrega supera el límite de 64 KiB.".to_owned());
    }
    Ok(preset)
}

fn validate_identifier(value: &str, label: &str, optional: bool) -> Result<(), String> {
    if optional && value.trim().is_empty() {
        return Ok(());
    }
    let value = value.trim();
    if value.is_empty()
        || value.chars().count() > 128
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
        || value
            .chars()
            .next()
            .is_none_or(|character| !character.is_ascii_alphabetic() && character != '_')
    {
        return Err(format!(
            "{label} solo admite letras ASCII, números y guiones bajos."
        ));
    }
    Ok(())
}

fn validate_preset_id(id: &str) -> Result<(), String> {
    if id.len() != PRESET_ID_LENGTH
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err("El identificador del preset local no es válido.".to_owned());
    }
    Ok(())
}

fn unused_preset_id(connection: &Connection) -> Result<String, String> {
    for _ in 0..8 {
        let id = connection
            .query_row("SELECT lower(hex(randomblob(16)))", [], |row| {
                row.get::<_, String>(0)
            })
            .map_err(|_| storage_error())?;
        let exists: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM delivery_presets WHERE id = ?1)",
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

fn preset_summary_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<DeliveryPresetSummary> {
    let id: String = row.get(0)?;
    let name: String = row.get(1)?;
    let encoded: String = row.get(2)?;
    let updated_at: String = row.get(3)?;
    let preset: DeliveryPreset = serde_json::from_str(&encoded).map_err(|_| {
        rusqlite::Error::FromSqlConversionFailure(
            2,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::other("invalid delivery preset document")),
        )
    })?;
    if preset.name != name {
        return Err(rusqlite::Error::FromSqlConversionFailure(
            2,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::other(
                "delivery preset catalog name mismatch",
            )),
        ));
    }
    Ok(preset_summary(&id, &preset, updated_at))
}

fn preset_summary(id: &str, preset: &DeliveryPreset, updated_at: String) -> DeliveryPresetSummary {
    DeliveryPresetSummary {
        id: id.to_owned(),
        name: preset.name.clone(),
        format: preset.format.clone(),
        updated_at,
        selected_column_count: preset.selected_columns.len(),
        remote: preset.database_target.is_some(),
    }
}

fn now_utc() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn storage_error() -> String {
    "No se pudo acceder al catálogo local de presets de entrega.".to_owned()
}

fn ensure_preset_catalog_not_cancelled(is_cancelled: &impl Fn() -> bool) -> Result<(), String> {
    if is_cancelled() {
        Err(OPERATION_CANCELLED_MESSAGE.to_owned())
    } else {
        Ok(())
    }
}

async fn run_operation<T, F>(app: AppHandle, operation: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(&DeliveryPresetStore) -> Result<T, String> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<DeliveryPresetState>();
        let _guard = state.operation.lock().map_err(|_| {
            "El catálogo local de presets de entrega no está disponible.".to_owned()
        })?;
        operation(&state.store)
    })
    .await
    .map_err(|_| "La operación de presets de entrega se interrumpió.".to_owned())?
}

#[tauri::command]
pub async fn list_delivery_presets(app: AppHandle) -> Result<Vec<DeliveryPresetSummary>, String> {
    let generation = app.state::<DeliveryPresetState>().begin_catalog();
    let cancellation_app = app.clone();
    run_operation(app, move |store| {
        let state = cancellation_app.state::<DeliveryPresetState>();
        store.list_with_cancel(|| state.catalog_was_cancelled(generation))
    })
    .await
}

#[tauri::command]
pub async fn open_delivery_preset(
    app: AppHandle,
    preset_id: String,
) -> Result<DeliveryPreset, String> {
    run_operation(app, move |store| store.open(preset_id)).await
}

#[tauri::command]
pub async fn save_delivery_preset(
    app: AppHandle,
    preset_id: Option<String>,
    preset: DeliveryPreset,
) -> Result<DeliveryPresetSummary, String> {
    run_operation(app, move |store| store.save(preset_id, preset)).await
}

#[tauri::command]
pub async fn delete_delivery_preset(app: AppHandle, preset_id: String) -> Result<(), String> {
    run_operation(app, move |store| store.delete(preset_id)).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::tempdir;

    fn preset() -> DeliveryPreset {
        DeliveryPreset {
            version: 1,
            name: "Cierre mensual".to_owned(),
            format: "postgresql".to_owned(),
            selected_columns: vec!["id".to_owned(), "total".to_owned()],
            privacy_mode: "hash".to_owned(),
            database_target: Some(DeliveryPresetDatabaseTarget {
                kind: "postgresql".to_owned(),
                schema: "public".to_owned(),
                table: "cierre".to_owned(),
                table_policy: "append".to_owned(),
            }),
        }
    }

    #[test]
    fn preset_is_persisted_reopened_and_never_contains_credentials() {
        let directory = tempdir().unwrap();
        let store = DeliveryPresetStore::initialize(directory.path().to_owned()).unwrap();
        let saved = store.save(None, preset()).unwrap();
        let reopened = store.open(saved.id.clone()).unwrap();
        assert_eq!(reopened, preset());
        assert_eq!(store.list().unwrap().len(), 1);

        let encoded = serde_json::to_value(reopened).unwrap();
        assert!(encoded.get("connectionString").is_none());
        assert!(encoded.get("credentials").is_none());
        assert!(encoded["databaseTarget"].get("connectionString").is_none());
    }

    #[test]
    fn preset_contract_rejects_secrets_unknown_fields_and_implicit_mysql_replace() {
        let mut encoded = serde_json::to_value(preset()).unwrap();
        encoded["databaseTarget"]["connectionString"] = json!("Pwd=secret");
        assert!(serde_json::from_value::<DeliveryPreset>(encoded).is_err());

        let mut mysql = preset();
        mysql.format = "mysql".to_owned();
        mysql.database_target.as_mut().unwrap().kind = "mysql".to_owned();
        mysql.database_target.as_mut().unwrap().table_policy = "replace".to_owned();
        assert!(validate_preset(mysql).is_err());

        let mut replacement = preset();
        replacement.database_target.as_mut().unwrap().table_policy = "replace".to_owned();
        assert!(validate_preset(replacement).is_ok());
    }

    #[test]
    fn preset_catalog_updates_and_deletes_by_explicit_id() {
        let directory = tempdir().unwrap();
        let store = DeliveryPresetStore::initialize(directory.path().to_owned()).unwrap();
        let saved = store.save(None, preset()).unwrap();
        let mut updated = preset();
        updated.name = "Cierre semanal".to_owned();
        let changed = store.save(Some(saved.id.clone()), updated).unwrap();
        assert_eq!(changed.id, saved.id);
        assert_eq!(changed.name, "Cierre semanal");
        store.delete(saved.id.clone()).unwrap();
        assert!(store.open(saved.id).is_err());
    }
}
