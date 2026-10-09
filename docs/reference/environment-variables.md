# Variables de entorno

Esta es la tabla única de las variables que lee Columnia, al compilar, al
ejecutar o en sus herramientas. Columnia instalada no necesita ninguna: todas
sirven para compilar una distribución, medir o probar. `tools/release.ps1`
aborta si encuentra alguna de las de pruebas definida en la sesión, para que no
influya en un release.

`node tools/check-documentation.mjs` comprueba que cada variable `COLUMNIA_*`
que aparece en el código esté en esta tabla.

| Variable | Cuándo se lee | Para qué sirve |
| --- | --- | --- |
| `COLUMNIA_UPDATER_ENDPOINT` | Al compilar (`option_env!` en `src-tauri/src/updater.rs`) | URL HTTPS del manifiesto de actualizaciones. Si no se define, el updater queda apagado en esa compilación y Columnia no hace ninguna petición de red para actualizarse. Ver [Red, privacidad y telemetría](network-privacy.md). |
| `COLUMNIA_UPDATER_ASSET_BASE_URL` | `tools/release.ps1 -WithUpdater` | URL HTTPS base desde la que se descargarán los instaladores; con ella se escriben las URL del manifiesto. Ver [Publicar un release](../how-to/publish-release.md). |
| `COLUMNIA_UPDATER_CONTRACT_TEST` | `tools/check-updater-manifest.mjs` | Con `1`, acepta una clave pública de prueba; solo la usa `tools/test-updater-manifest.mjs`. |
| `COLUMNIA_TEST_HARNESS_MANIFEST` | Al compilar los tests de Rust (`src-tauri/build.rs`) | Con `1`, incrusta el manifiesto de Windows que necesitan los binarios de test para arrancar. Necesaria para `cargo test` en Windows. |
| `COLUMNIA_TEST_LOCALE` | `src/test/setup.ts` | Locale con el que corre Vitest (p. ej. `es-ES`); `npm run test:locale` la usa para repetir la suite con otra configuración regional. |
| `COLUMNIA_PROBE_PROFILE_FILE`, `COLUMNIA_PROBE_PROFILE_PARTS` | `src-tauri/src/dataset/perf_probe_tests.rs` | Archivo que perfila la sonda de REN-01 (`perf_probe_profile`) y, con cualquier valor, el tiempo de cada columna y de los duplicados. |
| `COLUMNIA_PROBE_COMPARE_ROWS`, `COLUMNIA_PROBE_COMPARE_SHUFFLED` | `src-tauri/src/dataset/perf_probe_tests.rs` | Filas de los dos Parquet que genera la sonda de REN-08 (`perf_probe_keyed_comparison`, 2 000 000 por defecto) y, con cualquier valor, el archivo comparado con las filas en otro orden. |
| `COLUMNIA_PROBE_ROWS` | `src-tauri/src/dataset/perf_probe_tests.rs` | Filas de la sonda de rendimiento (por defecto 300 000). |
| `COLUMNIA_PROBE_RELOAD_FILE`, `COLUMNIA_PROBE_RELOADS`, `COLUMNIA_PROBE_RELOAD_STEPS`, `COLUMNIA_PROBE_RELOAD_PAUSE_MS` | `src-tauri/src/dataset/perf_probe_tests.rs` | CSV, número de recargas (por defecto 20), pasos que se repiten (`load,profile,history`) y pausa en milisegundos antes de medir, en la sonda de memoria de REN-02 (`perf_probe_reload_memory`). |
| `COLUMNIA_PROBE_ODBC` | `tools/probe-webview2-native-selectors.mjs` | Cadena de conexión de SQL Server para que la sonda de la app real pruebe también la entrega remota. |
| `COLUMNIA_PROBE_SCREENSHOT_DIR` | `tools/probe-webview2-native-selectors.mjs` | Carpeta opcional donde la sonda guarda una captura por fase. |
| `COLUMNIA_ODBC_SQLSERVER`, `COLUMNIA_ODBC_POSTGRESQL`, `COLUMNIA_ODBC_MYSQL` | Tests ignorados de `src-tauri/src/remote_databases.rs` | Cadenas de conexión de sesión para las pruebas ODBC externas (`cargo test --lib -- --ignored external_`). Usa una base de pruebas propia. |
| `COLUMNIA_DUCKDB_JOIN_TARGET_MIB` | Test de JOIN grande y `tools/benchmark-duckdb-join.ps1` | Tamaño en MiB del CSV generado para medir el JOIN de DuckDB. |
| `COLUMNIA_BENCHMARK_CANCELLATION_INPUT`, `COLUMNIA_BENCHMARK_CANCELLATION_OUTPUT` | Test de cancelación y `tools/benchmark-datasets.ps1` | Archivo de entrada y carpeta de salida de la medición de cancelación. |
| `COLUMNIA_CANCELLATION_BENCHMARK_JSON` | Salida, no entrada | Prefijo de la línea con la que el test de cancelación entrega su resultado a `tools/benchmark-datasets.ps1`. |
| `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` | WebView2, durante `tools/capture-release-evidence.ps1` | La herramienta la define temporalmente con `--remote-debugging-port` para capturar la evidencia visual y restaura el valor anterior al terminar. |
