# Trabajo vigente

Revisado: 2026-09-21. Esta es la única cola operativa derivada de la
[`auditoría consolidada`](../../AUDITORIA.md). El historial y las tareas cerradas
permanecen en [`ROADMAP.md`](../../ROADMAP.md#tier-9--valor-operativo-consolidado-abierto-2026-09-14).

Una tarea entra aquí solo si reduce fricción del flujo principal, permite repetir
trabajo, protege resultados o aporta evidencia necesaria para declarar soporte.
Las propuestas condicionadas a demanda se enumeran al final y no son trabajo
comprometido.

Avance RV16 del 2026-09-21: el motor ya separa catorce áreas: validación de
workspace/proyecto (`dataset/project_validation.rs`), perfiles y excepciones de
importación (`dataset/import_profile_validation.rs`), comparación del historial
(`dataset/snapshot_comparison.rs`), gramática y planificación de consultas
(`dataset/local_query.rs`), estadísticas y correlaciones numéricas
(`dataset/numeric_profile.rs`), contratos de reglas de calidad
(`dataset/quality_contracts.rs`), migración, validación y persistencia de
`dataset/quality_documents.rs`, evaluación eager/source-backed
(`dataset/quality_evaluation.rs`), validación y ejecución lazy de recetas
(`dataset/recipe_engine.rs`), planificación y ejecución source-backed de recetas
(`dataset/recipe_source_projection.rs`), protección de fórmulas en exportaciones CSV/Bundle con cancelación cooperativa (`dataset/csv_formula_safety.rs`), revisión de encabezados delimitados y sus contratos (`dataset/delimited_header_import.rs`), resúmenes categóricos
(`dataset/categorical_profile.rs`) y tendencias temporales
(`dataset/temporal_profile.rs`). El motor de recetas conserva el fallback eager
para operaciones fuera del plan lazy; la validación de renombrados también queda
disponible para el plan source-backed. Se preservan rutas públicas, comandos,
privacidad externa, JSON, cancelación, enforcement previo a exportación y límites
de muestreo. Tras esta extracción pasan `cargo fmt --all -- --check`,
`cargo check --tests` y `cargo test --lib` con el harness Windows Common Controls v6: 494 aprobadas, 0 fallidas y 5
ignoradas (dos benchmarks opt-in y tres integraciones ODBC externas). La matriz
incremental anterior pasó 16/16. RV16 sigue abierta.

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

La verificación frontend actual pasa `449/449` pruebas en 51 archivos con
`npx vitest run --maxWorkers=1`; `npm run test:coverage` aprueba la cobertura
global y las cinco capas críticas (86,51 % de sentencias y 81,49 % de ramas
globales). `npm run test:e2e` pasa `22/22` y ejecuta el build de producción.
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
Polars. Pasan `cargo fmt --check`, `cargo check --lib`, `npm run build`,
`npm run ipc:check`, el checker documental y `git diff --check`; no se ejecutaron
pruebas de producto.

El incremento sobre `0f4be2f` ordena inicio, cancelación y publicación de
la carga con `load_commit_lock`. La selección inicial y las hojas inspeccionadas
se publican solo si su generación sigue vigente. La carga prepara el historial
fuera del gate y, antes de publicar, valida el token y el ID pendiente; dataset,
comparación y selección se sustituyen juntos. Si cancelar gana durante la
creación síncrona del historial, el candidato se descarta y la sesión anterior
permanece activa. Pasan `cargo fmt --check`, `cargo check --lib`,
`npm run build`, `npm run ipc:check` y `git diff --check`; no se ejecutaron
pruebas de producto.

El incremento sobre `10f8d43` mueve `test_database_connection` a
`spawn_blocking` y lo asocia a `databaseConnection`, separado del token de
preflight para que cancelar una prueba no invalide otro análisis remoto.
Comprueba el token antes y
después de inicializar ODBC, conectar y ejecutar `SELECT 1`; las llamadas del
driver siguen siendo síncronas y el resultado se descarta al retornar si se
canceló. El comando independiente no tiene hoy un consumidor en la interfaz.
Pasan `cargo fmt --check`, `cargo check --lib`, `npm run build`,
`npm run ipc:check` y `git diff --check`; no se ejecutaron pruebas de producto.

El incremento sobre `c6d7bda` asocia la comprobación de red del updater
con `updateCheck`. El panel permite cancelarla, `tokio::select!` descarta la
consulta pendiente y un gate común decide entre cancelar y publicar el
resultado. Pasan `cargo fmt --check`, `cargo check --lib`, `npm run build`,
`npm run ipc:check`, el checker documental y `git diff --check`; no se
ejecutaron pruebas de producto.

El incremento sobre `be2a035` conecta las exportaciones source-backed Parquet y
JSON con el monitor de cancelación de DuckDB durante `COPY`. La operación escribe
en staging, copia al archivo temporal final con comprobaciones cada 64 KiB y no
publica el destino si se interrumpe. `sync_all` sigue síncrono.
Pasan `cargo fmt --check`, `cargo check --lib` (58 avisos
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
| RV01 | **Flujo contextual y estado central.** Una sola acción primaria por estado; visitar una fase no la completa; carga, perfil, plan, aplicación, validación y entrega rechazan resultados obsoletos o dobles ejecuciones. | Producto + frontend | Ninguna | **Parcial — coordinación y cancelación local implementadas; aceptación nativa pendiente.** `usePrepareController` y App rechazan clics repetidos y respuestas obsoletas; cada cambio de revisión invalida el éxito anterior de Entregar, cubierto por una regresión App de exportación→consolidación (1/1); las mutaciones, recetas, deshacer y rehacer de Preparar comparten un token cancelable. Los snapshots candidatos se preparan antes de la publicación breve bajo bloqueo; la carrera de commit/cancelación decide si se publica el resultado o se conserva el estado previo. Los smokes nativos de Playwright/WebView2 verifican selección, mutaciones, exportación e IPC; el smoke CDP completo pasa Playwright, foco, ProjectsPanel y operaciones de proyecto, y el benchmark nativo cubre carga/paginación/transformación/exportación de 100 MiB. Falta el recorrido Cargar→Entregar con tareas de trabajo reales. |
| RV02 | **Importación unificada y explicable.** Hoja, encabezados, esquema, ambigüedades y recursos se aceptan una vez; CSV permite revisar decisiones de encabezado antes de activar el dataset. | Producto + motor de importación | RV01 | **Parcial — las convenciones CSV/TSV se ejecutan de forma explícita y segura.** El disclosure plegado permite elegir interpretación de fecha y número; una columna se convierte solo si todos sus valores no nulos cumplen la convención, y cualquier valor inválido conserva la columna completa como texto. Sin definir y los archivos source-backed conservan valores léxicos. Los E2E recorren CSV, XLSX y Parquet con bridge simulado; este ahora refleja `inspect_workbook_sheets` como llamada independiente para Excel y la suite Playwright actual pasa 22/22 casos, incluido guardar, revisar y aplicar una tarea reutilizable. El smoke nativo enumera las hojas Excel antes de reabrir la selección y genera/vuelve a cargar por diálogos de Windows archivos CSV/XLSX/Parquet, verificando bytes, filas, esquema y valores; otro smoke carga CSV desde un archivo y pagina, transforma y previsualiza su exportación. La aceptación con datasets de trabajo reales sigue pendiente. |
| RV04 | **Excepciones, cancelación y recuperación.** Fechas, tipos, conflictos y esquema cambiado ofrecen conservar, resolver o excluir; cancelar nunca presenta una versión parcial como terminada y los fallos de disco conservan la última revisión válida. | Motor + frontend | RV01–RV03 | **Parcial — las rutas principales por filas de Revisar/Preparar/Entregar observan cancelación con publicación protegida; quedan la aceptación con datos reales/nativa y límites síncronos de APIs del sistema.** Los mutadores de Preparar, recetas, historial y operaciones source-backed de DuckDB responden a la cancelación. Una tarea guarda decisiones versionadas por columna, ligadas a receta y esquema exacto, y permite aplicarlas al reutilizarla: `review` conserva el error estricto, `nullify` convierte solo valores inválidos en nulos y `excludeRow` excluye filas inválidas y cuenta el total; los nulos reales se preservan. La validación previa exige el esquema exacto y la receta vinculada; las fuentes source-backed se materializan como candidato con el límite existente y solo se publican tras completar la operación, con undo disponible. Cambios de esquema invalidan las decisiones. Una receta ambigua con conversiones repetidas omite la política derivada para no impedir guardar la tarea. JOIN, Consolidar y Resolver conflictos en Review comparten exclusión mutua y el token `reviewMutation`; la resolución por origen/celda prepara y publica un candidato privado eager/source-backed, con historial y descarte de comparación dentro del gate atómico. La nueva acción de exclusión quita la fila activa para la clave elegida sin incorporar la versión comparada. Ambas rutas conservan orden y tipos; la UI separa exclusión por clave de las decisiones por celda. La cancelación conserva comparación, historial y archivos previos; hay regresiones para cancelación en el gate de publicación eager y source-backed, además de staging, cancelación antes de COPY y exploración interrumpible por bloques de disco. Los lectores eager CSV/TSV/TXT y Parquet usan Polars Streaming con lotes configurados en 8.192 filas y observan cancelación entre lotes; JSON y JSONL la comprueban entre registros. XLSX/XLSB se recorre por celdas en dos pasadas y se acumula por bloques; XLS/ODS conserva un Range completo de Calamine, pero la conversión posterior a DataFrame/snapshot consulta cancelación entre filas, columnas y bloques. El parser de `worksheet_range` sigue siendo síncrono. El JOIN eager de Review procesa bloques de filas activas con cancelación entre bloques y vuelve a unir cada uno contra el dataset comparado, con un coste adicional por medir sobre datasets de trabajo. El selector nativo sigue siendo modal. Los fallbacks eager source-backed de JOIN, resolución/consolidación de Review, mutaciones y recetas de Preparar, y exportación local/ODBC ahora propagan la cancelación durante la materialización: CSV/TSV/TXT y Parquet entre lotes; JSON/JSONL/NDJSON entre registros. El dataset solo deja su representación diferida cuando la lectura termina. El hash SHA-256 del CSV temporal de Bundle comprueba cancelación entre lecturas de 64 KiB en las rutas eager y source-backed; cada lectura individual sigue siendo síncrona. La protección eager de privacidad también consulta el token entre filas; los escritores eager CSV, JSON y Parquet procesan bloques de 8.192 filas. CSV neutraliza fórmulas por bloque y JSON conserva un único arreglo. La comparación de archivos en Review ahora comparte el token `datasetComparison`: el panel ofrece cancelar, las copias Parquet, conversiones DuckDB y lecturas eager comprueban el token, y el cálculo Parquet interrumpe entre bloques y registros derramados. El resultado anterior se conserva si la cancelación gana el gate de publicación. El selector nativo sigue modal; la apertura y `worksheet_range` de XLS/ODS son monolíticas, aunque el análisis y conversión posterior consultan el token; el cierre final y sync_all de los escritores Parquet siguen siendo tramos síncronos. La paginación de conflictos eager y source-backed usa ahora el mismo token; Review ofrece cancelar la carga y conserva la página previa si la cancelación gana. La vista principal paginada también usa el token `datasetPage`: Polars Streaming cancela entre lotes, el fallback eager y el armado de filas comprueban la generación, y Review conserva visible la página anterior hasta completar o cancelar. Los tramos de `open_workbook_auto`/`worksheet_range` solo detectan la cancelación al regresar; el análisis y la conversión del rango consultan el token entre filas, celdas y bloques. El perfil de calidad cancelado conserva un estado visible en Revisar/Preparar y aclara que se descartó el resultado parcial; la acción principal ofrece reintentarlo sin reinicios automáticos. El gate local de reglas de calidad en Entregar usa el token qualityValidation y ofrece «Cancelar validación»; interrumpe la materialización DuckDB y revisa el token entre bloques Parquet y filas. Las operaciones vectorizadas solo pueden comprobar la cancelación al regresar. El preflight ODBC usa databasePreflight para cancelar materialización, protección de privacidad y análisis de filas; las llamadas síncronas al driver solo descartan el resultado al volver. La enumeración de hojas de Excel se ejecuta tras devolver la selección a Cargar; la interfaz permite cancelarla con la generación `load` y descarta la selección pendiente. `open_workbook_auto` y `sheet_names` siguen siendo síncronos, así que el token se observa al terminar la lectura. La auditoría de IPC no encontró transformaciones de filas sin token: los handlers restantes sin token se limitan a lectura/ajuste de recursos del sistema, diálogos y acciones nativas, el paso final del updater, o CRUD de contratos/catálogos acotados (recetas y calidad hasta 1 MiB, cada tarea hasta 2 MiB (el catálogo es cancelable) y el catálogo de presets hasta 100). Las llamadas síncronas de Calamine, SQLite, ODBC, escritores y limpieza de archivos solo observan cancelación cuando regresan. El corte anterior obtuvo 426/426 pruebas frontend, 21 E2E y 490 pruebas Rust ejecutables (5 ignoradas); esas cifras son previas a este cambio. La revisión anterior añadió cuatro regresiones Rust y `cargo check --tests` confirma que compilan, pero no se han ejecutado: `cargo test` termina antes del harness con `STATUS_ENTRYPOINT_NOT_FOUND` (`0xc0000139`), incluso con un manifiesto Common Controls v6 temporal. Los smokes nativos verifican flujos de archivo, no cancelación durante cada operación. La inspección de hojas se separa de la selección nativa y ofrece cancelar la lectura con la generación `load`; la llamada síncrona de calamine detecta cancelación al regresar. La vista previa `preview_delimited_header_review` también comparte `load`, comprueba el token entre su lectura acotada y sus dos parseos síncronos, y «Cancelar» invalida el trabajo y descarta la selección. `cargo fmt --check`, `cargo check --lib`, `npm run build`, `npm run ipc:check`, el checker documental (4 pruebas internas aprobadas) y `git diff --check` pasan; no se ejecutaron pruebas de producto. Abrir un proyecto y restaurar una versión usan `projectOpen` para cancelar la validación del snapshot y la copia del historial. Restaurar valida el candidato antes del cambio persistente; el commit del catálogo y la activación comparten el gate. Si cancelar gana antes del commit, se conservan el catálogo y el dataset activo; si el commit gana, ambos se publican juntos. La interfaz ofrece «Cancelar apertura» y «Cancelar restauración». |
| RV05 | **Tarea reutilizable.** Guardar importación, receta, reglas y política de salida sin credenciales ni permiso implícito de sobrescritura; otro archivo compatible recorre el flujo sin reconstruir formularios y un esquema distinto exige revisión. | Proyectos + automatización | RV02–RV04 | **Parcial — la configuración no mutadora se aplica automáticamente al importar con el perfil y esquema exactos.** Al preparar una tarea y cargar un archivo compatible, se restauran reglas, formato, privacidad y receta como borrador, sin volver a pedir confirmación. La receta no se ejecuta hasta que la persona la aplica; el esquema distinto o una importación con valores predeterminados conservan la revisión explícita. No se guardan credenciales ni permiso de sobrescritura. Al guardar una tarea con el dataset activo, el panel comprueba la compatibilidad del esquema y habilita aplicarla cuando el backend la declara lista. La prueba del panel pasa 7/7 y Playwright cubre guardado, revisión y aplicación; el smoke CDP nativo confirma receta, exportación, reapertura y restauración de fase con cleanup. El nuevo `npm run smoke:restart` confirma en dos sesiones reales que una tarea sintética se guarda, reaparece en el catálogo y puede abrirse tras reiniciar la app; valida su contenido y confirma la limpieza del registro (0→1→0). El nuevo `npm run smoke:native-selectors` recorre el panel con una tarea sintética y selectores Win32: el CSV con la columna `extra` exige confirmación y muestra la diferencia; el CSV compatible aplica la tarea y deja la receta en Preparar sin ejecutarla. La tarea se elimina al terminar. Evidencia: `.local/validation/webview2-cdp/20260922T022017Z/native-selectors.stdout.log` y `.local/validation/webview2-cdp/20260922T022017Z/summary.json`. La memoria privada del runtime debug excedió el presupuesto diagnóstico no aplicado (441.495.552 frente a 268.435.456 bytes). RV05 sigue parcial hasta la aceptación con tareas y archivos de trabajo reales. |
| RV06 | **Interfaz compacta y accesible.** Retirar jerga y bloques repetidos; mantener historial/deshacer cerca del resultado; foco estable, teclado, errores asociados y zoom 200 % verificados en el flujo automático. | Diseño + accesibilidad | RV01–RV05 | **Parcial — jerarquía, teclado y anuncios de progreso acotados; aceptación con lector de pantalla pendiente.** La comparación de revisiones queda plegada y el historial/deshacer permanecen visibles. El trap de foco ahora omite controles dentro de `details` cerrados e incluye su resumen, para que Tab no llegue a un control invisible; hay una regresión que recorre el diálogo de importación hasta «Cargar archivo». Los modales propios usan ahora el elemento HTML `dialog` con `showModal()`: el navegador aplica la modalidad nativa e inhabilita la interacción con el fondo, mientras Columnia conserva el trap de teclado y restaura el foco al cerrar. El fallo de la vista previa CSV/TSV se anuncia dentro del diálogo de revisión; se quitó la alerta global redundante que quedaba fuera de contexto. En Entregar, los campos de esquema y tabla ODBC toman el nombre de su etiqueta visible, y los errores de validación se enlazan con el control correspondiente mediante `aria-describedby`. La alerta de validación del contrato de calidad se enlaza con el fieldset de la regla afectada cuando el error identifica una regla. La auditoría de Entregar alineó los nombres accesibles de límites, referencias, orden y presets con sus etiquetas visibles. El E2E comprueba jerarquía a 200 %, colores forzados y avance con teclado a 320 CSS px; la suite actual pasa 22/22 E2E y `DeliveryPhase.test.tsx` pasa 32/32. La suite frontend actual pasa 449/449 en 51 archivos; la cobertura crítica por capa también aprueba `npm run test:coverage`. Falta aceptación nativa con lector de pantalla. |

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
| RV12 | **Recursos y escala medibles.** Una matriz varía ancho, cardinalidad, texto y tamaño; mide RAM, disco, tiempo, cancelación y limpieza. Los avisos solo aparecen cuando cambian una decisión y no confunden estimación con reserva. | Rendimiento | Evidencia de RV07 | **Completada para la matriz sintética v1 y ampliada con smoke nativo.** `perf:matrix:summary` valida los ocho cruces (1 y 100 MiB × standard, wide, low-cardinality y long-text), cada uno con 3 transformaciones, 2 actualizaciones de proyecto, RAM/disco, cancelación source-backed, salida previa intacta y limpieza confirmada. El benchmark WebView2 de 100 MiB pasa con 819.137 filas, carga en 2,74 s, paginación en 38 ms, transformación en 2,62 s, exportación en 2,78 s y memoria dentro de su presupuesto de benchmark; confirma cleanup. El exportador CSV usa un hilo para mantener el orden con el límite de memoria; las mediciones cubren el motor nativo y excluyen UI/IPC. Las formas observadas en beta pueden añadirse como nuevas corridas. |
| RV14 | **Preflight y presets de entrega.** Tipos, nulabilidad, longitud y política remota se explican antes de escribir; los presets locales se releen y se verifican en la herramienta BI elegida sin introducir conectores nuevos. | Entrega | Recorridos confirmados en RV07 | **Implementación local lista; aceptación externa pendiente.** El preflight se repite en backend y bloquea antes de DDL enteros fuera de `i64` y decimales no representables como `f64` finitos; el binding devuelve error en vez de fabricar `NULL`. Los nulos reales conservan binding tipado, los presets no guardan credenciales y `replace` se rebaja a `create_only` al reabrir. Falta elegir herramienta BI de beta y comprobar allí formatos y presets. |
| RV15 | **Lotes gráficos.** Solo si RV07 confirma repetición frecuente: reutilizar el contrato batch con preflight conjunto, progreso por trabajo, resultados parciales honestos y ninguna sustitución implícita. | Automatización | RV05 y demanda observada | **Condicional — no iniciar hasta observar demanda frecuente en beta.** |
| RV16 | **Modularización gradual del motor.** Extraer una responsabilidad de `dataset.rs` por cambio con contratos estables, paridad conductual y sin reescritura general. | Mantenimiento | Al tocar el área por RV02–RV05 o RV12 | **En curso — catorce módulos extraídos.** `dataset/project_validation.rs` separa workspace/proyecto, `dataset/import_profile_validation.rs` valida perfiles y excepciones, `dataset/snapshot_comparison.rs` compara revisiones, `dataset/local_query.rs` contiene la gramática y planificación de consultas, `dataset/numeric_profile.rs` calcula estadísticas y correlaciones, `dataset/quality_contracts.rs` define contratos de reglas, `dataset/quality_documents.rs` migra, valida y persiste documentos, `dataset/quality_evaluation.rs` evalúa reglas y enforcement de exportación, `dataset/recipe_engine.rs` valida y ejecuta el plan lazy con fallback eager, `dataset/recipe_source_projection.rs` compila y ejecuta la ruta source-backed con cancelación, `dataset/csv_formula_safety.rs` protege exportaciones CSV/Bundle ante fórmulas en texto, `dataset/delimited_header_import.rs` genera las vistas first-row/generated y sus contratos, `dataset/categorical_profile.rs` resume grupos y `dataset/temporal_profile.rs` perfila tendencias y comparte periodos. Se conservan rutas de comandos, API, privacidad, JSON, cancelación, límites de muestreo y contratos. Tras este corte pasan `cargo fmt --all -- --check`, `cargo check --tests` y `cargo test --lib` con el harness Windows Common Controls v6 (494 aprobadas, 0 fallidas y 5 ignoradas). La matriz incremental anterior pasó 16/16. Continúa como trabajo gradual. |

## Fuera de la cola vigente

Diccionario de negocio, catálogos de equivalencias, reanudación avanzada de
lotes, vigilancia de carpetas, macOS/Linux y nuevos conectores solo se evaluarán
si la beta aporta casos repetidos, responsables y criterios de aceptación. No se
añaden por anticipación.
