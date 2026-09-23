use super::*;

pub(crate) fn load_dataset_for_automation(
    input: &Path,
    sheet_name: Option<&str>,
    header_mode: Option<SpreadsheetHeaderMode>,
) -> Result<(DataFrame, DatasetPreview), String> {
    load_dataset_for_automation_with_progress(input, sheet_name, header_mode, |_, _| {}, || false)
}

pub(crate) fn load_dataset_for_automation_with_progress<F, C>(
    input: &Path,
    sheet_name: Option<&str>,
    header_mode: Option<SpreadsheetHeaderMode>,
    mut report: F,
    is_cancelled: C,
) -> Result<(DataFrame, DatasetPreview), String>
where
    F: FnMut(&'static str, u8),
    C: Fn() -> bool + Sync,
{
    ensure_not_cancelled(is_cancelled())?;
    let (canonical, file_size_bytes, extension) = validate_dataset_file(input)?;
    ensure_materialization_budget(file_size_bytes)?;
    if spreadsheet_extensions(&extension) {
        let sheet_name = sheet_name.ok_or_else(|| "Selecciona una hoja del libro.".to_owned())?;
        let header_mode = header_mode
            .ok_or_else(|| "Elige cómo interpretar los encabezados del libro.".to_owned())?;
        if sheet_name.is_empty() {
            return Err("La hoja seleccionada no es válida.".to_owned());
        }
        report("Validando archivo", 10);
        ensure_not_cancelled(is_cancelled())?;
        report("Leyendo y detectando columnas", 25);
        let available_sheets = inspect_workbook(&canonical)?;
        if available_sheets
            .iter()
            .filter(|name| *name == sheet_name)
            .count()
            != 1
        {
            return Err("La hoja seleccionada no existe de forma única en el libro.".to_owned());
        }
        let frame =
            load_spreadsheet_sheet_with_cancel(&canonical, sheet_name, header_mode, || {
                is_cancelled()
            })?;
        ensure_not_cancelled(is_cancelled())?;
        report("Preparando vista previa", 85);
        let preview = dataset_preview(&canonical, &frame)?;
        report("Preparando sesión", 95);
        Ok((frame, preview))
    } else {
        if sheet_name.is_some() || header_mode.is_some() {
            return Err("Este formato no utiliza selección de hoja ni encabezado.".to_owned());
        }
        load_dataset_with_progress(&canonical, report, is_cancelled)
    }
}

pub(crate) fn should_use_source_backed_automation(
    input: &Path,
    sheet_name: Option<&str>,
    header_mode: Option<SpreadsheetHeaderMode>,
) -> Result<bool, String> {
    let (_, file_size_bytes, extension) = validate_dataset_file(input)?;
    if spreadsheet_extensions(&extension) {
        if sheet_name.is_none() || header_mode.is_none() {
            return Err("Selecciona una hoja y un modo de encabezado para el libro.".to_owned());
        }
        return Ok(should_defer_source_load(&extension, file_size_bytes)
            && matches!(extension.as_str(), "xlsx" | "xlsb"));
    }
    if sheet_name.is_some() || header_mode.is_some() {
        return Err("Este formato no utiliza selección de hoja ni encabezado.".to_owned());
    }
    Ok(should_defer_source_load(&extension, file_size_bytes))
}

pub(crate) fn inspect_source_backed_for_automation(
    input: &Path,
    sheet_name: Option<&str>,
    header_mode: Option<SpreadsheetHeaderMode>,
) -> Result<DatasetPreview, String> {
    let (_, preview) = load_source_backed_dataset_for_automation(input, sheet_name, header_mode)?;
    Ok(preview)
}

pub(crate) struct AutomationSourceBackedTransformResult {
    pub(crate) exported: ExportResult,
    pub(crate) changed: bool,
    pub(crate) input_row_count: usize,
    pub(crate) output_row_count: usize,
    pub(crate) input_column_count: usize,
    pub(crate) output_column_count: usize,
}

pub(super) fn export_source_backed_for_automation(
    dataset: &LoadedDataset,
    output: &Path,
    format: ExportFormat,
    quality_validation: Option<&QualityValidationResult>,
    recipe: Option<&StoredTransformRecipe>,
) -> Result<ExportResult, String> {
    let (source_path, _) = current_duckdb_file_source(dataset)
        .ok_or_else(|| "La fuente source-backed ya no está disponible para exportar.".to_owned())?;
    let expected_file_size = fs::metadata(&source_path)
        .map_err(|error| format!("No se pudo verificar la fuente source-backed: {error}"))?
        .len();
    let row_count = dataset.row_count;
    match format {
        ExportFormat::Csv => export_source_backed_csv_atomic(
            &source_path,
            expected_file_size,
            output,
            |_, _| {},
            || false,
        ),
        ExportFormat::Json => export_source_backed_json_atomic(
            &source_path,
            expected_file_size,
            output,
            |_, _| {},
            || false,
        ),
        ExportFormat::Parquet => export_source_backed_parquet_atomic(
            &source_path,
            expected_file_size,
            output,
            |_, _| {},
            || false,
        ),
        ExportFormat::Sql => export_source_backed_sql_atomic(
            &source_path,
            expected_file_size,
            output,
            |_, _| {},
            || false,
        ),
        ExportFormat::Excel => export_source_backed_xlsx_atomic(
            &source_path,
            expected_file_size,
            row_count,
            output,
            |_, _| {},
            || false,
        ),
        ExportFormat::Sqlite => export_source_backed_sqlite_atomic(
            &source_path,
            expected_file_size,
            row_count,
            output,
            |_, _| {},
            || false,
        ),
        ExportFormat::Bundle => export_source_backed_bundle_atomic(
            &source_path,
            expected_file_size,
            row_count,
            quality_validation,
            recipe,
            output,
            |_, _| {},
            || false,
        ),
    }
}

pub(crate) fn transform_source_backed_for_automation(
    input: &Path,
    sheet_name: Option<&str>,
    header_mode: Option<SpreadsheetHeaderMode>,
    recipe: &StoredTransformRecipe,
    output: &Path,
    format: ExportFormat,
) -> Result<AutomationSourceBackedTransformResult, String> {
    let (mut dataset, _) =
        load_source_backed_dataset_for_automation(input, sheet_name, header_mode)?;
    let input_row_count = dataset.row_count;
    let input_column_count = dataset.frame.width();
    let result = apply_recipe_to_dataset(&mut dataset, &recipe.recipe)?;
    let exported =
        export_source_backed_for_automation(&dataset, output, format, None, Some(recipe))?;
    Ok(AutomationSourceBackedTransformResult {
        exported,
        changed: result.changed,
        input_row_count,
        output_row_count: result.dataset.row_count,
        input_column_count,
        output_column_count: result.dataset.column_count,
    })
}

pub(crate) fn evaluate_source_backed_quality_rules_for_automation(
    input: &Path,
    sheet_name: Option<&str>,
    header_mode: Option<SpreadsheetHeaderMode>,
    quality_rules: &[QualityRule],
) -> Result<QualityValidationResult, String> {
    let (dataset, _) = load_source_backed_dataset_for_automation(input, sheet_name, header_mode)?;
    let (source_path, _) = current_duckdb_file_source(&dataset)
        .ok_or_else(|| "La fuente source-backed ya no está disponible para validar.".to_owned())?;
    let (source_path, expected_file_size, extension) = validate_dataset_file(&source_path)?;
    evaluate_source_quality_rules_with_cancel(
        &source_path,
        &extension,
        expected_file_size,
        dataset.row_count,
        quality_rules,
        || false,
    )
}

pub(crate) fn load_quality_rules_for_automation(input: &Path) -> Result<Vec<QualityRule>, String> {
    let document = parse_quality_rules_document(read_quality_rules_json(input)?, true)?;
    Ok(document.rules)
}

pub(crate) fn evaluate_quality_rules_for_automation(
    frame: &DataFrame,
    quality_rules: &[QualityRule],
) -> Result<QualityValidationResult, String> {
    evaluate_quality_rules(frame, quality_rules)
}

pub(crate) fn load_recipe_for_automation(input: &Path) -> Result<TransformRecipe, String> {
    load_recipe_file(input).map(|document| document.recipe)
}

pub(crate) fn load_stored_recipe_for_automation(
    input: &Path,
) -> Result<StoredTransformRecipe, String> {
    load_recipe_file(input)
}

pub(crate) fn apply_recipe_for_automation(
    source: &DataFrame,
    recipe: &TransformRecipe,
) -> Result<(DataFrame, bool), String> {
    validate_recipe_structure(recipe)?;
    let outcome = apply_recipe_to_frame(source, recipe)?;
    let candidate = outcome.0;
    let changed = !candidate.equals_missing(source);
    Ok((candidate, changed))
}

pub(crate) fn export_frame_for_automation(
    frame: &DataFrame,
    output: &Path,
    format: ExportFormat,
) -> Result<ExportResult, String> {
    export_frame_atomic(frame, output, format, |_, _| {}, || false)
}

pub(crate) fn export_frame_for_automation_with_recipe(
    frame: &DataFrame,
    output: &Path,
    format: ExportFormat,
    recipe: Option<&StoredTransformRecipe>,
) -> Result<ExportResult, String> {
    if recipe.is_none() {
        return export_frame_for_automation(frame, output, format);
    }
    export_frame_atomic_with_privacy_and_quality_and_recipe(
        frame,
        output,
        format,
        PrivacyMode::None,
        None,
        recipe,
        |_, _| {},
        || false,
    )
}
