use super::*;

pub(super) struct TextStatistics {
    pub(super) value_count: usize,
    pub(super) empty_count: usize,
    pub(super) sentinel_count: usize,
    pub(super) encoding_issue_count: usize,
    pub(super) minimum_length: Option<usize>,
    pub(super) maximum_length: Option<usize>,
    pub(super) average_length: Option<f64>,
    pub(super) suggested_type: Option<&'static str>,
    pub(super) type_match_percentage: Option<f64>,
    pub(super) invalid_type_count: Option<usize>,
    pub(super) temporal_bounds: TemporalBounds,
}

#[derive(Clone, Default)]
pub(super) struct TemporalBounds {
    minimum: Option<(NaiveDateTime, String)>,
    maximum: Option<(NaiveDateTime, String)>,
}

impl TemporalBounds {
    fn observe(&mut self, datetime: NaiveDateTime, value: String) {
        if self
            .minimum
            .as_ref()
            .is_none_or(|(current, _)| datetime < *current)
        {
            self.minimum = Some((datetime, value.clone()));
        }
        if self
            .maximum
            .as_ref()
            .is_none_or(|(current, _)| datetime > *current)
        {
            self.maximum = Some((datetime, value));
        }
    }

    fn minimum_value(&self) -> Option<String> {
        self.minimum.as_ref().map(|(_, value)| value.clone())
    }

    fn maximum_value(&self) -> Option<String> {
        self.maximum.as_ref().map(|(_, value)| value.clone())
    }
}

#[derive(Clone, Copy)]
pub(super) enum InferredDateFormat {
    Ymd,
    DmySlash,
    MdySlash,
    DmyDash,
    YmdSlash,
    DmyShort,
    DmyLong,
    CompactYmd,
    DmyShortDash,
    MdyShort,
    MdyLong,
    YmdTime,
    YmdSpaceTime,
    DmySlashTime,
    Iso8601,
}

pub(super) const INFERRED_DATE_FORMATS: &[InferredDateFormat] = &[
    InferredDateFormat::Ymd,
    InferredDateFormat::DmySlash,
    InferredDateFormat::MdySlash,
    InferredDateFormat::DmyDash,
    InferredDateFormat::YmdSlash,
    InferredDateFormat::DmyShort,
    InferredDateFormat::DmyLong,
    InferredDateFormat::CompactYmd,
    InferredDateFormat::DmyShortDash,
    InferredDateFormat::MdyShort,
    InferredDateFormat::MdyLong,
    InferredDateFormat::YmdTime,
    InferredDateFormat::YmdSpaceTime,
    InferredDateFormat::DmySlashTime,
    InferredDateFormat::Iso8601,
];

pub(super) fn parse_inferred_datetime(
    value: &str,
    format: InferredDateFormat,
) -> Option<NaiveDateTime> {
    let value = value.trim();
    let parse_date = |format| {
        NaiveDate::parse_from_str(value, format)
            .ok()
            .and_then(|date| date.and_hms_opt(0, 0, 0))
    };
    let parse_datetime = |formats: &[&str]| {
        formats
            .iter()
            .find_map(|format| NaiveDateTime::parse_from_str(value, format).ok())
    };

    match format {
        InferredDateFormat::Ymd => parse_date("%Y-%m-%d"),
        InferredDateFormat::DmySlash => parse_date("%d/%m/%Y"),
        InferredDateFormat::MdySlash => parse_date("%m/%d/%Y"),
        InferredDateFormat::DmyDash => parse_date("%d-%m-%Y"),
        InferredDateFormat::YmdSlash => parse_date("%Y/%m/%d"),
        InferredDateFormat::DmyShort => parse_date("%d %b %Y"),
        InferredDateFormat::DmyLong => parse_date("%d %B %Y"),
        InferredDateFormat::CompactYmd => parse_date("%Y%m%d"),
        InferredDateFormat::DmyShortDash => parse_date("%d-%b-%Y"),
        InferredDateFormat::MdyShort => parse_date("%b %d, %Y"),
        InferredDateFormat::MdyLong => parse_date("%B %d, %Y"),
        InferredDateFormat::YmdTime => {
            parse_datetime(&["%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%dT%H:%M:%S"])
        }
        InferredDateFormat::YmdSpaceTime => {
            parse_datetime(&["%Y-%m-%d %H:%M:%S%.f", "%Y-%m-%d %H:%M:%S"])
        }
        InferredDateFormat::DmySlashTime => parse_datetime(&["%d/%m/%Y %H:%M"]),
        InferredDateFormat::Iso8601 => parse_recipe_datetime(value, RecipeDateFormat::Iso8601).ok(),
    }
}

pub(super) fn is_supported_date(value: &str) -> bool {
    parse_supported_datetime(value).is_some()
}

pub(super) fn parse_supported_datetime(value: &str) -> Option<NaiveDateTime> {
    INFERRED_DATE_FORMATS
        .iter()
        .copied()
        .find_map(|format| parse_inferred_datetime(value, format))
}

pub(super) fn is_supported_date_candidate(value: &str) -> bool {
    let value = value.trim();
    if value.len() < 8 || !value.bytes().any(|byte| byte.is_ascii_digit()) {
        return false;
    }
    let compact_ymd = value.len() == 8 && value.bytes().all(|byte| byte.is_ascii_digit());
    if compact_ymd {
        return true;
    }
    if value.parse::<f64>().is_ok() {
        return false;
    }
    value
        .bytes()
        .any(|byte| matches!(byte, b'/' | b'-' | b':' | b',' | b' '))
}

pub(super) fn is_missing_sentinel(value: &str) -> bool {
    let normalized = normalize_text_value(value, true);
    SENTINEL_VALUES.contains(&normalized.as_str())
}

pub(super) fn mojibake_byte(character: char) -> Option<u8> {
    match character {
        '\u{20ac}' => Some(0x80),
        '\u{201a}' => Some(0x82),
        '\u{192}' => Some(0x83),
        '\u{201e}' => Some(0x84),
        '\u{2026}' => Some(0x85),
        '\u{2020}' => Some(0x86),
        '\u{2021}' => Some(0x87),
        '\u{2c6}' => Some(0x88),
        '\u{2030}' => Some(0x89),
        '\u{160}' => Some(0x8a),
        '\u{2039}' => Some(0x8b),
        '\u{152}' => Some(0x8c),
        '\u{17d}' => Some(0x8e),
        '\u{2018}' => Some(0x91),
        '\u{2019}' => Some(0x92),
        '\u{201c}' => Some(0x93),
        '\u{201d}' => Some(0x94),
        '\u{2022}' => Some(0x95),
        '\u{2013}' => Some(0x96),
        '\u{2014}' => Some(0x97),
        '\u{2dc}' => Some(0x98),
        '\u{2122}' => Some(0x99),
        '\u{161}' => Some(0x9a),
        '\u{203a}' => Some(0x9b),
        '\u{153}' => Some(0x9c),
        '\u{17e}' => Some(0x9e),
        '\u{178}' => Some(0x9f),
        character if (character as u32) <= 0xff => Some(character as u8),
        _ => None,
    }
}

pub(super) fn repair_mojibake(value: &str) -> Option<String> {
    if !MOJIBAKE_MARKERS.iter().any(|marker| value.contains(marker)) {
        return None;
    }
    let bytes = value
        .chars()
        .map(mojibake_byte)
        .collect::<Option<Vec<_>>>()?;
    let repaired = String::from_utf8(bytes).ok()?;
    (repaired != value && !repaired.contains('\u{fffd}')).then_some(repaired)
}

pub(super) fn boolean_token(value: &str) -> Option<&'static str> {
    match normalize_text_value(value, true).as_str() {
        "true" | "yes" | "si" => Some("true"),
        "false" | "no" => Some("false"),
        _ => None,
    }
}

pub(super) fn is_valid_suggested_type(value: &str, suggested_type: &str) -> bool {
    let value = value.trim();
    match suggested_type {
        "boolean" => boolean_token(value).is_some(),
        "integer" => value.parse::<i64>().is_ok() && semantic_numeric_value(value).is_some(),
        "decimal" => semantic_numeric_value(value).is_some(),
        "date" => is_supported_date(value),
        _ => false,
    }
}

pub(super) fn is_boolean_candidate(column: &Column) -> Result<bool, String> {
    if column.dtype() != &DataType::String {
        return Ok(false);
    }
    let values = column
        .str()
        .map_err(|error| format!("No se pudo analizar una columna booleana: {error}"))?;
    let non_empty = values
        .iter()
        .flatten()
        .filter(|value| !value.trim().is_empty())
        .collect::<Vec<_>>();
    if non_empty.len() < 3 {
        return Ok(false);
    }
    let recognized = non_empty
        .iter()
        .filter(|value| boolean_token(value).is_some())
        .count();
    Ok(recognized * 100 >= non_empty.len() * 90)
}

pub(super) fn privacy_signal(column_name: &str) -> Option<&'static str> {
    let normalized = normalize_text_value(column_name, true);
    let tokens = normalized
        .split(|character: char| !character.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    let compact = tokens.join("");
    let has_token = |values: &[&str]| {
        values
            .iter()
            .any(|value| tokens.iter().any(|token| token == value))
    };
    let has_text = |values: &[&str]| values.iter().any(|value| compact.contains(value));

    if has_text(&["email", "correo", "mail"]) {
        Some("email")
    } else if has_text(&["phone", "telefono", "tel", "movil", "celular"]) {
        Some("phone")
    } else if has_text(&["address", "direccion", "domicilio"]) {
        Some("address")
    } else if has_token(&[
        "id",
        "uuid",
        "identifier",
        "identificador",
        "codigo",
        "code",
        "clave",
        "key",
        "dni",
        "cedula",
        "pasaporte",
        "passport",
        "ssn",
    ]) {
        Some("identifier")
    } else if has_text(&["name", "nombre", "apellido", "surname"]) {
        Some("name")
    } else {
        None
    }
}

pub(super) fn suggest_text_type(
    value_count: usize,
    boolean_count: usize,
    integer_count: usize,
    decimal_count: usize,
    date_count: usize,
) -> (Option<&'static str>, Option<f64>, Option<usize>) {
    if value_count < 3 {
        return (None, None, None);
    }

    let mut best = ("boolean", boolean_count);
    for candidate in [
        ("integer", integer_count),
        ("decimal", decimal_count),
        ("date", date_count),
    ] {
        if candidate.1 > best.1 {
            best = candidate;
        }
    }

    let match_percentage = (best.1 as f64 / value_count as f64) * 100.0;
    if match_percentage < 90.0 {
        return (None, None, None);
    }

    (
        Some(best.0),
        Some(match_percentage),
        Some(value_count - best.1),
    )
}

pub(super) fn text_statistics(column: &Column) -> Result<Option<TextStatistics>, String> {
    if column.dtype() != &DataType::String {
        return Ok(None);
    }

    let values = column
        .str()
        .map_err(|error| format!("No se pudo analizar la columna de texto: {error}"))?;
    let mut empty_count = 0;
    let mut sentinel_count = 0;
    let mut encoding_issue_count = 0;
    let mut value_count: usize = 0;
    let mut boolean_count: usize = 0;
    let mut integer_count: usize = 0;
    let mut decimal_count: usize = 0;
    let mut date_count: usize = 0;
    let mut temporal_bounds = TemporalBounds::default();
    let mut total_length: usize = 0;
    let mut minimum_length: Option<usize> = None;
    let mut maximum_length: Option<usize> = None;

    for value in values.iter().flatten() {
        let length = value.chars().count();
        let trimmed = value.trim();
        let normalized = normalize_text_value(trimmed, true);
        empty_count += usize::from(trimmed.is_empty());
        sentinel_count += usize::from(SENTINEL_VALUES.contains(&normalized.as_str()));
        encoding_issue_count += usize::from(repair_mojibake(value).is_some());
        if !trimmed.is_empty() {
            boolean_count += usize::from(matches!(
                normalized.as_str(),
                "true" | "yes" | "si" | "false" | "no"
            ));
            let numeric = semantic_numeric_value(trimmed);
            integer_count += usize::from(trimmed.parse::<i64>().is_ok() && numeric.is_some());
            decimal_count += usize::from(numeric.is_some());
            if is_supported_date_candidate(trimmed) {
                if let Some(datetime) = quality_datetime_value(AnyValue::String(trimmed)) {
                    date_count += 1;
                    temporal_bounds.observe(datetime, trimmed.to_owned());
                }
            }
        }
        value_count += 1;
        total_length += length;
        minimum_length = Some(minimum_length.map_or(length, |current| current.min(length)));
        maximum_length = Some(maximum_length.map_or(length, |current| current.max(length)));
    }

    let average_length = (value_count > 0).then(|| total_length as f64 / value_count as f64);
    let non_empty_count = value_count.saturating_sub(empty_count);
    let (suggested_type, type_match_percentage, invalid_type_count) = suggest_text_type(
        non_empty_count,
        boolean_count,
        integer_count,
        decimal_count,
        date_count,
    );
    Ok(Some(TextStatistics {
        value_count,
        empty_count,
        sentinel_count,
        encoding_issue_count,
        minimum_length,
        maximum_length,
        average_length,
        suggested_type,
        type_match_percentage,
        invalid_type_count,
        temporal_bounds,
    }))
}

pub(super) enum ProfileColumnEvent {
    Progress {
        index: usize,
        stage: &'static str,
        phase: u8,
    },
    Completed {
        index: usize,
        result: Box<Result<ColumnProfile, String>>,
    },
}

pub(super) fn temporal_bounds_for_column<C>(
    column: &Column,
    is_cancelled: &C,
) -> Result<TemporalBounds, String>
where
    C: Fn() -> bool,
{
    let mut bounds = TemporalBounds::default();
    for row_index in 0..column.len() {
        if row_index % 4096 == 0 {
            ensure_not_cancelled(is_cancelled())?;
        }
        let value = column.get(row_index).map_err(|error| {
            format!(
                "No se pudo calcular el rango temporal de {}: {error}",
                column.name()
            )
        })?;
        if let Some(datetime) = quality_datetime_value(value.clone()) {
            let display_value = preview_value(value).unwrap_or_else(|| datetime.to_string());
            bounds.observe(datetime, display_value);
        }
    }
    ensure_not_cancelled(is_cancelled())?;
    Ok(bounds)
}

pub(super) fn profile_column<C, F>(
    column: &Column,
    row_count: usize,
    is_cancelled: &C,
    mut report: F,
) -> Result<ColumnProfile, String>
where
    C: Fn() -> bool,
    F: FnMut(&'static str, u8),
{
    ensure_not_cancelled(is_cancelled())?;
    let null_count = column.null_count();
    report("Contando valores únicos", 25);
    let unique_count = column
        .n_unique()
        .map_err(|error| format!("No se pudieron contar los valores únicos: {error}"))?
        .saturating_sub(usize::from(null_count > 0));
    let completeness_percentage = if row_count == 0 {
        100.0
    } else {
        ((row_count - null_count) as f64 / row_count as f64) * 100.0
    };

    ensure_not_cancelled(is_cancelled())?;
    report("Analizando texto", 50);
    let text_statistics = text_statistics(column)?;
    ensure_not_cancelled(is_cancelled())?;
    report("Calculando estadísticas numéricas", 75);
    let numeric_statistics = numeric_statistics(column, text_statistics.as_ref())?;
    let temporal_bounds = if matches!(column.dtype(), DataType::Date | DataType::Datetime(_, _)) {
        Some(temporal_bounds_for_column(column, is_cancelled)?)
    } else {
        text_statistics
            .as_ref()
            .filter(|statistics| statistics.suggested_type == Some("date"))
            .map(|statistics| statistics.temporal_bounds.clone())
    };
    let (minimum, maximum, mean) = if column.dtype().is_primitive_numeric() {
        let minimum = column
            .min_reduce()
            .map_err(|error| format!("No se pudo calcular el mínimo: {error}"))?
            .into_value();
        let maximum = column
            .max_reduce()
            .map_err(|error| format!("No se pudo calcular el máximo: {error}"))?
            .into_value();
        (
            preview_value(minimum),
            preview_value(maximum),
            numeric_statistics
                .as_ref()
                .and_then(|statistics| statistics.mean),
        )
    } else if let Some(bounds) = temporal_bounds.as_ref() {
        (bounds.minimum_value(), bounds.maximum_value(), None)
    } else if let Some(statistics) = numeric_statistics.as_ref() {
        (
            statistics.minimum.map(|value| value.to_string()),
            statistics.maximum.map(|value| value.to_string()),
            statistics.mean,
        )
    } else {
        (None, None, None)
    };

    Ok(ColumnProfile {
        name: column.name().to_string(),
        data_type: column.dtype().to_string(),
        null_count,
        completeness_percentage,
        unique_count,
        minimum,
        maximum,
        mean,
        empty_count: text_statistics
            .as_ref()
            .map(|statistics| statistics.empty_count),
        minimum_length: text_statistics
            .as_ref()
            .and_then(|statistics| statistics.minimum_length),
        maximum_length: text_statistics
            .as_ref()
            .and_then(|statistics| statistics.maximum_length),
        average_length: text_statistics
            .as_ref()
            .and_then(|statistics| statistics.average_length),
        suggested_type: text_statistics
            .as_ref()
            .and_then(|statistics| statistics.suggested_type)
            .map(str::to_owned),
        type_match_percentage: text_statistics
            .as_ref()
            .and_then(|statistics| statistics.type_match_percentage),
        invalid_type_count: text_statistics
            .as_ref()
            .and_then(|statistics| statistics.invalid_type_count),
        sentinel_count: text_statistics
            .as_ref()
            .map(|statistics| statistics.sentinel_count),
        encoding_issue_count: text_statistics
            .as_ref()
            .map(|statistics| statistics.encoding_issue_count),
        privacy_signal: privacy_signal(column.name()).map(str::to_owned),
        standard_deviation: numeric_statistics
            .as_ref()
            .and_then(|statistics| statistics.standard_deviation),
        first_quartile: numeric_statistics
            .as_ref()
            .and_then(|statistics| statistics.first_quartile),
        median: numeric_statistics
            .as_ref()
            .and_then(|statistics| statistics.median),
        third_quartile: numeric_statistics
            .as_ref()
            .and_then(|statistics| statistics.third_quartile),
        outlier_count: numeric_statistics
            .as_ref()
            .map(|statistics| statistics.outlier_count),
        histogram: numeric_statistics
            .as_ref()
            .and_then(|statistics| statistics.histogram.clone()),
    })
}

pub(super) struct SourceTextAccumulator {
    empty_count: usize,
    sentinel_count: usize,
    encoding_issue_count: usize,
    value_count: usize,
    boolean_count: usize,
    integer_count: usize,
    decimal_count: usize,
    date_count: usize,
    temporal_bounds: TemporalBounds,
    total_length: usize,
    minimum_length: Option<usize>,
    maximum_length: Option<usize>,
    categorical_candidates: HashMap<GroupKey, usize>,
}

impl SourceTextAccumulator {
    fn new() -> Self {
        Self {
            empty_count: 0,
            sentinel_count: 0,
            encoding_issue_count: 0,
            value_count: 0,
            boolean_count: 0,
            integer_count: 0,
            decimal_count: 0,
            date_count: 0,
            temporal_bounds: TemporalBounds::default(),
            total_length: 0,
            minimum_length: None,
            maximum_length: None,
            categorical_candidates: HashMap::with_capacity(MAX_GROUP_CANDIDATES),
        }
    }

    fn update(
        &mut self,
        column: &Column,
        numeric: &mut SourceNumericAccumulator,
    ) -> Result<(), String> {
        let values = column
            .str()
            .map_err(|error| format!("No se pudo analizar la columna source-backed: {error}"))?;
        for value in values.iter() {
            let Some(value) = value else {
                retain_group_candidate(&mut self.categorical_candidates, GroupKey::Missing);
                continue;
            };
            let length = value.chars().count();
            let trimmed = value.trim();
            let normalized = normalize_text_value(trimmed, true);
            self.empty_count = self
                .empty_count
                .saturating_add(usize::from(trimmed.is_empty()));
            self.sentinel_count = self
                .sentinel_count
                .saturating_add(usize::from(SENTINEL_VALUES.contains(&normalized.as_str())));
            self.encoding_issue_count = self
                .encoding_issue_count
                .saturating_add(usize::from(repair_mojibake(value).is_some()));
            self.value_count = self.value_count.saturating_add(1);
            self.total_length = self.total_length.saturating_add(length);
            self.minimum_length = Some(
                self.minimum_length
                    .map_or(length, |current| current.min(length)),
            );
            self.maximum_length = Some(
                self.maximum_length
                    .map_or(length, |current| current.max(length)),
            );
            if let Some(key) = categorical_group_key(AnyValue::String(value)) {
                retain_group_candidate(&mut self.categorical_candidates, key);
            }
            if trimmed.is_empty() {
                continue;
            }
            self.boolean_count = self.boolean_count.saturating_add(usize::from(matches!(
                normalized.as_str(),
                "true" | "yes" | "si" | "false" | "no"
            )));
            let parsed_numeric = semantic_numeric_value(trimmed);
            self.integer_count = self.integer_count.saturating_add(usize::from(
                trimmed.parse::<i64>().is_ok() && parsed_numeric.is_some(),
            ));
            self.decimal_count = self
                .decimal_count
                .saturating_add(usize::from(parsed_numeric.is_some()));
            if is_supported_date_candidate(trimmed) {
                if let Some(datetime) = quality_datetime_value(AnyValue::String(trimmed)) {
                    self.date_count = self.date_count.saturating_add(1);
                    self.temporal_bounds.observe(datetime, trimmed.to_owned());
                }
            }
            numeric.push(parsed_numeric)?;
        }
        Ok(())
    }

    fn finish(self) -> (TextStatistics, HashMap<GroupKey, usize>) {
        let categorical_candidates = self.categorical_candidates;
        let non_empty_count = self.value_count.saturating_sub(self.empty_count);
        let (suggested_type, type_match_percentage, invalid_type_count) = suggest_text_type(
            non_empty_count,
            self.boolean_count,
            self.integer_count,
            self.decimal_count,
            self.date_count,
        );
        (
            TextStatistics {
                value_count: self.value_count,
                empty_count: self.empty_count,
                sentinel_count: self.sentinel_count,
                encoding_issue_count: self.encoding_issue_count,
                minimum_length: self.minimum_length,
                maximum_length: self.maximum_length,
                average_length: (self.value_count > 0)
                    .then(|| self.total_length as f64 / self.value_count as f64),
                suggested_type,
                type_match_percentage,
                invalid_type_count,
                temporal_bounds: self.temporal_bounds,
            },
            categorical_candidates,
        )
    }
}

pub(super) struct SourceNumericAccumulator {
    value_count: usize,
    minimum: Option<f64>,
    maximum: Option<f64>,
    mean: f64,
    m2: f64,
    runs: Option<NumericRunWriter>,
}

impl SourceNumericAccumulator {
    fn new() -> Self {
        Self {
            value_count: 0,
            minimum: None,
            maximum: None,
            mean: 0.0,
            m2: 0.0,
            runs: None,
        }
    }

    fn push(&mut self, value: Option<f64>) -> Result<(), String> {
        let Some(value) = value.filter(|value| value.is_finite()) else {
            return Ok(());
        };
        self.value_count = self.value_count.saturating_add(1);
        self.minimum = Some(self.minimum.map_or(value, |current| current.min(value)));
        self.maximum = Some(self.maximum.map_or(value, |current| current.max(value)));
        let delta = value - self.mean;
        self.mean += delta / self.value_count as f64;
        self.m2 += delta * (value - self.mean);
        if self.runs.is_none() {
            self.runs = Some(NumericRunWriter::new()?);
        }
        self.runs
            .as_mut()
            .expect("la corrida numérica debe existir")
            .push(value)
    }
}

pub(super) struct SourceColumnAccumulator {
    name: String,
    data_type: String,
    is_string: bool,
    is_primitive_numeric: bool,
    is_temporal: bool,
    null_count: usize,
    text: Option<SourceTextAccumulator>,
    numeric: SourceNumericAccumulator,
    temporal_bounds: TemporalBounds,
}

impl SourceColumnAccumulator {
    fn new(column: &Column) -> Self {
        Self {
            name: column.name().to_string(),
            data_type: column.dtype().to_string(),
            is_string: column.dtype() == &DataType::String,
            is_primitive_numeric: column.dtype().is_primitive_numeric(),
            is_temporal: matches!(column.dtype(), DataType::Date | DataType::Datetime(_, _)),
            null_count: 0,
            text: (column.dtype() == &DataType::String).then(SourceTextAccumulator::new),
            numeric: SourceNumericAccumulator::new(),
            temporal_bounds: TemporalBounds::default(),
        }
    }

    fn update(&mut self, column: &Column) -> Result<(), String> {
        self.null_count = self.null_count.saturating_add(column.null_count());
        if let Some(text) = self.text.as_mut() {
            text.update(column, &mut self.numeric)?;
        } else if self.is_primitive_numeric {
            for row_index in 0..column.len() {
                let value = column.get(row_index).map_err(|error| {
                    format!("No se pudo analizar la columna numérica source-backed: {error}")
                })?;
                self.numeric.push(numeric_value(value))?;
            }
        } else if self.is_temporal {
            for row_index in 0..column.len() {
                let value = column.get(row_index).map_err(|error| {
                    format!("No se pudo analizar la columna temporal source-backed: {error}")
                })?;
                if let Some(datetime) = quality_datetime_value(value.clone()) {
                    let display_value =
                        preview_value(value).unwrap_or_else(|| datetime.to_string());
                    self.temporal_bounds.observe(datetime, display_value);
                }
            }
        }
        Ok(())
    }

    fn finish(
        self,
        row_count: usize,
        unique_count: usize,
    ) -> Result<(ColumnProfile, Option<HashMap<GroupKey, usize>>), String> {
        let (text, categorical_candidates) = self
            .text
            .map(SourceTextAccumulator::finish)
            .map_or((None, None), |(text, candidates)| {
                (Some(text), Some(candidates))
            });
        let numeric_eligible = self.is_primitive_numeric
            || (self.is_string
                && text.as_ref().is_some_and(|text| {
                    self.numeric.value_count > 0
                        && self.numeric.value_count
                            == text.value_count.saturating_sub(text.empty_count)
                }));
        let numeric_runs = self
            .numeric
            .runs
            .map(NumericRunWriter::finish)
            .transpose()?;
        let numeric_statistics = if numeric_eligible {
            match numeric_runs.as_ref() {
                Some(runs) => source_numeric_statistics(
                    runs,
                    self.numeric.value_count,
                    self.numeric.minimum,
                    self.numeric.maximum,
                    (self.numeric.value_count > 0).then_some(self.numeric.mean),
                    self.numeric.m2,
                )?,
                None => None,
            }
        } else {
            None
        };
        let temporal_bounds = if self.is_temporal {
            Some(&self.temporal_bounds)
        } else {
            text.as_ref()
                .filter(|statistics| statistics.suggested_type == Some("date"))
                .map(|statistics| &statistics.temporal_bounds)
        };
        let (minimum, maximum, mean) = if self.is_primitive_numeric {
            (
                self.numeric.minimum.map(|value| value.to_string()),
                self.numeric.maximum.map(|value| value.to_string()),
                numeric_statistics
                    .as_ref()
                    .and_then(|statistics| statistics.mean),
            )
        } else if let Some(bounds) = temporal_bounds {
            (bounds.minimum_value(), bounds.maximum_value(), None)
        } else if let Some(statistics) = numeric_statistics.as_ref() {
            (
                statistics.minimum.map(|value| value.to_string()),
                statistics.maximum.map(|value| value.to_string()),
                statistics.mean,
            )
        } else {
            (None, None, None)
        };
        Ok((
            ColumnProfile {
                name: self.name.clone(),
                data_type: self.data_type,
                null_count: self.null_count,
                completeness_percentage: if row_count == 0 {
                    100.0
                } else {
                    ((row_count.saturating_sub(self.null_count)) as f64 / row_count as f64) * 100.0
                },
                unique_count,
                minimum,
                maximum,
                mean,
                empty_count: text.as_ref().map(|statistics| statistics.empty_count),
                minimum_length: text
                    .as_ref()
                    .and_then(|statistics| statistics.minimum_length),
                maximum_length: text
                    .as_ref()
                    .and_then(|statistics| statistics.maximum_length),
                average_length: text
                    .as_ref()
                    .and_then(|statistics| statistics.average_length),
                suggested_type: text
                    .as_ref()
                    .and_then(|statistics| statistics.suggested_type)
                    .map(str::to_owned),
                type_match_percentage: text
                    .as_ref()
                    .and_then(|statistics| statistics.type_match_percentage),
                invalid_type_count: text
                    .as_ref()
                    .and_then(|statistics| statistics.invalid_type_count),
                sentinel_count: text.as_ref().map(|statistics| statistics.sentinel_count),
                encoding_issue_count: text
                    .as_ref()
                    .map(|statistics| statistics.encoding_issue_count),
                privacy_signal: privacy_signal(&self.name).map(str::to_owned),
                standard_deviation: numeric_statistics
                    .as_ref()
                    .and_then(|statistics| statistics.standard_deviation),
                first_quartile: numeric_statistics
                    .as_ref()
                    .and_then(|statistics| statistics.first_quartile),
                median: numeric_statistics
                    .as_ref()
                    .and_then(|statistics| statistics.median),
                third_quartile: numeric_statistics
                    .as_ref()
                    .and_then(|statistics| statistics.third_quartile),
                outlier_count: numeric_statistics
                    .as_ref()
                    .map(|statistics| statistics.outlier_count),
                histogram: numeric_statistics
                    .as_ref()
                    .and_then(|statistics| statistics.histogram.clone()),
            },
            categorical_candidates,
        ))
    }
}

pub(super) fn count_distinct_rows(frame: &DataFrame) -> Result<usize, String> {
    let streaming_result = frame
        .clone()
        .lazy()
        .unique(None, UniqueKeepStrategy::First)
        .select([len().alias("__distinct_rows")])
        .collect_with_engine(Engine::Streaming);

    match streaming_result {
        Ok(result) => {
            let result = result.unwrap_single();
            let value = result
                .column("__distinct_rows")
                .map_err(|error| format!("No se pudo leer el conteo de filas distintas: {error}"))?
                .get(0)
                .map_err(|error| format!("No se pudo leer el conteo de filas distintas: {error}"))?;
            match value {
                AnyValue::UInt8(value) => Ok(value.into()),
                AnyValue::UInt16(value) => Ok(value.into()),
                AnyValue::UInt32(value) => Ok(value as usize),
                AnyValue::UInt64(value) => usize::try_from(value).map_err(|_| {
                    "El conteo de filas distintas excede la capacidad local.".to_owned()
                }),
                AnyValue::UInt128(value) => usize::try_from(value).map_err(|_| {
                    "El conteo de filas distintas excede la capacidad local.".to_owned()
                }),
                _ => Err(
                    "El motor devolvió un tipo inesperado para el conteo de filas distintas."
                        .to_owned(),
                ),
            }
        }
        Err(streaming_error) => frame
            .unique::<Vec<String>, String>(None, UniqueKeepStrategy::First, None)
            .map(|distinct| distinct.height())
            .map_err(|fallback_error| {
                format!(
                    "No se pudieron detectar las filas duplicadas: {fallback_error} (streaming: {streaming_error})"
                )
            }),
    }
}

pub(super) fn quote_temporal_sql_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

pub(super) fn profile_dataset_with_progress<F, C>(
    frame: &DataFrame,
    mut report: F,
    is_cancelled: C,
    correlation_sample_rows: usize,
) -> Result<DatasetProfile, String>
where
    F: FnMut(&'static str, u8),
    C: Fn() -> bool + Sync,
{
    ensure_not_cancelled(is_cancelled())?;
    let row_count = frame.height();
    report("Detectando filas duplicadas", 10);
    let distinct_row_count = count_distinct_rows(frame)?;
    let duplicate_row_count = row_count.saturating_sub(distinct_row_count);
    let duplicate_percentage = if row_count == 0 {
        0.0
    } else {
        (duplicate_row_count as f64 / row_count as f64) * 100.0
    };
    report("Normalizando filas parecidas", 15);
    let near_duplicate_row_count =
        count_normalized_duplicate_rows(frame, duplicate_row_count, &is_cancelled, &mut report)?;
    ensure_not_cancelled(is_cancelled())?;
    let source_columns = frame.columns();
    let mut columns = Vec::with_capacity(source_columns.len());
    if source_columns.is_empty() {
        report("Perfil completado", 100);
    } else {
        report("Analizando columnas", 40);
        let column_count = source_columns.len();
        let worker_count = std::thread::available_parallelism()
            .map(|parallelism| parallelism.get().min(4))
            .unwrap_or(2)
            .min(column_count)
            .max(1);
        let work_queue =
            std::sync::Arc::new(Mutex::new((0..column_count).collect::<VecDeque<_>>()));
        let (sender, receiver) = mpsc::channel();
        let mut profiles = (0..column_count)
            .map(|_| None)
            .collect::<Vec<Option<ColumnProfile>>>();
        let mut progress = vec![0_u16; column_count];
        let mut completed_columns = 0usize;
        let mut first_error = None;
        let mut last_percent = 40_u8;

        std::thread::scope(|scope| {
            for _ in 0..worker_count {
                let work_queue = std::sync::Arc::clone(&work_queue);
                let sender = sender.clone();
                let cancellation = &is_cancelled;
                scope.spawn(move || {
                    while let Some(index) = work_queue
                        .lock()
                        .ok()
                        .and_then(|mut queue| queue.pop_front())
                    {
                        let column = &source_columns[index];
                        let progress_sender = sender.clone();
                        let result =
                            profile_column(column, row_count, cancellation, |stage, phase| {
                                let _ = progress_sender.send(ProfileColumnEvent::Progress {
                                    index,
                                    stage,
                                    phase,
                                });
                            });
                        let _ = sender.send(ProfileColumnEvent::Completed {
                            index,
                            result: Box::new(result),
                        });
                    }
                });
            }
            drop(sender);

            while let Ok(event) = receiver.recv() {
                match event {
                    ProfileColumnEvent::Progress {
                        index,
                        stage,
                        phase,
                    } => {
                        progress[index] = progress[index].max(u16::from(phase));
                        let weighted_progress = progress
                            .iter()
                            .map(|value| usize::from(*value))
                            .sum::<usize>();
                        let percent = 40 + ((weighted_progress * 50) / (column_count * 100)) as u8;
                        last_percent = last_percent.max(percent);
                        report(stage, last_percent);
                    }
                    ProfileColumnEvent::Completed { index, result } => {
                        progress[index] = 100;
                        completed_columns += 1;
                        match *result {
                            Ok(profile) => profiles[index] = Some(profile),
                            Err(error) => {
                                first_error.get_or_insert(error);
                            }
                        }
                        let weighted_progress = progress
                            .iter()
                            .map(|value| usize::from(*value))
                            .sum::<usize>();
                        let percent = 40 + ((weighted_progress * 50) / (column_count * 100)) as u8;
                        last_percent = last_percent.max(percent);
                        report("Analizando columnas", last_percent);
                        if completed_columns == column_count && last_percent < 90 {
                            report("Analizando columnas", 90);
                        }
                    }
                }
            }
        });

        if let Some(error) = first_error {
            return Err(error);
        }
        columns = profiles
            .into_iter()
            .map(|profile| profile.expect("cada columna debe producir un perfil"))
            .collect();
    }

    let categorical_group_summaries = if columns.is_empty() {
        None
    } else {
        report("Resumiendo categorías", 93);
        categorical_group_summaries(frame, &columns, &is_cancelled)?
    };

    let temporal_series = if columns.is_empty() {
        None
    } else {
        report("Resumiendo tendencia temporal", 95);
        temporal_series_summaries(frame, &columns, &is_cancelled)?
    };

    let numeric_correlations = if columns.is_empty() {
        None
    } else {
        report("Calculando correlaciones", 97);
        let correlations =
            numeric_correlation_matrix(frame, &columns, &is_cancelled, correlation_sample_rows)?;
        report("Analizando columnas", 100);
        correlations
    };

    Ok(DatasetProfile {
        row_count,
        duplicate_row_count,
        near_duplicate_row_count,
        duplicate_percentage,
        columns,
        numeric_correlations,
        categorical_group_summaries,
        temporal_series,
    })
}

pub(super) fn count_normalized_duplicate_rows<C, F>(
    frame: &DataFrame,
    exact_duplicate_row_count: usize,
    is_cancelled: &C,
    report: &mut F,
) -> Result<usize, String>
where
    C: Fn() -> bool + Sync,
    F: FnMut(&'static str, u8),
{
    if frame.height() == 0 {
        return Ok(0);
    }

    let fingerprint_columns = normalized_fingerprint_columns(frame.columns())?;
    let chunk_count = frame.height().div_ceil(NORMALIZED_DUPLICATE_CHUNK_ROWS);
    let spill_directory = tempfile::tempdir().map_err(|error| {
        format!("No se pudo preparar el almacenamiento temporal para duplicados parecidos: {error}")
    })?;
    let bucket_paths = (0..NORMALIZED_DUPLICATE_BUCKETS)
        .map(|bucket| {
            spill_directory
                .path()
                .join(format!("fingerprints-{bucket:03}.bin"))
        })
        .collect::<Vec<_>>();
    let mut bucket_writers = (0..NORMALIZED_DUPLICATE_BUCKETS)
        .map(|_| None::<BufWriter<File>>)
        .collect::<Vec<_>>();
    let (progress_sender, progress_receiver) = mpsc::sync_channel(2);
    let producer_result = std::thread::scope(|scope| {
        let producer = scope.spawn(move || {
            (0..chunk_count)
                .into_par_iter()
                .try_for_each(|chunk_index| {
                    if is_cancelled() {
                        return Err(OPERATION_CANCELLED_MESSAGE.to_owned());
                    }

                    let start = chunk_index * NORMALIZED_DUPLICATE_CHUNK_ROWS;
                    let end = (start + NORMALIZED_DUPLICATE_CHUNK_ROWS).min(frame.height());
                    let mut fingerprints = Vec::with_capacity(end - start);
                    for row_index in start..end {
                        if row_index % 4096 == 0 && is_cancelled() {
                            return Err(OPERATION_CANCELLED_MESSAGE.to_owned());
                        }
                        let key = normalized_row_fingerprint(&fingerprint_columns, row_index)?;
                        fingerprints.push(key);
                    }
                    progress_sender
                        .send((chunk_index, fingerprints))
                        .map_err(|_| OPERATION_CANCELLED_MESSAGE.to_owned())
                })
        });

        let mut completed_chunks = 0_usize;
        let mut spill_error = None;
        while let Ok((_, mut partial)) = progress_receiver.recv() {
            if spill_error.is_none() && !is_cancelled() {
                for (fingerprint_index, fingerprint) in partial.drain(..).enumerate() {
                    if fingerprint_index % 4096 == 0 && is_cancelled() {
                        break;
                    }
                    let bucket = (fingerprint >> 120) as usize;
                    let writer = if let Some(writer) = bucket_writers[bucket].as_mut() {
                        writer
                    } else {
                        let file = match File::create(&bucket_paths[bucket]) {
                            Ok(file) => file,
                            Err(error) => {
                                spill_error = Some(format!(
                                    "No se pudo preparar el almacenamiento temporal para duplicados parecidos: {error}"
                                ));
                                break;
                            }
                        };
                        bucket_writers[bucket]
                            .get_or_insert_with(|| BufWriter::with_capacity(64 * 1024, file))
                    };
                    if let Err(error) = writer.write_all(&fingerprint.to_le_bytes()) {
                        spill_error = Some(format!(
                            "No se pudieron guardar las huellas temporales de duplicados parecidos: {error}"
                        ));
                        break;
                    }
                }
            }
            completed_chunks += 1;
            let percent = 15 + ((completed_chunks.saturating_mul(25) / chunk_count).min(25)) as u8;
            report("Normalizando filas parecidas", percent);
        }

        let producer_result = producer
            .join()
            .map_err(|_| "El perfilado paralelo se interrumpió inesperadamente.".to_owned())?;
        producer_result?;
        ensure_not_cancelled(is_cancelled())?;
        if let Some(error) = spill_error {
            return Err(error);
        }

        for writer in bucket_writers.iter_mut().flatten() {
            writer.flush().map_err(|error| {
                format!(
                    "No se pudieron sincronizar las huellas temporales de duplicados parecidos: {error}"
                )
            })?;
        }
        drop(bucket_writers);
        Ok(())
    });
    producer_result?;

    report("Ordenando filas parecidas", 40);
    let mut normalized_duplicate_row_count = 0usize;
    for bucket_path in &bucket_paths {
        ensure_not_cancelled(is_cancelled())?;
        if !bucket_path.exists() {
            continue;
        }
        let bytes = fs::metadata(bucket_path)
            .map_err(|error| {
                format!("No se pudo inspeccionar el almacenamiento temporal: {error}")
            })?
            .len();
        if bytes % NORMALIZED_FINGERPRINT_BYTES as u64 != 0 {
            return Err(
                "El almacenamiento temporal de duplicados parecidos quedó incompleto.".to_owned(),
            );
        }
        let fingerprint_count = usize::try_from(bytes / NORMALIZED_FINGERPRINT_BYTES as u64)
            .map_err(|_| "El conteo de huellas temporales excede la capacidad local.".to_owned())?;
        let file = File::open(bucket_path).map_err(|error| {
            format!("No se pudo leer el almacenamiento temporal de duplicados parecidos: {error}")
        })?;
        let mut reader = BufReader::new(file);
        let mut fingerprints = Vec::with_capacity(fingerprint_count);
        let mut encoded = [0_u8; NORMALIZED_FINGERPRINT_BYTES];
        for index in 0..fingerprint_count {
            if index % 4096 == 0 {
                ensure_not_cancelled(is_cancelled())?;
            }
            reader.read_exact(&mut encoded).map_err(|error| {
                format!("No se pudo leer una huella temporal de duplicados parecidos: {error}")
            })?;
            fingerprints.push(u128::from_le_bytes(encoded));
        }
        fingerprints.sort_unstable();
        normalized_duplicate_row_count = normalized_duplicate_row_count.saturating_add(
            fingerprints
                .windows(2)
                .filter(|pair| pair[0] == pair[1])
                .count(),
        );
    }
    ensure_not_cancelled(is_cancelled())?;
    Ok(normalized_duplicate_row_count.saturating_sub(exact_duplicate_row_count))
}

pub(super) type SourceProfileColumns = (
    usize,
    usize,
    Vec<ColumnProfile>,
    Vec<Option<HashMap<GroupKey, usize>>>,
);

pub(super) fn profile_source_columns_with_keys<F, C>(
    path: &Path,
    row_count: usize,
    schema_columns: &[Column],
    is_cancelled: &C,
    mut report: F,
) -> Result<SourceProfileColumns, String>
where
    F: FnMut(&'static str, u8),
    C: Fn() -> bool + Sync,
{
    let normalized_directory = tempfile::tempdir().map_err(|error| {
        format!("No se pudo preparar el almacenamiento temporal para duplicados parecidos: {error}")
    })?;
    let normalized_bucket_paths = (0..NORMALIZED_DUPLICATE_BUCKETS)
        .map(|bucket| {
            normalized_directory
                .path()
                .join(format!("source-fingerprints-{bucket:03}.bin"))
        })
        .collect::<Vec<_>>();
    let mut normalized_writers = (0..NORMALIZED_DUPLICATE_BUCKETS)
        .map(|_| None::<BufWriter<File>>)
        .collect::<Vec<_>>();
    let mut accumulators = schema_columns
        .iter()
        .map(SourceColumnAccumulator::new)
        .collect::<Vec<_>>();
    let column_names = schema_columns
        .iter()
        .map(|column| column.name().to_string())
        .collect::<Vec<_>>();

    report("Analizando filas y columnas", 10);
    for_each_parquet_block_with_size(
        path,
        row_count,
        SOURCE_PROFILE_BLOCK_ROWS,
        |start, block| {
            ensure_not_cancelled(is_cancelled())?;
            let fingerprint_columns = normalized_fingerprint_columns(block.columns())?;
            let fingerprints = (0..block.height())
                .into_par_iter()
                .map(|row_index| {
                    if row_index.is_multiple_of(LOCAL_QUERY_CANCEL_CHECK_ROWS) {
                        ensure_not_cancelled(is_cancelled())?;
                    }
                    normalized_row_fingerprint(&fingerprint_columns, row_index)
                })
                .collect::<Result<Vec<_>, String>>()?;
            for fingerprint in fingerprints {
                let bucket = (fingerprint >> 120) as usize;
                let writer = if let Some(writer) = normalized_writers[bucket].as_mut() {
                    writer
                } else {
                    let file = File::create(&normalized_bucket_paths[bucket]).map_err(|error| {
                    format!("No se pudo preparar el almacenamiento temporal para duplicados parecidos: {error}")
                })?;
                    normalized_writers[bucket]
                        .get_or_insert_with(|| BufWriter::with_capacity(64 * 1024, file))
                };
                writer
                .write_all(&fingerprint.to_le_bytes())
                .map_err(|error| {
                    format!("No se pudieron guardar las huellas temporales de duplicados parecidos: {error}")
                })?;
            }

            accumulators
                .par_iter_mut()
                .zip(block.columns().par_iter())
                .try_for_each(|(accumulator, column)| accumulator.update(column))?;

            let processed_rows = start.saturating_add(block.height());
            let progress = processed_rows
                .saturating_mul(80)
                .checked_div(row_count)
                .unwrap_or(80);
            let percent = 10 + progress.min(80) as u8;
            report("Analizando filas y columnas", percent);
            Ok(())
        },
    )?;

    for writer in normalized_writers.iter_mut().flatten() {
        writer.flush().map_err(|error| {
            format!(
                "No se pudieron sincronizar las huellas temporales de duplicados parecidos: {error}"
            )
        })?;
    }
    let (distinct_row_count, distinct_counts) =
        crate::duckdb_query::count_file_distinct_rows_and_non_null_columns(
            path,
            crate::duckdb_query::DuckDbFileFormat::Parquet,
            &column_names,
            || false,
        )?;
    let normalized_duplicate_row_count =
        count_normalized_duplicate_fingerprints(&normalized_bucket_paths, is_cancelled)?;
    let exact_duplicate_row_count = row_count.saturating_sub(distinct_row_count);
    let near_duplicate_row_count =
        normalized_duplicate_row_count.saturating_sub(exact_duplicate_row_count);
    if distinct_counts.len() != accumulators.len() {
        return Err("DuckDB no devolvió todos los conteos distintos del perfil.".to_owned());
    }
    let mut columns = Vec::with_capacity(schema_columns.len());
    let mut categorical_candidates = Vec::with_capacity(schema_columns.len());
    for (index, accumulator) in accumulators.into_iter().enumerate() {
        let distinct_values = distinct_counts[index];
        let (column, candidates) = accumulator.finish(row_count, distinct_values)?;
        columns.push(column);
        categorical_candidates.push(candidates);
    }

    report("Analizando columnas", 90);
    Ok((
        exact_duplicate_row_count,
        near_duplicate_row_count,
        columns,
        categorical_candidates,
    ))
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct TemporalValueAccumulator {
    row_count: usize,
    value_count: usize,
    sum: f64,
}

impl TemporalValueAccumulator {
    fn add_row(&mut self, value: AnyValue<'_>) -> Result<(), String> {
        self.row_count = self.row_count.saturating_add(1);
        if let Some(value) = quality_aggregate_numeric_value(value) {
            let sum = self.sum + value;
            if !sum.is_finite() {
                return Err("La suma temporal excede el rango numérico admitido.".to_owned());
            }
            self.sum = sum;
            self.value_count = self.value_count.saturating_add(1);
        }
        Ok(())
    }

    fn merge(&mut self, other: Self) -> Result<(), String> {
        let sum = self.sum + other.sum;
        if !sum.is_finite() {
            return Err("La suma temporal excede el rango numérico admitido.".to_owned());
        }
        self.sum = sum;
        self.row_count = self.row_count.saturating_add(other.row_count);
        self.value_count = self.value_count.saturating_add(other.value_count);
        Ok(())
    }

    fn value(self, aggregation: TemporalAggregationKind) -> Option<f64> {
        if self.value_count == 0 {
            return None;
        }
        let value = match aggregation {
            TemporalAggregationKind::Sum => self.sum,
            TemporalAggregationKind::Mean => self.sum / self.value_count as f64,
        };
        value.is_finite().then_some(value)
    }
}

pub(super) fn temporal_aggregation_bucket_label(
    key: TemporalPeriodKey,
    granularity: &str,
) -> String {
    let key = match granularity {
        "day" => key,
        "month" => TemporalPeriodKey {
            year: key.year,
            month: key.month,
            day: 1,
        },
        _ => TemporalPeriodKey {
            year: key.year,
            month: 1,
            day: 1,
        },
    };
    temporal_period_label(key, granularity)
}

pub(super) struct TemporalAggregationSummaryData {
    total_row_count: usize,
    parsed_row_count: usize,
    first: Option<TemporalPeriodKey>,
    last: Option<TemporalPeriodKey>,
    by_day: HashMap<TemporalPeriodKey, TemporalValueAccumulator>,
}

pub(super) fn temporal_aggregation_series_from_values(
    date_column: &str,
    value_column: &str,
    aggregation: TemporalAggregationKind,
    data: TemporalAggregationSummaryData,
) -> Result<TemporalAggregationSeries, String> {
    let TemporalAggregationSummaryData {
        total_row_count,
        parsed_row_count,
        first,
        last,
        by_day,
    } = data;
    let (Some(first), Some(last)) = (first, last) else {
        return Ok(TemporalAggregationSeries {
            date_column: date_column.to_owned(),
            value_column: value_column.to_owned(),
            aggregation,
            granularity: "day".to_owned(),
            periods: Vec::new(),
            parsed_row_count: 0,
            unparsed_row_count: total_row_count,
            truncated: false,
        });
    };
    let first_date = NaiveDate::from_ymd_opt(first.year, first.month, first.day)
        .ok_or_else(|| "No se pudo interpretar el inicio de la tendencia temporal.".to_owned())?;
    let last_date = NaiveDate::from_ymd_opt(last.year, last.month, last.day)
        .ok_or_else(|| "No se pudo interpretar el fin de la tendencia temporal.".to_owned())?;
    let day_span = (last_date - first_date).num_days();
    let month_span = (i64::from(last.year) - i64::from(first.year)) * 12 + i64::from(last.month)
        - i64::from(first.month);
    let granularity = if day_span <= MAX_TEMPORAL_DAY_SPAN {
        "day"
    } else if month_span <= MAX_TEMPORAL_MONTH_SPAN {
        "month"
    } else {
        "year"
    };

    let counts = by_day
        .iter()
        .map(|(key, value)| (*key, value.row_count))
        .collect::<HashMap<_, _>>();
    let mut raw_periods = temporal_periods_between(first, last, granularity, &counts);
    raw_periods.retain(|(_, count)| *count > 0 || matches!(granularity, "month" | "day"));

    let mut daily_values = by_day.into_iter().collect::<Vec<_>>();
    daily_values.sort_unstable_by_key(|(key, _)| *key);
    let mut by_period = HashMap::<String, TemporalValueAccumulator>::new();
    for (key, value) in daily_values {
        by_period
            .entry(temporal_aggregation_bucket_label(key, granularity))
            .or_default()
            .merge(value)?;
    }

    let truncated = raw_periods.len() > MAX_TEMPORAL_PERIODS;
    let mut periods = Vec::with_capacity(raw_periods.len().min(MAX_TEMPORAL_PERIODS));
    if truncated {
        let split = raw_periods.len() - (MAX_TEMPORAL_PERIODS - 1);
        let previous_row_count = raw_periods[..split]
            .iter()
            .map(|(_, row_count)| *row_count)
            .sum();
        let mut previous = TemporalValueAccumulator::default();
        for (period, _) in &raw_periods[..split] {
            if let Some(value) = by_period.get(period) {
                previous.merge(*value)?;
            }
        }
        periods.push(TemporalAggregationPeriod {
            period: "Periodos anteriores".to_owned(),
            row_count: previous_row_count,
            value_count: previous.value_count,
            value: previous.value(aggregation),
        });
        raw_periods.drain(..split);
    }
    periods.extend(raw_periods.into_iter().map(|(period, row_count)| {
        let value = by_period.get(&period).copied().unwrap_or_default();
        TemporalAggregationPeriod {
            period,
            row_count,
            value_count: value.value_count,
            value: value.value(aggregation),
        }
    }));

    Ok(TemporalAggregationSeries {
        date_column: date_column.to_owned(),
        value_column: value_column.to_owned(),
        aggregation,
        granularity: granularity.to_owned(),
        periods,
        parsed_row_count,
        unparsed_row_count: total_row_count.saturating_sub(parsed_row_count),
        truncated,
    })
}

pub(super) fn temporal_aggregation_summary<C>(
    frame: &DataFrame,
    date_column: &str,
    value_column: &str,
    aggregation: TemporalAggregationKind,
    is_cancelled: &C,
) -> Result<TemporalAggregationSeries, String>
where
    C: Fn() -> bool + Sync,
{
    let date_values = frame.column(date_column).map_err(|_| {
        format!("La columna de fecha '{date_column}' no existe en el dataset activo.")
    })?;
    let metric_values = frame.column(value_column).map_err(|_| {
        format!("La columna numérica '{value_column}' no existe en el dataset activo.")
    })?;
    let mut data = TemporalAggregationSummaryData {
        total_row_count: frame.height(),
        parsed_row_count: 0,
        first: None,
        last: None,
        by_day: HashMap::new(),
    };
    for row_index in 0..frame.height() {
        if row_index % LOCAL_QUERY_CANCEL_CHECK_ROWS == 0 {
            ensure_not_cancelled(is_cancelled())?;
        }
        let date_value = date_values.get(row_index).map_err(|error| {
            format!("No se pudo leer la columna de fecha '{date_column}': {error}")
        })?;
        let Some(datetime) = quality_datetime_value(date_value) else {
            continue;
        };
        let metric_value = metric_values.get(row_index).map_err(|error| {
            format!("No se pudo leer la columna numérica '{value_column}': {error}")
        })?;
        let key = temporal_period_key(datetime);
        data.first = Some(
            data.first
                .map_or(key, |current: TemporalPeriodKey| current.min(key)),
        );
        data.last = Some(
            data.last
                .map_or(key, |current: TemporalPeriodKey| current.max(key)),
        );
        data.by_day.entry(key).or_default().add_row(metric_value)?;
        data.parsed_row_count = data.parsed_row_count.saturating_add(1);
    }
    ensure_not_cancelled(is_cancelled())?;
    temporal_aggregation_series_from_values(date_column, value_column, aggregation, data)
}

pub(super) fn source_temporal_aggregation_summary<C>(
    path: &Path,
    total_row_count: usize,
    date_column: &str,
    value_column: &str,
    aggregation: TemporalAggregationKind,
    is_cancelled: &C,
) -> Result<TemporalAggregationSeries, String>
where
    C: Fn() -> bool + Sync,
{
    let column_names = [date_column.to_owned(), value_column.to_owned()];
    let mut data = TemporalAggregationSummaryData {
        total_row_count,
        parsed_row_count: 0,
        first: None,
        last: None,
        by_day: HashMap::new(),
    };
    for_each_parquet_columns_block_with_size(
        path,
        total_row_count,
        &column_names,
        SOURCE_PROFILE_BLOCK_ROWS,
        |start, block| {
            let date_values = block
                .columns()
                .first()
                .ok_or_else(|| format!("No se pudo leer la columna de fecha '{date_column}'."))?;
            let metric_values = block
                .columns()
                .get(1)
                .ok_or_else(|| format!("No se pudo leer la columna numérica '{value_column}'."))?;
            for row_index in 0..block.height() {
                if (start + row_index) % LOCAL_QUERY_CANCEL_CHECK_ROWS == 0 {
                    ensure_not_cancelled(is_cancelled())?;
                }
                let date_value = date_values.get(row_index).map_err(|error| {
                    format!("No se pudo leer la columna de fecha '{date_column}': {error}")
                })?;
                let Some(datetime) = quality_datetime_value(date_value) else {
                    continue;
                };
                let metric_value = metric_values.get(row_index).map_err(|error| {
                    format!("No se pudo leer la columna numérica '{value_column}': {error}")
                })?;
                let key = temporal_period_key(datetime);
                data.first = Some(
                    data.first
                        .map_or(key, |current: TemporalPeriodKey| current.min(key)),
                );
                data.last = Some(
                    data.last
                        .map_or(key, |current: TemporalPeriodKey| current.max(key)),
                );
                data.by_day.entry(key).or_default().add_row(metric_value)?;
                data.parsed_row_count = data.parsed_row_count.saturating_add(1);
            }
            Ok(())
        },
    )?;
    ensure_not_cancelled(is_cancelled())?;
    temporal_aggregation_series_from_values(date_column, value_column, aggregation, data)
}

pub(super) fn source_temporal_projection_snapshot<C>(
    source_path: &Path,
    extension: &str,
    date_column: &str,
    value_column: &str,
    is_cancelled: C,
) -> Result<(tempfile::TempDir, PathBuf), String>
where
    C: Fn() -> bool + Send + 'static,
{
    let source_format = match extension {
        "csv" | "tsv" | "txt" => crate::duckdb_query::DuckDbFileFormat::Delimited {
            delimiter: detect_delimiter(source_path, extension)?,
        },
        _ => return Err("El formato source-backed no admite tendencias temporales.".to_owned()),
    };
    let directory = tempfile::tempdir()
        .map_err(|error| format!("No se pudo preparar el agregado temporal: {error}"))?;
    let snapshot = directory.path().join("temporal-aggregation.parquet");
    let projection = format!(
        "{}, {}",
        quote_temporal_sql_identifier(date_column),
        quote_temporal_sql_identifier(value_column)
    );
    crate::duckdb_query::materialize_file_to_parquet_with_projection(
        source_path,
        source_format,
        &snapshot,
        &projection,
        is_cancelled,
    )?;
    Ok((directory, snapshot))
}

pub(super) fn source_profile_snapshot(
    source_path: &Path,
    extension: &str,
) -> Result<(Option<tempfile::TempDir>, PathBuf), String> {
    source_profile_snapshot_with_cancel(source_path, extension, || false)
}

pub(super) fn source_profile_snapshot_with_cancel<C>(
    source_path: &Path,
    extension: &str,
    is_cancelled: C,
) -> Result<(Option<tempfile::TempDir>, PathBuf), String>
where
    C: Fn() -> bool + Clone + Send + Sync + 'static,
{
    ensure_not_cancelled(is_cancelled())?;
    match extension {
        "parquet" => Ok((None, source_path.to_owned())),
        "csv" | "tsv" | "txt" => {
            let (directory, snapshot) = persist_delimited_comparison_source_file_with_cancel(
                source_path,
                extension,
                is_cancelled,
            )?;
            Ok((Some(directory), snapshot))
        }
        _ => Err("El formato no admite un perfil source-backed.".to_owned()),
    }
}

pub(super) fn profile_source_backed_with_progress<F, C>(
    source_path: &Path,
    extension: &str,
    expected_file_size: u64,
    row_count: usize,
    mut report: F,
    is_cancelled: C,
    correlation_sample_rows: usize,
) -> Result<DatasetProfile, String>
where
    F: FnMut(&'static str, u8),
    C: Fn() -> bool + Sync,
{
    ensure_not_cancelled(is_cancelled())?;
    let (_snapshot_directory, snapshot_path) = source_profile_snapshot(source_path, extension)?;
    let schema = read_parquet_schema_frame(&snapshot_path)?;

    let (duplicate_row_count, near_duplicate_row_count, columns, categorical_candidates) =
        profile_source_columns_with_keys(
            &snapshot_path,
            row_count,
            schema.columns(),
            &is_cancelled,
            &mut report,
        )?;
    let duplicate_percentage = if row_count == 0 {
        0.0
    } else {
        (duplicate_row_count as f64 / row_count as f64) * 100.0
    };

    report("Resumiendo categorías", 93);
    let mut categorical = Vec::new();
    for (index, (schema_column, profile)) in schema.columns().iter().zip(&columns).enumerate() {
        if categorical.len() >= MAX_CATEGORICAL_GROUP_COLUMNS {
            break;
        }
        if schema_column.dtype() != &DataType::String
            || profile.empty_count.is_none()
            || profile.suggested_type.is_some()
            || profile.privacy_signal.is_some()
            || profile.unique_count < 2
        {
            continue;
        }
        let Some(candidates) = categorical_candidates.get(index).and_then(Option::as_ref) else {
            continue;
        };
        if let Some(summary) = source_categorical_group_summary(
            &snapshot_path,
            row_count,
            profile,
            candidates,
            &is_cancelled,
        )? {
            categorical.push(summary);
        }
    }
    let categorical_group_summaries = (!categorical.is_empty()).then_some(categorical);

    report("Resumiendo tendencia temporal", 95);
    let mut temporal = Vec::new();
    for (schema_column, profile) in schema.columns().iter().zip(&columns) {
        if temporal.len() >= MAX_TEMPORAL_COLUMNS
            || profile.privacy_signal.is_some()
            || (!matches!(
                schema_column.dtype(),
                DataType::Date | DataType::Datetime(_, _)
            ) && profile.suggested_type.as_deref() != Some("date"))
        {
            continue;
        }
        if let Some(summary) =
            source_temporal_series_summary(&snapshot_path, row_count, profile, &is_cancelled)?
        {
            temporal.push(summary);
        }
    }
    let temporal_series = (!temporal.is_empty()).then_some(temporal);

    report("Calculando correlaciones", 97);
    let numeric_correlations = source_numeric_correlation_matrix(
        &snapshot_path,
        row_count,
        &columns,
        &is_cancelled,
        correlation_sample_rows,
    )?;
    report("Analizando columnas", 100);
    ensure_not_cancelled(is_cancelled())?;
    let final_size = fs::metadata(source_path)
        .map_err(|error| format!("No se pudieron verificar los metadatos source-backed: {error}"))?
        .len();
    if final_size != expected_file_size {
        return Err("El archivo source-backed cambió durante el perfilado.".to_owned());
    }
    Ok(DatasetProfile {
        row_count,
        duplicate_row_count,
        near_duplicate_row_count,
        duplicate_percentage,
        columns,
        numeric_correlations,
        categorical_group_summaries,
        temporal_series,
    })
}

#[cfg(test)]
pub(super) fn profile_dataset(frame: &DataFrame) -> Result<DatasetProfile, String> {
    profile_dataset_with_progress(
        frame,
        |_, _| {},
        || false,
        MAX_NUMERIC_CORRELATION_SAMPLE_ROWS,
    )
}

#[cfg(test)]
pub(super) fn profile_dataset_with_sample_rows(
    frame: &DataFrame,
    sample_rows: usize,
) -> Result<DatasetProfile, String> {
    profile_dataset_with_progress(frame, |_, _| {}, || false, sample_rows)
}
