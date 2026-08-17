use serde::Serialize;

mod dataset;

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(dataset::DatasetState::default())
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
            dataset::redo_last_change
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
