//! REN-14: the part of the Preparar proposal that follows Rust's own rules
//! (numbers and dates typed only when every value converts, gaps filled with
//! the median or the most frequent value), applied to a large file block by
//! block after DuckDB wrote the rest of the corrections. The rules are those
//! of `cast_fully_numeric_columns`, `cast_fully_dated_columns` and
//! `impute_missing_values_in_columns`, in that order.

use super::column_typing::{fill_column, typed_date_column, Fill};
use super::date_inference::{parse_ordered_date, DateOrder};
use super::*;
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};

/// What the stage converts (names as in the input): the text columns to type
/// as numbers, the date columns and the columns whose gaps it fills.
pub(super) struct SourceStagePlan {
    pub(super) numbers: Vec<String>,
    pub(super) dates: Vec<DateColumnPlan>,
    pub(super) impute: Option<Vec<String>>,
}

#[derive(Default)]
pub(super) struct SourceStageOutcome {
    pub(super) typed_column_count: usize,
    pub(super) dated_column_count: usize,
    pub(super) imputed_cell_count: usize,
}

fn duckdb_cast_target(kind: NumericCastKind) -> crate::duckdb_query::NumericCastTarget {
    match kind {
        NumericCastKind::Integer => crate::duckdb_query::NumericCastTarget::Integer,
        NumericCastKind::IntegerFromDecimal => {
            crate::duckdb_query::NumericCastTarget::IntegerFromDecimal
        }
        NumericCastKind::Decimal => crate::duckdb_query::NumericCastTarget::Decimal,
    }
}

struct DateTarget {
    name: String,
    order: DateOrder,
    with_time: bool,
}

pub(super) const SOURCE_STAGE_BLOCK_ROWS: usize = 4 * LOCAL_QUERY_BLOCK_ROWS;

/// Returned by the visit of a check pass to end the read once every column
/// has its verdict; the pass then ends as a success.
const SOURCE_CHECKS_DECIDED: &str = "Todas las columnas tienen veredicto.";

/// What a pass over a block asks of a candidate column.
enum ColumnCheck {
    /// The column types as numbers when every value converts.
    Number(NumericCastCheck),
    /// Every non-null value read with `order`, and whether one has a time.
    Date {
        order: DateOrder,
        present: bool,
        with_time: bool,
        fits: bool,
    },
}

/// A text column whose verdict (number or date) the check pass finds.
struct CandidateCheck {
    name: String,
    /// Position of `name` among the names the pass reads.
    slot: usize,
    check: ColumnCheck,
}

impl CandidateCheck {
    fn number(name: &str, slot: usize) -> Self {
        Self {
            name: name.to_owned(),
            slot,
            check: ColumnCheck::Number(NumericCastCheck::new()),
        }
    }

    fn date(name: &str, slot: usize, order: DateOrder) -> Self {
        Self {
            name: name.to_owned(),
            slot,
            check: ColumnCheck::Date {
                order,
                present: false,
                with_time: false,
                fits: true,
            },
        }
    }

    /// True once no later value can change the verdict.
    fn is_decided(&self) -> bool {
        match &self.check {
            ColumnCheck::Number(check) => check.is_rejected(),
            ColumnCheck::Date { fits, .. } => !fits,
        }
    }

    fn observe(&mut self, values: &StringChunked) {
        match &mut self.check {
            ColumnCheck::Number(check) => {
                for value in values.iter().flatten() {
                    check.observe(value);
                    if check.is_rejected() {
                        break;
                    }
                }
            }
            ColumnCheck::Date {
                order,
                present,
                with_time,
                fits,
            } => {
                for value in values.iter().flatten() {
                    *present = true;
                    *with_time |= value.contains(':');
                    if parse_ordered_date(value, *order).is_none() {
                        *fits = false;
                        break;
                    }
                }
            }
        }
    }

    /// The numeric kind the column takes, if it is a number candidate that
    /// qualifies.
    fn numeric_kind(&self) -> Option<NumericCastKind> {
        match &self.check {
            ColumnCheck::Number(check) => check.kind(),
            ColumnCheck::Date { .. } => None,
        }
    }

    /// The date target of the column, if it is a date candidate that
    /// qualifies.
    fn date_target(&self) -> Option<DateTarget> {
        match &self.check {
            ColumnCheck::Date {
                order,
                present,
                with_time,
                fits,
            } => (*fits && *present).then(|| DateTarget {
                name: self.name.clone(),
                order: *order,
                with_time: *with_time,
            }),
            ColumnCheck::Number(_) => None,
        }
    }
}

/// The position of `name` in `names`, adding it when it is new.
fn slot_of(names: &mut Vec<String>, name: &str) -> usize {
    if let Some(slot) = names.iter().position(|seen| seen == name) {
        slot
    } else {
        names.push(name.to_owned());
        names.len() - 1
    }
}

/// AUD-036: gives every candidate its verdict in one read of the file. Each
/// block holds only the columns still undecided; they are checked in parallel,
/// one task per column, and the read ends when all are decided. The verdict of
/// a column does not depend on the others or on the order of the tasks.
fn check_candidate_columns<C>(
    input: &Path,
    row_count: usize,
    checks: &mut [CandidateCheck],
    names: &[String],
    is_cancelled: &C,
) -> Result<(), String>
where
    C: Fn() -> bool + Sync,
{
    if checks.is_empty() {
        return Ok(());
    }
    let settled = names
        .iter()
        .map(|_| AtomicBool::new(false))
        .collect::<Vec<_>>();
    let walked = for_each_parquet_wanted_columns_block_prefetched(
        input,
        row_count,
        names,
        SOURCE_STAGE_BLOCK_ROWS,
        &|index| !settled[index].load(AtomicOrdering::Acquire),
        |_, block| {
            ensure_not_cancelled(is_cancelled())?;
            let results = checks
                .par_iter_mut()
                .map(|candidate| {
                    if candidate.is_decided() {
                        return Ok(());
                    }
                    let values = block
                        .column(&candidate.name)
                        .and_then(|column| column.str())
                        .map_err(|error| {
                            format!("No se pudo leer la columna '{}': {error}", candidate.name)
                        })?;
                    candidate.observe(values);
                    Ok(())
                })
                .collect::<Vec<Result<(), String>>>();
            for result in results {
                result?;
            }
            // A column nobody needs any more is left out of the next reads.
            for (slot, flag) in settled.iter().enumerate() {
                if checks
                    .iter()
                    .all(|candidate| candidate.slot != slot || candidate.is_decided())
                {
                    flag.store(true, AtomicOrdering::Release);
                }
            }
            if checks.iter().all(CandidateCheck::is_decided) {
                return Err(SOURCE_CHECKS_DECIDED.to_owned());
            }
            Ok(())
        },
    );
    match walked {
        Err(error) if error == SOURCE_CHECKS_DECIDED => Ok(()),
        other => other,
    }
}

/// How often a non-blank text appears in a column, and the gaps it has.
struct TextFillCount {
    name: String,
    /// Each text with how many times it appears and where it first did.
    counts: HashMap<String, (usize, usize)>,
    gaps: usize,
}

impl TextFillCount {
    fn observe(&mut self, start: usize, values: &StringChunked) {
        for (offset, value) in values.iter().enumerate() {
            match value {
                None => self.gaps += 1,
                Some(value) if !value.trim().is_empty() => {
                    if let Some(entry) = self.counts.get_mut(value) {
                        entry.0 += 1;
                    } else {
                        self.counts.insert(value.to_owned(), (1, start + offset));
                    }
                }
                Some(_) => {}
            }
        }
    }

    /// The most frequent non-blank text, the first seen on a tie, when it is
    /// there at least twice and the column has a gap to fill.
    fn fill(self) -> Option<String> {
        if self.gaps == 0 {
            return None;
        }
        self.counts
            .into_iter()
            .filter(|(_, (count, _))| *count >= 2)
            .max_by(
                |(_, (left_count, left_first)), (_, (right_count, right_first))| {
                    left_count
                        .cmp(right_count)
                        .then(right_first.cmp(left_first))
                },
            )
            .map(|(value, _)| value)
    }
}

/// The text to fill each of `names` with (see [`TextFillCount::fill`]), in
/// the order of `names`, all counted in one read of the file, one task per
/// column.
fn text_fills<C>(
    input: &Path,
    row_count: usize,
    names: &[&str],
    is_cancelled: &C,
) -> Result<Vec<Option<String>>, String>
where
    C: Fn() -> bool + Sync,
{
    if names.is_empty() {
        return Ok(Vec::new());
    }
    let mut distinct = Vec::new();
    for name in names {
        slot_of(&mut distinct, name);
    }
    let mut counts = names
        .iter()
        .map(|name| TextFillCount {
            name: (*name).to_owned(),
            counts: HashMap::new(),
            gaps: 0,
        })
        .collect::<Vec<_>>();
    for_each_parquet_wanted_columns_block_prefetched(
        input,
        row_count,
        &distinct,
        SOURCE_STAGE_BLOCK_ROWS,
        &|_| true,
        |start, block| {
            ensure_not_cancelled(is_cancelled())?;
            let results = counts
                .par_iter_mut()
                .map(|count| {
                    let values = block
                        .column(&count.name)
                        .and_then(|column| column.str())
                        .map_err(|error| {
                            format!("No se pudo leer la columna '{}': {error}", count.name)
                        })?;
                    count.observe(start, values);
                    Ok(())
                })
                .collect::<Vec<Result<(), String>>>();
            for result in results {
                result?;
            }
            Ok(())
        },
    )?;
    Ok(counts.into_iter().map(TextFillCount::fill).collect())
}

fn convert_block(
    block: &DataFrame,
    numbers: &[(String, NumericCastKind)],
    dates: &[DateTarget],
    fills: &[(String, Fill<'static>)],
) -> Result<(DataFrame, usize), String> {
    let mut converted = block.clone();
    for (name, kind) in numbers {
        let values = block
            .column(name)
            .and_then(|column| column.str().cloned())
            .map_err(|error| format!("No se pudo leer la columna '{name}': {error}"))?;
        converted
            .replace(name, numeric_cast_column(name, &values, *kind))
            .map_err(|error| format!("No se pudo convertir la columna '{name}': {error}"))?;
    }
    // Fills read the typed columns.
    let block = &converted.clone();
    for target in dates {
        let name = target.name.as_str();
        let values = block
            .column(name)
            .and_then(|column| column.str().cloned())
            .map_err(|error| format!("No se pudo leer la columna '{name}': {error}"))?;
        let column =
            typed_date_column(name, &values, target.order, target.with_time).map_err(|error| {
                format!("No se pudo convertir la columna '{name}' a fecha: {error}")
            })?;
        converted.replace(name, column).map_err(|error| {
            format!("No se pudo convertir la columna '{name}' a fecha: {error}")
        })?;
    }
    let mut filled = 0usize;
    for (name, fill) in fills {
        let column = block
            .column(name)
            .map_err(|error| format!("No se pudo leer la columna '{name}': {error}"))?;
        filled += column.null_count();
        let column = fill_column(column, fill)?;
        converted
            .replace(name, column)
            .map_err(|error| format!("No se pudo imputar la columna '{name}': {error}"))?;
    }
    Ok((converted, filled))
}

/// Writes `input` with the dates and fills of `plan` to `output`. Returns
/// `None`, writing nothing, when no column qualifies.
pub(super) fn source_date_and_impute_stage<C>(
    input: &Path,
    output: &Path,
    plan: &SourceStagePlan,
    is_cancelled: &C,
    progress: &mut dyn FnMut(f64),
) -> Result<Option<SourceStageOutcome>, String>
where
    C: Fn() -> bool + Sync + Send + Clone + 'static,
{
    ensure_not_cancelled(is_cancelled())?;
    let schema = read_parquet_schema_frame(input)?;
    let row_count = crate::duckdb_query::count_file_rows(
        input,
        crate::duckdb_query::DuckDbFileFormat::Parquet,
        || false,
    )?;
    // AUD-036: numbers and dates are checked together, in one read.
    let mut names = Vec::new();
    let mut checks = Vec::new();
    for name in &plan.numbers {
        if name == "_cambios"
            || privacy_signal(name) == Some("identifier")
            || !schema
                .column(name)
                .is_ok_and(|column| column.dtype() == &DataType::String)
        {
            continue;
        }
        let slot = slot_of(&mut names, name);
        checks.push(CandidateCheck::number(name, slot));
    }
    for date in &plan.dates {
        let order = DateOrder::from_label(&date.order)
            .ok_or_else(|| format!("Orden de fecha no válido: '{}'.", date.order))?;
        if schema
            .column(&date.column)
            .is_ok_and(|column| column.dtype() == &DataType::String)
        {
            let slot = slot_of(&mut names, &date.column);
            checks.push(CandidateCheck::date(&date.column, slot, order));
        }
    }
    check_candidate_columns(input, row_count, &mut checks, &names, is_cancelled)?;
    let numbers = checks
        .iter()
        .filter_map(|check| Some((check.name.clone(), check.numeric_kind()?)))
        .collect::<Vec<_>>();
    let dates = checks
        .iter()
        .filter_map(CandidateCheck::date_target)
        .collect::<Vec<_>>();
    let mut fills = Vec::new();
    // The columns filled with a median: one DuckDB query reads them all.
    let mut medians = Vec::new();
    // The text columns filled with the most frequent value: one read counts
    // them all.
    let mut texts = Vec::new();
    for name in plan.impute.iter().flatten() {
        if name == "_cambios" || dates.iter().any(|target| &target.name == name) {
            continue;
        }
        let Ok(column) = schema.column(name) else {
            continue;
        };
        // A column this stage types fills with the median of its numbers.
        let typed = numbers
            .iter()
            .find(|(typed, _)| typed == name)
            .map(|(_, kind)| *kind);
        let data_type = match typed {
            Some(NumericCastKind::Decimal) => DataType::Float64,
            Some(_) => DataType::Int64,
            None => column.dtype().clone(),
        };
        let cast = typed.map(duckdb_cast_target);
        if data_type == DataType::String {
            texts.push(name.as_str());
        } else if data_type == DataType::Int64 || data_type.is_primitive_numeric() {
            medians.push((
                name.as_str(),
                crate::duckdb_query::LowerMedianRequest {
                    column: name,
                    as_double: data_type != DataType::Int64,
                    cast,
                },
            ));
        }
    }
    for (name, value) in texts
        .iter()
        .zip(text_fills(input, row_count, &texts, is_cancelled)?)
    {
        if let Some(value) = value {
            fills.push(((*name).to_owned(), Fill::Text(value.into())));
        }
    }
    let requests = medians
        .iter()
        .map(|(_, request)| crate::duckdb_query::LowerMedianRequest {
            column: request.column,
            as_double: request.as_double,
            cast: request.cast,
        })
        .collect::<Vec<_>>();
    let found = crate::duckdb_query::parquet_lower_medians(input, &requests, is_cancelled.clone())?;
    for ((name, _), median) in medians.iter().zip(found) {
        match median {
            Some(crate::duckdb_query::LowerMedian::Integer(value)) => {
                fills.push(((*name).to_owned(), Fill::Integer(value)));
            }
            Some(crate::duckdb_query::LowerMedian::Number(value)) => {
                fills.push(((*name).to_owned(), Fill::Number(value)));
            }
            None => {}
        }
    }
    if numbers.is_empty() && dates.is_empty() && fills.is_empty() {
        return Ok(None);
    }

    let (empty, _) = convert_block(&schema, &numbers, &dates, &fills)?;
    let file = File::create(output)
        .map_err(|error| format!("No se pudo preparar la versión corregida: {error}"))?;
    let mut writer = ParquetWriter::new(file)
        .batched(empty.schema())
        .map_err(|error| format!("No se pudo preparar la versión corregida: {error}"))?;
    let mut imputed_cell_count = 0usize;
    for_each_parquet_block_prefetched(
        input,
        row_count,
        SOURCE_STAGE_BLOCK_ROWS,
        |start, block| {
            ensure_not_cancelled(is_cancelled())?;
            progress(start as f64 / row_count.max(1) as f64);
            let (mut converted, filled) = convert_block(block, &numbers, &dates, &fills)?;
            imputed_cell_count += filled;
            converted.align_chunks_par();
            writer
                .write_batch(&converted)
                .map_err(|error| format!("No se pudo escribir la versión corregida: {error}"))
        },
    )?;
    writer
        .finish()
        .map_err(|error| format!("No se pudo cerrar la versión corregida: {error}"))?;
    Ok(Some(SourceStageOutcome {
        typed_column_count: numbers.len(),
        dated_column_count: dates.len(),
        imputed_cell_count,
    }))
}
