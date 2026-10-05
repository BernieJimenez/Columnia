use serde::Serialize;

use tauri::{DragDropEvent, Emitter, Manager, WindowEvent};

pub mod automation;
mod catalog_recovery;
mod crash_report;
mod dataset;
mod dataset_fingerprints;
mod delivery_presets;
mod diagnostics;
mod duckdb_query;
pub mod privacy;
mod project_recovery;
mod projects;
mod remote_databases;
mod remote_delivery_ledger;
mod resource;
mod reusable_tasks;
mod session_guard;
#[cfg(desktop)]
mod updater;

use resource::{PerformanceProfile, PerformanceSettings};

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct AppInfo {
    name: &'static str,
    version: &'static str,
    platform: &'static str,
    /// Optional so older desktop builds can omit the updater capability flag.
    updater_configured: Option<bool>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ResourceUsage {
    pub process_cpu_percentage: f32,
    pub system_cpu_percentage: f32,
    pub logical_cpu_count: usize,
    pub process_memory_bytes: u64,
    pub system_memory_used_bytes: u64,
    pub system_memory_total_bytes: u64,
    /// Optional so the frontend can remain compatible with older desktop builds.
    pub system_memory_available_bytes: Option<u64>,
    /// Optional capability block; absent means an older runtime did not expose GPU measurements.
    pub gpu: Option<GpuUsage>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GpuUsage {
    pub status: GpuStatus,
    pub usage_percentage: Option<f32>,
    pub memory_used_bytes: Option<u64>,
    pub memory_total_bytes: Option<u64>,
    pub reason: Option<&'static str>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum GpuStatus {
    Available,
    Unavailable,
}

fn updater_configured() -> bool {
    #[cfg(desktop)]
    {
        updater::configured()
    }
    #[cfg(not(desktop))]
    {
        false
    }
}

fn current_app_info() -> AppInfo {
    AppInfo {
        name: "Columnia",
        version: env!("CARGO_PKG_VERSION"),
        platform: std::env::consts::OS,
        updater_configured: Some(updater_configured()),
    }
}

#[tauri::command]
fn get_app_info() -> AppInfo {
    current_app_info()
}

#[tauri::command]
fn get_resource_usage() -> Result<ResourceUsage, String> {
    resource::get_resource_usage()
}

#[tauri::command]
fn get_performance_settings() -> Result<PerformanceSettings, String> {
    resource::get_performance_settings()
}

#[tauri::command]
fn set_performance_profile(profile: PerformanceProfile) -> Result<PerformanceSettings, String> {
    resource::set_performance_profile(profile)
}

#[cfg(desktop)]
fn restore_main_window(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };

    let _ = window.show();
    let _ = window.unminimize();
    let _ = window.set_focus();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();

    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            restore_main_window(app);
        }));
        // SEG-08: without an endpoint compiled in there is nothing to update
        // from, so the plugin is not loaded; the commands still answer that
        // the updater is not configured.
        if updater::configured() {
            builder = builder.plugin(tauri_plugin_updater::Builder::new().build());
        }
        builder = builder.manage(updater::UpdaterState::default());
    }

    builder
        .plugin(tauri_plugin_dialog::init())
        .manage(dataset::DatasetState::default())
        .manage(remote_databases::RemoteTargetConfirmations::default())
        .setup(|app| {
            let app_data_dir = app
                .path()
                .app_data_dir()
                .map_err(Box::<dyn std::error::Error>::from)?;
            crash_report::install(&app_data_dir);
            // DAT-04: history folders of earlier sessions hold copies of the data.
            std::thread::spawn(dataset::purge_finished_history_directories);
            let mut session = session_guard::SessionGuard::begin(&app_data_dir);
            app.manage(session_guard::UnsavedWork::default());
            dataset::remove_stale_converted_sources(&std::env::temp_dir());
            let projects = projects::ProjectState::initialize(app_data_dir.clone())
                .map_err(std::io::Error::other)?;
            app.manage(projects);
            // ARQ-02: a damaged secondary catalog is set aside, not fatal.
            let (reusable_tasks, tasks_aside) = catalog_recovery::initialize_or_set_aside(
                &app_data_dir.join("reusable-tasks.sqlite3"),
                || reusable_tasks::ReusableTaskState::initialize(app_data_dir.clone()),
            )
            .map_err(std::io::Error::other)?;
            app.manage(reusable_tasks);
            let (delivery_presets, presets_aside) = catalog_recovery::initialize_or_set_aside(
                &app_data_dir.join("delivery-presets.sqlite3"),
                || delivery_presets::DeliveryPresetState::initialize(app_data_dir.clone()),
            )
            .map_err(std::io::Error::other)?;
            app.manage(delivery_presets);
            session.record_set_aside_catalogs(tasks_aside.into_iter().chain(presets_aside));
            app.manage(session);
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let unsaved = window
                    .app_handle()
                    .try_state::<session_guard::UnsavedWork>()
                    .is_some_and(|state| state.is_set());
                if unsaved {
                    // FUN-09: ask before discarding work no project keeps. The
                    // answer arrives later, so the window closes from here.
                    api.prevent_close();
                    use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
                    let closing = window.clone();
                    window
                        .app_handle()
                        .dialog()
                        .message(session_guard::UNSAVED_WORK_CLOSE_MESSAGE)
                        .title("Cambios sin guardar")
                        .kind(MessageDialogKind::Warning)
                        .buttons(MessageDialogButtons::OkCancelCustom(
                            "Cerrar sin guardar".to_owned(),
                            "Volver".to_owned(),
                        ))
                        .show(move |close| {
                            if close {
                                if let Some(state) = closing
                                    .app_handle()
                                    .try_state::<session_guard::UnsavedWork>()
                                {
                                    state.set_clean();
                                }
                                let _ = closing.destroy();
                            }
                        });
                }
                return;
            }
            if let WindowEvent::DragDrop(DragDropEvent::Drop { paths, .. }) = event {
                if let Some(path) = paths.first() {
                    window
                        .app_handle()
                        .state::<dataset::DatasetState>()
                        .queue_dropped_path(path.clone());
                    let _ = window.emit("columnia://dataset-drop", ());
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_app_info,
            get_resource_usage,
            get_performance_settings,
            set_performance_profile,
            diagnostics::save_diagnostic_report,
            dataset::samples::list_sample_datasets,
            dataset::samples::inspect_sample_dataset,
            dataset::pick_dataset_source,
            dataset::inspect_dropped_dataset,
            dataset::inspect_workbook_sheets,
            dataset::convert_dataset_selection_encoding,
            dataset::reinterpret_dataset_selection,
            session_guard::get_session_status,
            session_guard::set_unsaved_work,
            dataset::preview_delimited_header_review,
            dataset::preview_dataset_selection,
            dataset::load_dataset_selection,
            dataset::discard_dataset_selection,
            dataset::compare_dataset,
            dataset::get_dataset_conflict_page,
            dataset::join_dataset,
            dataset::resolve_dataset_conflicts,
            dataset::clear_dataset_comparison,
            dataset::use_consolidated_dataset,
            dataset::get_dataset_page,
            dataset::query_dataset,
            dataset::get_dataset_profile,
            dataset::get_temporal_aggregation,
            dataset::validate_quality_rules,
            remote_databases::test_database_connection,
            dataset::preflight_database_export,
            dataset::export_dataset_to_database,
            dataset::cancel_operation,
            dataset::export_dataset,
            dataset::open_last_export,
            dataset::open_last_export_in_power_bi,
            dataset::get_explore_panel,
            dataset::save_transform_recipe,
            dataset::pick_transform_recipe,
            dataset::save_quality_rules_document,
            dataset::pick_quality_rules_migration,
            #[cfg(desktop)]
            updater::check_for_update,
            #[cfg(desktop)]
            updater::download_update,
            #[cfg(desktop)]
            updater::cancel_update_download,
            #[cfg(desktop)]
            updater::install_update,
            dataset::remove_duplicates,
            dataset::remove_near_duplicates,
            dataset::remove_empty_rows,
            dataset::enable_row_audit,
            dataset::remove_constant_columns,
            dataset::remove_empty_columns,
            dataset::remove_high_null_columns,
            dataset::remove_identifier_columns,
            dataset::remove_personal_columns,
            dataset::mask_personal_values,
            dataset::normalize_column_names,
            dataset::trim_text_values,
            dataset::normalize_text_values,
            dataset::parse_date_values,
            dataset::cast_numeric_values,
            dataset::normalize_sentinel_values,
            dataset::normalize_boolean_values,
            dataset::fix_encoding_values,
            dataset::nullify_invalid_type_values,
            dataset::impute_missing_values,
            dataset::impute_categorical_values,
            dataset::impute_outlier_values,
            dataset::cap_outlier_values,
            dataset::drop_outlier_values,
            dataset::preview_safe_corrections,
            dataset::apply_safe_corrections,
            dataset::apply_transform_recipe,
            dataset::get_history_state,
            dataset::compare_history_snapshots,
            dataset::undo_last_change,
            dataset::redo_last_change,
            #[cfg(debug_assertions)]
            dataset::probe_seed_dataset,
            #[cfg(debug_assertions)]
            dataset::probe_save_transform_recipe,
            #[cfg(debug_assertions)]
            dataset::probe_export_dataset,
            #[cfg(debug_assertions)]
            projects::probe_reopen_project,
            projects::list_projects,
            projects::list_project_versions,
            projects::save_project,
            projects::autosave_project,
            projects::restore_project_version,
            projects::open_project,
            projects::delete_project,
            reusable_tasks::list_reusable_tasks,
            reusable_tasks::save_reusable_task,
            reusable_tasks::open_reusable_task,
            reusable_tasks::delete_reusable_task,
            reusable_tasks::check_reusable_task_schema,
            delivery_presets::list_delivery_presets,
            delivery_presets::open_delivery_preset,
            delivery_presets::save_delivery_preset,
            delivery_presets::delete_delivery_preset,
        ])
        .build(tauri::generate_context!())
        .unwrap_or_else(|error| {
            // ARQ-02: the release build has no console; say why it closes.
            show_startup_error(&format!(
                "Columnia no pudo iniciarse: {error}\n\nLos datos están en %APPDATA%\\app.columnia.desktop."
            ));
            std::process::exit(1)
        })
        .run(|app, event| {
            // A normal exit clears the session marker (DAT-01).
            if let tauri::RunEvent::Exit = event {
                if let Some(guard) = app.try_state::<session_guard::SessionGuard>() {
                    guard.end();
                }
            }
        });
}

fn show_startup_error(message: &str) {
    eprintln!("{message}");
    #[cfg(windows)]
    {
        #[link(name = "user32")]
        extern "system" {
            fn MessageBoxW(
                window: *mut std::ffi::c_void,
                text: *const u16,
                caption: *const u16,
                kind: u32,
            ) -> i32;
        }
        let wide = |text: &str| text.encode_utf16().chain([0]).collect::<Vec<u16>>();
        let (text, caption) = (wide(message), wide("Columnia"));
        const MB_ICONERROR: u32 = 0x10;
        // SAFETY: both buffers are NUL-terminated UTF-16 that outlive the call,
        // and a null owner window is allowed.
        unsafe {
            MessageBoxW(
                std::ptr::null_mut(),
                text.as_ptr(),
                caption.as_ptr(),
                MB_ICONERROR,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_info_uses_package_version_and_current_platform() {
        let info = current_app_info();

        assert_eq!(info.name, "Columnia");
        assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
        assert!(["windows", "macos", "linux"].contains(&info.platform));
    }
}
