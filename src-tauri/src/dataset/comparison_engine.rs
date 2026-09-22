use super::*;

pub(super) fn row_signature(
    frame: &DataFrame,
    columns: &[String],
    row_index: usize,
) -> Result<String, String> {
    let mut signature = String::new();
    for name in columns {
        let value = frame
            .column(name)
            .map_err(|error| format!("No se pudo leer la columna '{name}': {error}"))?
            .get(row_index)
            .map_err(|error| format!("No se pudo comparar la fila {row_index}: {error}"))?;
        match preview_value(value) {
            Some(value) => {
                use std::fmt::Write;
                write!(&mut signature, "v{}:{value};", value.len())
                    .map_err(|_| "No se pudo preparar la comparación.".to_owned())?;
            }
            None => signature.push_str("n;"),
        }
    }
    Ok(signature)
}

pub(super) struct SpilledKeyRows {
    _directory: tempfile::TempDir,
    bucket_paths: Vec<PathBuf>,
}

#[derive(Clone, Copy)]
pub(super) struct KeyRowGroup {
    first_row_index: usize,
    count: usize,
}

pub(super) fn comparison_key_bucket(signature: &str) -> usize {
    (xxh3_64(signature.as_bytes()) as usize) % COMPARISON_KEY_BUCKETS
}

pub(super) fn create_spilled_key_rows() -> Result<SpilledKeyRows, String> {
    let directory = tempfile::tempdir().map_err(|error| {
        format!("No se pudo preparar el índice temporal de comparación: {error}")
    })?;
    let bucket_paths = (0..COMPARISON_KEY_BUCKETS)
        .map(|bucket| directory.path().join(format!("keys-{bucket:03}.bin")))
        .collect::<Vec<_>>();
    Ok(SpilledKeyRows {
        _directory: directory,
        bucket_paths,
    })
}

pub(super) fn spill_key_rows_with_cancel<C>(
    frame: &DataFrame,
    key_columns: &[String],
    is_cancelled: &C,
) -> Result<SpilledKeyRows, String>
where
    C: Fn() -> bool + Sync,
{
    let spill = create_spilled_key_rows()?;
    let mut writers = (0..COMPARISON_KEY_BUCKETS)
        .map(|_| None::<BufWriter<File>>)
        .collect::<Vec<_>>();

    for row_index in 0..frame.height() {
        if row_index % LOCAL_QUERY_CANCEL_CHECK_ROWS == 0 {
            ensure_not_cancelled(is_cancelled())?;
        }
        let signature = row_signature(frame, key_columns, row_index)?;
        let bucket = comparison_key_bucket(&signature);
        let writer = if let Some(writer) = writers[bucket].as_mut() {
            writer
        } else {
            let file = File::create(&spill.bucket_paths[bucket]).map_err(|error| {
                format!("No se pudo crear el índice temporal de comparación: {error}")
            })?;
            writers[bucket].get_or_insert_with(|| BufWriter::with_capacity(64 * 1024, file))
        };
        let key_bytes = signature.as_bytes();
        let key_length = u64::try_from(key_bytes.len()).map_err(|_| {
            "La clave de comparación supera la capacidad del índice temporal.".to_owned()
        })?;
        let row_number = u64::try_from(row_index)
            .map_err(|_| "El índice de fila supera la capacidad del índice temporal.".to_owned())?;
        writer
            .write_all(&key_length.to_le_bytes())
            .and_then(|_| writer.write_all(&row_number.to_le_bytes()))
            .and_then(|_| writer.write_all(key_bytes))
            .map_err(|error| {
                format!("No se pudo escribir el índice temporal de comparación: {error}")
            })?;
    }

    for writer in writers.iter_mut().flatten() {
        writer.flush().map_err(|error| {
            format!("No se pudo sincronizar el índice temporal de comparación: {error}")
        })?;
    }
    ensure_not_cancelled(is_cancelled())?;
    drop(writers);
    Ok(spill)
}

pub(super) fn spill_key_rows(
    frame: &DataFrame,
    key_columns: &[String],
) -> Result<SpilledKeyRows, String> {
    spill_key_rows_with_cancel(frame, key_columns, &|| false)
}

pub(super) fn append_spilled_key_rows(
    spill: &SpilledKeyRows,
    frame: &DataFrame,
    key_columns: &[String],
    row_offset: usize,
) -> Result<(), String> {
    let mut writers = (0..COMPARISON_KEY_BUCKETS)
        .map(|_| None::<BufWriter<File>>)
        .collect::<Vec<_>>();
    append_spilled_key_rows_with_writers(spill, frame, key_columns, row_offset, &mut writers)?;

    for writer in writers.iter_mut().flatten() {
        writer.flush().map_err(|error| {
            format!("No se pudo sincronizar el índice temporal de comparación: {error}")
        })?;
    }
    Ok(())
}

pub(super) fn append_spilled_key_rows_with_writers(
    spill: &SpilledKeyRows,
    frame: &DataFrame,
    key_columns: &[String],
    row_offset: usize,
    writers: &mut [Option<BufWriter<File>>],
) -> Result<(), String> {
    for row_index in 0..frame.height() {
        let signature = row_signature(frame, key_columns, row_index)?;
        let bucket = comparison_key_bucket(&signature);
        let writer = if let Some(writer) = writers[bucket].as_mut() {
            writer
        } else {
            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&spill.bucket_paths[bucket])
                .map_err(|error| {
                    format!("No se pudo abrir el índice temporal de comparación: {error}")
                })?;
            writers[bucket].get_or_insert_with(|| BufWriter::with_capacity(64 * 1024, file))
        };
        let key_bytes = signature.as_bytes();
        let key_length = u64::try_from(key_bytes.len()).map_err(|_| {
            "La clave de comparación supera la capacidad del índice temporal.".to_owned()
        })?;
        let row_number = row_offset
            .checked_add(row_index)
            .and_then(|index| u64::try_from(index).ok())
            .ok_or_else(|| {
                "El índice de fila supera la capacidad del índice temporal.".to_owned()
            })?;
        writer
            .write_all(&key_length.to_le_bytes())
            .and_then(|_| writer.write_all(&row_number.to_le_bytes()))
            .and_then(|_| writer.write_all(key_bytes))
            .map_err(|error| {
                format!("No se pudo escribir el índice temporal de comparación: {error}")
            })?;
    }
    Ok(())
}

pub(super) fn spill_parquet_rows(
    path: &Path,
    row_count: usize,
    columns: &[String],
) -> Result<SpilledKeyRows, String> {
    spill_parquet_rows_with_cancel(path, row_count, columns, &|| false)
}

pub(super) fn spill_parquet_rows_with_cancel<C>(
    path: &Path,
    row_count: usize,
    columns: &[String],
    is_cancelled: &C,
) -> Result<SpilledKeyRows, String>
where
    C: Fn() -> bool + Sync,
{
    ensure_not_cancelled(is_cancelled())?;
    let spill = create_spilled_key_rows()?;
    for_each_parquet_block_with_cancel(path, row_count, is_cancelled, |start, block| {
        append_spilled_key_rows(&spill, block, columns, start)
    })?;
    ensure_not_cancelled(is_cancelled())?;
    Ok(spill)
}

pub(super) struct SpilledKeyPayloadRows {
    _directory: tempfile::TempDir,
    bucket_paths: Vec<PathBuf>,
}

#[derive(Clone)]
pub(super) struct KeyPayloadGroup {
    first_row_index: usize,
    count: usize,
    payload: Option<String>,
}

pub(super) fn create_spilled_key_payload_rows() -> Result<SpilledKeyPayloadRows, String> {
    let directory = tempfile::tempdir().map_err(|error| {
        format!("No se pudo preparar el índice temporal de comparación por clave: {error}")
    })?;
    let bucket_paths = (0..COMPARISON_KEY_BUCKETS)
        .map(|bucket| {
            directory
                .path()
                .join(format!("payload-keys-{bucket:03}.bin"))
        })
        .collect::<Vec<_>>();
    Ok(SpilledKeyPayloadRows {
        _directory: directory,
        bucket_paths,
    })
}

pub(super) fn append_spilled_key_payload_rows_with_cancel<C>(
    spill: &SpilledKeyPayloadRows,
    frame: &DataFrame,
    key_columns: &[String],
    payload_columns: &[String],
    row_offset: usize,
    is_cancelled: &C,
) -> Result<(), String>
where
    C: Fn() -> bool + Sync,
{
    let mut writers = (0..COMPARISON_KEY_BUCKETS)
        .map(|_| None::<BufWriter<File>>)
        .collect::<Vec<_>>();

    for row_index in 0..frame.height() {
        if row_index.is_multiple_of(LOCAL_QUERY_CANCEL_CHECK_ROWS) {
            ensure_not_cancelled(is_cancelled())?;
        }
        let key = row_signature(frame, key_columns, row_index)?;
        let payload = row_signature(frame, payload_columns, row_index)?;
        let bucket = comparison_key_bucket(&key);
        let writer = if let Some(writer) = writers[bucket].as_mut() {
            writer
        } else {
            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&spill.bucket_paths[bucket])
                .map_err(|error| {
                    format!("No se pudo abrir el índice temporal de comparación por clave: {error}")
                })?;
            writers[bucket].get_or_insert_with(|| BufWriter::with_capacity(64 * 1024, file))
        };
        let key_bytes = key.as_bytes();
        let payload_bytes = payload.as_bytes();
        let key_length = u64::try_from(key_bytes.len()).map_err(|_| {
            "La clave de comparación supera la capacidad del índice temporal.".to_owned()
        })?;
        let payload_length = u64::try_from(payload_bytes.len()).map_err(|_| {
            "La carga de comparación supera la capacidad del índice temporal.".to_owned()
        })?;
        let row_number = row_offset
            .checked_add(row_index)
            .and_then(|index| u64::try_from(index).ok())
            .ok_or_else(|| {
                "El índice de fila supera la capacidad del índice temporal.".to_owned()
            })?;
        writer
            .write_all(&key_length.to_le_bytes())
            .and_then(|_| writer.write_all(&payload_length.to_le_bytes()))
            .and_then(|_| writer.write_all(&row_number.to_le_bytes()))
            .and_then(|_| writer.write_all(key_bytes))
            .and_then(|_| writer.write_all(payload_bytes))
            .map_err(|error| {
                format!("No se pudo escribir el índice temporal de comparación por clave: {error}")
            })?;
    }

    for writer in writers.iter_mut().flatten() {
        writer.flush().map_err(|error| {
            format!("No se pudo sincronizar el índice temporal de comparación por clave: {error}")
        })?;
    }
    ensure_not_cancelled(is_cancelled())
}

pub(super) fn spill_parquet_key_payload_rows(
    path: &Path,
    row_count: usize,
    key_columns: &[String],
    payload_columns: &[String],
) -> Result<SpilledKeyPayloadRows, String> {
    spill_parquet_key_payload_rows_with_cancel(
        path,
        row_count,
        key_columns,
        payload_columns,
        &|| false,
    )
}

pub(super) fn spill_parquet_key_payload_rows_with_cancel<C>(
    path: &Path,
    row_count: usize,
    key_columns: &[String],
    payload_columns: &[String],
    is_cancelled: &C,
) -> Result<SpilledKeyPayloadRows, String>
where
    C: Fn() -> bool + Sync,
{
    let spill = create_spilled_key_payload_rows()?;
    for_each_parquet_block_with_cancel(path, row_count, is_cancelled, |start, block| {
        append_spilled_key_payload_rows_with_cancel(
            &spill,
            block,
            key_columns,
            payload_columns,
            start,
            is_cancelled,
        )
    })?;
    Ok(spill)
}

pub(super) fn for_each_spilled_key_payload_record_with_cancel<C, F>(
    spill: &SpilledKeyPayloadRows,
    bucket: usize,
    is_cancelled: &C,
    mut visit: F,
) -> Result<(), String>
where
    C: Fn() -> bool + Sync,
    F: FnMut(String, String, usize) -> Result<(), String>,
{
    let path = spill
        .bucket_paths
        .get(bucket)
        .ok_or_else(|| "La cubeta del índice de comparación por clave no existe.".to_owned())?;
    if !path.exists() {
        return Ok(());
    }
    let bytes = fs::metadata(path)
        .map_err(|error| {
            format!("No se pudo inspeccionar el índice temporal de comparación por clave: {error}")
        })?
        .len();
    let record_header_bytes = (std::mem::size_of::<u64>() * 3) as u64;
    if bytes < record_header_bytes {
        return Err("El índice temporal de comparación por clave quedó incompleto.".to_owned());
    }

    let file = File::open(path).map_err(|error| {
        format!("No se pudo leer el índice temporal de comparación por clave: {error}")
    })?;
    let mut reader = BufReader::new(file);
    let mut record_index = 0usize;
    loop {
        if record_index.is_multiple_of(LOCAL_QUERY_CANCEL_CHECK_ROWS) {
            ensure_not_cancelled(is_cancelled())?;
        }
        let mut first_byte = [0_u8; 1];
        if reader.read(&mut first_byte).map_err(|error| {
            format!("No se pudo leer el índice temporal de comparación por clave: {error}")
        })? == 0
        {
            break;
        }
        let mut key_length_bytes = [0_u8; std::mem::size_of::<u64>()];
        key_length_bytes[0] = first_byte[0];
        reader.read_exact(&mut key_length_bytes[1..]).map_err(|_| {
            "El índice temporal de comparación por clave quedó incompleto.".to_owned()
        })?;
        let key_length = usize::try_from(u64::from_le_bytes(key_length_bytes))
            .map_err(|_| "La clave temporal excede la capacidad local.".to_owned())?;
        let mut payload_length_bytes = [0_u8; std::mem::size_of::<u64>()];
        reader.read_exact(&mut payload_length_bytes).map_err(|_| {
            "El índice temporal de comparación por clave quedó incompleto.".to_owned()
        })?;
        let payload_length = usize::try_from(u64::from_le_bytes(payload_length_bytes))
            .map_err(|_| "La carga temporal excede la capacidad local.".to_owned())?;
        let mut row_bytes = [0_u8; std::mem::size_of::<u64>()];
        reader.read_exact(&mut row_bytes).map_err(|_| {
            "El índice temporal de comparación por clave quedó incompleto.".to_owned()
        })?;
        let row_index = usize::try_from(u64::from_le_bytes(row_bytes))
            .map_err(|_| "El índice de fila temporal excede la capacidad local.".to_owned())?;
        let mut key_bytes = vec![0_u8; key_length];
        reader.read_exact(&mut key_bytes).map_err(|_| {
            "El índice temporal de comparación por clave quedó incompleto.".to_owned()
        })?;
        let key = String::from_utf8(key_bytes)
            .map_err(|_| "El índice temporal contiene una clave inválida.".to_owned())?;
        let mut payload_bytes = vec![0_u8; payload_length];
        reader.read_exact(&mut payload_bytes).map_err(|_| {
            "El índice temporal de comparación por clave quedó incompleto.".to_owned()
        })?;
        let payload = String::from_utf8(payload_bytes)
            .map_err(|_| "El índice temporal contiene una carga inválida.".to_owned())?;
        visit(key, payload, row_index)?;
        record_index = record_index
            .checked_add(1)
            .ok_or_else(|| "La cubeta temporal supera la capacidad local.".to_owned())?;
    }
    ensure_not_cancelled(is_cancelled())
}

pub(super) fn read_spilled_key_payload_bucket(
    spill: &SpilledKeyPayloadRows,
    bucket: usize,
) -> Result<HashMap<String, KeyPayloadGroup>, String> {
    read_spilled_key_payload_bucket_with_cancel(spill, bucket, &|| false)
}

pub(super) fn read_spilled_key_payload_bucket_with_cancel<C>(
    spill: &SpilledKeyPayloadRows,
    bucket: usize,
    is_cancelled: &C,
) -> Result<HashMap<String, KeyPayloadGroup>, String>
where
    C: Fn() -> bool + Sync,
{
    let mut groups = HashMap::new();
    for_each_spilled_key_payload_record_with_cancel(
        spill,
        bucket,
        is_cancelled,
        |key, payload, row_index| {
            let group = groups.entry(key).or_insert_with(|| KeyPayloadGroup {
                first_row_index: row_index,
                count: 0,
                payload: Some(payload.clone()),
            });
            group.count = group
                .count
                .checked_add(1)
                .ok_or_else(|| "La clave temporal supera la capacidad local.".to_owned())?;
            if group.count > 1 {
                group.payload = None;
            }
            Ok(())
        },
    )?;
    Ok(groups)
}

pub(super) fn for_each_spilled_key_record<F>(
    spill: &SpilledKeyRows,
    bucket: usize,
    visit: F,
) -> Result<(), String>
where
    F: FnMut(String, usize) -> Result<(), String>,
{
    for_each_spilled_key_record_with_cancel(spill, bucket, &|| false, visit)
}

pub(super) fn for_each_spilled_key_record_with_cancel<C, F>(
    spill: &SpilledKeyRows,
    bucket: usize,
    is_cancelled: &C,
    mut visit: F,
) -> Result<(), String>
where
    C: Fn() -> bool + Sync,
    F: FnMut(String, usize) -> Result<(), String>,
{
    let path = spill
        .bucket_paths
        .get(bucket)
        .ok_or_else(|| "La cubeta del índice temporal no existe.".to_owned())?;
    if !path.exists() {
        return Ok(());
    }
    let bytes = fs::metadata(path)
        .map_err(|error| format!("No se pudo inspeccionar el índice temporal: {error}"))?
        .len();
    if bytes < COMPARISON_KEY_RECORD_BYTES as u64 {
        return Err("El índice temporal de comparación quedó incompleto.".to_owned());
    }

    let file = File::open(path)
        .map_err(|error| format!("No se pudo leer el índice temporal de comparación: {error}"))?;
    let mut reader = BufReader::new(file);
    let mut record_index = 0usize;
    loop {
        if record_index.is_multiple_of(LOCAL_QUERY_CANCEL_CHECK_ROWS) {
            ensure_not_cancelled((*is_cancelled)())?;
        }
        let mut length_bytes = [0_u8; std::mem::size_of::<u64>()];
        let first_byte = reader.read(&mut length_bytes[..1]).map_err(|error| {
            format!("No se pudo leer el índice temporal de comparación: {error}")
        })?;
        if first_byte == 0 {
            break;
        }
        reader
            .read_exact(&mut length_bytes[1..])
            .map_err(|_| "El índice temporal de comparación quedó incompleto.".to_owned())?;
        let key_length = usize::try_from(u64::from_le_bytes(length_bytes))
            .map_err(|_| "La clave temporal excede la capacidad local.".to_owned())?;
        let mut row_bytes = [0_u8; std::mem::size_of::<u64>()];
        reader.read_exact(&mut row_bytes).map_err(|error| {
            format!("No se pudo leer el índice temporal de comparación: {error}")
        })?;
        let row_index = usize::try_from(u64::from_le_bytes(row_bytes))
            .map_err(|_| "El índice de fila temporal excede la capacidad local.".to_owned())?;
        let mut key_bytes = vec![0_u8; key_length];
        reader.read_exact(&mut key_bytes).map_err(|error| {
            format!("No se pudo leer la clave del índice temporal de comparación: {error}")
        })?;
        let key = String::from_utf8(key_bytes).map_err(|_| {
            "El índice temporal de comparación contiene una clave inválida.".to_owned()
        })?;
        visit(key, row_index)?;
        record_index = record_index
            .checked_add(1)
            .ok_or_else(|| "La cubeta temporal supera la capacidad local.".to_owned())?;
    }
    Ok(())
}

pub(super) fn read_spilled_key_bucket_with_cancel<C>(
    spill: &SpilledKeyRows,
    bucket: usize,
    is_cancelled: &C,
) -> Result<HashMap<String, KeyRowGroup>, String>
where
    C: Fn() -> bool + Sync,
{
    let mut record_index = 0usize;
    let mut rows_by_key = HashMap::new();
    for_each_spilled_key_record_with_cancel(spill, bucket, is_cancelled, |key, row_index| {
        if record_index.is_multiple_of(LOCAL_QUERY_CANCEL_CHECK_ROWS) {
            ensure_not_cancelled((*is_cancelled)())?;
        }
        record_index = record_index
            .checked_add(1)
            .ok_or_else(|| "La cubeta temporal supera la capacidad local.".to_owned())?;
        let group = rows_by_key.entry(key).or_insert(KeyRowGroup {
            first_row_index: row_index,
            count: 0,
        });
        group.count = group
            .count
            .checked_add(1)
            .ok_or_else(|| "La clave temporal supera la capacidad local.".to_owned())?;
        Ok(())
    })?;
    ensure_not_cancelled((*is_cancelled)())?;
    Ok(rows_by_key)
}

pub(super) fn read_spilled_key_bucket(
    spill: &SpilledKeyRows,
    bucket: usize,
) -> Result<HashMap<String, KeyRowGroup>, String> {
    read_spilled_key_bucket_with_cancel(spill, bucket, &|| false)
}

pub(super) fn common_row_count_from_spilled_indexes(
    current_signatures: &SpilledKeyRows,
    compared_signatures: &SpilledKeyRows,
) -> Result<usize, String> {
    common_row_count_from_spilled_indexes_with_cancel(
        current_signatures,
        compared_signatures,
        &|| false,
    )
}

pub(super) fn common_row_count_from_spilled_indexes_with_cancel<C>(
    current_signatures: &SpilledKeyRows,
    compared_signatures: &SpilledKeyRows,
    is_cancelled: &C,
) -> Result<usize, String>
where
    C: Fn() -> bool + Sync,
{
    let mut common = 0usize;
    for bucket in 0..COMPARISON_KEY_BUCKETS {
        ensure_not_cancelled(is_cancelled())?;
        let current_bucket =
            read_spilled_key_bucket_with_cancel(current_signatures, bucket, is_cancelled)?;
        let compared_bucket =
            read_spilled_key_bucket_with_cancel(compared_signatures, bucket, is_cancelled)?;
        for (record_index, (signature, group)) in current_bucket.iter().enumerate() {
            if record_index.is_multiple_of(LOCAL_QUERY_CANCEL_CHECK_ROWS) {
                ensure_not_cancelled(is_cancelled())?;
            }
            common = common.saturating_add(
                group.count.min(
                    compared_bucket
                        .get(signature)
                        .map_or(0, |other| other.count),
                ),
            );
        }
    }
    ensure_not_cancelled(is_cancelled())?;
    Ok(common)
}

pub(super) fn common_row_count_from_spilled_signatures(
    current: &DataFrame,
    compared: &DataFrame,
    shared_columns: &[String],
) -> Result<usize, String> {
    common_row_count_from_spilled_signatures_with_cancel(current, compared, shared_columns, &|| {
        false
    })
}

pub(super) fn common_row_count_from_spilled_signatures_with_cancel<C>(
    current: &DataFrame,
    compared: &DataFrame,
    shared_columns: &[String],
    is_cancelled: &C,
) -> Result<usize, String>
where
    C: Fn() -> bool + Sync,
{
    let current_signatures = spill_key_rows_with_cancel(current, shared_columns, is_cancelled)?;
    let compared_signatures = spill_key_rows_with_cancel(compared, shared_columns, is_cancelled)?;
    common_row_count_from_spilled_indexes_with_cancel(
        &current_signatures,
        &compared_signatures,
        is_cancelled,
    )
}

pub(super) fn common_row_count_from_parquet(
    current: &DataFrame,
    compared_path: &Path,
    compared_row_count: usize,
    shared_columns: &[String],
) -> Result<usize, String> {
    let current_signatures = spill_key_rows(current, shared_columns)?;
    let compared_signatures =
        spill_parquet_rows(compared_path, compared_row_count, shared_columns)?;
    common_row_count_from_spilled_indexes(&current_signatures, &compared_signatures)
}

pub(super) fn common_row_count_between_parquet(
    current_path: &Path,
    current_row_count: usize,
    compared_path: &Path,
    compared_row_count: usize,
    shared_columns: &[String],
) -> Result<usize, String> {
    common_row_count_between_parquet_with_cancel(
        current_path,
        current_row_count,
        compared_path,
        compared_row_count,
        shared_columns,
        &|| false,
    )
}

pub(super) fn common_row_count_between_parquet_with_cancel<C>(
    current_path: &Path,
    current_row_count: usize,
    compared_path: &Path,
    compared_row_count: usize,
    shared_columns: &[String],
    is_cancelled: &C,
) -> Result<usize, String>
where
    C: Fn() -> bool + Sync,
{
    let current_signatures = spill_parquet_rows_with_cancel(
        current_path,
        current_row_count,
        shared_columns,
        is_cancelled,
    )?;
    let compared_signatures = spill_parquet_rows_with_cancel(
        compared_path,
        compared_row_count,
        shared_columns,
        is_cancelled,
    )?;
    common_row_count_from_spilled_indexes_with_cancel(
        &current_signatures,
        &compared_signatures,
        is_cancelled,
    )
}

#[cfg(test)]
pub(super) fn collect_spilled_key_rows(
    spill: &SpilledKeyRows,
) -> Result<HashMap<String, Vec<usize>>, String> {
    let mut rows_by_key = HashMap::new();
    for bucket in 0..COMPARISON_KEY_BUCKETS {
        for_each_spilled_key_record(spill, bucket, |key, row_index| {
            rows_by_key
                .entry(key)
                .or_insert_with(Vec::new)
                .push(row_index);
            Ok(())
        })?;
    }
    Ok(rows_by_key)
}

pub(super) fn count_distinct_spilled_key_rows<C>(
    spill: &SpilledKeyRows,
    is_cancelled: &C,
) -> Result<usize, String>
where
    C: Fn() -> bool + Sync,
{
    let mut distinct_count = 0usize;
    for bucket in 0..COMPARISON_KEY_BUCKETS {
        ensure_not_cancelled(is_cancelled())?;
        let mut keys = Vec::new();
        let mut record_index = 0usize;
        for_each_spilled_key_record(spill, bucket, |key, _| {
            if record_index.is_multiple_of(LOCAL_QUERY_CANCEL_CHECK_ROWS) {
                ensure_not_cancelled(is_cancelled())?;
            }
            record_index = record_index.saturating_add(1);
            keys.push(key);
            Ok(())
        })?;
        keys.sort_unstable();
        keys.dedup();
        distinct_count = distinct_count
            .checked_add(keys.len())
            .ok_or_else(|| "El conteo de filas distintas excede la capacidad local.".to_owned())?;
    }
    Ok(distinct_count)
}

pub(super) fn count_unique_invalid_from_parquet<C>(
    path: &Path,
    row_count: usize,
    column_name: &str,
    is_cancelled: &C,
) -> Result<usize, String>
where
    C: Fn() -> bool + Sync,
{
    ensure_not_cancelled(is_cancelled())?;
    let spill = create_spilled_key_rows()?;
    let columns = vec![column_name.to_owned()];
    let mut null_count = 0usize;
    for_each_parquet_block(path, row_count, |start, block| {
        ensure_not_cancelled(is_cancelled())?;
        let column = block
            .column(column_name)
            .map_err(|error| format!("No se pudo evaluar unique source-backed: {error}"))?;
        null_count = null_count
            .checked_add(column.null_count())
            .ok_or_else(|| "El conteo de nulos excede la capacidad local.".to_owned())?;
        append_spilled_key_rows(&spill, block, &columns, start)
    })?;
    let distinct_count = count_distinct_spilled_key_rows(&spill, is_cancelled)?;
    let distinct_non_null = distinct_count.saturating_sub(usize::from(null_count > 0));
    Ok(row_count.saturating_sub(distinct_non_null))
}

pub(super) fn count_duplicate_combinations_from_parquet<C>(
    path: &Path,
    row_count: usize,
    columns: &[String],
    is_cancelled: &C,
) -> Result<usize, String>
where
    C: Fn() -> bool + Sync,
{
    ensure_not_cancelled(is_cancelled())?;
    let spill = create_spilled_key_rows()?;
    for_each_parquet_block(path, row_count, |start, block| {
        ensure_not_cancelled(is_cancelled())?;
        append_spilled_key_rows(&spill, block, columns, start)
    })?;

    let mut duplicate_count = 0usize;
    for bucket in 0..COMPARISON_KEY_BUCKETS {
        let groups = read_spilled_key_bucket_with_cancel(&spill, bucket, is_cancelled)?;
        for group in groups.values() {
            duplicate_count = duplicate_count
                .checked_add(group.count.saturating_sub(1))
                .ok_or_else(|| "El conteo de duplicados excede la capacidad local.".to_owned())?;
        }
    }
    Ok(duplicate_count)
}

pub(super) fn count_monotonic_invalid_from_parquet<C>(
    path: &Path,
    row_count: usize,
    column_name: &str,
    direction: QualityMonotonicDirection,
    is_cancelled: &C,
) -> Result<usize, String>
where
    C: Fn() -> bool + Sync,
{
    ensure_not_cancelled(is_cancelled())?;
    let mut previous: Option<AnyValue<'static>> = None;
    let mut invalid_count = 0usize;
    for_each_parquet_block(path, row_count, |_, block| {
        let column = block
            .column(column_name)
            .map_err(|error| format!("No se pudo evaluar monotonic source-backed: {error}"))?;
        for row_index in 0..column.len() {
            if row_index.is_multiple_of(LOCAL_QUERY_CANCEL_CHECK_ROWS) {
                ensure_not_cancelled(is_cancelled())?;
            }
            let value = column
                .get(row_index)
                .map_err(|error| format!("No se pudo leer monotonic source-backed: {error}"))?;
            if matches!(value, AnyValue::Null) {
                previous = None;
                continue;
            }
            if let Some(previous_value) = previous.as_ref() {
                let invalid =
                    match quality_monotonic_ordering(previous_value.clone(), value.clone()) {
                        Some(ordering) => match direction {
                            QualityMonotonicDirection::Increasing => {
                                ordering == std::cmp::Ordering::Greater
                            }
                            QualityMonotonicDirection::Decreasing => {
                                ordering == std::cmp::Ordering::Less
                            }
                        },
                        None => true,
                    };
                invalid_count = invalid_count.saturating_add(usize::from(invalid));
            }
            previous = Some(value.into_static());
        }
        Ok(())
    })?;
    Ok(invalid_count)
}

pub(super) fn quality_aggregate_observation_from_parquet<C>(
    path: &Path,
    row_count: usize,
    column_name: &str,
    is_cancelled: &C,
) -> Result<(usize, f64, Option<f64>, Option<f64>), String>
where
    C: Fn() -> bool + Sync,
{
    ensure_not_cancelled(is_cancelled())?;
    let mut count = 0usize;
    let mut sum = 0.0;
    let mut minimum = None;
    let mut maximum = None;
    for_each_parquet_block(path, row_count, |_, block| {
        let column = block
            .column(column_name)
            .map_err(|error| format!("No se pudo evaluar la agregación source-backed: {error}"))?;
        let (block_count, block_sum, block_minimum, block_maximum) =
            quality_aggregate_observation(column, block.height(), is_cancelled)?;
        count = count.saturating_add(block_count);
        sum += block_sum;
        if let Some(value) = block_minimum {
            minimum = Some(minimum.map_or(value, |current: f64| current.min(value)));
        }
        if let Some(value) = block_maximum {
            maximum = Some(maximum.map_or(value, |current: f64| current.max(value)));
        }
        Ok(())
    })?;
    Ok((count, sum, minimum, maximum))
}

pub(super) fn count_normalized_duplicate_fingerprints<C>(
    bucket_paths: &[PathBuf],
    is_cancelled: &C,
) -> Result<usize, String>
where
    C: Fn() -> bool + Sync,
{
    let mut normalized_duplicate_row_count = 0usize;
    for bucket_path in bucket_paths {
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
            if index.is_multiple_of(LOCAL_QUERY_CANCEL_CHECK_ROWS) {
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
    Ok(normalized_duplicate_row_count)
}

#[cfg(test)]
pub(super) fn collect_spilled_signature_counts(
    spill: &SpilledKeyRows,
) -> Result<HashMap<String, usize>, String> {
    let mut counts = HashMap::new();
    for bucket in 0..COMPARISON_KEY_BUCKETS {
        for (signature, group) in read_spilled_key_bucket(spill, bucket)? {
            counts.insert(signature, group.count);
        }
    }
    Ok(counts)
}

#[derive(Default)]
pub(super) struct KeyComparisonSummary {
    pub(super) matched_key_count: usize,
    pub(super) current_only_key_count: usize,
    pub(super) compared_only_key_count: usize,
    pub(super) conflicting_key_count: usize,
    pub(super) duplicate_key_count: usize,
}

pub(super) fn validate_key_columns(
    current: &DataFrame,
    compared: &DataFrame,
    key_columns: &[String],
) -> Result<(), String> {
    if key_columns.is_empty() {
        return Err("Selecciona al menos una columna clave para unir datasets.".to_owned());
    }
    for key in key_columns {
        let current_column = current
            .column(key)
            .map_err(|_| format!("La columna clave '{key}' no existe en el dataset activo."))?;
        let compared_column = compared
            .column(key)
            .map_err(|_| format!("La columna clave '{key}' no existe en el dataset comparado."))?;
        if current_column.dtype() != compared_column.dtype() {
            return Err(format!(
                "La columna clave '{key}' tiene tipos incompatibles entre los datasets."
            ));
        }
    }
    Ok(())
}

pub(super) fn compare_keyed_frames(
    current: &DataFrame,
    compared: &DataFrame,
    key_columns: &[String],
    shared_columns: &[String],
) -> Result<KeyComparisonSummary, String> {
    compare_keyed_frames_with_cancel(current, compared, key_columns, shared_columns, || false)
}

pub(super) fn compare_keyed_frames_with_cancel<C>(
    current: &DataFrame,
    compared: &DataFrame,
    key_columns: &[String],
    shared_columns: &[String],
    is_cancelled: C,
) -> Result<KeyComparisonSummary, String>
where
    C: Fn() -> bool + Sync,
{
    ensure_not_cancelled(is_cancelled())?;
    validate_key_columns(current, compared, key_columns)?;

    let current_rows = spill_key_rows_with_cancel(current, key_columns, &is_cancelled)?;
    let compared_rows = spill_key_rows_with_cancel(compared, key_columns, &is_cancelled)?;
    let shared_payload_columns = shared_columns
        .iter()
        .filter(|column| !key_columns.contains(column))
        .cloned()
        .collect::<Vec<_>>();
    let mut summary = KeyComparisonSummary::default();
    for bucket in 0..COMPARISON_KEY_BUCKETS {
        ensure_not_cancelled(is_cancelled())?;
        let current_bucket =
            read_spilled_key_bucket_with_cancel(&current_rows, bucket, &is_cancelled)?;
        let compared_bucket =
            read_spilled_key_bucket_with_cancel(&compared_rows, bucket, &is_cancelled)?;
        summary.matched_key_count += current_bucket
            .keys()
            .filter(|key| compared_bucket.contains_key(*key))
            .count();
        summary.current_only_key_count += current_bucket
            .keys()
            .filter(|key| !compared_bucket.contains_key(*key))
            .count();
        summary.compared_only_key_count += compared_bucket
            .keys()
            .filter(|key| !current_bucket.contains_key(*key))
            .count();
        summary.duplicate_key_count += current_bucket
            .values()
            .filter(|rows| rows.count > 1)
            .count();
        summary.duplicate_key_count += compared_bucket
            .iter()
            .filter(|(key, rows)| {
                rows.count > 1
                    && current_bucket
                        .get(*key)
                        .is_none_or(|current_rows| current_rows.count <= 1)
            })
            .count();

        for (key, current_key_rows) in &current_bucket {
            let Some(compared_key_rows) = compared_bucket.get(key) else {
                continue;
            };
            if current_key_rows.count != 1 || compared_key_rows.count != 1 {
                continue;
            }
            let current_payload = row_signature(
                current,
                &shared_payload_columns,
                current_key_rows.first_row_index,
            )?;
            let compared_payload = row_signature(
                compared,
                &shared_payload_columns,
                compared_key_rows.first_row_index,
            )?;
            if current_payload != compared_payload {
                summary.conflicting_key_count += 1;
            }
        }
    }

    ensure_not_cancelled(is_cancelled())?;
    Ok(summary)
}

pub(super) fn compare_keyed_parquet(
    current: &DataFrame,
    compared_path: &Path,
    compared_row_count: usize,
    key_columns: &[String],
    shared_columns: &[String],
) -> Result<KeyComparisonSummary, String> {
    let compared_schema = read_parquet_schema_frame(compared_path)?;
    validate_key_columns(current, &compared_schema, key_columns)?;
    let current_rows = spill_key_rows(current, key_columns)?;
    let compared_rows = spill_parquet_rows(compared_path, compared_row_count, key_columns)?;
    let shared_payload_columns = shared_columns
        .iter()
        .filter(|column| !key_columns.contains(column))
        .cloned()
        .collect::<Vec<_>>();
    let mut summary = KeyComparisonSummary::default();
    for bucket in 0..COMPARISON_KEY_BUCKETS {
        let current_bucket = read_spilled_key_bucket(&current_rows, bucket)?;
        let compared_bucket = read_spilled_key_bucket(&compared_rows, bucket)?;
        summary.matched_key_count += current_bucket
            .keys()
            .filter(|key| compared_bucket.contains_key(*key))
            .count();
        summary.current_only_key_count += current_bucket
            .keys()
            .filter(|key| !compared_bucket.contains_key(*key))
            .count();
        summary.compared_only_key_count += compared_bucket
            .keys()
            .filter(|key| !current_bucket.contains_key(*key))
            .count();
        summary.duplicate_key_count += current_bucket
            .values()
            .filter(|rows| rows.count > 1)
            .count();
        summary.duplicate_key_count += compared_bucket
            .iter()
            .filter(|(key, rows)| {
                rows.count > 1
                    && current_bucket
                        .get(*key)
                        .is_none_or(|current_rows| current_rows.count <= 1)
            })
            .count();
    }

    for_each_parquet_block(compared_path, compared_row_count, |_, block| {
        let block_rows = spill_key_rows(block, key_columns)?;
        for bucket in 0..COMPARISON_KEY_BUCKETS {
            let current_bucket = read_spilled_key_bucket(&current_rows, bucket)?;
            let compared_bucket = read_spilled_key_bucket(&compared_rows, bucket)?;
            let block_bucket = read_spilled_key_bucket(&block_rows, bucket)?;
            for (key, block_group) in block_bucket {
                let Some(current_group) = current_bucket.get(&key) else {
                    continue;
                };
                let Some(compared_group) = compared_bucket.get(&key) else {
                    continue;
                };
                if current_group.count != 1 || compared_group.count != 1 || block_group.count != 1 {
                    continue;
                }
                let current_payload = row_signature(
                    current,
                    &shared_payload_columns,
                    current_group.first_row_index,
                )?;
                let compared_payload =
                    row_signature(block, &shared_payload_columns, block_group.first_row_index)?;
                if current_payload != compared_payload {
                    summary.conflicting_key_count += 1;
                }
            }
        }
        Ok(())
    })?;

    Ok(summary)
}

pub(super) struct ParquetComparisonSource<'a> {
    pub(super) path: &'a Path,
    pub(super) row_count: usize,
}

pub(super) fn compare_keyed_parquet_sources(
    current: ParquetComparisonSource<'_>,
    compared: ParquetComparisonSource<'_>,
    current_schema: &DataFrame,
    compared_schema: &DataFrame,
    key_columns: &[String],
    shared_columns: &[String],
) -> Result<KeyComparisonSummary, String> {
    compare_keyed_parquet_sources_with_cancel(
        current,
        compared,
        current_schema,
        compared_schema,
        key_columns,
        shared_columns,
        &|| false,
    )
}

pub(super) fn compare_keyed_parquet_sources_with_cancel<C>(
    current: ParquetComparisonSource<'_>,
    compared: ParquetComparisonSource<'_>,
    current_schema: &DataFrame,
    compared_schema: &DataFrame,
    key_columns: &[String],
    shared_columns: &[String],
    is_cancelled: &C,
) -> Result<KeyComparisonSummary, String>
where
    C: Fn() -> bool + Sync,
{
    ensure_not_cancelled(is_cancelled())?;
    validate_key_columns(current_schema, compared_schema, key_columns)?;
    let shared_payload_columns = shared_columns
        .iter()
        .filter(|column| !key_columns.contains(column))
        .cloned()
        .collect::<Vec<_>>();
    let current_rows = spill_parquet_key_payload_rows_with_cancel(
        current.path,
        current.row_count,
        key_columns,
        &shared_payload_columns,
        is_cancelled,
    )?;
    let compared_rows = spill_parquet_key_payload_rows_with_cancel(
        compared.path,
        compared.row_count,
        key_columns,
        &shared_payload_columns,
        is_cancelled,
    )?;
    let mut summary = KeyComparisonSummary::default();
    for bucket in 0..COMPARISON_KEY_BUCKETS {
        ensure_not_cancelled(is_cancelled())?;
        let current_bucket =
            read_spilled_key_payload_bucket_with_cancel(&current_rows, bucket, is_cancelled)?;
        let compared_bucket =
            read_spilled_key_payload_bucket_with_cancel(&compared_rows, bucket, is_cancelled)?;
        for (record_index, (key, current_group)) in current_bucket.iter().enumerate() {
            if record_index.is_multiple_of(LOCAL_QUERY_CANCEL_CHECK_ROWS) {
                ensure_not_cancelled(is_cancelled())?;
            }
            match compared_bucket.get(key) {
                Some(_) => summary.matched_key_count += 1,
                None => summary.current_only_key_count += 1,
            }
            if current_group.count > 1 {
                summary.duplicate_key_count += 1;
            }
            let Some(compared_group) = compared_bucket.get(key) else {
                continue;
            };
            if current_group.count == 1
                && compared_group.count == 1
                && current_group.payload != compared_group.payload
            {
                summary.conflicting_key_count += 1;
            }
        }
        for (record_index, (key, compared_group)) in compared_bucket.iter().enumerate() {
            if record_index.is_multiple_of(LOCAL_QUERY_CANCEL_CHECK_ROWS) {
                ensure_not_cancelled(is_cancelled())?;
            }
            if !current_bucket.contains_key(key) {
                summary.compared_only_key_count += 1;
            }
            if compared_group.count > 1
                && current_bucket
                    .get(key)
                    .is_none_or(|current_group| current_group.count <= 1)
            {
                summary.duplicate_key_count += 1;
            }
        }
    }
    ensure_not_cancelled(is_cancelled())?;
    Ok(summary)
}

pub(super) struct KeyConflictRows {
    pub(super) current_row_index: usize,
    pub(super) compared_row_index: usize,
    pub(super) conflict: DatasetConflict,
}

pub(super) struct KeyConflictShape {
    pub(super) columns: Vec<String>,
}

pub(super) fn build_key_conflict_shape(
    current: &DataFrame,
    compared: &DataFrame,
    shared_payload_columns: &[String],
    current_row_index: usize,
    compared_row_index: usize,
) -> Result<Option<KeyConflictShape>, String> {
    let current_payload = row_signature(current, shared_payload_columns, current_row_index)?;
    let compared_payload = row_signature(compared, shared_payload_columns, compared_row_index)?;
    if current_payload == compared_payload {
        return Ok(None);
    }

    let columns = shared_payload_columns
        .iter()
        .map(|column| {
            let current_value = current
                .column(column)
                .map_err(|error| format!("No se pudo leer la columna '{column}': {error}"))?
                .get(current_row_index)
                .map_err(|error| format!("No se pudo leer el valor en conflicto: {error}"))
                .map(preview_value)?;
            let compared_value = compared
                .column(column)
                .map_err(|error| format!("No se pudo leer la columna '{column}': {error}"))?
                .get(compared_row_index)
                .map_err(|error| format!("No se pudo leer el valor en conflicto: {error}"))
                .map(preview_value)?;
            Ok((column.clone(), current_value, compared_value))
        })
        .collect::<Result<Vec<_>, String>>()?
        .into_iter()
        .filter(|(_, current_value, compared_value)| current_value != compared_value)
        .map(|(column, _, _)| column)
        .collect::<Vec<_>>();
    if columns.is_empty() {
        return Ok(None);
    }
    Ok(Some(KeyConflictShape { columns }))
}

pub(super) fn build_key_conflict_from_shape(
    current: &DataFrame,
    compared: &DataFrame,
    key_columns: &[String],
    shape: KeyConflictShape,
    current_row_index: usize,
    compared_row_index: usize,
) -> Result<KeyConflictRows, String> {
    let key = key_columns
        .iter()
        .map(|column| {
            current
                .column(column)
                .map_err(|error| format!("No se pudo leer la clave '{column}': {error}"))?
                .get(current_row_index)
                .map_err(|error| format!("No se pudo leer la fila en conflicto: {error}"))
                .map(preview_value)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let cells = shape
        .columns
        .into_iter()
        .map(|column| {
            let current_value = current
                .column(&column)
                .map_err(|error| format!("No se pudo leer la columna '{column}': {error}"))?
                .get(current_row_index)
                .map_err(|error| format!("No se pudo leer el valor en conflicto: {error}"))
                .map(preview_value)?;
            let compared_value = compared
                .column(&column)
                .map_err(|error| format!("No se pudo leer la columna '{column}': {error}"))?
                .get(compared_row_index)
                .map_err(|error| format!("No se pudo leer el valor en conflicto: {error}"))
                .map(preview_value)?;
            Ok(DatasetConflictCell {
                column,
                current: current_value,
                compared: compared_value,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(KeyConflictRows {
        current_row_index,
        compared_row_index,
        conflict: DatasetConflict { key, cells },
    })
}

pub(super) fn build_key_conflict(
    current: &DataFrame,
    compared: &DataFrame,
    key_columns: &[String],
    shared_payload_columns: &[String],
    current_row_index: usize,
    compared_row_index: usize,
) -> Result<Option<KeyConflictRows>, String> {
    let Some(shape) = build_key_conflict_shape(
        current,
        compared,
        shared_payload_columns,
        current_row_index,
        compared_row_index,
    )?
    else {
        return Ok(None);
    };
    Ok(Some(build_key_conflict_from_shape(
        current,
        compared,
        key_columns,
        shape,
        current_row_index,
        compared_row_index,
    )?))
}

pub(super) fn collect_key_conflicts_page(
    current: &DataFrame,
    compared: &DataFrame,
    key_columns: &[String],
    shared_columns: &[String],
    offset: usize,
    limit: usize,
) -> Result<(Vec<KeyConflictRows>, bool), String> {
    collect_key_conflicts_page_with_cancel(
        current,
        compared,
        key_columns,
        shared_columns,
        offset,
        limit,
        &|| false,
    )
}

pub(super) fn collect_key_conflicts_page_with_cancel<C>(
    current: &DataFrame,
    compared: &DataFrame,
    key_columns: &[String],
    shared_columns: &[String],
    offset: usize,
    limit: usize,
    is_cancelled: &C,
) -> Result<(Vec<KeyConflictRows>, bool), String>
where
    C: Fn() -> bool + Sync,
{
    ensure_not_cancelled(is_cancelled())?;
    validate_key_columns(current, compared, key_columns)?;
    let current_rows = spill_key_rows_with_cancel(current, key_columns, is_cancelled)?;
    let compared_rows = spill_key_rows_with_cancel(compared, key_columns, is_cancelled)?;
    let shared_payload_columns = shared_columns
        .iter()
        .filter(|column| !key_columns.contains(column))
        .cloned()
        .collect::<Vec<_>>();
    let row_index_bytes = std::mem::size_of::<u64>() as u64;
    let current_row_bytes = u64::try_from(current.height())
        .map_err(|_| "El dataset activo supera la capacidad del índice temporal.".to_owned())?;
    let mut conflict_markers = tempfile::tempfile().map_err(|error| {
        format!("No se pudo preparar el índice temporal de conflictos: {error}")
    })?;
    conflict_markers
        .set_len(current_row_bytes)
        .map_err(|error| {
            format!("No se pudo preparar el índice temporal de conflictos: {error}")
        })?;
    let mut conflict_rows = tempfile::tempfile().map_err(|error| {
        format!("No se pudo preparar el índice temporal de conflictos: {error}")
    })?;

    for bucket in 0..COMPARISON_KEY_BUCKETS {
        ensure_not_cancelled(is_cancelled())?;
        let current_bucket =
            read_spilled_key_bucket_with_cancel(&current_rows, bucket, is_cancelled)?;
        let compared_bucket =
            read_spilled_key_bucket_with_cancel(&compared_rows, bucket, is_cancelled)?;
        let mut record_index = 0usize;
        for (signature, current_group) in current_bucket {
            if record_index.is_multiple_of(LOCAL_QUERY_CANCEL_CHECK_ROWS) {
                ensure_not_cancelled(is_cancelled())?;
            }
            record_index = record_index.saturating_add(1);
            let Some(compared_group) = compared_bucket.get(&signature) else {
                continue;
            };
            if current_group.count != 1 || compared_group.count != 1 {
                continue;
            }
            let current_payload = row_signature(
                current,
                &shared_payload_columns,
                current_group.first_row_index,
            )?;
            let compared_payload = row_signature(
                compared,
                &shared_payload_columns,
                compared_group.first_row_index,
            )?;
            if current_payload == compared_payload {
                continue;
            }
            let marker_offset = u64::try_from(current_group.first_row_index).map_err(|_| {
                "El índice de fila supera la capacidad del índice temporal.".to_owned()
            })?;
            let compared_offset = u64::try_from(current_group.first_row_index)
                .ok()
                .and_then(|index| index.checked_mul(row_index_bytes))
                .ok_or_else(|| "El índice de conflicto supera la capacidad local.".to_owned())?;
            let compared_row_index =
                u64::try_from(compared_group.first_row_index).map_err(|_| {
                    "El índice de fila supera la capacidad del índice temporal.".to_owned()
                })?;
            conflict_markers
                .seek(SeekFrom::Start(marker_offset))
                .and_then(|_| conflict_markers.write_all(&[1]))
                .map_err(|error| {
                    format!("No se pudo escribir el índice temporal de conflictos: {error}")
                })?;
            conflict_rows
                .seek(SeekFrom::Start(compared_offset))
                .and_then(|_| conflict_rows.write_all(&compared_row_index.to_le_bytes()))
                .map_err(|error| {
                    format!("No se pudo escribir el índice temporal de conflictos: {error}")
                })?;
        }
    }

    conflict_markers
        .seek(SeekFrom::Start(0))
        .map_err(|error| format!("No se pudo leer el índice temporal de conflictos: {error}"))?;
    let mut marker_reader = BufReader::new(conflict_markers);
    let mut conflicts = Vec::new();
    let mut total = 0usize;
    for current_row_index in 0..current.height() {
        if current_row_index.is_multiple_of(LOCAL_QUERY_CANCEL_CHECK_ROWS) {
            ensure_not_cancelled(is_cancelled())?;
        }
        let mut marker = [0_u8; 1];
        marker_reader.read_exact(&mut marker).map_err(|error| {
            format!("No se pudo leer el índice temporal de conflictos: {error}")
        })?;
        if marker[0] == 0 {
            continue;
        }
        let row_offset = u64::try_from(current_row_index)
            .ok()
            .and_then(|index| index.checked_mul(row_index_bytes))
            .ok_or_else(|| "El índice de conflicto supera la capacidad local.".to_owned())?;
        conflict_rows
            .seek(SeekFrom::Start(row_offset))
            .map_err(|error| {
                format!("No se pudo leer el índice temporal de conflictos: {error}")
            })?;
        let mut compared_row_bytes = [0_u8; std::mem::size_of::<u64>()];
        conflict_rows
            .read_exact(&mut compared_row_bytes)
            .map_err(|error| {
                format!("No se pudo leer el índice temporal de conflictos: {error}")
            })?;
        let compared_row_index = usize::try_from(u64::from_le_bytes(compared_row_bytes))
            .map_err(|_| "El índice de fila temporal excede la capacidad local.".to_owned())?;
        if let Some(conflict) = build_key_conflict(
            current,
            compared,
            key_columns,
            &shared_payload_columns,
            current_row_index,
            compared_row_index,
        )? {
            let conflict_index = total;
            total = total.saturating_add(1);
            if conflict_index >= offset && conflicts.len() < limit {
                conflicts.push(conflict);
            }
        }
    }
    let page_end = offset.saturating_add(conflicts.len());
    ensure_not_cancelled(is_cancelled())?;
    Ok((conflicts, total > page_end))
}

pub(super) fn collect_key_conflicts_page_from_parquet(
    current: &DataFrame,
    compared_path: &Path,
    compared_row_count: usize,
    key_columns: &[String],
    shared_columns: &[String],
    offset: usize,
    limit: usize,
) -> Result<(Vec<KeyConflictRows>, bool), String> {
    collect_key_conflicts_page_from_parquet_with_cancel(
        current,
        compared_path,
        compared_row_count,
        key_columns,
        shared_columns,
        offset,
        limit,
        &|| false,
    )
}

pub(super) fn collect_key_conflicts_page_from_parquet_with_cancel<C>(
    current: &DataFrame,
    compared_path: &Path,
    compared_row_count: usize,
    key_columns: &[String],
    shared_columns: &[String],
    offset: usize,
    limit: usize,
    is_cancelled: &C,
) -> Result<(Vec<KeyConflictRows>, bool), String>
where
    C: Fn() -> bool + Sync,
{
    ensure_not_cancelled(is_cancelled())?;
    let compared_schema = read_parquet_schema_frame(compared_path)?;
    ensure_not_cancelled(is_cancelled())?;
    validate_key_columns(current, &compared_schema, key_columns)?;
    let current_rows = spill_key_rows_with_cancel(current, key_columns, is_cancelled)?;
    let compared_rows = spill_parquet_rows_with_cancel(
        compared_path,
        compared_row_count,
        key_columns,
        is_cancelled,
    )?;

    let shared_payload_columns = shared_columns
        .iter()
        .filter(|column| !key_columns.contains(column))
        .cloned()
        .collect::<Vec<_>>();
    let row_index_bytes = std::mem::size_of::<u64>() as u64;
    let current_row_bytes = u64::try_from(current.height())
        .map_err(|_| "El dataset activo supera la capacidad del índice temporal.".to_owned())?;
    let mut conflict_markers = tempfile::tempfile().map_err(|error| {
        format!("No se pudo preparar el índice temporal de conflictos: {error}")
    })?;
    conflict_markers
        .set_len(current_row_bytes)
        .map_err(|error| {
            format!("No se pudo preparar el índice temporal de conflictos: {error}")
        })?;
    let mut conflict_rows = tempfile::tempfile().map_err(|error| {
        format!("No se pudo preparar el índice temporal de conflictos: {error}")
    })?;

    for_each_parquet_block_with_cancel(
        compared_path,
        compared_row_count,
        is_cancelled,
        |start, block| {
            let block_rows = spill_key_rows_with_cancel(block, key_columns, is_cancelled)?;
            for bucket in 0..COMPARISON_KEY_BUCKETS {
                ensure_not_cancelled(is_cancelled())?;
                let current_bucket =
                    read_spilled_key_bucket_with_cancel(&current_rows, bucket, is_cancelled)?;
                let compared_bucket =
                    read_spilled_key_bucket_with_cancel(&compared_rows, bucket, is_cancelled)?;
                let block_bucket =
                    read_spilled_key_bucket_with_cancel(&block_rows, bucket, is_cancelled)?;
                let mut record_index = 0usize;
                for (signature, block_group) in block_bucket {
                    if record_index.is_multiple_of(LOCAL_QUERY_CANCEL_CHECK_ROWS) {
                        ensure_not_cancelled(is_cancelled())?;
                    }
                    record_index = record_index.saturating_add(1);
                    let Some(current_group) = current_bucket.get(&signature) else {
                        continue;
                    };
                    let Some(compared_group) = compared_bucket.get(&signature) else {
                        continue;
                    };
                    if current_group.count != 1
                        || compared_group.count != 1
                        || block_group.count != 1
                    {
                        continue;
                    }
                    let current_payload = row_signature(
                        current,
                        &shared_payload_columns,
                        current_group.first_row_index,
                    )?;
                    let compared_payload =
                        row_signature(block, &shared_payload_columns, block_group.first_row_index)?;
                    if current_payload == compared_payload {
                        continue;
                    }
                    let marker_offset =
                        u64::try_from(current_group.first_row_index).map_err(|_| {
                            "El índice de fila supera la capacidad del índice temporal.".to_owned()
                        })?;
                    let compared_offset = u64::try_from(current_group.first_row_index)
                        .ok()
                        .and_then(|index| index.checked_mul(row_index_bytes))
                        .ok_or_else(|| {
                            "El índice de conflicto supera la capacidad local.".to_owned()
                        })?;
                    let compared_row_index = start
                        .checked_add(block_group.first_row_index)
                        .and_then(|index| u64::try_from(index).ok())
                        .ok_or_else(|| {
                            "El índice de fila supera la capacidad del índice temporal.".to_owned()
                        })?;
                    conflict_markers
                        .seek(SeekFrom::Start(marker_offset))
                        .and_then(|_| conflict_markers.write_all(&[1]))
                        .map_err(|error| {
                            format!("No se pudo escribir el índice temporal de conflictos: {error}")
                        })?;
                    conflict_rows
                        .seek(SeekFrom::Start(compared_offset))
                        .and_then(|_| conflict_rows.write_all(&compared_row_index.to_le_bytes()))
                        .map_err(|error| {
                            format!("No se pudo escribir el índice temporal de conflictos: {error}")
                        })?;
                }
            }
            Ok(())
        },
    )?;

    ensure_not_cancelled(is_cancelled())?;
    conflict_markers
        .seek(SeekFrom::Start(0))
        .map_err(|error| format!("No se pudo leer el índice temporal de conflictos: {error}"))?;
    let mut marker_reader = BufReader::new(conflict_markers);
    let mut conflicts = Vec::new();
    let mut total = 0usize;
    let mut cached_compared_start = None;
    let mut cached_compared = None;
    for current_row_index in 0..current.height() {
        if current_row_index.is_multiple_of(LOCAL_QUERY_CANCEL_CHECK_ROWS) {
            ensure_not_cancelled(is_cancelled())?;
        }
        let mut marker = [0_u8; 1];
        marker_reader.read_exact(&mut marker).map_err(|error| {
            format!("No se pudo leer el índice temporal de conflictos: {error}")
        })?;
        if marker[0] == 0 {
            continue;
        }
        let row_offset = u64::try_from(current_row_index)
            .ok()
            .and_then(|index| index.checked_mul(row_index_bytes))
            .ok_or_else(|| "El índice de conflicto supera la capacidad local.".to_owned())?;
        conflict_rows
            .seek(SeekFrom::Start(row_offset))
            .map_err(|error| {
                format!("No se pudo leer el índice temporal de conflictos: {error}")
            })?;
        let mut compared_row_bytes = [0_u8; std::mem::size_of::<u64>()];
        conflict_rows
            .read_exact(&mut compared_row_bytes)
            .map_err(|error| {
                format!("No se pudo leer el índice temporal de conflictos: {error}")
            })?;
        let compared_row_index = usize::try_from(u64::from_le_bytes(compared_row_bytes))
            .map_err(|_| "El índice de fila temporal excede la capacidad local.".to_owned())?;
        if compared_row_index >= compared_row_count {
            return Err(format!(
                "{LOCAL_QUERY_SNAPSHOT_ERROR_PREFIX} el índice de conflicto apunta fuera del snapshot registrado."
            ));
        }
        let compared_start = (compared_row_index / LOCAL_QUERY_BLOCK_ROWS)
            .checked_mul(LOCAL_QUERY_BLOCK_ROWS)
            .ok_or_else(|| {
                format!(
                    "{LOCAL_QUERY_SNAPSHOT_ERROR_PREFIX} el índice del bloque excede la capacidad local."
                )
            })?;
        if cached_compared_start != Some(compared_start) {
            ensure_not_cancelled(is_cancelled())?;
            let length = LOCAL_QUERY_BLOCK_ROWS.min(compared_row_count - compared_start);
            let block = read_parquet_query_block_with_cancel(
                compared_path,
                compared_start,
                length,
                is_cancelled,
            )?;
            if block.height() != length {
                return Err(format!(
                    "{LOCAL_QUERY_SNAPSHOT_ERROR_PREFIX} el snapshot comparado cambió durante la lectura (se esperaban {length} filas en el bloque y se obtuvieron {}).",
                    block.height()
                ));
            }
            ensure_not_cancelled(is_cancelled())?;
            cached_compared_start = Some(compared_start);
            cached_compared = Some(block);
        }
        let compared_block = cached_compared.as_ref().ok_or_else(|| {
            format!(
                "{LOCAL_QUERY_SNAPSHOT_ERROR_PREFIX} no se pudo conservar el bloque del conflicto."
            )
        })?;
        let compared_row_in_block = compared_row_index - compared_start;
        if let Some(mut conflict) = build_key_conflict(
            current,
            compared_block,
            key_columns,
            &shared_payload_columns,
            current_row_index,
            compared_row_in_block,
        )? {
            conflict.compared_row_index = compared_row_index;
            let conflict_index = total;
            total = total.saturating_add(1);
            if conflict_index >= offset && conflicts.len() < limit {
                conflicts.push(conflict);
            }
        }
    }
    let page_end = offset.saturating_add(conflicts.len());
    ensure_not_cancelled(is_cancelled())?;
    Ok((conflicts, total > page_end))
}

pub(super) fn for_each_key_conflict_between_parquet<F>(
    current: ParquetComparisonSource<'_>,
    compared: ParquetComparisonSource<'_>,
    key_columns: &[String],
    shared_columns: &[String],
    max_conflicts: Option<usize>,
    visit: F,
) -> Result<usize, String>
where
    F: FnMut(
        usize,
        &DataFrame,
        usize,
        &DataFrame,
        usize,
        usize,
        usize,
        KeyConflictShape,
    ) -> Result<(), String>,
{
    for_each_key_conflict_between_parquet_with_cancel(
        current,
        compared,
        key_columns,
        shared_columns,
        max_conflicts,
        &|| false,
        visit,
    )
}

pub(super) fn for_each_key_conflict_between_parquet_with_cancel<C, F>(
    current: ParquetComparisonSource<'_>,
    compared: ParquetComparisonSource<'_>,
    key_columns: &[String],
    shared_columns: &[String],
    max_conflicts: Option<usize>,
    is_cancelled: &C,
    mut visit: F,
) -> Result<usize, String>
where
    C: Fn() -> bool + Sync,
    F: FnMut(
        usize,
        &DataFrame,
        usize,
        &DataFrame,
        usize,
        usize,
        usize,
        KeyConflictShape,
    ) -> Result<(), String>,
{
    ensure_not_cancelled(is_cancelled())?;
    let current_schema = read_parquet_schema_frame(current.path)?;
    let compared_schema = read_parquet_schema_frame(compared.path)?;
    validate_key_columns(&current_schema, &compared_schema, key_columns)?;
    let shared_payload_columns = shared_columns
        .iter()
        .filter(|column| !key_columns.contains(column))
        .cloned()
        .collect::<Vec<_>>();
    if shared_payload_columns.is_empty() {
        return Ok(0);
    }

    let current_rows = spill_parquet_key_payload_rows_with_cancel(
        current.path,
        current.row_count,
        key_columns,
        &shared_payload_columns,
        is_cancelled,
    )?;
    let compared_rows = spill_parquet_key_payload_rows_with_cancel(
        compared.path,
        compared.row_count,
        key_columns,
        &shared_payload_columns,
        is_cancelled,
    )?;
    let row_index_bytes = std::mem::size_of::<u64>() as u64;
    let current_row_bytes = u64::try_from(current.row_count)
        .map_err(|_| "El dataset activo supera la capacidad del índice temporal.".to_owned())?;
    let mut conflict_markers = tempfile::tempfile().map_err(|error| {
        format!("No se pudo preparar el índice temporal de conflictos: {error}")
    })?;
    conflict_markers
        .set_len(current_row_bytes)
        .map_err(|error| {
            format!("No se pudo preparar el índice temporal de conflictos: {error}")
        })?;
    let mut conflict_rows = tempfile::tempfile().map_err(|error| {
        format!("No se pudo preparar el índice temporal de conflictos: {error}")
    })?;

    for bucket in 0..COMPARISON_KEY_BUCKETS {
        ensure_not_cancelled(is_cancelled())?;
        let current_bucket =
            read_spilled_key_payload_bucket_with_cancel(&current_rows, bucket, is_cancelled)?;
        let compared_bucket =
            read_spilled_key_payload_bucket_with_cancel(&compared_rows, bucket, is_cancelled)?;
        let mut record_index = 0usize;
        for (key, current_group) in current_bucket {
            if record_index.is_multiple_of(LOCAL_QUERY_CANCEL_CHECK_ROWS) {
                ensure_not_cancelled(is_cancelled())?;
            }
            record_index = record_index.saturating_add(1);
            let Some(compared_group) = compared_bucket.get(&key) else {
                continue;
            };
            if current_group.count != 1
                || compared_group.count != 1
                || current_group.payload == compared_group.payload
            {
                continue;
            }
            let marker_offset = u64::try_from(current_group.first_row_index).map_err(|_| {
                "El índice de fila supera la capacidad del índice temporal.".to_owned()
            })?;
            let compared_offset = u64::try_from(current_group.first_row_index)
                .ok()
                .and_then(|index| index.checked_mul(row_index_bytes))
                .ok_or_else(|| "El índice de conflicto supera la capacidad local.".to_owned())?;
            let compared_row_index =
                u64::try_from(compared_group.first_row_index).map_err(|_| {
                    "El índice de fila supera la capacidad del índice temporal.".to_owned()
                })?;
            conflict_markers
                .seek(SeekFrom::Start(marker_offset))
                .and_then(|_| conflict_markers.write_all(&[1]))
                .map_err(|error| {
                    format!("No se pudo escribir el índice temporal de conflictos: {error}")
                })?;
            conflict_rows
                .seek(SeekFrom::Start(compared_offset))
                .and_then(|_| conflict_rows.write_all(&compared_row_index.to_le_bytes()))
                .map_err(|error| {
                    format!("No se pudo escribir el índice temporal de conflictos: {error}")
                })?;
        }
    }

    conflict_markers
        .seek(SeekFrom::Start(0))
        .map_err(|error| format!("No se pudo leer el índice temporal de conflictos: {error}"))?;
    let mut marker_reader = BufReader::new(conflict_markers);
    let mut total = 0usize;
    let mut cached_compared_start = None;
    let mut cached_compared = None;
    for_each_parquet_block_with_cancel(
        current.path,
        current.row_count,
        is_cancelled,
        |start, current_block| {
            for current_row_in_block in 0..current_block.height() {
                if current_row_in_block.is_multiple_of(LOCAL_QUERY_CANCEL_CHECK_ROWS) {
                    ensure_not_cancelled(is_cancelled())?;
                }
                let mut marker = [0_u8; 1];
                marker_reader.read_exact(&mut marker).map_err(|error| {
                    format!("No se pudo leer el índice temporal de conflictos: {error}")
                })?;
                if marker[0] == 0 {
                    continue;
                }
                let current_row_index = start
                    .checked_add(current_row_in_block)
                    .ok_or_else(|| "El índice de fila supera la capacidad local.".to_owned())?;
                let row_offset = u64::try_from(current_row_index)
                    .ok()
                    .and_then(|index| index.checked_mul(row_index_bytes))
                    .ok_or_else(|| {
                        "El índice de conflicto supera la capacidad local.".to_owned()
                    })?;
                conflict_rows
                    .seek(SeekFrom::Start(row_offset))
                    .map_err(|error| {
                        format!("No se pudo leer el índice temporal de conflictos: {error}")
                    })?;
                let mut compared_row_bytes = [0_u8; std::mem::size_of::<u64>()];
                conflict_rows
                    .read_exact(&mut compared_row_bytes)
                    .map_err(|error| {
                        format!("No se pudo leer el índice temporal de conflictos: {error}")
                    })?;
                let compared_row_index = usize::try_from(u64::from_le_bytes(compared_row_bytes))
                    .map_err(|_| {
                        "El índice de fila temporal excede la capacidad local.".to_owned()
                    })?;
                if compared_row_index >= compared.row_count {
                    return Err(
                        "El índice de conflicto apunta fuera del snapshot comparado.".to_owned(),
                    );
                }
                let compared_start = (compared_row_index / LOCAL_QUERY_BLOCK_ROWS)
                    .checked_mul(LOCAL_QUERY_BLOCK_ROWS)
                    .ok_or_else(|| "El índice del bloque excede la capacidad local.".to_owned())?;
                if cached_compared_start != Some(compared_start) {
                    let length = LOCAL_QUERY_BLOCK_ROWS.min(compared.row_count - compared_start);
                    ensure_not_cancelled(is_cancelled())?;
                    let block = read_parquet_query_block_with_cancel(
                        compared.path,
                        compared_start,
                        length,
                        is_cancelled,
                    )?;
                    if block.height() != length {
                        return Err("El snapshot comparado cambió durante la lectura.".to_owned());
                    }
                    ensure_not_cancelled(is_cancelled())?;
                    cached_compared_start = Some(compared_start);
                    cached_compared = Some(block);
                }
                let compared_block = cached_compared
                    .as_ref()
                    .ok_or_else(|| "No se pudo conservar el bloque del conflicto.".to_owned())?;
                let compared_row_in_block = compared_row_index - compared_start;
                let Some(shape) = build_key_conflict_shape(
                    current_block,
                    compared_block,
                    &shared_payload_columns,
                    current_row_in_block,
                    compared_row_in_block,
                )?
                else {
                    continue;
                };
                if max_conflicts.is_some_and(|limit| total >= limit) {
                    return Err(SOURCE_BACKED_RESOLUTION_LIMIT_REACHED.to_owned());
                }
                let conflict_index = total;
                total = total.checked_add(1).ok_or_else(|| {
                    "El índice de conflictos supera la capacidad local.".to_owned()
                })?;
                visit(
                    conflict_index,
                    current_block,
                    current_row_in_block,
                    compared_block,
                    compared_row_in_block,
                    current_row_index,
                    compared_row_index,
                    shape,
                )?;
            }
            Ok(())
        },
    )?;
    ensure_not_cancelled(is_cancelled())?;
    Ok(total)
}

pub(super) fn collect_key_conflicts_page_between_parquet(
    current: ParquetComparisonSource<'_>,
    compared: ParquetComparisonSource<'_>,
    key_columns: &[String],
    shared_columns: &[String],
    offset: usize,
    limit: usize,
) -> Result<(Vec<KeyConflictRows>, bool), String> {
    collect_key_conflicts_page_between_parquet_with_cancel(
        current,
        compared,
        key_columns,
        shared_columns,
        offset,
        limit,
        &|| false,
    )
}

pub(super) fn collect_key_conflicts_page_between_parquet_with_cancel<C>(
    current: ParquetComparisonSource<'_>,
    compared: ParquetComparisonSource<'_>,
    key_columns: &[String],
    shared_columns: &[String],
    offset: usize,
    limit: usize,
    is_cancelled: &C,
) -> Result<(Vec<KeyConflictRows>, bool), String>
where
    C: Fn() -> bool + Sync,
{
    ensure_not_cancelled(is_cancelled())?;
    let mut conflicts = Vec::new();
    let total = for_each_key_conflict_between_parquet_with_cancel(
        current,
        compared,
        key_columns,
        shared_columns,
        None,
        is_cancelled,
        |conflict_index,
         current_block,
         current_row_in_block,
         compared_block,
         compared_row_in_block,
         current_row_index,
         compared_row_index,
         shape| {
            if conflict_index < offset || conflicts.len() >= limit {
                return Ok(());
            }
            let mut conflict = build_key_conflict_from_shape(
                current_block,
                compared_block,
                key_columns,
                shape,
                current_row_in_block,
                compared_row_in_block,
            )?;
            conflict.current_row_index = current_row_index;
            conflict.compared_row_index = compared_row_index;
            conflicts.push(conflict);
            Ok(())
        },
    )?;
    let page_end = offset.saturating_add(conflicts.len());
    ensure_not_cancelled(is_cancelled())?;
    Ok((conflicts, total > page_end))
}

pub(super) fn source_backed_parquet_snapshot(
    context: &SourceBackedJoinContext,
) -> Result<Option<(PathBuf, usize, Option<tempfile::TempDir>)>, String> {
    source_backed_parquet_snapshot_with_cancel(context, || false)
}

pub(super) fn source_backed_parquet_snapshot_with_cancel<C>(
    context: &SourceBackedJoinContext,
    is_cancelled: C,
) -> Result<Option<(PathBuf, usize, Option<tempfile::TempDir>)>, String>
where
    C: Fn() -> bool + Clone + Send + 'static,
{
    if matches!(
        context.source_format,
        crate::duckdb_query::DuckDbFileFormat::Parquet
    ) {
        return Ok(Some((context.source_path.clone(), context.row_count, None)));
    }
    let directory = tempfile::tempdir().map_err(|error| {
        format!("No se pudo preparar el snapshot temporal source-backed: {error}")
    })?;
    let path = directory.path().join("current.parquet");
    if crate::duckdb_query::materialize_file_to_parquet_with_projection(
        &context.source_path,
        context.source_format,
        &path,
        "*",
        is_cancelled.clone(),
    )
    .is_err()
    {
        ensure_not_cancelled(is_cancelled())?;
        return Ok(None);
    }
    let row_count = crate::duckdb_query::count_file_rows(
        &path,
        crate::duckdb_query::DuckDbFileFormat::Parquet,
        is_cancelled,
    )?;
    Ok(Some((path, row_count, Some(directory))))
}

pub(super) fn disk_backed_conflict_page(
    state: &DatasetState,
    compared_path: &Path,
    compared_row_count: usize,
    key_columns: &[String],
    offset: usize,
    limit: usize,
) -> Result<Option<DatasetConflictPage>, String> {
    disk_backed_conflict_page_with_cancel(
        state,
        compared_path,
        compared_row_count,
        key_columns,
        offset,
        limit,
        || false,
    )
}

pub(super) fn disk_backed_conflict_page_with_cancel<C>(
    state: &DatasetState,
    compared_path: &Path,
    compared_row_count: usize,
    key_columns: &[String],
    offset: usize,
    limit: usize,
    is_cancelled: C,
) -> Result<Option<DatasetConflictPage>, String>
where
    C: Fn() -> bool + Clone + Send + Sync + 'static,
{
    ensure_not_cancelled(is_cancelled())?;
    let Some(context) = state
        .current
        .lock()
        .map_err(|_| "La sesión de datos quedó bloqueada inesperadamente.".to_owned())?
        .as_ref()
        .and_then(current_join_context)
    else {
        return Ok(None);
    };
    ensure_not_cancelled(is_cancelled())?;
    let (_, source_size_before, _) = validate_dataset_file(&context.source_path)?;
    ensure_not_cancelled(is_cancelled())?;
    if source_size_before != context.source_size_bytes {
        return Err("La fuente source-backed cambió antes de leer los conflictos.".to_owned());
    }
    validate_dataset_file(compared_path)?;
    ensure_not_cancelled(is_cancelled())?;

    let Some((current_path, current_row_count, _current_snapshot_directory)) =
        source_backed_parquet_snapshot_with_cancel(&context, is_cancelled.clone())?
    else {
        return Ok(None);
    };
    let current_schema = read_parquet_schema_frame(&current_path)?;
    ensure_not_cancelled(is_cancelled())?;
    let compared_schema = read_parquet_schema_frame(compared_path)?;
    ensure_not_cancelled(is_cancelled())?;
    let current_columns = current_schema
        .get_column_names()
        .iter()
        .map(|name| name.to_string())
        .collect::<Vec<_>>();
    let compared_columns = compared_schema
        .get_column_names()
        .iter()
        .map(|name| name.to_string())
        .collect::<Vec<_>>();
    let shared_columns = current_columns
        .iter()
        .filter(|name| compared_columns.contains(name))
        .cloned()
        .collect::<Vec<_>>();
    let (conflicts, has_next) = collect_key_conflicts_page_between_parquet_with_cancel(
        ParquetComparisonSource {
            path: &current_path,
            row_count: current_row_count,
        },
        ParquetComparisonSource {
            path: compared_path,
            row_count: compared_row_count,
        },
        key_columns,
        &shared_columns,
        offset,
        limit,
        &is_cancelled,
    )?;
    ensure_not_cancelled(is_cancelled())?;
    let (_, source_size_after, _) = validate_dataset_file(&context.source_path)?;
    ensure_not_cancelled(is_cancelled())?;
    if source_size_after != context.source_size_bytes {
        return Err("La fuente source-backed cambió durante la lectura de conflictos.".to_owned());
    }
    let current_context = state
        .current
        .lock()
        .map_err(|_| "La sesión de datos quedó bloqueada inesperadamente.".to_owned())?
        .as_ref()
        .and_then(current_join_context);
    if current_context.as_ref().map(|value| &value.source_path) != Some(&context.source_path)
        || current_context
            .as_ref()
            .map(|value| value.source_size_bytes)
            != Some(context.source_size_bytes)
        || current_context.as_ref().map(|value| value.row_count) != Some(context.row_count)
    {
        return Err("El dataset activo cambió durante la lectura de conflictos.".to_owned());
    }
    Ok(Some(DatasetConflictPage {
        offset,
        conflicts: conflicts.into_iter().map(|item| item.conflict).collect(),
        has_next,
    }))
}

pub(super) fn collect_key_conflicts(
    current: &DataFrame,
    compared: &DataFrame,
    key_columns: &[String],
    shared_columns: &[String],
) -> Result<(Vec<KeyConflictRows>, bool), String> {
    collect_key_conflicts_page(
        current,
        compared,
        key_columns,
        shared_columns,
        0,
        MAX_CONFLICT_PREVIEW,
    )
}

pub(super) fn collect_all_key_conflicts(
    current: &DataFrame,
    compared: &DataFrame,
    key_columns: &[String],
    shared_columns: &[String],
) -> Result<Vec<KeyConflictRows>, String> {
    collect_all_key_conflicts_with_cancel(current, compared, key_columns, shared_columns, &|| false)
}

pub(super) fn collect_all_key_conflicts_with_cancel<C>(
    current: &DataFrame,
    compared: &DataFrame,
    key_columns: &[String],
    shared_columns: &[String],
    is_cancelled: &C,
) -> Result<Vec<KeyConflictRows>, String>
where
    C: Fn() -> bool + Sync,
{
    let (conflicts, _) = collect_key_conflicts_page_with_cancel(
        current,
        compared,
        key_columns,
        shared_columns,
        0,
        usize::MAX,
        is_cancelled,
    )?;
    Ok(conflicts)
}

pub(super) fn normalize_key_columns(
    key_columns: Option<Vec<String>>,
) -> Result<Vec<String>, String> {
    let mut normalized = Vec::new();
    for key in key_columns.unwrap_or_default() {
        if key.is_empty() {
            return Err("Las columnas clave no pueden estar vacías.".to_owned());
        }
        if !normalized.contains(&key) {
            normalized.push(key);
        }
    }
    if normalized.len() > 16 {
        return Err("La comparación admite como máximo 16 columnas clave.".to_owned());
    }
    Ok(normalized)
}

pub(super) fn rows_with_new_keys(
    current: &DataFrame,
    compared: &DataFrame,
    key_columns: &[String],
) -> Result<DataFrame, String> {
    rows_with_new_keys_with_cancel(current, compared, key_columns, || false)
}

pub(super) fn rows_with_new_keys_with_cancel<C>(
    current: &DataFrame,
    compared: &DataFrame,
    key_columns: &[String],
    is_cancelled: C,
) -> Result<DataFrame, String>
where
    C: Fn() -> bool + Sync,
{
    ensure_not_cancelled(is_cancelled())?;
    validate_key_columns(current, compared, key_columns)?;
    let current_keys = spill_key_rows_with_cancel(current, key_columns, &is_cancelled)?;
    let compared_keys = spill_key_rows_with_cancel(compared, key_columns, &is_cancelled)?;
    let mut keep = vec![false; compared.height()];
    for bucket in 0..COMPARISON_KEY_BUCKETS {
        ensure_not_cancelled(is_cancelled())?;
        let current_bucket =
            read_spilled_key_bucket_with_cancel(&current_keys, bucket, &is_cancelled)?;
        let current_bucket = &current_bucket;
        for_each_spilled_key_record_with_cancel(
            &compared_keys,
            bucket,
            &is_cancelled,
            |signature, row_index| {
                if let Some(value) = keep.get_mut(row_index) {
                    *value = !current_bucket.contains_key(&signature);
                }
                Ok(())
            },
        )?;
    }
    ensure_not_cancelled(is_cancelled())?;
    compared
        .filter(&BooleanChunked::from_slice("new_keys".into(), &keep))
        .map_err(|error| format!("No se pudieron seleccionar las claves nuevas: {error}"))
}

pub(super) fn join_frames(
    current: &DataFrame,
    compared: &DataFrame,
    key_columns: &[String],
    join_type: DatasetJoinType,
) -> Result<DataFrame, String> {
    join_frames_on_keys(current, compared, key_columns, key_columns, join_type)
}

pub(super) fn join_frames_on_keys(
    current: &DataFrame,
    compared: &DataFrame,
    current_keys: &[String],
    compared_keys: &[String],
    join_type: DatasetJoinType,
) -> Result<DataFrame, String> {
    join_frames_on_keys_with_cancel(
        current,
        compared,
        current_keys,
        compared_keys,
        join_type,
        &|| false,
    )
}

pub(super) fn join_cardinality_upper_bound<C>(
    current: &DataFrame,
    compared: &DataFrame,
    current_keys: &[String],
    compared_keys: &[String],
    join_type: DatasetJoinType,
    is_cancelled: &C,
) -> Result<usize, String>
where
    C: Fn() -> bool + Sync,
{
    let current_index = spill_key_rows_with_cancel(current, current_keys, is_cancelled)?;
    let compared_index = spill_key_rows_with_cancel(compared, compared_keys, is_cancelled)?;
    let mut inner_rows = 0usize;
    for bucket in 0..COMPARISON_KEY_BUCKETS {
        let current_bucket =
            read_spilled_key_bucket_with_cancel(&current_index, bucket, is_cancelled)?;
        let compared_bucket =
            read_spilled_key_bucket_with_cancel(&compared_index, bucket, is_cancelled)?;
        for (key, current_group) in current_bucket {
            let Some(compared_group) = compared_bucket.get(&key) else {
                continue;
            };
            let matching_rows = current_group
                .count
                .checked_mul(compared_group.count)
                .ok_or_else(|| "El cardinal del JOIN supera la capacidad local.".to_owned())?;
            inner_rows = inner_rows
                .checked_add(matching_rows)
                .ok_or_else(|| "El cardinal del JOIN supera la capacidad local.".to_owned())?;
            if inner_rows > LOCAL_QUERY_JOIN_MAX_RESULT_ROWS {
                return Ok(inner_rows);
            }
        }
    }

    let bound = match join_type {
        DatasetJoinType::Inner => inner_rows,
        DatasetJoinType::Left => inner_rows
            .checked_add(current.height())
            .ok_or_else(|| "El cardinal del JOIN supera la capacidad local.".to_owned())?,
        DatasetJoinType::Full => inner_rows
            .checked_add(current.height())
            .and_then(|value| value.checked_add(compared.height()))
            .ok_or_else(|| "El cardinal del JOIN supera la capacidad local.".to_owned())?,
    };
    ensure_not_cancelled(is_cancelled())?;
    Ok(bound)
}

pub(super) fn validate_join_inputs_and_cardinality_with_cancel<C>(
    current: &DataFrame,
    compared: &DataFrame,
    current_keys: &[String],
    compared_keys: &[String],
    join_type: DatasetJoinType,
    is_cancelled: &C,
) -> Result<(), String>
where
    C: Fn() -> bool + Sync,
{
    if current_keys.is_empty() || current_keys.len() != compared_keys.len() {
        return Err(
            "El JOIN necesita el mismo número de columnas clave en ambos datasets.".to_owned(),
        );
    }
    if current.height().saturating_add(compared.height()) > LOCAL_QUERY_JOIN_MAX_INPUT_ROWS {
        return Err(format!(
            "El JOIN local limita las entradas a {LOCAL_QUERY_JOIN_MAX_INPUT_ROWS} filas para proteger la memoria."
        ));
    }
    for (current_key, compared_key) in current_keys.iter().zip(compared_keys) {
        let current_column = current.column(current_key).map_err(|_| {
            format!("La columna clave '{current_key}' no existe en el dataset activo.")
        })?;
        let compared_column = compared.column(compared_key).map_err(|_| {
            format!("La columna clave '{compared_key}' no existe en el dataset comparado.")
        })?;
        if current_column.dtype() != compared_column.dtype() {
            return Err(format!(
                "Las columnas clave '{current_key}' y '{compared_key}' tienen tipos incompatibles."
            ));
        }
    }
    ensure_not_cancelled(is_cancelled())?;
    let cardinality = join_cardinality_upper_bound(
        current,
        compared,
        current_keys,
        compared_keys,
        join_type,
        is_cancelled,
    )?;
    if cardinality > LOCAL_QUERY_JOIN_MAX_RESULT_ROWS {
        return Err(format!(
            "El resultado estimado del JOIN supera el límite local de {LOCAL_QUERY_JOIN_MAX_RESULT_ROWS} filas; reduce los duplicados de las claves."
        ));
    }
    Ok(())
}

pub(super) fn collect_join_frame_on_keys_with_cancel<C>(
    current: &DataFrame,
    compared: &DataFrame,
    current_keys: &[String],
    compared_keys: &[String],
    join_type: DatasetJoinType,
    is_cancelled: &C,
) -> Result<DataFrame, String>
where
    C: Fn() -> bool + Sync,
{
    collect_join_frame_on_keys_with_null_order(
        current,
        compared,
        current_keys,
        compared_keys,
        join_type,
        false,
        is_cancelled,
    )
}

pub(super) fn collect_join_frame_on_keys_with_null_order<C>(
    current: &DataFrame,
    compared: &DataFrame,
    current_keys: &[String],
    compared_keys: &[String],
    join_type: DatasetJoinType,
    nulls_last: bool,
    is_cancelled: &C,
) -> Result<DataFrame, String>
where
    C: Fn() -> bool + Sync,
{
    ensure_not_cancelled(is_cancelled())?;
    let current_order_column = (0..)
        .map(|suffix| {
            if suffix == 0 {
                "__columnia_join_current_order".to_owned()
            } else {
                format!("__columnia_join_current_order_{suffix}")
            }
        })
        .find(|candidate| {
            current.get_column_index(candidate).is_none()
                && compared.get_column_index(candidate).is_none()
        })
        .ok_or_else(|| "No se pudo preparar el orden interno del JOIN local.".to_owned())?;
    let compared_order_column = (0..)
        .map(|suffix| {
            if suffix == 0 {
                "__columnia_join_compared_order".to_owned()
            } else {
                format!("__columnia_join_compared_order_{suffix}")
            }
        })
        .find(|candidate| {
            candidate != &current_order_column
                && current.get_column_index(candidate).is_none()
                && compared.get_column_index(candidate).is_none()
        })
        .ok_or_else(|| "No se pudo preparar el orden interno del JOIN local.".to_owned())?;
    let current = current
        .with_row_index(current_order_column.clone().into(), None)
        .map_err(|error| format!("No se pudo preparar el orden del JOIN local: {error}"))?;
    let compared = compared
        .with_row_index(compared_order_column.clone().into(), None)
        .map_err(|error| format!("No se pudo preparar el orden del JOIN local: {error}"))?;
    let left_on = current_keys.iter().map(col).collect::<Vec<_>>();
    let right_on = compared_keys.iter().map(col).collect::<Vec<_>>();
    let mut join_args =
        JoinArgs::new(join_type.polars_type()).with_coalesce(JoinCoalesce::CoalesceColumns);
    join_args.maintain_order = MaintainOrderJoin::Left;
    let plan = current
        .clone()
        .lazy()
        .join(compared.clone().lazy(), left_on, right_on, join_args)
        .sort(
            [
                current_order_column.as_str(),
                compared_order_column.as_str(),
            ],
            SortMultipleOptions::default().with_nulls_last(nulls_last),
        );
    let mut joined =
        collect_lazy_frame_streaming(plan, "No se pudieron unir los datasets por clave")?;
    joined
        .drop_in_place(current_order_column.as_str())
        .map_err(|error| format!("No se pudo retirar el orden interno del JOIN local: {error}"))?;
    joined
        .drop_in_place(compared_order_column.as_str())
        .map_err(|error| format!("No se pudo retirar el orden interno del JOIN local: {error}"))?;
    ensure_not_cancelled(is_cancelled())?;
    if joined.height() > LOCAL_QUERY_JOIN_MAX_RESULT_ROWS {
        return Err(format!(
            "El resultado del JOIN supera el límite local de {LOCAL_QUERY_JOIN_MAX_RESULT_ROWS} filas."
        ));
    }
    Ok(joined)
}

pub(super) fn join_frames_on_keys_with_cancel<C>(
    current: &DataFrame,
    compared: &DataFrame,
    current_keys: &[String],
    compared_keys: &[String],
    join_type: DatasetJoinType,
    is_cancelled: &C,
) -> Result<DataFrame, String>
where
    C: Fn() -> bool + Sync,
{
    validate_join_inputs_and_cardinality_with_cancel(
        current,
        compared,
        current_keys,
        compared_keys,
        join_type,
        is_cancelled,
    )?;
    collect_join_frame_on_keys_with_null_order(
        current,
        compared,
        current_keys,
        compared_keys,
        join_type,
        matches!(join_type, DatasetJoinType::Full),
        is_cancelled,
    )
}

pub(super) fn join_frames_on_keys_in_blocks_with_cancel<C>(
    current: &DataFrame,
    compared: &DataFrame,
    current_keys: &[String],
    compared_keys: &[String],
    join_type: DatasetJoinType,
    is_cancelled: &C,
) -> Result<DataFrame, String>
where
    C: Fn() -> bool + Sync,
{
    validate_join_inputs_and_cardinality_with_cancel(
        current,
        compared,
        current_keys,
        compared_keys,
        join_type,
        is_cancelled,
    )?;
    let spec = LocalJoinQuerySpec {
        current_keys: current_keys.to_vec(),
        compared_keys: compared_keys.to_vec(),
        join_type,
        normalized_query: String::new(),
    };
    let mut joined: Option<DataFrame> = None;
    visit_local_join_blocks_with_cancel(current, compared, &spec, is_cancelled, |block| {
        ensure_not_cancelled(is_cancelled())?;
        if let Some(joined) = joined.as_mut() {
            joined.vstack_mut(&block).map_err(|error| {
                format!("No se pudieron acumular los bloques del JOIN: {error}")
            })?;
        } else {
            joined = Some(block);
        }
        Ok(())
    })?;
    ensure_not_cancelled(is_cancelled())?;
    if let Some(joined) = joined {
        return Ok(joined);
    }

    let empty_current = current.slice(0, 0);
    let empty_compared = compared.slice(0, 0);
    collect_join_frame_on_keys_with_cancel(
        &empty_current,
        &empty_compared,
        current_keys,
        compared_keys,
        join_type,
        is_cancelled,
    )
}

pub(super) fn compare_frames(
    current: &DataFrame,
    current_file_name: &str,
    compared: &DataFrame,
    compared_file_name: &str,
    key_columns: &[String],
) -> Result<DatasetComparison, String> {
    compare_frames_with_cancel(
        current,
        current_file_name,
        compared,
        compared_file_name,
        key_columns,
        &|| false,
    )
}

pub(super) fn compare_frames_with_cancel<C>(
    current: &DataFrame,
    current_file_name: &str,
    compared: &DataFrame,
    compared_file_name: &str,
    key_columns: &[String],
    is_cancelled: &C,
) -> Result<DatasetComparison, String>
where
    C: Fn() -> bool + Sync,
{
    ensure_not_cancelled(is_cancelled())?;
    let current_columns = current
        .get_column_names()
        .iter()
        .map(|name| name.to_string())
        .collect::<Vec<_>>();
    let compared_columns = compared
        .get_column_names()
        .iter()
        .map(|name| name.to_string())
        .collect::<Vec<_>>();
    let shared_columns = current_columns
        .iter()
        .filter(|name| compared_columns.contains(name))
        .cloned()
        .collect::<Vec<_>>();
    let current_only_columns = current_columns
        .iter()
        .filter(|name| !compared_columns.contains(name))
        .cloned()
        .collect::<Vec<_>>();
    let compared_only_columns = compared_columns
        .iter()
        .filter(|name| !current_columns.contains(name))
        .cloned()
        .collect::<Vec<_>>();
    let schema_compatible = current_columns == compared_columns
        && current
            .columns()
            .iter()
            .zip(compared.columns())
            .all(|(left, right)| left.dtype() == right.dtype());
    let (common_row_count, current_only_row_count, compared_only_row_count) =
        if shared_columns.is_empty() {
            (0, current.height(), compared.height())
        } else {
            let common = common_row_count_from_spilled_signatures_with_cancel(
                current,
                compared,
                &shared_columns,
                is_cancelled,
            )?;
            (
                common,
                current.height().saturating_sub(common),
                compared.height().saturating_sub(common),
            )
        };
    let (key_summary, conflicts, conflicts_truncated) = if key_columns.is_empty() {
        (KeyComparisonSummary::default(), Vec::new(), false)
    } else {
        let summary = compare_keyed_frames_with_cancel(
            current,
            compared,
            key_columns,
            &shared_columns,
            || is_cancelled(),
        )?;
        let (conflicts, truncated) = collect_key_conflicts_page_with_cancel(
            current,
            compared,
            key_columns,
            &shared_columns,
            0,
            MAX_CONFLICT_PREVIEW,
            is_cancelled,
        )?;
        (summary, conflicts, truncated)
    };
    let can_consolidate = schema_compatible
        && key_summary.conflicting_key_count == 0
        && key_summary.duplicate_key_count == 0;

    ensure_not_cancelled(is_cancelled())?;
    Ok(DatasetComparison {
        current_file_name: current_file_name.to_owned(),
        compared_file_name: compared_file_name.to_owned(),
        current_row_count: current.height(),
        compared_row_count: compared.height(),
        common_row_count,
        current_only_row_count,
        compared_only_row_count,
        shared_columns,
        current_only_columns,
        compared_only_columns,
        schema_compatible,
        key_columns: key_columns.to_vec(),
        matched_key_count: key_summary.matched_key_count,
        current_only_key_count: key_summary.current_only_key_count,
        compared_only_key_count: key_summary.compared_only_key_count,
        conflicting_key_count: key_summary.conflicting_key_count,
        duplicate_key_count: key_summary.duplicate_key_count,
        conflicts: conflicts.into_iter().map(|item| item.conflict).collect(),
        conflict_offset: 0,
        conflicts_truncated,
        can_consolidate,
    })
}

pub(super) fn compare_parquet_source(
    current: &DataFrame,
    current_file_name: &str,
    compared_path: &Path,
    compared_file_name: &str,
    compared_row_count: usize,
    key_columns: &[String],
) -> Result<DatasetComparison, String> {
    let compared = read_parquet_schema_frame(compared_path)?;
    let current_columns = current
        .get_column_names()
        .iter()
        .map(|name| name.to_string())
        .collect::<Vec<_>>();
    let compared_columns = compared
        .get_column_names()
        .iter()
        .map(|name| name.to_string())
        .collect::<Vec<_>>();
    let shared_columns = current_columns
        .iter()
        .filter(|name| compared_columns.contains(name))
        .cloned()
        .collect::<Vec<_>>();
    let current_only_columns = current_columns
        .iter()
        .filter(|name| !compared_columns.contains(name))
        .cloned()
        .collect::<Vec<_>>();
    let compared_only_columns = compared_columns
        .iter()
        .filter(|name| !current_columns.contains(name))
        .cloned()
        .collect::<Vec<_>>();
    let schema_compatible = current_columns == compared_columns
        && current
            .columns()
            .iter()
            .zip(compared.columns())
            .all(|(left, right)| left.dtype() == right.dtype());
    let (common_row_count, current_only_row_count, compared_only_row_count) =
        if shared_columns.is_empty() {
            (0, current.height(), compared_row_count)
        } else {
            let common = common_row_count_from_parquet(
                current,
                compared_path,
                compared_row_count,
                &shared_columns,
            )?;
            (
                common,
                current.height().saturating_sub(common),
                compared_row_count.saturating_sub(common),
            )
        };
    let (key_summary, conflicts, conflicts_truncated) = if key_columns.is_empty() {
        (KeyComparisonSummary::default(), Vec::new(), false)
    } else {
        let summary = compare_keyed_parquet(
            current,
            compared_path,
            compared_row_count,
            key_columns,
            &shared_columns,
        )?;
        let (conflicts, truncated) = collect_key_conflicts_page_from_parquet(
            current,
            compared_path,
            compared_row_count,
            key_columns,
            &shared_columns,
            0,
            MAX_CONFLICT_PREVIEW,
        )?;
        (summary, conflicts, truncated)
    };
    let can_consolidate = schema_compatible
        && key_summary.conflicting_key_count == 0
        && key_summary.duplicate_key_count == 0;

    Ok(DatasetComparison {
        current_file_name: current_file_name.to_owned(),
        compared_file_name: compared_file_name.to_owned(),
        current_row_count: current.height(),
        compared_row_count,
        common_row_count,
        current_only_row_count,
        compared_only_row_count,
        shared_columns,
        current_only_columns,
        compared_only_columns,
        schema_compatible,
        key_columns: key_columns.to_vec(),
        matched_key_count: key_summary.matched_key_count,
        current_only_key_count: key_summary.current_only_key_count,
        compared_only_key_count: key_summary.compared_only_key_count,
        conflicting_key_count: key_summary.conflicting_key_count,
        duplicate_key_count: key_summary.duplicate_key_count,
        conflicts: conflicts.into_iter().map(|item| item.conflict).collect(),
        conflict_offset: 0,
        conflicts_truncated,
        can_consolidate,
    })
}

pub(super) fn compare_parquet_sources(
    current_path: &Path,
    current_file_name: &str,
    current_row_count: usize,
    compared_path: &Path,
    compared_file_name: &str,
    compared_row_count: usize,
    key_columns: &[String],
) -> Result<DatasetComparison, String> {
    compare_parquet_sources_with_cancel(
        current_path,
        current_file_name,
        current_row_count,
        compared_path,
        compared_file_name,
        compared_row_count,
        key_columns,
        &|| false,
    )
}

pub(super) fn compare_parquet_sources_with_cancel<C>(
    current_path: &Path,
    current_file_name: &str,
    current_row_count: usize,
    compared_path: &Path,
    compared_file_name: &str,
    compared_row_count: usize,
    key_columns: &[String],
    is_cancelled: &C,
) -> Result<DatasetComparison, String>
where
    C: Fn() -> bool + Sync,
{
    ensure_not_cancelled(is_cancelled())?;
    let current_schema = read_parquet_schema_frame(current_path)?;
    ensure_not_cancelled(is_cancelled())?;
    let compared_schema = read_parquet_schema_frame(compared_path)?;
    ensure_not_cancelled(is_cancelled())?;
    let current_columns = current_schema
        .get_column_names()
        .iter()
        .map(|name| name.to_string())
        .collect::<Vec<_>>();
    let compared_columns = compared_schema
        .get_column_names()
        .iter()
        .map(|name| name.to_string())
        .collect::<Vec<_>>();
    let shared_columns = current_columns
        .iter()
        .filter(|name| compared_columns.contains(name))
        .cloned()
        .collect::<Vec<_>>();
    let current_only_columns = current_columns
        .iter()
        .filter(|name| !compared_columns.contains(name))
        .cloned()
        .collect::<Vec<_>>();
    let compared_only_columns = compared_columns
        .iter()
        .filter(|name| !current_columns.contains(name))
        .cloned()
        .collect::<Vec<_>>();
    let schema_compatible = current_columns == compared_columns
        && current_schema
            .columns()
            .iter()
            .zip(compared_schema.columns())
            .all(|(left, right)| left.dtype() == right.dtype());
    let (common_row_count, current_only_row_count, compared_only_row_count) =
        if shared_columns.is_empty() {
            (0, current_row_count, compared_row_count)
        } else {
            let common = common_row_count_between_parquet_with_cancel(
                current_path,
                current_row_count,
                compared_path,
                compared_row_count,
                &shared_columns,
                is_cancelled,
            )?;
            (
                common,
                current_row_count.saturating_sub(common),
                compared_row_count.saturating_sub(common),
            )
        };
    let (key_summary, conflicts, conflicts_truncated) = if key_columns.is_empty() {
        (KeyComparisonSummary::default(), Vec::new(), false)
    } else {
        let summary = compare_keyed_parquet_sources_with_cancel(
            ParquetComparisonSource {
                path: current_path,
                row_count: current_row_count,
            },
            ParquetComparisonSource {
                path: compared_path,
                row_count: compared_row_count,
            },
            &current_schema,
            &compared_schema,
            key_columns,
            &shared_columns,
            is_cancelled,
        )?;
        let (conflicts, truncated) = collect_key_conflicts_page_between_parquet_with_cancel(
            ParquetComparisonSource {
                path: current_path,
                row_count: current_row_count,
            },
            ParquetComparisonSource {
                path: compared_path,
                row_count: compared_row_count,
            },
            key_columns,
            &shared_columns,
            0,
            MAX_CONFLICT_PREVIEW,
            is_cancelled,
        )?;
        (summary, conflicts, truncated)
    };
    let can_consolidate = schema_compatible
        && key_summary.conflicting_key_count == 0
        && key_summary.duplicate_key_count == 0;

    ensure_not_cancelled(is_cancelled())?;
    Ok(DatasetComparison {
        current_file_name: current_file_name.to_owned(),
        compared_file_name: compared_file_name.to_owned(),
        current_row_count,
        compared_row_count,
        common_row_count,
        current_only_row_count,
        compared_only_row_count,
        shared_columns,
        current_only_columns,
        compared_only_columns,
        schema_compatible,
        key_columns: key_columns.to_vec(),
        matched_key_count: key_summary.matched_key_count,
        current_only_key_count: key_summary.current_only_key_count,
        compared_only_key_count: key_summary.compared_only_key_count,
        conflicting_key_count: key_summary.conflicting_key_count,
        duplicate_key_count: key_summary.duplicate_key_count,
        conflicts: conflicts.into_iter().map(|item| item.conflict).collect(),
        conflict_offset: 0,
        conflicts_truncated,
        can_consolidate,
    })
}
