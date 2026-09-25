# Trabajo vigente

Revisado: 2026-09-25. Esta es la única cola operativa derivada de la
[`auditoría consolidada`](../../AUDITORIA.md). El historial y las tareas cerradas
permanecen en [`ROADMAP.md`](../../ROADMAP.md#tier-9--valor-operativo-consolidado-abierto-2026-09-14).

Una tarea entra aquí solo si reduce fricción del flujo principal, permite repetir
trabajo, protege resultados o aporta evidencia necesaria para declarar soporte.
Las propuestas condicionadas a demanda se enumeran al final y no son trabajo
comprometido.

## Por qué el Tier 9 sigue abierto

El repositorio sigue avanzando localmente; el checklist completo no se cierra
porque sus gates de soporte requieren evidencia externa. RV07 necesita tres
participantes y datasets de trabajo; RV09, aceptación con lector de pantalla;
RV10, una instancia SQL Server conectable; RV11, sign-off jurídico y una VM
limpia. RV08 depende de la beta, RV14 requiere validar en la herramienta BI
elegida y RV15 permanece condicional a demanda repetida. RV01–RV06 conservan
pendientes de aceptación real donde los indica esta cola. Sin esos insumos, el trabajo ejecutable sigue en cerrar brechas locales de RV01–RV06; RV04 acaba de ampliar la cancelación dentro de la lectura Parquet por lotes y RV16 mantiene extracciones incrementales con contratos Tauri estables. Cada corte y gate se registra aquí.

Avance RV16 del 2026-09-23: hay treinta y ocho módulos con responsabilidades extraídas, entre ellas
validación de workspace/proyecto (`dataset/project_validation.rs`), perfiles
de importación (`dataset/import_profile_validation.rs`) y previsualización
de esquema (`dataset/import_schema_preview.rs`), consultas
(`dataset/local_query.rs`), contratos/evaluación de calidad
(`dataset/quality_contracts.rs`, `dataset/quality_documents.rs` y
`dataset/quality_evaluation.rs`), recetas eager/lazy y source-backed
(`dataset/recipe_eager.rs`, `dataset/recipe_engine.rs` y
`dataset/recipe_source_projection.rs`),
perfiles categóricos/temporales (`dataset/categorical_profile.rs` y
`dataset/temporal_profile.rs`), encabezados y seguridad de exportación
(`dataset/delimited_header_import.rs` y `dataset/csv_formula_safety.rs`),
comparación, ejecución de consultas, automatización CLI, historial de proyectos, rutas seguras, perfiles numéricos y lectura paginada
(`dataset/comparison_engine.rs` concentra índices derramados, comparación por claves,
conflictos y JOIN con cancelación; `dataset/query_execution.rs` concentra los
planes source-backed, la lectura por bloques Parquet, las agregaciones, la
paginación de consultas y el JOIN local con cancelación; `dataset/snapshot_comparison.rs` con
`compare_history_snapshots`, `dataset/automation.rs` concentra los helpers
de inspección, transformación, validación y exportación del CLI sin alterar
sus contratos; `dataset/project_history.rs` concentra captura, restauración y
resúmenes de snapshots de proyectos; `dataset/file_validation.rs`,
`dataset/numeric_profile.rs`, `dataset/page_reader.rs`, `dataset/profile_reader.rs`, `dataset/comparison_reader.rs` y `dataset/history.rs`). La selección manual, el drag/drop, la enumeración de hojas y la revisión previa de encabezados viven en `dataset/import_source_inspection.rs`; `dataset/import_loading.rs` concentra la carga final y el descarte, con validación y publicación atómica; `dataset/json_reader.rs` concentra el parseo cancelable de JSON/JSONL y la inspección de columnas sin cambiar errores ni contratos; `dataset/recipe_documents.rs` concentra validación, migración y persistencia atómica de recetas; `dataset/source_loading.rs` concentra la carga source-backed, materialización cancelable, estimación de recursos y loaders por formato; `dataset/spreadsheet_io.rs` concentra la inspección de libros, la conversión tipada de rangos y los snapshots por bloques con cancelación; `dataset/export_io.rs` concentra escritores, privacidad y publicación atómica de las salidas; `dataset/comparison_io.rs` concentra la carga y persistencia temporal de fuentes comparadas, incluidos sus snapshots y cancelación; `dataset/page_reader.rs` también contiene el comando de paginación con cancelación y fallbacks. Los contratos serializados se reexportan desde `dataset.rs` sin cambiar los nombres JSON ni las rutas Tauri. `dataset/history.rs` contiene `HistoryManager`, la navegación y restauración eager/source-backed cancelables de deshacer/rehacer; `dataset/snapshot_comparison.rs` carga ambas revisiones Parquet por lotes con cancelación y calcula/orquesta la comparación. El lector nuevo
resúmenes de snapshots de proyectos; `dataset/file_validation.rs`,
`dataset/numeric_profile.rs`, `dataset/page_reader.rs`, `dataset/profile_reader.rs`, `dataset/comparison_reader.rs`, `dataset/history.rs` y `dataset/profile_engine.rs`). `dataset/column_cleanup.rs` concentra la limpieza eager de columnas y el enmascarado; `dataset/profile_engine.rs` concentra la inferencia de tipos, estadísticas de texto, perfilado eager/source-backed y agregaciones temporales, conservando límites, cancelación y contratos. La selección manual, el drag/drop, la enumeración de hojas y la revisión previa de encabezados viven en `dataset/import_source_inspection.rs`; `dataset/import_loading.rs` concentra la carga final y el descarte, con validación y publicación atómica; `dataset/json_reader.rs` concentra el parseo cancelable de JSON/JSONL y la inspección de columnas sin cambiar errores ni contratos; `dataset/recipe_documents.rs` concentra validación, migración y persistencia atómica de recetas; `dataset/source_loading.rs` concentra la carga source-backed, materialización cancelable, estimación de recursos y loaders por formato; `dataset/spreadsheet_io.rs` concentra la inspección de libros, la conversión tipada de rangos y los snapshots por bloques con cancelación; `dataset/export_io.rs` concentra escritores, privacidad y publicación atómica de las salidas; `dataset/comparison_io.rs` concentra la carga y persistencia temporal de fuentes comparadas, incluidos sus snapshots y cancelación; `dataset/page_reader.rs` también contiene el comando de paginación con cancelación y fallbacks. Los contratos serializados se reexportan desde `dataset.rs` sin cambiar los nombres JSON ni las rutas Tauri. `dataset/history.rs` contiene `HistoryManager`, la navegación y restauración eager/source-backed cancelables de deshacer/rehacer; `dataset/snapshot_comparison.rs` carga ambas revisiones Parquet por lotes con cancelación y calcula/orquesta la comparación. El lector nuevo
preserva los límites y errores de página, la cancelación, el slice pushdown
Parquet y la detección de cambios en la fuente; no altera el contrato Tauri.
El corte actual registra 494 pruebas aprobadas y 5 ignoradas. En esta extracción pasan `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check`, `cargo check --manifest-path src-tauri/Cargo.toml --lib`, `npm run ipc:check` (85/4/70), `node tools/check-documentation.mjs` y `git diff --check`; la suite Rust se ejecutó completa sin fallos. RV16 sigue abierta.

La evidencia nativa más reciente está en
`.local/validation/webview2-cdp/20260921T225717Z/summary.json`: el smoke pasa
los diálogos reales de Windows y round-trips de bytes, filas, esquema y valores
para CSV, XLSX y Parquet. El probe enumera las hojas Excel con
`inspect_workbook_sheets` después de recibir la selección; el aviso de memoria
corresponde al ejecutable debug y no cierra el gate de memoria del release.
El benchmark WebView2 de 100 MiB pasa con 819.137 filas en
`.local/validation/performance-webview2/20260921T230202Z/summary.json`; el
smoke CDP completo pasa Playwright, foco, ProjectsPanel y operaciones IPC de
proyecto en `.local/validation/webview2-cdp/20260921T230251Z/summary.json`.

La verificación frontend actual pasa `457/457` pruebas en 51 archivos con
`npx vitest run --maxWorkers=1`; `npm run test:coverage` aprueba la cobertura
global y las cinco capas críticas (86,17 % de sentencias y 81,34 % de ramas
globales). `npm run test:e2e` pasa `22/22` y ejecuta el build de producción; la
suite Rust pasa 494 pruebas ejecutables, con 5 casos externos u opt-in ignorados.
Las regresiones de App reflejan la inspección Excel en dos llamadas, la carga
perezosa de Preparar, el análisis antes de Entregar y los nombres accesibles
vigentes. Los gates beta, legal e IPC pasan; RV07, RV09, RV10 y RV11 siguen
requiriendo evidencia externa.

Los perfiles de `tools/check.ps1` fijan `--maxWorkers=1` tanto para Vitest como
para cobertura. Así `verify:tier` usa la misma ejecución acotada que la evidencia
frontend aprobada y evita procesos huérfanos en Windows; `npm test` fuera del
gate conserva su configuración normal.

`npm run release:dry-run` sobre `1.25.0` valida árbol limpio, toolchains,
documentación, IPC, gobernanza y el gate legal técnico, pero se detiene en el
sign-off jurídico antes de empaquetar. No se crean ni publican artefactos; el
reporte es `.local/validation/20260921T232220Z-a8fb7b4-package.json`.

El protocolo beta incluye `npm run beta:check-summary`: valida el resumen Gate 1
contra las tres sesiones y el reporte Full del mismo release candidate, sin
exigir todavía el commit posterior de Gate 2. También bloquea en el resumen
versionado alias de participantes o datasets, correos, rutas locales y
credenciales. La evidencia humana de RV07 aún no existe en este checkout, por
lo que el comando solo podrá aprobarse cuando se completen las tres sesiones
reales.

Desde `ca745d9`, `list_projects` devuelve proyectos
y candidato de recuperación en una sola respuesta y transacción de lectura
SQLite bajo el token `projectCatalog`. La migración SQLite v15 agrega el índice
parcial que sirve a la consulta del candidato. La lista comprueba cancelación al leer cada fila y
medir el snapshot y el historial; el candidato se selecciona por ID y consulta
cancelación antes y después de su lectura SQLite síncrona. Las respuestas
obsoletas se descartan y el panel ofrece «Cancelar carga» y reintento. El
guardado manual y automático usan `projectSave`, copian snapshots e historial
por bloques y descartan la generación staged si cancelar gana antes del commit; `ParquetWriter::finish`,
`sync_all` y SQLite siguen siendo tramos síncronos. El borrado usa
`projectDelete`: cancelar antes del gate conserva catálogo y archivos; si el
gate gana, el borrado y la limpieza terminan bajo ese gate. Las llamadas SQLite
y las syscalls finales del borrado explícito siguen siendo síncronas. La
reconciliación de generaciones huérfanas comprueba cancelación al enumerar y
validar entradas, y antes/después de cada `remove_dir_all` seguro. La llamada
recursiva no se interrumpe mientras el sistema operativo la ejecuta; la
cancelación se reconoce al regresar. El incremento sobre `ad10fb4`
hace que la vista previa de encabezados CSV/TSV use la generación `load`: la
muestra de 64 KiB se lee en bloques de 8 KiB, el token se revisa entre bloques
y parseos síncronos, y el diálogo cancela antes de descartar la selección. La
carga completa usa el mismo detector cancelable antes de comenzar la lectura
Polars. Pasan `cargo fmt --check`, `cargo check --manifest-path src-tauri/Cargo.toml --lib`, `npm run build`,
`npm run ipc:check`, el checker documental y `git diff --check`; no se ejecutaron
pruebas de producto.

El incremento sobre `0f4be2f` ordena inicio, cancelación y publicación de
la carga con `load_commit_lock`. La selección inicial y las hojas inspeccionadas
se publican solo si su generación sigue vigente. La carga prepara el historial
fuera del gate y, antes de publicar, valida el token y el ID pendiente; dataset,
comparación y selección se sustituyen juntos. Si cancelar gana durante la
creación síncrona del historial, el candidato se descarta y la sesión anterior
permanece activa. Pasan `cargo fmt --check`, `cargo check --manifest-path src-tauri/Cargo.toml --lib`,
`npm run build`, `npm run ipc:check` y `git diff --check`; no se ejecutaron
pruebas de producto.

El incremento sobre `10f8d43` mueve `test_database_connection` a
`spawn_blocking` y lo asocia a `databaseConnection`, separado del token de
preflight para que cancelar una prueba no invalide otro análisis remoto.
Comprueba el token antes y
después de inicializar ODBC, conectar y ejecutar `SELECT 1`; las llamadas del
driver siguen siendo síncronas y el resultado se descarta al retornar si se
canceló. El comando independiente no tiene hoy un consumidor en la interfaz.
Pasan `cargo fmt --check`, `cargo check --manifest-path src-tauri/Cargo.toml --lib`, `npm run build`,
`npm run ipc:check` y `git diff --check`; no se ejecutaron pruebas de producto.

El incremento sobre `c6d7bda` asocia la comprobación de red del updater
con `updateCheck`. El panel permite cancelarla, `tokio::select!` descarta la
consulta pendiente y un gate común decide entre cancelar y publicar el
resultado. Pasan `cargo fmt --check`, `cargo check --manifest-path src-tauri/Cargo.toml --lib`, `npm run build`,
`npm run ipc:check`, el checker documental y `git diff --check`; no se
ejecutaron pruebas de producto.

El incremento sobre `be2a035` conecta las exportaciones source-backed Parquet y
JSON con el monitor de cancelación de DuckDB durante `COPY`. La operación escribe
en staging, copia al archivo temporal final con comprobaciones cada 64 KiB y no
publica el destino si se interrumpe. `sync_all` sigue síncrono.
Pasan `cargo fmt --check`, `cargo check --manifest-path src-tauri/Cargo.toml --lib` (58 avisos
dead-code existentes), `npm run build`, `npm run ipc:check`, el checker
documental y `git diff --check`; no se ejecutaron pruebas de producto.

El incremento sobre `b34c930` hace cancelable el cálculo SHA-256 del CSV
intermedio de Bundle. El hash revisa el token al iniciar, entre lecturas de 64
KiB y después de rebobinar, tanto en la exportación eager como source-backed; si
cancelar gana, el Bundle incompleto no se publica. La lectura de cada bloque y
`sync_all` siguen siendo síncronos.

El incremento sobre `d65b2b5` propaga cancelación a la protección eager de
privacidad y a las exportaciones eager CSV, JSON y Parquet. Polars procesa bloques
de 8.192 filas; CSV neutraliza fórmulas por bloque y JSON conserva el contrato de
un solo arreglo al combinar los registros. El Bundle eager reutiliza el escritor
CSV cancelable antes del hash del staging. Los cierres del escritor y `sync_all`
siguen siendo síncronos.

El incremento actual sobre `9403de9` comprueba cancelación después de enumerar
el directorio de snapshots y validar metadatos, y antes/después del borrado
recursivo seguro de cada generación huérfana. `std::fs::remove_dir_all` sigue
siendo una syscall síncrona y solo reconoce una cancelación al regresar. La
eliminación explícita de proyectos conserva su gate posterior al commit.

El incremento sobre `94b7fe5` añade `reusableTaskCatalog` para cancelar la
enumeración local de tareas entre filas SQLite y deserializaciones, con
«Cancelar carga» y reintento en el panel. Una operación SQLite o la
deserialización individual siguen siendo síncronas; la UI descarta respuestas
obsoletas y el backend descarta el resultado si cancelar gana.

El incremento sobre `2f6355d` asocia las versiones de proyecto con la
generación y el ID activo: al cambiar de proyecto o desmontar el controlador,
las respuestas obsoletas se descartan y no se muestran versiones del proyecto
anterior. La retención y la consulta limitan el historial a cinco versiones; la lista solo lee y deserializa las cinco más recientes.

El incremento sobre `cdcc2bd` añade cancelación `deliveryPresetCatalog`
entre filas SQLite y deserializaciones del catálogo local de presets de
entrega (máximo 100). El panel ofrece «Cancelar carga» y reintento; SQLite y la
deserialización de una fila siguen siendo síncronas.

El incremento sobre `628c27c` separa el anuncio de progreso del panel visual:
una región viva breve anuncia cambios de etapa y cancelación, mientras la barra
nativa conserva un nombre y valor accesibles sin anunciar cada cambio de
porcentaje ni el reloj.

El incremento sobre `239df30` aplica el límite de retención también al
leer versiones: `list_project_versions` usa SQL `LIMIT` y solo deserializa las
cinco más recientes, aunque el catálogo contenga registros sobrantes.

El incremento sobre `ffe6d12` añade `projectVersions` para cancelar la
carga del historial desde Proyectos y reintentarla. La inicialización y la
consulta limitada comprueban el token; la recuperación recorre el catálogo de
referencias y las generaciones entre filas/entradas. Si no se pueden leer las
referencias SQLite, se omite la limpieza en vez de asumir que no hay proyectos
activos. La reconciliación consulta cancelación al enumerar y validar entradas,
y antes/después de `remove_dir_all`, que preserva el borrado recursivo seguro
del sistema operativo. La llamada recursiva, `migrate`, cada lectura SQLite y
las syscalls individuales del sistema de archivos siguen siendo síncronas. El
borrado explícito continúa bajo el gate si gana el commit del catálogo.

## Cola consolidada el 2026-09-25

La cola se redujo a lo que aporta evidencia que todavía no existe. Las filas
anteriores de RV01–RV06, RV08, RV14, RV15 y RV16, con su evidencia, siguen en
[`historial-verificacion.md`](historial-verificacion.md) y la decisión en
[`ROADMAP.md`](../../ROADMAP.md#consolidación-de-la-cola-2026-09-25).

| Orden | ID | Criterio de cierre | Dependencia |
| --- | --- | --- | --- |
| 1 | RV07 | Tres participantes, dos casos reales por sesión, al menos tres datasets, 24/30 tareas sin ayuda, guardar/reabrir y entrega verificados, sin P0/P1; resumen sanitizado validado con `npm run beta:check-summary`. Incluye la aceptación con datos reales de carga, importación, cancelación y tareas reutilizables; cada fallo de datos se reduce a fixture y regresión. | Participantes y datos de trabajo |
| 2 | RV09 | Cargar→Entregar en Windows con teclado y NVDA, incluidos modales, tablas, progreso, zoom y alto contraste. | Mismo candidato de RV07 |
| 3 | RV11 | Solo si se distribuyen binarios: aprobación jurídica, VM limpia, instalación, reapertura, updater, hashes, firmas y sección de versión del CHANGELOG. | Decisión de distribución |

Cerradas: RV03, RV12, RV13 y RV10. RV10 se cerró el 2026-09-25 contra SQL
Server 2022 local (ODBC Driver 18, autenticación de Windows, `tempdb`): el
round-trip conserva `true`/`false`/`null`, Unicode y saltos de línea desde frame
y desde fuente incremental.

## Tier 10 — reauditoría del 2026-09-23

Abierto el 2026-09-23 con 33 tareas (2 de severidad alta, 22 media y 9 baja).
El detalle ejecutable vive en
[`ROADMAP.md`](../../ROADMAP.md#tier-10--reauditoría-2026-09-23-integridad-de-gates-frontera-ipc-y-accesibilidad-real-abierto-2026-09-23);
aquí solo se listan las que cambian el orden de trabajo.

| ID | Severidad | Motivo para priorizarla |
| --- | --- | --- |
| T10-01 | Alta | El ejecutable release abre una consola; bloquea cualquier binario de RV11. |
| T10-02 | Alta | La acción principal de Preparar tiene contraste 1,68:1 en temas oscuros; afecta a RV06/RV09. |
| T10-03, T10-04 | Media | El SQL local puede salir del subconjunto validado hacia DuckDB con acceso externo por defecto. |
| T10-07, T10-08 | Media | Los gates de red y documentación aprueban sin comprobar lo que declaran. |

El Tier 10 quedó cerrado el 2026-09-25 (33 de 33).

## Fuera de la cola vigente

Lotes gráficos (antes RV15), validación en una herramienta BI concreta (antes
RV14), diccionario de negocio, catálogos de equivalencias, reanudación avanzada
de lotes, vigilancia de carpetas, macOS/Linux y nuevos conectores solo se evaluarán
si la beta aporta casos repetidos, responsables y criterios de aceptación. No se
añaden por anticipación.
