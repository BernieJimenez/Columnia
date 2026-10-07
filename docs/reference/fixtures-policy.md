# Política de fixtures

Las fixtures son entradas y salidas pequeñas, deterministas y sintéticas para
probar contratos. No son un almacén de ejemplos reales ni un lugar para copiar
datasets de clientes.

## Permitido

- Filas inventadas sin relación con personas, empresas o expedientes reales.
- Valores obvios de prueba como `Alice`, `Bob`, `Ventas`, `10` o fechas creadas
  para el caso.
- Casos extremos reproducibles: nulos, Unicode, fórmulas como texto, errores de
  validación, colisiones y estructuras incompletas.
- Archivos JSON y CSV versionados; otros formatos solo cuando sean necesarios
  para cubrir una ruta del parser y su origen sintético quede documentado.

## Prohibido

- PII, datos financieros o de salud, credenciales, tokens, cookies y secretos.
- Copias descargadas de clientes, proveedores, sistemas internos o internet.
- Rutas absolutas, nombres de usuario, dumps de bases de datos y artefactos de
  ejecución local.
- Fixtures no deterministas que dependan de la hora, locale, red o una carpeta
  privada.

## Contrato mecánico

[`fixtures/manifest.json`](../../fixtures/manifest.json) enumera los archivos
versionados y declara que cada uno es sintético. `src/governance.test.ts`
comprueba que la lista coincide con el árbol, rechaza extensiones típicas de
secretos y busca marcadores de claves privadas. El manifest debe actualizarse
en el mismo cambio que agrega, elimina o renombra una fixture.

La salida temporal de smokes, benchmarks y validaciones va en `.local/`, que
está ignorado por Git y se elimina al terminar el flujo correspondiente.

## Fixtures de las pruebas del motor

Las pruebas Rust crean sus archivos con `temporary_csv`, `temporary_delimited`
y `temporary_xlsx_with_worksheet`, que devuelven un `TempFixture`: el archivo
(y su carpeta, si la tiene) se borra al salir de la prueba, también cuando una
aserción falla antes. No hay `fs::remove_file(...).expect` de limpieza al
final de las pruebas (QA-39, QA-41).

Las seis pruebas que construyen datos del tamaño de un límite se midieron el
2026-10-07 (tres ejecuciones cada una, `cargo test --lib -- --exact`, Windows
x64, perfil `test`); ninguna supera 0,05 s, así que no se parametrizan (QA-40):

| Prueba | Tamaño | Tiempo |
| --- | --- | --- |
| `duckdb_join_preparation_does_not_apply_the_polars_input_row_limit` | `LOCAL_QUERY_JOIN_MAX_INPUT_ROWS + 1` filas nulas | 0,01–0,02 s |
| `local_query_join_rejects_many_to_many_cardinality_before_materializing` | `LOCAL_QUERY_BLOCK_ROWS + 1 501` filas × 2 | 0,02 s |
| `local_query_aggregate_rejects_matching_rows_over_materialization_budget` | `LOCAL_QUERY_AGGREGATE_MAX_MATCHING_ROWS + 1` filas | 0,01 s |
| `eager_delimited_reader_observes_cancellation_during_batched_collection` | CSV de 250 000 filas | 0,04 s |
| `xlsx_export_rejects_more_rows_than_one_sheet_holds` | 1 048 576 filas | < 0,01 s |
| `accepts_a_dataset_above_the_previous_500_mebibyte_threshold` | archivo disperso de 500 MiB + 1 | < 0,01 s |
