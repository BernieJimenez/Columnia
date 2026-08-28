use std::sync::Mutex;

use serde::Serialize;
use tauri::{ipc::Channel, AppHandle, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};
use tokio_util::sync::CancellationToken;

const UPDATER_NOT_CONFIGURED: &str =
    "El updater no está configurado para esta compilación; define COLUMNIA_UPDATER_ENDPOINT al compilar.";
const DOWNLOAD_CANCELLED: &str = "La descarga de la actualización fue cancelada.";
const UPDATE_VERSION_INVALID: &str = "El canal updater devolvió una versión inválida.";
const UPDATE_NOT_NEWER: &str =
    "El canal updater devolvió una versión que no es posterior a la instalada.";

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub current_version: String,
    pub version: String,
    pub notes: Option<String>,
    pub date: Option<String>,
    pub size_bytes: Option<u64>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UpdaterProgress {
    pub phase: &'static str,
    pub downloaded_bytes: u64,
    pub content_length: Option<u64>,
}

struct PendingUpdate {
    update: Update,
    downloaded_bytes: Option<Vec<u8>>,
    cancellation: Option<CancellationToken>,
}

#[derive(Default)]
pub struct UpdaterState {
    pending: Mutex<Option<PendingUpdate>>,
}

pub const fn configured() -> bool {
    option_env!("COLUMNIA_UPDATER_ENDPOINT").is_some()
}

fn with_pending<T>(
    app: &AppHandle,
    operation: impl FnOnce(&mut Option<PendingUpdate>) -> Result<T, String>,
) -> Result<T, String> {
    let state = app.state::<UpdaterState>();
    let mut pending = state
        .pending
        .lock()
        .map_err(|_| "El estado del updater quedó bloqueado inesperadamente.".to_owned())?;
    operation(&mut pending)
}

fn require_configured() -> Result<(), String> {
    if configured() {
        Ok(())
    } else {
        Err(UPDATER_NOT_CONFIGURED.to_owned())
    }
}

fn manifest_size(raw_json: &serde_json::Value, target: &str) -> Option<u64> {
    let platform = raw_json
        .get("platforms")
        .and_then(|platforms| platforms.get(target));
    raw_json
        .get("sizeBytes")
        .or_else(|| raw_json.get("size"))
        .and_then(serde_json::Value::as_u64)
        .or_else(|| {
            platform
                .and_then(|value| value.get("sizeBytes").or_else(|| value.get("size")))
                .and_then(serde_json::Value::as_u64)
        })
}

fn metadata_size(update: &Update) -> Option<u64> {
    manifest_size(&update.raw_json, &update.target)
}

fn is_strictly_newer(current: &str, candidate: &str) -> Result<bool, String> {
    let current = semver::Version::parse(current).map_err(|_| UPDATE_VERSION_INVALID.to_owned())?;
    let candidate =
        semver::Version::parse(candidate).map_err(|_| UPDATE_VERSION_INVALID.to_owned())?;
    Ok(candidate > current)
}

fn update_info(update: &Update) -> UpdateInfo {
    UpdateInfo {
        current_version: update.current_version.clone(),
        version: update.version.clone(),
        notes: update.body.clone(),
        date: update.date.map(|date| date.to_string()),
        size_bytes: metadata_size(update),
    }
}

fn updater(app: &AppHandle) -> Result<tauri_plugin_updater::Updater, String> {
    require_configured()?;
    app.updater()
        .map_err(|error| format!("No se pudo preparar el updater: {error}"))
}

#[tauri::command]
pub async fn check_for_update(app: AppHandle) -> Result<Option<UpdateInfo>, String> {
    let updater = updater(&app)?;
    let update = updater
        .check()
        .await
        .map_err(|error| format!("No se pudo comprobar si hay actualizaciones: {error}"))?;
    let update = match update {
        Some(update) if is_strictly_newer(&update.current_version, &update.version)? => {
            Some(update)
        }
        Some(_) => return Err(UPDATE_NOT_NEWER.to_owned()),
        None => None,
    };
    let info = update.as_ref().map(update_info);
    with_pending(&app, |pending| {
        if pending
            .as_ref()
            .and_then(|value| value.cancellation.as_ref())
            .is_some()
        {
            return Err("Ya hay una descarga de actualización en curso.".to_owned());
        }
        *pending = update.map(|update| PendingUpdate {
            update,
            downloaded_bytes: None,
            cancellation: None,
        });
        Ok(info)
    })
}

#[tauri::command]
pub async fn download_update(
    app: AppHandle,
    on_progress: Channel<UpdaterProgress>,
) -> Result<(), String> {
    require_configured()?;
    let (update, cancellation) = {
        with_pending(&app, |pending| {
            let value = pending.as_mut().ok_or_else(|| {
                "Primero comprueba si hay una actualización disponible.".to_owned()
            })?;
            if value.downloaded_bytes.is_some() {
                return Ok((None, CancellationToken::new()));
            }
            if value.cancellation.is_some() {
                return Err("Ya hay una descarga de actualización en curso.".to_owned());
            }
            let cancellation = CancellationToken::new();
            value.cancellation = Some(cancellation.clone());
            Ok((Some(value.update.clone()), cancellation))
        })?
    };

    let Some(update) = update else {
        return Ok(());
    };

    let _ = on_progress.send(UpdaterProgress {
        phase: "started",
        downloaded_bytes: 0,
        content_length: None,
    });

    let mut downloaded_bytes = 0_u64;
    let progress_channel = on_progress.clone();
    let download_future = update.download(
        move |chunk_length, content_length| {
            downloaded_bytes = downloaded_bytes.saturating_add(chunk_length as u64);
            let _ = progress_channel.send(UpdaterProgress {
                phase: "progress",
                downloaded_bytes,
                content_length,
            });
        },
        || {},
    );
    let result = tokio::select! {
        bytes = download_future => bytes.map_err(|error| error.to_string()),
        _ = cancellation.cancelled() => Err(DOWNLOAD_CANCELLED.to_owned()),
    };

    let successful_bytes = result.as_ref().ok().cloned();
    {
        with_pending(&app, |pending| {
            if let Some(value) = pending.as_mut() {
                value.cancellation = None;
                if let Some(bytes) = successful_bytes {
                    value.downloaded_bytes = Some(bytes);
                }
            }
            Ok(())
        })?;
    }

    match result {
        Ok(_) => {
            let _ = on_progress.send(UpdaterProgress {
                phase: "finished",
                downloaded_bytes,
                content_length: Some(downloaded_bytes),
            });
            Ok(())
        }
        Err(error) => {
            if error == DOWNLOAD_CANCELLED {
                let _ = on_progress.send(UpdaterProgress {
                    phase: "cancelled",
                    downloaded_bytes,
                    content_length: None,
                });
            }
            Err(error)
        }
    }
}

#[tauri::command]
pub fn cancel_update_download(app: AppHandle) -> Result<(), String> {
    let cancellation = with_pending(&app, |pending| {
        Ok(pending
            .as_ref()
            .and_then(|value| value.cancellation.clone()))
    })?;
    if let Some(cancellation) = cancellation {
        cancellation.cancel();
    }
    Ok(())
}

#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<(), String> {
    require_configured()?;
    let (update, bytes) = {
        with_pending(&app, |pending| {
            let value = pending
                .as_mut()
                .ok_or_else(|| "No hay una actualización preparada para instalar.".to_owned())?;
            let bytes = value.downloaded_bytes.take().ok_or_else(|| {
                "Descarga primero la actualización antes de instalarla.".to_owned()
            })?;
            Ok((value.update.clone(), bytes))
        })?
    };
    tauri::async_runtime::spawn_blocking(move || {
        update
            .install(bytes)
            .map_err(|error| format!("No se pudo instalar la actualización: {error}"))
    })
    .await
    .map_err(|error| format!("La instalación de la actualización se interrumpió: {error}"))??;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        configured, is_strictly_newer, manifest_size, UpdateInfo, UPDATE_NOT_NEWER,
        UPDATE_VERSION_INVALID,
    };
    use serde_json::json;

    #[test]
    fn updater_is_disabled_without_a_build_endpoint() {
        assert_eq!(
            configured(),
            option_env!("COLUMNIA_UPDATER_ENDPOINT").is_some()
        );
    }

    #[test]
    fn update_info_keeps_a_stable_serializable_contract() {
        let info = UpdateInfo {
            current_version: "0.57.0".to_owned(),
            version: "0.58.0".to_owned(),
            notes: Some("Correcciones".to_owned()),
            date: None,
            size_bytes: Some(123),
        };
        assert_eq!(info.version, "0.58.0");
        assert_eq!(info.size_bytes, Some(123));
    }

    #[test]
    fn metadata_size_reads_top_level_and_platform_values() {
        assert_eq!(
            manifest_size(&json!({ "size": 123 }), "windows-x86_64"),
            Some(123)
        );
        assert_eq!(
            manifest_size(
                &json!({ "platforms": { "windows-x86_64": { "sizeBytes": 456 } } }),
                "windows-x86_64"
            ),
            Some(456)
        );
        assert_eq!(manifest_size(&json!({}), "windows-x86_64"), None);
    }

    #[test]
    fn update_policy_accepts_a_newer_stable_version() {
        assert_eq!(is_strictly_newer("0.57.0", "0.58.0"), Ok(true));
    }

    #[test]
    fn update_policy_rejects_downgrade_and_equal_versions() {
        assert_eq!(is_strictly_newer("0.57.0", "0.56.0"), Ok(false));
        assert_eq!(is_strictly_newer("0.57.0", "0.57.0"), Ok(false));
        assert_eq!(
            UPDATE_NOT_NEWER,
            "El canal updater devolvió una versión que no es posterior a la instalada."
        );
    }

    #[test]
    fn update_policy_uses_semver_prerelease_ordering() {
        assert_eq!(is_strictly_newer("0.57.0", "0.58.0-rc.1"), Ok(true));
        assert_eq!(is_strictly_newer("0.58.0", "0.58.0-rc.1"), Ok(false));
    }

    #[test]
    fn update_policy_rejects_invalid_versions_closed() {
        assert_eq!(
            is_strictly_newer("installed", "0.58.0"),
            Err(UPDATE_VERSION_INVALID.to_owned())
        );
        assert_eq!(
            is_strictly_newer("0.57.0", "latest"),
            Err(UPDATE_VERSION_INVALID.to_owned())
        );
    }
}
