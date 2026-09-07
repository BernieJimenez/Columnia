use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use tauri::{AppHandle, Manager};

use super::{
    canonicalize_existing_file, inspect_dataset_path, is_symbolic_link_or_reparse_point,
    DatasetSourceInspection, SampleDatasetDescriptor,
};

struct SampleDatasetDefinition {
    id: &'static str,
    name: &'static str,
    format: &'static str,
    description: &'static str,
    file_name: &'static str,
    content: &'static str,
}

const SAMPLE_DATASETS: &[SampleDatasetDefinition] = &[
    SampleDatasetDefinition {
        id: "quality",
        name: "Clientes · señales de calidad",
        format: "csv",
        description: "Nulos, identificadores y fechas para probar Revisar y Preparar.",
        file_name: "clientes_calidad.csv",
        content: "cliente_id,nombre,segmento,alta,monto\n1001,Ana Torres,Pyme,2025-01-14,1250.50\n1002,Luis Pérez,Empresa,2025-02-03,980.00\n,Cuenta sin identificador,Pyme,,450.00\n1004,María Rojas,Pyme,2025-02-28,\n",
    },
    SampleDatasetDefinition {
        id: "temporal",
        name: "Ventas · serie temporal",
        format: "tsv",
        description: "Una serie temporal pequeña para probar análisis y tendencias.",
        file_name: "ventas_mensuales.tsv",
        content: "fecha\tregion\tproducto\tunidades\tingresos\n2025-01-01\tNorte\tLicencia\t18\t2160\n2025-02-01\tNorte\tLicencia\t24\t2880\n2025-03-01\tSur\tSoporte\t15\t1800\n2025-04-01\tSur\tLicencia\t31\t3720\n2025-05-01\tNorte\tSoporte\t22\t2640\n",
    },
];

fn sample_dataset_definition(sample_id: &str) -> Result<&'static SampleDatasetDefinition, String> {
    SAMPLE_DATASETS
        .iter()
        .find(|sample| sample.id == sample_id)
        .ok_or_else(|| "El dataset de ejemplo solicitado no está disponible.".to_owned())
}

pub(super) fn ensure_sample_dataset(
    app_data_dir: &Path,
    sample_id: &str,
) -> Result<PathBuf, String> {
    let sample = sample_dataset_definition(sample_id)?;
    match fs::symlink_metadata(app_data_dir) {
        Ok(metadata) if is_symbolic_link_or_reparse_point(&metadata) || !metadata.is_dir() => {
            return Err("El almacenamiento local de ejemplos no es seguro.".to_owned());
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir_all(app_data_dir).map_err(|_| {
                "No se pudo preparar el almacenamiento local de ejemplos.".to_owned()
            })?;
        }
        Err(_) => {
            return Err("No se pudo verificar el almacenamiento local de ejemplos.".to_owned());
        }
    }
    let examples_dir = app_data_dir.join("examples");
    match fs::symlink_metadata(&examples_dir) {
        Ok(metadata) if is_symbolic_link_or_reparse_point(&metadata) || !metadata.is_dir() => {
            return Err("El almacenamiento local de ejemplos no es seguro.".to_owned());
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(&examples_dir).map_err(|_| {
                "No se pudo preparar el almacenamiento local de ejemplos.".to_owned()
            })?;
        }
        Err(_) => {
            return Err("No se pudo verificar el almacenamiento local de ejemplos.".to_owned());
        }
    }

    let path = examples_dir.join(sample.file_name);
    match OpenOptions::new().write(true).create_new(true).open(&path) {
        Ok(mut file) => {
            if let Err(error) = file
                .write_all(sample.content.as_bytes())
                .and_then(|_| file.sync_all())
            {
                let _ = fs::remove_file(&path);
                return Err(format!(
                    "No se pudo preparar el dataset de ejemplo: {error}"
                ));
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(_) => {
            return Err("No se pudo preparar el dataset de ejemplo.".to_owned());
        }
    }

    canonicalize_existing_file(&path, "el dataset de ejemplo")
}

#[tauri::command]
pub fn list_sample_datasets() -> Vec<SampleDatasetDescriptor> {
    SAMPLE_DATASETS
        .iter()
        .map(|sample| SampleDatasetDescriptor {
            id: sample.id.to_owned(),
            name: sample.name.to_owned(),
            format: sample.format.to_owned(),
            description: sample.description.to_owned(),
        })
        .collect()
}

#[tauri::command]
pub async fn inspect_sample_dataset(
    app: AppHandle,
    sample_id: String,
) -> Result<DatasetSourceInspection, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|_| "No se pudo resolver el almacenamiento local de ejemplos.".to_owned())?;
    let path = ensure_sample_dataset(&app_data_dir, &sample_id)?;
    inspect_dataset_path(&app, path).await
}
