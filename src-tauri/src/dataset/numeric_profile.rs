use super::*;

pub(super) fn numeric_value(value: AnyValue<'_>) -> Option<f64> {
    let value = match value {
        AnyValue::UInt8(value) => Some(value.into()),
        AnyValue::UInt16(value) => Some(value.into()),
        AnyValue::UInt32(value) => Some(value.into()),
        AnyValue::UInt64(value) => Some(value as f64),
        AnyValue::UInt128(value) => Some(value as f64),
        AnyValue::Int8(value) => Some(value.into()),
        AnyValue::Int16(value) => Some(value.into()),
        AnyValue::Int32(value) => Some(value.into()),
        AnyValue::Int64(value) => Some(value as f64),
        AnyValue::Int128(value) => Some(value as f64),
        AnyValue::Float32(value) => Some(value.into()),
        AnyValue::Float64(value) => Some(value),
        _ => None,
    };
    value.filter(|number| number.is_finite())
}

pub(super) struct NumericStatistics {
    pub(super) minimum: Option<f64>,
    pub(super) maximum: Option<f64>,
    pub(super) mean: Option<f64>,
    pub(super) standard_deviation: Option<f64>,
    pub(super) first_quartile: Option<f64>,
    pub(super) median: Option<f64>,
    pub(super) third_quartile: Option<f64>,
    pub(super) outlier_count: usize,
    pub(super) histogram: Option<Vec<HistogramBucket>>,
}

fn has_identifier_leading_zero(value: &str) -> bool {
    let unsigned = value
        .strip_prefix('+')
        .or_else(|| value.strip_prefix('-'))
        .unwrap_or(value);
    let mut characters = unsigned.chars();
    matches!((characters.next(), characters.next()), (Some('0'), Some(next)) if next.is_ascii_digit())
}

pub(super) fn semantic_numeric_value(value: &str) -> Option<f64> {
    let value = value.trim();
    if value.is_empty() || has_identifier_leading_zero(value) {
        return None;
    }
    if !value.contains('.') && !value.contains('e') && !value.contains('E') {
        let integer = value.parse::<i128>().ok()?;
        if integer.unsigned_abs() > (1_u128 << 53) {
            return None;
        }
    }
    value
        .parse::<f64>()
        .ok()
        .filter(|number| number.is_finite())
}

fn stable_histogram_boundary(value: f64) -> f64 {
    if value == 0.0 || !value.is_finite() {
        return value;
    }
    // Reparse a fixed-precision scientific representation so the value is
    // identical before and after serde_json serialization.
    format!("{value:.15e}").parse().unwrap_or(value)
}

fn numeric_histogram(values: &Float64Chunked, minimum: f64, maximum: f64) -> Vec<HistogramBucket> {
    if minimum == maximum {
        let count = values
            .iter()
            .flatten()
            .filter(|value| value.is_finite())
            .count();
        return vec![HistogramBucket {
            lower: minimum,
            upper: maximum,
            count,
        }];
    }

    // Normalize before calculating positions so ranges such as -1e308..1e308 do not
    // overflow when the difference between the endpoints is computed.
    let scale = minimum.abs().max(maximum.abs());
    let normalized_minimum = minimum / scale;
    let normalized_maximum = maximum / scale;
    let normalized_span = normalized_maximum - normalized_minimum;
    let mut counts = vec![0usize; NUMERIC_HISTOGRAM_BUCKETS];

    for value in values.iter().flatten().filter(|value| value.is_finite()) {
        let position = ((value / scale - normalized_minimum) / normalized_span).clamp(0.0, 1.0);
        let index = ((position * NUMERIC_HISTOGRAM_BUCKETS as f64).floor() as usize)
            .min(NUMERIC_HISTOGRAM_BUCKETS - 1);
        counts[index] += 1;
    }

    counts
        .into_iter()
        .enumerate()
        .map(|(index, count)| {
            let lower_fraction = index as f64 / NUMERIC_HISTOGRAM_BUCKETS as f64;
            let upper_fraction = (index + 1) as f64 / NUMERIC_HISTOGRAM_BUCKETS as f64;
            HistogramBucket {
                lower: if index == 0 {
                    minimum
                } else {
                    stable_histogram_boundary(
                        minimum * (1.0 - lower_fraction) + maximum * lower_fraction,
                    )
                },
                upper: if index + 1 == NUMERIC_HISTOGRAM_BUCKETS {
                    maximum
                } else {
                    stable_histogram_boundary(
                        minimum * (1.0 - upper_fraction) + maximum * upper_fraction,
                    )
                },
                count,
            }
        })
        .collect()
}

const SOURCE_PROFILE_NUMERIC_RUN_VALUES: usize = 262_144;

pub(super) struct NumericRunWriter {
    directory: tempfile::TempDir,
    pending: Vec<f64>,
    runs: Vec<PathBuf>,
}

impl NumericRunWriter {
    pub(super) fn new() -> Result<Self, String> {
        let directory = tempfile::tempdir().map_err(|error| {
            format!("No se pudo preparar el almacenamiento temporal numérico: {error}")
        })?;
        Ok(Self {
            directory,
            pending: Vec::with_capacity(SOURCE_PROFILE_NUMERIC_RUN_VALUES),
            runs: Vec::new(),
        })
    }

    pub(super) fn push(&mut self, value: f64) -> Result<(), String> {
        if !value.is_finite() {
            return Ok(());
        }
        self.pending.push(value);
        if self.pending.len() >= SOURCE_PROFILE_NUMERIC_RUN_VALUES {
            self.flush_pending()?;
        }
        Ok(())
    }

    fn flush_pending(&mut self) -> Result<(), String> {
        if self.pending.is_empty() {
            return Ok(());
        }
        self.pending.sort_unstable_by(f64::total_cmp);
        let path = self
            .directory
            .path()
            .join(format!("numeric-run-{:06}.bin", self.runs.len()));
        let file = File::create(&path)
            .map_err(|error| format!("No se pudo crear una corrida numérica temporal: {error}"))?;
        let mut writer = BufWriter::with_capacity(64 * 1024, file);
        for value in self.pending.drain(..) {
            writer.write_all(&value.to_le_bytes()).map_err(|error| {
                format!("No se pudo guardar una corrida numérica temporal: {error}")
            })?;
        }
        writer.flush().map_err(|error| {
            format!("No se pudo sincronizar una corrida numérica temporal: {error}")
        })?;
        self.runs.push(path);
        Ok(())
    }

    pub(super) fn finish(mut self) -> Result<NumericRuns, String> {
        self.flush_pending()?;
        Ok(NumericRuns {
            _directory: self.directory,
            paths: self.runs,
        })
    }
}

pub(super) struct NumericRuns {
    _directory: tempfile::TempDir,
    paths: Vec<PathBuf>,
}

struct NumericRunCursor {
    reader: BufReader<File>,
    current: Option<f64>,
}

struct NumericHeapEntry {
    value: f64,
    run_index: usize,
}

impl PartialEq for NumericHeapEntry {
    fn eq(&self, other: &Self) -> bool {
        self.value.total_cmp(&other.value) == std::cmp::Ordering::Equal
            && self.run_index == other.run_index
    }
}

impl Eq for NumericHeapEntry {}

impl PartialOrd for NumericHeapEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for NumericHeapEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other
            .value
            .total_cmp(&self.value)
            .then_with(|| other.run_index.cmp(&self.run_index))
    }
}

fn read_numeric_run_value(reader: &mut BufReader<File>) -> Result<Option<f64>, String> {
    let mut bytes = [0_u8; std::mem::size_of::<f64>()];
    match reader.read_exact(&mut bytes) {
        Ok(()) => Ok(Some(f64::from_le_bytes(bytes))),
        Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => Ok(None),
        Err(error) => Err(format!(
            "No se pudo leer una corrida numérica temporal: {error}"
        )),
    }
}

struct SortedNumericRuns {
    cursors: Vec<NumericRunCursor>,
    heap: BinaryHeap<NumericHeapEntry>,
}

impl SortedNumericRuns {
    fn new(runs: &NumericRuns) -> Result<Self, String> {
        let mut cursors = Vec::with_capacity(runs.paths.len());
        let mut heap = BinaryHeap::with_capacity(runs.paths.len());
        for (run_index, path) in runs.paths.iter().enumerate() {
            let file = File::open(path).map_err(|error| {
                format!("No se pudo abrir una corrida numérica temporal: {error}")
            })?;
            let mut cursor = NumericRunCursor {
                reader: BufReader::with_capacity(64 * 1024, file),
                current: None,
            };
            cursor.current = read_numeric_run_value(&mut cursor.reader)?;
            if let Some(value) = cursor.current {
                heap.push(NumericHeapEntry { value, run_index });
            }
            cursors.push(cursor);
        }
        Ok(Self { cursors, heap })
    }

    fn next(&mut self) -> Result<Option<f64>, String> {
        let Some(entry) = self.heap.pop() else {
            return Ok(None);
        };
        let cursor = self
            .cursors
            .get_mut(entry.run_index)
            .ok_or_else(|| "La corrida numérica temporal quedó desalineada.".to_owned())?;
        cursor.current = read_numeric_run_value(&mut cursor.reader)?;
        if let Some(value) = cursor.current {
            self.heap.push(NumericHeapEntry {
                value,
                run_index: entry.run_index,
            });
        }
        Ok(Some(entry.value))
    }
}

fn source_numeric_quantiles(
    runs: &NumericRuns,
    value_count: usize,
) -> Result<[Option<f64>; 3], String> {
    if value_count == 0 {
        return Ok([None; 3]);
    }
    let positions =
        [0.25_f64, 0.5, 0.75].map(|quantile| value_count.saturating_sub(1) as f64 * quantile);
    let lower = positions.map(f64::floor).map(|value| value as usize);
    let upper = positions.map(f64::ceil).map(|value| value as usize);
    let fractions = positions.map(|value| value.fract());
    let mut lower_values = [None; 3];
    let mut upper_values = [None; 3];
    let mut sorted = SortedNumericRuns::new(runs)?;
    let final_index = upper.into_iter().max().unwrap_or(0);
    for index in 0..=final_index {
        let value = sorted
            .next()?
            .ok_or_else(|| "La corrida numérica temporal quedó incompleta.".to_owned())?;
        for quantile_index in 0..3 {
            if index == lower[quantile_index] {
                lower_values[quantile_index] = Some(value);
            }
            if index == upper[quantile_index] {
                upper_values[quantile_index] = Some(value);
            }
        }
    }
    Ok(std::array::from_fn(|index| {
        let first = lower_values[index]?;
        let second = upper_values[index].unwrap_or(first);
        Some(first + (second - first) * fractions[index])
    }))
}

fn source_numeric_histogram_and_outliers(
    runs: &NumericRuns,
    value_count: usize,
    minimum: f64,
    maximum: f64,
    first_quartile: Option<f64>,
    third_quartile: Option<f64>,
) -> Result<(Vec<HistogramBucket>, usize), String> {
    let mut counts = vec![0_usize; NUMERIC_HISTOGRAM_BUCKETS];
    let (scale, normalized_minimum, normalized_span) = if minimum == maximum {
        (1.0, 0.0, 0.0)
    } else {
        let scale = minimum.abs().max(maximum.abs());
        let normalized_minimum = minimum / scale;
        let normalized_maximum = maximum / scale;
        (
            scale,
            normalized_minimum,
            normalized_maximum - normalized_minimum,
        )
    };
    let outlier_bounds = if value_count < 4 {
        None
    } else {
        let first_quartile = first_quartile
            .ok_or_else(|| "No se pudo calcular el primer cuartil numérico.".to_owned())?;
        let third_quartile = third_quartile
            .ok_or_else(|| "No se pudo calcular el tercer cuartil numérico.".to_owned())?;
        let interquartile_range = third_quartile - first_quartile;
        Some((
            first_quartile - 1.5 * interquartile_range,
            third_quartile + 1.5 * interquartile_range,
        ))
    };
    let mut outlier_count = 0;
    let mut sorted = SortedNumericRuns::new(runs)?;
    while let Some(value) = sorted.next()? {
        let bucket = if minimum == maximum {
            0
        } else {
            let position = ((value / scale - normalized_minimum) / normalized_span).clamp(0.0, 1.0);
            ((position * NUMERIC_HISTOGRAM_BUCKETS as f64).floor() as usize)
                .min(NUMERIC_HISTOGRAM_BUCKETS - 1)
        };
        counts[bucket] = counts[bucket].saturating_add(1);
        if let Some((lower, upper)) = outlier_bounds {
            outlier_count += usize::from(value < lower || value > upper);
        }
    }
    let histogram = if minimum == maximum {
        vec![HistogramBucket {
            lower: minimum,
            upper: maximum,
            count: counts[0],
        }]
    } else {
        counts
            .into_iter()
            .enumerate()
            .map(|(index, count)| {
                let lower_fraction = index as f64 / NUMERIC_HISTOGRAM_BUCKETS as f64;
                let upper_fraction = (index + 1) as f64 / NUMERIC_HISTOGRAM_BUCKETS as f64;
                HistogramBucket {
                    lower: if index == 0 {
                        minimum
                    } else {
                        stable_histogram_boundary(
                            minimum * (1.0 - lower_fraction) + maximum * lower_fraction,
                        )
                    },
                    upper: if index + 1 == NUMERIC_HISTOGRAM_BUCKETS {
                        maximum
                    } else {
                        stable_histogram_boundary(
                            minimum * (1.0 - upper_fraction) + maximum * upper_fraction,
                        )
                    },
                    count,
                }
            })
            .collect()
    };
    Ok((histogram, outlier_count))
}

pub(super) fn source_numeric_statistics(
    runs: &NumericRuns,
    value_count: usize,
    minimum: Option<f64>,
    maximum: Option<f64>,
    mean: Option<f64>,
    m2: f64,
) -> Result<Option<NumericStatistics>, String> {
    let (Some(minimum), Some(maximum), Some(mean)) = (minimum, maximum, mean) else {
        return Ok(None);
    };
    let quantiles = source_numeric_quantiles(runs, value_count)?;
    let (histogram, outlier_count) = source_numeric_histogram_and_outliers(
        runs,
        value_count,
        minimum,
        maximum,
        quantiles[0],
        quantiles[2],
    )?;
    Ok(Some(NumericStatistics {
        minimum: Some(minimum),
        maximum: Some(maximum),
        mean: Some(mean),
        standard_deviation: (value_count >= 2).then(|| (m2 / (value_count - 1) as f64).sqrt()),
        first_quartile: quantiles[0],
        median: quantiles[1],
        third_quartile: quantiles[2],
        outlier_count,
        histogram: Some(histogram),
    }))
}

pub(super) fn numeric_statistics(
    column: &Column,
    text_profile: Option<&TextStatistics>,
) -> Result<Option<NumericStatistics>, String> {
    let source = if column.dtype().is_primitive_numeric() {
        column.clone()
    } else if column.dtype() == &DataType::String {
        // The text pass already validates semantic numeric candidates for normal-sized
        // columns. Reuse that result so a 5 GB CSV is not parsed twice.
        let validated_numeric_profile = text_profile.is_some_and(|profile| {
            matches!(profile.suggested_type, Some("integer") | Some("decimal"))
                && profile.invalid_type_count == Some(0)
        });
        let needs_validation = !validated_numeric_profile;
        if needs_validation {
            if text_profile.is_some_and(|profile| profile.suggested_type.is_some()) {
                return Ok(None);
            }
            let text = column
                .str()
                .map_err(|error| format!("No se pudo analizar texto numérico: {error}"))?;
            let mut has_value = false;
            for value in text.iter().flatten() {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    continue;
                }
                has_value = true;
                if semantic_numeric_value(trimmed).is_none() {
                    return Ok(None);
                }
            }
            if !has_value {
                return Ok(None);
            }
        }
        column
            .cast(&DataType::Float64)
            .map_err(|error| format!("No se pudo convertir texto numérico: {error}"))?
    } else {
        return Ok(None);
    };

    let value_count = source.len().saturating_sub(source.null_count());
    if value_count == 0 {
        return Ok(None);
    }

    let minimum = numeric_value(
        source
            .min_reduce()
            .map_err(|error| format!("No se pudo calcular el mínimo: {error}"))?
            .into_value(),
    );
    let maximum = numeric_value(
        source
            .max_reduce()
            .map_err(|error| format!("No se pudo calcular el máximo: {error}"))?
            .into_value(),
    );
    let mean = numeric_value(
        source
            .mean_reduce()
            .map_err(|error| format!("No se pudo calcular el promedio: {error}"))?
            .into_value(),
    );
    let standard_deviation = if value_count < 2 {
        None
    } else {
        numeric_value(
            source
                .std_reduce(1)
                .map_err(|error| format!("No se pudo calcular la desviación estándar: {error}"))?
                .into_value(),
        )
    };
    let quartile_values = source
        .quantiles_reduce(&[0.25, 0.5, 0.75], QuantileMethod::Linear)
        .map_err(|error| format!("No se pudieron calcular los cuantiles: {error}"))?
        .into_value();
    let mut quartiles = [None; 3];
    if let AnyValue::List(values) = quartile_values {
        let mut values = values.iter();
        for quartile in &mut quartiles {
            *quartile = values.next().and_then(numeric_value);
        }
    }
    let first_quartile = quartiles[0];
    let median = quartiles[1];
    let third_quartile = quartiles[2];
    // Reuse one Float64 view for the histogram and outlier pass. Integer
    // columns used to be cast twice, which increased peak memory on wide files.
    let floating_source = if source.dtype() == &DataType::Float64 {
        source.clone()
    } else {
        source
            .cast(&DataType::Float64)
            .map_err(|error| format!("No se pudo preparar las estadísticas numéricas: {error}"))?
    };
    let floating_values = floating_source
        .f64()
        .map_err(|error| format!("No se pudieron leer las estadísticas numéricas: {error}"))?;
    let histogram = match (minimum, maximum) {
        (Some(minimum), Some(maximum)) => {
            Some(numeric_histogram(floating_values, minimum, maximum))
        }
        _ => None,
    };
    let outlier_count = if value_count < 4 {
        0
    } else {
        let q1 = first_quartile.expect("cuatro valores siempre producen Q1");
        let q3 = third_quartile.expect("cuatro valores siempre producen Q3");
        let interquartile_range = q3 - q1;
        let lower_bound = q1 - 1.5 * interquartile_range;
        let upper_bound = q3 + 1.5 * interquartile_range;
        floating_values
            .iter()
            .flatten()
            .filter(|value| *value < lower_bound || *value > upper_bound)
            .count()
    };

    Ok(Some(NumericStatistics {
        minimum,
        maximum,
        mean,
        standard_deviation,
        first_quartile,
        median,
        third_quartile,
        outlier_count,
        histogram,
    }))
}

fn correlation_numeric_value(value: AnyValue<'_>) -> Option<f64> {
    if let Some(value) = numeric_value(value.clone()) {
        return Some(value);
    }
    match value {
        AnyValue::String(value) => semantic_numeric_value(value),
        AnyValue::StringOwned(value) => semantic_numeric_value(value.as_str()),
        _ => None,
    }
}

pub(super) fn numeric_correlation_matrix<C>(
    frame: &DataFrame,
    profiles: &[ColumnProfile],
    is_cancelled: &C,
    sample_row_limit: usize,
) -> Result<Option<NumericCorrelationMatrix>, String>
where
    C: Fn() -> bool + Sync,
{
    let numeric_columns = frame
        .columns()
        .iter()
        .zip(profiles)
        .filter(|(_, profile)| profile.outlier_count.is_some())
        .take(MAX_NUMERIC_CORRELATION_COLUMNS)
        .map(|(column, profile)| (column, profile.name.as_str()))
        .collect::<Vec<_>>();

    if numeric_columns.len() < 2 {
        return Ok(None);
    }

    let row_count = frame.height();
    if row_count == 0 {
        return Ok(None);
    }
    let sampled_row_count = row_count.min(sample_row_limit);
    let mut values = Vec::with_capacity(numeric_columns.len());
    for (column_index, (column, _)) in numeric_columns.iter().enumerate() {
        let mut column_values = Vec::with_capacity(sampled_row_count);
        for sample_index in 0..sampled_row_count {
            if sample_index % 4096 == 0 {
                ensure_not_cancelled(is_cancelled())?;
            }
            // Evenly sample the frame so a large file is not represented only by its header.
            let row_index = sample_index.saturating_mul(row_count) / sampled_row_count;
            let value = column.get(row_index).map_err(|error| {
                format!(
                    "No se pudieron calcular correlaciones para la columna {}: {error}",
                    numeric_columns[column_index].1
                )
            })?;
            column_values.push(correlation_numeric_value(value));
        }
        values.push(column_values);
    }

    let mut pairs =
        Vec::with_capacity(numeric_columns.len().saturating_mul(numeric_columns.len()) / 2);
    for first_index in 0..numeric_columns.len() {
        for second_index in (first_index + 1)..numeric_columns.len() {
            let (coefficient, sample_count) =
                pearson_correlation(&values[first_index], &values[second_index]);
            pairs.push(NumericCorrelation {
                first_column: numeric_columns[first_index].1.to_owned(),
                second_column: numeric_columns[second_index].1.to_owned(),
                coefficient,
                sample_count,
            });
        }
    }

    Ok(Some(NumericCorrelationMatrix {
        columns: numeric_columns
            .into_iter()
            .map(|(_, name)| name.to_owned())
            .collect(),
        pairs,
        sampled_row_count,
        truncated: profiles
            .iter()
            .filter(|profile| profile.outlier_count.is_some())
            .count()
            > MAX_NUMERIC_CORRELATION_COLUMNS,
    }))
}

pub(super) fn validate_numeric_correlation_sample_rows(
    sample_rows: usize,
) -> Result<usize, String> {
    if !(MIN_NUMERIC_CORRELATION_SAMPLE_ROWS..=MAX_NUMERIC_CORRELATION_SAMPLE_ROWS)
        .contains(&sample_rows)
    {
        return Err(format!(
            "La muestra de correlaciones debe estar entre {MIN_NUMERIC_CORRELATION_SAMPLE_ROWS} y {MAX_NUMERIC_CORRELATION_SAMPLE_ROWS} filas."
        ));
    }
    Ok(sample_rows)
}

fn pearson_correlation(first: &[Option<f64>], second: &[Option<f64>]) -> (Option<f64>, usize) {
    let mut paired = Vec::new();
    for (first, second) in first.iter().zip(second) {
        if let (Some(first), Some(second)) = (first, second) {
            paired.push((*first, *second));
        }
    }

    let sample_count = paired.len();
    if sample_count < 2 {
        return (None, sample_count);
    }
    let first_mean = paired.iter().map(|(first, _)| first).sum::<f64>() / sample_count as f64;
    let second_mean = paired.iter().map(|(_, second)| second).sum::<f64>() / sample_count as f64;
    let mut covariance = 0.0;
    let mut first_variance = 0.0;
    let mut second_variance = 0.0;
    for (first, second) in paired {
        let first_delta = first - first_mean;
        let second_delta = second - second_mean;
        covariance += first_delta * second_delta;
        first_variance += first_delta * first_delta;
        second_variance += second_delta * second_delta;
    }
    let denominator = (first_variance * second_variance).sqrt();
    let coefficient = if denominator > 0.0 && denominator.is_finite() {
        Some((covariance / denominator).clamp(-1.0, 1.0))
    } else {
        None
    };
    (coefficient, sample_count)
}

pub(super) fn source_numeric_correlation_matrix<C>(
    path: &Path,
    row_count: usize,
    profiles: &[ColumnProfile],
    is_cancelled: &C,
    sample_row_limit: usize,
) -> Result<Option<NumericCorrelationMatrix>, String>
where
    C: Fn() -> bool + Sync,
{
    let numeric_columns = profiles
        .iter()
        .filter(|profile| profile.outlier_count.is_some())
        .take(MAX_NUMERIC_CORRELATION_COLUMNS)
        .map(|profile| profile.name.clone())
        .collect::<Vec<_>>();
    if numeric_columns.len() < 2 || row_count == 0 {
        return Ok(None);
    }
    let sampled_row_count = row_count.min(sample_row_limit);
    let mut values = (0..numeric_columns.len())
        .map(|_| Vec::with_capacity(sampled_row_count))
        .collect::<Vec<Vec<Option<f64>>>>();
    let mut next_sample = 0usize;
    for_each_parquet_columns_block_with_size(
        path,
        row_count,
        &numeric_columns,
        SOURCE_PROFILE_BLOCK_ROWS,
        |start, block| {
            while next_sample < sampled_row_count {
                let row_index = next_sample.saturating_mul(row_count) / sampled_row_count;
                if row_index < start {
                    return Ok(());
                }
                if row_index >= start + block.height() {
                    break;
                }
                ensure_not_cancelled(is_cancelled())?;
                for (column_index, column_name) in numeric_columns.iter().enumerate() {
                    let column = block.columns().get(column_index).ok_or_else(|| {
                        format!("No se pudo leer la columna de correlación {column_name}.")
                    })?;
                    let value = column.get(row_index - start).map_err(|error| {
                    format!("No se pudieron calcular correlaciones para la columna {column_name}: {error}")
                })?;
                    values[column_index].push(correlation_numeric_value(value));
                }
                next_sample += 1;
            }
            Ok(())
        },
    )?;
    if next_sample != sampled_row_count {
        return Err("No se pudo leer la muestra completa de correlaciones.".to_owned());
    }
    let mut pairs =
        Vec::with_capacity(numeric_columns.len().saturating_mul(numeric_columns.len()) / 2);
    for first_index in 0..numeric_columns.len() {
        for second_index in (first_index + 1)..numeric_columns.len() {
            let (coefficient, sample_count) =
                pearson_correlation(&values[first_index], &values[second_index]);
            pairs.push(NumericCorrelation {
                first_column: numeric_columns[first_index].clone(),
                second_column: numeric_columns[second_index].clone(),
                coefficient,
                sample_count,
            });
        }
    }
    Ok(Some(NumericCorrelationMatrix {
        columns: numeric_columns,
        pairs,
        sampled_row_count,
        truncated: profiles
            .iter()
            .filter(|profile| profile.outlier_count.is_some())
            .count()
            > MAX_NUMERIC_CORRELATION_COLUMNS,
    }))
}
