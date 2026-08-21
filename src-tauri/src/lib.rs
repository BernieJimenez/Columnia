use serde::Serialize;

use tauri::Manager;

pub mod automation;
mod dataset;
mod projects;

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct AppInfo {
    name: &'static str,
    version: &'static str,
    platform: &'static str,
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
            dataset::pick_dataset_source,
            dataset::load_dataset_selection,
            dataset::discard_dataset_selection,
            dataset::get_dataset_page,
            dataset::get_dataset_profile,
            dataset::validate_quality_rules,
            dataset::cancel_operation,
            dataset::export_dataset,
            dataset::save_transform_recipe,
            dataset::pick_transform_recipe,
            dataset::remove_duplicates,
            dataset::normalize_column_names,
            dataset::trim_text_values,
            dataset::normalize_text_values,
            dataset::apply_safe_corrections,
            dataset::apply_transform_recipe,
            dataset::get_history_state,
            dataset::undo_last_change,
            dataset::redo_last_change,
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
