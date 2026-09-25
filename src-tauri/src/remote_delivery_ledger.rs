//! Local record of `append` deliveries to remote databases.
//!
//! An `append` retried after a commit whose outcome is unknown can duplicate
//! every row. Columnia records each delivery before committing it so a second
//! delivery of the same content to the same table asks for confirmation. Only
//! SHA-256 keys are stored: never the connection string, credentials or data.

use std::{
    fs,
    path::{Path, PathBuf},
};

use polars::prelude::{AnyValue, DataFrame};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const LEDGER_FILE_NAME: &str = "remote-deliveries.json";
const LEDGER_VERSION: u32 = 1;
const MAX_LEDGER_ENTRIES: usize = 200;
const CANCEL_CHECK_ROWS: usize = 4_096;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DeliveryStatus {
    /// The commit was sent but its outcome was not confirmed.
    Uncertain,
    Committed,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeliveryRecord {
    pub(crate) key: String,
    pub(crate) status: DeliveryStatus,
    pub(crate) rows: usize,
    pub(crate) recorded_at: String,
}

#[derive(Default, Deserialize, Serialize)]
struct LedgerDocument {
    version: u32,
    deliveries: Vec<DeliveryRecord>,
}

pub(crate) struct DeliveryLedger {
    path: PathBuf,
}

impl DeliveryLedger {
    pub(crate) fn in_directory(directory: &Path) -> Self {
        Self {
            path: directory.join(LEDGER_FILE_NAME),
        }
    }

    pub(crate) fn find(&self, key: &str) -> Option<DeliveryRecord> {
        self.read()
            .deliveries
            .into_iter()
            .find(|record| record.key == key)
    }

    pub(crate) fn record(
        &self,
        key: &str,
        status: DeliveryStatus,
        rows: usize,
    ) -> Result<(), String> {
        let mut document = self.read();
        document.version = LEDGER_VERSION;
        document.deliveries.retain(|record| record.key != key);
        document.deliveries.push(DeliveryRecord {
            key: key.to_owned(),
            status,
            rows,
            recorded_at: chrono::Utc::now().to_rfc3339(),
        });
        let overflow = document.deliveries.len().saturating_sub(MAX_LEDGER_ENTRIES);
        document.deliveries.drain(..overflow);
        self.write(&document)
            .map_err(|error| format!("No se pudo registrar la entrega remota: {error}"))
    }

    /// An unreadable ledger is treated as empty: the next record replaces it.
    fn read(&self) -> LedgerDocument {
        fs::read(&self.path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<LedgerDocument>(&bytes).ok())
            .filter(|document| document.version == LEDGER_VERSION)
            .unwrap_or_default()
    }

    fn write(&self, document: &LedgerDocument) -> std::io::Result<()> {
        let directory = self
            .path
            .parent()
            .ok_or_else(|| std::io::Error::other("ruta del registro sin carpeta"))?;
        fs::create_dir_all(directory)?;
        let bytes = serde_json::to_vec_pretty(document).map_err(std::io::Error::other)?;
        let mut staged = tempfile::NamedTempFile::new_in(directory)?;
        std::io::Write::write_all(&mut staged, &bytes)?;
        staged.as_file().sync_all()?;
        staged.persist(&self.path).map_err(|error| error.error)?;
        Ok(())
    }
}

/// Key of one delivery: destination identity plus content identity.
pub(crate) fn delivery_key(target_identity: &str, content_identity: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(target_identity.as_bytes());
    hasher.update([0]);
    hasher.update(content_identity.as_bytes());
    hex::encode(hasher.finalize())
}

/// Content identity of an in-memory frame: schema and every value.
pub(crate) fn frame_content_identity<C>(
    frame: &DataFrame,
    is_cancelled: &C,
) -> Result<String, String>
where
    C: Fn() -> bool,
{
    let mut hasher = Sha256::new();
    hasher.update(frame.height().to_le_bytes());
    for column in frame.columns() {
        if is_cancelled() {
            return Err(crate::dataset::OPERATION_CANCELLED_MESSAGE.to_owned());
        }
        hasher.update(column.name().as_bytes());
        hasher.update([0x1f]);
        hasher.update(column.dtype().to_string().as_bytes());
        hasher.update([0x1e]);
        for row_index in 0..column.len() {
            if row_index.is_multiple_of(CANCEL_CHECK_ROWS) && is_cancelled() {
                return Err(crate::dataset::OPERATION_CANCELLED_MESSAGE.to_owned());
            }
            match column
                .get(row_index)
                .map_err(|error| format!("No se pudo leer el dataset para la entrega: {error}"))?
            {
                AnyValue::Null => hasher.update([0]),
                AnyValue::String(value) => {
                    hasher.update([1]);
                    hasher.update(value.as_bytes());
                }
                value => {
                    hasher.update([1]);
                    hasher.update(value.to_string().as_bytes());
                }
            }
            hasher.update([0x1f]);
        }
    }
    Ok(hex::encode(hasher.finalize()))
}

/// Content identity of a file-backed dataset. The file is not re-read: its
/// canonical path, size and modification time stand for its content.
pub(crate) fn file_content_identity(
    path: &Path,
    schema: &DataFrame,
    row_count: usize,
    variant: &str,
) -> Result<String, String> {
    let metadata = fs::metadata(path)
        .map_err(|error| format!("No se pudo leer la fuente para la entrega: {error}"))?;
    let modified = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let mut hasher = Sha256::new();
    for part in [
        path.to_string_lossy().as_ref(),
        &metadata.len().to_string(),
        &modified.to_string(),
        &row_count.to_string(),
        variant,
    ] {
        hasher.update(part.as_bytes());
        hasher.update([0]);
    }
    for column in schema.columns() {
        hasher.update(column.name().as_bytes());
        hasher.update([0x1f]);
        hasher.update(column.dtype().to_string().as_bytes());
        hasher.update([0x1e]);
    }
    Ok(hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use polars::df;
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn records_replace_the_same_key_and_survive_reopening() {
        let directory = tempdir().expect("carpeta temporal");
        let ledger = DeliveryLedger::in_directory(directory.path());
        assert_eq!(ledger.find("a"), None);
        ledger
            .record("a", DeliveryStatus::Uncertain, 3)
            .expect("registro dudoso");
        ledger
            .record("a", DeliveryStatus::Committed, 3)
            .expect("registro confirmado");
        let reopened = DeliveryLedger::in_directory(directory.path());
        let record = reopened.find("a").expect("entrega registrada");
        assert_eq!(record.status, DeliveryStatus::Committed);
        assert_eq!(record.rows, 3);
        let stored = fs::read_to_string(directory.path().join(LEDGER_FILE_NAME)).unwrap();
        assert_eq!(stored.matches("\"key\"").count(), 1);
    }

    #[test]
    fn keeps_only_the_most_recent_entries_and_ignores_a_corrupt_file() {
        let directory = tempdir().expect("carpeta temporal");
        fs::write(directory.path().join(LEDGER_FILE_NAME), b"{no es json").unwrap();
        let ledger = DeliveryLedger::in_directory(directory.path());
        assert_eq!(ledger.find("k0"), None);
        for index in 0..(MAX_LEDGER_ENTRIES + 5) {
            ledger
                .record(&format!("k{index}"), DeliveryStatus::Committed, index)
                .expect("registro");
        }
        assert_eq!(ledger.find("k0"), None);
        assert!(ledger
            .find(&format!("k{}", MAX_LEDGER_ENTRIES + 4))
            .is_some());
        assert_eq!(ledger.read().deliveries.len(), MAX_LEDGER_ENTRIES);
    }

    #[test]
    fn frame_identity_changes_with_values_nulls_and_schema() {
        let base = df!("id" => &[1i64, 2], "name" => &[Some("a"), None]).unwrap();
        let same = df!("id" => &[1i64, 2], "name" => &[Some("a"), None]).unwrap();
        let other_value = df!("id" => &[1i64, 3], "name" => &[Some("a"), None]).unwrap();
        let empty_text = df!("id" => &[1i64, 2], "name" => &[Some("a"), Some("")]).unwrap();
        let renamed = df!("id" => &[1i64, 2], "nombre" => &[Some("a"), None]).unwrap();
        let identity = |frame: &DataFrame| frame_content_identity(frame, &|| false).unwrap();
        assert_eq!(identity(&base), identity(&same));
        for different in [&other_value, &empty_text, &renamed] {
            assert_ne!(identity(&base), identity(different));
        }
        assert_eq!(
            frame_content_identity(&base, &|| true),
            Err(crate::dataset::OPERATION_CANCELLED_MESSAGE.to_owned())
        );
    }

    #[test]
    fn keys_separate_targets_and_contents() {
        assert_ne!(delivery_key("ab", "c"), delivery_key("a", "bc"));
        assert_eq!(delivery_key("a", "b"), delivery_key("a", "b"));
    }

    #[test]
    fn file_identity_follows_the_file_and_the_privacy_variant() {
        let directory = tempdir().expect("carpeta temporal");
        let path = directory.path().join("datos.csv");
        fs::write(&path, "id\n1\n").unwrap();
        let schema = df!("id" => &[1i64]).unwrap();
        let first = file_content_identity(&path, &schema, 1, "none").unwrap();
        assert_eq!(
            first,
            file_content_identity(&path, &schema, 1, "none").unwrap()
        );
        assert_ne!(
            first,
            file_content_identity(&path, &schema, 1, "mask").unwrap()
        );
        fs::write(&path, "id\n1\n2\n").unwrap();
        assert_ne!(
            first,
            file_content_identity(&path, &schema, 2, "none").unwrap()
        );
    }
}
