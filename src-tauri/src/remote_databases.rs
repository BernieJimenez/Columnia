use crate::crash_report::LockRecovering;
use std::{fmt::Debug, path::Path};

use odbc_api::{
    parameter::InputParameter, Bit, ColumnsRow, ConnectionOptions, Environment, IntoParameter,
    Nullable, TablesRow,
};
use polars::prelude::{AnyValue, DataFrame};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::dataset::{DatasetState, OPERATION_CANCELLED_MESSAGE};
use crate::duckdb_query::DuckDbFileFormat;
use crate::remote_delivery_ledger::{DeliveryRecord, DeliveryStatus};

const MAX_CONNECTION_STRING_CHARS: usize = 16 * 1024;
const MAX_IDENTIFIER_CHARS: usize = 128;
/// Longest value of a field the native dialog shows whole.
const MAX_SHOWN_CHARS: usize = 100;
const PREFLIGHT_CANCEL_CHECK_ROWS: usize = 1_024;
/// A remote server that never answers must not block a delivery forever:
/// cancellation is only observed between driver calls.
const ODBC_LOGIN_TIMEOUT_SEC: u32 = 15;
const ODBC_STATEMENT_TIMEOUT_SEC: usize = 300;
/// PROD-16: an INSERT batch (at most `MAX_ROWS_PER_INSERT` rows) that takes
/// longer is blocked, usually by a lock; giving up sooner lets Cancelar and
/// the error reach the person without waiting the full statement limit.
const ODBC_BATCH_TIMEOUT_SEC: usize = 30;
/// AUD-013: listing tables queries the server's catalog; one that does not
/// answer in this time fails with a message (login + catalog stay under 45 s).
const ODBC_CATALOG_TIMEOUT_SEC: usize = 30;

const REMOTE_TARGET_NOT_CONFIRMED: &str =
    "La conexión remota se canceló: no se confirmó el destino en el diálogo de Windows.";
const REPEATED_DELIVERY_NOT_CONFIRMED: &str =
    "La entrega se canceló para no duplicar filas en la tabla remota.";
/// SQL Server accepts at most 2,100 parameters per statement; MySQL and
/// PostgreSQL accept far more, so this limit fits every dialect.
const MAX_PARAMETERS_PER_INSERT: usize = 2_000;
const MAX_ROWS_PER_INSERT: usize = 500;

/// Hooks around the commit of a delivery. A delivery is recorded as uncertain
/// before its commit and as committed after it, so a retry after a failed or
/// interrupted commit can be detected.
pub(crate) trait DeliveryJournal {
    fn before_commit(&mut self, rows: usize) -> Result<(), String>;
    fn after_commit(&mut self, rows: usize);
}

impl DeliveryJournal for () {
    fn before_commit(&mut self, _rows: usize) -> Result<(), String> {
        Ok(())
    }

    fn after_commit(&mut self, _rows: usize) {}
}

impl<J: DeliveryJournal> DeliveryJournal for Option<J> {
    fn before_commit(&mut self, rows: usize) -> Result<(), String> {
        self.as_mut()
            .map_or(Ok(()), |journal| journal.before_commit(rows))
    }

    fn after_commit(&mut self, rows: usize) {
        if let Some(journal) = self.as_mut() {
            journal.after_commit(rows);
        }
    }
}

/// Identity of a destination table without credentials, stable across sessions.
pub(crate) fn remote_target_identity(target: &DatabaseTarget) -> String {
    // An invalid connection never reaches this point (it is validated before
    // anything is delivered); the empty list only keeps the function total.
    let mut attributes = target
        .connection
        .attributes(target.kind)
        .unwrap_or_default()
        .into_iter()
        .filter(|(key, _)| *key != "Pwd")
        .map(|(key, value)| format!("{}={value}", key.to_ascii_lowercase()))
        .collect::<Vec<_>>();
    attributes.sort();
    format!(
        "{}\u{1f}{}\u{1f}{}",
        target.kind.label(),
        attributes.join("\u{1e}"),
        qualified_table(target)
    )
}

fn describe_repeated_delivery(record: &DeliveryRecord) -> String {
    let when = chrono::DateTime::parse_from_rfc3339(&record.recorded_at)
        .map(|time| {
            time.with_timezone(&chrono::Local)
                .format("%d/%m/%Y %H:%M")
                .to_string()
        })
        .unwrap_or_else(|_| "una entrega anterior".to_owned());
    match record.status {
        DeliveryStatus::Committed => format!(
            "Este mismo dataset ya se añadió a esta tabla ({when}). Si lo entregas otra vez, sus {} filas quedarán duplicadas.\n\n¿Añadirlo de nuevo?",
            record.rows
        ),
        DeliveryStatus::Uncertain => format!(
            "Una entrega anterior de este mismo dataset a esta tabla ({when}) no confirmó si se guardó. Revisa la tabla: si se guardó, sus {} filas quedarán duplicadas.\n\n¿Añadirlo de nuevo?",
            record.rows
        ),
    }
}

/// A repeated `append` of the same content needs a native confirmation, like
/// the connection itself, so the webview cannot skip it.
pub(crate) fn confirm_repeated_delivery(
    app: &AppHandle,
    record: &DeliveryRecord,
) -> Result<(), String> {
    use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
    let confirmed = app
        .dialog()
        .message(describe_repeated_delivery(record))
        .title("Posibles filas duplicadas")
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Añadir de nuevo".to_owned(),
            "Cancelar".to_owned(),
        ))
        .blocking_show();
    if confirmed {
        Ok(())
    } else {
        Err(REPEATED_DELIVERY_NOT_CONFIRMED.to_owned())
    }
}

fn rows_per_insert(column_count: usize) -> usize {
    (MAX_PARAMETERS_PER_INSERT / column_count.max(1)).clamp(1, MAX_ROWS_PER_INSERT)
}

/// Sends rows as multi-row `INSERT ... VALUES (...), (...)` statements inside
/// the caller's transaction instead of one round trip per row.
struct BatchInserter<'c, 'env> {
    connection: &'c odbc_api::Connection<'env>,
    table: String,
    columns: Vec<String>,
    rows_per_batch: usize,
    full_batch: odbc_api::Prepared<odbc_api::handles::StatementImpl<'c>>,
    pending: Vec<Box<dyn InputParameter>>,
    pending_rows: usize,
    target: &'c DatabaseTarget,
}

impl<'c, 'env> BatchInserter<'c, 'env> {
    fn new(
        connection: &'c odbc_api::Connection<'env>,
        table: &str,
        columns: Vec<String>,
        target: &'c DatabaseTarget,
    ) -> Result<Self, String> {
        let rows_per_batch = rows_per_insert(columns.len());
        Self::with_rows_per_batch(connection, table, columns, target, rows_per_batch)
    }

    fn with_rows_per_batch(
        connection: &'c odbc_api::Connection<'env>,
        table: &str,
        columns: Vec<String>,
        target: &'c DatabaseTarget,
        rows_per_batch: usize,
    ) -> Result<Self, String> {
        let mut full_batch = connection
            .prepare(&parameterized_insert_sql(table, &columns, rows_per_batch))
            .map_err(|error| {
                format_driver_error("No se pudo preparar la inserción remota", error, target)
            })?;
        full_batch
            .set_query_timeout_sec(ODBC_BATCH_TIMEOUT_SEC)
            .map_err(|error| {
                format_driver_error(
                    "No se pudo limitar el tiempo de la inserción remota",
                    error,
                    target,
                )
            })?;
        Ok(Self {
            connection,
            table: table.to_owned(),
            pending: Vec::with_capacity(rows_per_batch * columns.len()),
            columns,
            rows_per_batch,
            full_batch,
            pending_rows: 0,
            target,
        })
    }

    /// Returns how many rows were sent to the server by this call.
    fn push_row(&mut self, row: Vec<Box<dyn InputParameter>>) -> Result<usize, String> {
        self.pending.extend(row);
        self.pending_rows += 1;
        if self.pending_rows < self.rows_per_batch {
            return Ok(0);
        }
        self.full_batch
            .execute(self.pending.as_slice())
            .map_err(|error| {
                format_driver_error(
                    "No se pudo insertar un lote en la tabla remota",
                    error,
                    self.target,
                )
            })?;
        self.pending.clear();
        Ok(std::mem::take(&mut self.pending_rows))
    }

    fn finish(mut self) -> Result<usize, String> {
        if self.pending_rows == 0 {
            return Ok(0);
        }
        let sql = parameterized_insert_sql(&self.table, &self.columns, self.pending_rows);
        self.connection
            .execute(&sql, self.pending.as_slice(), Some(ODBC_BATCH_TIMEOUT_SEC))
            .map_err(|error| {
                format_driver_error(
                    "No se pudo insertar un lote en la tabla remota",
                    error,
                    self.target,
                )
            })?;
        Ok(std::mem::take(&mut self.pending_rows))
    }
}

/// What the person reads when the commit fails (DAT-10): the server may
/// have saved the rows even though the answer did not arrive.
pub(crate) const UNCERTAIN_COMMIT_MESSAGE: &str = "No se pudo confirmar si la entrega se guardó: puede haberse guardado completa. Revisa la tabla remota antes de repetir; volver a añadir filas podría duplicarlas.";

/// MySQL commits `CREATE TABLE` at once, so a delivery that fails after it
/// leaves an empty table that blocks the next «Crear» (DAT-12). The table this
/// delivery created is dropped; with any other policy the error is returned
/// as it was.
fn undo_created_mysql_table(
    connection: &odbc_api::Connection<'_>,
    target: &DatabaseTarget,
    table: &str,
    error: String,
) -> String {
    if target.kind != DatabaseKind::Mysql || target.table_policy != DatabaseTablePolicy::CreateOnly
    {
        return error;
    }
    let _ = connection.rollback();
    let dropped = connection
        .execute(
            &drop_table_sql(table, target.kind),
            (),
            Some(ODBC_STATEMENT_TIMEOUT_SEC),
        )
        .is_ok()
        && connection.commit().is_ok();
    if dropped {
        format!(
            "{error} Se eliminó la tabla {table} que se había creado; puedes volver a intentarlo."
        )
    } else {
        format!("{error} No se pudo eliminar la tabla vacía {table}; bórrala antes de volver a intentarlo.")
    }
}

fn commit_delivery(
    connection: &odbc_api::Connection<'_>,
    journal: &mut impl DeliveryJournal,
    rows_written: usize,
    target: &DatabaseTarget,
) -> Result<(), String> {
    journal.before_commit(rows_written)?;
    connection.commit().map_err(|error| {
        format!(
            "{} {}",
            UNCERTAIN_COMMIT_MESSAGE,
            format_driver_error("Detalle al confirmar", error, target)
        )
    })?;
    journal.after_commit(rows_written);
    Ok(())
}

/// Remote targets the person confirmed in a native dialog during this session.
/// Only SHA-256 fingerprints are kept, never the connection string.
#[derive(Default)]
pub struct RemoteTargetConfirmations(std::sync::Mutex<std::collections::HashSet<String>>);

fn remote_target_fingerprint(target: &DatabaseTarget) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    let parts = [target.kind.label()]
        .into_iter()
        .chain(target.connection.fingerprint_parts())
        .chain([
            target.schema.as_str(),
            target.table.as_str(),
            match target.table_policy {
                DatabaseTablePolicy::Append => "append",
                DatabaseTablePolicy::CreateOnly => "create_only",
                DatabaseTablePolicy::Replace => "replace",
            },
        ]);
    for part in parts {
        hasher.update(part.as_bytes());
        hasher.update([0]);
    }
    hex::encode(hasher.finalize())
}

/// Human description of a delivery target without credentials.
fn describe_remote_target(target: &DatabaseTarget) -> String {
    let (server, details) = describe_connection(&target.connection, target.kind);
    let policy = match target.table_policy {
        DatabaseTablePolicy::Append => "añadir filas",
        DatabaseTablePolicy::CreateOnly => "crear la tabla",
        DatabaseTablePolicy::Replace => "reemplazar la tabla",
    };
    format!(
        "Columnia se conectará a {} en {} y podrá escribir en la tabla {} ({policy}). Los datos del dataset activo saldrán de este equipo.\n\n{}\n\n¿Confirmas este destino?",
        target.kind.label(),
        server,
        qualified_table(target),
        details
    )
}

/// A field's value as the native dialog shows it: cut to `MAX_SHOWN_CHARS`
/// so a long value cannot push the rest of the text out of sight.
fn shown(value: &str) -> String {
    let value = trimmed(value);
    if value.chars().count() > MAX_SHOWN_CHARS {
        format!(
            "{}…",
            value.chars().take(MAX_SHOWN_CHARS).collect::<String>()
        )
    } else {
        value.to_owned()
    }
}

/// The server and, one per line, everything that decides where and as whom
/// Columnia signs in (server, port, database, user, encryption, certificate
/// and driver), never the password. It describes a connection already
/// validated, so it never shows a value that is then rejected.
fn describe_connection(connection: &DatabaseConnection, kind: DatabaseKind) -> (String, String) {
    let is_sql_server = kind == DatabaseKind::SqlServer;
    let server = shown(&connection.server);
    let mut lines = vec![format!("Servidor: {server}")];
    let port = trimmed(&connection.port);
    if !port.is_empty() {
        lines.push(format!("Puerto: {}", shown(port)));
    }
    let database = trimmed(&connection.database);
    lines.push(if database.is_empty() {
        "Base de datos: la predeterminada del usuario".to_owned()
    } else {
        format!("Base de datos: {}", shown(database))
    });
    lines.push(match connection.authentication {
        DatabaseAuthentication::Windows => {
            "Usuario: tu usuario de Windows (autenticación de Windows)".to_owned()
        }
        DatabaseAuthentication::Password => format!("Usuario: {}", shown(&connection.user)),
    });
    lines.push(if is_sql_server {
        "Cifrado: siempre".to_owned()
    } else if connection.allow_unencrypted {
        "Cifrado: solo si el servidor lo ofrece (puede conectar sin cifrar)".to_owned()
    } else {
        "Cifrado: obligatorio".to_owned()
    });
    // `SSLmode=require` and `SSLMODE=REQUIRED` encrypt without checking the
    // certificate, so PostgreSQL and MySQL say so too.
    lines.push(if !is_sql_server {
        "Certificado: no se verifica".to_owned()
    } else if connection.trust_server_certificate {
        "Certificado: acepta cualquier certificado del servidor, sin verificarlo".to_owned()
    } else {
        "Certificado: se verifica".to_owned()
    });
    lines.push(format!("Controlador: {}", shown(&connection.driver)));
    (server, lines.join("\n"))
}

/// PROD-15: the driver is the name of an installed one; a path would load any
/// library.
fn validate_driver_name(driver: &str) -> Result<(), String> {
    let driver = trimmed(driver);
    if driver.is_empty() {
        return Err("Elige el controlador ODBC.".to_owned());
    }
    let lower = driver.to_ascii_lowercase();
    if driver.contains(['\\', '/', ':']) || lower.ends_with(".dll") || lower.ends_with(".so") {
        return Err("El controlador ODBC debe ser el nombre de uno instalado (por ejemplo, ODBC Driver 18 for SQL Server), no la ruta de un archivo.".to_owned());
    }
    Ok(())
}

/// PROD-15: the driver must be one of the drivers Windows lists as installed.
fn ensure_installed_driver(driver: &str, installed: &[String]) -> Result<(), String> {
    let driver = trimmed(driver);
    if installed
        .iter()
        .any(|name| name.trim().eq_ignore_ascii_case(driver))
    {
        Ok(())
    } else {
        Err(format!(
            "El controlador ODBC «{driver}» no está instalado en este equipo. Instálalo o elige uno de los que muestra el Administrador de orígenes de datos ODBC."
        ))
    }
}

/// Why a connection is opened. Only the connection string differs, and only
/// for SQL Server; the identity of a delivery destination never depends on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ConnectionPurpose {
    /// Write into a table, or test the connection of a delivery.
    Delivery,
    /// Read tables to import them.
    Import,
}

/// Opens the connection after checking the driver against the installed ones.
fn connect<'env>(
    environment: &'env Environment,
    target: &DatabaseTarget,
    purpose: ConnectionPurpose,
) -> Result<odbc_api::Connection<'env>, String> {
    let connection_string = target
        .connection
        .connection_string_for(target.kind, purpose)?;
    let installed = environment
        .drivers()
        .map_err(|error| {
            format_driver_error(
                "No se pudieron listar los controladores ODBC",
                error,
                target,
            )
        })?
        .into_iter()
        .map(|driver| driver.description)
        .collect::<Vec<_>>();
    ensure_installed_driver(&target.connection.driver, &installed)?;
    environment
        .connect_with_connection_string(&connection_string, odbc_connection_options())
        .map_err(|error| format_driver_error("No se pudo abrir la conexión", error, target))
}

/// A compromised interface can invoke any IPC command, so every remote
/// connection needs a native confirmation that the webview cannot fake.
pub(crate) fn confirm_remote_target(
    app: &AppHandle,
    target: &DatabaseTarget,
) -> Result<(), String> {
    confirm_with_native_dialog(
        app,
        remote_target_fingerprint(target),
        describe_remote_target(target),
        "Confirmar conexión remota",
    )
}

/// Asks once per session and fingerprint in a Windows dialog; the answer is
/// remembered only when accepted.
fn confirm_with_native_dialog(
    app: &AppHandle,
    fingerprint: String,
    message: String,
    title: &str,
) -> Result<(), String> {
    use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
    let confirmations = app.state::<RemoteTargetConfirmations>();
    let already_confirmed = confirmations.0.lock_recovering().contains(&fingerprint);
    if already_confirmed {
        return Ok(());
    }
    let confirmed = app
        .dialog()
        .message(message)
        .title(title)
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Conectar".to_owned(),
            "Cancelar".to_owned(),
        ))
        .blocking_show();
    if !confirmed {
        return Err(REMOTE_TARGET_NOT_CONFIRMED.to_owned());
    }
    confirmations.0.lock_recovering().insert(fingerprint);
    Ok(())
}

fn odbc_connection_options() -> ConnectionOptions {
    ConnectionOptions {
        login_timeout_sec: Some(ODBC_LOGIN_TIMEOUT_SEC),
        ..ConnectionOptions::default()
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DatabaseKind {
    Postgresql,
    Mysql,
    SqlServer,
}

impl DatabaseKind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Postgresql => "PostgreSQL",
            Self::Mysql => "MySQL",
            Self::SqlServer => "SQL Server",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DatabaseTablePolicy {
    Append,
    CreateOnly,
    Replace,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DatabaseAuthentication {
    /// Windows sign-in (`Trusted_Connection`); SQL Server only.
    Windows,
    /// User and password.
    Password,
}

const MAX_DRIVER_CHARS: usize = 256;
const MAX_SERVER_CHARS: usize = 255;
const MAX_USER_CHARS: usize = 256;
const MAX_PASSWORD_CHARS: usize = 1_024;

/// The fields of a remote connection, as the interface sends them. Rust builds
/// the ODBC connection string from them, so no field can add, repeat or
/// replace an attribute of it. It is intentionally not persisted and has no
/// `Serialize`.
#[derive(Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DatabaseConnection {
    /// Name of an installed ODBC driver.
    pub driver: String,
    /// On SQL Server, `host`, `host,port` or `host\instance`.
    pub server: String,
    /// PostgreSQL and MySQL only; empty is the engine's usual port.
    #[serde(default)]
    pub port: String,
    #[serde(default)]
    pub database: String,
    pub authentication: DatabaseAuthentication,
    #[serde(default)]
    pub user: String,
    #[serde(default)]
    pub password: String,
    /// SQL Server only: accept any certificate of the server.
    #[serde(default)]
    pub trust_server_certificate: bool,
    /// PostgreSQL and MySQL only: connect without encryption when the server
    /// offers none.
    #[serde(default)]
    pub allow_unencrypted: bool,
}

impl std::fmt::Debug for DatabaseConnection {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DatabaseConnection")
            .field("driver", &self.driver)
            .field("server", &self.server)
            .field("port", &self.port)
            .field("database", &self.database)
            .field("authentication", &self.authentication)
            .field("user", &self.user)
            .field("password", &"[REDACTED]")
            .field("trust_server_certificate", &self.trust_server_certificate)
            .field("allow_unencrypted", &self.allow_unencrypted)
            .finish()
    }
}

/// An ODBC attribute value. A value with `;`, `{`, `}` or `=`, or with white
/// space at either end, goes in braces with `}` doubled, so it can never end
/// itself, add attributes or repeat one.
fn odbc_value(value: &str) -> String {
    let needs_braces = value.contains([';', '{', '}', '='])
        || value.starts_with(is_odbc_space)
        || value.ends_with(is_odbc_space);
    if needs_braces {
        format!("{{{}}}", value.replace('}', "}}"))
    } else {
        value.to_owned()
    }
}

/// White space as the interface's own `trim` and `\s` understand it.
fn is_odbc_space(character: char) -> bool {
    character.is_whitespace() || character == '\u{feff}'
}

fn trimmed(value: &str) -> &str {
    value.trim_matches(is_odbc_space)
}

/// Bidirectional formatting characters (LRM, RLM, embeddings/overrides and
/// isolates) that can reorder what a native dialog shows.
fn is_bidi_format(character: char) -> bool {
    matches!(
        character,
        '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}'
    )
}

/// `reject_bidi` is off only for the password, which no dialog displays.
fn field_text_issue(value: &str, label: &str, limit: usize, reject_bidi: bool) -> Option<String> {
    if value.chars().count() > limit {
        return Some(format!("{label} no puede superar {limit} caracteres."));
    }
    if value.chars().any(|character| {
        character.is_control()
            || matches!(character, '\u{2028}' | '\u{2029}')
            || (reject_bidi && is_bidi_format(character))
    }) {
        return Some(format!(
            "{label} contiene caracteres de control no permitidos; bórralos y vuelve a intentarlo."
        ));
    }
    None
}

impl DatabaseConnection {
    /// Checks every field for the engine and, if they are valid, returns the
    /// attributes of the connection string in order, with each value already
    /// written for ODBC (braced when it needs it).
    fn attributes(&self, kind: DatabaseKind) -> Result<Vec<(&'static str, String)>, String> {
        let is_sql_server = kind == DatabaseKind::SqlServer;
        for (value, label, limit, reject_bidi) in [
            (&self.driver, "El controlador", MAX_DRIVER_CHARS, true),
            (&self.server, "El servidor", MAX_SERVER_CHARS, true),
            (&self.port, "El puerto", 16, true),
            (
                &self.database,
                "La base de datos",
                MAX_IDENTIFIER_CHARS * 2,
                true,
            ),
            (&self.user, "El usuario", MAX_USER_CHARS, true),
            (&self.password, "La contraseña", MAX_PASSWORD_CHARS, false),
        ] {
            if let Some(issue) = field_text_issue(value, label, limit, reject_bidi) {
                return Err(issue);
            }
        }
        validate_driver_name(&self.driver)?;
        if is_sql_server && is_legacy_sql_server_driver(&self.driver) {
            return Err(LEGACY_SQL_SERVER_DRIVER_MESSAGE.to_owned());
        }
        let server = trimmed(&self.server);
        if server.is_empty() {
            return Err("Indica el servidor antes de continuar.".to_owned());
        }
        let port = trimmed(&self.port);
        if is_sql_server {
            if !port.is_empty() {
                return Err("En SQL Server el puerto va en el servidor (por ejemplo, servidor,1433); deja el campo del puerto vacío.".to_owned());
            }
        } else if !port.is_empty()
            && (!port.chars().all(|character| character.is_ascii_digit())
                || !(1..=65_535).contains(&port.parse::<u32>().unwrap_or(0)))
        {
            return Err("El puerto es un número entre 1 y 65535.".to_owned());
        }
        match self.authentication {
            DatabaseAuthentication::Windows => {
                if !is_sql_server {
                    return Err(format!(
                        "{} no admite la autenticación de Windows; indica un usuario y una contraseña.",
                        kind.label()
                    ));
                }
                if !trimmed(&self.user).is_empty() || !self.password.is_empty() {
                    return Err("Con la autenticación de Windows no se indica usuario ni contraseña; bórralos o elige la autenticación con usuario.".to_owned());
                }
            }
            DatabaseAuthentication::Password => {
                if trimmed(&self.user).is_empty() {
                    return Err(format!("Indica el usuario de {}.", kind.label()));
                }
            }
        }
        if self.trust_server_certificate && !is_sql_server {
            return Err(format!(
                "Aceptar cualquier certificado solo se aplica a SQL Server, no a {}.",
                kind.label()
            ));
        }
        if self.allow_unencrypted && is_sql_server {
            return Err(
                "Conectar sin cifrado solo se aplica a PostgreSQL y MySQL; SQL Server siempre cifra."
                    .to_owned(),
            );
        }

        let mut attributes = vec![
            (
                "Driver",
                format!("{{{}}}", trimmed(&self.driver).replace('}', "}}")),
            ),
            ("Server", odbc_value(server)),
        ];
        if !port.is_empty() {
            attributes.push(("Port", port.to_owned()));
        }
        let database = trimmed(&self.database);
        if !database.is_empty() {
            attributes.push(("Database", odbc_value(database)));
        }
        match self.authentication {
            DatabaseAuthentication::Windows => {
                attributes.push(("Trusted_Connection", "yes".to_owned()));
            }
            DatabaseAuthentication::Password => {
                attributes.push(("Uid", odbc_value(trimmed(&self.user))));
                attributes.push(("Pwd", odbc_value(&self.password)));
            }
        }
        match kind {
            DatabaseKind::SqlServer => {
                attributes.push(("Encrypt", "yes".to_owned()));
                if self.trust_server_certificate {
                    attributes.push(("TrustServerCertificate", "yes".to_owned()));
                }
            }
            DatabaseKind::Postgresql => attributes.push((
                "SSLmode",
                if self.allow_unencrypted {
                    "prefer"
                } else {
                    "require"
                }
                .to_owned(),
            )),
            DatabaseKind::Mysql => attributes.push((
                "SSLMODE",
                if self.allow_unencrypted {
                    "PREFERRED"
                } else {
                    "REQUIRED"
                }
                .to_owned(),
            )),
        }
        Ok(attributes)
    }

    /// The connection string of a delivery.
    fn connection_string(&self, kind: DatabaseKind) -> Result<String, String> {
        self.connection_string_for(kind, ConnectionPurpose::Delivery)
    }

    /// The only place a connection string is made: from validated fields.
    /// Importing from SQL Server asks for a read-only connection
    /// (`ApplicationIntent=ReadOnly`); it is a request to the server, not a
    /// guarantee: that is the permission of the user the person signs in as.
    fn connection_string_for(
        &self,
        kind: DatabaseKind,
        purpose: ConnectionPurpose,
    ) -> Result<String, String> {
        let mut attributes = self.attributes(kind)?;
        if purpose == ConnectionPurpose::Import && kind == DatabaseKind::SqlServer {
            attributes.push(("ApplicationIntent", "ReadOnly".to_owned()));
        }
        let text = format!(
            "{};",
            attributes
                .iter()
                .map(|(key, value)| format!("{key}={value}"))
                .collect::<Vec<_>>()
                .join(";")
        );
        if text.chars().count() > MAX_CONNECTION_STRING_CHARS {
            return Err(format!(
                "Los datos de la conexión son demasiado largos: la cadena ODBC no puede superar {MAX_CONNECTION_STRING_CHARS} caracteres."
            ));
        }
        Ok(text)
    }

    fn validate(&self, kind: DatabaseKind) -> Result<(), String> {
        self.connection_string(kind).map(|_| ())
    }

    /// Every field, for the confirmation fingerprints.
    fn fingerprint_parts(&self) -> [&str; 9] {
        [
            &self.driver,
            &self.server,
            &self.port,
            &self.database,
            match self.authentication {
                DatabaseAuthentication::Windows => "windows",
                DatabaseAuthentication::Password => "password",
            },
            &self.user,
            &self.password,
            if self.trust_server_certificate {
                "1"
            } else {
                "0"
            },
            if self.allow_unencrypted { "1" } else { "0" },
        ]
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DatabaseTarget {
    pub kind: DatabaseKind,
    /// The fields of the connection. They are intentionally not persisted.
    pub connection: DatabaseConnection,
    pub schema: String,
    pub table: String,
    pub table_policy: DatabaseTablePolicy,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseConnectionResult {
    pub kind: DatabaseKind,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RemoteInputColumn {
    pub(crate) name: String,
    pub(crate) data_type: String,
    pub(crate) null_count: usize,
    pub(crate) maximum_length: Option<usize>,
    pub(crate) contains_nul: bool,
    pub(crate) unrepresentable_integer_count: usize,
    pub(crate) unrepresentable_decimal_count: usize,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RemoteExportPreflight {
    pub kind: DatabaseKind,
    pub schema: String,
    pub table: String,
    pub table_policy: DatabaseTablePolicy,
    pub table_exists: bool,
    pub ready: bool,
    pub issues: Vec<RemotePreflightIssue>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RemotePreflightIssue {
    pub severity: &'static str,
    pub category: &'static str,
    pub column: Option<String>,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ExistingColumn {
    name: String,
    data_type: String,
    maximum_length: Option<usize>,
    nullable: bool,
    has_default: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TypeFamily {
    Text,
    Boolean,
    Integer,
    Decimal,
    Temporal,
    Unknown,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct RemoteExportResult {
    pub(crate) rows_written: usize,
    pub(crate) table_name: String,
    pub(crate) format: &'static str,
}

#[tauri::command]
pub async fn test_database_connection(
    app: AppHandle,
    target: DatabaseTarget,
) -> Result<DatabaseConnectionResult, String> {
    validate_database_target(&target)?;
    let confirmation_app = app.clone();
    let confirmation_target = target.clone();
    tauri::async_runtime::spawn_blocking(move || {
        confirm_remote_target(&confirmation_app, &confirmation_target)
    })
    .await
    .map_err(|_| "La confirmación de la conexión se interrumpió.".to_owned())??;
    let generation = app.state::<DatasetState>().begin_database_connection();
    let cancellation_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        test_database_connection_with_cancel(&target, || {
            cancellation_app
                .state::<DatasetState>()
                .database_connection_was_cancelled(generation)
        })
    })
    .await
    .map_err(|_| "La prueba de conexión ODBC se interrumpió inesperadamente.".to_owned())?
}

fn test_database_connection_with_cancel<C>(
    target: &DatabaseTarget,
    is_cancelled: C,
) -> Result<DatabaseConnectionResult, String>
where
    C: Fn() -> bool,
{
    ensure_connection_test_not_cancelled(&is_cancelled)?;
    validate_database_target(target)?;

    let environment = Environment::new();
    ensure_connection_test_not_cancelled(&is_cancelled)?;
    let environment = environment
        .map_err(|error| format_driver_error("No se pudo inicializar ODBC", error, target))?;

    let connection = connect(&environment, target, ConnectionPurpose::Delivery);
    ensure_connection_test_not_cancelled(&is_cancelled)?;
    let connection = connection?;

    let query_result = connection.execute("SELECT 1", (), Some(10));
    ensure_connection_test_not_cancelled(&is_cancelled)?;
    query_result.map_err(|error| format_driver_error("La prueba SELECT 1 falló", error, target))?;

    Ok(DatabaseConnectionResult {
        kind: target.kind,
        message: format!("Conexión ODBC verificada para {}.", target.kind.label()),
    })
}

fn ensure_connection_test_not_cancelled(is_cancelled: &impl Fn() -> bool) -> Result<(), String> {
    if is_cancelled() {
        Err(OPERATION_CANCELLED_MESSAGE.to_owned())
    } else {
        Ok(())
    }
}

pub(crate) fn preflight_export_with_cancel<C>(
    target: &DatabaseTarget,
    input_columns: &[RemoteInputColumn],
    is_cancelled: &C,
) -> Result<RemoteExportPreflight, String>
where
    C: Fn() -> bool + Sync,
{
    ensure_not_cancelled(is_cancelled)?;
    validate_database_target(target)?;
    let environment = Environment::new()
        .map_err(|error| format_driver_error("No se pudo inicializar ODBC", error, target))?;
    let connection = connect(&environment, target, ConnectionPurpose::Delivery)?;
    ensure_not_cancelled(is_cancelled)?;
    connection
        .execute("SELECT 1", (), Some(10))
        .map_err(|error| format_driver_error("La prueba SELECT 1 falló", error, target))?;
    ensure_not_cancelled(is_cancelled)?;
    let (table_exists, is_base_table, existing_columns) = inspect_destination(&connection, target)?;
    ensure_not_cancelled(is_cancelled)?;
    Ok(assess_remote_export(
        target,
        input_columns,
        table_exists,
        is_base_table,
        &existing_columns,
    ))
}

pub(crate) fn preflight_source_backed<C>(
    source_path: &Path,
    source_format: DuckDbFileFormat,
    schema: &DataFrame,
    row_count: usize,
    target: &DatabaseTarget,
    is_cancelled: C,
) -> Result<RemoteExportPreflight, String>
where
    C: Fn() -> bool + Clone + Send + Sync + 'static,
{
    ensure_not_cancelled(&is_cancelled)?;
    let input_columns = input_shape_from_source(
        source_path,
        source_format,
        schema,
        row_count,
        is_cancelled.clone(),
    )?;
    ensure_not_cancelled(&is_cancelled)?;
    preflight_export_with_cancel(target, &input_columns, &is_cancelled)
}

#[cfg(test)]
pub(crate) fn input_shape_from_frame(frame: &DataFrame) -> Result<Vec<RemoteInputColumn>, String> {
    input_shape_from_frame_with_cancel(frame, &|| false)
}

pub(crate) fn input_shape_from_frame_with_cancel<C>(
    frame: &DataFrame,
    is_cancelled: &C,
) -> Result<Vec<RemoteInputColumn>, String>
where
    C: Fn() -> bool + Sync,
{
    ensure_not_cancelled(is_cancelled)?;
    let mut input_columns = Vec::with_capacity(frame.width());
    for column in frame.columns() {
        ensure_not_cancelled(is_cancelled)?;
        let data_type = column.dtype().to_string();
        let is_text = type_family(&data_type) == TypeFamily::Text;
        let normalized_dtype = data_type.to_ascii_lowercase();
        let is_integer = is_integer_dtype(&normalized_dtype);
        let is_decimal = is_decimal_dtype(&normalized_dtype);
        let mut null_count = 0usize;
        let mut maximum_length = 0usize;
        let mut contains_nul = false;
        let mut unrepresentable_integer_count = 0usize;
        let mut unrepresentable_decimal_count = 0usize;
        if is_text || is_integer || is_decimal {
            for row_index in 0..column.len() {
                if row_index.is_multiple_of(PREFLIGHT_CANCEL_CHECK_ROWS) {
                    ensure_not_cancelled(is_cancelled)?;
                }
                let value = column.get(row_index).map_err(|error| {
                    format!(
                        "No se pudo revisar la columna '{}' para la entrega: {error}",
                        column.name()
                    )
                })?;
                let text = match value {
                    AnyValue::Null => {
                        null_count += 1;
                        continue;
                    }
                    AnyValue::String(value) => value.to_owned(),
                    AnyValue::StringOwned(value) => value.as_str().to_owned(),
                    value => value.to_string(),
                };
                if is_text {
                    maximum_length = maximum_length.max(text.chars().count());
                    contains_nul |= text.contains('\0');
                }
                if is_integer && text.parse::<i64>().is_err() {
                    unrepresentable_integer_count = unrepresentable_integer_count.saturating_add(1);
                }
                if is_decimal && !matches!(text.parse::<f64>(), Ok(value) if value.is_finite()) {
                    unrepresentable_decimal_count = unrepresentable_decimal_count.saturating_add(1);
                }
            }
        } else {
            null_count = column.null_count();
        }
        ensure_not_cancelled(is_cancelled)?;
        input_columns.push(RemoteInputColumn {
            name: column.name().to_string(),
            data_type,
            null_count,
            maximum_length: is_text.then_some(maximum_length),
            contains_nul,
            unrepresentable_integer_count,
            unrepresentable_decimal_count,
        });
    }
    ensure_not_cancelled(is_cancelled)?;
    Ok(input_columns)
}

fn input_shape_from_source<C>(
    source_path: &Path,
    source_format: DuckDbFileFormat,
    schema: &DataFrame,
    expected_row_count: usize,
    is_cancelled: C,
) -> Result<Vec<RemoteInputColumn>, String>
where
    C: Fn() -> bool + Clone + Send + Sync + 'static,
{
    ensure_not_cancelled(&is_cancelled)?;
    let mut columns = schema
        .columns()
        .iter()
        .map(|column| RemoteInputColumn {
            name: column.name().to_string(),
            data_type: column.dtype().to_string(),
            null_count: 0,
            maximum_length: (type_family(&column.dtype().to_string()) == TypeFamily::Text)
                .then_some(0),
            contains_nul: false,
            unrepresentable_integer_count: 0,
            unrepresentable_decimal_count: 0,
        })
        .collect::<Vec<_>>();
    let integer_columns = columns
        .iter()
        .map(|column| is_integer_dtype(&column.data_type.to_ascii_lowercase()))
        .collect::<Vec<_>>();
    let decimal_columns = columns
        .iter()
        .map(|column| is_decimal_dtype(&column.data_type.to_ascii_lowercase()))
        .collect::<Vec<_>>();
    let mut row_count = 0usize;
    let streamed_columns = crate::duckdb_query::stream_file_rows(
        source_path,
        source_format,
        |values| {
            if values.len() != columns.len() {
                return Err("El ancho de la fuente cambió durante el preflight remoto.".to_owned());
            }
            for (index, (column, value)) in columns.iter_mut().zip(values.iter()).enumerate() {
                match value {
                    None => column.null_count = column.null_count.saturating_add(1),
                    Some(value) => {
                        if let Some(maximum_length) = column.maximum_length.as_mut() {
                            *maximum_length = (*maximum_length).max(value.chars().count());
                            column.contains_nul |= value.contains('\0');
                        }
                        if integer_columns[index] && value.parse::<i64>().is_err() {
                            column.unrepresentable_integer_count =
                                column.unrepresentable_integer_count.saturating_add(1);
                        }
                        if decimal_columns[index]
                            && !matches!(value.parse::<f64>(), Ok(number) if number.is_finite())
                        {
                            column.unrepresentable_decimal_count =
                                column.unrepresentable_decimal_count.saturating_add(1);
                        }
                    }
                }
            }
            row_count = row_count.saturating_add(1);
            Ok(())
        },
        is_cancelled.clone(),
    )?;
    ensure_not_cancelled(&is_cancelled)?;
    if row_count != expected_row_count {
        return Err(
            "El conteo de filas cambió durante el preflight remoto; vuelve a revisar el dataset."
                .to_owned(),
        );
    }
    let expected_names = columns
        .iter()
        .map(|column| column.name.as_str())
        .collect::<Vec<_>>();
    let actual_names = streamed_columns
        .iter()
        .map(|column| column.name.as_str())
        .collect::<Vec<_>>();
    if !same_streamed_names(&expected_names, &actual_names) {
        return Err("El esquema source-backed cambió durante el preflight remoto.".to_owned());
    }
    Ok(columns)
}

/// Whether the file streamed the expected columns. DuckDB renames a name
/// that repeats another but for case (`dup` after `Dup` becomes `dup_1`), so
/// those are compared by position (FUN-21).
fn same_streamed_names(expected: &[&str], actual: &[&str]) -> bool {
    expected.len() == actual.len()
        && expected
            .iter()
            .zip(actual)
            .enumerate()
            .all(|(index, (wanted, streamed))| {
                wanted == streamed
                    || expected.iter().enumerate().any(|(other_index, other)| {
                        other_index != index && other.to_lowercase() == wanted.to_lowercase()
                    })
            })
}

fn inspect_destination(
    connection: &odbc_api::Connection<'_>,
    target: &DatabaseTarget,
) -> Result<(bool, bool, Vec<ExistingColumn>), String> {
    let table_rows = connection
        .tables("", &target.schema, &target.table, "")
        .map_err(|error| {
            format_driver_error("No se pudo revisar la política del destino", error, target)
        })?
        .collect::<Result<Vec<TablesRow>, _>>()
        .map_err(|error| {
            format_driver_error("No se pudo leer la política del destino", error, target)
        })?;
    let matching_objects = table_rows
        .iter()
        .filter(|row| {
            let table_name = row.table.as_str().ok().flatten().unwrap_or_default();
            let schema_name = row.schema.as_str().ok().flatten().unwrap_or_default();
            table_name.eq_ignore_ascii_case(&target.table)
                && (target.schema.trim().is_empty()
                    || schema_name.eq_ignore_ascii_case(&target.schema))
        })
        .collect::<Vec<_>>();
    let table_exists = !matching_objects.is_empty();
    let is_base_table = matching_objects.iter().any(|row| {
        row.table_type
            .as_str()
            .ok()
            .flatten()
            .unwrap_or_default()
            .to_ascii_lowercase()
            .contains("table")
    });
    if !table_exists || !is_base_table {
        return Ok((table_exists, is_base_table, Vec::new()));
    }

    let columns = connection
        .columns("", &target.schema, &target.table, "")
        .map_err(|error| {
            format_driver_error("No se pudo revisar el esquema remoto", error, target)
        })?
        .collect::<Result<Vec<ColumnsRow>, _>>()
        .map_err(|error| format_driver_error("No se pudo leer el esquema remoto", error, target))?
        .into_iter()
        .map(|column| {
            let maximum_length = column
                .column_size
                .into_opt()
                .and_then(|value| usize::try_from(value).ok())
                .filter(|value| *value > 0);
            let nullable = column
                .is_nullable
                .as_str()
                .ok()
                .flatten()
                .is_none_or(|value| !value.eq_ignore_ascii_case("NO"));
            let has_default = column
                .column_default
                .as_str()
                .ok()
                .flatten()
                .is_some_and(|value| !value.trim().is_empty());
            ExistingColumn {
                name: column
                    .column_name
                    .as_str()
                    .ok()
                    .flatten()
                    .unwrap_or_default()
                    .to_owned(),
                data_type: column
                    .type_name
                    .as_str()
                    .ok()
                    .flatten()
                    .unwrap_or_default()
                    .to_owned(),
                maximum_length,
                nullable,
                has_default,
            }
        })
        .collect::<Vec<_>>();
    if columns.is_empty() {
        return Err(
            "El controlador no devolvió el esquema de la tabla. La entrega se detuvo antes de escribir."
                .to_owned(),
        );
    }
    Ok((table_exists, is_base_table, columns))
}

/// SEG-09: whether the server is this machine. The host is compared whole,
/// after removing the protocol (`tcp:`), the port (`,1433` or `:5432`) and the
/// instance (`\SQLEXPRESS`), so `localhost.example.com` is a remote host.
fn is_local_server(server: &str) -> bool {
    let server = server.trim();
    let server = server
        .split_once(':')
        .filter(|(protocol, _)| matches!(*protocol, "tcp" | "np" | "lpc" | "admin"))
        .map_or(server, |(_, rest)| rest);
    let host = server.split([',', '\\']).next().unwrap_or_default().trim();
    let host = match host.strip_prefix('[') {
        Some(bracketed) => bracketed.split(']').next().unwrap_or_default(),
        None if host.matches(':').count() == 1 => host.split(':').next().unwrap_or_default(),
        None => host,
    };
    matches!(
        host,
        "localhost" | "127.0.0.1" | "::1" | "(local)" | "(localdb)" | "."
    )
}

/// Personal data may travel to the remote server; the delivery must state
/// whether the channel is encrypted. Rust builds the connection string, so
/// the channel is always encrypted unless the person allowed otherwise: that
/// stays visible as a warning, and a local server is noted.
fn transport_encryption_issue(target: &DatabaseTarget) -> Option<(&'static str, String)> {
    let connection = &target.connection;
    let label = target.kind.label();
    let is_sql_server = target.kind == DatabaseKind::SqlServer;
    if is_sql_server || !connection.allow_unencrypted {
        return (is_sql_server && connection.trust_server_certificate).then(|| ("warning", format!(
            "La conexión con {label} cifra el canal pero acepta cualquier certificado (TrustServerCertificate); valida el certificado del servidor si los datos contienen información personal."
        )));
    }
    if is_local_server(&connection.server) {
        return Some((
            "info",
            format!("La conexión con {label} apunta a este equipo y no exige cifrado del canal."),
        ));
    }
    Some(("warning", format!(
        "Se permitió conectar sin cifrado con {label}: el cifrado es oportunista, así que si el servidor no lo ofrece, o alguien con acceso a la red lo impide, los datos viajarán sin cifrar."
    )))
}

fn assess_remote_export(
    target: &DatabaseTarget,
    input_columns: &[RemoteInputColumn],
    table_exists: bool,
    is_base_table: bool,
    existing_columns: &[ExistingColumn],
) -> RemoteExportPreflight {
    let mut issues = Vec::new();
    if let Some((severity, message)) = transport_encryption_issue(target) {
        push_issue(&mut issues, severity, "policy", None, message);
    }
    if input_columns.is_empty() {
        push_issue(
            &mut issues,
            "blocking",
            "type",
            None,
            "El dataset no contiene columnas para entregar.".to_owned(),
        );
    }
    for issue in column_name_issues(target.kind, input_columns) {
        push_issue(&mut issues, "blocking", "type", Some(issue.0), issue.1);
    }
    for column in input_columns {
        if column.unrepresentable_integer_count > 0 {
            push_issue(
                &mut issues,
                "blocking",
                "value",
                Some(column.name.clone()),
                format!(
                    "La columna '{}' tiene {} entero(s) que no caben en el parámetro Int64 usado por ODBC; no se escribirán como NULL.",
                    column.name, column.unrepresentable_integer_count
                ),
            );
        }
        if column.unrepresentable_decimal_count > 0 {
            push_issue(
                &mut issues,
                "blocking",
                "value",
                Some(column.name.clone()),
                format!(
                    "La columna '{}' tiene {} valor(es) numérico(s) no finito(s) o no representable(s) por ODBC; no se escribirán como NULL.",
                    column.name, column.unrepresentable_decimal_count
                ),
            );
        }
    }
    if table_exists && !is_base_table {
        push_issue(
            &mut issues,
            "blocking",
            "policy",
            None,
            "El destino existe, pero no se identificó como una tabla escribible.".to_owned(),
        );
    }
    match (target.table_policy, table_exists) {
        (DatabaseTablePolicy::Append, false) => push_issue(
            &mut issues,
            "blocking",
            "policy",
            None,
            "Añadir a tabla existente requiere que el destino ya exista.".to_owned(),
        ),
        (DatabaseTablePolicy::CreateOnly, true) => push_issue(
            &mut issues,
            "blocking",
            "policy",
            None,
            "Crear; fallar si existe está seleccionado y la tabla ya existe.".to_owned(),
        ),
        (DatabaseTablePolicy::Replace, true) => push_issue(
            &mut issues,
            "warning",
            "policy",
            None,
            "La tabla existente se eliminará y se volverá a crear. La política Reemplazar fue seleccionada explícitamente.".to_owned(),
        ),
        (DatabaseTablePolicy::CreateOnly | DatabaseTablePolicy::Replace, false) => push_issue(
            &mut issues,
            "info",
            "policy",
            None,
            "El destino no existe; se creará una tabla nueva.".to_owned(),
        ),
        (DatabaseTablePolicy::Append, true) => {}
    }

    if table_exists && is_base_table && target.table_policy == DatabaseTablePolicy::Append {
        for source in input_columns {
            let destination = existing_columns
                .iter()
                .find(|column| column.name == source.name);
            let Some(destination) = destination else {
                push_issue(
                    &mut issues,
                    "blocking",
                    "type",
                    Some(source.name.clone()),
                    format!(
                        "La tabla remota no contiene la columna '{}' con ese nombre exacto.",
                        source.name
                    ),
                );
                continue;
            };
            if !remote_types_compatible(&source.data_type, &destination.data_type) {
                push_issue(
                    &mut issues,
                    "blocking",
                    "type",
                    Some(source.name.clone()),
                    format!(
                        "La columna '{}' llega como {} y el destino usa {}; no se garantiza una conversión segura.",
                        source.name, source.data_type, destination.data_type
                    ),
                );
            }
            if source.null_count > 0 && !destination.nullable {
                push_issue(
                    &mut issues,
                    "blocking",
                    "nullability",
                    Some(source.name.clone()),
                    format!(
                        "La columna '{}' contiene {} nulos y el destino los rechaza.",
                        source.name, source.null_count
                    ),
                );
            }
            if source.contains_nul {
                push_issue(
                    &mut issues,
                    "blocking",
                    "value",
                    Some(source.name.clone()),
                    format!(
                        "La columna '{}' contiene caracteres NUL no compatibles con ODBC.",
                        source.name
                    ),
                );
            }
            if type_family(&source.data_type) == TypeFamily::Text {
                match (source.maximum_length, destination.maximum_length) {
                    (Some(actual), Some(limit)) if actual > limit => push_issue(
                        &mut issues,
                        "blocking",
                        "length",
                        Some(source.name.clone()),
                        format!(
                            "La columna '{}' alcanza {} caracteres y el destino admite {}.",
                            source.name, actual, limit
                        ),
                    ),
                    (None, Some(_)) => push_issue(
                        &mut issues,
                        "blocking",
                        "length",
                        Some(source.name.clone()),
                        format!(
                            "No se pudo verificar la longitud máxima de '{}'; se requiere un destino sin límite o revisar la columna.",
                            source.name
                        ),
                    ),
                    _ => {}
                }
            }
        }
        for destination in existing_columns {
            if input_columns
                .iter()
                .any(|source| source.name == destination.name)
            {
                continue;
            }
            if !destination.nullable && !destination.has_default {
                push_issue(
                    &mut issues,
                    "blocking",
                    "nullability",
                    Some(destination.name.clone()),
                    format!(
                        "La columna adicional '{}' es obligatoria y no tiene valor predeterminado; la inserción no podría completarla.",
                        destination.name
                    ),
                );
            }
        }
    }

    let ready = !issues.iter().any(|issue| issue.severity == "blocking");
    RemoteExportPreflight {
        kind: target.kind,
        schema: target.schema.clone(),
        table: target.table.clone(),
        table_policy: target.table_policy,
        table_exists,
        ready,
        issues,
    }
}

/// Column names the engine cannot create (FUN-20, FUN-21): longer than its
/// identifier limit, or equal to another name but for case where the engine
/// does not tell them apart. Returns the column and the message.
fn column_name_issues(kind: DatabaseKind, columns: &[RemoteInputColumn]) -> Vec<(String, String)> {
    let mut issues = Vec::new();
    for column in columns {
        let (length, limit, unit) = match kind {
            DatabaseKind::SqlServer => (column.name.chars().count(), 128, "caracteres"),
            DatabaseKind::Mysql => (column.name.chars().count(), 64, "caracteres"),
            DatabaseKind::Postgresql => (column.name.len(), 63, "bytes"),
        };
        if length > limit {
            issues.push((
                column.name.clone(),
                format!(
                    "El nombre de la columna '{}' tiene {length} {unit}; {} admite como máximo {limit}. Acórtalo en Preparar antes de entregar.",
                    column.name,
                    kind.label()
                ),
            ));
        }
    }
    if kind != DatabaseKind::Postgresql {
        for (index, column) in columns.iter().enumerate() {
            if let Some(other) = columns[..index]
                .iter()
                .find(|other| other.name.to_lowercase() == column.name.to_lowercase())
            {
                issues.push((
                    column.name.clone(),
                    format!(
                        "Las columnas '{}' y '{}' solo se diferencian en mayúsculas y {} no las distingue. Renombra una de ellas en Preparar.",
                        other.name,
                        column.name,
                        kind.label()
                    ),
                ));
            }
        }
    }
    issues
}

fn push_issue(
    issues: &mut Vec<RemotePreflightIssue>,
    severity: &'static str,
    category: &'static str,
    column: Option<String>,
    message: String,
) {
    issues.push(RemotePreflightIssue {
        severity,
        category,
        column,
        message,
    });
}

fn remote_types_compatible(source_type: &str, destination_type: &str) -> bool {
    let source = type_family(source_type);
    let destination = type_family(destination_type);
    match source {
        TypeFamily::Integer => matches!(destination, TypeFamily::Integer | TypeFamily::Decimal),
        TypeFamily::Decimal => matches!(destination, TypeFamily::Decimal),
        TypeFamily::Boolean => matches!(destination, TypeFamily::Boolean),
        TypeFamily::Temporal => matches!(destination, TypeFamily::Temporal | TypeFamily::Text),
        TypeFamily::Text => matches!(destination, TypeFamily::Text),
        TypeFamily::Unknown => false,
    }
}

/// Classifies both Polars dtype names (`str`, `f64`, `datetime[μs]`) and the
/// type names that ODBC catalogs report (`nvarchar(10)`, `double precision`).
/// Only the base name counts, so `interval` or `point` are not integers.
fn type_family(data_type: &str) -> TypeFamily {
    let normalized = data_type.trim().to_ascii_lowercase();
    let base = normalized.split(['(', '[', ' ']).next().unwrap_or_default();
    match base {
        "bool" | "boolean" | "bit" => TypeFamily::Boolean,
        "i8" | "i16" | "i32" | "i64" | "i128" | "u8" | "u16" | "u32" | "u64" | "int"
        | "integer" | "tinyint" | "smallint" | "mediumint" | "bigint" | "int1" | "int2"
        | "int4" | "int8" | "int16" | "int32" | "int64" | "uint8" | "uint16" | "uint32"
        | "uint64" | "serial" | "smallserial" | "bigserial" => TypeFamily::Integer,
        "f32" | "f64" | "float" | "float4" | "float8" | "float32" | "float64" | "real"
        | "double" | "decimal" | "numeric" | "number" | "money" | "smallmoney" => {
            TypeFamily::Decimal
        }
        "date" | "datetime" | "datetime2" | "smalldatetime" | "datetimeoffset" | "timestamp"
        | "timestamptz" | "time" | "timetz" => TypeFamily::Temporal,
        "str" | "string" | "utf8" | "text" | "ntext" | "tinytext" | "mediumtext" | "longtext"
        | "citext" | "char" | "nchar" | "character" | "varchar" | "nvarchar" | "varchar2"
        | "nvarchar2" | "clob" | "nclob" | "cat" | "categorical" | "enum" => TypeFamily::Text,
        _ => TypeFamily::Unknown,
    }
}

pub(crate) fn validate_database_target(target: &DatabaseTarget) -> Result<(), String> {
    target.connection.validate(target.kind)?;
    validate_identifier(&target.schema, "el esquema", true)?;
    validate_identifier(&target.table, "la tabla", false)?;
    if target.kind == DatabaseKind::Mysql && target.table_policy == DatabaseTablePolicy::Replace {
        return Err(
            "La política Reemplazar está deshabilitada para MySQL hasta validar una sustitución atómica segura."
                .to_owned(),
        );
    }
    Ok(())
}

pub(crate) fn export_frame<F, C>(
    frame: &DataFrame,
    target: &DatabaseTarget,
    mut report: F,
    is_cancelled: C,
    mut journal: impl DeliveryJournal,
) -> Result<RemoteExportResult, String>
where
    F: FnMut(&'static str, u8),
    C: Fn() -> bool + Sync,
{
    validate_database_target(target)?;
    ensure_not_cancelled(&is_cancelled)?;
    let input_columns = input_shape_from_frame_with_cancel(frame, &is_cancelled)?;
    let environment = Environment::new()
        .map_err(|error| format_driver_error("No se pudo inicializar ODBC", error, target))?;
    let connection = connect(&environment, target, ConnectionPurpose::Delivery)?;
    ensure_not_cancelled(&is_cancelled)?;
    connection
        .execute("SELECT 1", (), Some(10))
        .map_err(|error| format_driver_error("La prueba SELECT 1 falló", error, target))?;
    ensure_not_cancelled(&is_cancelled)?;
    let (table_exists, is_base_table, existing_columns) = inspect_destination(&connection, target)?;
    ensure_not_cancelled(&is_cancelled)?;
    let preflight = assess_remote_export(
        target,
        &input_columns,
        table_exists,
        is_base_table,
        &existing_columns,
    );
    ensure_preflight_ready(&preflight)?;
    connection
        .set_autocommit(false)
        .map_err(|error| format_driver_error("No se pudo iniciar la transacción", error, target))?;

    let table = qualified_table(target);
    let columns = frame
        .get_column_names()
        .iter()
        .map(|name| quote_identifier(name, target.kind))
        .collect::<Vec<_>>();
    if columns.is_empty() {
        return Err("No se puede entregar un dataset sin columnas.".to_owned());
    }

    report("Preparando tabla remota", 10);
    if matches!(target.table_policy, DatabaseTablePolicy::Replace) {
        execute_statement(
            &connection,
            &drop_table_sql(&table, target.kind),
            "No se pudo reemplazar la tabla remota",
            target,
        )?;
    }
    if !matches!(target.table_policy, DatabaseTablePolicy::Append) {
        execute_statement(
            &connection,
            &create_table_sql(frame, &table, target.kind),
            "No se pudo crear la tabla remota",
            target,
        )?;
    }

    report("Escribiendo filas remotas", 25);
    let written = (|| -> Result<usize, String> {
        let mut inserter = BatchInserter::new(&connection, &table, columns, target)?;
        let mut rows_written = 0usize;
        for row_index in 0..frame.height() {
            ensure_not_cancelled(&is_cancelled)?;
            let params = frame
                .columns()
                .iter()
                .map(|column| {
                    let value = column
                        .get(row_index)
                        .map_err(|error| format!("No se pudo leer la fila {row_index}: {error}"))?;
                    sql_parameter(value, column.dtype())
                })
                .collect::<Result<Vec<_>, String>>()?;
            let sent = inserter.push_row(params)?;
            if sent > 0 {
                rows_written += sent;
                let percent = 25 + (rows_written * 60 / frame.height()).min(60) as u8;
                report("Escribiendo filas remotas", percent);
            }
        }
        ensure_not_cancelled(&is_cancelled)?;
        rows_written += inserter.finish()?;
        ensure_not_cancelled(&is_cancelled)?;
        Ok(rows_written)
    })();
    let rows_written =
        written.map_err(|error| undo_created_mysql_table(&connection, target, &table, error))?;
    commit_delivery(&connection, &mut journal, rows_written, target)?;
    report("Entrega remota lista", 100);
    Ok(RemoteExportResult {
        rows_written,
        table_name: table,
        format: target.kind.label(),
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn export_source_backed<F, C>(
    source_path: &Path,
    source_format: DuckDbFileFormat,
    schema: &DataFrame,
    row_count: usize,
    target: &DatabaseTarget,
    mut report: F,
    is_cancelled: C,
    mut journal: impl DeliveryJournal,
) -> Result<RemoteExportResult, String>
where
    F: FnMut(&'static str, u8),
    C: Fn() -> bool + Clone + Send + Sync + 'static,
{
    validate_database_target(target)?;
    ensure_not_cancelled(&is_cancelled)?;
    if schema.width() == 0 {
        return Err("No se puede entregar un dataset sin columnas.".to_owned());
    }
    let input_columns = input_shape_from_source(
        source_path,
        source_format,
        schema,
        row_count,
        is_cancelled.clone(),
    )?;
    let environment = Environment::new()
        .map_err(|error| format_driver_error("No se pudo inicializar ODBC", error, target))?;
    let connection = connect(&environment, target, ConnectionPurpose::Delivery)?;
    connection
        .execute("SELECT 1", (), Some(10))
        .map_err(|error| format_driver_error("La prueba SELECT 1 falló", error, target))?;
    let (table_exists, is_base_table, existing_columns) = inspect_destination(&connection, target)?;
    let preflight = assess_remote_export(
        target,
        &input_columns,
        table_exists,
        is_base_table,
        &existing_columns,
    );
    ensure_preflight_ready(&preflight)?;
    connection
        .set_autocommit(false)
        .map_err(|error| format_driver_error("No se pudo iniciar la transacción", error, target))?;

    let table = qualified_table(target);
    let columns = schema
        .columns()
        .iter()
        .map(|column| quote_identifier(column.name(), target.kind))
        .collect::<Vec<_>>();
    report("Preparando tabla remota", 10);
    if matches!(target.table_policy, DatabaseTablePolicy::Replace) {
        execute_statement(
            &connection,
            &drop_table_sql(&table, target.kind),
            "No se pudo reemplazar la tabla remota",
            target,
        )?;
    }
    if !matches!(target.table_policy, DatabaseTablePolicy::Append) {
        execute_statement(
            &connection,
            &create_table_sql(schema, &table, target.kind),
            "No se pudo crear la tabla remota",
            target,
        )?;
    }

    report("Escribiendo filas remotas", 25);
    let written = (|| -> Result<usize, String> {
        let mut inserter = BatchInserter::new(&connection, &table, columns, target)?;
        let mut rows_written = 0usize;
        let streamed_columns = crate::duckdb_query::stream_file_rows(
            source_path,
            source_format,
            |values| {
                ensure_not_cancelled(&is_cancelled)?;
                if values.len() != schema.width() {
                    return Err(
                        "La transmisión source-backed devolvió un ancho inesperado para ODBC."
                            .to_owned(),
                    );
                }
                let params = schema
                    .columns()
                    .iter()
                    .zip(values.iter())
                    .map(|(column, value)| sql_parameter_text(value.as_deref(), column.dtype()))
                    .collect::<Result<Vec<_>, String>>()?;
                let sent = inserter.push_row(params)?;
                if sent == 0 {
                    return Ok(());
                }
                rows_written = rows_written.saturating_add(sent);
                let percent = if row_count == 0 {
                    85
                } else {
                    25 + (rows_written
                        .saturating_mul(60)
                        .checked_div(row_count)
                        .unwrap_or_default())
                    .min(60) as u8
                };
                report("Escribiendo filas remotas", percent);
                Ok(())
            },
            is_cancelled.clone(),
        )?;
        ensure_not_cancelled(&is_cancelled)?;
        rows_written = rows_written.saturating_add(inserter.finish()?);
        let expected_columns = schema
            .columns()
            .iter()
            .map(|column| column.name().to_string())
            .collect::<Vec<_>>();
        let actual_columns = streamed_columns
            .iter()
            .map(|column| column.name.clone())
            .collect::<Vec<_>>();
        let expected_names = expected_columns
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        let actual_names = actual_columns
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        if !same_streamed_names(&expected_names, &actual_names) {
            return Err(
                "La transmisión source-backed devolvió columnas distintas al esquema ODBC."
                    .to_owned(),
            );
        }
        if rows_written != row_count {
            return Err(
                "El conteo del dataset source-backed cambió durante la entrega ODBC.".to_owned(),
            );
        }
        ensure_not_cancelled(&is_cancelled)?;
        Ok(rows_written)
    })();
    let rows_written =
        written.map_err(|error| undo_created_mysql_table(&connection, target, &table, error))?;
    commit_delivery(&connection, &mut journal, rows_written, target)?;
    report("Entrega remota lista", 100);
    Ok(RemoteExportResult {
        rows_written,
        table_name: table,
        format: target.kind.label(),
    })
}

fn ensure_preflight_ready(preflight: &RemoteExportPreflight) -> Result<(), String> {
    if preflight.ready {
        return Ok(());
    }
    let blocking = preflight
        .issues
        .iter()
        .filter(|issue| issue.severity == "blocking")
        .map(|issue| issue.message.as_str())
        .collect::<Vec<_>>();
    Err(format!(
        "Preflight remoto bloqueó la entrega antes de escribir: {}",
        blocking.join(" ")
    ))
}

fn execute_statement(
    connection: &odbc_api::Connection<'_>,
    statement: &str,
    context: &str,
    target: &DatabaseTarget,
) -> Result<(), String> {
    connection
        .execute(statement, (), Some(ODBC_STATEMENT_TIMEOUT_SEC))
        .map(|_| ())
        .map_err(|error| format_driver_error(context, error, target))
}

fn parameterized_insert_sql(table: &str, columns: &[String], row_count: usize) -> String {
    let row = format!("({})", vec!["?"; columns.len()].join(", "));
    format!(
        "INSERT INTO {table} ({}) VALUES {}",
        columns.join(", "),
        vec![row; row_count.max(1)].join(", ")
    )
}

fn ensure_not_cancelled<C>(is_cancelled: &C) -> Result<(), String>
where
    C: Fn() -> bool,
{
    if is_cancelled() {
        Err(crate::dataset::OPERATION_CANCELLED_MESSAGE.to_owned())
    } else {
        Ok(())
    }
}

fn validate_identifier(value: &str, label: &str, optional: bool) -> Result<(), String> {
    if optional && value.trim().is_empty() {
        return Ok(());
    }
    let value = value.trim();
    if value.is_empty()
        || value.chars().count() > MAX_IDENTIFIER_CHARS
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
        || value
            .chars()
            .next()
            .is_none_or(|character| !character.is_ascii_alphabetic() && character != '_')
    {
        return Err(format!(
            "{label} solo puede contener letras ASCII, números y guiones bajos, y debe comenzar por letra o guion bajo."
        ));
    }
    Ok(())
}

fn quote_identifier(value: &str, kind: DatabaseKind) -> String {
    match kind {
        DatabaseKind::Postgresql => format!("\"{}\"", value.replace('"', "\"\"")),
        DatabaseKind::Mysql => format!("`{}`", value.replace('`', "``")),
        DatabaseKind::SqlServer => format!("[{}]", value.replace(']', "]]")),
    }
}

fn qualified_table(target: &DatabaseTarget) -> String {
    if target.schema.trim().is_empty() {
        quote_identifier(&target.table, target.kind)
    } else {
        format!(
            "{}.{}",
            quote_identifier(&target.schema, target.kind),
            quote_identifier(&target.table, target.kind)
        )
    }
}

fn drop_table_sql(table: &str, kind: DatabaseKind) -> String {
    match kind {
        DatabaseKind::Postgresql | DatabaseKind::Mysql => {
            format!("DROP TABLE IF EXISTS {table}")
        }
        DatabaseKind::SqlServer => format!(
            "IF OBJECT_ID(N'{escaped}', N'U') IS NOT NULL DROP TABLE {table}",
            escaped = table.replace('\'', "''")
        ),
    }
}

fn create_table_sql(frame: &DataFrame, table: &str, kind: DatabaseKind) -> String {
    let columns = frame
        .columns()
        .iter()
        .map(|column| {
            format!(
                "{} {}",
                quote_identifier(column.name(), kind),
                database_type(column.dtype(), kind)
            )
        })
        .collect::<Vec<_>>();
    format!("CREATE TABLE {table} ({})", columns.join(", "))
}

fn database_type(dtype: &polars::prelude::DataType, kind: DatabaseKind) -> &'static str {
    // FUN-66: a time of day is a TIME, not a DATETIME2/TIMESTAMP. (This
    // Polars build has no Decimal type: decimals arrive as f64, see FUN-57.)
    if matches!(dtype, polars::prelude::DataType::Time) {
        return "TIME";
    }
    let dtype = dtype.to_string().to_ascii_lowercase();
    if dtype.contains("bool") {
        return match kind {
            DatabaseKind::SqlServer => "BIT",
            _ => "BOOLEAN",
        };
    }
    if is_integer_dtype(&dtype) {
        return "BIGINT";
    }
    if is_decimal_dtype(&dtype) {
        return match kind {
            DatabaseKind::SqlServer => "FLOAT",
            _ => "DOUBLE PRECISION",
        };
    }
    if dtype.contains("date") || dtype.contains("datetime") || dtype.contains("time") {
        return match kind {
            DatabaseKind::SqlServer => "DATETIME2",
            _ => "TIMESTAMP",
        };
    }
    match kind {
        DatabaseKind::SqlServer => "NVARCHAR(MAX)",
        DatabaseKind::Postgresql => "TEXT",
        DatabaseKind::Mysql => "LONGTEXT",
    }
}

fn sql_parameter(
    value: AnyValue<'_>,
    dtype: &polars::prelude::DataType,
) -> Result<Box<dyn InputParameter>, String> {
    if matches!(value, AnyValue::Null) {
        return sql_parameter_text(None, dtype);
    }
    let text = match value {
        AnyValue::String(value) => value.to_owned(),
        AnyValue::StringOwned(value) => value.to_string(),
        // FUN-66: a zoned datetime travels as its UTC wall time, without the
        // zone suffix no TIMESTAMP/DATETIME2 column reads; a time of day keeps
        // at most microseconds, which every server accepts.
        AnyValue::Datetime(value, unit, Some(_)) => utc_datetime_text(value, unit),
        AnyValue::DatetimeOwned(value, unit, Some(_)) => utc_datetime_text(value, unit),
        AnyValue::Time(nanoseconds) => time_of_day_text(nanoseconds),
        value => value.to_string(),
    };
    sql_parameter_text(Some(&text), dtype)
}

fn utc_datetime_text(value: i64, unit: polars::prelude::TimeUnit) -> String {
    use polars::prelude::TimeUnit;
    let nanoseconds = match unit {
        TimeUnit::Nanoseconds => Some(value),
        TimeUnit::Microseconds => value.checked_mul(1_000),
        TimeUnit::Milliseconds => value.checked_mul(1_000_000),
    };
    nanoseconds
        .map(chrono::DateTime::from_timestamp_nanos)
        .map(|datetime| {
            datetime
                .naive_utc()
                .format("%Y-%m-%d %H:%M:%S%.6f")
                .to_string()
        })
        .unwrap_or_else(|| value.to_string())
}

fn time_of_day_text(nanoseconds: i64) -> String {
    let seconds = nanoseconds.div_euclid(1_000_000_000);
    let micros = nanoseconds.rem_euclid(1_000_000_000) / 1_000;
    chrono::NaiveTime::from_num_seconds_from_midnight_opt(
        u32::try_from(seconds).unwrap_or_default(),
        u32::try_from(micros * 1_000).unwrap_or_default(),
    )
    .map(|time| time.format("%H:%M:%S%.f").to_string())
    .unwrap_or_else(|| nanoseconds.to_string())
}

fn sql_parameter_text(
    value: Option<&str>,
    dtype: &polars::prelude::DataType,
) -> Result<Box<dyn InputParameter>, String> {
    let normalized_dtype = dtype.to_string().to_ascii_lowercase();
    let Some(text) = value else {
        return Ok(if normalized_dtype.contains("bool") {
            Box::new(Nullable::<Bit>::null())
        } else if is_integer_dtype(&normalized_dtype) {
            Box::new(Nullable::<i64>::null())
        } else if is_decimal_dtype(&normalized_dtype) {
            Box::new(Nullable::<f64>::null())
        } else {
            Box::new(None::<String>.into_parameter())
        });
    };
    if normalized_dtype.contains("bool") {
        return Ok(Box::new(Bit::from_bool(parse_boolean_parameter(text)?)));
    }
    if is_integer_dtype(&normalized_dtype) || is_decimal_dtype(&normalized_dtype) {
        if is_integer_dtype(&normalized_dtype) {
            let value = text.parse::<i64>().map_err(|_| {
                "Una celda entera no cabe en el parámetro Int64 usado por ODBC.".to_owned()
            })?;
            return Ok(Box::new(value));
        }
        let value = text
            .parse::<f64>()
            .map_err(|_| "Una celda decimal no puede representarse como número ODBC.".to_owned())?;
        if !value.is_finite() {
            return Err("Una celda decimal no finita no se puede escribir con ODBC.".to_owned());
        }
        return Ok(Box::new(value));
    }
    if text.contains('\0') {
        return Err("Una celda contiene un carácter NUL no compatible con SQL/ODBC.".to_owned());
    }
    Ok(Box::new(text.to_owned().into_parameter()))
}

fn parse_boolean_parameter(value: &str) -> Result<bool, String> {
    match value.to_ascii_lowercase().as_str() {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err("Se encontró un booleano incompatible al entregar la tabla remota.".to_owned()),
    }
}

fn is_integer_dtype(dtype: &str) -> bool {
    dtype.contains("int")
        || dtype.contains("uint")
        || matches!(
            dtype,
            "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64"
        )
}

fn is_decimal_dtype(dtype: &str) -> bool {
    dtype.contains("float")
        || dtype.contains("decimal")
        || dtype.contains("double")
        || matches!(dtype, "f32" | "f64")
}

/// The password, as the person typed it and as it is written in the
/// connection string (braced, `}` doubled), longest first so a longer form is
/// never left half redacted by a shorter one.
fn password_forms(password: &str) -> Vec<String> {
    let written = odbc_value(password);
    let inner = written
        .strip_prefix('{')
        .and_then(|rest| rest.strip_suffix('}'))
        .unwrap_or(&written)
        .to_owned();
    let mut forms = vec![written, inner, password.to_owned()];
    forms.retain(|form| !form.is_empty());
    forms.sort_by_key(|form| std::cmp::Reverse(form.len()));
    forms.dedup();
    forms
}

fn format_driver_error<E: Debug>(context: &str, error: E, target: &DatabaseTarget) -> String {
    let mut detail = format!("{error:?}");
    // Redact both the written form and the bare value the driver may echo.
    for form in password_forms(&target.connection.password) {
        detail = detail.replace(&form, "[REDACTED]");
    }
    let readable = readable_driver_detail(&detail);
    if detail.contains("HYT00") || detail.contains("HYT01") {
        return format!(
            "{context} para {}: la base remota no respondió a tiempo (conexión {ODBC_LOGIN_TIMEOUT_SEC} s, lista de tablas {ODBC_CATALOG_TIMEOUT_SEC} s, lote {ODBC_BATCH_TIMEOUT_SEC} s, sentencia {ODBC_STATEMENT_TIMEOUT_SEC} s). Comprueba la red o los bloqueos de la tabla y vuelve a intentarlo. {readable}",
            target.kind.label()
        );
    }
    format!("{context} para {}: {readable}", target.kind.label())
}

/// odbc-api's `Debug` dump («Diagnostics { record: State: 08S01, … }») as a
/// sentence: what happened, then the server's own message and codes (UX-04).
fn readable_driver_detail(detail: &str) -> String {
    let capture = |pattern: &str| {
        regex::Regex::new(pattern)
            .ok()
            .and_then(|regex| regex.captures(detail))
            .and_then(|captures| captures.get(1))
            .map(|value| value.as_str().trim().to_owned())
    };
    let Some(state) = capture(r"State: ([0-9A-Z]{5})") else {
        let plain = detail
            .replace("Diagnostics {", "")
            .replace("record:", "")
            .replace(['{', '}'], "");
        return format!("El controlador ODBC informó: {}", plain.trim());
    };
    let native = capture(r"Native error: (-?\d+)").unwrap_or_else(|| "?".to_owned());
    let message = capture(r#"(?s)Message: (.*?)(?:,\s*function:|\s*\}\s*$|$)"#)
        .map(|message| {
            let without_vendor = regex::Regex::new(r"^(\[[^\]]*\]\s*)+")
                .map(|regex| regex.replace(&message, "").into_owned())
                .unwrap_or(message);
            without_vendor
                .trim()
                .trim_end_matches(['.', ',', ' '])
                .to_owned()
        })
        .unwrap_or_default();
    let what = match &state[..2] {
        "08" => "se perdió la conexión con el servidor o no se pudo abrir",
        "28" => "el servidor rechazó el usuario o la contraseña",
        "42" => "el servidor rechazó la sentencia (nombre de tabla o columna, permisos o sintaxis)",
        "23" => "los datos incumplen una restricción de la tabla remota",
        "22" => "un valor no cabe en el tipo de la columna remota",
        "40" => "el servidor deshizo la transacción",
        "HY" if state.starts_with("HYT") => "la base remota no respondió a tiempo",
        _ => "el controlador ODBC devolvió un error",
    };
    if message.is_empty() {
        format!("{what} (SQLSTATE {state}, código {native}).")
    } else {
        format!("{what}. Mensaje del servidor: «{message}» (SQLSTATE {state}, código {native}).")
    }
}

// --- Importing a table from a database (2026-10-30) ---------------------------
//
// Columnia only reads: the person picks a table or view the server itself
// lists, and Columnia builds the one SELECT it runs. A free query could carry
// DELETE or DROP from a compromised interface, so none is accepted. The rows
// arrive as text, like a CSV, and are written to a managed Parquet file that
// then follows the ordinary Cargar flow.

/// The most rows one import brings; the person chooses a smaller limit.
pub(crate) const MAX_IMPORT_ROWS: usize = 5_000_000;
/// Longer cells are cut to this many characters and the column is named.
const MAX_IMPORT_CELL_CHARS: usize = 8_000;
/// The most columns one import reads; a server that reports more is refused
/// before any buffer is reserved for them.
const MAX_IMPORT_COLUMNS: usize = 2_000;
/// Size of the ODBC fetch buffer; the rows per batch follow from it. Each batch
/// goes to the Parquet file as soon as it arrives, so memory is bounded by one
/// batch, not by the table.
const IMPORT_BUFFER_BYTES: usize = 64 * 1024 * 1024;
const MAX_LISTED_TABLES: usize = 5_000;
/// Catalog rows read before giving up: far above the tables shown, so a server
/// that never stops listing objects ends with an error instead of filling memory.
const MAX_CATALOG_ROWS: usize = 100_000;
const DATABASE_IMPORT_DIRECTORY: &str = "database-imports";
const IMPORT_TEMPORARY_SUFFIX: &str = ".parquet.partial";

/// Empties the imports folder of earlier sessions (AUD-020). The dataset that
/// was loaded from there lives only in memory (`source_path` is not persisted
/// in projects or sessions), so at startup nothing reads these files; they
/// would stay as plain copies of the person's data, plus any temporary left by
/// a killed write. Safe at startup: the single-instance plugin guarantees no
/// other Columnia process uses them. Links and folders are left alone.
/// Returns how many files were removed.
pub(crate) fn purge_database_imports(app_data_dir: &Path) -> usize {
    let Ok(entries) = std::fs::read_dir(app_data_dir.join(DATABASE_IMPORT_DIRECTORY)) else {
        return 0;
    };
    entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .filter(|entry| std::fs::remove_file(entry.path()).is_ok())
        .count()
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DatabaseSource {
    pub kind: DatabaseKind,
    /// The fields of the connection. They are intentionally not persisted.
    pub connection: DatabaseConnection,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseSourceTable {
    pub schema: String,
    pub name: String,
    pub is_view: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseImportResult {
    pub inspection: crate::dataset::DatasetSourceInspection,
    pub rows_imported: usize,
    /// The table had more rows than the chosen limit.
    pub more_rows_available: bool,
    /// Columns with a value longer than `MAX_IMPORT_CELL_CHARS`, cut to it.
    pub truncated_columns: Vec<String>,
    /// Columns with a text that was not valid UTF-16 (a lone surrogate): each
    /// bad unit was replaced by U+FFFD.
    pub replaced_columns: Vec<String>,
}

struct FetchedTable {
    path: std::path::PathBuf,
    rows: usize,
    more_rows_available: bool,
    truncated_columns: Vec<String>,
    replaced_columns: Vec<String>,
}

fn validate_database_source(source: &DatabaseSource) -> Result<(), String> {
    source.connection.validate(source.kind)
}

/// Names come from the server's own catalog and are quoted, so any text is
/// accepted except control characters; the table must also be listed.
fn validate_source_object_name(value: &str, label: &str, optional: bool) -> Result<(), String> {
    if value.is_empty() {
        return if optional {
            Ok(())
        } else {
            Err(format!("Elige {label}."))
        };
    }
    if value.chars().count() > MAX_IDENTIFIER_CHARS || value.chars().any(char::is_control) {
        return Err(format!(
            "{label} no puede superar {MAX_IDENTIFIER_CHARS} caracteres ni contener caracteres de control."
        ));
    }
    Ok(())
}

fn validate_import_row_limit(row_limit: usize) -> Result<(), String> {
    if row_limit == 0 || row_limit > MAX_IMPORT_ROWS {
        return Err(format!(
            "El límite de filas debe estar entre 1 y {}.",
            MAX_IMPORT_ROWS
        ));
    }
    Ok(())
}

/// The helpers shared with deliveries take a target; reading uses its
/// connection and kind only.
fn source_as_target(source: &DatabaseSource) -> DatabaseTarget {
    DatabaseTarget {
        kind: source.kind,
        connection: source.connection.clone(),
        schema: String::new(),
        table: String::new(),
        table_policy: DatabaseTablePolicy::CreateOnly,
    }
}

/// Distinct from every delivery fingerprint: confirming a read never
/// authorises a write to the same server.
fn remote_source_fingerprint(source: &DatabaseSource) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    let parts = ["import", source.kind.label()]
        .into_iter()
        .chain(source.connection.fingerprint_parts());
    for part in parts {
        hasher.update(part.as_bytes());
        hasher.update([0]);
    }
    hex::encode(hasher.finalize())
}

fn describe_remote_source(source: &DatabaseSource) -> String {
    let (server, details) = describe_connection(&source.connection, source.kind);
    // AUD-008: the interface lets the person allow an unencrypted channel, so
    // the dialog Rust shows says it too, with the same text as a delivery.
    let warning = transport_encryption_issue(&source_as_target(source))
        .filter(|(severity, _)| *severity == "warning")
        .map(|(_, message)| format!("\n\nAtención: {message}"))
        .unwrap_or_default();
    // The promise is what Columnia does; the guarantee is the user's permission.
    format!(
        "Columnia se conectará a {} en {} para leer una tabla. Solo leerá: no escribirá ni modificará nada en el servidor. Para asegurarlo, usa un usuario que solo tenga permiso de lectura. Las filas que elijas se guardarán en este equipo.\n\n{}{}\n\n¿Confirmas este origen?",
        source.kind.label(),
        server,
        details,
        warning
    )
}

fn confirm_remote_source(app: &AppHandle, source: &DatabaseSource) -> Result<(), String> {
    confirm_with_native_dialog(
        app,
        remote_source_fingerprint(source),
        describe_remote_source(source),
        "Confirmar lectura de una base de datos",
    )
}

/// Tables and views the person may read, without the server's own schemas.
fn list_source_tables(
    connection: &odbc_api::Connection<'_>,
    target: &DatabaseTarget,
) -> Result<Vec<DatabaseSourceTable>, String> {
    // A statement of its own, so the catalog query carries a timeout: a server
    // that never answers the catalog fails here instead of hanging (AUD-013).
    let mut statement = connection
        .preallocate()
        .map_err(|error| format_driver_error("No se pudieron listar las tablas", error, target))?;
    statement
        .set_query_timeout_sec(ODBC_CATALOG_TIMEOUT_SEC)
        .map_err(|error| format_driver_error("No se pudieron listar las tablas", error, target))?;
    let cursor = statement
        .tables_cursor("", "", "", "TABLE,VIEW")
        .map_err(|error| format_driver_error("No se pudieron listar las tablas", error, target))?;
    let rows = catalog_rows(cursor)
        .map_err(|error| format_driver_error("No se pudieron leer las tablas", error, target))?;
    let mut unreadable = 0usize;
    let mut tables = read_catalog_tables(rows, MAX_CATALOG_ROWS, |row: CatalogRow| {
        if row.unreadable {
            unreadable += 1;
            return None;
        }
        source_table(
            target.kind,
            &row.catalog.unwrap_or_default(),
            &row.schema.unwrap_or_default(),
            &row.table.unwrap_or_default(),
            &row.table_type.unwrap_or_default(),
        )
    })
    .map_err(|error| match error {
        CatalogReadError::Driver(error) => {
            format_driver_error("No se pudieron leer las tablas", error, target)
        }
        CatalogReadError::TooManyRows => catalog_too_large_message(),
    })?;
    if unreadable > 0 {
        return Err(unreadable_catalog_message(unreadable));
    }
    sort_source_tables(&mut tables);
    tables.truncate(MAX_LISTED_TABLES);
    Ok(tables)
}

/// One row of the catalog read in wide text. A cell is None when the server
/// sent NULL. A row with a cell that could not be decoded (or was cut) is
/// flagged `unreadable`, so it is counted instead of shown wrong.
#[derive(Debug, Default, PartialEq, Eq)]
struct CatalogRow {
    catalog: Option<String>,
    schema: Option<String>,
    table: Option<String>,
    table_type: Option<String>,
    /// Some cell was not valid UTF-16 or was cut: nothing of the row is trusted.
    unreadable: bool,
}

/// NULL stays None; UTF-16 that is not valid is an Err, never replaced.
fn decode_catalog_cell(cell: Option<&[u16]>) -> Result<Option<String>, ()> {
    cell.map(|units| String::from_utf16(units).map_err(|_| ()))
        .transpose()
}

fn unreadable_catalog_message(count: usize) -> String {
    format!(
        "El servidor devolvió {count} tablas o vistas con un nombre que Columnia no puede leer. No se muestra la lista para no ocultarlas en silencio."
    )
}

/// The catalog columns TABLE_CAT, TABLE_SCHEM, TABLE_NAME and TABLE_TYPE as
/// UTF-16: the narrow form goes through the Windows code page and loses or
/// garbles accents, `ñ` or any character outside it.
fn catalog_rows<C: odbc_api::Cursor>(
    cursor: C,
) -> Result<impl Iterator<Item = Result<CatalogRow, odbc_api::Error>>, odbc_api::Error> {
    use odbc_api::buffers::{ColumnarBuffer, TextColumn};

    const BATCH_ROWS: usize = 1_000;
    // Above the 128 characters of the longest identifiers of the three engines.
    const CELL_UNITS: usize = 256;
    let buffer = ColumnarBuffer::new(
        (1u16..=4)
            .map(|column| (column, TextColumn::<u16>::new(BATCH_ROWS, CELL_UNITS)))
            .collect(),
    );
    let mut block = cursor.bind_buffer(buffer)?;
    let mut queue = std::collections::VecDeque::new();
    let mut finished = false;
    Ok(std::iter::from_fn(move || loop {
        if let Some(row) = queue.pop_front() {
            return Some(Ok(row));
        }
        if finished {
            return None;
        }
        match block.fetch() {
            Err(error) => {
                finished = true;
                return Some(Err(error));
            }
            Ok(None) => finished = true,
            Ok(Some(batch)) => {
                let cut =
                    (0..4).any(|column| batch.column(column).has_truncated_values().is_some());
                let columns: Vec<_> = (0..4).map(|column| batch.column(column)).collect();
                for row in 0..batch.num_rows() {
                    let mut unreadable = cut;
                    let mut text = |index: usize| {
                        // `get`, not `iter`: the iterator panics on invalid UTF-16.
                        let cell = columns[index].get(row);
                        decode_catalog_cell(cell).unwrap_or_else(|()| {
                            unreadable = true;
                            None
                        })
                    };
                    let (catalog, schema, table, table_type) = (text(0), text(1), text(2), text(3));
                    queue.push_back(CatalogRow {
                        catalog,
                        schema,
                        table,
                        table_type,
                        unreadable,
                    });
                }
            }
        }
    }))
}

enum CatalogReadError<E> {
    Driver(E),
    TooManyRows,
}

fn catalog_too_large_message() -> String {
    format!(
        "El servidor lista más de {MAX_CATALOG_ROWS} tablas y vistas; Columnia no puede mostrarlas todas. Usa un usuario con acceso a menos objetos."
    )
}

/// Reads the server's catalog row by row, keeping only what `convert` accepts,
/// and stops with an error once `limit` rows went by: a catalog that never
/// ends must not fill the memory.
fn read_catalog_tables<R, E>(
    rows: impl Iterator<Item = Result<R, E>>,
    limit: usize,
    mut convert: impl FnMut(R) -> Option<DatabaseSourceTable>,
) -> Result<Vec<DatabaseSourceTable>, CatalogReadError<E>> {
    let mut tables = Vec::new();
    let mut seen = 0usize;
    for row in rows {
        seen += 1;
        if seen > limit {
            return Err(CatalogReadError::TooManyRows);
        }
        if let Some(table) = convert(row.map_err(CatalogReadError::Driver)?) {
            tables.push(table);
        }
    }
    Ok(tables)
}

/// One row of the server's table list as Columnia shows it; None for the
/// server's own schemas. MySQL names its databases as catalogs, without
/// schemas: the database takes the schema's place, so `base`.`tabla` reads
/// exactly the listed table.
fn source_table(
    kind: DatabaseKind,
    catalog: &str,
    schema: &str,
    name: &str,
    table_type: &str,
) -> Option<DatabaseSourceTable> {
    if name.is_empty() {
        return None;
    }
    let schema = if kind == DatabaseKind::Mysql && schema.is_empty() {
        catalog
    } else {
        schema
    };
    if is_system_schema(kind, schema) {
        return None;
    }
    Some(DatabaseSourceTable {
        schema: schema.to_owned(),
        name: name.to_owned(),
        is_view: table_type.to_ascii_lowercase().contains("view"),
    })
}

fn is_system_schema(kind: DatabaseKind, schema: &str) -> bool {
    let lower = schema.to_ascii_lowercase();
    match kind {
        DatabaseKind::SqlServer => matches!(lower.as_str(), "sys" | "information_schema"),
        DatabaseKind::Postgresql => lower == "information_schema" || lower.starts_with("pg_"),
        DatabaseKind::Mysql => matches!(
            lower.as_str(),
            "information_schema" | "mysql" | "performance_schema" | "sys"
        ),
    }
}

fn sort_source_tables(tables: &mut [DatabaseSourceTable]) {
    tables.sort_by(|left, right| {
        (left.schema.to_lowercase(), left.name.to_lowercase())
            .cmp(&(right.schema.to_lowercase(), right.name.to_lowercase()))
    });
}

/// The listed object the person chose, matched exactly; a name that is not
/// listed is never sent to the server.
fn find_listed_table<'a>(
    tables: &'a [DatabaseSourceTable],
    schema: &str,
    table: &str,
) -> Option<&'a DatabaseSourceTable> {
    tables
        .iter()
        .find(|listed| listed.name == table && listed.schema == schema)
}

/// The only statement an import runs: every column of one listed object.
/// One row more than the limit tells whether the table had more.
fn select_rows_sql(table: &DatabaseSourceTable, row_limit: usize, kind: DatabaseKind) -> String {
    let fetch = row_limit.saturating_add(1);
    let qualified = if table.schema.is_empty() {
        quote_identifier(&table.name, kind)
    } else {
        format!(
            "{}.{}",
            quote_identifier(&table.schema, kind),
            quote_identifier(&table.name, kind)
        )
    };
    match kind {
        DatabaseKind::SqlServer => format!("SELECT TOP ({fetch}) * FROM {qualified}"),
        DatabaseKind::Postgresql | DatabaseKind::Mysql => {
            format!("SELECT * FROM {qualified} LIMIT {fetch}")
        }
    }
}

/// Column names as Columnia needs them: present and unique.
fn unique_column_names(names: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    names
        .into_iter()
        .enumerate()
        .map(|(index, name)| {
            let base = if name.trim().is_empty() {
                format!("column_{}", index + 1)
            } else {
                name
            };
            let mut candidate = base.clone();
            let mut suffix = 2;
            while !seen.insert(candidate.to_lowercase()) {
                candidate = format!("{base}_{suffix}");
                suffix += 1;
            }
            candidate
        })
        .collect()
}

/// A readable, safe file name for the imported table.
fn import_file_name(table: &DatabaseSourceTable) -> String {
    let label = if table.schema.is_empty() {
        table.name.clone()
    } else {
        format!("{}.{}", table.schema, table.name)
    };
    let mut safe = label
        .chars()
        .map(|character| {
            if character.is_alphanumeric() || matches!(character, '_' | '-' | '.' | ' ') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches(['.', ' '])
        .to_owned();
    if safe.is_empty() {
        safe = "tabla".to_owned();
    }
    let safe = safe.chars().take(100).collect::<String>();
    format!("{safe}.parquet")
}

/// The number of columns to read, or the reason the table is refused. Checked
/// before any buffer is reserved, so a server that reports thousands of
/// columns cannot make Columnia allocate for them.
fn check_import_column_count(count: usize) -> Result<u16, String> {
    if count == 0 {
        return Err("La tabla no tiene columnas que importar.".to_owned());
    }
    if count > MAX_IMPORT_COLUMNS {
        return Err(format!(
            "La tabla tiene {count} columnas; Columnia importa como mucho {MAX_IMPORT_COLUMNS}."
        ));
    }
    u16::try_from(count).map_err(|_| "La tabla tiene demasiadas columnas.".to_owned())
}

/// One ODBC batch as a small all-text frame; `None` stays null.
fn import_batch_frame(
    names: &[String],
    columns: Vec<Vec<Option<String>>>,
) -> Result<DataFrame, String> {
    use polars::prelude::{IntoColumn, NamedFrom, Series};
    let rows = columns.first().map_or(0, Vec::len);
    let columns = names
        .iter()
        .zip(columns)
        .map(|(name, column)| Series::new(name.as_str().into(), column).into_column())
        .collect::<Vec<_>>();
    DataFrame::new(rows, columns)
        .map_err(|error| format!("No se pudo preparar la tabla importada: {error}"))
}

struct ImportRead {
    rows: usize,
    more_rows_available: bool,
    truncated_columns: Vec<String>,
    replaced_columns: Vec<String>,
    /// Every row is already in the temporary file; it still has to be persisted.
    writer: ImportParquetWriter,
}

fn read_cursor_to_parquet<Cur, F, C>(
    mut cursor: Cur,
    row_limit: usize,
    pending: PendingImport,
    report: &mut F,
    is_cancelled: &C,
    target: &DatabaseTarget,
) -> Result<ImportRead, String>
where
    Cur: odbc_api::Cursor + odbc_api::ResultSetMetadata,
    F: FnMut(&'static str, u8),
    C: Fn() -> bool,
{
    use odbc_api::buffers::{ColumnarBuffer, TextColumn};

    let column_count = cursor
        .num_result_cols()
        .map_err(|error| format_driver_error("No se pudieron leer las columnas", error, target))?;
    let column_count = check_import_column_count(usize::try_from(column_count).unwrap_or(0))?;
    let mut names = Vec::with_capacity(usize::from(column_count));
    let mut lengths = Vec::with_capacity(usize::from(column_count));
    for column in 1..=column_count {
        names.push(cursor.col_name(column).map_err(|error| {
            format_driver_error("No se pudo leer el nombre de una columna", error, target)
        })?);
        let display = cursor
            .col_display_size(column)
            .ok()
            .flatten()
            .map_or(MAX_IMPORT_CELL_CHARS, std::num::NonZeroUsize::get);
        lengths.push(display.clamp(1, MAX_IMPORT_CELL_CHARS));
    }
    let names = unique_column_names(names);
    let bytes_per_row = lengths
        .iter()
        .map(|length| (length + 1) * 2 + 8)
        .sum::<usize>()
        .max(1);
    let batch_size = (IMPORT_BUFFER_BYTES / bytes_per_row).clamp(1, 10_000);
    let buffer = ColumnarBuffer::new(
        lengths
            .iter()
            .enumerate()
            .map(|(index, length)| {
                // Wide text: the narrow form uses the Windows code page and
                // would garble accents.
                (
                    index as u16 + 1,
                    TextColumn::<u16>::new(batch_size, *length),
                )
            })
            .collect(),
    );
    let mut block = cursor
        .bind_buffer(buffer)
        .map_err(|error| format_driver_error("No se pudo preparar la lectura", error, target))?;

    // Each batch is written to the temporary Parquet as soon as it arrives:
    // memory holds one batch, never the table. If the read fails or is
    // cancelled, the writer is dropped and the temporary file goes with it.
    let writer = pending.start(&names)?;
    stream_batches_to_parquet(
        names,
        row_limit,
        writer,
        |wanted| {
            let Some(batch) = block.fetch().map_err(|error| {
                format_driver_error("No se pudieron leer las filas", error, target)
            })?
            else {
                return Ok(None);
            };
            let available = batch.num_rows();
            let take = available.min(wanted);
            let mut columns = Vec::with_capacity(usize::from(column_count));
            let mut truncated = Vec::with_capacity(usize::from(column_count));
            let mut replaced = Vec::with_capacity(usize::from(column_count));
            for index in 0..usize::from(column_count) {
                let slice = batch.column(index);
                truncated.push(slice.has_truncated_values().is_some());
                let mut replaced_here = false;
                // `get`, not `iter`: the iterator panics on invalid UTF-16.
                columns.push(
                    (0..take)
                        .map(|row| {
                            slice.get(row).map(|units| {
                                let (text, was_replaced) = decode_wide_cell(units);
                                replaced_here |= was_replaced;
                                text
                            })
                        })
                        .collect::<Vec<_>>(),
                );
                replaced.push(replaced_here);
            }
            Ok(Some(ImportBatch {
                columns,
                available,
                truncated,
                replaced,
            }))
        },
        report,
        is_cancelled,
    )
}

/// One batch as the driver delivered it, already as text.
struct ImportBatch {
    /// The rows kept, by column: at most the rows still wanted.
    columns: Vec<Vec<Option<String>>>,
    /// The rows the driver delivered, kept or not.
    available: usize,
    /// Per column: a value of this batch was longer than the buffer and was cut.
    truncated: Vec<bool>,
    /// Per column: a value of this batch had invalid UTF-16 and was repaired.
    replaced: Vec<bool>,
}

/// Decodes one wide cell; a lone surrogate becomes U+FFFD and the flag says
/// that something was replaced.
fn decode_wide_cell(units: &[u16]) -> (String, bool) {
    match String::from_utf16(units) {
        Ok(text) => (text, false),
        Err(_) => (String::from_utf16_lossy(units), true),
    }
}

/// The loop of an import, apart from the driver: asks `next_batch` for batches
/// (given the rows still wanted), writes each as it arrives, stops at
/// `row_limit` (a batch with more rows than wanted marks `more_rows_available`),
/// notes the columns that came cut and checks for cancellation before every
/// batch. On error or cancellation the writer is dropped, and its temporary
/// file with it.
fn stream_batches_to_parquet<N, F, C>(
    names: Vec<String>,
    row_limit: usize,
    mut writer: ImportParquetWriter,
    mut next_batch: N,
    report: &mut F,
    is_cancelled: &C,
) -> Result<ImportRead, String>
where
    N: FnMut(usize) -> Result<Option<ImportBatch>, String>,
    F: FnMut(&'static str, u8),
    C: Fn() -> bool,
{
    let mut truncated = vec![false; names.len()];
    let mut replaced = vec![false; names.len()];
    let mut rows = 0usize;
    let mut more_rows_available = false;
    loop {
        ensure_not_cancelled(is_cancelled)?;
        let Some(batch) = next_batch(row_limit - rows)? else {
            break;
        };
        for (flag, cut) in truncated.iter_mut().zip(&batch.truncated) {
            *flag |= *cut;
        }
        for (flag, fixed) in replaced.iter_mut().zip(&batch.replaced) {
            *flag |= *fixed;
        }
        let take = batch.available.min(row_limit - rows);
        writer.write_rows(batch.columns)?;
        rows += take;
        let percent = 20 + ((rows as u64 * 60) / row_limit.max(1) as u64).min(60) as u8;
        report("Leyendo filas", percent);
        if take < batch.available {
            more_rows_available = true;
            break;
        }
    }

    let truncated_columns = names
        .iter()
        .zip(&truncated)
        .filter(|(_, truncated)| **truncated)
        .map(|(name, _)| name.clone())
        .collect();
    let replaced_columns = names
        .iter()
        .zip(&replaced)
        .filter(|(_, replaced)| **replaced)
        .map(|(name, _)| name.clone())
        .collect();
    Ok(ImportRead {
        rows,
        more_rows_available,
        truncated_columns,
        replaced_columns,
        writer,
    })
}

/// The destination of an import and the temporary file beside it, ready before
/// the first row is read. Nothing earlier is touched until `finish`.
struct PendingImport {
    directory: std::path::PathBuf,
    destination: std::path::PathBuf,
    temporary: tempfile::NamedTempFile,
    /// The file the active dataset reads, by identity, never to be deleted.
    canonical_keep: Option<std::path::PathBuf>,
}

impl PendingImport {
    /// Checks the imports folder and creates the temporary file. When the
    /// active dataset reads the file this import would replace, the new one
    /// sits beside it as «(nueva)».
    fn prepare(directory: &Path, file_name: &str, keep: Option<&Path>) -> Result<Self, String> {
        match std::fs::symlink_metadata(directory) {
            Ok(metadata) if !metadata.is_dir() || metadata.file_type().is_symlink() => {
                return Err("La carpeta de importaciones no es una carpeta normal.".to_owned());
            }
            Ok(_) => {}
            Err(_) => std::fs::create_dir_all(directory)
                .map_err(|_| "No se pudo crear la carpeta de importaciones.".to_owned())?,
        }
        // Compare by file identity, not by path text: the active dataset reports a
        // canonical path (`\\?\C:\...` on Windows) while `read_dir` does not.
        let canonical_keep = keep.and_then(|keep| std::fs::canonicalize(keep).ok());
        let mut destination = directory.join(file_name);
        if is_same_file(canonical_keep.as_deref(), &destination) {
            // The active dataset still reads that file: the new import sits beside it.
            let stem = file_name.trim_end_matches(".parquet");
            destination = directory.join(format!("{stem} (nueva).parquet"));
        }
        // The `.partial` suffix marks a write in progress; if the process dies the
        // startup purge (`purge_database_imports`) removes it with the rest.
        let temporary = tempfile::Builder::new()
            .suffix(IMPORT_TEMPORARY_SUFFIX)
            .tempfile_in(directory)
            .map_err(|_| "No se pudo crear el archivo de la importación.".to_owned())?;
        Ok(Self {
            directory: directory.to_path_buf(),
            destination,
            temporary,
            canonical_keep,
        })
    }

    /// Opens the Parquet writer with every column as text.
    fn start(self, names: &[String]) -> Result<ImportParquetWriter, String> {
        use polars::prelude::ParquetWriter;
        let schema_frame = import_batch_frame(names, vec![Vec::new(); names.len()])?;
        let file = self
            .temporary
            .as_file()
            .try_clone()
            .map_err(|_| "No se pudo crear el archivo de la importación.".to_owned())?;
        let writer = ParquetWriter::new(file)
            .batched(schema_frame.schema())
            .map_err(|error| format!("No se pudo guardar la tabla importada: {error}"))?;
        Ok(ImportParquetWriter {
            writer,
            names: names.to_vec(),
            buffered: None,
            group_rows: usize::MAX,
            pending: self,
        })
    }
}

fn is_same_file(canonical_keep: Option<&Path>, path: &Path) -> bool {
    canonical_keep
        .is_some_and(|keep| std::fs::canonicalize(path).is_ok_and(|canonical| canonical == keep))
}

/// A Parquet file written batch by batch into a temporary file.
struct ImportParquetWriter {
    writer: polars::io::parquet::write::BatchedWriter<std::fs::File>,
    names: Vec<String>,
    /// Rows waiting to fill a row group; never more than `group_rows`.
    buffered: Option<DataFrame>,
    /// Rows per row group: the smallest `parquet_row_group_rows` seen so far.
    group_rows: usize,
    pending: PendingImport,
}

impl ImportParquetWriter {
    /// The one place a batch reaches the file. Batches are gathered into row
    /// groups of `parquet_row_group_rows` rows (as the rest of the project
    /// writes them) and a group is written as soon as it is full.
    fn write_rows(&mut self, columns: Vec<Vec<Option<String>>>) -> Result<(), String> {
        if columns.first().is_none_or(Vec::is_empty) {
            return Ok(());
        }
        let mut frame = import_batch_frame(&self.names, columns)?;
        self.group_rows = self
            .group_rows
            .min(crate::dataset::parquet_row_group_rows(&frame));
        if self
            .buffered
            .as_ref()
            .is_some_and(|buffered| buffered.height() >= self.group_rows)
        {
            self.flush()?;
        }
        while frame.height() > 0 {
            let held = self.buffered.as_ref().map_or(0, DataFrame::height);
            let take = (self.group_rows - held).min(frame.height());
            let part = frame.slice(0, take);
            frame = frame.slice(take as i64, frame.height() - take);
            match self.buffered.as_mut() {
                Some(buffered) => buffered
                    .vstack_mut(&part)
                    .map(|_| ())
                    .map_err(|error| format!("No se pudo preparar la tabla importada: {error}"))?,
                None => self.buffered = Some(part),
            }
            if held + take >= self.group_rows {
                self.flush()?;
            }
        }
        Ok(())
    }

    /// Writes the gathered rows as one row group (`write_batch` writes one
    /// group per chunk, so the frame is first made a single chunk).
    fn flush(&mut self) -> Result<(), String> {
        let Some(mut group) = self.buffered.take() else {
            return Ok(());
        };
        group.rechunk_mut();
        self.writer
            .write_batch(&group)
            .map_err(|error| format!("No se pudo guardar la tabla importada: {error}"))
    }

    /// Closes the file, puts it in place atomically and only then removes
    /// earlier imports, except the file the active dataset still reads.
    ///
    /// AUD-103: a thread whose import was cancelled may come back long after
    /// the interface moved on. The cancellation is checked right before the
    /// file is put in place, so such a thread writes nothing and removes
    /// nothing: its temporary is dropped (deleted) with it.
    fn finish<C>(mut self, is_cancelled: &C) -> Result<std::path::PathBuf, String>
    where
        C: Fn() -> bool,
    {
        self.flush()?;
        let Self {
            writer, pending, ..
        } = self;
        writer
            .finish()
            .map_err(|error| format!("No se pudo guardar la tabla importada: {error}"))?;
        drop(writer);
        let PendingImport {
            directory,
            destination,
            temporary,
            canonical_keep,
        } = pending;
        temporary
            .as_file()
            .sync_all()
            .map_err(|_| "No se pudo guardar la tabla importada.".to_owned())?;
        // Last point of no return: persisting and the cleanup below both stay
        // behind this check.
        ensure_not_cancelled(is_cancelled)?;
        temporary
            .persist(&destination)
            .map_err(|_| "No se pudo guardar la tabla importada.".to_owned())?;
        // Only now that the new file is in place are earlier imports removed.
        if let Ok(entries) = std::fs::read_dir(&directory) {
            for entry in entries.flatten() {
                let path = entry.path();
                let is_parquet = path
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("parquet"));
                if is_parquet
                    && path != destination
                    && !is_same_file(canonical_keep.as_deref(), &path)
                {
                    let _ = std::fs::remove_file(path);
                }
            }
        }
        Ok(destination)
    }
}

#[allow(clippy::too_many_arguments)]
fn import_table_with_cancel<F, C>(
    source: &DatabaseSource,
    schema: &str,
    table: &str,
    row_limit: usize,
    directory: &Path,
    keep: Option<&Path>,
    mut report: F,
    is_cancelled: C,
) -> Result<FetchedTable, String>
where
    F: FnMut(&'static str, u8),
    C: Fn() -> bool,
{
    validate_database_source(source)?;
    let target = source_as_target(source);
    ensure_not_cancelled(&is_cancelled)?;
    report("Conectando con la base de datos", 5);
    let environment = Environment::new()
        .map_err(|error| format_driver_error("No se pudo inicializar ODBC", error, &target))?;
    let connection = connect(&environment, &target, ConnectionPurpose::Import);
    ensure_not_cancelled(&is_cancelled)?;
    let connection = connection?;

    report("Comprobando la tabla", 10);
    let tables = list_source_tables(&connection, &target)?;
    ensure_not_cancelled(&is_cancelled)?;
    let listed = find_listed_table(&tables, schema, table)
        .cloned()
        .ok_or_else(|| {
            format!("La tabla «{table}» no aparece entre las que el servidor permite leer.")
        })?;

    // The temporary file exists before the query runs; if anything fails from
    // here on it is discarded and no earlier import is touched.
    let pending = PendingImport::prepare(directory, &import_file_name(&listed), keep)?;
    report("Leyendo filas", 20);
    let cursor = connection
        .execute(
            &select_rows_sql(&listed, row_limit, source.kind),
            (),
            Some(ODBC_STATEMENT_TIMEOUT_SEC),
        )
        .map_err(|error| format_driver_error("No se pudo leer la tabla", error, &target))?
        .ok_or_else(|| "La tabla no devolvió filas.".to_owned())?;
    let read = read_cursor_to_parquet(
        cursor,
        row_limit,
        pending,
        &mut report,
        &is_cancelled,
        &target,
    )?;

    ensure_not_cancelled(&is_cancelled)?;
    report("Guardando la tabla en este equipo", 85);
    let path = read.writer.finish(&is_cancelled)?;
    Ok(FetchedTable {
        path,
        rows: read.rows,
        more_rows_available: read.more_rows_available,
        truncated_columns: read.truncated_columns,
        replaced_columns: read.replaced_columns,
    })
}

/// The installed ODBC drivers that reach one engine, the preferred first, so
/// the connection form picks one without the person typing its name. It only
/// reads the local driver list; no connection is opened.
#[tauri::command]
pub async fn list_database_drivers(kind: DatabaseKind) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let environment = Environment::new()
            .map_err(|error| driver_list_error("No se pudo inicializar ODBC", error))?;
        let drivers = environment
            .drivers()
            .map_err(|error| {
                driver_list_error("No se pudieron listar los controladores ODBC", error)
            })?
            .into_iter()
            .map(|driver| driver.description)
            .collect::<Vec<_>>();
        Ok(database_drivers(kind, drivers))
    })
    .await
    .map_err(|_| "La lista de controladores se interrumpió.".to_owned())?
}

/// A readable sentence for a failure to list the ODBC drivers, never the
/// `Debug` dump of the error (no braces, «Diagnostics» or «State:»).
fn driver_list_error<E: Debug>(context: &str, error: E) -> String {
    let readable = readable_driver_detail(&format!("{error:?}"))
        .replace("Diagnostics", "")
        .replace("State:", "SQLSTATE");
    format!("{context}: {readable}")
}

fn database_drivers(kind: DatabaseKind, drivers: Vec<String>) -> Vec<String> {
    match kind {
        DatabaseKind::SqlServer => sql_server_drivers(drivers),
        // psqlODBC: Unicode keeps accents; ANSI uses the Windows code page.
        DatabaseKind::Postgresql => ranked_drivers(
            drivers,
            |lower| lower.contains("postgres"),
            |lower| (u8::from(!lower.contains("unicode")), 0, 0),
        ),
        // Connector/ODBC by version, Unicode before ANSI; MariaDB's after.
        DatabaseKind::Mysql => ranked_drivers(
            drivers,
            |lower| lower.contains("mysql") || lower.contains("mariadb"),
            |lower| {
                (
                    u8::from(!lower.contains("mysql")),
                    u8::from(lower.contains("ansi")),
                    u32::MAX - driver_version(lower),
                )
            },
        ),
    }
}

fn ranked_drivers(
    drivers: Vec<String>,
    reaches: impl Fn(&str) -> bool,
    rank: impl Fn(&str) -> (u8, u8, u32),
) -> Vec<String> {
    let mut drivers = drivers
        .into_iter()
        .filter(|name| reaches(&name.to_ascii_lowercase()))
        .collect::<Vec<_>>();
    drivers.sort_by_key(|name| rank(&name.to_ascii_lowercase()));
    drivers.dedup();
    drivers
}

/// «MySQL ODBC 9.1 Unicode Driver» → 9001; 0 without a version.
fn driver_version(name: &str) -> u32 {
    name.split_whitespace()
        .find_map(|word| {
            let (major, minor) = word.split_once('.')?;
            Some(major.parse::<u32>().ok()? * 1000 + minor.parse::<u32>().ok()?)
        })
        .unwrap_or(0)
}

const LEGACY_SQL_SERVER_DRIVER_MESSAGE: &str =
    "Este controlador no admite el cifrado; elige ODBC Driver 17 o 18.";

/// The old «SQL Server» driver of Windows (`sqlsrv32`), by its exact name: it
/// cannot encrypt the connection, which Columnia always requires.
fn is_legacy_sql_server_driver(name: &str) -> bool {
    name.trim().eq_ignore_ascii_case("SQL Server")
}

/// SQL Server drivers by preference: «ODBC Driver N» from the highest N, then
/// the Native Client. The legacy «SQL Server» driver is left out.
fn sql_server_drivers(drivers: Vec<String>) -> Vec<String> {
    let rank = |name: &str| -> (u8, u32) {
        let lower = name.to_ascii_lowercase();
        if let Some(version) = lower
            .strip_prefix("odbc driver ")
            .and_then(|rest| rest.split_whitespace().next())
            .and_then(|number| number.parse::<u32>().ok())
        {
            (0, u32::MAX - version)
        } else if lower.contains("native client") {
            (1, 0)
        } else {
            (2, 0)
        }
    };
    let mut drivers = drivers
        .into_iter()
        .filter(|name| {
            name.to_ascii_lowercase().contains("sql server") && !is_legacy_sql_server_driver(name)
        })
        .collect::<Vec<_>>();
    drivers.sort_by_key(|name| rank(name));
    drivers.dedup();
    drivers
}

const CANCEL_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(100);

/// Runs blocking `work` and gives the result back, or the cancellation error
/// as soon as `is_cancelled` turns true, even while the driver call inside
/// `work` has not returned (AUD-013). The thread keeps running until the
/// driver gives up (the timeouts bound it); its result is discarded.
async fn run_blocking_unless_cancelled<T, W, C>(
    work: W,
    is_cancelled: C,
    interrupted_message: &'static str,
) -> Result<T, String>
where
    T: Send + 'static,
    W: FnOnce() -> Result<T, String> + Send + 'static,
    C: Fn() -> bool + Send + 'static,
{
    let finished = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let (cancel_sender, mut cancel_receiver) = tokio::sync::oneshot::channel::<()>();
    let watcher_finished = std::sync::Arc::clone(&finished);
    std::thread::spawn(move || {
        while !watcher_finished.load(std::sync::atomic::Ordering::Acquire) {
            if is_cancelled() {
                let _ = cancel_sender.send(());
                return;
            }
            std::thread::sleep(CANCEL_POLL_INTERVAL);
        }
    });
    let mut handle = tauri::async_runtime::spawn_blocking(work);
    let outcome = tokio::select! {
        joined = &mut handle => Some(joined),
        Ok(()) = &mut cancel_receiver => None,
    };
    finished.store(true, std::sync::atomic::Ordering::Release);
    match outcome {
        Some(joined) => joined.map_err(|_| interrupted_message.to_owned())?,
        None => Err(crate::dataset::OPERATION_CANCELLED_MESSAGE.to_owned()),
    }
}

#[tauri::command]
pub async fn list_database_tables(
    app: AppHandle,
    source: DatabaseSource,
) -> Result<Vec<DatabaseSourceTable>, String> {
    validate_database_source(&source)?;
    let confirmation_app = app.clone();
    let confirmation_source = source.clone();
    tauri::async_runtime::spawn_blocking(move || {
        confirm_remote_source(&confirmation_app, &confirmation_source)
    })
    .await
    .map_err(|_| "La confirmación de la conexión se interrumpió.".to_owned())??;
    let generation = app.state::<DatasetState>().begin_database_connection();
    let cancellation_app = app.clone();
    let watcher_app = app.clone();
    run_blocking_unless_cancelled(
        move || {
            let is_cancelled = || {
                cancellation_app
                    .state::<DatasetState>()
                    .database_connection_was_cancelled(generation)
            };
            let target = source_as_target(&source);
            ensure_not_cancelled(&is_cancelled)?;
            let environment = Environment::new().map_err(|error| {
                format_driver_error("No se pudo inicializar ODBC", error, &target)
            })?;
            let connection = connect(&environment, &target, ConnectionPurpose::Import);
            ensure_not_cancelled(&is_cancelled)?;
            let tables = list_source_tables(&connection?, &target);
            ensure_not_cancelled(&is_cancelled)?;
            tables
        },
        move || {
            watcher_app
                .state::<DatasetState>()
                .database_connection_was_cancelled(generation)
        },
        "La lista de tablas se interrumpió inesperadamente.",
    )
    .await
}

#[tauri::command]
pub async fn import_database_table(
    app: AppHandle,
    source: DatabaseSource,
    schema: String,
    table: String,
    row_limit: usize,
    on_progress: tauri::ipc::Channel<crate::dataset::OperationProgress>,
) -> Result<DatabaseImportResult, String> {
    validate_database_source(&source)?;
    validate_source_object_name(&schema, "El esquema", true)?;
    validate_source_object_name(&table, "una tabla", false)?;
    validate_import_row_limit(row_limit)?;
    let confirmation_app = app.clone();
    let confirmation_source = source.clone();
    tauri::async_runtime::spawn_blocking(move || {
        confirm_remote_source(&confirmation_app, &confirmation_source)
    })
    .await
    .map_err(|_| "La confirmación de la conexión se interrumpió.".to_owned())??;
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|_| "No se encontró la carpeta de datos de Columnia.".to_owned())?
        .join(DATABASE_IMPORT_DIRECTORY);
    let keep = app.state::<DatasetState>().active_source_path();
    let generation = app.state::<DatasetState>().begin_database_connection();
    let cancellation_app = app.clone();
    let watcher_app = app.clone();
    // AUD-103: Cancelar returns at once even if the driver call is blocked; the
    // thread that returns later is discarded and (see `finish`) writes nothing.
    // The dataset is only opened below, after this result, so it never changes
    // for a cancelled import.
    let fetched = run_blocking_unless_cancelled(
        move || {
            import_table_with_cancel(
                &source,
                &schema,
                &table,
                row_limit,
                &directory,
                keep.as_deref(),
                |stage, percent| {
                    crate::dataset::send_progress(&on_progress, "load", stage, percent)
                },
                || {
                    cancellation_app
                        .state::<DatasetState>()
                        .database_connection_was_cancelled(generation)
                },
            )
        },
        move || {
            watcher_app
                .state::<DatasetState>()
                .database_connection_was_cancelled(generation)
        },
        "La importación se interrumpió inesperadamente.",
    )
    .await?;
    let inspection = crate::dataset::inspect_dataset_path(&app, fetched.path).await?;
    Ok(DatabaseImportResult {
        inspection,
        rows_imported: fetched.rows,
        more_rows_available: fetched.more_rows_available,
        truncated_columns: fetched.truncated_columns,
        replaced_columns: fetched.replaced_columns,
    })
}

/// Support for the ignored tests of the SQL Server script and the BULK INSERT
/// package (AUD-075): `odbc_api` only lives in this file, so the tests of
/// other modules run their scripts and read the tables back through here.
#[cfg(test)]
pub(crate) mod sqlserver_script_testing {
    use odbc_api::{Connection, ConnectionOptions, Cursor, Environment};

    pub(crate) const CONNECTION_VARIABLE: &str = "COLUMNIA_ODBC_SQLSERVER";

    /// One cell as text; `None` is NULL.
    pub(crate) type Row = Vec<Option<String>>;

    fn with_connection<T>(
        work: impl FnOnce(&Connection<'_>) -> Result<T, String>,
    ) -> Result<T, String> {
        let text = std::env::var(CONNECTION_VARIABLE).map_err(|_| {
            format!("La prueba requiere la variable {CONNECTION_VARIABLE} con una cadena de conexión de sesión.")
        })?;
        let environment = Environment::new().map_err(|error| format!("ODBC: {error:?}"))?;
        let connection = environment
            .connect_with_connection_string(&text, ConnectionOptions::default())
            .map_err(|error| format!("conexión ODBC: {error:?}"))?;
        work(&connection)
    }

    fn read_rows(mut cursor: impl Cursor) -> Result<Vec<Row>, String> {
        let columns = cursor
            .num_result_cols()
            .map_err(|error| format!("columnas ODBC: {error:?}"))?
            .max(0) as u16;
        let mut rows = Vec::new();
        let mut buffer = Vec::<u16>::new();
        while let Some(mut row) = cursor
            .next_row()
            .map_err(|error| format!("lectura ODBC: {error:?}"))?
        {
            let mut cells = Vec::new();
            for column in 1..=columns {
                let present = row
                    .get_wide_text(column, &mut buffer)
                    .map_err(|error| format!("celda ODBC: {error:?}"))?;
                cells.push(present.then(|| String::from_utf16_lossy(&buffer)));
            }
            rows.push(cells);
        }
        Ok(rows)
    }

    /// Runs a whole script as one batch (the scripts have no `GO`) and returns
    /// the rows of its final result set, if it has one.
    pub(crate) fn run_script(sql: &str) -> Result<Vec<Row>, String> {
        with_connection(|connection| {
            match connection
                .execute(sql, (), None)
                .map_err(|error| format!("SQL externo: {error:?}"))?
            {
                Some(cursor) => read_rows(cursor),
                None => Ok(Vec::new()),
            }
        })
    }

    /// Runs a query and returns every row with each cell as text.
    pub(crate) fn query_rows(sql: &str) -> Result<Vec<Row>, String> {
        with_connection(|connection| {
            let cursor = connection
                .execute(sql, (), None)
                .map_err(|error| format!("consulta ODBC: {error:?}"))?
                .ok_or_else(|| "La consulta no devolvió filas.".to_owned())?;
            read_rows(cursor)
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{
        env, fs,
        path::Path,
        time::{SystemTime, UNIX_EPOCH},
    };

    use odbc_api::{buffers::RowVec, parameter::VarWCharArray, Cursor};
    use polars::{
        df,
        prelude::{col, DataFrame, DataType, IntoLazy, ParquetWriter},
    };
    use tempfile::tempdir;

    fn named_column(name: &str) -> RemoteInputColumn {
        RemoteInputColumn {
            name: name.to_owned(),
            data_type: "str".to_owned(),
            null_count: 0,
            maximum_length: Some(1),
            contains_nul: false,
            unrepresentable_integer_count: 0,
            unrepresentable_decimal_count: 0,
        }
    }

    /// FUN-20: names longer than the engine allows block the analysis.
    #[test]
    fn preflight_blocks_column_names_longer_than_the_engine_allows() {
        let long = "x".repeat(129);
        let input = vec![named_column(&long), named_column(&"y".repeat(128))];
        let report =
            assess_remote_export(&target(DatabaseKind::SqlServer), &input, false, false, &[]);
        assert!(!report.ready);
        let blocking = report
            .issues
            .iter()
            .filter(|issue| issue.severity == "blocking" && issue.category == "type")
            .collect::<Vec<_>>();
        assert_eq!(blocking.len(), 1, "{blocking:?}");
        assert_eq!(blocking[0].column.as_deref(), Some(long.as_str()));
        assert!(blocking[0].message.contains("como máximo 128"));

        let postgres =
            column_name_issues(DatabaseKind::Postgresql, &[named_column(&"á".repeat(32))]);
        assert_eq!(postgres.len(), 1, "64 bytes en UTF-8");
        assert!(
            column_name_issues(DatabaseKind::Mysql, &[named_column(&"m".repeat(64))]).is_empty()
        );
        assert_eq!(
            column_name_issues(DatabaseKind::Mysql, &[named_column(&"m".repeat(65))]).len(),
            1
        );
    }

    /// FUN-21: names that differ only in case are named in the analysis where
    /// the engine does not tell them apart, and do not look like a changed file.
    #[test]
    fn preflight_names_columns_that_differ_only_in_case() {
        let input = vec![named_column("Dup"), named_column("dup")];
        let issues = column_name_issues(DatabaseKind::SqlServer, &input);
        assert_eq!(issues.len(), 1);
        assert!(issues[0].1.contains("'Dup' y 'dup'"), "{}", issues[0].1);
        assert!(column_name_issues(DatabaseKind::Postgresql, &input).is_empty());

        assert!(same_streamed_names(&["Dup", "dup"], &["Dup", "dup_1"]));
        assert!(!same_streamed_names(&["a", "b"], &["a", "c"]));
        assert!(!same_streamed_names(&["a"], &["a", "b"]));
    }

    /// UX-04 and DAT-10: driver errors read as sentences, and a failed commit
    /// says the rows may have been saved.
    #[test]
    fn driver_errors_read_as_sentences() {
        let dump = r#"Diagnostics { record: State: 08S01, Native error: 10054, Message: [Microsoft][ODBC Driver 18 for SQL Server]TCP Provider: Se ha forzado la interrupción de una conexión existente por el host remoto.
 , function: "SQLExecute" }"#;
        let readable = readable_driver_detail(dump);
        assert!(!readable.contains("Diagnostics {"), "{readable}");
        assert!(!readable.contains("record:"), "{readable}");
        assert!(readable.contains("se perdió la conexión"), "{readable}");
        assert!(
            readable.contains("SQLSTATE 08S01, código 10054"),
            "{readable}"
        );
        assert!(readable.contains("TCP Provider"), "{readable}");
        assert!(!readable.contains("[Microsoft]"), "{readable}");

        let message = format_driver_error(
            "No se pudo insertar un lote",
            dump,
            &target(DatabaseKind::SqlServer),
        );
        assert!(!message.contains("Diagnostics {"), "{message}");
        assert!(UNCERTAIN_COMMIT_MESSAGE.contains("puede haberse guardado"));
    }

    /// AUD-027: listing the drivers never shows the `Debug` dump of the error.
    #[test]
    fn driver_list_errors_have_no_debug_dump() {
        struct Raw(&'static str);
        impl Debug for Raw {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str(self.0)
            }
        }
        for dump in [
            "Diagnostics { record: State: 01000, Native error: 0, Message: sin acceso }",
            "Diagnostics { record: sin estado }",
            "FailedReadingInstalledDrivers { State: x }",
        ] {
            let message =
                driver_list_error("No se pudieron listar los controladores ODBC", Raw(dump));
            for forbidden in ["{", "}", "Diagnostics", "State:"] {
                assert!(!message.contains(forbidden), "{message}");
            }
            assert!(message.starts_with("No se pudieron listar"), "{message}");
        }
    }

    use super::*;

    /// A valid connection by user and password to the local machine; the
    /// encryption is relaxed for PostgreSQL and MySQL, so it is only noted.
    fn connection(kind: DatabaseKind) -> DatabaseConnection {
        DatabaseConnection {
            driver: "test".to_owned(),
            server: "localhost".to_owned(),
            port: String::new(),
            database: String::new(),
            authentication: DatabaseAuthentication::Password,
            user: "ana".to_owned(),
            password: "secret".to_owned(),
            trust_server_certificate: false,
            allow_unencrypted: kind != DatabaseKind::SqlServer,
        }
    }

    fn target(kind: DatabaseKind) -> DatabaseTarget {
        DatabaseTarget {
            kind,
            connection: connection(kind),
            schema: "public".to_owned(),
            table: "ventas".to_owned(),
            table_policy: DatabaseTablePolicy::CreateOnly,
        }
    }

    /// An ODBC connection string split by the grammar of the Driver Manager:
    /// a brace opens a value only at its start, `}}` inside it is a literal
    /// `}` and a single `}` closes it. Keys come in lower case.
    fn parse_odbc_connection_string(text: &str) -> Result<Vec<(String, String)>, String> {
        let characters = text.chars().collect::<Vec<_>>();
        let mut position = 0;
        let mut attributes = Vec::new();
        while position < characters.len() {
            let start = position;
            while position < characters.len() && characters[position] != '=' {
                if characters[position] == ';' {
                    return Err(format!("atributo sin «=» en la posición {start}"));
                }
                position += 1;
            }
            if position == characters.len() {
                return Err(format!("atributo sin «=» en la posición {start}"));
            }
            let key = characters[start..position]
                .iter()
                .collect::<String>()
                .trim()
                .to_ascii_lowercase();
            position += 1;
            let mut value = String::new();
            if characters.get(position) == Some(&'{') {
                position += 1;
                loop {
                    match characters.get(position) {
                        None => return Err("llave sin cerrar".to_owned()),
                        Some('}') if characters.get(position + 1) == Some(&'}') => {
                            value.push('}');
                            position += 2;
                        }
                        Some('}') => {
                            position += 1;
                            break;
                        }
                        Some(character) => {
                            value.push(*character);
                            position += 1;
                        }
                    }
                }
                match characters.get(position) {
                    None => {}
                    Some(';') => position += 1,
                    Some(other) => return Err(format!("«{other}» tras cerrar la llave")),
                }
            } else {
                while position < characters.len() && characters[position] != ';' {
                    value.push(characters[position]);
                    position += 1;
                }
                position += 1;
            }
            attributes.push((key, value));
        }
        Ok(attributes)
    }

    /// The only value of a key; fails if it is missing or repeated.
    fn only_value<'a>(attributes: &'a [(String, String)], key: &str) -> &'a str {
        let found = attributes
            .iter()
            .filter(|(name, _)| name == key)
            .collect::<Vec<_>>();
        assert_eq!(
            found.len(),
            1,
            "{key} debe aparecer una vez: {attributes:?}"
        );
        found[0].1.as_str()
    }

    /// The session's connection string of the external tests, as the fields
    /// Rust now takes. Only the attributes the guided form can produce are
    /// understood.
    fn connection_from_odbc_string(kind: DatabaseKind, text: &str) -> DatabaseConnection {
        let mut connection = DatabaseConnection {
            driver: String::new(),
            server: String::new(),
            port: String::new(),
            database: String::new(),
            authentication: DatabaseAuthentication::Password,
            user: String::new(),
            password: String::new(),
            trust_server_certificate: false,
            allow_unencrypted: false,
        };
        for (key, value) in
            parse_odbc_connection_string(text).expect("cadena ODBC de sesión válida")
        {
            match key.as_str() {
                "driver" => connection.driver = value,
                "server" | "host" => connection.server = value,
                "port" => connection.port = value,
                "database" | "db" => connection.database = value,
                "uid" | "user" | "user id" => connection.user = value,
                "pwd" | "password" => connection.password = value,
                "trusted_connection" if value.eq_ignore_ascii_case("yes") => {
                    connection.authentication = DatabaseAuthentication::Windows;
                }
                "trustservercertificate" => {
                    connection.trust_server_certificate = value.eq_ignore_ascii_case("yes");
                }
                "sslmode"
                    if matches!(value.to_ascii_lowercase().as_str(), "prefer" | "preferred") =>
                {
                    connection.allow_unencrypted = true;
                }
                "sslmode"
                    if matches!(value.to_ascii_lowercase().as_str(), "require" | "required") => {}
                "encrypt" if value.eq_ignore_ascii_case("yes") => {}
                other => panic!(
                    "La cadena de sesión usa «{other}», que la conexión por campos no admite."
                ),
            }
        }
        assert!(
            !(kind == DatabaseKind::SqlServer && connection.allow_unencrypted),
            "SQL Server siempre cifra"
        );
        connection
    }

    #[test]
    fn inserts_are_batched_within_the_parameter_limit_of_every_dialect() {
        assert_eq!(rows_per_insert(1), MAX_ROWS_PER_INSERT);
        assert_eq!(rows_per_insert(20), 100);
        assert_eq!(rows_per_insert(3_000), 1);
        for columns in [1, 7, 20, 150, 2_000, 5_000] {
            assert!(rows_per_insert(columns) * columns <= MAX_PARAMETERS_PER_INSERT.max(columns));
        }
    }

    #[test]
    fn target_identity_ignores_secrets_and_attribute_order() {
        let mut first = target(DatabaseKind::SqlServer);
        first.table_policy = DatabaseTablePolicy::Append;
        first.connection.driver = "ODBC".to_owned();
        first.connection.server = "db1".to_owned();
        first.connection.database = "ventas".to_owned();
        first.connection.password = "uno".to_owned();
        let mut second = first.clone();
        second.connection.password = "dos".to_owned();
        assert_eq!(
            remote_target_identity(&first),
            remote_target_identity(&second)
        );
        assert!(!remote_target_identity(&first).contains("uno"));
        let mut other_table = first.clone();
        other_table.table = "otra".to_owned();
        assert_ne!(
            remote_target_identity(&first),
            remote_target_identity(&other_table)
        );
        let mut other_server = first.clone();
        other_server.connection.server = "db2".to_owned();
        assert_ne!(
            remote_target_identity(&first),
            remote_target_identity(&other_server)
        );
    }

    /// The identity is what `remote-deliveries.json` stores: for the same data
    /// it must stay exactly what it was when Rust analyzed the connection
    /// string the interface sent. These literals were produced by that code.
    #[test]
    fn target_identity_keeps_the_literal_the_ledger_already_stores() {
        let identity = |kind, connection: DatabaseConnection, schema: &str| {
            remote_target_identity(&DatabaseTarget {
                kind,
                connection,
                schema: schema.to_owned(),
                table: "ventas".to_owned(),
                table_policy: DatabaseTablePolicy::Append,
            })
        };
        let base = |driver: &str, server: &str| DatabaseConnection {
            driver: driver.to_owned(),
            server: server.to_owned(),
            ..connection(DatabaseKind::SqlServer)
        };
        assert_eq!(
            identity(
                DatabaseKind::SqlServer,
                DatabaseConnection {
                    database: "Ventas".to_owned(),
                    user: "ana".to_owned(),
                    password: "p;w".to_owned(),
                    ..base("ODBC Driver 18 for SQL Server", "tcp:db.example.com,1433")
                },
                "dbo"
            ),
            "SQL Server\u{1f}database=Ventas\u{1e}driver={ODBC Driver 18 for SQL Server}\u{1e}encrypt=yes\u{1e}server=tcp:db.example.com,1433\u{1e}uid=ana\u{1f}[dbo].[ventas]"
        );
        assert_eq!(
            identity(
                DatabaseKind::SqlServer,
                DatabaseConnection {
                    authentication: DatabaseAuthentication::Windows,
                    user: String::new(),
                    password: String::new(),
                    trust_server_certificate: true,
                    ..base("ODBC Driver 18 for SQL Server", "localhost")
                },
                ""
            ),
            "SQL Server\u{1f}driver={ODBC Driver 18 for SQL Server}\u{1e}encrypt=yes\u{1e}server=localhost\u{1e}trusted_connection=yes\u{1e}trustservercertificate=yes\u{1f}[ventas]"
        );
        assert_eq!(
            identity(
                DatabaseKind::Postgresql,
                DatabaseConnection {
                    port: "5433".to_owned(),
                    database: "ventas".to_owned(),
                    password: "x".to_owned(),
                    allow_unencrypted: false,
                    ..base("PostgreSQL Unicode(x64)", "db.example.com")
                },
                "public"
            ),
            "PostgreSQL\u{1f}database=ventas\u{1e}driver={PostgreSQL Unicode(x64)}\u{1e}port=5433\u{1e}server=db.example.com\u{1e}sslmode=require\u{1e}uid=ana\u{1f}\"public\".\"ventas\""
        );
        assert_eq!(
            identity(
                DatabaseKind::Mysql,
                DatabaseConnection {
                    password: "x".to_owned(),
                    allow_unencrypted: true,
                    ..base("MySQL ODBC 9.1 Unicode Driver", "a;b")
                },
                ""
            ),
            "MySQL\u{1f}driver={MySQL ODBC 9.1 Unicode Driver}\u{1e}server={a;b}\u{1e}sslmode=PREFERRED\u{1e}uid=ana\u{1f}`ventas`"
        );
    }

    #[test]
    fn repeated_delivery_warning_distinguishes_committed_and_uncertain() {
        let record = |status| DeliveryRecord {
            key: "k".to_owned(),
            status,
            rows: 1_250,
            recorded_at: "2026-09-25T10:00:00+00:00".to_owned(),
        };
        let committed = describe_repeated_delivery(&record(DeliveryStatus::Committed));
        assert!(committed.contains("ya se añadió"));
        assert!(committed.contains("1250 filas quedarán duplicadas"));
        let uncertain = describe_repeated_delivery(&record(DeliveryStatus::Uncertain));
        assert!(uncertain.contains("no confirmó si se guardó"));
        assert!(uncertain.contains("25/09/2026") || uncertain.contains("24/09/2026"));
    }

    #[test]
    fn optional_journal_records_only_when_present() {
        #[derive(Default)]
        struct Recorder(Vec<&'static str>);
        impl DeliveryJournal for &mut Recorder {
            fn before_commit(&mut self, _rows: usize) -> Result<(), String> {
                self.0.push("before");
                Ok(())
            }
            fn after_commit(&mut self, _rows: usize) {
                self.0.push("after");
            }
        }
        let mut recorder = Recorder::default();
        let mut journal = Some(&mut recorder);
        journal.before_commit(3).unwrap();
        journal.after_commit(3);
        assert_eq!(recorder.0, ["before", "after"]);
        let mut absent: Option<&mut Recorder> = None;
        assert!(absent.before_commit(3).is_ok());
    }

    #[test]
    fn confirmation_describes_the_target_without_secrets() {
        let mut value = target(DatabaseKind::SqlServer);
        value.connection.driver = "ODBC Driver 18".to_owned();
        value.connection.server = "tcp:db.example.com,1433".to_owned();
        let message = describe_remote_target(&value);
        assert!(message.contains("SQL Server"), "{message}");
        assert!(message.contains("tcp:db.example.com,1433"), "{message}");
        assert!(message.contains("[public].[ventas]"), "{message}");
        assert!(!message.contains("secret"), "{message}");
    }

    #[test]
    fn confirmation_names_the_driver_and_the_user() {
        // PROD-15: the native dialog said where, not which driver nor as whom.
        let mut value = target(DatabaseKind::SqlServer);
        value.connection.driver = "ODBC Driver 18 for SQL Server".to_owned();
        value.connection.server = "db".to_owned();
        let message = describe_remote_target(&value);
        assert!(
            message.contains("Controlador: ODBC Driver 18 for SQL Server"),
            "{message}"
        );
        assert!(message.contains("Usuario: ana"), "{message}");
        assert!(!message.contains("secret"), "{message}");
        assert!(!message.contains("DSN"), "{message}");

        value.connection.authentication = DatabaseAuthentication::Windows;
        value.connection.user = String::new();
        value.connection.password = String::new();
        let message = describe_remote_target(&value);
        assert!(
            message.contains("Usuario: tu usuario de Windows"),
            "{message}"
        );
        assert!(!message.contains("Usuario: ana"), "{message}");
    }

    /// AUD-003: the dialog says every decision of the connection, one per
    /// line, and the password in none.
    #[test]
    fn confirmation_shows_server_port_database_user_encryption_and_certificate() {
        let lines = |message: &str| {
            message
                .lines()
                .map(str::to_owned)
                .collect::<std::collections::HashSet<_>>()
        };

        let mut sql_server = target(DatabaseKind::SqlServer);
        sql_server.connection.driver = "ODBC Driver 18 for SQL Server".to_owned();
        sql_server.connection.server = "db.example.com,1433".to_owned();
        sql_server.connection.database = "Ventas".to_owned();
        sql_server.connection.password = "contraseña-secreta".to_owned();
        let message = describe_remote_target(&sql_server);
        let shown_lines = lines(&message);
        for expected in [
            "Servidor: db.example.com,1433",
            "Base de datos: Ventas",
            "Usuario: ana",
            "Cifrado: siempre",
            "Certificado: se verifica",
            "Controlador: ODBC Driver 18 for SQL Server",
        ] {
            assert!(shown_lines.contains(expected), "{expected}: {message}");
        }
        assert!(!message.contains("Puerto:"), "{message}");
        assert!(!message.contains("contraseña-secreta"), "{message}");

        sql_server.connection.authentication = DatabaseAuthentication::Windows;
        sql_server.connection.user = String::new();
        sql_server.connection.password = String::new();
        sql_server.connection.database = String::new();
        sql_server.connection.trust_server_certificate = true;
        let message = describe_remote_target(&sql_server);
        let shown_lines = lines(&message);
        assert!(
            shown_lines.contains("Usuario: tu usuario de Windows (autenticación de Windows)"),
            "{message}"
        );
        assert!(
            shown_lines.contains("Base de datos: la predeterminada del usuario"),
            "{message}"
        );
        assert!(
            shown_lines.contains(
                "Certificado: acepta cualquier certificado del servidor, sin verificarlo"
            ),
            "{message}"
        );

        let mut postgres = target(DatabaseKind::Postgresql);
        postgres.connection.port = "5433".to_owned();
        postgres.connection.database = "ventas".to_owned();
        postgres.connection.allow_unencrypted = false;
        let message = describe_remote_target(&postgres);
        let shown_lines = lines(&message);
        assert!(shown_lines.contains("Puerto: 5433"), "{message}");
        assert!(shown_lines.contains("Cifrado: obligatorio"), "{message}");
        assert!(
            shown_lines.contains("Certificado: no se verifica"),
            "{message}"
        );
        postgres.connection.allow_unencrypted = true;
        let message = describe_remote_target(&postgres);
        assert!(
            lines(&message)
                .contains("Cifrado: solo si el servidor lo ofrece (puede conectar sin cifrar)"),
            "{message}"
        );
        let mut mysql = target(DatabaseKind::Mysql);
        mysql.connection.allow_unencrypted = false;
        let message = describe_remote_target(&mysql);
        assert!(message.contains("Cifrado: obligatorio"), "{message}");
        assert!(
            lines(&message).contains("Certificado: no se verifica"),
            "{message}"
        );
    }

    /// AUD-003: a long field cannot push the rest of the text out of sight.
    #[test]
    fn confirmation_cuts_long_values_to_about_a_hundred_characters() {
        let long_server = "s".repeat(MAX_SERVER_CHARS);
        let mut value = target(DatabaseKind::SqlServer);
        value.connection.server = long_server.clone();
        value.connection.database = "d".repeat(200);
        value.connection.user = "u".repeat(200);
        value.connection.driver = "c".repeat(200);
        value.connection.trust_server_certificate = true;
        validate_database_target(&value).expect("campos largos pero válidos");
        let message = describe_remote_target(&value);
        assert!(!message.contains(&long_server), "{message}");
        let cut = format!("{}…", "s".repeat(MAX_SHOWN_CHARS));
        // The server appears in the sentence and in its line, both cut.
        assert_eq!(message.matches(&cut).count(), 2, "{message}");
        for (label, letter) in [
            ("Base de datos", "d"),
            ("Usuario", "u"),
            ("Controlador", "c"),
        ] {
            let cut = format!("{label}: {}…", letter.repeat(MAX_SHOWN_CHARS));
            assert!(message.contains(&cut), "{label}: {message}");
            assert!(
                !message.contains(&letter.repeat(MAX_SHOWN_CHARS + 1)),
                "{label}: {message}"
            );
        }
        // The cut keeps what the person needs to decide: the rest is still there.
        assert!(message.contains("Cifrado: siempre"), "{message}");
        assert!(
            message.contains("Certificado: acepta cualquier"),
            "{message}"
        );
        assert!(message.contains("¿Confirmas este destino?"), "{message}");

        // A value within the limit is shown whole and without the mark.
        assert_eq!(shown("  corto  "), "corto");
        let exactly = "x".repeat(MAX_SHOWN_CHARS);
        assert_eq!(shown(&exactly), exactly);
        assert_eq!(
            shown(&"ñ".repeat(MAX_SHOWN_CHARS + 1)).chars().count(),
            MAX_SHOWN_CHARS + 1
        );
    }

    #[test]
    fn a_driver_given_as_a_file_path_is_rejected() {
        let mut value = target(DatabaseKind::Postgresql);
        for driver in [
            r"C:\Temp\malicioso.dll",
            "/tmp/libmal.so",
            r"\\servidor\compartido\driver",
            "malicioso.DLL",
            "   ",
            "",
        ] {
            value.connection.driver = driver.to_owned();
            let error = validate_database_target(&value).expect_err(driver);
            assert!(
                error.contains("no la ruta de un archivo")
                    || error.contains("Elige el controlador"),
                "{driver}: {error}"
            );
        }
        value.connection.driver = "PostgreSQL Unicode(x64)".to_owned();
        assert!(validate_database_target(&value).is_ok());
    }

    /// AUD-002: the interface sends fields; a connection string, a DSN or any
    /// other attribute is not a field and does not deserialize.
    #[test]
    fn the_interface_cannot_send_a_connection_string() {
        let connection = serde_json::json!({
            "driver": "ODBC Driver 18 for SQL Server",
            "server": "localhost",
            "authentication": "windows",
        });
        let target_json = |connection: serde_json::Value| {
            serde_json::json!({
                "kind": "sqlserver",
                "connection": connection,
                "schema": "dbo",
                "table": "ventas",
                "tablePolicy": "create_only",
            })
        };
        let parsed =
            serde_json::from_value::<DatabaseTarget>(target_json(connection.clone())).unwrap();
        assert_eq!(
            parsed.connection.authentication,
            DatabaseAuthentication::Windows
        );
        assert!(parsed.connection.port.is_empty());

        let mut legacy = target_json(connection.clone());
        legacy["connectionString"] =
            serde_json::json!("Driver={ODBC Driver 17 for SQL Server};Server=localhost;");
        assert!(serde_json::from_value::<DatabaseTarget>(legacy).is_err());
        let mut without_connection = target_json(connection.clone());
        without_connection
            .as_object_mut()
            .unwrap()
            .remove("connection");
        without_connection["connectionString"] = serde_json::json!("Driver={x};Server=db;");
        assert!(serde_json::from_value::<DatabaseTarget>(without_connection).is_err());

        let mut extra = connection.clone();
        extra["dsn"] = serde_json::json!("Ventas");
        assert!(serde_json::from_value::<DatabaseTarget>(target_json(extra)).is_err());
        let mut unknown_authentication = connection.clone();
        unknown_authentication["authentication"] = serde_json::json!("kerberos");
        assert!(
            serde_json::from_value::<DatabaseTarget>(target_json(unknown_authentication)).is_err()
        );

        let source = |extra: Option<(&str, serde_json::Value)>| {
            let mut value = serde_json::json!({ "kind": "sqlserver", "connection": connection });
            if let Some((key, entry)) = extra {
                value[key] = entry;
            }
            serde_json::from_value::<DatabaseSource>(value)
        };
        assert!(source(None).is_ok());
        assert!(source(Some(("connectionString", serde_json::json!("Server=x;")))).is_err());
    }

    #[test]
    fn debug_output_never_shows_the_password() {
        let mut value = target(DatabaseKind::Postgresql);
        value.connection.password = "contraseña-única-123".to_owned();
        let shown = format!("{value:?} {:?}", value.connection);
        assert!(!shown.contains("contraseña-única-123"), "{shown}");
        assert!(shown.contains("[REDACTED]"), "{shown}");
    }

    #[test]
    fn only_installed_drivers_are_accepted() {
        let installed = [
            "ODBC Driver 18 for SQL Server".to_owned(),
            "PostgreSQL Unicode(x64)".to_owned(),
        ];
        assert!(ensure_installed_driver("odbc driver 18 for sql server", &installed).is_ok());
        let error = ensure_installed_driver("Otro", &installed).unwrap_err();
        assert!(error.contains("«Otro» no está instalado"), "{error}");
    }

    #[test]
    fn confirmation_fingerprint_changes_with_the_destination() {
        let base = target(DatabaseKind::Postgresql);
        let mut other_table = base.clone();
        other_table.table = "otra".to_owned();
        let mut other_policy = base.clone();
        other_policy.table_policy = DatabaseTablePolicy::Append;
        let mut other_server = base.clone();
        other_server.connection.server = "otro".to_owned();
        let mut other_user = base.clone();
        other_user.connection.user = "luis".to_owned();
        let mut other_password = base.clone();
        other_password.connection.password = "otra".to_owned();
        let mut other_encryption = base.clone();
        other_encryption.connection.allow_unencrypted = false;
        let fingerprint = remote_target_fingerprint(&base);
        assert_eq!(fingerprint, remote_target_fingerprint(&base.clone()));
        for other in [
            other_table,
            other_policy,
            other_server,
            other_user,
            other_password,
            other_encryption,
        ] {
            assert_ne!(fingerprint, remote_target_fingerprint(&other));
        }
        assert!(!fingerprint.contains("secret"));

        // Confirming a read never authorizes a write to the same server.
        let source = DatabaseSource {
            kind: base.kind,
            connection: base.connection.clone(),
        };
        assert_eq!(
            remote_source_fingerprint(&source),
            remote_source_fingerprint(&source.clone())
        );
        assert_ne!(remote_source_fingerprint(&source), fingerprint);
        let mut other_source = source.clone();
        other_source.connection.server = "otro".to_owned();
        assert_ne!(
            remote_source_fingerprint(&source),
            remote_source_fingerprint(&other_source)
        );
    }

    #[test]
    fn redacts_braced_secrets_that_contain_separators() {
        let mut value = target(DatabaseKind::SqlServer);
        value.connection.password = "p;a=ss".to_owned();
        let error = format_driver_error("falló", "Login failed; PWD={p;a=ss} p;a=ss", &value);
        assert!(!error.contains("p;a=ss"), "{error}");
        assert!(error.contains("[REDACTED]"), "{error}");

        // A closing brace is written doubled, and the driver may echo either form.
        value.connection.password = "{a}b;Encrypt=yes}".to_owned();
        let error = format_driver_error(
            "falló",
            "Pwd={{a}}b;Encrypt=yes}}} o {a}b;Encrypt=yes}",
            &value,
        );
        assert!(!error.contains("Encrypt=yes"), "{error}");
        assert!(!error.contains("a}b"), "{error}");
        assert!(error.contains("[REDACTED]"), "{error}");
    }

    #[test]
    fn transport_encryption_is_always_requested_unless_allowed_otherwise() {
        let with = |kind, change: &dyn Fn(&mut DatabaseConnection)| {
            let mut value = target(kind);
            value.connection.allow_unencrypted = false;
            value.connection.server = "db.example.com".to_owned();
            change(&mut value.connection);
            transport_encryption_issue(&value).map(|(severity, _)| severity)
        };
        for kind in [
            DatabaseKind::SqlServer,
            DatabaseKind::Postgresql,
            DatabaseKind::Mysql,
        ] {
            assert_eq!(with(kind, &|_| {}), None, "{kind:?}");
            // A local server is no reason to skip the encryption that was asked for.
            assert_eq!(
                with(kind, &|connection| connection.server =
                    "localhost".to_owned()),
                None
            );
        }
        assert_eq!(
            with(DatabaseKind::SqlServer, &|connection| connection
                .trust_server_certificate =
                true),
            Some("warning")
        );
        for kind in [DatabaseKind::Postgresql, DatabaseKind::Mysql] {
            assert_eq!(
                with(kind, &|connection| connection.allow_unencrypted = true),
                Some("warning")
            );
            assert_eq!(
                with(kind, &|connection| {
                    connection.allow_unencrypted = true;
                    connection.server = "localhost".to_owned();
                }),
                Some("info")
            );
        }
        // SEG-09: the host is compared whole, so lookalikes stay remote.
        for remote in [
            "localhost.example.com",
            "mi-localhost.red",
            "127.0.0.1.nip.io",
        ] {
            assert_eq!(
                with(DatabaseKind::Postgresql, &|connection| {
                    connection.allow_unencrypted = true;
                    connection.server = remote.to_owned();
                }),
                Some("warning"),
                "{remote}"
            );
        }
        for local in [
            "tcp:localhost,1433",
            ".\\SQLEXPRESS",
            "(localdb)\\MSSQLLocalDB",
            "[::1]:5432",
            "127.0.0.1:3306",
        ] {
            assert_eq!(
                with(DatabaseKind::Mysql, &|connection| {
                    connection.allow_unencrypted = true;
                    connection.server = local.to_owned();
                }),
                Some("info"),
                "{local}"
            );
        }
    }

    /// AUD-002: whatever the fields hold, the string Rust builds says exactly
    /// the attributes Rust chose, each once, and every value is what the field
    /// said, read with the grammar of the Driver Manager.
    #[test]
    fn hostile_fields_cannot_add_repeat_or_replace_attributes() {
        let hostile = DatabaseConnection {
            driver: "ODBC Driver 18 for SQL Server".to_owned(),
            server: r"a{b;Driver=C:\x.dll".to_owned(),
            port: String::new(),
            database: r"Ventas;FileDSN=C:\x.dsn".to_owned(),
            authentication: DatabaseAuthentication::Password,
            user: "sa}};Server=X};Server=Y".to_owned(),
            password: "{a}}b;Encrypt=yes}".to_owned(),
            trust_server_certificate: false,
            allow_unencrypted: false,
        };
        let text = hostile.connection_string(DatabaseKind::SqlServer).unwrap();
        let attributes = parse_odbc_connection_string(&text).unwrap();
        assert_eq!(
            attributes
                .iter()
                .map(|(key, _)| key.as_str())
                .collect::<Vec<_>>(),
            ["driver", "server", "database", "uid", "pwd", "encrypt"],
            "{text}"
        );
        assert_eq!(
            only_value(&attributes, "driver"),
            "ODBC Driver 18 for SQL Server"
        );
        assert_eq!(only_value(&attributes, "server"), r"a{b;Driver=C:\x.dll");
        assert_eq!(
            only_value(&attributes, "database"),
            r"Ventas;FileDSN=C:\x.dsn"
        );
        assert_eq!(only_value(&attributes, "uid"), "sa}};Server=X};Server=Y");
        assert_eq!(only_value(&attributes, "pwd"), "{a}}b;Encrypt=yes}");
        assert_eq!(only_value(&attributes, "encrypt"), "yes");

        // The same holds in the other engines, whose encryption setting is
        // Rust's, not the password's.
        for (kind, key, required) in [
            (DatabaseKind::Postgresql, "sslmode", "require"),
            (DatabaseKind::Mysql, "sslmode", "REQUIRED"),
        ] {
            let connection = DatabaseConnection {
                port: "5432".to_owned(),
                ..hostile.clone()
            };
            let text = connection.connection_string(kind).unwrap();
            let attributes = parse_odbc_connection_string(&text).unwrap();
            assert_eq!(only_value(&attributes, "server"), r"a{b;Driver=C:\x.dll");
            assert_eq!(only_value(&attributes, "port"), "5432");
            assert_eq!(only_value(&attributes, "pwd"), "{a}}b;Encrypt=yes}");
            assert_eq!(only_value(&attributes, key), required);
            assert_eq!(attributes.len(), 7, "{text}");
        }

        // The case of the audit's evidence: it never reaches the driver as such.
        let evidence = DatabaseConnection {
            driver: "ODBC Driver 17 for SQL Server".to_owned(),
            server: "localhost".to_owned(),
            database: "VentasRetail".to_owned(),
            authentication: DatabaseAuthentication::Password,
            user: "sa}};Server=SERVIDOR_FALSO};Server=localhost".to_owned(),
            ..hostile.clone()
        };
        let attributes = parse_odbc_connection_string(
            &evidence.connection_string(DatabaseKind::SqlServer).unwrap(),
        )
        .unwrap();
        assert_eq!(only_value(&attributes, "server"), "localhost");
        assert_eq!(
            only_value(&attributes, "uid"),
            "sa}};Server=SERVIDOR_FALSO};Server=localhost"
        );
    }

    /// AUD-003: the keys of the string are a closed list per engine. Whatever a
    /// field holds (attributes of the drivers that run statements, load
    /// libraries or redirect the host, or a repeated key), the string has
    /// exactly those keys, each once, in the same order.
    #[test]
    fn the_keys_of_the_string_are_a_closed_list_whatever_the_fields_hold() {
        let payloads = [
            ";Address=localhost",
            ";INITSTMT=DROP TABLE x",
            ";ConnSettings=SET search_path=evil",
            ";PLUGIN_DIR=C:\\evil",
            ";Server=otro",
            ";Uid=sa",
            "x;Server=otro;Uid=sa;Pwd=;Driver={y}",
            "x};Address=localhost;INITSTMT=1;{",
            "{a};ConnSettings=1",
            "=Server=otro",
        ];
        let engines = [
            (
                DatabaseKind::SqlServer,
                "ODBC Driver 18 for SQL Server",
                "",
                vec!["driver", "server", "database", "uid", "pwd", "encrypt"],
            ),
            (
                DatabaseKind::Postgresql,
                "PostgreSQL Unicode(x64)",
                "5432",
                vec![
                    "driver", "server", "port", "database", "uid", "pwd", "sslmode",
                ],
            ),
            (
                DatabaseKind::Mysql,
                "MySQL ODBC 9.1 Unicode Driver",
                "3306",
                vec![
                    "driver", "server", "port", "database", "uid", "pwd", "sslmode",
                ],
            ),
        ];
        for (kind, driver, port, keys) in engines {
            for purpose in [ConnectionPurpose::Delivery, ConnectionPurpose::Import] {
                let mut expected = keys.clone();
                if purpose == ConnectionPurpose::Import && kind == DatabaseKind::SqlServer {
                    expected.push("applicationintent");
                }
                for payload in payloads {
                    for field in ["server", "database", "user", "password"] {
                        let mut value = DatabaseConnection {
                            driver: driver.to_owned(),
                            server: "localhost".to_owned(),
                            port: port.to_owned(),
                            database: "ventas".to_owned(),
                            authentication: DatabaseAuthentication::Password,
                            user: "ana".to_owned(),
                            password: "secreta".to_owned(),
                            ..connection(kind)
                        };
                        match field {
                            "server" => value.server = payload.to_owned(),
                            "database" => value.database = payload.to_owned(),
                            "user" => value.user = payload.to_owned(),
                            _ => value.password = payload.to_owned(),
                        }
                        let text = value.connection_string_for(kind, purpose).unwrap();
                        let attributes = parse_odbc_connection_string(&text)
                            .unwrap_or_else(|e| panic!("{kind:?} {field} {payload}: {e}: {text}"));
                        assert_eq!(
                            attributes
                                .iter()
                                .map(|(key, _)| key.as_str())
                                .collect::<Vec<_>>(),
                            expected,
                            "{kind:?} {purpose:?} {field} {payload}: {text}"
                        );
                        let key = match field {
                            "user" => "uid",
                            "password" => "pwd",
                            other => other,
                        };
                        assert_eq!(only_value(&attributes, key), payload, "{field}: {text}");
                    }
                }
            }
        }
    }

    /// AUD-003: a connection with one more field than Rust knows (a connection
    /// string, an attribute of the driver) does not deserialize, in a delivery
    /// target and in an import source alike.
    #[test]
    fn a_connection_with_an_extra_field_does_not_deserialize() {
        let connection = serde_json::json!({
            "driver": "ODBC Driver 18 for SQL Server",
            "server": "localhost",
            "authentication": "password",
            "user": "ana",
            "password": "secreta",
        });
        let target_with = |connection: serde_json::Value| {
            serde_json::from_value::<DatabaseTarget>(serde_json::json!({
                "kind": "sqlserver",
                "connection": connection,
                "schema": "dbo",
                "table": "ventas",
                "tablePolicy": "create_only",
            }))
        };
        let source_with = |connection: serde_json::Value| {
            serde_json::from_value::<DatabaseSource>(
                serde_json::json!({ "kind": "sqlserver", "connection": connection }),
            )
        };
        assert!(target_with(connection.clone()).is_ok());
        assert!(source_with(connection.clone()).is_ok());
        for (name, entry) in [
            ("connectionString", "Driver={x};Server=otro;"),
            ("connection_string", "Driver={x};Server=otro;"),
            ("address", "otro"),
            ("initstmt", "DROP TABLE x"),
            ("INITSTMT", "DROP TABLE x"),
            ("connSettings", "SET x=1"),
            ("pluginDir", "C:\\evil"),
            ("applicationIntent", "ReadWrite"),
            ("encrypt", "no"),
        ] {
            let mut extra = connection.clone();
            extra[name] = serde_json::json!(entry);
            assert!(target_with(extra.clone()).is_err(), "{name}");
            assert!(source_with(extra).is_err(), "{name}");
        }
        // A field repeated in the JSON text is as invalid as an unknown one.
        let repeated = r#"{"driver":"d","server":"a","server":"b","authentication":"windows"}"#;
        assert!(serde_json::from_str::<DatabaseConnection>(repeated).is_err());
    }

    /// AUD-003: importing from SQL Server asks for a read-only connection; a
    /// delivery, its test and the other engines never carry the attribute, and
    /// the identity stored for deliveries does not know about it.
    #[test]
    fn only_the_import_from_sql_server_asks_for_a_read_only_connection() {
        let sql_server = DatabaseConnection {
            driver: "ODBC Driver 18 for SQL Server".to_owned(),
            ..connection(DatabaseKind::SqlServer)
        };
        let delivery = sql_server
            .connection_string(DatabaseKind::SqlServer)
            .unwrap();
        let import = sql_server
            .connection_string_for(DatabaseKind::SqlServer, ConnectionPurpose::Import)
            .unwrap();
        assert!(!delivery.to_ascii_lowercase().contains("applicationintent"));
        assert_eq!(import, format!("{delivery}ApplicationIntent=ReadOnly;"));
        assert_eq!(
            delivery,
            sql_server
                .connection_string_for(DatabaseKind::SqlServer, ConnectionPurpose::Delivery)
                .unwrap()
        );
        let parsed = parse_odbc_connection_string(&import).unwrap();
        assert_eq!(only_value(&parsed, "applicationintent"), "ReadOnly");

        for (kind, driver) in [
            (DatabaseKind::Postgresql, "PostgreSQL Unicode(x64)"),
            (DatabaseKind::Mysql, "MySQL ODBC 9.1 Unicode Driver"),
        ] {
            let value = DatabaseConnection {
                driver: driver.to_owned(),
                ..connection(kind)
            };
            for purpose in [ConnectionPurpose::Delivery, ConnectionPurpose::Import] {
                let text = value.connection_string_for(kind, purpose).unwrap();
                assert!(
                    !text.to_ascii_lowercase().contains("applicationintent"),
                    "{kind:?} {purpose:?}: {text}"
                );
            }
        }

        // An invalid connection is rejected for either purpose.
        let invalid = DatabaseConnection {
            server: String::new(),
            ..sql_server.clone()
        };
        assert!(invalid
            .connection_string_for(DatabaseKind::SqlServer, ConnectionPurpose::Import)
            .is_err());

        // The delivery identity is made of the delivery attributes only.
        let identity = remote_target_identity(&DatabaseTarget {
            kind: DatabaseKind::SqlServer,
            connection: sql_server,
            schema: "dbo".to_owned(),
            table: "ventas".to_owned(),
            table_policy: DatabaseTablePolicy::Append,
        });
        assert!(!identity.to_ascii_lowercase().contains("applicationintent"));
    }

    /// The external tests keep taking a whole connection string from the
    /// session; it becomes fields, and what Rust builds from them is the same
    /// connection the driver read (the string of the audit's evidence included).
    #[test]
    fn session_connection_strings_become_fields_and_back() {
        let session = "Driver={ODBC Driver 17 for SQL Server};Uid={sa}};Server=SERVIDOR_FALSO};Server=localhost;Database=VentasRetail;Trusted_Connection=yes;";
        let parsed = parse_odbc_connection_string(session).unwrap();
        assert_eq!(only_value(&parsed, "uid"), "sa};Server=SERVIDOR_FALSO");
        assert_eq!(parsed.iter().filter(|(key, _)| key == "server").count(), 1);

        let connection = connection_from_odbc_string(
            DatabaseKind::SqlServer,
            "Driver={ODBC Driver 18 for SQL Server};Server=db,1433;Uid=ana;Pwd={s;x};Encrypt=yes;TrustServerCertificate=yes;",
        );
        assert_eq!(connection.server, "db,1433");
        assert_eq!(connection.password, "s;x");
        assert!(connection.trust_server_certificate);
        assert_eq!(
            connection.connection_string(DatabaseKind::SqlServer).unwrap(),
            "Driver={ODBC Driver 18 for SQL Server};Server=db,1433;Uid=ana;Pwd={s;x};Encrypt=yes;TrustServerCertificate=yes;"
        );
        assert!(parse_odbc_connection_string("Server={sin cerrar").is_err());
        assert!(parse_odbc_connection_string("Server={a}b;X=1").is_err());
    }

    #[test]
    fn the_connection_string_is_the_one_the_interface_used_to_build() {
        // The cases of the TypeScript builders this replaces.
        let sql_server = DatabaseConnection {
            authentication: DatabaseAuthentication::Windows,
            user: String::new(),
            password: String::new(),
            database: "VentasRetail".to_owned(),
            ..DatabaseConnection {
                driver: "ODBC Driver 18 for SQL Server".to_owned(),
                server: "localhost".to_owned(),
                ..connection(DatabaseKind::SqlServer)
            }
        };
        assert_eq!(
            sql_server.connection_string(DatabaseKind::SqlServer).unwrap(),
            "Driver={ODBC Driver 18 for SQL Server};Server=localhost;Database=VentasRetail;Trusted_Connection=yes;Encrypt=yes;"
        );
        let with_user = DatabaseConnection {
            server: "db.example.com,1433".to_owned(),
            authentication: DatabaseAuthentication::Password,
            user: "ana".to_owned(),
            password: "s3creta".to_owned(),
            database: String::new(),
            trust_server_certificate: true,
            ..sql_server.clone()
        };
        assert_eq!(
            with_user.connection_string(DatabaseKind::SqlServer).unwrap(),
            "Driver={ODBC Driver 18 for SQL Server};Server=db.example.com,1433;Uid=ana;Pwd=s3creta;Encrypt=yes;TrustServerCertificate=yes;"
        );
        let braced_password = DatabaseConnection {
            password: "a;Encrypt=no".to_owned(),
            trust_server_certificate: false,
            ..with_user.clone()
        };
        assert!(braced_password
            .connection_string(DatabaseKind::SqlServer)
            .unwrap()
            .contains("Pwd={a;Encrypt=no};Encrypt=yes;"));

        for (value, written) in [
            ("a;Encrypt=no", "{a;Encrypt=no}"),
            ("x}y", "{x}}y}"),
            (" espacio", "{ espacio}"),
            ("espacio ", "{espacio }"),
            ("a=b", "{a=b}"),
            ("a{b", "{a{b}"),
            ("normal", "normal"),
            ("con espacio dentro", "con espacio dentro"),
            ("", ""),
        ] {
            assert_eq!(odbc_value(value), written, "{value:?}");
        }

        let mysql = DatabaseConnection {
            driver: "MySQL ODBC 9.1 Unicode Driver".to_owned(),
            server: "localhost".to_owned(),
            user: "root".to_owned(),
            password: "x".to_owned(),
            allow_unencrypted: false,
            ..connection(DatabaseKind::Mysql)
        };
        assert_eq!(
            mysql.connection_string(DatabaseKind::Mysql).unwrap(),
            "Driver={MySQL ODBC 9.1 Unicode Driver};Server=localhost;Uid=root;Pwd=x;SSLMODE=REQUIRED;"
        );
        let mysql_open = DatabaseConnection {
            database: "tienda".to_owned(),
            password: String::new(),
            allow_unencrypted: true,
            ..mysql.clone()
        };
        assert_eq!(
            mysql_open.connection_string(DatabaseKind::Mysql).unwrap(),
            "Driver={MySQL ODBC 9.1 Unicode Driver};Server=localhost;Database=tienda;Uid=root;Pwd=;SSLMODE=PREFERRED;"
        );
        let postgres = DatabaseConnection {
            driver: "PostgreSQL Unicode(x64)".to_owned(),
            port: " 5433 ".to_owned(),
            user: " root ".to_owned(),
            password: "a}b;Driver=x".to_owned(),
            allow_unencrypted: true,
            ..mysql.clone()
        };
        assert_eq!(
            postgres.connection_string(DatabaseKind::Postgresql).unwrap(),
            "Driver={PostgreSQL Unicode(x64)};Server=localhost;Port=5433;Uid=root;Pwd={a}}b;Driver=x};SSLmode=prefer;"
        );
        let postgres_strict = DatabaseConnection {
            allow_unencrypted: false,
            ..postgres
        };
        assert!(postgres_strict
            .connection_string(DatabaseKind::Postgresql)
            .unwrap()
            .ends_with("SSLmode=require;"));
    }

    #[test]
    fn invalid_field_combinations_are_rejected_with_a_way_out() {
        let sql_server = DatabaseConnection {
            driver: "ODBC Driver 18 for SQL Server".to_owned(),
            ..connection(DatabaseKind::SqlServer)
        };
        let postgres = DatabaseConnection {
            driver: "PostgreSQL Unicode(x64)".to_owned(),
            ..connection(DatabaseKind::Postgresql)
        };
        assert!(sql_server.validate(DatabaseKind::SqlServer).is_ok());
        assert!(postgres.validate(DatabaseKind::Postgresql).is_ok());

        let rejected = |kind, connection: DatabaseConnection, expected: &str| {
            let error = connection.validate(kind).unwrap_err();
            assert!(error.contains(expected), "{error}");
        };
        let windows = DatabaseConnection {
            authentication: DatabaseAuthentication::Windows,
            user: String::new(),
            password: String::new(),
            ..postgres.clone()
        };
        rejected(
            DatabaseKind::Postgresql,
            windows.clone(),
            "no admite la autenticación de Windows",
        );
        rejected(
            DatabaseKind::Mysql,
            windows.clone(),
            "no admite la autenticación de Windows",
        );
        assert!(DatabaseConnection {
            allow_unencrypted: false,
            ..windows.clone()
        }
        .validate(DatabaseKind::SqlServer)
        .is_ok());
        rejected(
            DatabaseKind::SqlServer,
            DatabaseConnection {
                user: "ana".to_owned(),
                allow_unencrypted: false,
                ..windows.clone()
            },
            "no se indica usuario",
        );
        for port in ["70000", "0", "65536", "33a", "-1", "1.5", "٣"] {
            rejected(
                DatabaseKind::Postgresql,
                DatabaseConnection {
                    port: port.to_owned(),
                    ..postgres.clone()
                },
                "entre 1 y 65535",
            );
        }
        for port in ["", " ", "1", "5432", "65535"] {
            assert!(DatabaseConnection {
                port: port.to_owned(),
                ..postgres.clone()
            }
            .validate(DatabaseKind::Mysql)
            .is_ok());
        }
        rejected(
            DatabaseKind::SqlServer,
            DatabaseConnection {
                port: "1433".to_owned(),
                ..sql_server.clone()
            },
            "servidor,1433",
        );
        rejected(
            DatabaseKind::SqlServer,
            DatabaseConnection {
                allow_unencrypted: true,
                ..sql_server.clone()
            },
            "solo se aplica a PostgreSQL y MySQL",
        );
        rejected(
            DatabaseKind::Postgresql,
            DatabaseConnection {
                trust_server_certificate: true,
                ..postgres.clone()
            },
            "solo se aplica a SQL Server",
        );
        rejected(
            DatabaseKind::Mysql,
            DatabaseConnection {
                user: "  ".to_owned(),
                ..postgres.clone()
            },
            "Indica el usuario de MySQL",
        );
        rejected(
            DatabaseKind::Mysql,
            DatabaseConnection {
                server: " ".to_owned(),
                ..postgres.clone()
            },
            "Indica el servidor",
        );
        // C:\x.dll is a file, not an installed driver.
        rejected(
            DatabaseKind::SqlServer,
            DatabaseConnection {
                driver: r"C:\x.dll".to_owned(),
                ..sql_server.clone()
            },
            "no la ruta de un archivo",
        );
        for control in ['\u{7}', '\n', '\r', '\0', '\u{2028}', '\u{2029}', '\u{85}'] {
            for field in 0..6 {
                let mut connection = DatabaseConnection {
                    port: "5432".to_owned(),
                    ..postgres.clone()
                };
                let value = format!("a{control}b");
                match field {
                    0 => connection.driver = value,
                    1 => connection.server = value,
                    2 => connection.port = value,
                    3 => connection.database = value,
                    4 => connection.user = value,
                    _ => connection.password = value,
                }
                rejected(DatabaseKind::Postgresql, connection, "control");
            }
        }
        for bidi in [
            '\u{200e}', '\u{200f}', '\u{202a}', '\u{202b}', '\u{202c}', '\u{202d}', '\u{202e}',
            '\u{2066}', '\u{2067}', '\u{2068}', '\u{2069}',
        ] {
            let value = format!("a{bidi}b");
            for field in 0..5 {
                let mut connection = DatabaseConnection {
                    port: "5432".to_owned(),
                    ..postgres.clone()
                };
                match field {
                    0 => connection.driver = value.clone(),
                    1 => connection.server = value.clone(),
                    2 => connection.port = value.clone(),
                    3 => connection.database = value.clone(),
                    _ => connection.user = value.clone(),
                }
                rejected(DatabaseKind::Postgresql, connection, "control");
            }
        }
        // Accents and ñ stay valid; the password is never shown in a dialog,
        // so it is not restricted.
        let accented = DatabaseConnection {
            server: "Señor-Ñandú".to_owned(),
            database: "Producción".to_owned(),
            user: "Ñandú".to_owned(),
            password: "a\u{202e}b".to_owned(),
            ..postgres.clone()
        };
        assert!(accented.validate(DatabaseKind::Postgresql).is_ok());
        rejected(
            DatabaseKind::Postgresql,
            DatabaseConnection {
                password: "x".repeat(MAX_PASSWORD_CHARS + 1),
                ..postgres.clone()
            },
            "no puede superar",
        );
        rejected(
            DatabaseKind::Postgresql,
            DatabaseConnection {
                server: "x".repeat(MAX_SERVER_CHARS + 1),
                ..postgres.clone()
            },
            "no puede superar",
        );
        // A password with a closing brace or a semicolon is a password, not a rejection.
        assert!(DatabaseConnection {
            password: "}};{".to_owned(),
            ..postgres.clone()
        }
        .validate(DatabaseKind::Postgresql)
        .is_ok());
    }

    #[test]
    fn time_and_zoned_datetime_columns_reach_the_server_readable() {
        // FUN-66
        assert_eq!(
            database_type(&DataType::Time, DatabaseKind::SqlServer),
            "TIME"
        );
        assert_eq!(
            database_type(&DataType::Time, DatabaseKind::Postgresql),
            "TIME"
        );
        use polars::prelude::TimeUnit;
        assert_eq!(
            time_of_day_text(((8 * 60 + 26) * 60) * 1_000_000_000),
            "08:26:00"
        );
        assert_eq!(time_of_day_text(1_000_123_456), "00:00:01.000123");
        assert_eq!(
            utc_datetime_text(1_291_191_960_000_000, TimeUnit::Microseconds),
            "2010-12-01 08:26:00.000000"
        );
    }

    #[test]
    fn connections_use_an_explicit_login_timeout() {
        assert_eq!(
            odbc_connection_options().login_timeout_sec,
            Some(ODBC_LOGIN_TIMEOUT_SEC)
        );
    }

    #[test]
    fn driver_timeouts_are_explained_without_leaking_secrets() {
        let error = format_driver_error(
            "No se pudo insertar una fila en la tabla remota",
            "State: HYT00, Native error: 0, Message: Query timeout expired; Pwd=secret",
            &target(DatabaseKind::SqlServer),
        );
        assert!(error.contains("no respondió a tiempo"), "{error}");
        assert!(!error.contains("secret"), "{error}");
    }

    #[test]
    fn listing_tables_has_its_own_short_timeout() {
        // AUD-013: login plus catalog query must fail within 45 s.
        const { assert!(ODBC_CATALOG_TIMEOUT_SEC < ODBC_STATEMENT_TIMEOUT_SEC) };
        const { assert!(ODBC_LOGIN_TIMEOUT_SEC as usize + ODBC_CATALOG_TIMEOUT_SEC <= 45) };
        let error = format_driver_error(
            "No se pudieron listar las tablas",
            "State: HYT00, Native error: 0, Message: Query timeout expired",
            &target(DatabaseKind::SqlServer),
        );
        assert!(error.contains("no respondió a tiempo"), "{error}");
        assert!(
            error.contains(&format!("lista de tablas {ODBC_CATALOG_TIMEOUT_SEC} s")),
            "{error}"
        );
    }

    #[test]
    fn cancelling_returns_control_while_the_driver_call_is_still_blocked() {
        let started = std::time::Instant::now();
        let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = std::sync::Arc::clone(&cancelled);
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(100));
            flag.store(true, std::sync::atomic::Ordering::SeqCst);
        });
        let result = tauri::async_runtime::block_on(run_blocking_unless_cancelled(
            || {
                std::thread::sleep(std::time::Duration::from_secs(10));
                Ok(1)
            },
            move || cancelled.load(std::sync::atomic::Ordering::SeqCst),
            "interrumpido",
        ));
        assert_eq!(
            result,
            Err(crate::dataset::OPERATION_CANCELLED_MESSAGE.to_owned())
        );
        assert!(started.elapsed().as_secs() < 2, "{:?}", started.elapsed());
    }

    #[test]
    fn without_cancelling_the_blocking_work_returns_its_result() {
        let result = tauri::async_runtime::block_on(run_blocking_unless_cancelled(
            || Ok(7),
            || false,
            "interrumpido",
        ));
        assert_eq!(result, Ok(7));
    }

    #[test]
    fn a_blocked_insert_batch_gives_up_long_before_the_statement_limit() {
        // PROD-16: Cancelar is only seen between batches, so a batch blocked by a
        // table lock held the delivery for the whole 300 s statement limit.
        const { assert!(ODBC_BATCH_TIMEOUT_SEC <= 30) };
        const { assert!(ODBC_BATCH_TIMEOUT_SEC < ODBC_STATEMENT_TIMEOUT_SEC) };
        let error = format_driver_error(
            "No se pudo insertar un lote en la tabla remota",
            "State: HYT00, Native error: 0, Message: Query timeout expired",
            &target(DatabaseKind::SqlServer),
        );
        assert!(
            error.contains(&format!("lote {ODBC_BATCH_TIMEOUT_SEC} s")),
            "{error}"
        );
    }

    /// Manual check against an unroutable address; needs an installed SQL
    /// Server ODBC driver (PowerShell, from `src-tauri`):
    /// `$env:COLUMNIA_TEST_HARNESS_MANIFEST = "1"; cargo test --lib unreachable_server -- --ignored`.
    #[test]
    #[ignore = "requiere un controlador ODBC de SQL Server instalado"]
    fn unreachable_server_fails_within_the_login_timeout() {
        let target = DatabaseTarget {
            kind: DatabaseKind::SqlServer,
            connection: DatabaseConnection {
                driver: "ODBC Driver 18 for SQL Server".to_owned(),
                server: "tcp:10.255.255.1,1433".to_owned(),
                user: "auditoria".to_owned(),
                password: "no-es-real".to_owned(),
                ..connection(DatabaseKind::SqlServer)
            },
            schema: String::new(),
            table: "ventas".to_owned(),
            table_policy: DatabaseTablePolicy::CreateOnly,
        };
        let started = std::time::Instant::now();
        let result = test_database_connection_with_cancel(&target, || false);
        assert!(result.is_err());
        assert!(
            started.elapsed().as_secs() <= u64::from(ODBC_LOGIN_TIMEOUT_SEC) + 10,
            "tardó {:?}",
            started.elapsed()
        );
    }

    #[test]
    fn validates_identifiers_and_redacts_secrets() {
        let mut value = target(DatabaseKind::Postgresql);
        assert!(validate_database_target(&value).is_ok());
        value.table = "ventas; DROP TABLE users".to_owned();
        assert!(validate_database_target(&value).is_err());
        let error = format_driver_error("falló", "Pwd=secret", &target(DatabaseKind::Postgresql));
        assert!(!error.contains("secret"));
        assert!(error.contains("[REDACTED]"));
    }

    #[test]
    fn generates_dialect_specific_schema_and_escaped_literals() {
        let frame = df!(
            "name" => &["O'Brien"],
            "amount" => &[10i64],
            "active" => &[true]
        )
        .expect("frame válido");
        assert_eq!(
            create_table_sql(&frame, "\"ventas\"", DatabaseKind::Postgresql),
            "CREATE TABLE \"ventas\" (\"name\" TEXT, \"amount\" BIGINT, \"active\" BOOLEAN)"
        );
        assert!(sql_parameter(
            frame.column("name").expect("name").get(0).expect("value"),
            &polars::prelude::DataType::String,
        )
        .is_ok());
        assert_eq!(
            qualified_table(&target(DatabaseKind::SqlServer)),
            "[public].[ventas]"
        );
    }

    #[test]
    fn streamed_parameters_preserve_adversarial_text_and_typed_nulls() {
        assert!(sql_parameter_text(Some("O'Brien; DROP TABLE users"), &DataType::String).is_ok());
        assert!(sql_parameter_text(Some("42"), &DataType::Int64).is_ok());
        assert!(sql_parameter_text(Some("not-a-number"), &DataType::Float64).is_err());
        assert!(sql_parameter_text(None, &DataType::String).is_ok());
        let columns = vec!["name".to_owned(), "active".to_owned()];
        assert_eq!(
            parameterized_insert_sql("`ventas`", &columns, 1),
            "INSERT INTO `ventas` (name, active) VALUES (?, ?)"
        );
        assert_eq!(
            parameterized_insert_sql("`ventas`", &columns, 3),
            "INSERT INTO `ventas` (name, active) VALUES (?, ?), (?, ?), (?, ?)"
        );
    }

    #[test]
    fn materialized_preflight_blocks_unsigned_overflow_and_non_finite_numbers() {
        let frame = df!(
            "unsigned" => &[u64::MAX, 42_u64],
            "measurement" => &[f64::NAN, f64::INFINITY]
        )
        .expect("frame numérico válido");
        let input = input_shape_from_frame(&frame).expect("se debe revisar cada valor numérico");

        assert_eq!(input[0].unrepresentable_integer_count, 1);
        assert_eq!(input[0].null_count, 0);
        assert_eq!(input[1].unrepresentable_decimal_count, 2);
        assert_eq!(input[1].null_count, 0);

        let report =
            assess_remote_export(&target(DatabaseKind::Postgresql), &input, false, false, &[]);
        assert!(
            !report.ready,
            "CreateOnly no debe aceptar valores convertidos silenciosamente en NULL"
        );
        assert!(report.issues.iter().any(|issue| {
            issue.column.as_deref() == Some("unsigned")
                && issue.message.contains("no caben en el parámetro Int64")
        }));
        assert!(report.issues.iter().any(|issue| {
            issue.column.as_deref() == Some("measurement") && issue.message.contains("no finito(s)")
        }));
    }

    #[test]
    fn source_backed_preflight_blocks_unsigned_overflow_and_non_finite_numbers() {
        let directory = tempdir().expect("se debe crear la carpeta temporal");
        let path = directory.path().join("unrepresentable.parquet");
        let mut frame = df!(
            "unsigned" => &[u64::MAX, 42_u64],
            "measurement" => &[f64::NAN, f64::INFINITY]
        )
        .expect("frame Parquet válido");
        let mut file = fs::File::create(&path).expect("se debe crear Parquet");
        ParquetWriter::new(&mut file)
            .finish(&mut frame)
            .expect("se debe escribir Parquet");

        let schema = frame.slice(0, 0);
        let input = input_shape_from_source(&path, DuckDbFileFormat::Parquet, &schema, 2, || false)
            .expect("el preflight debe inspeccionar todos los valores source-backed");

        assert_eq!(input[0].unrepresentable_integer_count, 1);
        assert_eq!(input[0].null_count, 0);
        assert_eq!(input[1].unrepresentable_decimal_count, 2);
        assert_eq!(input[1].null_count, 0);
        let report =
            assess_remote_export(&target(DatabaseKind::Postgresql), &input, false, false, &[]);
        assert!(!report.ready);
        assert_eq!(
            report
                .issues
                .iter()
                .filter(|issue| issue.severity == "blocking")
                .count(),
            2
        );
    }

    #[test]
    fn numeric_binding_rejects_overflow_and_non_finite_values_but_preserves_real_nulls() {
        assert!(sql_parameter(AnyValue::UInt64(u64::MAX), &DataType::UInt64).is_err());
        assert!(sql_parameter(AnyValue::Float64(f64::NAN), &DataType::Float64).is_err());
        assert!(sql_parameter(AnyValue::Float64(f64::INFINITY), &DataType::Float64).is_err());
        assert!(sql_parameter(AnyValue::UInt64(42), &DataType::UInt64).is_ok());
        assert!(sql_parameter(AnyValue::Null, &DataType::UInt64).is_ok());
        assert!(sql_parameter(AnyValue::Null, &DataType::Float64).is_ok());
        assert!(sql_parameter_text(Some("18446744073709551615"), &DataType::UInt64).is_err());
        assert!(sql_parameter_text(Some("NaN"), &DataType::Float64).is_err());
        assert!(sql_parameter_text(Some("Infinity"), &DataType::Float64).is_err());
    }

    #[test]
    fn boolean_parameters_parse_values_reject_invalid_text_and_map_dialect_types() {
        assert_eq!(parse_boolean_parameter("true"), Ok(true));
        assert_eq!(parse_boolean_parameter("FALSE"), Ok(false));
        assert!(parse_boolean_parameter("not-a-boolean").is_err());

        assert!(sql_parameter(AnyValue::Boolean(true), &DataType::Boolean).is_ok());
        assert!(sql_parameter(AnyValue::Boolean(false), &DataType::Boolean).is_ok());
        assert!(sql_parameter(AnyValue::Null, &DataType::Boolean).is_ok());
        assert!(sql_parameter_text(Some("not-a-boolean"), &DataType::Boolean).is_err());
        assert!(sql_parameter_text(None, &DataType::Boolean).is_ok());

        assert_eq!(
            database_type(&DataType::Boolean, DatabaseKind::Postgresql),
            "BOOLEAN"
        );
        assert_eq!(
            database_type(&DataType::Boolean, DatabaseKind::Mysql),
            "BOOLEAN"
        );
        assert_eq!(
            database_type(&DataType::Boolean, DatabaseKind::SqlServer),
            "BIT"
        );
    }

    #[test]
    fn maps_table_policies_to_explicit_ddl() {
        let mut table = target(DatabaseKind::Mysql);
        assert_eq!(
            drop_table_sql("`public`.`ventas`", table.kind),
            "DROP TABLE IF EXISTS `public`.`ventas`"
        );
        assert!(matches!(
            table.table_policy,
            DatabaseTablePolicy::CreateOnly
        ));
        table.table_policy = DatabaseTablePolicy::Replace;
        assert!(validate_database_target(&table).is_err());
    }

    #[test]
    fn preflight_explains_safe_new_table_and_blocks_conflicting_policies() {
        let input = vec![RemoteInputColumn {
            name: "name".to_owned(),
            data_type: "str".to_owned(),
            null_count: 0,
            maximum_length: Some(12),
            contains_nul: false,
            unrepresentable_integer_count: 0,
            unrepresentable_decimal_count: 0,
        }];
        let mut destination = target(DatabaseKind::Postgresql);
        let create = assess_remote_export(&destination, &input, false, false, &[]);
        assert!(create.ready);
        assert!(create.issues.iter().any(|issue| issue.category == "policy"));

        let exists = assess_remote_export(&destination, &input, true, true, &[]);
        assert!(!exists.ready);
        assert!(exists
            .issues
            .iter()
            .any(|issue| { issue.severity == "blocking" && issue.message.contains("ya existe") }));

        destination.table_policy = DatabaseTablePolicy::Append;
        let missing_append = assess_remote_export(&destination, &input, false, false, &[]);
        assert!(!missing_append.ready);
    }

    /// QA-20: the append analysis reads the dtype names Polars really gives
    /// (`str`, `i64`, `f64`, `bool`), from a real frame, not hand-written ones.
    #[test]
    fn append_preflight_accepts_a_real_frame_into_matching_columns() {
        let frame = df!(
            "nombre" => ["Ana", "Luis"],
            "unidades" => [1_i64, 2],
            "importe" => [1.5_f64, 2.0],
            "activo" => [true, false],
        )
        .unwrap();
        let input = input_shape_from_frame(&frame).expect("forma del dataset");
        assert_eq!(
            input[0].data_type,
            frame.column("nombre").unwrap().dtype().to_string()
        );
        let mut destination = target(DatabaseKind::SqlServer);
        destination.table_policy = DatabaseTablePolicy::Append;
        let existing = [
            ("nombre", "NVARCHAR(50)", Some(50)),
            ("unidades", "BIGINT", None),
            ("importe", "FLOAT", None),
            ("activo", "BIT", None),
        ]
        .map(|(name, data_type, maximum_length)| ExistingColumn {
            name: name.to_owned(),
            data_type: data_type.to_owned(),
            maximum_length,
            nullable: true,
            has_default: false,
        });
        let report = assess_remote_export(&destination, &input, true, true, &existing);
        assert!(report.ready, "{:?}", report.issues);
    }

    #[test]
    fn append_preflight_checks_types_nulls_lengths_nul_and_required_extra_columns() {
        let mut destination = target(DatabaseKind::SqlServer);
        destination.table_policy = DatabaseTablePolicy::Append;
        let input = vec![
            RemoteInputColumn {
                name: "name".to_owned(),
                data_type: "str".to_owned(),
                null_count: 2,
                maximum_length: Some(14),
                contains_nul: true,
                unrepresentable_integer_count: 0,
                unrepresentable_decimal_count: 0,
            },
            RemoteInputColumn {
                name: "amount".to_owned(),
                data_type: "bool".to_owned(),
                null_count: 0,
                maximum_length: None,
                contains_nul: false,
                unrepresentable_integer_count: 0,
                unrepresentable_decimal_count: 0,
            },
        ];
        let existing = vec![
            ExistingColumn {
                name: "name".to_owned(),
                data_type: "NVARCHAR(10)".to_owned(),
                maximum_length: Some(10),
                nullable: false,
                has_default: false,
            },
            ExistingColumn {
                name: "amount".to_owned(),
                data_type: "BIGINT".to_owned(),
                maximum_length: None,
                nullable: true,
                has_default: false,
            },
            ExistingColumn {
                name: "required_at_destination".to_owned(),
                data_type: "INT".to_owned(),
                maximum_length: None,
                nullable: false,
                has_default: false,
            },
        ];
        let report = assess_remote_export(&destination, &input, true, true, &existing);
        assert!(!report.ready);
        assert!(report.issues.iter().any(|issue| issue.category == "type"));
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.category == "nullability"));
        assert!(report.issues.iter().any(|issue| issue.category == "length"));
        assert!(report.issues.iter().any(|issue| issue.category == "value"));
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.message.contains("required_at_destination")));
    }

    #[test]
    fn append_preflight_accepts_real_polars_dtypes_against_a_table_columnia_created() {
        let frame = df!(
            "id" => [1i64, 2],
            "nombre" => ["Ana", "Luis"],
            "precio" => [1.5f64, 2.25],
            "activo" => [true, false],
        )
        .expect("frame")
        .lazy()
        .with_column(col("id").cast(DataType::Date).alias("fecha"))
        .collect()
        .expect("fecha");
        let input = input_shape_from_frame(&frame).expect("shape");
        assert_eq!(
            input
                .iter()
                .map(|column| column.data_type.as_str())
                .collect::<Vec<_>>(),
            ["i64", "str", "f64", "bool", "date"]
        );
        assert_eq!(input[1].maximum_length, Some(4));
        let existing = [
            ("id", "bigint", None),
            ("nombre", "nvarchar", Some(4000)),
            ("precio", "float", None),
            ("activo", "bit", None),
            ("fecha", "datetime2", None),
        ]
        .into_iter()
        .map(|(name, data_type, maximum_length)| ExistingColumn {
            name: name.to_owned(),
            data_type: data_type.to_owned(),
            maximum_length,
            nullable: true,
            has_default: false,
        })
        .collect::<Vec<_>>();
        let mut destination = target(DatabaseKind::SqlServer);
        destination.connection.server = "db".to_owned();
        destination.table_policy = DatabaseTablePolicy::Append;
        let report = assess_remote_export(&destination, &input, true, true, &existing);
        let blocking = report
            .issues
            .iter()
            .filter(|issue| issue.severity == "blocking")
            .map(|issue| issue.message.as_str())
            .collect::<Vec<_>>();
        assert!(blocking.is_empty(), "{blocking:?}");
    }

    #[test]
    fn type_family_uses_the_base_type_name() {
        for (name, family) in [
            ("str", TypeFamily::Text),
            ("f64", TypeFamily::Decimal),
            ("datetime[μs, UTC]", TypeFamily::Temporal),
            ("decimal[38,2]", TypeFamily::Decimal),
            ("NVARCHAR(10)", TypeFamily::Text),
            ("double precision", TypeFamily::Decimal),
            ("int unsigned", TypeFamily::Integer),
            ("timestamp with time zone", TypeFamily::Temporal),
            ("character varying", TypeFamily::Text),
            ("interval", TypeFamily::Unknown),
            ("image", TypeFamily::Unknown),
            ("point", TypeFamily::Unknown),
        ] {
            assert_eq!(type_family(name), family, "{name}");
        }
    }

    #[test]
    fn replace_policy_is_reported_as_explicit_warning() {
        let mut destination = target(DatabaseKind::Postgresql);
        destination.table_policy = DatabaseTablePolicy::Replace;
        let report = assess_remote_export(&destination, &[], true, true, &[]);
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.severity == "warning" && issue.category == "policy"));
    }

    fn external_target(kind: DatabaseKind, variable: &str) -> DatabaseTarget {
        let session_connection = env::var(variable).unwrap_or_else(|_| {
            panic!(
                "La prueba ODBC externa requiere la variable {variable} con una cadena de conexión de sesión."
            )
        });
        DatabaseTarget {
            kind,
            connection: connection_from_odbc_string(kind, &session_connection),
            schema: match kind {
                DatabaseKind::Postgresql => "public".to_owned(),
                DatabaseKind::Mysql => String::new(),
                DatabaseKind::SqlServer => "dbo".to_owned(),
            },
            table: "columnia_t6_external".to_owned(),
            table_policy: DatabaseTablePolicy::CreateOnly,
        }
    }

    /// The string the external tests hand straight to the driver.
    fn external_connection_string(target: &DatabaseTarget) -> String {
        target
            .connection
            .connection_string(target.kind)
            .expect("la conexión de sesión debe ser válida")
    }

    fn adversarial_frame() -> DataFrame {
        df!(
            "ordinal" => &[1i64, 2, 3],
            "text" => &[
                "O'Brien; DROP TABLE users \\ barra",
                "línea 1\nlínea 2 — Unicode",
                "comillas \"dobles\" y dos puntos: sí"
            ],
            "active" => &[Some(true), Some(false), None]
        )
        .expect("frame externo válido")
    }

    fn source_csv(path: &Path) {
        fs::write(
            path,
            "ordinal,text,active\n1,\"O'Brien; DROP TABLE users \\ barra\",true\n2,\"línea 1\nlínea 2 — Unicode\",false\n3,\"comillas \"\"dobles\"\" y dos puntos: sí\",\n",
        )
        .expect("CSV externo válido");
    }

    fn query_round_trip(
        target: &DatabaseTarget,
        expected: &[(i64, Option<&str>, &str)],
    ) -> Result<(), String> {
        let environment = Environment::new().map_err(|error| format!("ODBC: {error:?}"))?;
        let connection = environment
            .connect_with_connection_string(
                &external_connection_string(target),
                ConnectionOptions::default(),
            )
            .map_err(|error| format!("conexión ODBC: {error:?}"))?;
        let ordinal = quote_identifier("ordinal", target.kind);
        let text = quote_identifier("text", target.kind);
        let active = quote_identifier("active", target.kind);
        let table = qualified_table(target);
        let text_projection = match target.kind {
            DatabaseKind::Postgresql => format!("encode(convert_to({text}, 'UTF8'), 'hex')"),
            DatabaseKind::Mysql => format!("HEX({text})"),
            DatabaseKind::SqlServer => text.clone(),
        };
        let active_true = match target.kind {
            DatabaseKind::Postgresql => active.clone(),
            DatabaseKind::Mysql | DatabaseKind::SqlServer => format!("{active} = 1"),
        };
        let query = format!(
            "SELECT {ordinal}, {text_projection}, CASE WHEN {active} IS NULL THEN 'null' WHEN {active_true} THEN 'true' ELSE 'false' END FROM {table} ORDER BY {ordinal}"
        );
        let cursor = connection
            .execute(&query, (), None)
            .map_err(|error| format!("consulta ODBC: {error:?}"))?
            .ok_or_else(|| "La consulta ODBC no devolvió filas.".to_owned())?;
        let mut cursor = cursor
            .bind_buffer(RowVec::<(i64, VarWCharArray<512>, VarWCharArray<8>)>::new(
                16,
            ))
            .map_err(|error| format!("buffer ODBC: {error:?}"))?;
        let mut actual = Vec::new();
        while let Some(batch) = cursor
            .fetch()
            .map_err(|error| format!("lectura ODBC: {error:?}"))?
        {
            for (ordinal, text, active) in batch.iter() {
                // PostgreSQL and MySQL project the text as hex, whose letter
                // case varies by server; SQL Server returns the text itself.
                let text = text.as_utf16().map(|value| match target.kind {
                    DatabaseKind::Postgresql | DatabaseKind::Mysql => {
                        value.to_string().to_ascii_lowercase()
                    }
                    DatabaseKind::SqlServer => value.to_string(),
                });
                let active = active
                    .as_utf16()
                    .map(|value| value.to_string())
                    .unwrap_or_default();
                actual.push((*ordinal, text, active));
            }
        }
        let expected = expected
            .iter()
            .map(|(ordinal, text, active)| {
                let text = text.map(|text| match target.kind {
                    DatabaseKind::Postgresql | DatabaseKind::Mysql => text
                        .as_bytes()
                        .iter()
                        .map(|byte| format!("{byte:02x}"))
                        .collect::<String>(),
                    DatabaseKind::SqlServer => text.to_owned(),
                });
                (*ordinal, text, (*active).to_owned())
            })
            .collect::<Vec<_>>();
        assert_eq!(actual, expected, "round-trip ODBC alteró los valores");
        Ok(())
    }

    fn execute_external_sql(target: &DatabaseTarget, sql: &str) -> Result<(), String> {
        let environment = Environment::new().map_err(|error| format!("ODBC: {error:?}"))?;
        let connection = environment
            .connect_with_connection_string(
                &external_connection_string(target),
                ConnectionOptions::default(),
            )
            .map_err(|error| format!("conexión ODBC: {error:?}"))?;
        connection
            .execute(sql, (), None)
            .map(|_| ())
            .map_err(|error| format!("SQL externo: {error:?}"))
    }

    fn run_external_round_trip(target: &DatabaseTarget, suffix: &str) -> Result<(), String> {
        let expected = [
            (1, Some("O'Brien; DROP TABLE users \\ barra"), "true"),
            (2, Some("línea 1\nlínea 2 — Unicode"), "false"),
            (3, Some("comillas \"dobles\" y dos puntos: sí"), "null"),
        ];
        let mut frame_target = target.clone();
        frame_target.table = format!("columnia_t6_{suffix}_frame");
        export_frame(&adversarial_frame(), &frame_target, |_, _| {}, || false, ())?;
        query_round_trip(&frame_target, &expected)?;

        let directory = tempdir().map_err(|error| format!("temp externo: {error}"))?;
        let source = directory.path().join("adversarial.csv");
        source_csv(&source);
        let schema = adversarial_frame();
        let mut source_target = target.clone();
        source_target.table = format!("columnia_t6_{suffix}_source");
        export_source_backed(
            &source,
            DuckDbFileFormat::Delimited { delimiter: b',' },
            &schema,
            schema.height(),
            &source_target,
            |_, _| {},
            || false,
            (),
        )?;
        query_round_trip(&source_target, &expected)?;

        execute_external_sql(
            &frame_target,
            &drop_table_sql(&qualified_table(&frame_target), frame_target.kind),
        )?;
        execute_external_sql(
            &source_target,
            &drop_table_sql(&qualified_table(&source_target), source_target.kind),
        )?;
        Ok(())
    }

    fn unique_suffix(prefix: &str) -> String {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("reloj del sistema válido")
            .as_millis();
        format!("{prefix}_{millis}")
    }

    #[test]
    #[ignore = "requiere servidores y controladores ODBC reales configurados por variables de sesión"]
    fn external_odbc_round_trip_postgresql() {
        let target = external_target(DatabaseKind::Postgresql, "COLUMNIA_ODBC_POSTGRESQL");
        run_external_round_trip(&target, &unique_suffix("pg"))
            .expect("round-trip PostgreSQL completo");
    }

    #[test]
    #[ignore = "requiere servidor y controlador ODBC MySQL reales configurados por variables de sesión"]
    fn external_odbc_round_trip_mysql_with_and_without_no_backslash_escapes() {
        let target = external_target(DatabaseKind::Mysql, "COLUMNIA_ODBC_MYSQL");
        run_external_round_trip(&target, &unique_suffix("mysql_default"))
            .expect("round-trip MySQL por defecto completo");
        execute_external_sql(&target, "SET GLOBAL sql_mode = 'NO_BACKSLASH_ESCAPES'")
            .expect("activar NO_BACKSLASH_ESCAPES");
        let result = run_external_round_trip(&target, &unique_suffix("mysql_no_backslash"));
        let _ = execute_external_sql(&target, "SET GLOBAL sql_mode = 'STRICT_TRANS_TABLES,ERROR_FOR_DIVISION_BY_ZERO,NO_AUTO_CREATE_USER,NO_ENGINE_SUBSTITUTION'");
        result.expect("round-trip MySQL con NO_BACKSLASH_ESCAPES completo");
    }

    /// T10-14: rows per second one row per statement versus batched, measured
    /// inside a transaction that is rolled back.
    #[test]
    #[ignore = "requiere servidor y controlador ODBC SQL Server reales configurados por variables de sesión"]
    fn external_odbc_append_batching_benchmark_sql_server() {
        let mut target = external_target(DatabaseKind::SqlServer, "COLUMNIA_ODBC_SQLSERVER");
        target.table = unique_suffix("columnia_t10_14");
        let rows = 5_000usize;
        let frame = df!(
            "id" => (0..rows as i64).collect::<Vec<_>>(),
            "name" => (0..rows).map(|index| format!("cliente {index}")).collect::<Vec<_>>(),
            "amount" => (0..rows).map(|index| index as f64 * 1.5).collect::<Vec<_>>(),
            "active" => (0..rows).map(|index| index % 2 == 0).collect::<Vec<_>>()
        )
        .expect("frame de medición");
        let table = qualified_table(&target);
        execute_external_sql(&target, &create_table_sql(&frame, &table, target.kind))
            .expect("tabla de medición");
        let environment = Environment::new().expect("ODBC");
        let connection = environment
            .connect_with_connection_string(
                &external_connection_string(&target),
                odbc_connection_options(),
            )
            .expect("conexión ODBC");
        let columns = frame
            .get_column_names()
            .iter()
            .map(|name| quote_identifier(name, target.kind))
            .collect::<Vec<_>>();
        let measure = |rows_per_batch: usize| {
            connection.set_autocommit(false).expect("transacción");
            let started = std::time::Instant::now();
            let mut inserter = BatchInserter::with_rows_per_batch(
                &connection,
                &table,
                columns.clone(),
                &target,
                rows_per_batch,
            )
            .expect("inserción preparada");
            let mut sent = 0;
            for row_index in 0..frame.height() {
                let params = frame
                    .columns()
                    .iter()
                    .map(|column| sql_parameter(column.get(row_index).unwrap(), column.dtype()))
                    .collect::<Result<Vec<_>, String>>()
                    .expect("parámetros");
                sent += inserter.push_row(params).expect("lote");
            }
            sent += inserter.finish().expect("último lote");
            let elapsed = started.elapsed();
            connection.rollback().expect("rollback");
            assert_eq!(sent, rows);
            elapsed
        };
        let row_by_row = measure(1);
        let batched = measure(rows_per_insert(columns.len()));
        execute_external_sql(&target, &drop_table_sql(&table, target.kind))
            .expect("limpieza de la tabla de medición");
        println!(
            "T10-14 {rows} filas x {} columnas: fila a fila {:.0} ms ({:.0} filas/s); por lotes de {} {:.0} ms ({:.0} filas/s)",
            columns.len(),
            row_by_row.as_secs_f64() * 1_000.0,
            rows as f64 / row_by_row.as_secs_f64(),
            rows_per_insert(columns.len()),
            batched.as_secs_f64() * 1_000.0,
            rows as f64 / batched.as_secs_f64(),
        );
        assert!(batched < row_by_row);
    }

    #[test]
    #[ignore = "requiere servidor y controlador ODBC SQL Server reales configurados por variables de sesión"]
    fn external_sql_server_preflight_names_long_and_case_duplicate_columns() {
        // FUN-20 and FUN-21 against a real server, from a file-backed dataset.
        let target = external_target(DatabaseKind::SqlServer, "COLUMNIA_ODBC_SQLSERVER");
        let long = "x".repeat(129);
        let mut frame = df!(
            "Dup" => ["a", "b"],
            "dup" => ["c", "d"],
            long.as_str() => [1_i64, 2],
        )
        .unwrap();
        let directory = tempdir().unwrap();
        let path = directory.path().join("hostil.parquet");
        ParquetWriter::new(fs::File::create(&path).unwrap())
            .finish(&mut frame)
            .unwrap();
        let report = preflight_source_backed(
            &path,
            DuckDbFileFormat::Parquet,
            &frame.clear(),
            2,
            &target,
            || false,
        )
        .expect("el análisis no debe confundir los nombres con un archivo cambiado");
        assert!(!report.ready);
        let messages = report
            .issues
            .iter()
            .map(|issue| issue.message.as_str())
            .collect::<Vec<_>>();
        assert!(
            messages
                .iter()
                .any(|message| message.contains("'Dup' y 'dup'")),
            "{messages:?}"
        );
        assert!(
            messages
                .iter()
                .any(|message| message.contains("como máximo 128")),
            "{messages:?}"
        );
    }

    #[test]
    #[ignore = "requiere servidor y controlador ODBC SQL Server reales configurados por variables de sesión"]
    fn external_sql_server_reads_time_and_zoned_datetime_columns() {
        // FUN-66 against a real server: a TIME column and a UTC datetime.
        use polars::prelude::{NamedFrom, Series, TimeUnit, TimeZone};
        let mut target = external_target(DatabaseKind::SqlServer, "COLUMNIA_ODBC_SQLSERVER");
        target.table = unique_suffix("columnia_fun66");
        let hora = Series::new("hora".into(), &[((8 * 60 + 26) * 60) * 1_000_000_000_i64])
            .cast(&DataType::Time)
            .expect("hora");
        let momento = Series::new("momento".into(), &[1_291_191_960_000_000_i64])
            .cast(&DataType::Datetime(
                TimeUnit::Microseconds,
                Some(TimeZone::UTC),
            ))
            .expect("momento");
        let frame = DataFrame::new(1, vec![hora.into(), momento.into()]).expect("frame");
        let result = export_frame(&frame, &target, |_, _| {}, || false, ());
        let table = qualified_table(&target);
        let count = result.and_then(|_| {
            let environment = Environment::new().map_err(|error| format!("ODBC: {error:?}"))?;
            let connection = environment
                .connect_with_connection_string(
                    &external_connection_string(&target),
                    ConnectionOptions::default(),
                )
                .map_err(|error| format!("conexión ODBC: {error:?}"))?;
            let mut cursor = connection
                .execute(
                    &format!(
                        "SELECT COUNT(*) FROM {table} WHERE [hora] = '08:26:00' AND [momento] = '2010-12-01 08:26:00' AND SQL_VARIANT_PROPERTY([hora], 'BaseType') = 'time'"
                    ),
                    (),
                    None,
                )
                .map_err(|error| format!("consulta ODBC: {error:?}"))?
                .ok_or_else(|| "sin filas".to_owned())?;
            let mut row = cursor
                .next_row()
                .map_err(|error| format!("fila ODBC: {error:?}"))?
                .ok_or_else(|| "sin filas".to_owned())?;
            let mut count = 0_i64;
            row.get_data(1, &mut count)
                .map_err(|error| format!("dato ODBC: {error:?}"))?;
            Ok(count)
        });
        let _ = execute_external_sql(&target, &drop_table_sql(&table, target.kind));
        assert_eq!(count.expect("entrega y lectura"), 1);
    }

    #[test]
    #[ignore = "requiere servidor y controlador ODBC SQL Server reales configurados por variables de sesión"]
    fn external_odbc_round_trip_sql_server() {
        let target = external_target(DatabaseKind::SqlServer, "COLUMNIA_ODBC_SQLSERVER");
        run_external_round_trip(&target, &unique_suffix("sqlserver"))
            .expect("round-trip SQL Server completo");
    }

    /// FUN-03: a table that Columnia created accepts the same dataset again
    /// with the append policy.
    #[test]
    #[ignore = "requiere servidor y controlador ODBC SQL Server reales configurados por variables de sesión"]
    fn external_odbc_append_to_a_table_columnia_created_sql_server() {
        let mut target = external_target(DatabaseKind::SqlServer, "COLUMNIA_ODBC_SQLSERVER");
        target.table = unique_suffix("columnia_fun03");
        let frame = df!(
            "id" => [1i64, 2],
            "nombre" => ["Ana", "Luis"],
            "precio" => [1.5f64, 2.25],
            "activo" => [true, false],
        )
        .expect("frame")
        .lazy()
        .with_column(col("id").cast(DataType::Date).alias("fecha"))
        .collect()
        .expect("fecha");
        let input = input_shape_from_frame(&frame).expect("shape");
        export_frame(&frame, &target, |_, _| {}, || false, ()).expect("crear tabla");
        target.table_policy = DatabaseTablePolicy::Append;
        let result = preflight_export_with_cancel(&target, &input, &|| false).and_then(|report| {
            let blocking = report
                .issues
                .iter()
                .filter(|issue| issue.severity == "blocking")
                .map(|issue| issue.message.clone())
                .collect::<Vec<_>>();
            if blocking.is_empty() {
                export_frame(&frame, &target, |_, _| {}, || false, ())
            } else {
                Err(blocking.join(" | "))
            }
        });
        let _ = execute_external_sql(
            &target,
            &drop_table_sql(&qualified_table(&target), target.kind),
        );
        result.expect("añadir a la tabla creada por Columnia");
    }

    /// A SQL Server source from the connection string of a session variable.
    fn sql_server_source(connection_string: &str) -> DatabaseSource {
        DatabaseSource {
            kind: DatabaseKind::SqlServer,
            connection: connection_from_odbc_string(DatabaseKind::SqlServer, connection_string),
        }
    }

    /// AUD-008: the interface may allow an unencrypted channel, so the dialog
    /// Rust shows warns about it for a remote server and about a certificate
    /// that is not checked; a local server or a strict connection gets no warning.
    #[test]
    fn the_import_dialog_warns_about_an_unencrypted_or_unchecked_channel() {
        let source = |kind, server: &str, allow_unencrypted, trust| DatabaseSource {
            kind,
            connection: DatabaseConnection {
                driver: "Driver".to_owned(),
                server: server.to_owned(),
                port: String::new(),
                database: String::new(),
                authentication: DatabaseAuthentication::Password,
                user: "ana".to_owned(),
                password: "x".to_owned(),
                trust_server_certificate: trust,
                allow_unencrypted,
            },
        };
        let remote = describe_remote_source(&source(
            DatabaseKind::Postgresql,
            "db.example.com",
            true,
            false,
        ));
        assert!(
            remote.contains("Atención: Se permitió conectar sin cifrado"),
            "{remote}"
        );
        assert!(remote.contains("viajarán sin cifrar"), "{remote}");
        let certificate = describe_remote_source(&source(
            DatabaseKind::SqlServer,
            "db.example.com",
            false,
            true,
        ));
        assert!(
            certificate.contains("acepta cualquier certificado"),
            "{certificate}"
        );
        assert!(certificate.contains("Atención:"), "{certificate}");
        for quiet in [
            source(DatabaseKind::Postgresql, "localhost", true, false),
            source(DatabaseKind::Postgresql, "db.example.com", false, false),
            source(DatabaseKind::SqlServer, "db.example.com", false, false),
        ] {
            let message = describe_remote_source(&quiet);
            assert!(!message.contains("Atención:"), "{message}");
        }
    }

    fn listed(schema: &str, name: &str) -> DatabaseSourceTable {
        DatabaseSourceTable {
            schema: schema.to_owned(),
            name: name.to_owned(),
            is_view: false,
        }
    }

    #[test]
    fn import_accepts_the_three_engines_and_only_safe_connection_fields() {
        let valid = sql_server_source(
            "Driver={ODBC Driver 18 for SQL Server};Server=localhost;Trusted_Connection=yes",
        );
        assert!(validate_database_source(&valid).is_ok());
        for kind in [DatabaseKind::Postgresql, DatabaseKind::Mysql] {
            let source = DatabaseSource {
                kind,
                connection: DatabaseConnection {
                    driver: "PostgreSQL Unicode(x64)".to_owned(),
                    server: "db".to_owned(),
                    ..connection(kind)
                },
            };
            assert!(validate_database_source(&source).is_ok());
        }
        let with = |change: &dyn Fn(&mut DatabaseConnection)| {
            let mut source = valid.clone();
            change(&mut source.connection);
            validate_database_source(&source)
        };
        assert!(with(&|connection| connection.server = "  ".to_owned()).is_err());
        assert!(with(&|connection| connection.driver = r"C:\x\evil.dll".to_owned()).is_err());
        assert!(with(&|connection| connection.server = "a\u{7}".to_owned()).is_err());
        assert!(with(&|connection| connection.port = "1433".to_owned()).is_err());
        // A value that looks like attributes is only a value.
        assert!(with(&|connection| connection.server = "a;FileDSN=x.dsn".to_owned()).is_ok());
    }

    #[test]
    fn import_row_limit_and_names_are_bounded() {
        assert!(validate_import_row_limit(0).is_err());
        assert!(validate_import_row_limit(1).is_ok());
        assert!(validate_import_row_limit(MAX_IMPORT_ROWS).is_ok());
        assert!(validate_import_row_limit(MAX_IMPORT_ROWS + 1).is_err());
        assert!(validate_source_object_name("", "El esquema", true).is_ok());
        assert!(validate_source_object_name("", "una tabla", false).is_err());
        assert!(validate_source_object_name("Ventas por año", "una tabla", false).is_ok());
        assert!(validate_source_object_name("a\u{0}b", "una tabla", false).is_err());
        assert!(validate_source_object_name(
            &"x".repeat(MAX_IDENTIFIER_CHARS + 1),
            "una tabla",
            false
        )
        .is_err());
    }

    /// The only statement an import sends: a quoted SELECT of a listed object.
    #[test]
    fn import_reads_one_listed_object_with_quoted_names_and_one_extra_row() {
        let tables = vec![
            listed("dbo", "Ventas"),
            listed("ventas", "Ventas"),
            listed("dbo", "Clientes]; DROP TABLE x;--"),
        ];
        assert_eq!(
            find_listed_table(&tables, "dbo", "Ventas"),
            Some(&tables[0])
        );
        assert_eq!(
            find_listed_table(&tables, "ventas", "Ventas"),
            Some(&tables[1])
        );
        assert_eq!(find_listed_table(&tables, "dbo", "Otra"), None);
        assert_eq!(
            find_listed_table(&tables, "", "Ventas"),
            None,
            "el esquema debe coincidir exactamente"
        );
        assert_eq!(
            select_rows_sql(&tables[0], 1000, DatabaseKind::SqlServer),
            "SELECT TOP (1001) * FROM [dbo].[Ventas]"
        );
        assert_eq!(
            select_rows_sql(&tables[2], 10, DatabaseKind::SqlServer),
            "SELECT TOP (11) * FROM [dbo].[Clientes]]; DROP TABLE x;--]"
        );
        assert_eq!(
            select_rows_sql(&listed("", "t"), 5, DatabaseKind::SqlServer),
            "SELECT TOP (6) * FROM [t]"
        );
        assert_eq!(
            select_rows_sql(
                &listed("public", "ventas\"; DROP"),
                10,
                DatabaseKind::Postgresql
            ),
            "SELECT * FROM \"public\".\"ventas\"\"; DROP\" LIMIT 11"
        );
        assert_eq!(
            select_rows_sql(&listed("tienda", "pedidos`x"), 10, DatabaseKind::Mysql),
            "SELECT * FROM `tienda`.`pedidos``x` LIMIT 11"
        );
    }

    /// Each engine hides its own schemas; MySQL lists databases as catalogs.
    #[test]
    fn listed_tables_skip_each_engines_own_schemas() {
        let row = |kind, catalog: &str, schema: &str| {
            source_table(kind, catalog, schema, "t", "TABLE").map(|table| table.schema)
        };
        assert_eq!(
            row(DatabaseKind::SqlServer, "Ventas", "dbo"),
            Some("dbo".to_owned())
        );
        assert_eq!(row(DatabaseKind::SqlServer, "Ventas", "sys"), None);
        assert_eq!(
            row(DatabaseKind::Postgresql, "ventas", "public"),
            Some("public".to_owned())
        );
        assert_eq!(row(DatabaseKind::Postgresql, "ventas", "pg_catalog"), None);
        assert_eq!(row(DatabaseKind::Postgresql, "ventas", "pg_toast"), None);
        assert_eq!(
            row(DatabaseKind::Postgresql, "ventas", "information_schema"),
            None
        );
        assert_eq!(
            row(DatabaseKind::Mysql, "tienda", ""),
            Some("tienda".to_owned())
        );
        assert_eq!(row(DatabaseKind::Mysql, "mysql", ""), None);
        assert_eq!(row(DatabaseKind::Mysql, "performance_schema", ""), None);
        assert_eq!(row(DatabaseKind::Mysql, "", ""), Some(String::new()));
        assert_eq!(
            source_table(DatabaseKind::Mysql, "tienda", "", "", "TABLE"),
            None
        );
        assert!(
            source_table(DatabaseKind::Postgresql, "", "public", "v", "VIEW")
                .unwrap()
                .is_view
        );
    }

    #[test]
    fn imported_columns_and_file_names_are_usable() {
        assert_eq!(
            unique_column_names(vec![
                "id".into(),
                "".into(),
                "ID".into(),
                "id".into(),
                "nombre".into()
            ]),
            vec!["id", "column_2", "ID_2", "id_3", "nombre"]
        );
        assert_eq!(
            import_file_name(&listed("dbo", "Ventas")),
            "dbo.Ventas.parquet"
        );
        assert_eq!(
            import_file_name(&listed("", "Ventas por año")),
            "Ventas por año.parquet"
        );
        assert_eq!(
            import_file_name(&listed("dbo", "a/b\\c:*?")),
            "dbo.a_b_c___.parquet"
        );
        assert_eq!(import_file_name(&listed("", "...")), "tabla.parquet");
    }

    #[test]
    fn tables_sort_by_schema_and_name_without_regard_to_case() {
        let mut tables = vec![
            listed("dbo", "ventas"),
            listed("audit", "Log"),
            listed("dbo", "Clientes"),
        ];
        sort_source_tables(&mut tables);
        let order = tables
            .iter()
            .map(|table| format!("{}.{}", table.schema, table.name))
            .collect::<Vec<_>>();
        assert_eq!(order, vec!["audit.Log", "dbo.Clientes", "dbo.ventas"]);
    }

    /// Confirming a read never stands for a write to the same server, and the
    /// dialog never shows the password.
    #[test]
    fn reading_is_confirmed_apart_from_writing_and_without_secrets() {
        let connection =
            "Driver={ODBC Driver 18 for SQL Server};Server=db.example.com;Uid=ana;Pwd={s3cr;et}";
        let source = sql_server_source(connection);
        let target = source_as_target(&source);
        assert_ne!(
            remote_source_fingerprint(&source),
            remote_target_fingerprint(&target)
        );
        let message = describe_remote_source(&source);
        assert!(message.contains("db.example.com"));
        assert!(message.contains("Usuario: ana"));
        assert!(message.contains("Solo leerá"));
        // AUD-003: the promise is Columnia's; the guarantee is the permission.
        assert!(
            message.contains("usa un usuario que solo tenga permiso de lectura"),
            "{message}"
        );
        assert!(message.contains("Cifrado: siempre"), "{message}");
        assert!(!message.contains("s3cr"));
    }

    /// Writes a text frame the way an import does: one batch through the
    /// streaming writer, then persisted.
    fn write_import_parquet(
        directory: &Path,
        file_name: &str,
        keep: Option<&Path>,
        frame: &mut DataFrame,
    ) -> Result<std::path::PathBuf, String> {
        let names = frame
            .get_column_names()
            .iter()
            .map(|name| name.to_string())
            .collect::<Vec<_>>();
        let columns = frame
            .columns()
            .iter()
            .map(|column| {
                column
                    .str()
                    .expect("texto")
                    .iter()
                    .map(|value| value.map(str::to_owned))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let mut writer = PendingImport::prepare(directory, file_name, keep)?.start(&names)?;
        writer.write_rows(columns)?;
        writer.finish(&|| false)
    }

    /// Rows and row groups of a Parquet file, read from its footer only.
    fn parquet_footer(path: &Path) -> (usize, usize) {
        use polars::prelude::SerReader;
        let mut reader =
            polars::prelude::ParquetReader::new(fs::File::open(path).expect("parquet"));
        let metadata = reader.get_metadata().expect("metadatos");
        (metadata.num_rows, metadata.row_groups.len())
    }

    /// Rows of every row group of a Parquet file, read from its footer only.
    fn parquet_group_rows(path: &Path) -> Vec<usize> {
        use polars::prelude::SerReader;
        let mut reader =
            polars::prelude::ParquetReader::new(fs::File::open(path).expect("parquet"));
        let metadata = reader.get_metadata().expect("metadatos");
        metadata
            .row_groups
            .iter()
            .map(|group| group.num_rows())
            .collect()
    }

    /// `rows` rows by `columns` short text columns, with a null every 7th row.
    fn wide_text_batch(first: usize, rows: usize, columns: usize) -> Vec<Vec<Option<String>>> {
        (0..columns)
            .map(|column| {
                (first..first + rows)
                    .map(|row| (row % 7 != column % 7).then(|| format!("v{row}-{column}")))
                    .collect()
            })
            .collect()
    }

    fn text_batch(first: usize, rows: usize) -> Vec<Vec<Option<String>>> {
        let ids = (first..first + rows)
            .map(|row| Some(format!("id-{row}")))
            .collect();
        let names = (first..first + rows)
            .map(|row| (row % 3 != 0).then(|| format!("Íñigo {row}")))
            .collect();
        vec![ids, names]
    }

    /// AUD-012: a server that reports thousands of columns is refused before
    /// any buffer is reserved for them.
    #[test]
    fn an_import_with_too_many_columns_is_refused_with_a_clear_message() {
        let message = check_import_column_count(100_000).expect_err("100.000 columnas");
        assert!(message.contains("100000 columnas"), "{message}");
        assert!(message.contains("como mucho 2000"), "{message}");
        assert!(check_import_column_count(MAX_IMPORT_COLUMNS + 1).is_err());
        assert_eq!(check_import_column_count(MAX_IMPORT_COLUMNS), Ok(2_000));
        assert_eq!(check_import_column_count(1), Ok(1));
        let empty = check_import_column_count(0).expect_err("sin columnas");
        assert!(empty.contains("no tiene columnas"), "{empty}");
    }

    /// AUD-012: a catalog that never ends stops at the limit, reading no more
    /// rows than the limit allows.
    #[test]
    fn a_catalog_without_end_stops_with_an_error() {
        let mut read = 0usize;
        let rows = std::iter::repeat_with(|| {
            read += 1;
            Ok::<_, String>(read)
        });
        let result = read_catalog_tables(rows, 1_000, |number| {
            Some(listed("dbo", &format!("t{number}")))
        });
        assert!(matches!(result, Err(CatalogReadError::TooManyRows)));
        assert_eq!(read, 1_001, "se deja de leer al pasar el tope");
        assert!(catalog_too_large_message().contains("100000 tablas y vistas"));
    }

    /// AUD-024: the catalog is read in UTF-16, so accents, `ñ` and characters
    /// outside the Windows code page arrive intact; NULL stays NULL.
    #[test]
    fn catalog_cells_decode_from_utf16_keeping_nulls() {
        let wide = |text: &str| text.encode_utf16().collect::<Vec<u16>>();
        for text in ["Año", "Ventas_ñ", "Zona_Ω", "dbo", ""] {
            let units = wide(text);
            assert_eq!(decode_catalog_cell(Some(&units)), Ok(Some(text.to_owned())));
        }
        assert_eq!(decode_catalog_cell(None), Ok(None));
    }

    #[test]
    fn a_catalog_cell_that_is_not_utf16_is_an_error_not_an_empty_schema() {
        let lone_surrogate = [0x0064, 0xD800, 0x0062];
        assert_eq!(decode_catalog_cell(Some(&lone_surrogate)), Err(()));
        let message = unreadable_catalog_message(2);
        assert!(message.contains("2 tablas o vistas"), "{message}");
    }

    #[test]
    fn listed_names_with_wide_characters_reach_the_select_intact() {
        let table = source_table(DatabaseKind::SqlServer, "", "Ventas_ñ", "Zona_Ω", "TABLE")
            .expect("tabla");
        assert_eq!(
            (table.schema.as_str(), table.name.as_str()),
            ("Ventas_ñ", "Zona_Ω")
        );
        assert!(find_listed_table(std::slice::from_ref(&table), "Ventas_ñ", "Zona_Ω").is_some());
        let sql = select_rows_sql(&table, 5, DatabaseKind::SqlServer);
        assert!(sql.contains("Ventas_ñ") && sql.contains("Zona_Ω"), "{sql}");
    }

    #[test]
    fn a_catalog_within_the_limit_keeps_only_the_accepted_rows() {
        let rows = (0..10).map(Ok::<_, String>);
        let tables = read_catalog_tables(rows, 10, |number| {
            (number % 2 == 0).then(|| listed("dbo", &format!("t{number}")))
        })
        .unwrap_or_else(|_| panic!("el catálogo cabe en el tope"));
        assert_eq!(tables.len(), 5);
        let failing = vec![Ok(1), Err("se cayó".to_owned()), Ok(2)].into_iter();
        let result = read_catalog_tables(failing, 10, |_| None);
        assert!(matches!(result, Err(CatalogReadError::Driver(message)) if message == "se cayó"));
    }

    #[test]
    fn a_batch_becomes_an_all_text_frame_that_keeps_its_nulls() {
        let names = vec!["id".to_owned(), "nombre".to_owned()];
        let frame = import_batch_frame(&names, text_batch(0, 4)).expect("frame");
        assert_eq!((frame.height(), frame.width()), (4, 2));
        assert!(frame
            .dtypes()
            .iter()
            .all(|dtype| *dtype == DataType::String));
        let names_column = frame.column("nombre").expect("columna");
        assert_eq!(names_column.null_count(), 2, "las filas 0 y 3 son nulas");
        assert_eq!(
            frame
                .column("id")
                .expect("columna")
                .str()
                .expect("texto")
                .get(2),
            Some("id-2")
        );
    }

    /// AUD-012: every batch goes to the file as it arrives; the file holds all
    /// the rows with their values and nulls, gathered into one row group.
    #[test]
    fn batches_are_streamed_into_one_parquet_with_values_and_nulls() {
        use polars::prelude::SerReader;
        let directory = tempfile::tempdir().expect("carpeta");
        let imports = directory.path().join(DATABASE_IMPORT_DIRECTORY);
        let names = vec!["id".to_owned(), "nombre".to_owned()];
        let mut writer = PendingImport::prepare(&imports, "dbo.Lotes.parquet", None)
            .expect("destino")
            .start(&names)
            .expect("escritor");
        writer.write_rows(text_batch(0, 4)).expect("lote 1");
        writer
            .write_rows(vec![Vec::new(), Vec::new()])
            .expect("un lote vacío no escribe nada");
        writer.write_rows(text_batch(4, 1)).expect("lote 2");
        writer.write_rows(text_batch(5, 6)).expect("lote 3");
        let path = writer.finish(&|| false).expect("se guarda");
        assert_eq!(
            parquet_footer(&path),
            (11, 1),
            "11 filas de 3 lotes caben en un solo grupo"
        );

        let frame = polars::prelude::ParquetReader::new(fs::File::open(&path).expect("parquet"))
            .finish()
            .expect("se lee");
        assert_eq!(frame.get_column_names(), vec!["id", "nombre"]);
        let ids = frame.column("id").expect("id");
        let ids = ids.str().expect("texto");
        let people = frame.column("nombre").expect("nombre");
        let people = people.str().expect("texto");
        for row in 0..11 {
            assert_eq!(ids.get(row).map(str::to_owned), Some(format!("id-{row}")));
            let expected = (row % 3 != 0).then(|| format!("Íñigo {row}"));
            assert_eq!(people.get(row).map(str::to_owned), expected, "fila {row}");
        }
    }

    /// AUD-012: many batches stream through without the table being held.
    #[test]
    fn many_batches_land_in_the_file_one_after_another() {
        let directory = tempfile::tempdir().expect("carpeta");
        let imports = directory.path().join(DATABASE_IMPORT_DIRECTORY);
        let names = vec!["id".to_owned(), "nombre".to_owned()];
        let mut writer = PendingImport::prepare(&imports, "dbo.Muchos.parquet", None)
            .expect("destino")
            .start(&names)
            .expect("escritor");
        for batch in 0..20 {
            writer
                .write_rows(text_batch(batch * 10_000, 10_000))
                .expect("lote");
        }
        let path = writer.finish(&|| false).expect("se guarda");
        let groups = parquet_group_rows(&path);
        assert_eq!(groups.iter().sum::<usize>(), 200_000);
        assert!(groups.iter().all(|rows| *rows <= 65_536), "{groups:?}");
        assert!(groups.len() < 20, "menos grupos que lotes: {groups:?}");
    }

    /// AUD-037: 100.000 rows by 60 text columns in batches of 10.000 are
    /// regrouped into row groups of at most 65.536 rows, with every value and
    /// null intact.
    #[test]
    fn wide_import_batches_are_regrouped_into_row_groups_of_the_project_size() {
        use polars::prelude::SerReader;
        let directory = tempfile::tempdir().expect("carpeta");
        let imports = directory.path().join(DATABASE_IMPORT_DIRECTORY);
        let names = (0..60)
            .map(|column| format!("c{column}"))
            .collect::<Vec<_>>();
        let mut writer = PendingImport::prepare(&imports, "dbo.Ancha.parquet", None)
            .expect("destino")
            .start(&names)
            .expect("escritor");
        for batch in 0..10 {
            writer
                .write_rows(wide_text_batch(batch * 10_000, 10_000, 60))
                .expect("lote");
        }
        let path = writer.finish(&|| false).expect("se guarda");
        let groups = parquet_group_rows(&path);
        assert_eq!(groups.iter().sum::<usize>(), 100_000);
        assert!(groups.iter().all(|rows| *rows <= 65_536), "{groups:?}");
        assert!(groups.len() < 10, "menos grupos que lotes: {groups:?}");

        let frame = polars::prelude::ParquetReader::new(fs::File::open(&path).expect("parquet"))
            .finish()
            .expect("se lee");
        assert_eq!((frame.height(), frame.width()), (100_000, 60));
        for (row, column) in [(0, 0), (7, 3), (65_536, 17), (99_999, 59)] {
            let values = frame.column(&format!("c{column}")).expect("columna");
            let values = values.str().expect("texto");
            let value = (row % 7 != column % 7).then(|| format!("v{row}-{column}"));
            assert_eq!(values.get(row).map(str::to_owned), value, "fila {row}");
        }
        let nulls: usize = frame
            .columns()
            .iter()
            .map(|column| column.null_count())
            .sum();
        let expected_nulls: usize = (0..60)
            .map(|column| (0..100_000).filter(|row| row % 7 == column % 7).count())
            .sum();
        assert_eq!(nulls, expected_nulls);
    }

    /// AUD-037: rows wide enough to pass the byte target get fewer rows per
    /// group, and no group passes `HISTORY_SNAPSHOT_BATCH_ROWS`.
    #[test]
    fn very_wide_rows_get_smaller_row_groups() {
        let directory = tempfile::tempdir().expect("carpeta");
        let imports = directory.path().join(DATABASE_IMPORT_DIRECTORY);
        let names = vec!["texto".to_owned()];
        let mut writer = PendingImport::prepare(&imports, "dbo.Texto.parquet", None)
            .expect("destino")
            .start(&names)
            .expect("escritor");
        for batch in 0..4 {
            let long = (0..2_000)
                .map(|row| Some(format!("{batch}-{row}-{}", "x".repeat(20_000))))
                .collect();
            writer.write_rows(vec![long]).expect("lote");
        }
        let path = writer.finish(&|| false).expect("se guarda");
        let groups = parquet_group_rows(&path);
        assert_eq!(groups.iter().sum::<usize>(), 8_000);
        assert!(groups.iter().all(|rows| *rows <= 1_700), "{groups:?}");
        assert!(groups.len() >= 5, "{groups:?}");
    }

    /// AUD-037: the T-SQL script of the regrouped import is generated through
    /// DuckDB without running out of memory.
    #[test]
    fn tsql_script_of_a_wide_import_runs_within_the_memory_limit() {
        let directory = tempfile::tempdir().expect("carpeta");
        let imports = directory.path().join(DATABASE_IMPORT_DIRECTORY);
        let names = (0..60)
            .map(|column| format!("c{column}"))
            .collect::<Vec<_>>();
        let mut writer = PendingImport::prepare(&imports, "dbo.Ancha.parquet", None)
            .expect("destino")
            .start(&names)
            .expect("escritor");
        for batch in 0..10 {
            writer
                .write_rows(wide_text_batch(batch * 10_000, 10_000, 60))
                .expect("lote");
        }
        let path = writer.finish(&|| false).expect("se guarda");
        let script = directory.path().join("ancha.sql");
        let target = crate::tsql_script::TsqlTarget::resolve(None, "ancha").expect("destino");
        crate::duckdb_query::export_file_to_tsql_with_cancel(
            &path,
            crate::duckdb_query::DuckDbFileFormat::Parquet,
            &script,
            target,
            || false,
        )
        .expect("el script T-SQL debe generarse");
        assert!(fs::metadata(&script).expect("script").len() > 1_000_000);
    }

    /// A table without rows still gives a valid Parquet with its columns.
    #[test]
    fn a_table_without_rows_gives_a_valid_parquet_with_its_columns() {
        use polars::prelude::SerReader;
        let directory = tempfile::tempdir().expect("carpeta");
        let imports = directory.path().join(DATABASE_IMPORT_DIRECTORY);
        let names = vec!["id".to_owned(), "nombre".to_owned()];
        let writer = PendingImport::prepare(&imports, "dbo.Vacia.parquet", None)
            .expect("destino")
            .start(&names)
            .expect("escritor");
        let path = writer.finish(&|| false).expect("se guarda");
        assert_eq!(parquet_footer(&path).0, 0);
        let frame = polars::prelude::ParquetReader::new(fs::File::open(&path).expect("parquet"))
            .finish()
            .expect("se lee");
        assert_eq!(frame.height(), 0);
        assert_eq!(frame.get_column_names(), vec!["id", "nombre"]);
        assert!(frame
            .dtypes()
            .iter()
            .all(|dtype| *dtype == DataType::String));
    }

    /// If the read fails or is cancelled midway, the temporary file is
    /// discarded, no new Parquet appears and earlier imports stay.
    #[test]
    fn an_import_abandoned_midway_leaves_no_new_file_and_keeps_earlier_ones() {
        use polars::prelude::df;
        let directory = tempfile::tempdir().expect("carpeta");
        let imports = directory.path().join(DATABASE_IMPORT_DIRECTORY);
        let mut frame = df!("nombre" => ["Ana"]).expect("frame");
        let earlier =
            write_import_parquet(&imports, "dbo.X.parquet", None, &mut frame).expect("anterior");
        let names = vec!["id".to_owned(), "nombre".to_owned()];
        let mut writer = PendingImport::prepare(&imports, "dbo.Nueva.parquet", None)
            .expect("destino")
            .start(&names)
            .expect("escritor");
        writer.write_rows(text_batch(0, 100)).expect("primer lote");
        drop(writer);
        assert!(earlier.exists(), "la importación anterior sigue ahí");
        let names: Vec<String> = fs::read_dir(&imports)
            .expect("carpeta")
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            names,
            vec!["dbo.X.parquet"],
            "sin temporales ni Parquet nuevo"
        );
    }

    /// A driver that delivers `batches` (rows, columns cut) one after another,
    /// keeping at most the rows wanted, as the ODBC reader does.
    fn scripted_batches(
        batches: Vec<(usize, Vec<bool>)>,
        asked: &std::cell::Cell<usize>,
    ) -> impl FnMut(usize) -> Result<Option<ImportBatch>, String> + '_ {
        let mut first = 0usize;
        let mut batches = batches.into_iter();
        move |wanted| {
            asked.set(asked.get() + 1);
            let Some((rows, truncated)) = batches.next() else {
                return Ok(None);
            };
            let columns = text_batch(first, rows.min(wanted));
            first += rows;
            Ok(Some(ImportBatch {
                columns,
                available: rows,
                truncated,
                replaced: vec![false; 2],
            }))
        }
    }

    fn import_writer(directory: &Path) -> (Vec<String>, ImportParquetWriter) {
        let names = vec!["id".to_owned(), "nombre".to_owned()];
        let imports = directory.join(DATABASE_IMPORT_DIRECTORY);
        let writer = PendingImport::prepare(&imports, "dbo.T.parquet", None)
            .expect("destino")
            .start(&names)
            .expect("escritor");
        (names, writer)
    }

    #[test]
    fn startup_purge_empties_imports_including_killed_temporaries() {
        let directory = tempfile::tempdir().expect("carpeta");
        // A write in progress: its temporary must carry the `.partial` suffix.
        let (_names, writer) = import_writer(directory.path());
        let imports = directory.path().join(DATABASE_IMPORT_DIRECTORY);
        let temporary_names: Vec<String> = fs::read_dir(&imports)
            .expect("carpeta")
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(temporary_names.len(), 1);
        assert!(temporary_names[0].ends_with(".parquet.partial"));
        // Simulate a killed process: the temporary is never cleaned up.
        std::mem::forget(writer);
        fs::write(imports.join("dbo.Old.parquet"), b"datos").expect("importación previa");
        fs::write(imports.join(".tmpAbC123"), b"resto").expect("temporal viejo");
        fs::create_dir(imports.join("carpeta")).expect("subcarpeta");

        let removed = purge_database_imports(directory.path());

        assert_eq!(removed, 3);
        let left: Vec<_> = fs::read_dir(&imports).expect("carpeta").flatten().collect();
        assert_eq!(left.len(), 1, "solo la subcarpeta ajena queda");
        assert_eq!(purge_database_imports(&directory.path().join("nada")), 0);
    }

    /// AUD-103: Cancelar returns while the import thread is still blocked in the
    /// driver, and when that thread comes back it writes no Parquet and deletes
    /// no earlier one (its temporary is removed).
    #[test]
    fn a_cancelled_import_returns_at_once_and_the_late_thread_touches_nothing() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::{mpsc, Arc};

        let directory = tempfile::tempdir().expect("carpeta");
        let imports = directory.path().join(DATABASE_IMPORT_DIRECTORY);
        let (_names, writer) = import_writer(directory.path());
        let previous = imports.join("dbo.Old.parquet");
        fs::write(&previous, b"datos anteriores").expect("importación previa");

        let cancelled = Arc::new(AtomicBool::new(false));
        let (release, blocked) = mpsc::channel::<()>();
        let (done, finished) = mpsc::channel::<Result<std::path::PathBuf, String>>();
        let work_flag = Arc::clone(&cancelled);
        let watcher_flag = Arc::clone(&cancelled);
        let flag = Arc::clone(&cancelled);
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(100));
            flag.store(true, Ordering::SeqCst);
        });

        let started = std::time::Instant::now();
        let result = tauri::async_runtime::block_on(run_blocking_unless_cancelled(
            move || {
                // The server never answers until the test lets it.
                let _ = blocked.recv();
                let outcome = writer.finish(&|| work_flag.load(Ordering::SeqCst));
                let _ = done.send(outcome.clone());
                outcome
            },
            move || watcher_flag.load(Ordering::SeqCst),
            "interrumpido",
        ));
        assert_eq!(
            result,
            Err(crate::dataset::OPERATION_CANCELLED_MESSAGE.to_owned())
        );
        assert!(started.elapsed().as_secs() < 2, "{:?}", started.elapsed());

        // The blocked thread comes back after the cancellation.
        release.send(()).expect("se libera el trabajo");
        let late = finished
            .recv_timeout(std::time::Duration::from_secs(10))
            .expect("el hilo tardío termina");
        assert_eq!(
            late,
            Err(crate::dataset::OPERATION_CANCELLED_MESSAGE.to_owned())
        );
        assert_eq!(fs::read(&previous).expect("sigue"), b"datos anteriores");
        let names: Vec<String> = fs::read_dir(&imports)
            .expect("carpeta")
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["dbo.Old.parquet".to_owned()], "{names:?}");
    }

    struct ScriptedImport {
        rows: usize,
        more_rows_available: bool,
        truncated_columns: Vec<String>,
        /// Rows that reached the Parquet file.
        written: usize,
        percents: Vec<u8>,
    }

    /// Runs the import loop over scripted batches and finishes the file.
    fn run_scripted_import(row_limit: usize, batches: Vec<(usize, Vec<bool>)>) -> ScriptedImport {
        let directory = tempfile::tempdir().expect("carpeta");
        let (names, writer) = import_writer(directory.path());
        let asked = std::cell::Cell::new(0);
        let mut percents = Vec::new();
        let read = stream_batches_to_parquet(
            names,
            row_limit,
            writer,
            scripted_batches(batches, &asked),
            &mut |_, percent| percents.push(percent),
            &|| false,
        )
        .expect("la lectura termina");
        let path = read.writer.finish(&|| false).expect("se guarda");
        ScriptedImport {
            rows: read.rows,
            more_rows_available: read.more_rows_available,
            truncated_columns: read.truncated_columns,
            written: parquet_footer(&path).0,
            percents,
        }
    }

    /// AUD-074: the row limit is exact, with or without the extra row that
    /// tells there is more.
    #[test]
    fn the_import_stops_at_the_row_limit_and_flags_the_extra_row() {
        let none = vec![false, false];
        let outcome =
            |import: &ScriptedImport| (import.rows, import.more_rows_available, import.written);
        // 8 rows delivered, 5 wanted: the first batch alone is cut.
        let import = run_scripted_import(5, vec![(4, none.clone()), (4, none.clone())]);
        assert_eq!(outcome(&import), (5, true, 5));
        assert!(import
            .percents
            .iter()
            .all(|percent| (20..=80).contains(percent)));
        assert_eq!(import.percents.last(), Some(&80));

        // Exactly the limit, and one more row in a later batch.
        let import = run_scripted_import(5, vec![(5, none.clone()), (1, none.clone())]);
        assert_eq!(outcome(&import), (5, true, 5));

        // The table has exactly the limit: nothing more to say.
        let import = run_scripted_import(5, vec![(3, none.clone()), (2, none.clone())]);
        assert_eq!(outcome(&import), (5, false, 5));

        // Fewer rows than the limit.
        let import = run_scripted_import(50, vec![(3, none)]);
        assert_eq!(outcome(&import), (3, false, 3));
    }

    /// AUD-074: a column with a value cut in any batch is reported once; one
    /// never cut is not.
    #[test]
    fn the_import_reports_the_columns_whose_values_were_cut() {
        let read = run_scripted_import(
            100,
            vec![
                (2, vec![false, false]),
                (2, vec![false, true]),
                (2, vec![false, false]),
            ],
        );
        assert_eq!(read.truncated_columns, vec!["nombre".to_owned()]);
        let read = run_scripted_import(100, vec![(2, vec![false, false])]);
        assert!(read.truncated_columns.is_empty());
        let read = run_scripted_import(100, vec![(2, vec![true, true])]);
        assert_eq!(read.truncated_columns, vec!["id", "nombre"]);
    }

    /// AUD-104: a lone surrogate no longer panics; it becomes U+FFFD and is
    /// flagged, while valid text (pairs included) passes untouched.
    #[test]
    fn wide_cells_replace_lone_surrogates_and_flag_them() {
        let wide = |text: &str| text.encode_utf16().collect::<Vec<u16>>();
        assert_eq!(
            decode_wide_cell(&wide("normal")),
            ("normal".to_owned(), false)
        );
        assert_eq!(decode_wide_cell(&[]), (String::new(), false));
        assert_eq!(decode_wide_cell(&wide("a😀ñ")), ("a😀ñ".to_owned(), false));
        assert_eq!(
            decode_wide_cell(&[0xD800, u16::from(b'x')]),
            ("\u{FFFD}x".to_owned(), true)
        );
        assert_eq!(
            decode_wide_cell(&[u16::from(b'x'), 0xDC00]),
            ("x\u{FFFD}".to_owned(), true)
        );
        assert_eq!(decode_wide_cell(&[0xD800]), ("\u{FFFD}".to_owned(), true));
    }

    /// AUD-104: a column repaired in any batch is reported once.
    #[test]
    fn the_import_reports_the_columns_with_replaced_characters() {
        let directory = tempfile::tempdir().expect("carpeta");
        let (names, writer) = import_writer(directory.path());
        let mut sent = false;
        let read = stream_batches_to_parquet(
            names,
            100,
            writer,
            |_| {
                if sent {
                    return Ok(None);
                }
                sent = true;
                Ok(Some(ImportBatch {
                    columns: text_batch(0, 2),
                    available: 2,
                    truncated: vec![false, false],
                    replaced: vec![false, true],
                }))
            },
            &mut |_, _| {},
            &|| false,
        )
        .expect("la lectura termina");
        assert_eq!(read.replaced_columns, vec!["nombre".to_owned()]);
        assert!(read.truncated_columns.is_empty());
    }

    /// AUD-074: cancelling after the first batch stops the read, reports the
    /// cancellation and leaves no file, not even a temporary one.
    #[test]
    fn cancelling_between_batches_stops_the_read_and_leaves_no_file() {
        let directory = tempfile::tempdir().expect("carpeta");
        let (names, writer) = import_writer(directory.path());
        let asked = std::cell::Cell::new(0);
        let checks = std::cell::Cell::new(0);
        let none = vec![false, false];
        let result = stream_batches_to_parquet(
            names,
            1_000,
            writer,
            scripted_batches(
                vec![(10, none.clone()), (10, none.clone()), (10, none)],
                &asked,
            ),
            &mut |_, _| {},
            // Not cancelled before the first batch, cancelled before the second.
            &|| {
                checks.set(checks.get() + 1);
                checks.get() > 1
            },
        );
        assert_eq!(
            result.err().as_deref(),
            Some(crate::dataset::OPERATION_CANCELLED_MESSAGE)
        );
        assert_eq!(asked.get(), 1, "no se pide ningún lote más tras cancelar");
        let left = fs::read_dir(directory.path().join(DATABASE_IMPORT_DIRECTORY))
            .expect("carpeta")
            .count();
        assert_eq!(left, 0, "ni Parquet ni temporales");
    }

    /// AUD-074: a read that fails midway reports the error and leaves no file.
    #[test]
    fn a_driver_error_midway_is_returned_and_leaves_no_file() {
        let directory = tempfile::tempdir().expect("carpeta");
        let (names, writer) = import_writer(directory.path());
        let mut calls = 0;
        let result = stream_batches_to_parquet(
            names,
            1_000,
            writer,
            |_| {
                calls += 1;
                if calls == 1 {
                    Ok(Some(ImportBatch {
                        columns: text_batch(0, 3),
                        available: 3,
                        truncated: vec![false, false],
                        replaced: vec![false, false],
                    }))
                } else {
                    Err("se cayó la conexión".to_owned())
                }
            },
            &mut |_, _| {},
            &|| false,
        );
        assert_eq!(result.err().as_deref(), Some("se cayó la conexión"));
        let left = fs::read_dir(directory.path().join(DATABASE_IMPORT_DIRECTORY))
            .expect("carpeta")
            .count();
        assert_eq!(left, 0);
    }

    #[test]
    fn writing_the_import_keeps_the_active_file_and_replaces_earlier_imports() {
        use polars::prelude::df;
        let directory = tempfile::tempdir().expect("carpeta");
        let imports = directory.path().join(DATABASE_IMPORT_DIRECTORY);
        let mut frame = df!("nombre" => ["Ana", "Íñigo"]).expect("frame");
        let first = write_import_parquet(&imports, "dbo.Ventas.parquet", None, &mut frame)
            .expect("primera");
        let second = write_import_parquet(&imports, "dbo.Clientes.parquet", None, &mut frame)
            .expect("segunda");
        assert!(
            !first.exists(),
            "una importación anterior que nadie usa se borra"
        );
        assert!(second.exists());
        // The active dataset reads `second`: a new import of the same table sits beside it.
        let third =
            write_import_parquet(&imports, "dbo.Clientes.parquet", Some(&second), &mut frame)
                .expect("tercera");
        assert!(second.exists());
        assert_ne!(third, second);
        assert!(third
            .file_name()
            .unwrap()
            .to_string_lossy()
            .contains("(nueva)"));
    }

    #[test]
    fn writing_the_import_keeps_the_active_file_given_as_a_canonical_path() {
        use polars::prelude::df;
        let directory = tempfile::tempdir().expect("carpeta");
        let imports = directory.path().join(DATABASE_IMPORT_DIRECTORY);
        let mut frame = df!("nombre" => ["Ana"]).expect("frame");
        let second = write_import_parquet(&imports, "dbo.Clientes.parquet", None, &mut frame)
            .expect("segunda");
        // The active dataset reports the canonical path (`\\?\C:\...` on Windows).
        let canonical = std::fs::canonicalize(&second).expect("ruta canónica");
        let third = write_import_parquet(
            &imports,
            "dbo.Clientes.parquet",
            Some(&canonical),
            &mut frame,
        )
        .expect("tercera");
        assert!(second.exists(), "el archivo del dataset activo se conserva");
        assert_ne!(third, second);
        assert!(third
            .file_name()
            .unwrap()
            .to_string_lossy()
            .contains("(nueva)"));
        // Another table imported while the canonical file is active keeps it too.
        let fourth =
            write_import_parquet(&imports, "dbo.Otra.parquet", Some(&canonical), &mut frame)
                .expect("cuarta");
        assert!(second.exists());
        assert!(fourth.exists());
        assert!(!third.exists(), "la importación sin uso anterior se borra");
    }

    #[test]
    fn a_failed_write_does_not_delete_earlier_imports() {
        use polars::prelude::df;
        let directory = tempfile::tempdir().expect("carpeta");
        let imports = directory.path().join(DATABASE_IMPORT_DIRECTORY);
        let mut frame = df!("nombre" => ["Ana"]).expect("frame");
        let earlier =
            write_import_parquet(&imports, "dbo.X.parquet", None, &mut frame).expect("anterior");
        // A folder with the destination's name makes `persist` fail.
        let blocker = imports.join("dbo.Y.parquet");
        std::fs::create_dir(&blocker).expect("bloqueo");
        let result = write_import_parquet(&imports, "dbo.Y.parquet", None, &mut frame);
        assert!(result.is_err());
        assert!(earlier.exists(), "la importación anterior sigue ahí");
        let names: Vec<String> = std::fs::read_dir(&imports)
            .expect("carpeta")
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names.len(), 2, "sin temporales sueltos: {names:?}");
    }

    #[test]
    fn the_new_import_never_deletes_itself() {
        use polars::prelude::df;
        let directory = tempfile::tempdir().expect("carpeta");
        let imports = directory.path().join(DATABASE_IMPORT_DIRECTORY);
        let mut frame = df!("nombre" => ["Ana"]).expect("frame");
        write_import_parquet(&imports, "dbo.Z.parquet", None, &mut frame).expect("primera");
        let again =
            write_import_parquet(&imports, "dbo.Z.parquet", None, &mut frame).expect("misma tabla");
        assert!(again.exists());
        assert_eq!(std::fs::read_dir(&imports).expect("carpeta").count(), 1);
    }

    /// Creates a table of its own in the test database of one engine, imports
    /// it as text and compares what arrives; the table is dropped at the end,
    /// also when the comparison fails.
    fn run_external_import(target: &DatabaseTarget, suffix: &str) -> Result<(), String> {
        use polars::prelude::SerReader;
        let mut target = target.clone();
        target.table = format!("columnia_aud074_{suffix}");
        let table = qualified_table(&target);
        let kind = target.kind;
        let (text_type, quote) = match kind {
            DatabaseKind::SqlServer => ("NVARCHAR(100)", "N"),
            _ => ("VARCHAR(100)", ""),
        };
        let drop = drop_table_sql(&table, kind);
        execute_external_sql(&target, &drop)?;
        execute_external_sql(
            &target,
            &format!(
                "CREATE TABLE {table} ({} INTEGER, {} {text_type})",
                quote_identifier("id", kind),
                quote_identifier("nombre", kind)
            ),
        )?;
        let expected: [(&str, Option<&str>); 5] = [
            ("1", Some("fila 1")),
            ("2", None),
            ("3", Some("Íñigo; O'Brien")),
            ("4", Some("fila 4")),
            ("5", Some("fila 5")),
        ];
        let outcome = (|| {
            let values = expected
                .iter()
                .map(|(id, text)| match text {
                    Some(text) => format!("({id}, {quote}'{}')", text.replace('\'', "''")),
                    None => format!("({id}, NULL)"),
                })
                .collect::<Vec<_>>()
                .join(", ");
            execute_external_sql(&target, &format!("INSERT INTO {table} VALUES {values}"))?;
            let source = DatabaseSource {
                kind,
                connection: target.connection.clone(),
            };
            let directory = tempdir().map_err(|error| format!("temp externo: {error}"))?;
            // Limit 3 of 5 rows: three rows and the flag that there are more.
            let fetched = import_table_with_cancel(
                &source,
                &target.schema,
                &target.table,
                3,
                directory.path(),
                None,
                |_, _| {},
                || false,
            )?;
            assert_eq!((fetched.rows, fetched.more_rows_available), (3, true));
            assert!(fetched.truncated_columns.is_empty());
            let frame = polars::prelude::ParquetReader::new(
                fs::File::open(&fetched.path).map_err(|error| format!("Parquet: {error}"))?,
            )
            .finish()
            .map_err(|error| format!("Parquet: {error}"))?;
            assert_eq!(frame.height(), 3);
            assert!(frame
                .dtypes()
                .iter()
                .all(|dtype| *dtype == DataType::String));
            // Without ORDER BY the engine may deliver any three rows: each
            // one is compared with its own id.
            let ids = frame.column("id").expect("id");
            let ids = ids.str().expect("texto");
            let names = frame.column("nombre").expect("nombre");
            let names = names.str().expect("texto");
            for row in 0..3 {
                let id = ids.get(row).expect("id sin nulos");
                let (_, text) = expected
                    .iter()
                    .find(|(expected_id, _)| *expected_id == id)
                    .unwrap_or_else(|| panic!("id inesperado {id}"));
                assert_eq!(names.get(row), *text, "fila {id}");
            }
            // The whole table fits: no extra row.
            let all = import_table_with_cancel(
                &source,
                &target.schema,
                &target.table,
                5,
                directory.path(),
                None,
                |_, _| {},
                || false,
            )?;
            assert_eq!((all.rows, all.more_rows_available), (5, false));
            Ok::<(), String>(())
        })();
        let cleanup = execute_external_sql(&target, &drop);
        outcome?;
        cleanup
    }

    #[test]
    #[ignore = "requiere servidor y controlador ODBC PostgreSQL reales configurados por variables de sesión"]
    fn external_odbc_import_postgresql() {
        let target = external_target(DatabaseKind::Postgresql, "COLUMNIA_ODBC_POSTGRESQL");
        run_external_import(&target, &unique_suffix("pg"))
            .expect("importación PostgreSQL completa");
    }

    #[test]
    #[ignore = "requiere servidor y controlador ODBC MySQL reales configurados por variables de sesión"]
    fn external_odbc_import_mysql() {
        let target = external_target(DatabaseKind::Mysql, "COLUMNIA_ODBC_MYSQL");
        run_external_import(&target, &unique_suffix("mysql")).expect("importación MySQL completa");
    }

    /// Reads a real table without writing anything. Set COLUMNIA_ODBC_IMPORT
    /// to a connection string and COLUMNIA_ODBC_IMPORT_TABLE to «schema.table».
    /// COLUMNIA_ODBC_IMPORT_ROWS sets the row limit (25 by default). With a
    /// limit above 25 the test only reads the Parquet footer and prints no
    /// frame, so the process memory shows what the import itself used.
    #[test]
    #[ignore = "requiere un SQL Server real configurado por variables de sesión"]
    fn external_odbc_import_reads_a_table_as_text() {
        let source =
            sql_server_source(&env::var("COLUMNIA_ODBC_IMPORT").expect("COLUMNIA_ODBC_IMPORT"));
        let qualified = env::var("COLUMNIA_ODBC_IMPORT_TABLE").expect("COLUMNIA_ODBC_IMPORT_TABLE");
        let row_limit = env::var("COLUMNIA_ODBC_IMPORT_ROWS")
            .map(|value| {
                value
                    .trim()
                    .parse::<usize>()
                    .expect("COLUMNIA_ODBC_IMPORT_ROWS")
            })
            .unwrap_or(25);
        let (schema, table) = qualified.split_once('.').expect("esquema.tabla");
        let directory = tempfile::tempdir().expect("carpeta");
        let fetched = import_table_with_cancel(
            &source,
            schema,
            table,
            row_limit,
            directory.path(),
            None,
            |_, _| {},
            || false,
        )
        .expect("la tabla se importa");
        assert!(fetched.path.exists());
        assert!(fetched.rows > 0 && fetched.rows <= row_limit);
        if row_limit > 25 {
            let (rows, row_groups) = parquet_footer(&fetched.path);
            assert_eq!(rows, fetched.rows);
            eprintln!(
                "importadas {} filas en {} grupos de filas; más filas: {}; columnas recortadas: {:?}",
                fetched.rows, row_groups, fetched.more_rows_available, fetched.truncated_columns
            );
            return;
        }
        use polars::prelude::SerReader;
        let frame = polars::prelude::ParquetReader::new(
            std::fs::File::open(&fetched.path).expect("parquet"),
        )
        .finish()
        .expect("se lee el Parquet");
        assert_eq!(frame.height(), fetched.rows);
        assert!(frame
            .dtypes()
            .iter()
            .all(|dtype| *dtype == DataType::String));
        eprintln!(
            "importadas {} filas y {} columnas; más filas: {}; columnas recortadas: {:?}; columnas: {:?}",
            fetched.rows,
            frame.width(),
            fetched.more_rows_available,
            fetched.truncated_columns,
            frame.get_column_names()
        );
        eprintln!("{frame}");
    }

    /// AUD-104: imports a real table with a lone UTF-16 surrogate in a text
    /// cell: no panic, and the column is reported as repaired. Launch (from
    /// src-tauri/), with COLUMNIA_ODBC_IMPORT as the connection string and
    /// COLUMNIA_ODBC_IMPORT_SURROGATE_TABLE as «schema.table»; the test skips
    /// itself when the latter is not set:
    ///   cargo test --lib external_odbc_import_survives_lone_surrogates -- --ignored --nocapture
    #[test]
    #[ignore = "requiere un SQL Server real configurado por variables de sesión"]
    fn external_odbc_import_survives_lone_surrogates() {
        let Ok(qualified) = env::var("COLUMNIA_ODBC_IMPORT_SURROGATE_TABLE") else {
            eprintln!("COLUMNIA_ODBC_IMPORT_SURROGATE_TABLE no definida: se omite");
            return;
        };
        let source =
            sql_server_source(&env::var("COLUMNIA_ODBC_IMPORT").expect("COLUMNIA_ODBC_IMPORT"));
        let (schema, table) = qualified.split_once('.').expect("esquema.tabla");
        let directory = tempfile::tempdir().expect("carpeta");
        let fetched = import_table_with_cancel(
            &source,
            schema,
            table,
            25,
            directory.path(),
            None,
            |_, _| {},
            || false,
        )
        .expect("la tabla se importa sin pánico");
        assert!(fetched.rows > 0);
        assert!(
            !fetched.replaced_columns.is_empty(),
            "la columna con el sustituto debe señalarse"
        );
        eprintln!(
            "importadas {} filas; columnas con caracteres sustituidos: {:?}",
            fetched.rows, fetched.replaced_columns
        );
    }

    /// AUD-024: lists the tables of a real server and checks that the names
    /// with accents, `ñ` or `Ω` appear exactly, then imports one of them.
    /// Launch (PowerShell, from src-tauri/):
    ///   $env:COLUMNIA_TEST_HARNESS_MANIFEST = "1"
    ///   $env:COLUMNIA_ODBC_IMPORT = "<connection string>"
    ///   $env:COLUMNIA_ODBC_EXPECT_TABLES = "dbo.Año;Ventas_ñ.Pedidos;dbo.Zona_Ω"
    ///   cargo test --lib external_odbc_catalog_lists_wide_names -- --ignored --nocapture
    /// COLUMNIA_ODBC_EXPECT_ROWS (optional) is the row count the first
    /// expected table must have (the import limit is 1000 rows).
    #[test]
    #[ignore = "requiere un SQL Server real configurado por variables de sesión"]
    fn external_odbc_catalog_lists_wide_names() {
        let source =
            sql_server_source(&env::var("COLUMNIA_ODBC_IMPORT").expect("COLUMNIA_ODBC_IMPORT"));
        let expected =
            env::var("COLUMNIA_ODBC_EXPECT_TABLES").expect("COLUMNIA_ODBC_EXPECT_TABLES");
        let target = source_as_target(&source);
        let environment = Environment::new().expect("entorno ODBC");
        let connection =
            connect(&environment, &target, ConnectionPurpose::Import).expect("conexión");
        let tables = list_source_tables(&connection, &target).expect("lista de tablas");
        let mut first = None;
        for qualified in expected.split(';').filter(|item| !item.trim().is_empty()) {
            let (schema, name) = qualified.trim().split_once('.').expect("esquema.tabla");
            assert!(
                find_listed_table(&tables, schema, name).is_some(),
                "no aparece {qualified}; listadas: {:?}",
                tables
                    .iter()
                    .map(|table| format!("{}.{}", table.schema, table.name))
                    .collect::<Vec<_>>()
            );
            first.get_or_insert((schema.to_owned(), name.to_owned()));
        }
        let (schema, name) = first.expect("al menos una tabla esperada");
        let directory = tempfile::tempdir().expect("carpeta");
        let fetched = import_table_with_cancel(
            &source,
            &schema,
            &name,
            1_000,
            directory.path(),
            None,
            |_, _| {},
            || false,
        )
        .expect("la tabla se importa");
        if let Ok(rows) = env::var("COLUMNIA_ODBC_EXPECT_ROWS") {
            assert_eq!(
                fetched.rows,
                rows.trim()
                    .parse::<usize>()
                    .expect("COLUMNIA_ODBC_EXPECT_ROWS")
            );
        }
        eprintln!("{schema}.{name}: {} filas importadas", fetched.rows);
    }

    #[test]
    fn postgresql_and_mysql_drivers_prefer_unicode_and_the_newest() {
        let installed = vec![
            "ODBC Driver 18 for SQL Server".to_owned(),
            "PostgreSQL ANSI(x64)".to_owned(),
            "PostgreSQL Unicode(x64)".to_owned(),
            "MariaDB ODBC 3.1 Driver".to_owned(),
            "MySQL ODBC 8.0 ANSI Driver".to_owned(),
            "MySQL ODBC 8.0 Unicode Driver".to_owned(),
            "MySQL ODBC 9.1 Unicode Driver".to_owned(),
        ];
        assert_eq!(
            database_drivers(DatabaseKind::Postgresql, installed.clone()),
            vec!["PostgreSQL Unicode(x64)", "PostgreSQL ANSI(x64)"]
        );
        assert_eq!(
            database_drivers(DatabaseKind::Mysql, installed.clone()),
            vec![
                "MySQL ODBC 9.1 Unicode Driver",
                "MySQL ODBC 8.0 Unicode Driver",
                "MySQL ODBC 8.0 ANSI Driver",
                "MariaDB ODBC 3.1 Driver",
            ]
        );
        assert_eq!(
            database_drivers(DatabaseKind::SqlServer, installed),
            vec!["ODBC Driver 18 for SQL Server"]
        );
        assert_eq!(driver_version("mysql odbc 10.12 unicode driver"), 10012);
        assert_eq!(driver_version("postgresql unicode(x64)"), 0);
    }

    #[test]
    fn sql_server_drivers_come_newest_first_without_other_databases() {
        let installed = vec![
            "SQL Server".to_owned(),
            "Microsoft Access Driver (*.mdb, *.accdb)".to_owned(),
            "ODBC Driver 17 for SQL Server".to_owned(),
            "PostgreSQL Unicode(x64)".to_owned(),
            "ODBC Driver 18 for SQL Server".to_owned(),
            "SQL Server Native Client 11.0".to_owned(),
        ];
        assert_eq!(
            sql_server_drivers(installed),
            vec![
                "ODBC Driver 18 for SQL Server",
                "ODBC Driver 17 for SQL Server",
                "SQL Server Native Client 11.0",
            ]
        );
    }

    /// AUD-101: the legacy «SQL Server» driver is not offered (it cannot
    /// encrypt), while the Native Client and the ODBC Drivers stay.
    #[test]
    fn the_legacy_sql_server_driver_is_not_offered() {
        let installed = vec![
            "SQL Server".to_owned(),
            "ODBC Driver 17 for SQL Server".to_owned(),
            "SQL Server Native Client 11.0".to_owned(),
            "PostgreSQL Unicode".to_owned(),
        ];
        assert_eq!(
            database_drivers(DatabaseKind::SqlServer, installed.clone()),
            vec![
                "ODBC Driver 17 for SQL Server",
                "SQL Server Native Client 11.0"
            ]
        );
        assert_eq!(
            database_drivers(DatabaseKind::Postgresql, installed),
            vec!["PostgreSQL Unicode"]
        );
        assert!(
            database_drivers(DatabaseKind::SqlServer, vec![" sql server ".to_owned()]).is_empty()
        );
        assert!(is_legacy_sql_server_driver(" sql SERVER "));
        assert!(!is_legacy_sql_server_driver(
            "SQL Server Native Client 11.0"
        ));
    }

    /// AUD-101: a saved connection with the legacy driver says why it cannot
    /// connect, before any connection is tried.
    #[test]
    fn the_legacy_sql_server_driver_is_refused_with_a_clear_message() {
        let mut value = connection(DatabaseKind::SqlServer);
        value.driver = "SQL Server".to_owned();
        let error = value
            .connection_string(DatabaseKind::SqlServer)
            .unwrap_err();
        assert_eq!(
            error,
            "Este controlador no admite el cifrado; elige ODBC Driver 17 o 18."
        );
        value.driver = "ODBC Driver 17 for SQL Server".to_owned();
        assert!(value.connection_string(DatabaseKind::SqlServer).is_ok());
    }
}
