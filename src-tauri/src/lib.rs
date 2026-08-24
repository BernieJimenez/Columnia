use serde::Serialize;

use tauri::Manager;

pub mod automation;
mod dataset;
mod projects;
mod resource;

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct AppInfo {
    name: &'static str,
    version: &'static str,
    platform: &'static str,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ResourceUsage {
    pub process_cpu_percentage: f32,
    pub system_cpu_percentage: f32,
    pub process_memory_bytes: u64,
    pub system_memory_used_bytes: u64,
    pub system_memory_total_bytes: u64,
}

fn current_app_info() -> AppInfo {
    AppInfo {
        name: "Columnia",
        version: env!("CARGO_PKG_VERSION"),
        platform: std::env::consts::OS,
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
    }

    builder
        .plugin(tauri_plugin_dialog::init())
        .manage(dataset::DatasetState::default())
        .setup(|app| {
            let app_data_dir = app
                .path()
                .app_data_dir()
                .map_err(Box::<dyn std::error::Error>::from)?;
            let projects =
                projects::ProjectState::initialize(app_data_dir).map_err(std::io::Error::other)?;
            app.manage(projects);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_app_info,
            get_resource_usage,
            dataset::pick_dataset_source,
            dataset::load_dataset_selection,
            dataset::discard_dataset_selection,
            dataset::compare_dataset,
            dataset::join_dataset,
            dataset::resolve_dataset_conflicts,
            dataset::clear_dataset_comparison,
            dataset::use_consolidated_dataset,
            dataset::get_dataset_page,
            dataset::query_dataset,
            dataset::get_dataset_profile,
            dataset::validate_quality_rules,
            dataset::cancel_operation,
            dataset::export_dataset,
            dataset::save_transform_recipe,
            dataset::pick_transform_recipe,
            dataset::save_quality_rules_document,
            dataset::pick_quality_rules_migration,
            dataset::remove_duplicates,
            dataset::remove_empty_rows,
            dataset::enable_row_audit,
            dataset::remove_constant_columns,
            dataset::remove_empty_columns,
            dataset::remove_high_null_columns,
            dataset::normalize_column_names,
            dataset::trim_text_values,
            dataset::normalize_text_values,
            dataset::normalize_sentinel_values,
            dataset::normalize_boolean_values,
            dataset::impute_missing_values,
            dataset::apply_safe_corrections,
            dataset::apply_transform_recipe,
            dataset::get_history_state,
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
            projects::get_recovery_candidate,
            projects::save_project,
            projects::open_project,
            projects::delete_project
        ])
        .run(tauri::generate_context!())
        .expect("Columnia no pudo iniciar el runtime de escritorio");
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
