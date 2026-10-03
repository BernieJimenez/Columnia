use std::{
    fs,
    path::{Path, PathBuf},
};

pub(super) fn dataset_extension(path: &Path) -> Result<String, String> {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .filter(|extension| {
            matches!(
                extension.as_str(),
                "csv"
                    | "tsv"
                    | "txt"
                    | "json"
                    | "jsonl"
                    | "ndjson"
                    | "parquet"
                    | "xlsx"
                    | "xls"
                    | "xlsb"
                    | "ods"
            )
        })
        .ok_or_else(|| {
            "Columnia admite CSV, TSV, TXT delimitado, JSON, Parquet y libros Excel/ODS en esta versión.".to_owned()
        })
}

#[cfg(not(windows))]
pub(super) fn is_symbolic_link_or_reparse_point(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

#[cfg(windows)]
pub(super) fn is_symbolic_link_or_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;

    metadata.file_type().is_symlink()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

pub(super) fn canonicalize_existing_file(path: &Path, label: &str) -> Result<PathBuf, String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("No se pudo verificar {label}: {error}"))?;
    if is_symbolic_link_or_reparse_point(&metadata) {
        return Err(format!(
            "{label} no puede ser un enlace simbólico o punto de reanálisis."
        ));
    }
    if !metadata.is_file() {
        return Err(format!("{label} no existe o no es un archivo regular."));
    }

    let canonical = fs::canonicalize(path)
        .map_err(|error| format!("No se pudo resolver la ruta real de {label}: {error}"))?;
    let canonical_metadata = fs::metadata(&canonical)
        .map_err(|error| format!("No se pudo verificar la ruta real de {label}: {error}"))?;
    if !canonical_metadata.is_file() {
        return Err(format!("{label} no apunta a un archivo regular."));
    }
    Ok(canonical)
}

pub(super) fn canonicalize_write_destination(path: &Path, label: &str) -> Result<PathBuf, String> {
    let file_name = path
        .file_name()
        .filter(|name| !name.is_empty())
        .ok_or_else(|| format!("No se pudo resolver el nombre de {label}."))?;
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let canonical_parent = fs::canonicalize(parent)
        .map_err(|error| format!("No se pudo resolver la carpeta de {label}: {error}"))?;
    if !canonical_parent.is_dir() {
        return Err(format!("La carpeta de {label} no es un directorio válido."));
    }

    match fs::symlink_metadata(path) {
        Ok(metadata) if is_symbolic_link_or_reparse_point(&metadata) => {
            return Err(format!(
                "El destino de {label} no puede ser un enlace simbólico o punto de reanálisis."
            ));
        }
        Ok(metadata) if !metadata.is_file() => {
            return Err(format!("El destino de {label} no es un archivo regular."));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(format!(
                "No se pudo verificar el destino de {label}: {error}"
            ));
        }
    }

    Ok(canonical_parent.join(file_name))
}

/// Message shown when an export would replace the file it was read from.
pub(crate) const DESTINATION_IS_SOURCE_MESSAGE: &str =
    "El destino es el archivo de origen. Elige otro nombre: Columnia nunca modifica el archivo original.";

/// Rejects a destination that is the same file as one of `sources`. The
/// comparison uses file identity, so `..`, letter case and hard links cannot
/// hide the original.
pub(crate) fn ensure_destination_is_not_source(
    destination: &Path,
    sources: &[&Path],
) -> Result<(), String> {
    if fs::symlink_metadata(destination).is_err() {
        return Ok(());
    }
    for source in sources {
        let same = same_file::is_same_file(destination, source).unwrap_or_else(|_| {
            match (fs::canonicalize(destination), fs::canonicalize(source)) {
                (Ok(left), Ok(right)) => {
                    left.to_string_lossy().to_lowercase() == right.to_string_lossy().to_lowercase()
                }
                _ => false,
            }
        });
        if same {
            return Err(DESTINATION_IS_SOURCE_MESSAGE.to_owned());
        }
    }
    Ok(())
}

pub(super) fn validate_dataset_file(path: &Path) -> Result<(PathBuf, u64, String), String> {
    let canonical = canonicalize_existing_file(path, "el dataset seleccionado")?;
    let extension = dataset_extension(&canonical)?;

    let size = fs::metadata(&canonical)
        .map_err(|error| format!("No se pudieron leer los metadatos del archivo: {error}"))?
        .len();

    Ok((canonical, size, extension))
}
