# Trabajo vigente

Revisado: 2026-09-23. Esta es la única cola operativa derivada de la
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

## Ahora — completar el flujo automático

| ID | Resultado y criterio de cierre | Responsable | Dependencia | Estado / evidencia |
| --- | --- | --- | --- | --- |
| RV01 | **Flujo contextual y estado central.** Una sola acción primaria por estado; visitar una fase no la completa; carga, perfil, plan, aplicación, validación y entrega rechazan resultados obsoletos o dobles ejecuciones. | Producto + frontend | Ninguna | **Parcial — coordinación y cancelación local implementadas; aceptación nativa pendiente.** «Hecho» de Cargar y Revisar queda ligado a la revisión activa y cada cambio invalida Entregar (regresiones App 1/1); Preparar comparte un token cancelable y publica bajo bloqueo. Los smokes nativos, el CDP completo y el benchmark de 100 MiB pasan. Falta el recorrido Cargar→Entregar con tareas de trabajo reales. [Evidencia completa](historial-verificacion.md#rv01). |
| RV02 | **Importación unificada y explicable.** Hoja, encabezados, esquema, ambigüedades y recursos se aceptan una vez; CSV permite revisar decisiones de encabezado antes de activar el dataset. | Producto + motor de importación | RV01 | **Parcial — esquema previo local implementado; aceptación con datos reales pendiente.** Todos los formatos muestran recursos, perfil y diferencias de esquema antes de reemplazar el dataset activo (`dataset/import_schema_preview.rs`); la confirmación final vuelve a leer el archivo. En ese corte pasaron build, `cargo check` e `ipc:check`, sin repetir pruebas de producto. [Evidencia completa](historial-verificacion.md#rv02). |
| RV04 | **Excepciones, cancelación y recuperación.** Fechas, tipos, conflictos y esquema cambiado ofrecen conservar, resolver o excluir; cancelar nunca presenta una versión parcial como terminada y los fallos de disco conservan la última revisión válida. | Motor + frontend | RV01–RV03 | **Parcial — las rutas principales por filas de Revisar, Preparar y Entregar observan cancelación con publicación protegida.** Lectores por lotes, DuckDB interrumpible y gates de commit/cancelación en Review, proyectos, historial y exportación, con regresiones. Quedan tramos síncronos de APIs del sistema (`worksheet_range` de XLS/ODS, apertura de libros, driver ODBC, `sync_all`, `remove_dir_all`), medir el JOIN por bloques y la aceptación nativa con datos reales. [Evidencia completa](historial-verificacion.md#rv04). |
| RV05 | **Tarea reutilizable.** Guardar importación, receta, reglas y política de salida sin credenciales ni permiso implícito de sobrescritura; otro archivo compatible recorre el flujo sin reconstruir formularios y un esquema distinto exige revisión. | Proyectos + automatización | RV02–RV04 | **Parcial — la configuración no mutadora se aplica al importar con perfil y esquema exactos.** Reglas, formato, privacidad y receta se restauran como borrador; la receta exige aplicación explícita y un esquema distinto pide revisión. No se guardan credenciales. Pasan el panel (7/7), Playwright, `smoke:restart` y `smoke:native-selectors` con tareas sintéticas. Falta la aceptación con tareas y archivos de trabajo reales. [Evidencia completa](historial-verificacion.md#rv05). |
| RV06 | **Interfaz compacta y accesible.** Retirar jerga y bloques repetidos; mantener historial/deshacer cerca del resultado; foco estable, teclado, errores asociados y zoom 200 % verificados en el flujo automático. | Diseño + accesibilidad | RV01–RV05 | **Parcial — jerarquía, teclado y anuncios de progreso acotados; aceptación con lector de pantalla pendiente.** Modales con `dialog` y `showModal()`, trap que omite `details` cerrados y foco devuelto al disparador (T10-10); campos ODBC y contrato de calidad con nombres y errores asociados; contraste AA por tema (T10-02, T10-19). Los E2E cubren 200 %, colores forzados y 320 CSS px. Falta la aceptación nativa con lector de pantalla. [Evidencia completa](historial-verificacion.md#rv06). |

## Después — demostrar valor y soporte

| ID | Resultado y criterio de cierre | Responsable | Dependencia | Estado |
| --- | --- | --- | --- | --- |
| RV07 | **Beta con tareas reales.** Tres participantes distintos completan dos casos reales por sesión sobre el mismo candidato, con al menos tres datasets en total; consiguen 24/30 tareas sin ayuda, completan el flujo, guardan/reabren y verifican la entrega en cada sesión, sin P0/P1 abierto; se publica un resumen sanitizado y los reportes detallados quedan bajo `.local/beta/`. | Producto | Candidato con RV01–RV06 y gate Full | **Abierto — requiere participantes y datos de trabajo.** El checker local `npm run beta:check-summary` ya valida consistencia, privacidad y el reporte Full; faltan las tres sesiones humanas. |
| RV08 | **Regresiones derivadas de beta.** Cada fallo dependiente de datos se reduce a una fixture sintética, se reproduce antes de corregirse y obtiene una regresión pertinente. | Mantenimiento | RV07 | **Abierto — depende de hallazgos de RV07.** |
| RV09 | **Aceptación nativa de accesibilidad.** Recorrido Cargar→Entregar en Windows con teclado y lector de pantalla real, incluidos modales, tablas, progreso, zoom y alto contraste. | QA accesibilidad | Mismo candidato de RV07 | **Abierto — requiere verificación nativa.** |
| RV10 | **Aceptación SQL Server.** Exportar y releer `true`/`false`/`null` desde frame y fuente incremental conserva tipos y valores; registrar driver y configuración sin credenciales. | QA ODBC | Instancia SQL Server accesible | **Abierto — drivers ODBC 17/18 y `sqlcmd` instalados; falta una instancia conectable.** `MSSQLSERVER` existe pero sigue detenido; Windows rechaza `Start-Service` con `Cannot open 'MSSQLSERVER' service on computer '.'`. El puerto 1433 no responde, no hay comandos Docker/Podman disponibles y `(localdb)\MSSQLLocalDB` tampoco está disponible. No se creó ni modificó una base; el round-trip aún no se ejecutó. |
| RV11 | **Candidato instalable y distribución.** Resolver notices y decisión por canal; probar en VM limpia instalación, reapertura, fallos/firmas del updater y recuperación sobre artefactos ligados al commit; volver a descargar y verificar hashes y firmas publicados. | Release + responsable de distribución | RV07, RV09 y decisiones externas | **Parcial — fuente únicamente aprobada.** GitHub permite publicar código fuente y la revisión técnica de notices está aprobada. `npm run release:dry-run` valida los gates locales y se detiene en el sign-off jurídico obligatorio antes de empaquetar; el reporte queda en `.local/validation/20260921T232220Z-a8fb7b4-package.json`. Eso no autoriza instaladores/updater ni comercialización; faltan aprobación jurídica, candidato/canal binario autorizado, VM limpia y verificación de assets descargados. ONAPI sigue siendo requisito previo a comercializar. |

## Siguiente valor — priorizar con evidencia de beta

| ID | Resultado y criterio de cierre | Responsable | Dependencia | Estado / evidencia |
| --- | --- | --- | --- | --- |
| RV12 | **Recursos y escala medibles.** Una matriz varía ancho, cardinalidad, texto y tamaño; mide RAM, disco, tiempo, cancelación y limpieza. Los avisos solo aparecen cuando cambian una decisión y no confunden estimación con reserva. | Rendimiento | Evidencia de RV07 | **Completada para la matriz sintética v1 y ampliada con smoke nativo.** `perf:matrix:summary` valida ocho cruces (1 y 100 MiB × cuatro formas) con transformaciones, RAM/disco, cancelación y limpieza. El benchmark WebView2 de 100 MiB (819.137 filas) carga en 2,74 s, pagina en 38 ms y queda dentro de su presupuesto. Las formas observadas en beta pueden añadirse como corridas nuevas. [Evidencia completa](historial-verificacion.md#rv12). |
| RV14 | **Preflight y presets de entrega.** Tipos, nulabilidad, longitud y política remota se explican antes de escribir; los presets locales se releen y se verifican en la herramienta BI elegida sin introducir conectores nuevos. | Entrega | Recorridos confirmados en RV07 | **Implementación local lista; aceptación externa pendiente.** El preflight se repite en backend y bloquea antes de DDL enteros fuera de `i64` y decimales no representables como `f64` finitos; el binding devuelve error en vez de fabricar `NULL`. Los nulos reales conservan binding tipado, los presets no guardan credenciales y `replace` se rebaja a `create_only` al reabrir. Falta elegir herramienta BI de beta y comprobar allí formatos y presets. |
| RV15 | **Lotes gráficos.** Solo si RV07 confirma repetición frecuente: reutilizar el contrato batch con preflight conjunto, progreso por trabajo, resultados parciales honestos y ninguna sustitución implícita. | Automatización | RV05 y demanda observada | **Condicional — no iniciar hasta observar demanda frecuente en beta.** |
| RV16 | **Modularización gradual del motor.** Extraer una responsabilidad de `dataset.rs` por cambio con contratos estables, paridad conductual y sin reescritura general. | Mantenimiento | Al tocar el área por RV02–RV05 o RV12 | **En curso — treinta y ocho módulos con responsabilidades extraídas de `dataset.rs`.** Validación, importación, paginación, perfiles, comparación, historial, recetas, consultas, exportación atómica, automatización CLI y coordinación de cancelación viven en módulos internos; los comandos conservan nombres y parámetros Tauri. El trabajo continúa de forma gradual. [Evidencia completa](historial-verificacion.md#rv16). |

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

## Fuera de la cola vigente

Diccionario de negocio, catálogos de equivalencias, reanudación avanzada de
lotes, vigilancia de carpetas, macOS/Linux y nuevos conectores solo se evaluarán
si la beta aporta casos repetidos, responsables y criterios de aceptación. No se
añaden por anticipación.
