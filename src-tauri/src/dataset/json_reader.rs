use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum JsonColumnKind {
    Null,
    Boolean,
    Int64,
    Float64,
    String,
}

fn json_value_kind(value: &JsonValue) -> JsonColumnKind {
    match value {
        JsonValue::Null => JsonColumnKind::Null,
        JsonValue::Bool(_) => JsonColumnKind::Boolean,
        JsonValue::Number(number) if number.as_i64().is_some() => JsonColumnKind::Int64,
        JsonValue::Number(number)
            if number
                .to_string()
                .chars()
                .any(|character| matches!(character, '.' | 'e' | 'E'))
                && number.as_f64().is_some_and(f64::is_finite) =>
        {
            JsonColumnKind::Float64
        }
        JsonValue::Number(_)
        | JsonValue::String(_)
        | JsonValue::Array(_)
        | JsonValue::Object(_) => JsonColumnKind::String,
    }
}

fn merge_json_kinds(left: JsonColumnKind, right: JsonColumnKind) -> JsonColumnKind {
    use JsonColumnKind::*;
    match (left, right) {
        (Null, kind) | (kind, Null) => kind,
        (left, right) if left == right => left,
        (Int64, Float64) | (Float64, Int64) => Float64,
        _ => String,
    }
}

fn json_value_as_text(value: &JsonValue) -> Option<String> {
    match value {
        JsonValue::Null => None,
        JsonValue::String(value) => Some(value.clone()),
        other => Some(other.to_string()),
    }
}

fn json_records_to_frame(records: &[JsonMap<String, JsonValue>]) -> Result<DataFrame, String> {
    if records.is_empty() {
        return Err("El JSON no contiene registros.".to_owned());
    }
    let mut seen = HashSet::new();
    let mut names = Vec::new();
    for record in records {
        for name in record.keys() {
            if seen.insert(name.clone()) {
                names.push(name.clone());
            }
        }
    }
    if names.is_empty() {
        return Err("Los registros JSON no contienen campos.".to_owned());
    }

    let mut columns = Vec::with_capacity(names.len());
    for name in names {
        let values = records
            .iter()
            .map(|record| record.get(&name).unwrap_or(&JsonValue::Null))
            .collect::<Vec<_>>();
        let mut kind = values.iter().fold(JsonColumnKind::Null, |kind, value| {
            merge_json_kinds(kind, json_value_kind(value))
        });
        if kind == JsonColumnKind::Float64
            && values.iter().any(|value| {
                value
                    .as_i64()
                    .is_some_and(|number| number.unsigned_abs() > (1_u64 << 53))
            })
        {
            kind = JsonColumnKind::String;
        }
        let column_name = name.as_str().into();
        let column = match kind {
            JsonColumnKind::Null => {
                Column::full_null(column_name, values.len(), &polars::prelude::DataType::Null)
            }
            JsonColumnKind::Boolean => Series::new(
                column_name,
                values
                    .iter()
                    .map(|value| value.as_bool())
                    .collect::<Vec<_>>(),
            )
            .into_column(),
            JsonColumnKind::Int64 => Series::new(
                column_name,
                values
                    .iter()
                    .map(|value| value.as_i64())
                    .collect::<Vec<_>>(),
            )
            .into_column(),
            JsonColumnKind::Float64 => Series::new(
                column_name,
                values
                    .iter()
                    .map(|value| value.as_f64())
                    .collect::<Vec<_>>(),
            )
            .into_column(),
            JsonColumnKind::String => Series::new(
                column_name,
                values
                    .iter()
                    .map(|value| json_value_as_text(value))
                    .collect::<Vec<_>>(),
            )
            .into_column(),
        };
        columns.push(column);
    }
    DataFrame::new(records.len(), columns)
        .map_err(|error| format!("No se pudo construir el dataset JSON: {error}"))
}

struct CancellableJsonRecordsSeed<C> {
    is_cancelled: C,
}

impl<'de, C> DeserializeSeed<'de> for CancellableJsonRecordsSeed<C>
where
    C: Fn() -> bool,
{
    type Value = Vec<JsonMap<String, JsonValue>>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_seq(CancellableJsonRecordsVisitor {
            is_cancelled: self.is_cancelled,
        })
    }
}

struct CancellableJsonRecordsVisitor<C> {
    is_cancelled: C,
}

impl<'de, C> Visitor<'de> for CancellableJsonRecordsVisitor<C>
where
    C: Fn() -> bool,
{
    type Value = Vec<JsonMap<String, JsonValue>>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("un arreglo JSON de objetos")
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        use serde::de::Error;

        let mut records = Vec::new();
        loop {
            if (self.is_cancelled)() {
                return Err(A::Error::custom(OPERATION_CANCELLED_MESSAGE));
            }
            let Some(value) = sequence.next_element::<JsonValue>()? else {
                break;
            };
            if (self.is_cancelled)() {
                return Err(A::Error::custom(OPERATION_CANCELLED_MESSAGE));
            }
            match value {
                JsonValue::Object(record) => records.push(record),
                _ => {
                    return Err(A::Error::custom(format!(
                        "El registro JSON {} no es un objeto.",
                        records.len() + 1
                    )));
                }
            }
        }
        Ok(records)
    }
}

fn first_non_whitespace_json_byte<R>(reader: &mut R) -> std::io::Result<Option<u8>>
where
    R: BufRead,
{
    loop {
        let (whitespace, first, at_eof) = {
            let buffer = reader.fill_buf()?;
            if buffer.is_empty() {
                (0, None, true)
            } else {
                let whitespace = buffer
                    .iter()
                    .take_while(|byte| byte.is_ascii_whitespace())
                    .count();
                (whitespace, buffer.get(whitespace).copied(), false)
            }
        };
        reader.consume(whitespace);
        if first.is_some() || at_eof {
            return Ok(first);
        }
    }
}

#[cfg(test)]
pub(super) fn load_json_records(path: &Path) -> Result<DataFrame, String> {
    load_json_records_with_cancel(path, || false)
}

pub(super) fn load_json_records_with_cancel<C>(
    path: &Path,
    is_cancelled: C,
) -> Result<DataFrame, String>
where
    C: Fn() -> bool,
{
    let file =
        fs::File::open(path).map_err(|error| format!("No se pudo abrir el JSON: {error}"))?;
    let mut reader = BufReader::new(file);
    let first_byte = first_non_whitespace_json_byte(&mut reader)
        .map_err(|error| format!("No se pudo leer el JSON: {error}"))?;
    let cancellation = Arc::new(is_cancelled);
    ensure_not_cancelled(cancellation())?;

    let records = match first_byte {
        Some(b'[') => {
            let mut deserializer = serde_json::Deserializer::from_reader(reader);
            let seed_cancellation = Arc::clone(&cancellation);
            let records = CancellableJsonRecordsSeed {
                is_cancelled: move || seed_cancellation(),
            }
            .deserialize(&mut deserializer)
            .map_err(|error| {
                if cancellation() {
                    OPERATION_CANCELLED_MESSAGE.to_owned()
                } else {
                    format!("El JSON no es válido: {error}")
                }
            })?;
            if deserializer.end().is_err() {
                return Err("Un arreglo JSON debe ser el único valor del archivo.".to_owned());
            }
            records
        }
        Some(b'{') => {
            let mut values = serde_json::Deserializer::from_reader(reader).into_iter::<JsonValue>();
            let first = values
                .next()
                .transpose()
                .map_err(|error| format!("El JSON no es válido: {error}"))?
                .ok_or_else(|| "El archivo JSON está vacío.".to_owned())?;
            ensure_not_cancelled(cancellation())?;
            let JsonValue::Object(record) = first else {
                return Err(
                    "El JSON debe ser un arreglo de objetos o contener un objeto por línea."
                        .to_owned(),
                );
            };
            let mut records = vec![record];
            let mut record_number = 2_usize;
            loop {
                ensure_not_cancelled(cancellation())?;
                let Some(value) = values.next() else {
                    break;
                };
                let value = value.map_err(|error| format!("JSON Lines inválido: {error}"))?;
                ensure_not_cancelled(cancellation())?;
                match value {
                    JsonValue::Object(record) => records.push(record),
                    _ => {
                        return Err(format!(
                            "La línea JSON {record_number} no contiene un objeto."
                        ));
                    }
                }
                record_number = record_number.saturating_add(1);
            }
            records
        }
        None => return Err("El archivo JSON está vacío.".to_owned()),
        _ => {
            return Err(
                "El JSON debe ser un arreglo de objetos o contener un objeto por línea.".to_owned(),
            );
        }
    };
    ensure_not_cancelled(cancellation())?;
    let frame = json_records_to_frame(&records)?;
    ensure_not_cancelled(cancellation())?;
    Ok(frame)
}

struct JsonColumnNamesVisitor<C = fn() -> bool> {
    names: Vec<String>,
    seen: HashSet<String>,
    records: usize,
    is_cancelled: C,
}

fn never_cancel() -> bool {
    false
}

impl Default for JsonColumnNamesVisitor<fn() -> bool> {
    fn default() -> Self {
        Self::new(never_cancel)
    }
}

impl<C> JsonColumnNamesVisitor<C> {
    fn new(is_cancelled: C) -> Self {
        Self {
            names: Vec::new(),
            seen: HashSet::new(),
            records: 0,
            is_cancelled,
        }
    }

    fn add_record(&mut self, record: &JsonMap<String, JsonValue>) {
        for name in record.keys() {
            if self.seen.insert(name.clone()) {
                self.names.push(name.clone());
            }
        }
        self.records = self.records.saturating_add(1);
    }
}

impl<'de, C> Visitor<'de> for JsonColumnNamesVisitor<C>
where
    C: Fn() -> bool,
{
    type Value = Self;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("un arreglo JSON de objetos")
    }

    fn visit_seq<A>(mut self, mut sequence: A) -> Result<Self, A::Error>
    where
        A: SeqAccess<'de>,
    {
        while let Some(record) = sequence.next_element::<JsonMap<String, JsonValue>>()? {
            if self.records.is_multiple_of(LOCAL_QUERY_CANCEL_CHECK_ROWS) && (self.is_cancelled)() {
                return Err(<A::Error as serde::de::Error>::custom(
                    OPERATION_CANCELLED_MESSAGE,
                ));
            }
            self.add_record(&record);
        }
        Ok(self)
    }
}

#[cfg(test)]
pub(super) fn json_record_column_names(path: &Path) -> Result<Vec<String>, String> {
    json_record_column_names_with_cancel(path, || false)
}

pub(super) fn json_record_column_names_with_cancel<C>(
    path: &Path,
    is_cancelled: C,
) -> Result<Vec<String>, String>
where
    C: Fn() -> bool + Clone,
{
    ensure_not_cancelled(is_cancelled())?;
    let mut probe = BufReader::new(
        fs::File::open(path).map_err(|error| format!("No se pudo abrir el JSON: {error}"))?,
    );
    let first_byte = loop {
        ensure_not_cancelled(is_cancelled())?;
        let mut byte = [0_u8; 1];
        let read = probe
            .read(&mut byte)
            .map_err(|error| format!("El JSON no se puede leer: {error}"))?;
        if read == 0 {
            return Err("El archivo JSON está vacío.".to_owned());
        }
        if !byte[0].is_ascii_whitespace() {
            break byte[0];
        }
    };
    drop(probe);

    let file =
        fs::File::open(path).map_err(|error| format!("No se pudo abrir el JSON: {error}"))?;
    if first_byte == b'[' {
        let mut deserializer = serde_json::Deserializer::from_reader(BufReader::new(file));
        let collected_result =
            deserializer.deserialize_any(JsonColumnNamesVisitor::new(is_cancelled.clone()));
        ensure_not_cancelled(is_cancelled())?;
        let collected =
            collected_result.map_err(|error| format!("El JSON no es válido: {error}"))?;
        ensure_not_cancelled(is_cancelled())?;
        deserializer
            .end()
            .map_err(|error| format!("El JSON no es válido: {error}"))?;
        if collected.records == 0 || collected.names.is_empty() {
            return Err("El JSON no contiene registros con campos.".to_owned());
        }
        return Ok(collected.names);
    }

    if first_byte != b'{' {
        return Err(
            "El JSON debe ser un arreglo de objetos o contener un objeto por línea.".to_owned(),
        );
    }
    let values =
        serde_json::Deserializer::from_reader(BufReader::new(file)).into_iter::<JsonValue>();
    let mut collected = JsonColumnNamesVisitor::new(is_cancelled.clone());
    for value in values {
        ensure_not_cancelled(is_cancelled())?;
        let value = value.map_err(|error| format!("JSON Lines inválido: {error}"))?;
        match value {
            JsonValue::Object(record) => collected.add_record(&record),
            _ => {
                return Err(format!(
                    "La línea JSON {} no contiene un objeto.",
                    collected.records.saturating_add(1)
                ));
            }
        }
    }
    if collected.records == 0 || collected.names.is_empty() {
        return Err("El JSON no contiene registros con campos.".to_owned());
    }
    ensure_not_cancelled(is_cancelled())?;
    Ok(collected.names)
}
