use super::*;

#[derive(Clone, Debug)]
pub(super) struct LocalJoinQuerySpec {
    pub(super) current_keys: Vec<String>,
    pub(super) compared_keys: Vec<String>,
    pub(super) join_type: DatasetJoinType,
    pub(super) normalized_query: String,
}

#[derive(Clone)]
pub(super) struct SourceBackedJoinContext {
    pub(super) source_path: PathBuf,
    pub(super) source_format: crate::duckdb_query::DuckDbFileFormat,
    pub(super) source_size_bytes: u64,
    pub(super) schema: DataFrame,
    pub(super) file_name: String,
    pub(super) row_count: usize,
    pub(super) original_source_path: PathBuf,
    pub(super) original_file_size_bytes: u64,
    pub(super) history_directory: PathBuf,
    pub(super) snapshot_only: bool,
}

pub(super) struct SourceBackedResultOutput<'a> {
    pub(super) compared_path: &'a Path,
    pub(super) compared_size_bytes: u64,
    pub(super) output_path: &'a Path,
    pub(super) output_row_count: usize,
    pub(super) file_name: &'a str,
    pub(super) label: &'a str,
}

pub(super) struct SourceBackedJoinRequest {
    pub(super) context: SourceBackedJoinContext,
    pub(super) compared_path: PathBuf,
    pub(super) compared_format: crate::duckdb_query::DuckDbFileFormat,
    pub(super) compared_schema: DataFrame,
    pub(super) compared_file_name: String,
    pub(super) compared_size_bytes: u64,
    pub(super) key_columns: Vec<String>,
    pub(super) join_type: DatasetJoinType,
}

pub(super) struct SourceBackedConsolidationRequest {
    pub(super) context: SourceBackedJoinContext,
    pub(super) compared_path: PathBuf,
    pub(super) compared_format: crate::duckdb_query::DuckDbFileFormat,
    pub(super) compared_schema: DataFrame,
    pub(super) compared_file_name: String,
    pub(super) compared_size_bytes: u64,
    pub(super) key_columns: Vec<String>,
}

pub(super) struct SourceBackedConflictResolutionRequest {
    pub(super) context: SourceBackedJoinContext,
    pub(super) compared_path: PathBuf,
    pub(super) compared_size_bytes: u64,
    pub(super) compared_row_count: usize,
    pub(super) compared_schema: DataFrame,
    pub(super) compared_file_name: String,
    pub(super) key_columns: Vec<String>,
    pub(super) decisions: Vec<ConflictResolution>,
}

pub(super) struct SourceBackedJoinOutputGuard {
    path: PathBuf,
    keep_on_drop: bool,
}

impl SourceBackedJoinOutputGuard {
    pub(super) fn new(path: &Path) -> Self {
        Self {
            path: path.to_owned(),
            keep_on_drop: false,
        }
    }

    pub(super) fn keep(&mut self) {
        self.keep_on_drop = true;
    }
}

impl Drop for SourceBackedJoinOutputGuard {
    fn drop(&mut self) {
        if !self.keep_on_drop {
            let _ = fs::remove_file(&self.path);
        }
    }
}

pub(super) fn build_duckdb_join_view_query(
    current: &DataFrame,
    compared: &DataFrame,
    spec: &LocalJoinQuerySpec,
    joined_schema: &DataFrame,
    current_order_column: &str,
    compared_order_column: &str,
    current_row_count: usize,
) -> Result<String, String> {
    let current_names = current
        .get_column_names()
        .iter()
        .map(|name| name.to_string())
        .collect::<HashSet<_>>();
    let compared_names = compared
        .get_column_names()
        .iter()
        .map(|name| name.to_string())
        .collect::<HashSet<_>>();
    let mut projections = joined_schema
        .get_column_names()
        .iter()
        .map(|name| {
            let output_name = name.to_string();
            let expression = if current_names.contains(&output_name) {
                if let Some(key_index) =
                    spec.current_keys.iter().position(|key| key == &output_name)
                {
                    let compared_key = spec
                        .compared_keys
                        .get(key_index)
                        .ok_or_else(|| "Falta una clave derecha para el JOIN DuckDB.".to_owned())?;
                    if spec.join_type == DatasetJoinType::Full {
                        format!(
                            "COALESCE(c.{}, r.{})",
                            duckdb_identifier(&output_name),
                            duckdb_identifier(compared_key)
                        )
                    } else {
                        format!("c.{}", duckdb_identifier(&output_name))
                    }
                } else {
                    format!("c.{}", duckdb_identifier(&output_name))
                }
            } else if compared_names.contains(&output_name) {
                format!("r.{}", duckdb_identifier(&output_name))
            } else if let Some(compared_name) = output_name.strip_suffix("_right") {
                if compared_names.contains(compared_name) {
                    format!("r.{}", duckdb_identifier(compared_name))
                } else {
                    return Err(format!(
                        "No se pudo mapear la columna duplicada '{output_name}' del JOIN DuckDB."
                    ));
                }
            } else {
                return Err(format!(
                    "No se pudo mapear la columna '{output_name}' del JOIN DuckDB."
                ));
            };
            Ok(format!(
                "{expression} AS {}",
                duckdb_identifier(&output_name)
            ))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let join_keyword = match spec.join_type {
        DatasetJoinType::Inner => "INNER JOIN",
        DatasetJoinType::Left => "LEFT JOIN",
        DatasetJoinType::Full => "FULL OUTER JOIN",
    };
    let conditions = spec
        .current_keys
        .iter()
        .zip(&spec.compared_keys)
        .map(|(current_key, compared_key)| {
            format!(
                "c.{} = r.{}",
                duckdb_identifier(current_key),
                duckdb_identifier(compared_key)
            )
        })
        .collect::<Vec<_>>()
        .join(" AND ");
    projections.push(format!(
        "CASE WHEN c.{current_order} IS NULL THEN {} + r.{compared_order} ELSE c.{current_order} END AS {current_order}",
        current_row_count,
        current_order = duckdb_identifier(current_order_column),
        compared_order = duckdb_identifier(compared_order_column),
    ));
    projections.push(format!(
        "r.{compared_order} AS {compared_order}",
        compared_order = duckdb_identifier(compared_order_column),
    ));
    Ok(format!(
        "CREATE VIEW dataset AS SELECT {} FROM __columnia_current AS c {join_keyword} __columnia_compared AS r ON {conditions}",
        projections.join(", ")
    ))
}

pub(super) fn source_backed_join_plan(
    current: &DataFrame,
    compared: &DataFrame,
    key_columns: &[String],
    join_type: DatasetJoinType,
    current_row_count: usize,
) -> Result<(DataFrame, String, String, String, String), String> {
    let joined_schema =
        join_frames_on_keys(current, compared, key_columns, key_columns, join_type)?;
    let mut used_names = current
        .get_column_names()
        .iter()
        .chain(compared.get_column_names().iter())
        .map(|name| name.to_string())
        .collect::<HashSet<_>>();
    let current_order_column =
        unique_duckdb_internal_name(&mut used_names, "__columnia_join_current_order");
    let compared_order_column =
        unique_duckdb_internal_name(&mut used_names, "__columnia_join_compared_order");
    let spec = LocalJoinQuerySpec {
        current_keys: key_columns.to_vec(),
        compared_keys: key_columns.to_vec(),
        join_type,
        normalized_query: "SELECT * FROM dataset".to_owned(),
    };
    let dataset_view_query = build_duckdb_join_view_query(
        current,
        compared,
        &spec,
        &joined_schema,
        &current_order_column,
        &compared_order_column,
        current_row_count,
    )?;
    let projection = joined_schema
        .get_column_names()
        .iter()
        .map(|name| duckdb_identifier(name))
        .collect::<Vec<_>>()
        .join(", ");
    let output_query = format!(
        "SELECT {projection} FROM dataset ORDER BY {}, {}",
        duckdb_identifier(&current_order_column),
        duckdb_identifier(&compared_order_column),
    );
    Ok((
        joined_schema,
        dataset_view_query,
        output_query,
        current_order_column,
        compared_order_column,
    ))
}

pub(super) fn source_backed_consolidation_plan(
    current: &DataFrame,
    compared: &DataFrame,
    key_columns: &[String],
    current_row_count: usize,
) -> Result<(String, String, String, String), String> {
    if current.get_column_names() != compared.get_column_names()
        || current
            .columns()
            .iter()
            .zip(compared.columns())
            .any(|(left, right)| left.dtype() != right.dtype())
    {
        return Err(
            "Los esquemas no son compatibles. La consolidación requiere las mismas columnas y tipos."
                .to_owned(),
        );
    }
    if !key_columns.is_empty() {
        validate_key_columns(current, compared, key_columns)?;
    }
    let mut used_names = current
        .get_column_names()
        .iter()
        .map(|name| name.to_string())
        .collect::<HashSet<_>>();
    let current_order_column =
        unique_duckdb_internal_name(&mut used_names, "__columnia_consolidation_current_order");
    let compared_order_column =
        unique_duckdb_internal_name(&mut used_names, "__columnia_consolidation_compared_order");
    let consolidation_order_column =
        unique_duckdb_internal_name(&mut used_names, "__columnia_consolidation_order");
    let output_projection = current
        .get_column_names()
        .iter()
        .map(|name| duckdb_identifier(name))
        .collect::<Vec<_>>()
        .join(", ");
    let current_projection = current
        .get_column_names()
        .iter()
        .map(|name| {
            let identifier = duckdb_identifier(name);
            format!("c.{identifier} AS {identifier}")
        })
        .collect::<Vec<_>>()
        .join(", ");
    let compared_projection = compared
        .get_column_names()
        .iter()
        .map(|name| {
            let identifier = duckdb_identifier(name);
            format!("r.{identifier} AS {identifier}")
        })
        .collect::<Vec<_>>()
        .join(", ");
    let consolidation_order = duckdb_identifier(&consolidation_order_column);
    let current_order = duckdb_identifier(&current_order_column);
    let compared_order = duckdb_identifier(&compared_order_column);
    let key_predicate = if key_columns.is_empty() {
        "TRUE".to_owned()
    } else {
        let conditions = key_columns
            .iter()
            .map(|key| {
                let identifier = duckdb_identifier(key);
                format!("existing.{identifier} IS NOT DISTINCT FROM r.{identifier}")
            })
            .collect::<Vec<_>>()
            .join(" AND ");
        format!("NOT EXISTS (SELECT 1 FROM __columnia_current AS existing WHERE {conditions})")
    };
    let dataset_view_query = format!(
        "CREATE VIEW dataset AS SELECT {current_projection}, c.{current_order} AS {consolidation_order}, CAST(0 AS BIGINT) AS {compared_order} FROM __columnia_current AS c UNION ALL SELECT {compared_projection}, {current_row_count} + r.{compared_order} AS {consolidation_order}, r.{compared_order} AS {compared_order} FROM __columnia_compared AS r WHERE {key_predicate}"
    );
    let output_query = format!(
        "SELECT {output_projection} FROM dataset ORDER BY {consolidation_order}, {compared_order}"
    );
    Ok((
        dataset_view_query,
        output_query,
        current_order_column,
        compared_order_column,
    ))
}

#[cfg(test)]
pub(super) fn validate_source_backed_consolidation(
    current_path: &Path,
    current_format: crate::duckdb_query::DuckDbFileFormat,
    compared_path: &Path,
    compared_format: crate::duckdb_query::DuckDbFileFormat,
    current: &DataFrame,
    key_columns: &[String],
) -> Result<(), String> {
    validate_source_backed_consolidation_with_cancellation(
        current_path,
        current_format,
        compared_path,
        compared_format,
        current,
        key_columns,
        || false,
    )
}

pub(super) fn validate_source_backed_consolidation_with_cancellation<C>(
    current_path: &Path,
    current_format: crate::duckdb_query::DuckDbFileFormat,
    compared_path: &Path,
    compared_format: crate::duckdb_query::DuckDbFileFormat,
    current: &DataFrame,
    key_columns: &[String],
    is_cancelled: C,
) -> Result<(), String>
where
    C: Fn() -> bool + Send + Clone + 'static,
{
    if key_columns.is_empty() {
        return Ok(());
    }
    let keys = key_columns
        .iter()
        .map(|key| duckdb_identifier(key))
        .collect::<Vec<_>>();
    let group_columns = keys.join(", ");
    let duplicate_query = format!(
        "SELECT COUNT(*) FROM (SELECT {group_columns} FROM __columnia_current GROUP BY {group_columns} HAVING COUNT(*) > 1 UNION ALL SELECT {group_columns} FROM __columnia_compared GROUP BY {group_columns} HAVING COUNT(*) > 1) AS duplicate_groups"
    );
    let duplicate_count = crate::duckdb_query::query_file_sources_scalar_with_cancel(
        crate::duckdb_query::DuckDbFileSourcesScalarQuery {
            current_path,
            current_format,
            compared_path,
            compared_format,
            query: &duplicate_query,
        },
        is_cancelled.clone(),
    )?;
    if duplicate_count > 0 {
        return Err(SOURCE_BACKED_CONSOLIDATION_CONFLICT_ERROR.to_owned());
    }
    let payload_columns = current
        .get_column_names()
        .iter()
        .filter(|name| !key_columns.iter().any(|key| key == name.as_str()))
        .map(|name| name.to_string())
        .collect::<Vec<_>>();
    if payload_columns.is_empty() {
        return Ok(());
    }
    let key_conditions = key_columns
        .iter()
        .map(|key| {
            let identifier = duckdb_identifier(key);
            format!("c.{identifier} IS NOT DISTINCT FROM r.{identifier}")
        })
        .collect::<Vec<_>>()
        .join(" AND ");
    let payload_conditions = payload_columns
        .iter()
        .map(|column| {
            let identifier = duckdb_identifier(column);
            format!("c.{identifier} IS NOT DISTINCT FROM r.{identifier}")
        })
        .collect::<Vec<_>>()
        .join(" AND ");
    let conflict_query = format!(
        "SELECT COUNT(*) FROM __columnia_current AS c JOIN __columnia_compared AS r ON {key_conditions} WHERE NOT ({payload_conditions})"
    );
    let conflict_count = crate::duckdb_query::query_file_sources_scalar_with_cancel(
        crate::duckdb_query::DuckDbFileSourcesScalarQuery {
            current_path,
            current_format,
            compared_path,
            compared_format,
            query: &conflict_query,
        },
        is_cancelled,
    )?;
    if conflict_count > 0 {
        return Err(SOURCE_BACKED_CONSOLIDATION_CONFLICT_ERROR.to_owned());
    }
    Ok(())
}

pub(super) fn local_compare(
    left: AnyValue<'_>,
    right: &str,
    operator: LocalPredicateOperator,
) -> bool {
    let Some(left) = preview_value(left) else {
        return false;
    };
    let ordering = match (left.parse::<f64>(), right.parse::<f64>()) {
        (Ok(left), Ok(right)) => left.partial_cmp(&right),
        _ => Some(left.as_str().cmp(right)),
    };
    match operator {
        LocalPredicateOperator::Eq => ordering == Some(std::cmp::Ordering::Equal),
        LocalPredicateOperator::Neq => ordering != Some(std::cmp::Ordering::Equal),
        LocalPredicateOperator::Gt => ordering == Some(std::cmp::Ordering::Greater),
        LocalPredicateOperator::Gte => matches!(
            ordering,
            Some(std::cmp::Ordering::Greater | std::cmp::Ordering::Equal)
        ),
        LocalPredicateOperator::Lt => ordering == Some(std::cmp::Ordering::Less),
        LocalPredicateOperator::Lte => matches!(
            ordering,
            Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal)
        ),
        LocalPredicateOperator::IsNull | LocalPredicateOperator::IsNotNull => false,
    }
}

pub(super) fn local_predicate_matches(
    frame: &DataFrame,
    row_index: usize,
    predicate: &LocalPredicate,
) -> Result<bool, String> {
    let value = frame
        .column(&predicate.column)
        .map_err(|_| {
            format!(
                "La columna '{}' no existe en el dataset activo.",
                predicate.column
            )
        })?
        .get(row_index)
        .map_err(|error| format!("No se pudo evaluar el filtro local: {error}"))?;
    Ok(match predicate.operator {
        LocalPredicateOperator::IsNull => matches!(value, AnyValue::Null),
        LocalPredicateOperator::IsNotNull => !matches!(value, AnyValue::Null),
        operator => local_compare(
            value,
            predicate.value.as_deref().unwrap_or_default(),
            operator,
        ),
    })
}

pub(super) fn local_row_matches(
    frame: &DataFrame,
    row_index: usize,
    predicates: &[LocalPredicate],
) -> Result<bool, String> {
    for predicate in predicates {
        if !local_predicate_matches(frame, row_index, predicate)? {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(super) fn local_query_block_flags_with_cancel<C>(
    frame: &DataFrame,
    predicates: &[LocalPredicate],
    start: usize,
    end: usize,
    is_cancelled: &C,
) -> Result<Vec<bool>, String>
where
    C: Fn() -> bool + Sync,
{
    (start..end)
        .into_par_iter()
        .map(|row_index| {
            if row_index % LOCAL_QUERY_CANCEL_CHECK_ROWS == 0 {
                ensure_not_cancelled(is_cancelled())?;
            }
            local_row_matches(frame, row_index, predicates)
        })
        .collect::<Result<Vec<_>, _>>()
}

pub(super) fn local_query_block_match_count_with_cancel<C>(
    frame: &DataFrame,
    predicates: &[LocalPredicate],
    start: usize,
    end: usize,
    is_cancelled: &C,
) -> Result<usize, String>
where
    C: Fn() -> bool + Sync,
{
    Ok(
        local_query_block_flags_with_cancel(frame, predicates, start, end, is_cancelled)?
            .into_iter()
            .filter(|matches| *matches)
            .count(),
    )
}

pub(super) fn local_query_block_rows_with_cancel<C>(
    frame: &DataFrame,
    predicates: &[LocalPredicate],
    start: usize,
    end: usize,
    is_cancelled: &C,
) -> Result<Vec<usize>, String>
where
    C: Fn() -> bool + Sync,
{
    Ok(
        local_query_block_flags_with_cancel(frame, predicates, start, end, is_cancelled)?
            .into_iter()
            .enumerate()
            .filter_map(|(offset, matches)| matches.then_some(start + offset))
            .collect(),
    )
}

#[derive(Clone)]
pub(super) struct LocalAggregateState {
    function: LocalAggregate,
    column_name: Option<String>,
    count: usize,
    total: f64,
    numeric_extreme: Option<f64>,
    text_extreme: Option<String>,
    all_numeric: bool,
}

impl LocalAggregateState {
    fn from_projection(
        frame: &DataFrame,
        projection: &LocalProjection,
    ) -> Result<Option<Self>, String> {
        let LocalProjection::Aggregate {
            function, column, ..
        } = projection
        else {
            return Ok(None);
        };
        if let Some(name) = column {
            frame
                .column(name)
                .map_err(|_| format!("La columna '{name}' no existe en el dataset activo."))?;
        }
        Ok(Some(Self {
            function: *function,
            column_name: column.clone(),
            count: 0,
            total: 0.0,
            numeric_extreme: match function {
                LocalAggregate::Minimum => Some(f64::INFINITY),
                LocalAggregate::Maximum => Some(f64::NEG_INFINITY),
                _ => None,
            },
            text_extreme: None,
            all_numeric: true,
        }))
    }

    fn add_row(&mut self, frame: &DataFrame, row_index: usize) -> Result<(), String> {
        if matches!(self.function, LocalAggregate::Count) {
            let include = match self.column_name.as_deref() {
                Some(name) => {
                    frame
                        .column(name)
                        .map_err(|_| {
                            format!("La columna '{name}' no existe en el dataset activo.")
                        })?
                        .get(row_index)
                        .map_err(|error| format!("No se pudo leer la agregación local: {error}"))?
                        != AnyValue::Null
                }
                None => true,
            };
            if include {
                self.count = self.count.checked_add(1).ok_or_else(|| {
                    "La agregación local supera la capacidad de conteo.".to_owned()
                })?;
            }
            return Ok(());
        }

        let column_name = self
            .column_name
            .as_deref()
            .ok_or_else(|| "La agregación necesita una columna válida.".to_owned())?;
        let Some(value) = preview_value(
            frame
                .column(column_name)
                .map_err(|_| format!("La columna '{column_name}' no existe en el dataset activo."))?
                .get(row_index)
                .map_err(|error| format!("No se pudo leer la agregación local: {error}"))?,
        ) else {
            return Ok(());
        };

        self.count = self
            .count
            .checked_add(1)
            .ok_or_else(|| "La agregación local supera la capacidad de conteo.".to_owned())?;
        match self.function {
            LocalAggregate::Sum | LocalAggregate::Average => {
                let number = value.parse::<f64>().map_err(|_| {
                    format!("La columna '{column_name}' debe ser numérica para SUM/AVG.")
                })?;
                self.total += number;
            }
            LocalAggregate::Minimum | LocalAggregate::Maximum => {
                match self.text_extreme.as_mut() {
                    Some(current) => {
                        let replace = if matches!(self.function, LocalAggregate::Minimum) {
                            value.as_str() < current.as_str()
                        } else {
                            value.as_str() > current.as_str()
                        };
                        if replace {
                            *current = value.clone();
                        }
                    }
                    None => self.text_extreme = Some(value.clone()),
                }
                if let Ok(number) = value.parse::<f64>() {
                    let current = self
                        .numeric_extreme
                        .expect("MIN/MAX numérico debe iniciar con un extremo");
                    self.numeric_extreme =
                        Some(if matches!(self.function, LocalAggregate::Minimum) {
                            f64::min(current, number)
                        } else {
                            f64::max(current, number)
                        });
                } else {
                    self.all_numeric = false;
                }
            }
            LocalAggregate::Count => unreachable!("COUNT se resuelve antes"),
        }
        Ok(())
    }

    fn finish(&self) -> Option<String> {
        match self.function {
            LocalAggregate::Count => Some(self.count.to_string()),
            LocalAggregate::Sum => (self.count > 0).then(|| self.total.to_string()),
            LocalAggregate::Average => {
                (self.count > 0).then(|| (self.total / self.count as f64).to_string())
            }
            LocalAggregate::Minimum | LocalAggregate::Maximum => {
                if self.count == 0 {
                    None
                } else if self.all_numeric {
                    Some(
                        self.numeric_extreme
                            .expect("MIN/MAX numérico debe tener valores")
                            .to_string(),
                    )
                } else {
                    self.text_extreme.clone()
                }
            }
        }
    }
}

#[derive(Clone)]
pub(super) struct LocalAggregateAccumulator {
    states: Vec<LocalAggregateState>,
}

impl LocalAggregateAccumulator {
    pub(super) fn new(frame: &DataFrame, projections: &[LocalProjection]) -> Result<Self, String> {
        let mut states = Vec::new();
        for projection in projections {
            if let Some(state) = LocalAggregateState::from_projection(frame, projection)? {
                states.push(state);
            }
        }
        Ok(Self { states })
    }

    fn add_row(&mut self, frame: &DataFrame, row_index: usize) -> Result<(), String> {
        for state in &mut self.states {
            state.add_row(frame, row_index)?;
        }
        Ok(())
    }

    fn finish(&self) -> Vec<Option<String>> {
        self.states
            .iter()
            .map(LocalAggregateState::finish)
            .collect()
    }
}

pub(super) struct LocalAggregateGroup {
    key: Vec<Option<String>>,
    accumulator: LocalAggregateAccumulator,
}

pub(super) struct LocalAggregateExecution {
    matching_count: usize,
    global: Option<LocalAggregateAccumulator>,
    groups: Vec<LocalAggregateGroup>,
}

pub(super) struct LocalAggregateQueryContext<'a, C> {
    projections: &'a [LocalProjection],
    predicates: &'a [LocalPredicate],
    group_by: Option<&'a [String]>,
    is_cancelled: &'a C,
}

pub(super) fn local_query_aggregate_row(
    projections: &[LocalProjection],
    group_by: Option<&[String]>,
    group_values: Option<&[Option<String>]>,
    aggregate_values: &[Option<String>],
) -> Result<Vec<Option<String>>, String> {
    let mut aggregate_index = 0usize;
    projections
        .iter()
        .map(|projection| match projection {
            LocalProjection::Column { name, .. } => {
                let group_index = group_by
                    .and_then(|groups| groups.iter().position(|group| group == name))
                    .ok_or_else(|| format!("La columna '{name}' no es una clave GROUP BY."))?;
                Ok(group_values
                    .and_then(|values| values.get(group_index))
                    .cloned()
                    .flatten())
            }
            LocalProjection::Aggregate { .. } => {
                let value = aggregate_values
                    .get(aggregate_index)
                    .cloned()
                    .ok_or_else(|| "Falta un resultado de agregación local.".to_owned())?;
                aggregate_index += 1;
                Ok(value)
            }
        })
        .collect()
}

pub(super) fn aggregate_local_query_into_execution_with_cancel<C>(
    frame: &DataFrame,
    block_count: usize,
    execution: &mut LocalAggregateExecution,
    positions: &mut HashMap<Vec<Option<String>>, usize>,
    context: &LocalAggregateQueryContext<'_, C>,
) -> Result<(), String>
where
    C: Fn() -> bool + Sync,
{
    let template = LocalAggregateAccumulator::new(frame, context.projections)?;
    let group_columns = context
        .group_by
        .map(|groups| {
            groups
                .iter()
                .map(|name| {
                    frame
                        .column(name)
                        .map(|_| name.clone())
                        .map_err(|_| format!("La columna '{name}' no existe en el dataset activo."))
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?;
    for block_index in 0..block_count {
        let start = block_index * LOCAL_QUERY_BLOCK_ROWS;
        let end = (start + LOCAL_QUERY_BLOCK_ROWS).min(frame.height());
        let flags = local_query_block_flags_with_cancel(
            frame,
            context.predicates,
            start,
            end,
            context.is_cancelled,
        )?;
        for (offset, matches) in flags.into_iter().enumerate() {
            if !matches {
                continue;
            }
            if execution
                .matching_count
                .is_multiple_of(LOCAL_QUERY_CANCEL_CHECK_ROWS)
            {
                ensure_not_cancelled((context.is_cancelled)())?;
            }
            let row_index = start + offset;
            if let Some(accumulator) = execution.global.as_mut() {
                accumulator.add_row(frame, row_index)?;
            } else {
                let columns = group_columns
                    .as_ref()
                    .ok_or_else(|| "GROUP BY requiere columnas válidas.".to_owned())?;
                let key = columns
                    .iter()
                    .map(|name| {
                        frame
                            .column(name)
                            .map_err(|error| format!("No se pudo leer la clave GROUP BY: {error}"))?
                            .get(row_index)
                            .map_err(|error| format!("No se pudo leer la clave GROUP BY: {error}"))
                            .map(preview_value)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let group_index = if let Some(position) = positions.get(&key) {
                    *position
                } else {
                    let position = execution.groups.len();
                    positions.insert(key.clone(), position);
                    execution.groups.push(LocalAggregateGroup {
                        key,
                        accumulator: template.clone(),
                    });
                    position
                };
                execution.groups[group_index]
                    .accumulator
                    .add_row(frame, row_index)?;
            }
            execution.matching_count = execution
                .matching_count
                .checked_add(1)
                .ok_or_else(|| "La agregación local supera la capacidad de conteo.".to_owned())?;
        }
    }
    ensure_not_cancelled((context.is_cancelled)())?;
    Ok(())
}

pub(super) fn aggregate_local_query_state_with_cancel<C>(
    frame: &DataFrame,
    projections: &[LocalProjection],
    predicates: &[LocalPredicate],
    group_by: Option<&[String]>,
    block_count: usize,
    is_cancelled: &C,
) -> Result<LocalAggregateExecution, String>
where
    C: Fn() -> bool + Sync,
{
    let mut execution = LocalAggregateExecution {
        matching_count: 0,
        global: group_by
            .is_none()
            .then_some(LocalAggregateAccumulator::new(frame, projections)?),
        groups: Vec::new(),
    };
    let mut positions = HashMap::new();
    let context = LocalAggregateQueryContext {
        projections,
        predicates,
        group_by,
        is_cancelled,
    };
    aggregate_local_query_into_execution_with_cancel(
        frame,
        block_count,
        &mut execution,
        &mut positions,
        &context,
    )?;
    Ok(execution)
}

pub(super) fn finish_local_aggregate_execution(
    execution: LocalAggregateExecution,
    projections: &[LocalProjection],
    group_by: Option<&[String]>,
) -> Result<Vec<Vec<Option<String>>>, String> {
    if let Some(accumulator) = execution.global {
        let aggregate_values = accumulator.finish();
        return Ok(vec![local_query_aggregate_row(
            projections,
            None,
            None,
            &aggregate_values,
        )?]);
    }

    execution
        .groups
        .into_iter()
        .map(|group| {
            let aggregate_values = group.accumulator.finish();
            local_query_aggregate_row(projections, group_by, Some(&group.key), &aggregate_values)
        })
        .collect()
}

pub(super) fn aggregate_local_query_with_cancel<C>(
    frame: &DataFrame,
    projections: &[LocalProjection],
    predicates: &[LocalPredicate],
    group_by: Option<&[String]>,
    block_count: usize,
    is_cancelled: &C,
) -> Result<Vec<Vec<Option<String>>>, String>
where
    C: Fn() -> bool + Sync,
{
    let execution = aggregate_local_query_state_with_cancel(
        frame,
        projections,
        predicates,
        group_by,
        block_count,
        is_cancelled,
    )?;
    finish_local_aggregate_execution(execution, projections, group_by)
}

pub(super) fn local_query_columns(
    frame: &DataFrame,
    plan: &LocalQueryPlan,
) -> Result<Vec<DatasetColumn>, String> {
    plan.projections
        .iter()
        .map(|projection| match projection {
            LocalProjection::Column { name, output_name } => frame
                .column(name)
                .map(|column| DatasetColumn {
                    name: output_name.clone(),
                    data_type: column.dtype().to_string(),
                })
                .map_err(|_| format!("La columna '{name}' no existe en el dataset activo.")),
            LocalProjection::Aggregate {
                function,
                column,
                output_name,
            } => Ok(DatasetColumn {
                name: output_name.clone(),
                data_type: match function {
                    LocalAggregate::Count => "UInt64".to_owned(),
                    LocalAggregate::Sum | LocalAggregate::Average => "Float64".to_owned(),
                    LocalAggregate::Minimum | LocalAggregate::Maximum => column
                        .as_deref()
                        .and_then(|name| frame.column(name).ok())
                        .map(|column| column.dtype().to_string())
                        .unwrap_or_else(|| "String".to_owned()),
                },
            }),
        })
        .collect()
}

#[cfg(test)]
pub(super) fn execute_local_query(
    frame: &DataFrame,
    query: &str,
) -> Result<DatasetQueryResult, String> {
    let never_cancelled = || false;
    execute_local_query_with_cancel(frame, query, &never_cancelled)
}

pub(super) fn execute_local_query_with_cancel<C>(
    frame: &DataFrame,
    query: &str,
    is_cancelled: &C,
) -> Result<DatasetQueryResult, String>
where
    C: Fn() -> bool + Sync,
{
    let plan = parse_local_query(query, frame)?;
    execute_local_query_plan_with_cancel(frame, &plan, is_cancelled)
}

pub(super) fn read_parquet_query_block(
    path: &Path,
    start: usize,
    length: usize,
) -> Result<DataFrame, String> {
    let slice_offset = i64::try_from(start).map_err(|_| {
        format!(
            "{LOCAL_QUERY_SNAPSHOT_ERROR_PREFIX} el bloque solicitado excede la capacidad del lector."
        )
    })?;
    let plan = parquet_scan(path)
        .map_err(|error| format!("{LOCAL_QUERY_SNAPSHOT_ERROR_PREFIX} {error}"))?
        .slice(slice_offset, length as IdxSize);
    collect_lazy_frame_streaming(
        plan,
        "No se pudo leer el bloque Parquet de la consulta local",
    )
    .map_err(|error| format!("{LOCAL_QUERY_SNAPSHOT_ERROR_PREFIX} {error}"))
}

pub(super) fn read_parquet_query_block_with_cancel<C>(
    path: &Path,
    start: usize,
    length: usize,
    is_cancelled: &C,
) -> Result<DataFrame, String>
where
    C: Fn() -> bool + Sync + ?Sized,
{
    ensure_not_cancelled(is_cancelled())?;
    let slice_offset = i64::try_from(start).map_err(|_| {
        format!(
            "{LOCAL_QUERY_SNAPSHOT_ERROR_PREFIX} el bloque solicitado excede la capacidad del lector."
        )
    })?;
    let plan = parquet_scan(path)
        .map_err(|error| format!("{LOCAL_QUERY_SNAPSHOT_ERROR_PREFIX} {error}"))?
        .slice(slice_offset, length as IdxSize);
    collect_lazy_frame_streaming_with_cancel(
        plan,
        "No se pudo leer el bloque Parquet de la consulta local",
        is_cancelled,
    )
    .map_err(|error| {
        if error == OPERATION_CANCELLED_MESSAGE {
            error
        } else {
            format!("{LOCAL_QUERY_SNAPSHOT_ERROR_PREFIX} {error}")
        }
    })
}

pub(super) fn for_each_parquet_block<F>(
    path: &Path,
    row_count: usize,
    mut visit: F,
) -> Result<(), String>
where
    F: FnMut(usize, &DataFrame) -> Result<(), String>,
{
    for_each_parquet_block_with_size(path, row_count, LOCAL_QUERY_BLOCK_ROWS, &mut visit)
}

pub(super) fn for_each_parquet_block_with_size<F>(
    path: &Path,
    row_count: usize,
    block_rows: usize,
    mut visit: F,
) -> Result<(), String>
where
    F: FnMut(usize, &DataFrame) -> Result<(), String>,
{
    let block_count = row_count.div_ceil(block_rows);
    for block_index in 0..block_count {
        let start = block_index
            .checked_mul(block_rows)
            .ok_or_else(|| {
                format!(
                    "{LOCAL_QUERY_SNAPSHOT_ERROR_PREFIX} el índice del bloque excede la capacidad local."
                )
            })?;
        let length = block_rows.min(row_count - start);
        let block = read_parquet_query_block(path, start, length)?;
        if block.height() != length {
            return Err(format!(
                "{LOCAL_QUERY_SNAPSHOT_ERROR_PREFIX} el snapshot Parquet cambió durante la lectura (se esperaban {length} filas en el bloque y se obtuvieron {}).",
                block.height()
            ));
        }
        visit(start, &block)?;
    }
    let trailing_block = read_parquet_query_block(path, row_count, 1)?;
    if trailing_block.height() != 0 {
        return Err(format!(
            "{LOCAL_QUERY_SNAPSHOT_ERROR_PREFIX} el snapshot Parquet contiene más filas que las registradas."
        ));
    }
    Ok(())
}

pub(super) fn for_each_parquet_block_with_cancel<C, F>(
    path: &Path,
    row_count: usize,
    is_cancelled: &C,
    mut visit: F,
) -> Result<(), String>
where
    C: Fn() -> bool + Sync,
    F: FnMut(usize, &DataFrame) -> Result<(), String>,
{
    let block_count = row_count.div_ceil(LOCAL_QUERY_BLOCK_ROWS);
    for block_index in 0..block_count {
        ensure_not_cancelled(is_cancelled())?;
        let start = block_index
            .checked_mul(LOCAL_QUERY_BLOCK_ROWS)
            .ok_or_else(|| {
                format!(
                    "{LOCAL_QUERY_SNAPSHOT_ERROR_PREFIX} el índice del bloque excede la capacidad local."
                )
            })?;
        let length = LOCAL_QUERY_BLOCK_ROWS.min(row_count - start);
        let block = read_parquet_query_block_with_cancel(path, start, length, is_cancelled)?;
        if block.height() != length {
            return Err(format!(
                "{LOCAL_QUERY_SNAPSHOT_ERROR_PREFIX} el snapshot Parquet cambió durante la lectura (se esperaban {length} filas en el bloque y se obtuvieron {}).",
                block.height()
            ));
        }
        ensure_not_cancelled(is_cancelled())?;
        visit(start, &block)?;
    }
    ensure_not_cancelled(is_cancelled())?;
    let trailing_block = read_parquet_query_block_with_cancel(path, row_count, 1, is_cancelled)?;
    if trailing_block.height() != 0 {
        return Err(format!(
            "{LOCAL_QUERY_SNAPSHOT_ERROR_PREFIX} el snapshot Parquet contiene más filas que las registradas."
        ));
    }
    ensure_not_cancelled(is_cancelled())
}

pub(super) fn read_parquet_columns_block(
    path: &Path,
    column_names: &[String],
    start: usize,
    length: usize,
) -> Result<DataFrame, String> {
    let slice_offset = i64::try_from(start).map_err(|_| {
        format!(
            "{LOCAL_QUERY_SNAPSHOT_ERROR_PREFIX} las columnas solicitadas exceden la capacidad del lector."
        )
    })?;
    let projection = column_names.iter().map(col).collect::<Vec<_>>();
    let plan = parquet_scan(path)
        .map_err(|error| format!("{LOCAL_QUERY_SNAPSHOT_ERROR_PREFIX} {error}"))?
        .select(projection)
        .slice(slice_offset, length as IdxSize);
    collect_lazy_frame_streaming(
        plan,
        "No se pudo leer el bloque de columnas Parquet del perfil",
    )
    .map_err(|error| format!("{LOCAL_QUERY_SNAPSHOT_ERROR_PREFIX} {error}"))
}

pub(super) fn for_each_parquet_columns_block_with_size<F>(
    path: &Path,
    row_count: usize,
    column_names: &[String],
    block_rows: usize,
    mut visit: F,
) -> Result<(), String>
where
    F: FnMut(usize, &DataFrame) -> Result<(), String>,
{
    let block_count = row_count.div_ceil(block_rows);
    for block_index in 0..block_count {
        let start = block_index
            .checked_mul(block_rows)
            .ok_or_else(|| {
                format!(
                    "{LOCAL_QUERY_SNAPSHOT_ERROR_PREFIX} el índice del bloque de columnas excede la capacidad del lector."
                )
            })?;
        let length = block_rows.min(row_count - start);
        let block = read_parquet_columns_block(path, column_names, start, length)?;
        if block.height() != length || block.width() != column_names.len() {
            return Err(
                "No se pudo leer el bloque de columnas Parquet con el tamaño esperado.".to_owned(),
            );
        }
        visit(start, &block)?;
    }
    let trailing_block = read_parquet_columns_block(path, column_names, row_count, 1)?;
    if trailing_block.height() != 0 {
        return Err(
            "El snapshot Parquet contiene más filas que las registradas para las columnas del perfil."
                .to_owned(),
        );
    }
    Ok(())
}

pub(super) fn read_parquet_column_block(
    path: &Path,
    column_name: &str,
    start: usize,
    length: usize,
) -> Result<DataFrame, String> {
    let slice_offset = i64::try_from(start).map_err(|_| {
        format!(
            "{LOCAL_QUERY_SNAPSHOT_ERROR_PREFIX} la columna solicitada excede la capacidad del lector."
        )
    })?;
    let plan = parquet_scan(path)
        .map_err(|error| format!("{LOCAL_QUERY_SNAPSHOT_ERROR_PREFIX} {error}"))?
        .select([col(column_name)])
        .slice(slice_offset, length as IdxSize);
    collect_lazy_frame_streaming(
        plan,
        "No se pudo leer el bloque de columna Parquet del perfil",
    )
}

pub(super) fn for_each_parquet_column_block_with_size<F>(
    path: &Path,
    row_count: usize,
    column_name: &str,
    block_rows: usize,
    mut visit: F,
) -> Result<(), String>
where
    F: FnMut(usize, &Column) -> Result<(), String>,
{
    let block_count = row_count.div_ceil(block_rows);
    for block_index in 0..block_count {
        let start = block_index
            .checked_mul(block_rows)
            .ok_or_else(|| {
                format!(
                    "{LOCAL_QUERY_SNAPSHOT_ERROR_PREFIX} el índice del bloque de columna excede la capacidad del lector."
                )
            })?;
        let length = block_rows.min(row_count - start);
        let block = read_parquet_column_block(path, column_name, start, length)?;
        if block.height() != length || block.width() != 1 {
            return Err(format!(
                "No se pudo leer el bloque de la columna '{column_name}' con el tamaño esperado."
            ));
        }
        let column = block
            .columns()
            .first()
            .ok_or_else(|| format!("No se pudo obtener la columna '{column_name}'."))?;
        visit(start, column)?;
    }
    let trailing_block = read_parquet_column_block(path, column_name, row_count, 1)?;
    if trailing_block.height() != 0 {
        return Err(format!(
            "El snapshot Parquet contiene más filas que las registradas para la columna '{column_name}'."
        ));
    }
    Ok(())
}

pub(super) fn parquet_row_count(path: &Path) -> Result<usize, String> {
    let result = collect_lazy_frame_streaming(
        parquet_scan(path)?.select([len().alias("__row_count")]),
        "No se pudo contar el snapshot Parquet",
    )?;
    let value = result
        .column("__row_count")
        .map_err(|error| format!("No se pudo leer el conteo del snapshot Parquet: {error}"))?
        .get(0)
        .map_err(|error| format!("No se pudo leer el conteo del snapshot Parquet: {error}"))?;
    match value {
        AnyValue::UInt8(value) => Ok(value.into()),
        AnyValue::UInt16(value) => Ok(value.into()),
        AnyValue::UInt32(value) => Ok(value as usize),
        AnyValue::UInt64(value) => usize::try_from(value)
            .map_err(|_| "El conteo del snapshot Parquet excede la capacidad local.".to_owned()),
        AnyValue::UInt128(value) => usize::try_from(value)
            .map_err(|_| "El conteo del snapshot Parquet excede la capacidad local.".to_owned()),
        _ => Err(
            "El motor devolvió un tipo inesperado para el conteo del snapshot Parquet.".to_owned(),
        ),
    }
}

pub(super) fn local_query_has_join(query: &str) -> bool {
    query
        .split_whitespace()
        .any(|token| token.eq_ignore_ascii_case("join"))
}

pub(super) fn should_route_join_to_duckdb(query: &str, has_compared_disk_source: bool) -> bool {
    local_query_has_join(query) && has_compared_disk_source
}

pub(super) fn current_source_backed_context(
    dataset: &LoadedDataset,
) -> Option<(PathBuf, u64, usize)> {
    let source_path = dataset.source_path.as_ref()?;
    let (canonical_source, source_size, _) = validate_dataset_file(source_path).ok()?;
    if source_size != dataset.file_size_bytes {
        return None;
    }
    if let Some(snapshot_path) = dataset.history.source_snapshot_path.as_ref() {
        let (canonical_snapshot, snapshot_size, extension) =
            validate_dataset_file(snapshot_path).ok()?;
        if extension != "parquet" {
            return None;
        }
        return Some((canonical_snapshot, snapshot_size, dataset.row_count));
    }
    Some((canonical_source, source_size, dataset.row_count))
}

pub(super) fn current_history_parquet_snapshot(
    dataset: &LoadedDataset,
) -> Option<(PathBuf, u64, usize)> {
    if !dataset.history.snapshots_enabled {
        return None;
    }
    let current_entry = dataset.history.entries.get(dataset.history.cursor)?;
    let (path, size, extension) = validate_dataset_file(&current_entry.path).ok()?;
    (extension == "parquet").then_some((path, size, dataset.row_count))
}

pub(super) fn current_duckdb_file_source(
    dataset: &LoadedDataset,
) -> Option<(PathBuf, crate::duckdb_query::DuckDbFileFormat)> {
    let (canonical, _, _) = current_source_backed_context(dataset)?;
    let extension = dataset_extension(&canonical).ok()?;
    let format = match extension.as_str() {
        "parquet" => crate::duckdb_query::DuckDbFileFormat::Parquet,
        "csv" | "tsv" | "txt" => {
            let delimiter = detect_delimiter(&canonical, &extension).ok()?;
            if dataset.delimited_header_mode == Some(SpreadsheetHeaderMode::Generated) {
                crate::duckdb_query::DuckDbFileFormat::DelimitedWithoutHeader {
                    delimiter,
                    column_count: dataset.frame.width(),
                }
            } else {
                crate::duckdb_query::DuckDbFileFormat::Delimited { delimiter }
            }
        }
        "json" | "jsonl" | "ndjson" => crate::duckdb_query::DuckDbFileFormat::Json,
        _ => return None,
    };
    Some((canonical, format))
}

pub(super) fn source_backed_join_context(
    dataset: &LoadedDataset,
) -> Option<SourceBackedJoinContext> {
    if !dataset.source_backed {
        return None;
    }
    let (source_path, source_format) = current_duckdb_file_source(dataset)?;
    let source_size_bytes = fs::metadata(&source_path).ok()?.len();
    Some(SourceBackedJoinContext {
        source_path,
        source_format,
        source_size_bytes,
        schema: dataset.frame.clone(),
        file_name: dataset.file_name.clone(),
        row_count: dataset.row_count,
        original_source_path: dataset.source_path.clone()?,
        original_file_size_bytes: dataset.file_size_bytes,
        history_directory: dataset.history.directory.path().to_owned(),
        snapshot_only: false,
    })
}

pub(super) fn snapshot_backed_join_context(
    dataset: &LoadedDataset,
) -> Option<SourceBackedJoinContext> {
    if dataset.source_backed {
        return None;
    }
    let (source_path, source_size_bytes, row_count) = current_history_parquet_snapshot(dataset)?;
    Some(SourceBackedJoinContext {
        source_path: source_path.clone(),
        source_format: crate::duckdb_query::DuckDbFileFormat::Parquet,
        source_size_bytes,
        schema: dataset.frame.slice(0, 0),
        file_name: dataset.file_name.clone(),
        row_count,
        original_source_path: source_path,
        original_file_size_bytes: source_size_bytes,
        history_directory: dataset.history.directory.path().to_owned(),
        snapshot_only: true,
    })
}

pub(super) fn current_join_context(dataset: &LoadedDataset) -> Option<SourceBackedJoinContext> {
    source_backed_join_context(dataset).or_else(|| snapshot_backed_join_context(dataset))
}

pub(super) fn execute_local_query_from_parquet_with_cancel<C>(
    path: &Path,
    row_count: usize,
    query: &str,
    is_cancelled: &C,
) -> Result<DatasetQueryResult, String>
where
    C: Fn() -> bool + Sync,
{
    ensure_not_cancelled(is_cancelled())?;
    let schema = read_parquet_schema_frame(path)
        .map_err(|error| format!("{LOCAL_QUERY_SNAPSHOT_ERROR_PREFIX} {error}"))?;
    let plan = parse_local_query(query, &schema)?;
    let columns = local_query_columns(&schema, &plan)?;
    let block_count = row_count.div_ceil(LOCAL_QUERY_BLOCK_ROWS);
    let mut block_counts = Vec::with_capacity(block_count);

    for block_index in 0..block_count {
        ensure_not_cancelled(is_cancelled())?;
        let start = block_index
            .checked_mul(LOCAL_QUERY_BLOCK_ROWS)
            .ok_or_else(|| {
                format!(
                    "{LOCAL_QUERY_SNAPSHOT_ERROR_PREFIX} el índice del bloque excede la capacidad del lector."
                )
            })?;
        let length = LOCAL_QUERY_BLOCK_ROWS.min(row_count - start);
        let block = read_parquet_query_block_with_cancel(path, start, length, is_cancelled)?;
        if block.height() != length {
            return Err(format!(
                "{LOCAL_QUERY_SNAPSHOT_ERROR_PREFIX} su número de filas no coincide con el dataset activo."
            ));
        }
        block_counts.push(local_query_block_match_count_with_cancel(
            &block,
            &plan.predicates,
            0,
            block.height(),
            is_cancelled,
        )?);
    }
    let trailing_block = read_parquet_query_block_with_cancel(path, row_count, 1, is_cancelled)?;
    if trailing_block.height() != 0 {
        return Err(format!(
            "{LOCAL_QUERY_SNAPSHOT_ERROR_PREFIX} su número de filas no coincide con el dataset activo."
        ));
    }

    let matching_count = block_counts.iter().try_fold(0usize, |total, count| {
        total
            .checked_add(*count)
            .ok_or_else(|| "La consulta local supera la capacidad de conteo.".to_owned())
    })?;
    if plan.aggregate && matching_count > LOCAL_QUERY_AGGREGATE_MAX_MATCHING_ROWS {
        return Err(format!(
            "La agregación local limita las filas coincidentes a {LOCAL_QUERY_AGGREGATE_MAX_MATCHING_ROWS} para proteger la memoria."
        ));
    }
    ensure_not_cancelled(is_cancelled())?;

    if plan.aggregate {
        let mut execution = LocalAggregateExecution {
            matching_count: 0,
            global: plan
                .group_by
                .is_none()
                .then_some(LocalAggregateAccumulator::new(&schema, &plan.projections)?),
            groups: Vec::new(),
        };
        let mut positions = HashMap::new();
        let context = LocalAggregateQueryContext {
            projections: &plan.projections,
            predicates: &plan.predicates,
            group_by: plan.group_by.as_deref(),
            is_cancelled,
        };
        for block_index in 0..block_count {
            ensure_not_cancelled(is_cancelled())?;
            let start = block_index
                .checked_mul(LOCAL_QUERY_BLOCK_ROWS)
                .ok_or_else(|| {
                    format!(
                        "{LOCAL_QUERY_SNAPSHOT_ERROR_PREFIX} el índice del bloque excede la capacidad del lector."
                    )
                })?;
            let length = LOCAL_QUERY_BLOCK_ROWS.min(row_count - start);
            let block = read_parquet_query_block_with_cancel(path, start, length, is_cancelled)?;
            if block.height() != length {
                return Err(format!(
                    "{LOCAL_QUERY_SNAPSHOT_ERROR_PREFIX} su número de filas no coincide con el dataset activo."
                ));
            }
            aggregate_local_query_into_execution_with_cancel(
                &block,
                1,
                &mut execution,
                &mut positions,
                &context,
            )?;
        }
        if execution.matching_count != matching_count {
            return Err(format!(
                "{LOCAL_QUERY_SNAPSHOT_ERROR_PREFIX} cambió durante la consulta."
            ));
        }
        let aggregate_rows = finish_local_aggregate_execution(
            execution,
            &plan.projections,
            plan.group_by.as_deref(),
        )?;
        let row_count = aggregate_rows.len();
        if plan.offset > row_count {
            return Err("La página solicitada está fuera del resultado agregado.".to_owned());
        }
        let end = plan.offset.saturating_add(plan.limit).min(row_count);
        return Ok(DatasetQueryResult {
            columns,
            row_count,
            offset: plan.offset,
            rows: aggregate_rows[plan.offset..end].to_vec(),
            truncated: plan.offset.saturating_add(plan.limit) < row_count,
        });
    }

    if plan.offset > matching_count {
        return Err("La página solicitada está fuera del resultado filtrado.".to_owned());
    }
    let mut offset_in_matches = plan.offset;
    let mut remaining = plan.limit;
    let mut rows = Vec::with_capacity(plan.limit.min(matching_count));
    for (block_index, block_match_count) in block_counts.iter().copied().enumerate() {
        if remaining == 0 {
            break;
        }
        if offset_in_matches >= block_match_count {
            offset_in_matches -= block_match_count;
            continue;
        }

        ensure_not_cancelled(is_cancelled())?;
        let start = block_index
            .checked_mul(LOCAL_QUERY_BLOCK_ROWS)
            .ok_or_else(|| {
                format!(
                    "{LOCAL_QUERY_SNAPSHOT_ERROR_PREFIX} el índice del bloque excede la capacidad del lector."
                )
            })?;
        let length = LOCAL_QUERY_BLOCK_ROWS.min(row_count - start);
        let block = read_parquet_query_block_with_cancel(path, start, length, is_cancelled)?;
        if block.height() != length {
            return Err(format!(
                "{LOCAL_QUERY_SNAPSHOT_ERROR_PREFIX} su número de filas no coincide con el dataset activo."
            ));
        }
        let block_rows = local_query_block_rows_with_cancel(
            &block,
            &plan.predicates,
            0,
            block.height(),
            is_cancelled,
        )?;
        let take = remaining.min(block_match_count - offset_in_matches);
        rows.extend(
            block_rows
                .iter()
                .skip(offset_in_matches)
                .take(take)
                .map(|row_index| {
                    plan.projections
                        .iter()
                        .map(|projection| {
                            let LocalProjection::Column { name, .. } = projection else {
                                return Ok(None);
                            };
                            block
                                .column(name)
                                .map_err(|error| {
                                    format!(
                                        "No se pudo preparar la consulta local desde Parquet: {error}"
                                    )
                                })?
                                .get(*row_index)
                                .map_err(|error| {
                                    format!(
                                        "No se pudo preparar la consulta local desde Parquet: {error}"
                                    )
                                })
                                .map(preview_value)
                        })
                        .collect::<Result<Vec<_>, String>>()
                })
                .collect::<Result<Vec<_>, String>>()?,
        );
        remaining -= take;
        offset_in_matches = 0;
    }
    ensure_not_cancelled(is_cancelled())?;
    Ok(DatasetQueryResult {
        columns,
        row_count: matching_count,
        offset: plan.offset,
        rows,
        truncated: plan.offset.saturating_add(plan.limit) < matching_count,
    })
}

pub(super) fn execute_local_query_plan_with_cancel<C>(
    frame: &DataFrame,
    plan: &LocalQueryPlan,
    is_cancelled: &C,
) -> Result<DatasetQueryResult, String>
where
    C: Fn() -> bool + Sync,
{
    ensure_not_cancelled(is_cancelled())?;
    let block_count = frame.height().div_ceil(LOCAL_QUERY_BLOCK_ROWS);
    let (matching_count, matching_rows) = if plan.aggregate {
        // Count first so aggregate queries validate their row budget before the
        // second pass updates bounded aggregate/group state by block.
        let block_counts = (0..block_count)
            .into_par_iter()
            .map(|block_index| {
                let start = block_index * LOCAL_QUERY_BLOCK_ROWS;
                let end = (start + LOCAL_QUERY_BLOCK_ROWS).min(frame.height());
                local_query_block_match_count_with_cancel(
                    frame,
                    &plan.predicates,
                    start,
                    end,
                    is_cancelled,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let matching_count = block_counts.iter().sum::<usize>();
        if matching_count > LOCAL_QUERY_AGGREGATE_MAX_MATCHING_ROWS {
            return Err(format!(
                "La agregación local limita las filas coincidentes a {LOCAL_QUERY_AGGREGATE_MAX_MATCHING_ROWS} para proteger la memoria."
            ));
        }
        ensure_not_cancelled(is_cancelled())?;
        (matching_count, Vec::new())
    } else {
        // A paged query first counts each block in parallel. Only the block(s)
        // containing the requested window are scanned a second time, so a
        // query over millions of matching rows never indexes every match.
        let block_counts = (0..block_count)
            .into_par_iter()
            .map(|block_index| {
                let start = block_index * LOCAL_QUERY_BLOCK_ROWS;
                let end = (start + LOCAL_QUERY_BLOCK_ROWS).min(frame.height());
                local_query_block_match_count_with_cancel(
                    frame,
                    &plan.predicates,
                    start,
                    end,
                    is_cancelled,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let matching_count = block_counts.iter().sum();
        ensure_not_cancelled(is_cancelled())?;
        if plan.offset > matching_count {
            return Err("La página solicitada está fuera del resultado filtrado.".to_owned());
        }

        let mut offset_in_matches = plan.offset;
        let mut remaining = plan.limit;
        let mut matching_rows = Vec::with_capacity(plan.limit.min(matching_count));
        for (block_index, block_match_count) in block_counts.iter().copied().enumerate() {
            if remaining == 0 {
                break;
            }
            if offset_in_matches >= block_match_count {
                offset_in_matches -= block_match_count;
                continue;
            }

            let start = block_index * LOCAL_QUERY_BLOCK_ROWS;
            let end = (start + LOCAL_QUERY_BLOCK_ROWS).min(frame.height());
            let block_rows = local_query_block_rows_with_cancel(
                frame,
                &plan.predicates,
                start,
                end,
                is_cancelled,
            )?;
            let take = remaining.min(block_match_count - offset_in_matches);
            matching_rows.extend(block_rows.into_iter().skip(offset_in_matches).take(take));
            remaining -= take;
            offset_in_matches = 0;
        }
        (matching_count, matching_rows)
    };
    ensure_not_cancelled(is_cancelled())?;
    let columns = local_query_columns(frame, plan)?;
    let (row_count, rows, offset, truncated) = if plan.aggregate {
        let aggregate_rows = aggregate_local_query_with_cancel(
            frame,
            &plan.projections,
            &plan.predicates,
            plan.group_by.as_deref(),
            block_count,
            is_cancelled,
        )?;
        ensure_not_cancelled(is_cancelled())?;
        let row_count = aggregate_rows.len();
        if plan.offset > row_count {
            return Err("La página solicitada está fuera del resultado agregado.".to_owned());
        }
        let end = plan.offset.saturating_add(plan.limit).min(row_count);
        (
            row_count,
            aggregate_rows[plan.offset..end].to_vec(),
            plan.offset,
            plan.offset.saturating_add(plan.limit) < row_count,
        )
    } else {
        if plan.offset > matching_count {
            return Err("La página solicitada está fuera del resultado filtrado.".to_owned());
        }
        let rows = matching_rows
            .iter()
            .map(|row_index| {
                plan.projections
                    .iter()
                    .map(|projection| {
                        let LocalProjection::Column { name, .. } = projection else {
                            return Ok(None);
                        };
                        frame
                            .column(name)
                            .map_err(|error| {
                                format!("No se pudo preparar la consulta local: {error}")
                            })?
                            .get(*row_index)
                            .map_err(|error| {
                                format!("No se pudo preparar la consulta local: {error}")
                            })
                            .map(preview_value)
                    })
                    .collect::<Result<Vec<_>, String>>()
            })
            .collect::<Result<Vec<_>, _>>()?;
        ensure_not_cancelled(is_cancelled())?;
        (
            matching_count,
            rows,
            plan.offset,
            plan.offset.saturating_add(plan.limit) < matching_count,
        )
    };
    Ok(DatasetQueryResult {
        columns,
        row_count,
        offset,
        rows,
        truncated,
    })
}

#[cfg(test)]
pub(super) fn execute_local_query_with_comparison(
    current: &DataFrame,
    compared: Option<&DataFrame>,
    query: &str,
) -> Result<DatasetQueryResult, String> {
    let never_cancelled = || false;
    execute_local_query_with_comparison_and_cancel(current, compared, query, &never_cancelled)
}

pub(super) fn visit_unmatched_join_right_blocks_with_cancel<C, F>(
    current: &DataFrame,
    compared: &DataFrame,
    current_keys: &[String],
    compared_keys: &[String],
    is_cancelled: &C,
    mut visit: F,
) -> Result<(), String>
where
    C: Fn() -> bool + Sync,
    F: FnMut(DataFrame) -> Result<(), String>,
{
    ensure_not_cancelled(is_cancelled())?;
    if current_keys.len() != compared_keys.len() {
        return Err("El FULL JOIN necesita el mismo número de columnas clave.".to_owned());
    }
    let current_index = spill_key_rows_with_cancel(current, current_keys, is_cancelled)?;
    let compared_block_count = compared.height().div_ceil(LOCAL_QUERY_BLOCK_ROWS);
    for block_index in 0..compared_block_count {
        ensure_not_cancelled(is_cancelled())?;
        let start = block_index
            .checked_mul(LOCAL_QUERY_BLOCK_ROWS)
            .ok_or_else(|| "El índice del bloque del JOIN excede la capacidad local.".to_owned())?;
        let length = LOCAL_QUERY_BLOCK_ROWS.min(compared.height() - start);
        let block = compared.slice(start as i64, length);
        let mut keep = vec![false; block.height()];
        let mut rows_by_bucket = (0..COMPARISON_KEY_BUCKETS)
            .map(|_| Vec::<(usize, String)>::new())
            .collect::<Vec<_>>();

        for (row_index, keep_row) in keep.iter_mut().enumerate() {
            if row_index % LOCAL_QUERY_CANCEL_CHECK_ROWS == 0 {
                ensure_not_cancelled(is_cancelled())?;
            }
            let joinable = compared_keys.iter().try_fold(true, |joinable, key| {
                let value = block
                    .column(key)
                    .map_err(|error| format!("No se pudo leer la clave del FULL JOIN: {error}"))?
                    .get(row_index)
                    .map_err(|error| format!("No se pudo leer la clave del FULL JOIN: {error}"))?;
                Ok::<_, String>(joinable && !matches!(value, AnyValue::Null))
            })?;
            let signature = row_signature(&block, compared_keys, row_index)?;
            if joinable {
                rows_by_bucket[comparison_key_bucket(&signature)].push((row_index, signature));
            } else {
                *keep_row = true;
            }
        }

        for (bucket, rows) in rows_by_bucket.iter().enumerate() {
            if rows.is_empty() {
                continue;
            }
            let current_bucket =
                read_spilled_key_bucket_with_cancel(&current_index, bucket, is_cancelled)?;
            for (row_index, signature) in rows {
                if let Some(keep_row) = keep.get_mut(*row_index) {
                    *keep_row = !current_bucket.contains_key(signature);
                }
            }
        }

        let unmatched = block
            .filter(&BooleanChunked::from_slice(
                "full_join_unmatched".into(),
                &keep,
            ))
            .map_err(|error| {
                format!("No se pudieron seleccionar las filas derechas del FULL JOIN: {error}")
            })?;
        if unmatched.height() > 0 {
            visit(unmatched)?;
        }
    }
    ensure_not_cancelled(is_cancelled())?;
    Ok(())
}

pub(super) fn visit_local_join_blocks_with_cancel<C, F>(
    current: &DataFrame,
    compared: &DataFrame,
    spec: &LocalJoinQuerySpec,
    is_cancelled: &C,
    mut visit: F,
) -> Result<(), String>
where
    C: Fn() -> bool + Sync,
    F: FnMut(DataFrame) -> Result<(), String>,
{
    let left_join_type = match spec.join_type {
        DatasetJoinType::Full => DatasetJoinType::Left,
        join_type => join_type,
    };
    let current_block_count = current.height().div_ceil(LOCAL_QUERY_BLOCK_ROWS);
    for block_index in 0..current_block_count {
        ensure_not_cancelled(is_cancelled())?;
        let start = block_index * LOCAL_QUERY_BLOCK_ROWS;
        let end = (start + LOCAL_QUERY_BLOCK_ROWS).min(current.height());
        let current_block = current.slice(start as i64, end - start);
        let joined_block = collect_join_frame_on_keys_with_cancel(
            &current_block,
            compared,
            &spec.current_keys,
            &spec.compared_keys,
            left_join_type,
            is_cancelled,
        )?;
        visit(joined_block)?;
    }

    if matches!(spec.join_type, DatasetJoinType::Full) {
        let empty_current = current.slice(0, 0);
        let right_order_column = (0..)
            .map(|suffix| {
                if suffix == 0 {
                    "__columnia_full_join_order".to_owned()
                } else {
                    format!("__columnia_full_join_order_{suffix}")
                }
            })
            .find(|candidate| {
                current.get_column_index(candidate).is_none()
                    && compared.get_column_index(candidate).is_none()
            })
            .ok_or_else(|| "No se pudo preparar el orden del FULL JOIN local.".to_owned())?;
        visit_unmatched_join_right_blocks_with_cancel(
            current,
            compared,
            &spec.current_keys,
            &spec.compared_keys,
            is_cancelled,
            |right_block| {
                let right_block = right_block
                    .with_row_index(right_order_column.clone().into(), None)
                    .map_err(|error| {
                        format!("No se pudo preparar el orden del FULL JOIN local: {error}")
                    })?;
                let joined_block = collect_join_frame_on_keys_with_cancel(
                    &empty_current,
                    &right_block,
                    &spec.current_keys,
                    &spec.compared_keys,
                    DatasetJoinType::Full,
                    is_cancelled,
                )?;
                let mut joined_block = joined_block
                    .sort(
                        [right_order_column.as_str()],
                        SortMultipleOptions::default(),
                    )
                    .map_err(|error| {
                        format!("No se pudo conservar el orden del FULL JOIN local: {error}")
                    })?;
                joined_block
                    .drop_in_place(right_order_column.as_str())
                    .map_err(|error| {
                        format!("No se pudo retirar el orden interno del FULL JOIN: {error}")
                    })?;
                visit(joined_block)?;
                Ok(())
            },
        )?;
    }
    ensure_not_cancelled(is_cancelled())?;
    Ok(())
}

pub(super) fn execute_local_join_query_in_blocks_with_cancel<C>(
    current: &DataFrame,
    compared: &DataFrame,
    spec: &LocalJoinQuerySpec,
    is_cancelled: &C,
) -> Result<(Option<DatasetQueryResult>, bool), String>
where
    C: Fn() -> bool + Sync,
{
    if current.height() == 0 {
        return Ok((None, false));
    }

    validate_join_inputs_and_cardinality_with_cancel(
        current,
        compared,
        &spec.current_keys,
        &spec.compared_keys,
        spec.join_type,
        is_cancelled,
    )?;
    ensure_not_cancelled(is_cancelled())?;

    let mut plan = None;
    let mut columns = None;
    let mut matching_count = 0usize;
    let mut offset_in_matches = 0usize;
    let mut remaining = 0usize;
    let mut rows = Vec::new();
    let mut aggregate_execution = None;
    let mut aggregate_positions = HashMap::new();

    visit_local_join_blocks_with_cancel(current, compared, spec, is_cancelled, |joined_block| {
        if plan.is_none() {
            let parsed = parse_local_query(&spec.normalized_query, &joined_block)?;
            columns = Some(local_query_columns(&joined_block, &parsed)?);
            if parsed.aggregate {
                aggregate_execution =
                    Some(LocalAggregateExecution {
                        matching_count: 0,
                        global: parsed.group_by.is_none().then_some(
                            LocalAggregateAccumulator::new(&joined_block, &parsed.projections)?,
                        ),
                        groups: Vec::new(),
                    });
            } else {
                offset_in_matches = parsed.offset;
                remaining = parsed.limit;
                rows = Vec::with_capacity(parsed.limit);
            }
            plan = Some(parsed);
        }

        let base_plan = plan
            .as_ref()
            .expect("la consulta JOIN paginada debe tener un plan");
        if let Some(execution) = aggregate_execution.as_mut() {
            let context = LocalAggregateQueryContext {
                projections: &base_plan.projections,
                predicates: &base_plan.predicates,
                group_by: base_plan.group_by.as_deref(),
                is_cancelled,
            };
            let joined_block_count = joined_block.height().div_ceil(LOCAL_QUERY_BLOCK_ROWS);
            aggregate_local_query_into_execution_with_cancel(
                &joined_block,
                joined_block_count,
                execution,
                &mut aggregate_positions,
                &context,
            )?;
            if execution.matching_count > LOCAL_QUERY_AGGREGATE_MAX_MATCHING_ROWS {
                return Err(format!(
                    "La agregación local limita las filas coincidentes a {LOCAL_QUERY_AGGREGATE_MAX_MATCHING_ROWS} para proteger la memoria."
                ));
            }
            return Ok(());
        }

        let mut probe_plan = base_plan.clone();
        probe_plan.offset = 0;
        probe_plan.limit = if offset_in_matches == 0 && remaining > 0 {
            remaining
        } else {
            1
        };
        let probe = execute_local_query_plan_with_cancel(&joined_block, &probe_plan, is_cancelled)?;
        matching_count = matching_count
            .checked_add(probe.row_count)
            .ok_or_else(|| "La consulta local supera la capacidad de conteo.".to_owned())?;

        if offset_in_matches >= probe.row_count {
            offset_in_matches -= probe.row_count;
            return Ok(());
        }
        if remaining == 0 {
            return Ok(());
        }

        let rows_before = rows.len();
        if offset_in_matches == 0 {
            rows.extend(probe.rows);
        } else {
            let mut page_plan = base_plan.clone();
            page_plan.offset = offset_in_matches;
            page_plan.limit = remaining;
            let page =
                execute_local_query_plan_with_cancel(&joined_block, &page_plan, is_cancelled)?;
            rows.extend(page.rows);
        }
        let added = rows.len().saturating_sub(rows_before);
        remaining = remaining
            .checked_sub(added)
            .expect("la página JOIN no puede exceder su límite");
        offset_in_matches = 0;
        Ok(())
    })?;

    let plan = plan.expect("la consulta JOIN paginada debe visitar un bloque");
    if let Some(execution) = aggregate_execution {
        let aggregate_rows = finish_local_aggregate_execution(
            execution,
            &plan.projections,
            plan.group_by.as_deref(),
        )?;
        let row_count = aggregate_rows.len();
        if plan.offset > row_count {
            return Err("La página solicitada está fuera del resultado agregado.".to_owned());
        }
        let end = plan.offset.saturating_add(plan.limit).min(row_count);
        return Ok((
            Some(DatasetQueryResult {
                columns: columns.expect("la consulta JOIN agregada debe tener columnas"),
                row_count,
                offset: plan.offset,
                rows: aggregate_rows[plan.offset..end].to_vec(),
                truncated: plan.offset.saturating_add(plan.limit) < row_count,
            }),
            true,
        ));
    }

    if plan.offset > matching_count {
        return Err("La página solicitada está fuera del resultado filtrado.".to_owned());
    }
    ensure_not_cancelled(is_cancelled())?;
    Ok((
        Some(DatasetQueryResult {
            columns: columns.expect("la consulta JOIN paginada debe tener columnas"),
            row_count: matching_count,
            offset: plan.offset,
            rows,
            truncated: plan.offset.saturating_add(plan.limit) < matching_count,
        }),
        true,
    ))
}

pub(super) fn execute_local_query_with_comparison_and_cancel<C>(
    current: &DataFrame,
    compared: Option<&DataFrame>,
    query: &str,
    is_cancelled: &C,
) -> Result<DatasetQueryResult, String>
where
    C: Fn() -> bool + Sync,
{
    ensure_not_cancelled(is_cancelled())?;
    if let Some(spec) = parse_local_join_query_spec(query, current, compared)? {
        let mut validated = false;
        if let Some(compared) = compared {
            let (result, validation_done) = execute_local_join_query_in_blocks_with_cancel(
                current,
                compared,
                &spec,
                is_cancelled,
            )?;
            validated = validation_done;
            if let Some(result) = result {
                return Ok(result);
            }
        }
        let compared = compared.expect("la especificación JOIN validó compared");
        let joined = if validated {
            collect_join_frame_on_keys_with_cancel(
                current,
                compared,
                &spec.current_keys,
                &spec.compared_keys,
                spec.join_type,
                is_cancelled,
            )?
        } else {
            join_frames_on_keys_with_cancel(
                current,
                compared,
                &spec.current_keys,
                &spec.compared_keys,
                spec.join_type,
                is_cancelled,
            )?
        };
        execute_local_query_with_cancel(&joined, &spec.normalized_query, is_cancelled)
    } else {
        execute_local_query_with_cancel(current, query, is_cancelled)
    }
}
