use std::collections::HashSet;

use polars::prelude::DataFrame;
use regex::Regex;

use super::{
    build_duckdb_join_view_query, join_frames_on_keys_with_cancel, DatasetJoinType,
    LocalJoinQuerySpec, LOCAL_QUERY_JOIN_MAX_INPUT_ROWS, LOCAL_QUERY_MAX_GROUP_COLUMNS,
    LOCAL_QUERY_MAX_JOIN_COLUMNS, MAX_PAGE_SIZE, MAX_QUERY_CHARS,
};

#[derive(Clone, Copy, Debug)]
pub(super) enum LocalAggregate {
    Count,
    Sum,
    Average,
    Minimum,
    Maximum,
}

#[derive(Clone, Debug)]
pub(super) enum LocalProjection {
    Column {
        name: String,
        output_name: String,
    },
    Aggregate {
        function: LocalAggregate,
        column: Option<String>,
        output_name: String,
    },
}

#[derive(Clone, Copy, Debug)]
pub(super) enum LocalPredicateOperator {
    IsNull,
    IsNotNull,
    Eq,
    Neq,
    Gt,
    Gte,
    Lt,
    Lte,
}

#[derive(Clone, Debug)]
pub(super) struct LocalPredicate {
    pub(super) column: String,
    pub(super) operator: LocalPredicateOperator,
    pub(super) value: Option<String>,
    /// Whether the literal was written as a quoted string rather than a number
    /// or boolean; DuckDB compares both kinds differently.
    pub(super) quoted_value: bool,
}

#[derive(Clone, Debug)]
pub(super) struct LocalQueryPlan {
    pub(super) projections: Vec<LocalProjection>,
    pub(super) predicates: Vec<LocalPredicate>,
    pub(super) group_by: Option<Vec<String>>,
    pub(super) offset: usize,
    pub(super) limit: usize,
    pub(super) aggregate: bool,
}

fn local_identifier(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
        let value = value[1..value.len() - 1].replace("\"\"", "\"");
        if !value.is_empty() {
            return Ok(value);
        }
    } else if !value.is_empty()
        && value
            .chars()
            .all(|character| character.is_alphanumeric() || character == '_' || character == '.')
    {
        return Ok(value.to_owned());
    }
    Err("La consulta solo permite nombres de columnas simples o entre comillas dobles.".to_owned())
}

fn split_local_sql_list(value: &str) -> Result<Vec<String>, String> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut depth = 0usize;
    let mut quoted = false;
    let characters = value.chars().collect::<Vec<_>>();
    for (index, character) in characters.iter().enumerate() {
        match character {
            '\'' => {
                if quoted && characters.get(index + 1) == Some(&'\'') {
                    continue;
                }
                quoted = !quoted;
            }
            '(' if !quoted => depth = depth.saturating_add(1),
            ')' if !quoted => {
                if depth == 0 {
                    return Err("La lista de columnas tiene paréntesis desbalanceados.".to_owned());
                }
                depth -= 1;
            }
            ',' if !quoted && depth == 0 => {
                let part = characters[start..index].iter().collect::<String>();
                if part.trim().is_empty() {
                    return Err("La proyección contiene una expresión vacía.".to_owned());
                }
                parts.push(part);
                start = index + 1;
            }
            _ => {}
        }
    }
    if quoted || depth != 0 {
        return Err("La consulta tiene comillas o paréntesis desbalanceados.".to_owned());
    }
    let part = characters[start..].iter().collect::<String>();
    if part.trim().is_empty() {
        return Err("La proyección contiene una expresión vacía.".to_owned());
    }
    parts.push(part);
    Ok(parts)
}

fn split_local_predicates(value: &str) -> Result<Vec<String>, String> {
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut quoted = false;
    let characters = value.chars().collect::<Vec<_>>();
    let mut index = 0usize;
    while index < characters.len() {
        match characters[index] {
            '\'' => {
                if quoted && characters.get(index + 1) == Some(&'\'') {
                    index += 2;
                    continue;
                }
                quoted = !quoted;
                index += 1;
            }
            _ if !quoted
                && index + 3 <= characters.len()
                && characters[index..index + 3]
                    .iter()
                    .map(|character| character.to_ascii_lowercase())
                    .eq(['a', 'n', 'd'])
                && (index == 0 || characters[index - 1].is_whitespace())
                && (index + 3 == characters.len() || characters[index + 3].is_whitespace()) =>
            {
                let part = characters[start..index].iter().collect::<String>();
                if part.trim().is_empty() {
                    return Err("El filtro contiene una condición vacía.".to_owned());
                }
                parts.push(part);
                index += 3;
                start = index;
            }
            _ => index += 1,
        }
    }
    if quoted {
        return Err("El filtro contiene comillas desbalanceadas.".to_owned());
    }
    let part = characters[start..].iter().collect::<String>();
    if part.trim().is_empty() {
        return Err("El filtro contiene una condición vacía.".to_owned());
    }
    parts.push(part);
    Ok(parts)
}

/// Returns the literal value and whether it was a quoted string. A quoted
/// literal must be a single token: interior quotes are only accepted doubled.
fn parse_local_literal(value: &str) -> Result<(String, bool), String> {
    let value = value.trim();
    let quoted =
        Regex::new(r"^'(?:[^']|'')*'$").expect("el patrón de literal local debe ser válido");
    if quoted.is_match(value) {
        return Ok((value[1..value.len() - 1].replace("''", "'"), true));
    }
    let numeric = Regex::new(r"^-?(?:\d+(?:\.\d*)?|\.\d+)$")
        .expect("el patrón numérico local debe ser válido");
    if numeric.is_match(value) || matches!(value.to_ascii_lowercase().as_str(), "true" | "false") {
        return Ok((value.to_owned(), false));
    }
    Err("Los filtros solo permiten un literal entre comillas, un número o un booleano.".to_owned())
}

fn parse_local_projection(
    projection: &str,
    frame: &DataFrame,
    group_by: Option<&[String]>,
) -> Result<(Vec<LocalProjection>, bool), String> {
    if projection.trim() == "*" {
        return Ok((
            frame
                .get_column_names()
                .iter()
                .map(|name| LocalProjection::Column {
                    name: name.to_string(),
                    output_name: name.to_string(),
                })
                .collect(),
            false,
        ));
    }

    let aggregate_pattern = Regex::new(
        r#"(?is)^\s*(count|sum|avg|average|min|max)\s*\(\s*(\*|(?:\"(?:\"\"|[^\"])+\"|[[:alnum:]_.]+))\s*\)(?:\s+as\s+((?:\"(?:\"\"|[^\"])+\"|[[:alnum:]_.]+)))?\s*$"#,
    )
    .expect("el patrón de agregaciones locales debe ser válido");
    let column_pattern = Regex::new(r#"(?is)^\s*((?:\"(?:\"\"|[^\"])+\"|[[:alnum:]_.]+))\s*$"#)
        .expect("el patrón de columnas locales debe ser válido");
    let mut projections = Vec::new();
    let mut aggregate = false;
    let mut column_projection = false;

    for expression in split_local_sql_list(projection)? {
        if let Some(captures) = aggregate_pattern.captures(&expression) {
            aggregate = true;
            if column_projection
                && (group_by.is_none()
                    || projections.iter().any(|projection| {
                        matches!(projection, LocalProjection::Column { name, .. } if !group_by
                            .is_some_and(|groups| groups.iter().any(|group| group == name)))
                    }))
            {
                return Err(
                    "No mezcles columnas y agregaciones salvo la clave GROUP BY.".to_owned(),
                );
            }
            let function = match captures
                .get(1)
                .expect("la función agregada debe existir")
                .as_str()
                .to_ascii_lowercase()
                .as_str()
            {
                "count" => LocalAggregate::Count,
                "sum" => LocalAggregate::Sum,
                "avg" | "average" => LocalAggregate::Average,
                "min" => LocalAggregate::Minimum,
                "max" => LocalAggregate::Maximum,
                _ => unreachable!("la expresión ya fue validada por el patrón"),
            };
            let argument = captures
                .get(2)
                .expect("el argumento agregado debe existir")
                .as_str();
            let column = if argument == "*" {
                if !matches!(function, LocalAggregate::Count) {
                    return Err("COUNT es la única agregación que permite '*'.".to_owned());
                }
                None
            } else {
                Some(local_identifier(argument)?)
            };
            if let Some(column) = &column {
                frame.column(column).map_err(|_| {
                    format!("La columna '{column}' no existe en el dataset activo.")
                })?;
            }
            let default_name = match (&function, &column) {
                (LocalAggregate::Count, None) => "count".to_owned(),
                (LocalAggregate::Count, Some(column)) => format!("count_{column}"),
                (LocalAggregate::Sum, Some(column)) => format!("sum_{column}"),
                (LocalAggregate::Average, Some(column)) => format!("avg_{column}"),
                (LocalAggregate::Minimum, Some(column)) => format!("min_{column}"),
                (LocalAggregate::Maximum, Some(column)) => format!("max_{column}"),
                _ => unreachable!("las agregaciones no válidas ya fueron rechazadas"),
            };
            let output_name = captures
                .get(3)
                .map(|value| local_identifier(value.as_str()))
                .transpose()?
                .unwrap_or(default_name);
            projections.push(LocalProjection::Aggregate {
                function,
                column,
                output_name,
            });
        } else if let Some(captures) = column_pattern.captures(&expression) {
            column_projection = true;
            let name =
                local_identifier(captures.get(1).expect("la columna debe existir").as_str())?;
            if aggregate
                && (group_by.is_none()
                    || !group_by.is_some_and(|groups| groups.iter().any(|group| group == &name)))
            {
                return Err("No mezcles columnas y agregaciones en una misma consulta.".to_owned());
            }
            frame
                .column(&name)
                .map_err(|_| format!("La columna '{name}' no existe en el dataset activo."))?;
            projections.push(LocalProjection::Column {
                name: name.clone(),
                output_name: name,
            });
        } else {
            return Err("La proyección solo permite columnas o COUNT/SUM/AVG/MIN/MAX.".to_owned());
        }
    }
    Ok((projections, aggregate))
}

#[derive(Clone, Debug)]
struct LocalJoinOperand {
    table: Option<String>,
    column: String,
}

fn parse_local_join_operand(value: &str) -> Result<LocalJoinOperand, String> {
    let identifier = local_identifier(value)?;
    let mut parts = identifier.split('.');
    let first = parts.next().unwrap_or_default();
    let second = parts.next();
    if parts.next().is_some() {
        return Err("El JOIN solo permite columnas de dataset o compared.".to_owned());
    }
    if let Some(column) = second {
        if !matches!(first, "dataset" | "compared") || column.is_empty() {
            return Err("El JOIN solo permite columnas de dataset o compared.".to_owned());
        }
        Ok(LocalJoinOperand {
            table: Some(first.to_owned()),
            column: column.to_owned(),
        })
    } else {
        Ok(LocalJoinOperand {
            table: None,
            column: first.to_owned(),
        })
    }
}

fn parse_local_join_condition(
    condition: &str,
) -> Result<(LocalJoinOperand, LocalJoinOperand), String> {
    let pattern = Regex::new(
        r#"(?is)^\s*((?:\"(?:\"\"|[^\"])+\"|[[:alnum:]_.]+))\s*=\s*((?:\"(?:\"\"|[^\"])+\"|[[:alnum:]_.]+))\s*$"#,
    )
    .expect("el patrón de condición JOIN local debe ser válido");
    let captures = pattern.captures(condition).ok_or_else(|| {
        "Cada condición del JOIN debe comparar una columna de dataset con una de compared."
            .to_owned()
    })?;
    let left = parse_local_join_operand(
        captures
            .get(1)
            .expect("la clave izquierda debe existir")
            .as_str(),
    )?;
    let right = parse_local_join_operand(
        captures
            .get(2)
            .expect("la clave derecha debe existir")
            .as_str(),
    )?;
    Ok((left, right))
}

fn split_local_join_tail(value: &str) -> Result<(&str, &str), String> {
    let characters = value.char_indices().collect::<Vec<_>>();
    let mut single_quoted = false;
    let mut double_quoted = false;
    let mut index = 0usize;
    while index < characters.len() {
        let (byte_index, character) = characters[index];
        if character == '\'' && !double_quoted {
            if single_quoted
                && characters
                    .get(index + 1)
                    .is_some_and(|(_, next)| *next == '\'')
            {
                index += 2;
                continue;
            }
            single_quoted = !single_quoted;
            index += 1;
            continue;
        }
        if character == '"' && !single_quoted {
            if double_quoted
                && characters
                    .get(index + 1)
                    .is_some_and(|(_, next)| *next == '"')
            {
                index += 2;
                continue;
            }
            double_quoted = !double_quoted;
            index += 1;
            continue;
        }
        if !single_quoted
            && !double_quoted
            && character.is_ascii_alphabetic()
            && (index == 0 || characters[index - 1].1.is_whitespace())
        {
            let token_end = (index..characters.len())
                .find(|candidate| {
                    !characters[*candidate].1.is_ascii_alphanumeric()
                        && characters[*candidate].1 != '_'
                })
                .unwrap_or(characters.len());
            let token = value[byte_index
                ..characters
                    .get(token_end)
                    .map(|(byte_index, _)| *byte_index)
                    .unwrap_or(value.len())]
                .to_ascii_lowercase();
            let is_tail_keyword = matches!(token.as_str(), "where" | "limit" | "offset");
            let is_group_by = if token == "group" {
                let mut by_start = token_end;
                while by_start < characters.len() && characters[by_start].1.is_whitespace() {
                    by_start += 1;
                }
                let by_end = (by_start..characters.len())
                    .find(|candidate| {
                        !characters[*candidate].1.is_ascii_alphanumeric()
                            && characters[*candidate].1 != '_'
                    })
                    .unwrap_or(characters.len());
                by_end > by_start
                    && value[characters[by_start].0
                        ..characters
                            .get(by_end)
                            .map(|(byte_index, _)| *byte_index)
                            .unwrap_or(value.len())]
                        .eq_ignore_ascii_case("by")
            } else {
                false
            };
            if is_tail_keyword || is_group_by {
                return Ok((&value[..byte_index], &value[byte_index..]));
            }
            index = token_end;
            continue;
        }
        index += 1;
    }
    if single_quoted || double_quoted {
        return Err("El JOIN contiene comillas desbalanceadas.".to_owned());
    }
    Ok((value, ""))
}

pub(super) fn parse_local_join_query_spec(
    query: &str,
    current: &DataFrame,
    compared: Option<&DataFrame>,
) -> Result<Option<LocalJoinQuerySpec>, String> {
    parse_local_join_query_spec_with_input_limit(query, current, compared, true)
}

fn parse_local_join_query_spec_for_duckdb(
    query: &str,
    current: &DataFrame,
    compared: Option<&DataFrame>,
) -> Result<Option<LocalJoinQuerySpec>, String> {
    parse_local_join_query_spec_with_input_limit(query, current, compared, false)
}

fn parse_local_join_query_spec_with_input_limit(
    query: &str,
    current: &DataFrame,
    compared: Option<&DataFrame>,
    enforce_input_limit: bool,
) -> Result<Option<LocalJoinQuerySpec>, String> {
    let pattern = Regex::new(
        r#"(?is)^\s*select\s+(.+?)\s+from\s+dataset\s+(?:(inner|left|full)\s+)?join\s+compared\s+on\s+(.+)$"#,
    )
    .expect("el patrón de JOIN local debe ser válido");
    let Some(captures) = pattern.captures(query) else {
        if query.to_ascii_lowercase().contains("join") {
            return Err(
                "El JOIN local debe usar FROM dataset JOIN compared ON columna = columna [AND columna = columna]."
                    .to_owned(),
            );
        }
        return Ok(None);
    };
    let compared = compared.ok_or_else(|| {
        "No hay un dataset comparado cargado. Selecciona una fuente en Comparar datasets antes de usar JOIN."
            .to_owned()
    })?;
    if enforce_input_limit
        && current.height().saturating_add(compared.height()) > LOCAL_QUERY_JOIN_MAX_INPUT_ROWS
    {
        return Err(format!(
            "El JOIN local limita las entradas a {LOCAL_QUERY_JOIN_MAX_INPUT_ROWS} filas para proteger la memoria."
        ));
    }

    let (join_clause, query_tail) = split_local_join_tail(
        captures
            .get(3)
            .expect("las condiciones deben existir")
            .as_str(),
    )?;
    let conditions = split_local_predicates(join_clause)?;
    if conditions.is_empty() || conditions.len() > LOCAL_QUERY_MAX_JOIN_COLUMNS {
        return Err(format!(
            "El JOIN requiere entre 1 y {LOCAL_QUERY_MAX_JOIN_COLUMNS} pares de columnas clave."
        ));
    }
    let mut current_keys = Vec::with_capacity(conditions.len());
    let mut compared_keys = Vec::with_capacity(conditions.len());
    for condition in conditions {
        let (left, right) = parse_local_join_condition(&condition)?;
        let left_table = left.table.as_deref().unwrap_or("dataset");
        let right_table = right.table.as_deref().unwrap_or("compared");
        if left_table == right_table {
            return Err(
                "El JOIN debe relacionar una columna de dataset con una de compared.".to_owned(),
            );
        }
        let (current_key, compared_key) = if left_table == "dataset" {
            (left.column, right.column)
        } else {
            (right.column, left.column)
        };
        if current_keys.iter().any(|key| key == &current_key)
            || compared_keys.iter().any(|key| key == &compared_key)
        {
            return Err("Las columnas clave del JOIN no pueden repetirse.".to_owned());
        }
        current_keys.push(current_key);
        compared_keys.push(compared_key);
    }
    let join_type = match captures
        .get(2)
        .map(|value| value.as_str().to_ascii_lowercase())
    {
        Some(value) if value == "left" => DatasetJoinType::Left,
        Some(value) if value == "full" => DatasetJoinType::Full,
        _ => DatasetJoinType::Inner,
    };
    let projection = captures
        .get(1)
        .expect("la proyección debe existir")
        .as_str();
    let normalized_query = if query_tail.trim().is_empty() {
        format!("SELECT {projection} FROM dataset")
    } else {
        format!("SELECT {projection} FROM dataset {}", query_tail.trim())
    };
    Ok(Some(LocalJoinQuerySpec {
        current_keys,
        compared_keys,
        join_type,
        normalized_query,
    }))
}

fn parse_local_predicates(
    where_clause: Option<&str>,
    frame: &DataFrame,
) -> Result<Vec<LocalPredicate>, String> {
    let Some(where_clause) = where_clause else {
        return Ok(Vec::new());
    };
    let null_pattern = Regex::new(r"(?is)^\s*(.+?)\s+is\s+(not\s+)?null\s*$")
        .expect("el patrón de nulos local debe ser válido");
    let comparison_pattern = Regex::new(r"(?is)^\s*(.+?)\s*(<>|!=|>=|<=|=|>|<)\s*(.+?)\s*$")
        .expect("el patrón de comparación local debe ser válido");
    split_local_predicates(where_clause)?
        .into_iter()
        .map(|condition| {
            if let Some(captures) = null_pattern.captures(&condition) {
                let column =
                    local_identifier(captures.get(1).expect("la columna debe existir").as_str())?;
                frame.column(&column).map_err(|_| {
                    format!("La columna '{column}' no existe en el dataset activo.")
                })?;
                return Ok(LocalPredicate {
                    column,
                    operator: if captures.get(2).is_some() {
                        LocalPredicateOperator::IsNotNull
                    } else {
                        LocalPredicateOperator::IsNull
                    },
                    value: None,
                    quoted_value: false,
                });
            }
            let captures = comparison_pattern.captures(&condition).ok_or_else(|| {
                "El filtro solo permite comparaciones simples o IS NULL.".to_owned()
            })?;
            let column =
                local_identifier(captures.get(1).expect("la columna debe existir").as_str())?;
            frame
                .column(&column)
                .map_err(|_| format!("La columna '{column}' no existe en el dataset activo."))?;
            let operator = match captures.get(2).expect("el operador debe existir").as_str() {
                "=" => LocalPredicateOperator::Eq,
                "!=" | "<>" => LocalPredicateOperator::Neq,
                ">" => LocalPredicateOperator::Gt,
                ">=" => LocalPredicateOperator::Gte,
                "<" => LocalPredicateOperator::Lt,
                "<=" => LocalPredicateOperator::Lte,
                _ => unreachable!("el operador ya fue validado por el patrón"),
            };
            let (value, quoted_value) =
                parse_local_literal(captures.get(3).expect("el literal debe existir").as_str())?;
            Ok(LocalPredicate {
                column,
                operator,
                value: Some(value),
                quoted_value,
            })
        })
        .collect()
}

pub(super) fn parse_local_query(query: &str, frame: &DataFrame) -> Result<LocalQueryPlan, String> {
    if query.chars().count() > MAX_QUERY_CHARS {
        return Err(format!(
            "La consulta supera el límite local de {MAX_QUERY_CHARS} caracteres."
        ));
    }
    if query.contains(';') || query.contains("--") || query.contains("/*") || query.contains("*/") {
        return Err(
            "La consulta solo permite una sentencia SELECT sin comentarios ni separadores."
                .to_owned(),
        );
    }
    let pattern = Regex::new(
        r"(?is)^\s*select\s+(.+?)\s+from\s+dataset(?:\s+where\s+(.+?))?(?:\s+group\s+by\s+(.+?))?(?:\s+limit\s+(\d+))?(?:\s+offset\s+(\d+))?\s*$",
    )
    .map_err(|_| "No se pudo preparar el analizador SQL local.".to_owned())?;
    let captures = pattern.captures(query).ok_or_else(|| {
        "Usa SELECT columnas FROM dataset con LIMIT y OFFSET opcionales.".to_owned()
    })?;
    let projection = captures
        .get(1)
        .map(|value| value.as_str().trim())
        .unwrap_or_default();
    let limit = captures
        .get(4)
        .map(|value| value.as_str().parse::<usize>())
        .transpose()
        .map_err(|_| "LIMIT debe ser un entero válido.".to_owned())?
        .unwrap_or(50);
    let offset = captures
        .get(5)
        .map(|value| value.as_str().parse::<usize>())
        .transpose()
        .map_err(|_| "OFFSET debe ser un entero válido.".to_owned())?
        .unwrap_or(0);
    if limit == 0 || limit > MAX_PAGE_SIZE {
        return Err(format!("LIMIT debe estar entre 1 y {MAX_PAGE_SIZE}."));
    }

    let group_by = captures
        .get(3)
        .map(|value| {
            let groups = split_local_sql_list(value.as_str())?
                .into_iter()
                .map(|group| local_identifier(&group))
                .collect::<Result<Vec<_>, _>>()?;
            if groups.is_empty() || groups.len() > LOCAL_QUERY_MAX_GROUP_COLUMNS {
                return Err(format!(
                    "GROUP BY requiere entre 1 y {LOCAL_QUERY_MAX_GROUP_COLUMNS} columnas."
                ));
            }
            let mut unique = HashSet::new();
            for group in &groups {
                if !unique.insert(group) {
                    return Err(format!("La clave GROUP BY '{group}' está duplicada."));
                }
                frame
                    .column(group)
                    .map_err(|_| format!("La columna '{group}' no existe en el dataset activo."))?;
            }
            Ok(groups)
        })
        .transpose()?;
    if let Some(group_by) = &group_by {
        if group_by.is_empty() {
            return Err("GROUP BY necesita al menos una columna.".to_owned());
        }
    }
    let (projections, aggregate) = parse_local_projection(projection, frame, group_by.as_deref())?;
    if group_by.is_some() && !aggregate {
        return Err("GROUP BY necesita al menos una agregación.".to_owned());
    }
    let predicates = parse_local_predicates(captures.get(2).map(|value| value.as_str()), frame)?;
    Ok(LocalQueryPlan {
        projections,
        predicates,
        group_by,
        offset,
        limit,
        aggregate,
    })
}

fn local_query_without_window(query: &str) -> String {
    let limit_pattern = Regex::new(r"(?is)^\s*(.+)\s+limit\s+\d+(?:\s+offset\s+\d+)?\s*$")
        .expect("el patrón LIMIT local debe ser válido");
    if let Some(captures) = limit_pattern.captures(query) {
        return captures
            .get(1)
            .expect("la consulta base debe existir")
            .as_str()
            .trim()
            .to_owned();
    }
    let offset_pattern = Regex::new(r"(?is)^\s*(.+)\s+offset\s+\d+\s*$")
        .expect("el patrón OFFSET local debe ser válido");
    offset_pattern
        .captures(query)
        .and_then(|captures| captures.get(1))
        .map(|value| value.as_str().trim().to_owned())
        .unwrap_or_else(|| query.trim().to_owned())
}

#[cfg(test)]
pub(super) fn prepare_duckdb_query(
    query: &str,
    current: &DataFrame,
    compared: Option<&DataFrame>,
) -> Result<crate::duckdb_query::DuckDbQuerySpec, String> {
    prepare_duckdb_query_with_row_count(query, current, compared, current.height())
}

pub(super) fn prepare_duckdb_query_with_row_count(
    query: &str,
    current: &DataFrame,
    compared: Option<&DataFrame>,
    current_row_count: usize,
) -> Result<crate::duckdb_query::DuckDbQuerySpec, String> {
    // Source-backed schemas intentionally contain zero rows. Keep the real
    // count separate so FULL JOIN rows from the right side sort after every
    // active row without materializing the active dataset just to build SQL.
    let (plan, query_for_duckdb, dataset_view_query, current_order_column, compared_order_column) =
        if let Some(spec) = parse_local_join_query_spec_for_duckdb(query, current, compared)? {
            let compared = compared.expect("la especificación JOIN ya validó compared");
            let empty_current = current.slice(0, 0);
            let empty_compared = compared.slice(0, 0);
            let empty_joined = join_frames_on_keys_with_cancel(
                &empty_current,
                &empty_compared,
                &spec.current_keys,
                &spec.compared_keys,
                spec.join_type,
                &|| false,
            )?;
            let plan = parse_local_query(&spec.normalized_query, &empty_joined)?;
            let mut used_names = current
                .get_column_names()
                .iter()
                .chain(compared.get_column_names().iter())
                .map(|name| name.to_string())
                .collect::<HashSet<_>>();
            let current_order_column =
                unique_duckdb_internal_name(&mut used_names, "__columnia_duckdb_join_order");
            let compared_order_column =
                unique_duckdb_internal_name(&mut used_names, "__columnia_duckdb_join_right_order");
            let dataset_view_query = build_duckdb_join_view_query(
                current,
                compared,
                &spec,
                &empty_joined,
                &current_order_column,
                &compared_order_column,
                current_row_count,
            )?;
            (
                plan,
                spec.normalized_query,
                Some(dataset_view_query),
                Some(current_order_column),
                Some(compared_order_column),
            )
        } else {
            let mut used_names = current
                .get_column_names()
                .iter()
                .map(|name| name.to_string())
                .collect::<HashSet<_>>();
            let current_order_column =
                unique_duckdb_internal_name(&mut used_names, "__columnia_duckdb_order");
            (
                parse_local_query(query, current)?,
                query.to_owned(),
                None,
                Some(current_order_column),
                None,
            )
        };
    let query_for_duckdb = canonicalize_duckdb_query(&query_for_duckdb, &plan)?;
    let base_query = local_query_without_window(&query_for_duckdb);
    if base_query.is_empty() {
        return Err("La consulta local no contiene una sentencia SELECT válida.".to_owned());
    }
    let order_by = if plan.aggregate {
        let mut columns = current_order_column
            .as_deref()
            .map(|column| format!("MIN({})", duckdb_identifier(column)))
            .into_iter()
            .collect::<Vec<_>>();
        if let Some(column) = compared_order_column.as_deref() {
            columns.push(format!("MIN({})", duckdb_identifier(column)));
        }
        format!(" ORDER BY {}", columns.join(", "))
    } else {
        let mut columns = current_order_column
            .as_deref()
            .map(duckdb_identifier)
            .into_iter()
            .collect::<Vec<_>>();
        if let Some(column) = compared_order_column.as_deref() {
            columns.push(duckdb_identifier(column));
        }
        format!(" ORDER BY {}", columns.join(", "))
    };
    Ok(crate::duckdb_query::DuckDbQuerySpec {
        bounded_query: format!(
            "{base_query}{order_by} LIMIT {} OFFSET {}",
            plan.limit, plan.offset
        ),
        count_query: format!("SELECT COUNT(*) FROM ({base_query}) AS __columnia_count"),
        offset: plan.offset,
        limit: plan.limit,
        dataset_view_query,
        current_order_column,
        compared_order_column,
    })
}

pub(super) fn duckdb_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

pub(super) fn unique_duckdb_internal_name(used_names: &mut HashSet<String>, base: &str) -> String {
    if used_names.insert(base.to_owned()) {
        return base.to_owned();
    }
    for suffix in 1.. {
        let candidate = format!("{base}_{suffix}");
        if used_names.insert(candidate.clone()) {
            return candidate;
        }
    }
    unreachable!("el nombre interno DuckDB debe poder acotarse")
}

fn canonicalize_duckdb_query(query: &str, plan: &LocalQueryPlan) -> Result<String, String> {
    let pattern = Regex::new(r"(?is)^\s*select\s+.+?\s+from\s+dataset\b")
        .expect("el patrón de canonicalización DuckDB debe ser válido");
    if !pattern.is_match(query) {
        return Err(
            "La consulta local no contiene una sentencia SELECT válida para DuckDB.".to_owned(),
        );
    }
    let projection = plan
        .projections
        .iter()
        .map(|projection| match projection {
            LocalProjection::Column { name, output_name } => {
                if name == output_name {
                    duckdb_identifier(name)
                } else {
                    format!(
                        "{} AS {}",
                        duckdb_identifier(name),
                        duckdb_identifier(output_name)
                    )
                }
            }
            LocalProjection::Aggregate {
                function,
                column,
                output_name,
            } => {
                let function = match function {
                    LocalAggregate::Count => "COUNT",
                    LocalAggregate::Sum => "SUM",
                    LocalAggregate::Average => "AVG",
                    LocalAggregate::Minimum => "MIN",
                    LocalAggregate::Maximum => "MAX",
                };
                let argument = column
                    .as_deref()
                    .map(duckdb_identifier)
                    .unwrap_or_else(|| "*".to_owned());
                format!(
                    "{function}({argument}) AS {}",
                    duckdb_identifier(output_name)
                )
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    // Rebuild the filter and grouping from the validated plan instead of
    // forwarding the user's text: only identifiers and re-escaped literals
    // reach DuckDB, never an unparsed fragment of the original query.
    let mut canonical = format!("SELECT {projection} FROM dataset");
    if !plan.predicates.is_empty() {
        let predicates = plan
            .predicates
            .iter()
            .map(duckdb_predicate)
            .collect::<Result<Vec<_>, _>>()?;
        canonical.push_str(" WHERE ");
        canonical.push_str(&predicates.join(" AND "));
    }
    if let Some(groups) = &plan.group_by {
        canonical.push_str(" GROUP BY ");
        canonical.push_str(
            &groups
                .iter()
                .map(|group| duckdb_identifier(group))
                .collect::<Vec<_>>()
                .join(", "),
        );
    }
    Ok(canonical)
}

fn duckdb_predicate(predicate: &LocalPredicate) -> Result<String, String> {
    let column = duckdb_identifier(&predicate.column);
    let operator = match predicate.operator {
        LocalPredicateOperator::IsNull => return Ok(format!("{column} IS NULL")),
        LocalPredicateOperator::IsNotNull => return Ok(format!("{column} IS NOT NULL")),
        LocalPredicateOperator::Eq => "=",
        LocalPredicateOperator::Neq => "<>",
        LocalPredicateOperator::Gt => ">",
        LocalPredicateOperator::Gte => ">=",
        LocalPredicateOperator::Lt => "<",
        LocalPredicateOperator::Lte => "<=",
    };
    let value = predicate
        .value
        .as_deref()
        .ok_or_else(|| "La comparación local no tiene un literal válido.".to_owned())?;
    let literal = if predicate.quoted_value {
        format!("'{}'", value.replace('\'', "''"))
    } else {
        // Unquoted literals were already restricted to numbers and booleans.
        let numeric = Regex::new(r"^-?(?:\d+(?:\.\d*)?|\.\d+)$")
            .expect("el patrón numérico local debe ser válido");
        if !numeric.is_match(value)
            && !matches!(value.to_ascii_lowercase().as_str(), "true" | "false")
        {
            return Err("La comparación local no tiene un literal válido.".to_owned());
        }
        value.to_owned()
    };
    Ok(format!("{column} {operator} {literal}"))
}

#[cfg(test)]
mod duckdb_canonical_tests {
    use polars::prelude::*;

    use super::prepare_duckdb_query;

    fn frame() -> DataFrame {
        df!("city" => ["Santo Domingo", "O'Brien"], "value" => [10_i64, 20])
            .expect("frame de prueba")
    }

    #[test]
    fn rejects_a_quoted_literal_that_is_more_than_one_token() {
        let error = prepare_duckdb_query(
            "SELECT city FROM dataset WHERE city = 'a' OR 'b' = 'b'",
            &frame(),
            None,
        )
        .err()
        .expect("un literal con varios tokens debe rechazarse");
        assert!(error.contains("un literal entre comillas"), "{error}");
    }

    #[test]
    fn rebuilds_the_filter_with_identifiers_and_escaped_literals() {
        let spec = prepare_duckdb_query(
            "select city from dataset where city = 'O''Brien' and value >= 10 limit 5",
            &frame(),
            None,
        )
        .expect("consulta válida");
        assert!(
            spec.bounded_query.starts_with(
                r#"SELECT "city" FROM dataset WHERE "city" = 'O''Brien' AND "value" >= 10 ORDER BY"#
            ),
            "{}",
            spec.bounded_query
        );
        assert!(
            spec.bounded_query.ends_with("LIMIT 5 OFFSET 0"),
            "{}",
            spec.bounded_query
        );
    }

    #[test]
    fn rebuilds_group_by_from_the_plan() {
        let spec = prepare_duckdb_query(
            "SELECT city, COUNT(*) AS total FROM dataset WHERE value IS NOT NULL GROUP BY city",
            &frame(),
            None,
        )
        .expect("consulta agregada válida");
        assert!(
            spec.count_query
                .contains(r#"WHERE "value" IS NOT NULL GROUP BY "city""#),
            "{}",
            spec.count_query
        );
    }

    #[test]
    fn no_unparsed_fragment_of_the_query_reaches_duckdb() {
        let spec =
            prepare_duckdb_query("SELECT city FROM dataset WHERE value = 10", &frame(), None)
                .expect("consulta válida");
        assert!(
            !spec.bounded_query.contains("value = 10"),
            "{}",
            spec.bounded_query
        );
        assert!(
            spec.bounded_query.contains(r#""value" = 10"#),
            "{}",
            spec.bounded_query
        );
    }
}
