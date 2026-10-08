use super::*;

pub(super) fn spreadsheet_extensions(extension: &str) -> bool {
    matches!(extension, "xlsx" | "xls" | "xlsb" | "ods")
}

pub(super) fn import_format_for_extension(extension: &str) -> &'static str {
    if spreadsheet_extensions(extension) {
        "excel"
    } else if extension == "parquet" {
        "parquet"
    } else if matches!(extension, "json" | "jsonl" | "ndjson") {
        "json"
    } else if extension == "tsv" {
        "tsv"
    } else {
        "csv"
    }
}

pub(super) fn inspect_workbook(path: &Path) -> Result<Vec<String>, String> {
    let workbook = open_workbook_auto(path)
        .map_err(|error| format!("No se pudo abrir el libro seleccionado: {error}"))?;
    let sheets = workbook.sheet_names();
    if sheets.is_empty() {
        return Err("El libro no contiene hojas disponibles.".to_owned());
    }
    Ok(sheets)
}

/// PROD-19: what Cargar says about each sheet before loading it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct WorkbookSheetDetails {
    pub(super) name: String,
    pub(super) hidden: bool,
    /// Merged ranges: only their first cell keeps the value.
    pub(super) merged_cell_count: usize,
}

pub(super) fn inspect_workbook_details(path: &Path) -> Result<Vec<WorkbookSheetDetails>, String> {
    let mut workbook = open_workbook_auto(path)
        .map_err(|error| format!("No se pudo abrir el libro seleccionado: {error}"))?;
    let sheets = workbook
        .sheets_metadata()
        .iter()
        .map(|sheet| {
            (
                sheet.name.clone(),
                sheet.visible != calamine::SheetVisible::Visible,
            )
        })
        .collect::<Vec<_>>();
    if sheets.is_empty() {
        return Err("El libro no contiene hojas disponibles.".to_owned());
    }
    Ok(sheets
        .into_iter()
        .map(|(name, hidden)| {
            // Only XLSX and XLS record merged ranges; a failure only means no notice.
            let merged_cell_count = match &mut workbook {
                Sheets::Xlsx(xlsx) => xlsx
                    .merge_cells_by_sheet_name(&name)
                    .map_or(0, |cells| cells.len()),
                Sheets::Xls(xls) => xls
                    .merge_cells_by_sheet_name(&name)
                    .map_or(0, |cells| cells.len()),
                _ => 0,
            };
            WorkbookSheetDetails {
                name,
                hidden,
                merged_cell_count,
            }
        })
        .collect())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SpreadsheetColumnKind {
    Null,
    Boolean,
    Int64,
    Float64,
    Datetime,
    Duration,
    String,
}

/// QA-16: XLSX stores every number as a double, so an integer written by
/// Columnia (or typed in Excel) comes back as `1.0`. A whole number that a
/// double holds exactly counts as an integer; one fraction makes the column
/// decimal.
fn integral_float(value: f64) -> Option<i64> {
    (value.is_finite() && value.fract() == 0.0 && value.abs() <= (1_u64 << 53) as f64)
        .then_some(value as i64)
}

pub(super) fn spreadsheet_cell_kind(cell: &Data) -> SpreadsheetColumnKind {
    match cell {
        Data::Empty => SpreadsheetColumnKind::Null,
        Data::Bool(_) => SpreadsheetColumnKind::Boolean,
        Data::Int(_) => SpreadsheetColumnKind::Int64,
        Data::Float(value) if integral_float(*value).is_some() => SpreadsheetColumnKind::Int64,
        Data::Float(_) => SpreadsheetColumnKind::Float64,
        Data::DateTime(value) if value.is_duration() => SpreadsheetColumnKind::Duration,
        Data::DateTime(_) => SpreadsheetColumnKind::Datetime,
        Data::DateTimeIso(_) if cell.as_datetime().is_some() => SpreadsheetColumnKind::Datetime,
        Data::DurationIso(_) if cell.as_duration().is_some() => SpreadsheetColumnKind::Duration,
        Data::String(_) | Data::DateTimeIso(_) | Data::DurationIso(_) | Data::Error(_) => {
            SpreadsheetColumnKind::String
        }
    }
}

pub(super) fn merge_spreadsheet_kinds(
    left: SpreadsheetColumnKind,
    right: SpreadsheetColumnKind,
) -> SpreadsheetColumnKind {
    use SpreadsheetColumnKind::*;
    match (left, right) {
        (Null, kind) | (kind, Null) => kind,
        (left, right) if left == right => left,
        (Int64, Float64) | (Float64, Int64) => Float64,
        _ => String,
    }
}

/// Unique column names. A repeated header gets the first `_n` suffix that is
/// neither taken nor another header of the sheet, so `a, a, a_2` gives three
/// names (FUN-15).
pub(super) fn unique_spreadsheet_headers(headers: Vec<String>) -> Vec<String> {
    let bases = headers
        .into_iter()
        .enumerate()
        .map(|(index, header)| {
            let trimmed = header.trim();
            if trimmed.is_empty() {
                format!("column_{}", index + 1)
            } else {
                trimmed.to_owned()
            }
        })
        .collect::<Vec<_>>();
    let reserved = bases.iter().cloned().collect::<HashSet<_>>();
    let mut used = HashSet::<String>::with_capacity(bases.len());
    bases
        .into_iter()
        .map(|base| {
            if used.insert(base.clone()) {
                return base;
            }
            let name = (2..)
                .map(|suffix| format!("{base}_{suffix}"))
                .find(|candidate| !used.contains(candidate) && !reserved.contains(candidate))
                .expect("siempre hay un sufijo libre");
            used.insert(name.clone());
            name
        })
        .collect()
}

pub(super) fn map_spreadsheet_cells_with_cancel<T, C, F>(
    cells: &[&Data],
    is_cancelled: &C,
    mut map: F,
) -> Result<Vec<T>, String>
where
    C: Fn() -> bool + Sync,
    F: FnMut(&Data) -> Result<T, String>,
{
    let mut values = Vec::with_capacity(cells.len());
    for (index, cell) in cells.iter().enumerate() {
        if index % CANCELLABLE_READ_BATCH_ROWS == 0 {
            ensure_not_cancelled(is_cancelled())?;
        }
        values.push(map(cell)?);
    }
    ensure_not_cancelled(is_cancelled())?;
    Ok(values)
}

pub(super) fn spreadsheet_cells_to_column<C>(
    name: &str,
    cells: &[&Data],
    forced_kind: Option<SpreadsheetColumnKind>,
    is_cancelled: &C,
) -> Result<Column, String>
where
    C: Fn() -> bool + Sync,
{
    let mut kind = forced_kind.unwrap_or(SpreadsheetColumnKind::Null);
    if forced_kind.is_none() {
        for (index, cell) in cells.iter().enumerate() {
            if index % CANCELLABLE_READ_BATCH_ROWS == 0 {
                ensure_not_cancelled(is_cancelled())?;
            }
            kind = merge_spreadsheet_kinds(kind, spreadsheet_cell_kind(cell));
        }
    }
    if kind == SpreadsheetColumnKind::Float64 {
        for (index, cell) in cells.iter().enumerate() {
            if index % CANCELLABLE_READ_BATCH_ROWS == 0 {
                ensure_not_cancelled(is_cancelled())?;
            }
            if matches!(cell, Data::Int(value) if value.unsigned_abs() > (1_u64 << 53)) {
                kind = SpreadsheetColumnKind::String;
                break;
            }
        }
    }
    let column_name = name.into();
    let incompatible =
        |value: &Data| format!("La columna '{name}' contiene un valor incompatible: {value}");

    match kind {
        SpreadsheetColumnKind::Null => Ok(Column::full_null(
            column_name,
            cells.len(),
            &polars::prelude::DataType::Null,
        )),
        SpreadsheetColumnKind::Boolean => {
            let values =
                map_spreadsheet_cells_with_cancel(cells, is_cancelled, |cell| match cell {
                    Data::Empty => Ok(None),
                    Data::Bool(value) => Ok(Some(*value)),
                    other => Err(incompatible(other)),
                })?;
            Ok(Series::new(column_name, values).into_column())
        }
        SpreadsheetColumnKind::Int64 => {
            let values =
                map_spreadsheet_cells_with_cancel(cells, is_cancelled, |cell| match cell {
                    Data::Empty => Ok(None),
                    Data::Int(value) => Ok(Some(*value)),
                    Data::Float(value) => integral_float(*value)
                        .map(Some)
                        .ok_or_else(|| incompatible(cell)),
                    other => Err(incompatible(other)),
                })?;
            Ok(Series::new(column_name, values).into_column())
        }
        SpreadsheetColumnKind::Float64 => {
            let values =
                map_spreadsheet_cells_with_cancel(cells, is_cancelled, |cell| match cell {
                    Data::Empty => Ok(None),
                    Data::Int(value) if value.unsigned_abs() <= (1_u64 << 53) => {
                        Ok(Some(*value as f64))
                    }
                    Data::Int(_) => Err(format!(
                        "La columna '{name}' mezcla decimales con enteros que perderían precisión."
                    )),
                    Data::Float(value) => Ok(Some(*value)),
                    other => Err(incompatible(other)),
                })?;
            Ok(Series::new(column_name, values).into_column())
        }
        SpreadsheetColumnKind::Datetime => {
            let values =
                map_spreadsheet_cells_with_cancel(cells, is_cancelled, |cell| match cell {
                    Data::Empty => Ok(None),
                    Data::DateTime(value) if !value.is_duration() => value
                        .as_datetime()
                        .map(|value| Some(value.and_utc().timestamp_millis()))
                        .ok_or_else(|| incompatible(cell)),
                    Data::DateTimeIso(_) => cell
                        .as_datetime()
                        .map(|value| Some(value.and_utc().timestamp_millis()))
                        .ok_or_else(|| incompatible(cell)),
                    other => Err(incompatible(other)),
                })?;
            Ok(Series::new(column_name, values)
                .cast(&polars::prelude::DataType::Datetime(
                    TimeUnit::Milliseconds,
                    None,
                ))
                .map_err(|error| format!("No se pudo conservar una fecha de '{name}': {error}"))?
                .into_column())
        }
        SpreadsheetColumnKind::Duration => {
            let values =
                map_spreadsheet_cells_with_cancel(cells, is_cancelled, |cell| match cell {
                    Data::Empty => Ok(None),
                    Data::DateTime(value) if value.is_duration() => value
                        .as_duration()
                        .map(|value| Some(value.num_milliseconds()))
                        .ok_or_else(|| incompatible(cell)),
                    Data::DurationIso(_) => cell
                        .as_duration()
                        .map(|value| Some(value.num_milliseconds()))
                        .ok_or_else(|| incompatible(cell)),
                    other => Err(incompatible(other)),
                })?;
            Ok(Series::new(column_name, values)
                .cast(&polars::prelude::DataType::Duration(TimeUnit::Milliseconds))
                .map_err(|error| format!("No se pudo conservar una duración de '{name}': {error}"))?
                .into_column())
        }
        SpreadsheetColumnKind::String => {
            let values = map_spreadsheet_cells_with_cancel(cells, is_cancelled, |cell| {
                Ok(match cell {
                    Data::Empty => None,
                    // A date in a column mixed with text keeps its date, not
                    // Excel's serial number (FUN-31).
                    Data::DateTime(value) if !value.is_duration() => Some(
                        cell.as_datetime()
                            .map(|datetime| datetime.to_string())
                            .unwrap_or_else(|| value.to_string()),
                    ),
                    value => Some(value.to_string()),
                })
            })?;
            Ok(Series::new(column_name, values).into_column())
        }
    }
}

/// PROD-19: rows read before deciding where the header is.
const HEADER_DETECTION_ROWS: usize = 20;

/// PROD-19: the header row from the filled cells of the first rows. Leading
/// rows with one value at most (a title, a subtitle, a blank line) are
/// skipped when a later row fills at least half of the widest one; anything
/// else keeps the first row, as before.
pub(super) fn detect_header_row(filled_cells: &[usize]) -> usize {
    let widest = filled_cells.iter().copied().max().unwrap_or(0);
    if widest < 3 {
        return 0;
    }
    let needed = widest.div_ceil(2).max(2);
    for (row, &filled) in filled_cells.iter().enumerate() {
        if filled >= needed {
            return row;
        }
        if filled > 1 {
            return 0;
        }
    }
    0
}

#[derive(Clone, Debug)]
pub(super) struct SpreadsheetSnapshotPlan {
    start: (u32, u32),
    width: usize,
    height: usize,
    data_start: usize,
    pub(super) data_rows: usize,
    headers: Vec<String>,
    kinds: Vec<SpreadsheetColumnKind>,
}

struct SpreadsheetSnapshotPlanBuilder {
    dimensions: Dimensions,
    width: usize,
    height: usize,
    data_start: usize,
    /// PROD-19: the header row once decided; `None` while the first rows are
    /// still being buffered (first-row headers only).
    header_row: Option<usize>,
    pending: Vec<((usize, usize), Data)>,
    filled_cells: Vec<usize>,
    header_values: Vec<String>,
    kinds: Vec<SpreadsheetColumnKind>,
    saw_cell: bool,
    /// Last row and column that hold a value. The plan uses this extent and
    /// not the declared `<dimension>`, which can claim `A1:XFD1048576` in a
    /// 5 KB workbook (ARQ-01).
    used_rows: usize,
    used_columns: usize,
}

impl SpreadsheetSnapshotPlanBuilder {
    fn new(dimensions: Dimensions, header_mode: SpreadsheetHeaderMode) -> Result<Self, String> {
        let width = dimensions
            .end
            .1
            .checked_sub(dimensions.start.1)
            .and_then(|value| value.checked_add(1))
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| "La hoja Excel tiene demasiadas columnas.".to_owned())?;
        let height = dimensions
            .end
            .0
            .checked_sub(dimensions.start.0)
            .and_then(|value| value.checked_add(1))
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| "La hoja Excel tiene demasiadas filas.".to_owned())?;
        if width == 0 || height == 0 {
            return Err("La hoja seleccionada está vacía.".to_owned());
        }
        let first_row = header_mode == SpreadsheetHeaderMode::FirstRow;
        Ok(Self {
            dimensions,
            width,
            height,
            // Generated headers read every row; first-row headers decide later.
            data_start: 0,
            header_row: (!first_row).then_some(usize::MAX),
            pending: Vec::new(),
            filled_cells: vec![0; HEADER_DETECTION_ROWS],
            header_values: vec![String::new(); width],
            kinds: vec![SpreadsheetColumnKind::Null; width],
            saw_cell: false,
            used_rows: 0,
            used_columns: 0,
        })
    }

    fn visit(&mut self, position: (u32, u32), value: Data) {
        let Some(row) = position.0.checked_sub(self.dimensions.start.0) else {
            return;
        };
        let Some(column) = position.1.checked_sub(self.dimensions.start.1) else {
            return;
        };
        let Some(row) = usize::try_from(row).ok() else {
            return;
        };
        let Some(column) = usize::try_from(column).ok() else {
            return;
        };
        self.visit_relative((row, column), value);
    }

    fn visit_relative(&mut self, (row, column): (usize, usize), value: Data) {
        if row >= self.height || column >= self.width || matches!(value, Data::Empty) {
            return;
        }
        self.saw_cell = true;
        self.used_rows = self.used_rows.max(row + 1);
        self.used_columns = self.used_columns.max(column + 1);
        if self.header_row.is_none() {
            if row < HEADER_DETECTION_ROWS {
                self.filled_cells[row] += 1;
                self.pending.push(((row, column), value));
                return;
            }
            self.decide_header_row();
        }
        self.place((row, column), value);
    }

    /// PROD-19: fixes the header row and places the buffered first rows.
    fn decide_header_row(&mut self) {
        let header_row = detect_header_row(&self.filled_cells);
        self.header_row = Some(header_row);
        self.data_start = header_row + 1;
        for (position, value) in std::mem::take(&mut self.pending) {
            self.place(position, value);
        }
    }

    fn place(&mut self, (row, column): (usize, usize), value: Data) {
        if Some(row) == self.header_row {
            self.header_values[column] = value.to_string();
            return;
        }
        if row < self.data_start {
            return;
        }
        self.kinds[column] =
            merge_spreadsheet_kinds(self.kinds[column], spreadsheet_cell_kind(&value));
        if self.kinds[column] == SpreadsheetColumnKind::Float64
            && matches!(value, Data::Int(number) if number.unsigned_abs() > (1_u64 << 53))
        {
            self.kinds[column] = SpreadsheetColumnKind::String;
        }
    }

    fn finish(
        mut self,
        header_mode: SpreadsheetHeaderMode,
    ) -> Result<SpreadsheetSnapshotPlan, String> {
        if !self.saw_cell {
            return Err("La hoja seleccionada está vacía.".to_owned());
        }
        if self.header_row.is_none() {
            self.decide_header_row();
        }
        self.width = self.used_columns;
        self.height = self.used_rows.max(self.data_start);
        self.header_values.truncate(self.width);
        self.kinds.truncate(self.width);
        let headers = match header_mode {
            SpreadsheetHeaderMode::FirstRow => unique_spreadsheet_headers(self.header_values),
            SpreadsheetHeaderMode::Generated => (1..=self.width)
                .map(|index| format!("column_{index}"))
                .collect(),
        };
        Ok(SpreadsheetSnapshotPlan {
            start: self.dimensions.start,
            width: self.width,
            height: self.height,
            data_start: self.data_start,
            data_rows: self.height.saturating_sub(self.data_start),
            headers,
            kinds: self.kinds,
        })
    }
}

pub(super) fn spreadsheet_snapshot_plan_from_range<C>(
    range: &Range<Data>,
    header_mode: SpreadsheetHeaderMode,
    is_cancelled: &C,
) -> Result<SpreadsheetSnapshotPlan, String>
where
    C: Fn() -> bool + Sync,
{
    ensure_not_cancelled(is_cancelled())?;
    if range.is_empty() {
        return Err("La hoja seleccionada está vacía.".to_owned());
    }
    let dimensions = Dimensions::new(
        range
            .start()
            .ok_or_else(|| "La hoja seleccionada está vacía.".to_owned())?,
        range
            .end()
            .ok_or_else(|| "La hoja seleccionada está vacía.".to_owned())?,
    );
    let mut builder = SpreadsheetSnapshotPlanBuilder::new(dimensions, header_mode)?;
    for (row, values) in range.rows().enumerate() {
        if row % CANCELLABLE_READ_BATCH_ROWS == 0 {
            ensure_not_cancelled(is_cancelled())?;
        }
        for (column, value) in values.iter().enumerate() {
            if column % 256 == 0 {
                ensure_not_cancelled(is_cancelled())?;
            }
            builder.visit_relative((row, column), value.clone());
        }
    }
    ensure_not_cancelled(is_cancelled())?;
    builder.finish(header_mode)
}

pub(super) fn spreadsheet_block_frame_with_cancel<C>(
    plan: &SpreadsheetSnapshotPlan,
    rows: &[Vec<Data>],
    is_cancelled: &C,
) -> Result<DataFrame, String>
where
    C: Fn() -> bool + Sync,
{
    ensure_not_cancelled(is_cancelled())?;
    for (row_index, row) in rows.iter().enumerate() {
        if row_index % CANCELLABLE_READ_BATCH_ROWS == 0 {
            ensure_not_cancelled(is_cancelled())?;
        }
        if row.len() != plan.width {
            return Err("El bloque Excel no coincide con el esquema detectado.".to_owned());
        }
    }
    let mut columns = Vec::with_capacity(plan.width);
    for (column_index, name) in plan.headers.iter().enumerate() {
        ensure_not_cancelled(is_cancelled())?;
        let cells = rows
            .iter()
            .map(|row| &row[column_index])
            .collect::<Vec<_>>();
        columns.push(spreadsheet_cells_to_column(
            name,
            &cells,
            Some(plan.kinds[column_index]),
            is_cancelled,
        )?);
    }
    DataFrame::new(rows.len(), columns)
        .map_err(|error| format!("No se pudo construir un bloque Excel: {error}"))
}

pub(super) fn spreadsheet_block_row_count(
    plan: &SpreadsheetSnapshotPlan,
    block_index: usize,
) -> usize {
    plan.data_rows
        .saturating_sub(block_index.saturating_mul(SPREADSHEET_SNAPSHOT_BLOCK_ROWS))
        .min(SPREADSHEET_SNAPSHOT_BLOCK_ROWS)
}

pub(super) fn empty_spreadsheet_block(
    plan: &SpreadsheetSnapshotPlan,
    block_index: usize,
) -> Vec<Vec<Data>> {
    (0..spreadsheet_block_row_count(plan, block_index))
        .map(|_| vec![Data::Empty; plan.width])
        .collect()
}

pub(super) fn write_spreadsheet_blocks_with_cancel<C, F>(
    destination: &Path,
    plan: &SpreadsheetSnapshotPlan,
    is_cancelled: &C,
    mut fill_block: F,
) -> Result<(), String>
where
    C: Fn() -> bool + Sync,
    F: FnMut(usize, usize) -> Result<Vec<Vec<Data>>, String>,
{
    let schema_frame = spreadsheet_block_frame_with_cancel(plan, &[], is_cancelled)?;
    let mut file = File::create(destination)
        .map_err(|error| format!("No se pudo crear el snapshot Excel: {error}"))?;
    let mut writer = ParquetWriter::new(&mut file)
        .set_parallel(false)
        .batched(schema_frame.schema())
        .map_err(|error| format!("No se pudo preparar el snapshot Excel: {error}"))?;
    let block_count = plan.data_rows.div_ceil(SPREADSHEET_SNAPSHOT_BLOCK_ROWS);
    for block_index in 0..block_count {
        ensure_not_cancelled(is_cancelled())?;
        let expected_rows = spreadsheet_block_row_count(plan, block_index);
        let rows = fill_block(block_index, expected_rows)?;
        if rows.len() != expected_rows {
            return Err("El bloque Excel no contiene el número esperado de filas.".to_owned());
        }
        let frame = spreadsheet_block_frame_with_cancel(plan, &rows, is_cancelled)?;
        writer
            .write_batch(&frame)
            .map_err(|error| format!("No se pudo escribir el snapshot Excel: {error}"))?;
    }
    ensure_not_cancelled(is_cancelled())?;
    writer
        .finish()
        .map_err(|error| format!("No se pudo cerrar el snapshot Excel: {error}"))?;
    file.sync_all()
        .map_err(|error| format!("No se pudo sincronizar el snapshot Excel: {error}"))
}

pub(super) fn write_spreadsheet_range_snapshot_with_cancel<C>(
    range: &Range<Data>,
    header_mode: SpreadsheetHeaderMode,
    destination: &Path,
    is_cancelled: &C,
) -> Result<usize, String>
where
    C: Fn() -> bool + Sync,
{
    ensure_not_cancelled(is_cancelled())?;
    let plan = spreadsheet_snapshot_plan_from_range(range, header_mode, is_cancelled)?;
    write_spreadsheet_blocks_with_cancel(
        destination,
        &plan,
        is_cancelled,
        |block_index, expected_rows| {
            let first_row = plan
                .data_start
                .saturating_add(block_index.saturating_mul(SPREADSHEET_SNAPSHOT_BLOCK_ROWS));
            let mut rows = Vec::with_capacity(expected_rows);
            for row_offset in 0..expected_rows {
                ensure_not_cancelled(is_cancelled())?;
                let mut row = Vec::with_capacity(plan.width);
                for column in 0..plan.width {
                    if column % 256 == 0 {
                        ensure_not_cancelled(is_cancelled())?;
                    }
                    row.push(
                        range
                            .get((first_row + row_offset, column))
                            .cloned()
                            .unwrap_or(Data::Empty),
                    );
                }
                rows.push(row);
            }
            Ok(rows)
        },
    )?;
    ensure_not_cancelled(is_cancelled())?;
    Ok(plan.data_rows)
}

const SPREADSHEET_STREAMING_UNSUPPORTED: &str =
    "El formato Excel no ofrece lectura de celdas secuencial; se usará el fallback compatible.";

pub(super) fn visit_streamed_spreadsheet_cells<F>(
    path: &Path,
    sheet_name: &str,
    mut visit: F,
    is_cancelled: impl Fn() -> bool,
) -> Result<(), String>
where
    F: FnMut(Dimensions, (u32, u32), Data) -> Result<(), String>,
{
    let mut workbook = open_workbook_auto(path)
        .map_err(|error| format!("No se pudo abrir el libro seleccionado: {error}"))?;
    match &mut workbook {
        Sheets::Xlsx(workbook) => {
            let mut reader = workbook
                .worksheet_cells_reader(sheet_name)
                .map_err(|error| format!("No se pudo leer la hoja seleccionada: {error}"))?;
            let dimensions = reader.dimensions();
            loop {
                if is_cancelled() {
                    return Err(OPERATION_CANCELLED_MESSAGE.to_owned());
                }
                let Some(cell) = reader
                    .next_cell()
                    .map_err(|error| format!("No se pudo leer la hoja seleccionada: {error}"))?
                else {
                    break;
                };
                visit(
                    dimensions,
                    cell.get_position(),
                    cell.get_value().clone().into(),
                )?;
            }
        }
        Sheets::Xlsb(workbook) => {
            let mut reader = workbook
                .worksheet_cells_reader(sheet_name)
                .map_err(|error| format!("No se pudo leer la hoja seleccionada: {error}"))?;
            let dimensions = reader.dimensions();
            loop {
                if is_cancelled() {
                    return Err(OPERATION_CANCELLED_MESSAGE.to_owned());
                }
                let Some(cell) = reader
                    .next_cell()
                    .map_err(|error| format!("No se pudo leer la hoja seleccionada: {error}"))?
                else {
                    break;
                };
                visit(
                    dimensions,
                    cell.get_position(),
                    cell.get_value().clone().into(),
                )?;
            }
        }
        Sheets::Xls(_) | Sheets::Ods(_) => return Err(SPREADSHEET_STREAMING_UNSUPPORTED.to_owned()),
    }
    Ok(())
}

pub(super) fn spreadsheet_snapshot_plan_from_stream(
    path: &Path,
    sheet_name: &str,
    header_mode: SpreadsheetHeaderMode,
    is_cancelled: impl Fn() -> bool,
) -> Result<SpreadsheetSnapshotPlan, String> {
    let mut builder = None;
    let mut builder_error = None;
    visit_streamed_spreadsheet_cells(
        path,
        sheet_name,
        |dimensions, position, value| {
            if builder.is_none() && builder_error.is_none() {
                match SpreadsheetSnapshotPlanBuilder::new(dimensions, header_mode) {
                    Ok(value) => builder = Some(value),
                    Err(error) => builder_error = Some(error),
                }
            }
            if builder_error.is_none() {
                if let Some(builder) = builder.as_mut() {
                    builder.visit(position, value);
                }
            }
            Ok(())
        },
        is_cancelled,
    )?;
    if let Some(error) = builder_error {
        return Err(error);
    }
    builder
        .ok_or_else(|| "La hoja seleccionada está vacía.".to_owned())?
        .finish(header_mode)
}

pub(super) fn write_streamed_spreadsheet_cells<C, F>(
    destination: &Path,
    plan: &SpreadsheetSnapshotPlan,
    is_cancelled: C,
    mut next_cell: F,
) -> Result<(), String>
where
    C: Fn() -> bool + Sync,
    F: FnMut() -> Result<Option<((u32, u32), Data)>, String>,
{
    ensure_not_cancelled(is_cancelled())?;
    let schema_frame = spreadsheet_block_frame_with_cancel(plan, &[], &is_cancelled)?;
    let mut file = File::create(destination)
        .map_err(|error| format!("No se pudo crear el snapshot Excel: {error}"))?;
    let mut writer = ParquetWriter::new(&mut file)
        .set_parallel(false)
        .batched(schema_frame.schema())
        .map_err(|error| format!("No se pudo preparar el snapshot Excel: {error}"))?;
    let block_count = plan.data_rows.div_ceil(SPREADSHEET_SNAPSHOT_BLOCK_ROWS);
    let mut current_block = 0_usize;
    let mut rows = empty_spreadsheet_block(plan, current_block);

    loop {
        ensure_not_cancelled(is_cancelled())?;
        let Some((position, value)) = next_cell()? else {
            break;
        };
        let Some(row) = position.0.checked_sub(plan.start.0) else {
            continue;
        };
        let Some(column) = position.1.checked_sub(plan.start.1) else {
            continue;
        };
        let Some(row) = usize::try_from(row).ok() else {
            continue;
        };
        let Some(column) = usize::try_from(column).ok() else {
            continue;
        };
        if row >= plan.height || column >= plan.width || row < plan.data_start {
            continue;
        }
        let data_row = row - plan.data_start;
        if data_row >= plan.data_rows {
            continue;
        }
        let block_index = data_row / SPREADSHEET_SNAPSHOT_BLOCK_ROWS;
        if block_index < current_block {
            return Err("El lector Excel devolvió celdas fuera de orden.".to_owned());
        }
        while current_block < block_index {
            ensure_not_cancelled(is_cancelled())?;
            let frame = spreadsheet_block_frame_with_cancel(plan, &rows, &is_cancelled)?;
            writer
                .write_batch(&frame)
                .map_err(|error| format!("No se pudo escribir el snapshot Excel: {error}"))?;
            current_block += 1;
            rows = empty_spreadsheet_block(plan, current_block);
        }
        if block_index < block_count {
            rows[data_row % SPREADSHEET_SNAPSHOT_BLOCK_ROWS][column] = value;
        }
    }

    while current_block < block_count {
        ensure_not_cancelled(is_cancelled())?;
        let frame = spreadsheet_block_frame_with_cancel(plan, &rows, &is_cancelled)?;
        writer
            .write_batch(&frame)
            .map_err(|error| format!("No se pudo escribir el snapshot Excel: {error}"))?;
        current_block += 1;
        rows = empty_spreadsheet_block(plan, current_block);
    }
    ensure_not_cancelled(is_cancelled())?;
    writer
        .finish()
        .map_err(|error| format!("No se pudo cerrar el snapshot Excel: {error}"))?;
    ensure_not_cancelled(is_cancelled())?;
    file.sync_all()
        .map_err(|error| format!("No se pudo sincronizar el snapshot Excel: {error}"))
}

pub(super) fn read_streamed_spreadsheet_frame<C>(
    path: &Path,
    sheet_name: &str,
    plan: &SpreadsheetSnapshotPlan,
    is_cancelled: C,
) -> Result<DataFrame, String>
where
    C: Fn() -> bool + Sync,
{
    ensure_not_cancelled(is_cancelled())?;
    let mut frame = spreadsheet_block_frame_with_cancel(plan, &[], &is_cancelled)?;
    let block_count = plan.data_rows.div_ceil(SPREADSHEET_SNAPSHOT_BLOCK_ROWS);
    if block_count == 0 {
        return Ok(frame);
    }

    let mut current_block = 0_usize;
    let mut rows = empty_spreadsheet_block(plan, current_block);
    visit_streamed_spreadsheet_cells(
        path,
        sheet_name,
        |_, position, value| {
            let Some(row) = position.0.checked_sub(plan.start.0) else {
                return Ok(());
            };
            let Some(column) = position.1.checked_sub(plan.start.1) else {
                return Ok(());
            };
            let Some(row) = usize::try_from(row).ok() else {
                return Ok(());
            };
            let Some(column) = usize::try_from(column).ok() else {
                return Ok(());
            };
            if row >= plan.height || column >= plan.width || row < plan.data_start {
                return Ok(());
            }
            let data_row = row - plan.data_start;
            if data_row >= plan.data_rows {
                return Ok(());
            }
            let block_index = data_row / SPREADSHEET_SNAPSHOT_BLOCK_ROWS;
            if block_index < current_block {
                return Err("El lector Excel devolvió celdas fuera de orden.".to_owned());
            }
            while current_block < block_index {
                ensure_not_cancelled(is_cancelled())?;
                let block = spreadsheet_block_frame_with_cancel(plan, &rows, &is_cancelled)?;
                frame.vstack_mut(&block).map_err(|error| {
                    format!("No se pudo acumular un bloque de la hoja Excel: {error}")
                })?;
                current_block += 1;
                if current_block < block_count {
                    rows = empty_spreadsheet_block(plan, current_block);
                }
            }
            if current_block < block_count {
                rows[data_row % SPREADSHEET_SNAPSHOT_BLOCK_ROWS][column] = value;
            }
            Ok(())
        },
        &is_cancelled,
    )?;

    while current_block < block_count {
        ensure_not_cancelled(is_cancelled())?;
        let block = spreadsheet_block_frame_with_cancel(plan, &rows, &is_cancelled)?;
        frame
            .vstack_mut(&block)
            .map_err(|error| format!("No se pudo acumular un bloque de la hoja Excel: {error}"))?;
        current_block += 1;
        if current_block < block_count {
            rows = empty_spreadsheet_block(plan, current_block);
        }
    }
    ensure_not_cancelled(is_cancelled())?;
    Ok(frame)
}

pub(super) fn write_streamed_spreadsheet_snapshot(
    path: &Path,
    sheet_name: &str,
    plan: &SpreadsheetSnapshotPlan,
    destination: &Path,
    is_cancelled: impl Fn() -> bool + Sync,
) -> Result<(), String> {
    let mut workbook = open_workbook_auto(path)
        .map_err(|error| format!("No se pudo abrir el libro seleccionado: {error}"))?;
    match &mut workbook {
        Sheets::Xlsx(workbook) => {
            let mut reader = workbook
                .worksheet_cells_reader(sheet_name)
                .map_err(|error| format!("No se pudo leer la hoja seleccionada: {error}"))?;
            write_streamed_spreadsheet_cells(destination, plan, &is_cancelled, || {
                if is_cancelled() {
                    return Err(OPERATION_CANCELLED_MESSAGE.to_owned());
                }
                reader
                    .next_cell()
                    .map_err(|error| format!("No se pudo leer la hoja seleccionada: {error}"))
                    .map(|cell| {
                        cell.map(|cell| (cell.get_position(), cell.get_value().clone().into()))
                    })
            })
        }
        Sheets::Xlsb(workbook) => {
            let mut reader = workbook
                .worksheet_cells_reader(sheet_name)
                .map_err(|error| format!("No se pudo leer la hoja seleccionada: {error}"))?;
            write_streamed_spreadsheet_cells(destination, plan, &is_cancelled, || {
                if is_cancelled() {
                    return Err(OPERATION_CANCELLED_MESSAGE.to_owned());
                }
                reader
                    .next_cell()
                    .map_err(|error| format!("No se pudo leer la hoja seleccionada: {error}"))
                    .map(|cell| {
                        cell.map(|cell| (cell.get_position(), cell.get_value().clone().into()))
                    })
            })
        }
        Sheets::Xls(_) | Sheets::Ods(_) => Err(SPREADSHEET_STREAMING_UNSUPPORTED.to_owned()),
    }
}

#[cfg(test)]
pub(super) fn spreadsheet_range_to_frame(
    range: &Range<Data>,
    header_mode: SpreadsheetHeaderMode,
) -> Result<DataFrame, String> {
    spreadsheet_range_to_frame_with_cancel(range, header_mode, &|| false)
}

/// Filled cells of the first rows of a range, for [`detect_header_row`].
fn range_filled_cells(range: &Range<Data>) -> Vec<usize> {
    (0..range.height().min(HEADER_DETECTION_ROWS))
        .map(|row| {
            (0..range.width())
                .filter(|&column| {
                    range
                        .get((row, column))
                        .is_some_and(|cell| !matches!(cell, Data::Empty))
                })
                .count()
        })
        .collect()
}

/// PROD-19: how many rows above the header the loaders skip, for Cargar to
/// say so. It reads only the first rows; streaming and range paths count
/// from the same origin as their loaders.
pub(super) fn spreadsheet_title_row_count(
    path: &Path,
    sheet_name: &str,
    header_mode: SpreadsheetHeaderMode,
    is_cancelled: impl Fn() -> bool,
) -> Result<usize, String> {
    if header_mode != SpreadsheetHeaderMode::FirstRow {
        return Ok(0);
    }
    const FIRST_ROWS_READ: &str = "first rows read";
    let mut filled_cells = vec![0_usize; HEADER_DETECTION_ROWS];
    let streamed = visit_streamed_spreadsheet_cells(
        path,
        sheet_name,
        |dimensions, position, value| {
            let row = position.0.saturating_sub(dimensions.start.0) as usize;
            if row >= HEADER_DETECTION_ROWS {
                return Err(FIRST_ROWS_READ.to_owned());
            }
            if position.1 >= dimensions.start.1 && !matches!(value, Data::Empty) {
                filled_cells[row] += 1;
            }
            Ok(())
        },
        is_cancelled,
    );
    match streamed {
        Ok(()) => Ok(detect_header_row(&filled_cells)),
        Err(error) if error == FIRST_ROWS_READ => Ok(detect_header_row(&filled_cells)),
        Err(error) if error == SPREADSHEET_STREAMING_UNSUPPORTED => {
            let mut workbook = open_workbook_auto(path)
                .map_err(|error| format!("No se pudo abrir el libro seleccionado: {error}"))?;
            let range = workbook
                .worksheet_range(sheet_name)
                .map_err(|error| format!("No se pudo leer la hoja seleccionada: {error}"))?;
            Ok(detect_header_row(&range_filled_cells(&range)))
        }
        Err(error) => Err(error),
    }
}

pub(super) fn spreadsheet_range_to_frame_with_cancel<C>(
    range: &Range<Data>,
    header_mode: SpreadsheetHeaderMode,
    is_cancelled: &C,
) -> Result<DataFrame, String>
where
    C: Fn() -> bool + Sync,
{
    ensure_not_cancelled(is_cancelled())?;
    if range.is_empty() {
        return Err("La hoja seleccionada está vacía.".to_owned());
    }
    let width = range.width();
    let height = range.height();
    let header_row = match header_mode {
        SpreadsheetHeaderMode::FirstRow => detect_header_row(&range_filled_cells(range)),
        SpreadsheetHeaderMode::Generated => 0,
    };
    let data_start = match header_mode {
        SpreadsheetHeaderMode::FirstRow => header_row + 1,
        SpreadsheetHeaderMode::Generated => 0,
    };
    let headers = match header_mode {
        SpreadsheetHeaderMode::FirstRow => {
            let mut headers = Vec::with_capacity(width);
            for column in 0..width {
                if column % 256 == 0 {
                    ensure_not_cancelled(is_cancelled())?;
                }
                headers.push(
                    range
                        .get((header_row, column))
                        .map(ToString::to_string)
                        .unwrap_or_default(),
                );
            }
            unique_spreadsheet_headers(headers)
        }
        SpreadsheetHeaderMode::Generated => {
            (1..=width).map(|index| format!("column_{index}")).collect()
        }
    };
    let mut columns = Vec::with_capacity(width);
    for (column_index, name) in headers.iter().enumerate() {
        ensure_not_cancelled(is_cancelled())?;
        let mut cells = Vec::with_capacity(height.saturating_sub(data_start));
        for (row_index, row) in (data_start..height).enumerate() {
            if row_index % CANCELLABLE_READ_BATCH_ROWS == 0 {
                ensure_not_cancelled(is_cancelled())?;
            }
            cells.push(range.get((row, column_index)).expect("rango rectangular"));
        }
        columns.push(spreadsheet_cells_to_column(
            name,
            &cells,
            None,
            is_cancelled,
        )?);
    }
    ensure_not_cancelled(is_cancelled())?;
    DataFrame::new(height.saturating_sub(data_start), columns)
        .map_err(|error| format!("No se pudo construir el dataset desde la hoja: {error}"))
}

#[cfg(test)]
pub(super) fn load_spreadsheet_sheet(
    path: &Path,
    sheet_name: &str,
    header_mode: SpreadsheetHeaderMode,
) -> Result<DataFrame, String> {
    load_spreadsheet_sheet_with_cancel(path, sheet_name, header_mode, || false)
}

pub(super) fn load_spreadsheet_sheet_with_cancel<C>(
    path: &Path,
    sheet_name: &str,
    header_mode: SpreadsheetHeaderMode,
    is_cancelled: C,
) -> Result<DataFrame, String>
where
    C: Fn() -> bool + Sync,
{
    ensure_not_cancelled(is_cancelled())?;
    match spreadsheet_snapshot_plan_from_stream(path, sheet_name, header_mode, &is_cancelled) {
        Ok(plan) => read_streamed_spreadsheet_frame(path, sheet_name, &plan, is_cancelled),
        Err(error) if error == SPREADSHEET_STREAMING_UNSUPPORTED => {
            ensure_not_cancelled(is_cancelled())?;
            let frame = load_spreadsheet_sheet_range(path, sheet_name, header_mode, &is_cancelled)?;
            ensure_not_cancelled(is_cancelled())?;
            Ok(frame)
        }
        Err(error) => Err(error),
    }
}

pub(super) fn load_spreadsheet_sheet_range<C>(
    path: &Path,
    sheet_name: &str,
    header_mode: SpreadsheetHeaderMode,
    is_cancelled: &C,
) -> Result<DataFrame, String>
where
    C: Fn() -> bool + Sync,
{
    ensure_not_cancelled(is_cancelled())?;
    let mut workbook = open_workbook_auto(path)
        .map_err(|error| format!("No se pudo abrir el libro seleccionado: {error}"))?;
    if !workbook.sheet_names().iter().any(|name| name == sheet_name) {
        return Err("La hoja seleccionada ya no está disponible en el libro.".to_owned());
    }
    let range = workbook
        .worksheet_range(sheet_name)
        .map_err(|error| format!("No se pudo leer la hoja seleccionada: {error}"))?;
    ensure_not_cancelled(is_cancelled())?;
    spreadsheet_range_to_frame_with_cancel(&range, header_mode, is_cancelled)
}
