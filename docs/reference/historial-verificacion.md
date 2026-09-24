# Historial de verificación

Revisado: 2026-09-23. Este documento conserva, sin cambios, la evidencia
detallada que antes vivía en el estado vigente de [`CONTEXTO.md`](../../CONTEXTO.md)
y en las celdas de la [cola vigente](roadmap-current.md). Se trasladó en T10-22
para que esos documentos quepan en una pantalla. Es un registro histórico: las
cifras de cada párrafo corresponden al corte en que se escribió, y el estado
vigente y los criterios de cierre siguen en `CONTEXTO.md`, la cola y `ROADMAP.md`.

## Tabla de traslado

| Origen | Destino en este documento | Resumen que queda en el origen |
| --- | --- | --- |
| `CONTEXTO.md`, «Estado operativo verificado — 2026-09-23» (4.191 palabras) | [Estado operativo hasta el 2026-09-23](#estado-operativo-hasta-el-2026-09-23) | Sección con el mismo título, ≤ 600 palabras, con cifras del último gate Full |
| `roadmap-current.md`, celda «Estado / evidencia» de RV01 (146 palabras) | [RV01](#rv01) | Estado y pendiente en ≤ 80 palabras con enlace aquí |
| `roadmap-current.md`, celda «Estado / evidencia» de RV02 (123 palabras) | [RV02](#rv02) | Estado y pendiente en ≤ 80 palabras con enlace aquí |
| `roadmap-current.md`, celda «Estado / evidencia» de RV04 (1259 palabras) | [RV04](#rv04) | Estado y pendiente en ≤ 80 palabras con enlace aquí |
| `roadmap-current.md`, celda «Estado / evidencia» de RV05 (241 palabras) | [RV05](#rv05) | Estado y pendiente en ≤ 80 palabras con enlace aquí |
| `roadmap-current.md`, celda «Estado / evidencia» de RV06 (265 palabras) | [RV06](#rv06) | Estado y pendiente en ≤ 80 palabras con enlace aquí |
| `roadmap-current.md`, celda «Estado / evidencia» de RV12 (114 palabras) | [RV12](#rv12) | Estado y pendiente en ≤ 80 palabras con enlace aquí |
| `roadmap-current.md`, celda «Estado / evidencia» de RV16 (426 palabras) | [RV16](#rv16) | Estado y pendiente en ≤ 80 palabras con enlace aquí |

Ningún párrafo se reescribió al trasladarlo (solo se ajustó la ruta de dos enlaces
relativos a esta carpeta); los resúmenes del origen solo
condensan lo que aquí consta completo.

## Estado operativo hasta el 2026-09-23

La base del ajuste de versión fue `master` en `5348360`, versión `0.168.0`.
A petición del usuario, el proyecto adoptó `1.25.0` para representar el alcance
acumulado; los cortes posteriores avanzaron los manifiestos sincronizados de npm,
Cargo y Tauri a `1.26.0`. Este corte parte del commit `e705a5a`. La versión
identifica el alcance del producto y no implica que estén cerrados los gates de
beta con datos reales, accesibilidad nativa, SQL Server o distribución binaria.

RV01: los marcadores «Hecho» de Cargar y Revisar llevan el número de revisión
del dataset. Cada mutación reinicia Cargar en la revisión nueva e invalida la
finalización anterior de Revisar, también cuando Preparar publica un cambio; la
regresión App Revisar→Preparar→corrección pasa 1/1. La regresión previa confirma
que exportar y luego consolidar invalida Entregar. La aceptación Cargar→Entregar
con datasets de trabajo reales sigue pendiente.

RV02: todos los formatos muestran recursos y perfil antes de importar. Una
inspección nativa calcula filas, columnas, tipos y diferencias con el perfil sin
activar el candidato; el esquema se revisa dentro del mismo diálogo antes de la
confirmación final. La carga final vuelve a leer el archivo. Excel permite hoja
y encabezados; CSV/TSV añaden muestra, interpretaciones y convenciones. La
aceptación con datasets de trabajo reales sigue pendiente. `npm run build`,
`cargo check --manifest-path src-tauri/Cargo.toml --lib` y `npm run ipc:check`
pasan; no se ejecutaron pruebas de producto en este corte.

RV16: `dataset/file_validation.rs` concentra la validación segura de rutas y
`dataset/page_reader.rs` la paginación en memoria/source-backed. Los puntos
de entrada y las pruebas existentes mantienen sus rutas. `dataset/source_loading.rs`
concentra la carga source-backed, la materialización cancelable, los límites de
recursos y los loaders por formato; `dataset/spreadsheet_io.rs` concentra la
inspección de libros, conversión tipada de rangos y snapshots por bloques;
`dataset/export_io.rs` concentra escritores, privacidad y publicación atómica;
`dataset/recipe_eager.rs` concentra la validación y aplicación eager de recetas;
`dataset/comparison_io.rs` concentra la carga y persistencia temporal de fuentes
comparadas, incluidos sus snapshots y cancelación.
`dataset/query_execution.rs` concentra las consultas locales, los planes
source-backed, la lectura por bloques Parquet, las agregaciones y los JOIN con
cancelación.
`dataset/automation.rs` concentra los helpers de inspección, transformación,
validación y exportación del CLI, incluidos los caminos source-backed.
`dataset/project_history.rs` concentra la captura, restauración y los resúmenes
de snapshots de proyectos.
`dataset/profile_engine.rs` concentra la inferencia de tipos, estadísticas de
texto, perfilado eager/source-backed y agregaciones temporales, conservando
límites, cancelación y contratos.
La
compilación de la librería pasa con
`cargo check --manifest-path src-tauri/Cargo.toml --lib`; el corte actual repite
la suite Rust (494 aprobadas y 5 ignoradas), la suite frontend (457/457) y el
build de producción.

El smoke nativo de RV05 recorre con Playwright el panel de tareas y el selector
Win32: el esquema distinto muestra `extra` y pide confirmación, mientras que un
archivo compatible carga la receta guardada como borrador en Preparar. La tarea
sintética se elimina al terminar. Evidencia UTC: `.local/validation/webview2-cdp/20260922T022017Z/native-selectors.stdout.log` y
`.local/validation/webview2-cdp/20260922T022017Z/summary.json`. El runtime debug
alcanzó 441.495.552 bytes de memoria privada, por encima del presupuesto
diagnóstico no aplicado de 268.435.456 bytes.

RV16 mantiene treinta y ocho módulos con responsabilidades extraídas de
`dataset.rs`. Incluye validación de proyectos y archivos, inspección de fuentes,
previsualización de esquema, carga y descarte, paginación, lectura de perfiles,
historial con deshacer/rehacer, comparación, perfiles numéricos/categóricos/temporales, recetas,
consultas, encabezados y reglas de calidad. `dataset/page_reader.rs` conserva la
ruta Tauri de `get_dataset_page` y centraliza cancelación, snapshots y fallbacks.
`dataset/profile_reader.rs` concentra `get_dataset_profile` y
`get_temporal_aggregation`, incluidos cache, rutas eager/source-backed y
cancelación por generación. `dataset/comparison_reader.rs` reúne la lectura inicial de la comparación y la
paginación de conflictos; ambas conservan cancelación y vigencia del snapshot.
`dataset/recipe_eager.rs` concentra la validación, conversiones y aplicación de
recetas sobre DataFrame eager; conserva excepciones, filtros y resúmenes.
`dataset/comparison_engine.rs` concentra los índices derramados, la comparación
por claves, los conflictos y JOIN con cancelación.
`dataset/query_execution.rs` concentra los planes source-backed, la lectura por
bloques Parquet, las agregaciones, la paginación de consultas y el JOIN local con
cancelación.
`dataset/automation.rs` concentra la carga, inspección, transformación,
validación y exportación para automatización CLI sin cambiar sus contratos.
`dataset/project_history.rs` concentra la captura, restauración y los resúmenes
de snapshots de proyectos.
`dataset/profile_engine.rs` concentra la inferencia de tipos, estadísticas de
texto, perfilado eager/source-backed y agregaciones temporales.
`dataset/history.rs` contiene la navegación cancelable de deshacer/rehacer y restaura snapshots eager/source-backed con lectura Parquet por lotes cancelable; `dataset/snapshot_comparison.rs` carga ambas revisiones con cancelación por lotes y calcula/orquesta `compare_history_snapshots`. `dataset/source_loading.rs` reúne la lectura diferida y la materialización protegida por presupuesto; `dataset/spreadsheet_io.rs` reúne la lectura de libros, conversión tipada y snapshots cancelables por bloques; `dataset/export_io.rs` reúne escritores, privacidad y publicación atómica cancelable; `dataset/comparison_io.rs` reúne la carga y persistencia temporal de fuentes comparadas, incluidos sus snapshots y cancelación. Resolver conflictos y Consolidar interrumpen la lectura eager del snapshot comparado; la consulta local con Polars cancela también esa lectura. Se mantienen los comandos y resultados existentes. En este corte pasan `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check`,
`cargo check --manifest-path src-tauri/Cargo.toml --lib`, `npm run ipc:check`,
`node tools/check-documentation.mjs`, `npm test` (457/457) y la suite Rust
(494/494 ejecutables, 5 ignoradas). RV16 sigue
en curso y la cola operativa vigente está en
[`docs/reference/roadmap-current.md`](roadmap-current.md) y el
historial de decisiones y entregas en [`ROADMAP.md`](../../ROADMAP.md). Este contexto
resume el estado; esas fuentes definen los criterios de cierre.

El gate beta local tiene ahora dos comprobaciones separadas: `beta:check-summary`
valida el resumen Gate 1 contra las tres sesiones y el reporte Full del candidato,
sin esperar a un commit posterior; `beta:check-gate1` conserva la comprobación de
descendencia necesaria para preparar Gate 2. El primer checker también rechaza
alias, correos, rutas y credenciales en el resumen versionado. No se ha inventado
evidencia de participantes: RV07 sigue abierto hasta ejecutar las tres sesiones
con datos de trabajo reales.

Los perfiles `tools/check.ps1` fijan `--maxWorkers=1` para las pruebas frontend y
la cobertura. Esto hace reproducible `verify:tier` en Windows y evita que una
cancelación deje procesos Vitest huérfanos; el comando `npm test` del producto
conserva su configuración normal.

Para RV10, Windows tiene instalados ODBC Driver 17/18 y `sqlcmd`, pero el
servicio local `MSSQLSERVER` está detenido; `Start-Service` falla con
`Cannot open 'MSSQLSERVER' service on computer '.'`. El puerto 1433 no
responde, no hay comandos Docker/Podman disponibles y `(localdb)\MSSQLLocalDB`
no respondió. No se creó ni modificó una base; el round-trip sigue pendiente.

La tarjeta de progreso conserva una barra nativa con nombre y valor accesibles. Una región viva breve anuncia los cambios de etapa y la solicitud de cancelación; el porcentaje y el reloj no vuelven a anunciar toda la tarjeta con cada actualización.

Los diálogos propios usan el elemento HTML `dialog` con `showModal()`. El
navegador los presenta en la capa modal y deja inerte el contenido de fondo;
Columnia conserva el trap de teclado, restaura el foco al cerrar y asocia nombre
y descripción accesibles. La aceptación con lector de pantalla real sigue
pendiente.

El fallo de la vista previa CSV/TSV se presenta dentro del diálogo de revisión;
se quitó la alerta global duplicada que quedaba fuera de contexto.

En Entregar, los campos de destino ODBC toman sus nombres accesibles de las
etiquetas visibles; el esquema conserva su indicación de opcional y los errores
de validación se asocian al campo correspondiente.
La alerta de validación del contrato de calidad se asocia con el grupo de la
regla que identifica. Las etiquetas accesibles de límites, referencias y orden
del contrato incluyen ahora el texto visible completo.

La prueba `DeliveryPhase.test.tsx` pasa 32/32, `ReusableTaskPanel.test.tsx`
pasa 7/7 y Playwright pasa 22/22 E2E. El mock de importación simula la
inspección de hojas Excel como llamada separada `inspect_workbook_sheets`; el
recorrido de tareas reutilizables confirma que guardar comprueba el esquema y
habilita aplicar al dataset activo. El smoke nativo más reciente
(`.local/validation/webview2-cdp/20260921T225717Z`) también pasa: abre y guarda
mediante diálogos de Windows, y verifica round-trips reales de CSV/XLSX/Parquet
con bytes, filas, esquema y valores. El aviso de memoria pertenece al
ejecutable debug y no cierra el gate de memoria del release.
El smoke CDP completo (`.local/validation/webview2-cdp/20260921T230251Z`)
aprueba Playwright, landmarks, foco, ProjectsPanel y 18 operaciones IPC nativas
de proyecto, incluidas receta, exportación, reapertura, restauración de fase y
cleanup.
El smoke `npm run smoke:restart` (`.local/validation/webview2-restart/20260922T015059Z`) confirma que `activePhase=prepare` y una tarea reutilizable sintética sobreviven a dos lanzamientos reales de la app. La tarea pasa de 0 a 1 entrada, se lista y reabre con sus campos intactos tras el reinicio, y luego se elimina (catálogo 1→0). La aceptación con tareas y archivos de trabajo sigue pendiente. El pico de memoria privada debug fue 302.895.104 bytes frente al presupuesto diagnóstico de 268.435.456 bytes, que no es requisito de este smoke.

El benchmark nativo de 100 MiB
(`.local/validation/performance-webview2/20260921T230202Z`) confirma 819.137
filas, carga, paginación, transformación, exportación y memoria dentro de su
presupuesto de benchmark.

Las regresiones de `App` quedaron sincronizadas con la inspección Excel separada,
la carga perezosa de Preparar, el análisis previo a navegar a Entregar y las
etiquetas accesibles actuales. El gate Full del 2026-09-23 pasa 457/457
pruebas en 51 archivos (458 tras T10-10); `npm run test:coverage` aprueba los umbrales por capa
crítica, con 86,17 % de sentencias y 81,34 % de ramas globales. `npm run
test:e2e` pasa 22/22 (23 tras T10-10) y ejecuta el build de producción. Los gates
`beta:workflows:check`, `legal:check` e `ipc:check` también pasan. El límite de
un worker hace reproducible la suite frontend en esta estación y no cambia el
producto.

El `npm run release:dry-run` del mismo corte confirma el árbol limpio y los
gates de toolchains, documentación, IPC, gobernanza y legal técnico, pero se
detiene en el sign-off jurídico obligatorio antes del empaquetado. No se
crearon ni publicaron artefactos; el reporte es
`.local/validation/20260921T232220Z-a8fb7b4-package.json`.

En el código local están implementadas la importación CSV/TSV con convenciones
explícitas de fecha y número, las políticas reutilizables de excepciones de
conversión, y la cancelación compartida de JOIN, consolidación y resolución
manual de conflictos en Review. Además de elegir valores por celda o una fila
completa, se puede excluir explícitamente la fila activa de una clave conflictiva;
no se agrega la versión comparada. La ruta eager y la ruta source-backed DuckDB
conservan orden y tipos, y publican el candidato staged con historial reversible.
La comparación y el historial se conservan si cancelar gana antes del commit;
una regresión integrada verifica también la cancelación source-backed justo en
el gate de publicación. Filas duplicadas se diagnostican aparte. DuckDB puede
interrumpir consultas source-backed activas. Los lectores eager CSV/TSV/TXT y
Parquet recopilan con Polars Streaming en bloques configurados en 8.192 filas y
observan cancelación entre lotes.
JSON/JSONL la comprueba entre registros. XLSX/XLSB lee celdas en dos pasadas y
acumula filas por bloques; XLS/ODS conserva la lectura completa mediante
`worksheet_range`. Para XLS/ODS, el análisis del `Range`, la conversión a
DataFrame y la escritura de snapshots consultan cancelación entre filas,
columnas y bloques después de que Calamine devuelve el rango completo. La
apertura del libro y `worksheet_range` siguen siendo llamadas monolíticas.
El JOIN eager de Review recorre bloques de filas activas y comprueba cancelación
entre bloques, repitiendo el JOIN contra el dataset comparado para cada bloque.
Las materializaciones eager source-backed usadas como fallback por JOIN,
resolución/consolidación de Review, mutaciones/recetas de Preparar y exportaciones
local/ODBC y la comparación de archivos aceptan el token de cancelación.
CSV/TSV/TXT y Parquet interrumpen entre lotes; JSON/JSONL/NDJSON, entre registros;
los índices y la comparación Parquet, entre bloques y registros derramados. DuckDB
interrumpe una conversión source-backed activa. El hash SHA-256 del CSV temporal
de Bundle comprueba cancelación entre lecturas de 64 KiB, tanto en la ruta eager
como source-backed; cada lectura de bloque sigue siendo síncrona. La protección
eager de privacidad y las exportaciones CSV, JSON y Parquet también observan el
token: los escritores procesan bloques de 8.192 filas, CSV neutraliza fórmulas
dentro de cada bloque y JSON conserva un solo arreglo. El resultado de comparación
solo se publica al final, bajo un gate que ordena cancelación y commit.
La reconciliación de generaciones huérfanas consulta cancelación al enumerar y
validar entradas, y antes/después de cada borrado recursivo seguro. `remove_dir_all`
no puede interrumpirse mientras su llamada síncrona está activa; cancelar durante
ese tramo se reconoce al regresar. El borrado explícito de proyecto conserva el
gate: si el commit del catálogo gana, termina su limpieza antes de reconocer una
cancelación tardía.
El selector nativo
es modal y no se puede cerrar desde este control mientras está abierto; XLS/ODS
conserva `worksheet_range` monolítico, pero su conversión posterior comprueba el
token entre filas y celdas. El conteo de snapshots Parquet
usa DuckDB con interrupción; la escritura de snapshots eager produce bloques y
consulta el token entre ellos. El cierre del escritor y `sync_all` siguen siendo
llamadas síncronas. La
paginación de conflictos eager y source-backed comparte el token
`datasetComparison`; Review permite cancelar su carga y conserva la página previa
si la cancelación gana. La paginación principal del dataset también admite
cancelación con `datasetPage`: la lectura Parquet/CSV/TSV/TXT desde snapshots o
fuentes source-backed usa Polars Streaming cancelable; el fallback eager y el
armado de filas comprueban la generación. Review mantiene visible la página previa
hasta completar o cancelar. La conversión de rangos `.xls`/`.ods` ahora consulta
cancelación por lotes después de que Calamine devuelve el rango completo. Otros
comandos todavía tienen rutas sin token y las rutas canceladas descartan el
resultado incompleto. El perfil cancelado conserva un estado visible y confirma
el descarte del resultado parcial; la acción principal permite reintentarlo bajo
demanda. En Entregar, la validación local de
reglas usa ahora el token `qualityValidation` y ofrece «Cancelar validación»; al
cancelarse, conserva el gate previo y descarta el resultado incompleto. La fuente
source-backed puede interrumpirse durante la materialización DuckDB, y la
evaluación revisa el token entre bloques Parquet y filas. Las operaciones
vectorizadas comprueban el token cuando devuelven. El preflight de compatibilidad
ODBC usa `databasePreflight` para cancelar la preparación de snapshots privados y
los recorridos por filas; el exportador eager también puede cancelar el análisis
de columnas. Las llamadas síncronas al driver ODBC solo detectan la cancelación
cuando retornan y descartan el resultado.

La selección de un libro Excel devuelve primero sus metadatos; Cargar obtiene los
nombres de hoja en una segunda llamada y ofrece «Cancelar inspección». Usa la
generación `load` para descartar una selección obsoleta. `open_workbook_auto` y
`sheet_names` siguen siendo síncronos, así que la cancelación se confirma cuando
la llamada de calamine retorna. El selector nativo conserva su comportamiento
modal.

La vista previa de encabezados CSV/TSV también usa la generación `load` de la
selección pendiente. El diálogo cancela el token antes de descartar el archivo;
la muestra acotada a 64 KiB se lee en bloques de 8 KiB y el token se consulta
entre bloques, durante la detección del delimitador y entre los dos parseos de
interpretación. La carga completa CSV/TSV/TXT también usa el detector cancelable
antes de iniciar la lectura Polars. Las llamadas individuales de archivo y
Polars siguen siendo síncronas y observan la cancelación cuando retornan.

El inicio, cancelación y publicación de una carga se ordenan con
`load_commit_lock`. La inspección publica la selección y las hojas solo si su
generación sigue vigente; la carga valida la generación y el ID de selección
después de preparar el historial, y sustituye dataset, comparación y selección
juntos. Si cancelar gana durante la creación síncrona del historial, el
candidato se descarta y permanece activa la sesión anterior. Pasan
`cargo fmt --check`, `cargo check --manifest-path src-tauri/Cargo.toml --lib`, `npm run build`, `npm run ipc:check`
y `git diff --check`; no se ejecutaron pruebas de producto.

La prueba Tauri `test_database_connection` ahora ejecuta inicialización ODBC,
conexión y `SELECT 1` dentro de `spawn_blocking`, bajo la generación
`databaseConnection`. Comprueba cancelación antes y después de cada llamada al
driver; ODBC no interrumpe una llamada activa, pero el resultado se descarta al
retornar si la generación cambió. No hay una acción de interfaz que invoque hoy
este comando independiente; el preflight de Entregar mantiene su propio botón
de cancelación. Pasan `cargo fmt --check`, `cargo check --manifest-path src-tauri/Cargo.toml --lib`,
`npm run build`, `npm run ipc:check` y `git diff --check`; no se ejecutaron
pruebas de producto.

El catálogo local de tareas reutilizables usa `reusableTaskCatalog` y consulta
cancelación al avanzar por las filas SQLite y antes y después de deserializar
cada resumen. El panel ofrece «Cancelar carga» y reintento; la UI descarta
respuestas obsoletas y no publica un catálogo parcial. Una lectura SQLite o la
deserialización de un documento individual siguen siendo síncronas y observan
la cancelación cuando retornan.

La carga de versiones del proyecto lleva una generación asociada al proyecto
activo. Al cambiar de proyecto o desmontar el controlador, las respuestas y
errores anteriores se descartan; mientras las versiones no correspondan al
proyecto activo, la interfaz mantiene el estado de carga. El backend limita la
consulta con SQL `LIMIT` a las cinco versiones más recientes antes de
deserializar payloads, incluso si quedan registros sobrantes en el catálogo.

El catálogo de presets de entrega usa `deliveryPresetCatalog`: el recorrido
SQLite y la deserialización de cada resumen revisan cancelación por fila. El
panel ofrece «Cancelar carga» y reintento; se conservan los presets existentes
y no se publica una lista parcial. Cada lectura SQLite y deserialización
individual siguen siendo síncronas.

Abrir un proyecto y restaurar una versión usan `projectOpen`: consultan la
cancelación durante el hash por bloques, el conteo DuckDB, la lectura Parquet y
la copia/validación del historial. Restaurar prepara y valida el candidato antes
de cambiar el catálogo; el commit del catálogo y la activación del dataset
comparten el gate con cancelar. Si cancelar gana antes del commit, se conservan
el proyecto persistido y el dataset activo; si gana el commit, ambos cambian
juntos. El panel ofrece «Cancelar apertura» y «Cancelar restauración».

Guardar manualmente y el autoguardado usan `projectSave`. La materialización
source-backed, la copia de snapshots e historial y el hash SHA-256 comprueban
cancelación mientras avanzan; las generaciones se preparan en una carpeta
temporal y se eliminan si cancelar gana antes del commit. El gate ordena la
cancelación y la transacción del catálogo: si el commit entra primero, guardar
termina y la cancelación espera; si cancelar entra primero, el catálogo y la
última versión válida permanecen intactos. El panel ofrece «Cancelar guardado»
y «Cancelar autoguardado». El cierre del escritor Parquet, `sync_all` y la
transacción SQLite siguen siendo tramos síncronos, con comprobación del token
después del cierre/sync y un gate antes del commit.

Eliminar un proyecto usa `projectDelete`: consulta el token al reunir y validar
las rutas del proyecto y de sus versiones anteriores. Si cancelar gana antes
del gate, el catálogo y los archivos quedan disponibles. El gate cubre el
DELETE SQLite y la limpieza de generaciones/snapshots, para que una cancelación
tardía espere a que la eliminación termine en vez de informar una cancelación
después del commit. SQLite y `remove_dir_all` siguen siendo llamadas síncronas;
un fallo de limpieza posterior al commit se informa como proyecto eliminado
con archivos pendientes de limpieza. El panel ofrece «Cancelar eliminación».

La carga del catálogo usa una sola invocación `list_projects` que devuelve los
proyectos y el candidato de recuperación dentro de la misma transacción de
lectura SQLite. Ambos observan `projectCatalog`; el token se comprueba entre
filas, al medir bytes de snapshots/historial y antes y después de la consulta
indexada del candidato.
Esa consulta SQLite de una fila sigue siendo síncrona. El panel ofrece
«Cancelar carga» y, si se cancela la carga inicial, permite reintentar.

La carga de versiones usa `projectVersions`; Proyectos permite cancelarla y
reintentar. Tanto la lectura limitada a cinco entradas como la inicialización
del catálogo comprueban el token. La recuperación del disco recorre las
referencias SQLite y las generaciones cooperativamente, y omite la limpieza si
no logra leer el catálogo para evitar borrar generaciones activas. Comprueba
cancelación al enumerar y validar entradas y antes/después del `remove_dir_all`
seguro; ese borrado y las llamadas SQLite individuales son síncronos y no se
interrumpen mientras el sistema operativo los ejecuta. El borrado explícito
mantiene el cleanup bajo el gate después del commit SQLite, por lo que una
cancelación tardía espera a que termine.

Las tareas reutilizables aplican reglas, formato, privacidad y receta como
borrador al importar un archivo con el perfil y esquema guardados; la receta
requiere ejecución explícita. Si el perfil no se usa o el esquema cambia, la
interfaz conserva la revisión de la configuración antes de aplicarla.

En Preparar, el botón para avanzar pasa a secundario mientras haya un plan o una
receta lista para aplicar; cuando no existe una acción local disponible,
continuar mantiene la prioridad. La comparación de revisiones sigue plegada y
el historial/deshacer queda cerca del resultado. El E2E comprueba la jerarquía
visual a 200 % y recorre el avance con teclado a 320 CSS px.

En los diálogos, el trap de teclado filtra controles dentro de elementos
`details` cerrados e incluye el resumen del disclosure. Así, Tab no termina en
un selector oculto al revisar encabezados CSV; la regresión recorre el diálogo
hasta «Cargar archivo».

Verificación frontend en el corte anterior: `npm test` 426/426,
`npm run test:e2e -- --workers=1` 21/21, `npm run build`, `npm run ipc:check`,
`npm run docs:check` y `npm audit --omit=optional` pasan; el audit reporta 0
vulnerabilidades. La suite Rust del corte anterior pasó 490 pruebas (0 fallidas,
5 ignoradas).
En el avance de comparación, `cargo fmt`, `cargo check --manifest-path src-tauri/Cargo.toml --lib`, `npm run build` y
`git diff --check` pasan; no se ejecutaron pruebas. Los tests nuevos del corte
anterior solo compilaron y no se ejecutaron por el fallo del loader de Windows
descrito abajo. `smoke:cdp` pasó en WebView2 y
verificó ProjectsPanel, IPC, transformaciones, exportación y reapertura de
proyecto. `smoke:native-selectors` usó los diálogos reales de Windows para
exportar y volver a cargar CSV, XLSX y Parquet generados por un dataset de prueba
de dos filas; verificó valores, columnas y la hoja XLSX. La importación nativa
adicional cargó un CSV de prueba de dos filas, leyó páginas, transformó y
previsualizó una exportación. Evidencia principal:
`.local/validation/webview2-cdp/20260920T210957Z`, con smoke de proyectos en
`.local/validation/webview2-cdp/20260920T205628Z` y carga CSV en
`.local/validation/webview2-cdp/20260920T205234Z`. En la corrida de selectores
debug, el working set pico fue 558,764,032 bytes y la memoria privada
296,427,520 bytes, por encima de los presupuestos de 512 MiB y 256 MiB. El gate
es informativo en debug; esta corrida no demuestra el presupuesto del binario
release.

Las E2E usan el bridge simulado y los archivos de los smokes son sintéticos.
Estas pruebas ejercitan IPC y bytes reales, pero no sustituyen beta con datos de
trabajo, un recorrido Cargar→Entregar completo ni aceptación nativa con lector de
pantalla.

En la validación de calidad de RV04, `cargo fmt`, `cargo check --manifest-path src-tauri/Cargo.toml --lib`,
`npm run build`, el checker documental y `git diff --check` pasan; no se
ejecutaron pruebas.
En el preflight ODBC de RV04, `cargo fmt --check`, `cargo check --manifest-path src-tauri/Cargo.toml --lib`,
`npm run build`, el checker documental y `git diff --check` pasan; no se
ejecutaron pruebas. El driver ODBC no se interrumpe durante una llamada síncrona;
se descarta el resultado al regresar.
En la comprobación de actualizaciones de RV04, `cargo fmt --check`,
`cargo check --manifest-path src-tauri/Cargo.toml --lib`, `npm run build`, `npm run ipc:check`, el checker documental
y `git diff --check` pasan; no se ejecutaron pruebas. `updateCheck` cancela el
futuro de red y serializa cancelación con la publicación del resultado.
Las exportaciones source-backed Parquet y JSON ahora ejecutan `COPY` con el
monitor de cancelación de DuckDB; la copia al archivo temporal final también
revisa el token cada 64 KiB. Si se interrumpe, no se publica el destino.
`sync_all` sigue síncrono. `cargo fmt --check`, `cargo check --manifest-path src-tauri/Cargo.toml --lib`,
`npm run build`, `npm run ipc:check`, el checker documental y `git diff --check`
pasan; no se ejecutaron pruebas.
En la enumeración de hojas de Excel, `cargo fmt --check`, `cargo check --manifest-path src-tauri/Cargo.toml --lib`,
`npm run build` y `npm run ipc:check` pasan; también pasan el checker documental
y `git diff --check`. No se ejecutaron pruebas. El inventario IPC ahora registra
85 comandos de producción. La lectura de nombres de hoja por calamine sigue
siendo síncrona y solo comprueba cancelación antes y después.
En la paginación principal de Review, `cargo fmt --check`, `cargo check --manifest-path src-tauri/Cargo.toml --lib`,
`npm run build`, `npm run ipc:check`, el checker documental y `git diff --check`
pasan; no se ejecutaron pruebas. La lectura por lotes de snapshots Parquet y
fuentes delimitadas source-backed comprueba cancelación durante Polars Streaming.
En el fallback XLS/ODS de importación y comparación, `cargo fmt --check`,
`cargo check --manifest-path src-tauri/Cargo.toml --lib` y `git diff --check` pasan; no se ejecutaron pruebas. Tras
recibir el rango completo, el análisis, la conversión de columnas y la escritura
de snapshots consultan el token entre filas, celdas y bloques; el parser síncrono
de Calamine aún no se puede interrumpir.
El análisis de calidad ahora conserva un estado visible si se cancela y explica
que el resultado parcial se descartó; la acción principal permite reintentarlo
sin repetir automáticamente el análisis. `npm run build` y `git diff --check`
pasan; no se ejecutaron pruebas.
Nota superada el 2026-09-21: el harness Rust de Windows fallaba con
`STATUS_ENTRYPOINT_NOT_FOUND` (`0xc0000139`). Se resolvió incrustando el
manifiesto de Common Controls v6 cuando `COLUMNIA_TEST_HARNESS_MANIFEST=1`
(`src-tauri/build.rs`); `tools/check.ps1` define esa variable y las regresiones
se ejecutan en el gate Full.

Siguen abiertos los criterios con evidencia que no se puede fabricar localmente:
la beta de tres participantes y su resumen sanitizado; accesibilidad manual con
lector de pantalla/alto contraste; round-trip contra SQL Server real; y un
candidato binario/canal autorizado probado en VM limpia.
RV04 conserva `worksheet_range` síncrono para `.xls`/`.ods` y otros comandos sin
token de cancelación, la apertura del libro durante la inspección de hojas, el
selector nativo modal y tramos síncronos de
conteo/escritura de snapshots. También falta medir el coste del JOIN por bloques
y validar cancelación con datos reales en una sesión nativa. RV14 requiere
seleccionar y validar la herramienta BI a partir de beta.
No sustituir estas evidencias por fixtures o resultados sintéticos.

## Evidencia detallada de la cola vigente

### RV01

**Parcial — coordinación y cancelación local implementadas; aceptación nativa pendiente.** `usePrepareController` y App rechazan clics repetidos y respuestas obsoletas; el estado «Hecho» de Cargar y Revisar queda ligado a la revisión activa: cada mutación marca Cargar e invalida Revisar, cubierto por una regresión App Revisar→Preparar→corrección (1/1). Cada cambio de revisión también invalida el éxito anterior de Entregar, cubierto por la regresión App de exportación→consolidación (1/1); las mutaciones, recetas, deshacer y rehacer de Preparar comparten un token cancelable. Los snapshots candidatos se preparan antes de la publicación breve bajo bloqueo; la carrera de commit/cancelación decide si se publica el resultado o se conserva el estado previo. Los smokes nativos de Playwright/WebView2 verifican selección, mutaciones, exportación e IPC; el smoke CDP completo pasa Playwright, foco, ProjectsPanel y operaciones de proyecto, y el benchmark nativo cubre carga/paginación/transformación/exportación de 100 MiB. Falta el recorrido Cargar→Entregar con tareas de trabajo reales.

### RV02

**Parcial — esquema previo local implementado; aceptación con datos reales pendiente.** Todos los formatos muestran recursos y perfil antes de reemplazar el dataset activo. La inspección nativa (`dataset/import_schema_preview.rs`) calcula filas, columnas y tipos y presenta diferencias con el perfil guardado sin activar el candidato. Excel agrega hoja/encabezados; CSV/TSV agregan muestra, interpretaciones y convenciones. La confirmación final vuelve a leer el archivo; si este cambia y aparece una discrepancia nueva, permanece la vía de recuperación existente. `npm run build`, `cargo check --manifest-path src-tauri/Cargo.toml --lib` y `npm run ipc:check` pasan. El E2E 22/22 y los smokes nativos son evidencia anterior a este cambio; no se repitieron aquí. No se ejecutaron pruebas de producto en este corte. Falta aceptar el flujo con datasets de trabajo reales.

### RV04

**Parcial — las rutas principales por filas de Revisar/Preparar/Entregar observan cancelación con publicación protegida; quedan la aceptación con datos reales/nativa y límites síncronos de APIs del sistema.** Los bloques Parquet de consulta local, conflictos y apertura de proyectos, junto con las vistas previas source-backed de recetas, ahora consultan cancelación durante la lectura por lotes y no exponen un frame parcial. Los mutadores de Preparar, recetas, historial y operaciones source-backed de DuckDB responden a la cancelación. Una tarea guarda decisiones versionadas por columna, ligadas a receta y esquema exacto, y permite aplicarlas al reutilizarla: `review` conserva el error estricto, `nullify` convierte solo valores inválidos en nulos y `excludeRow` excluye filas inválidas y cuenta el total; los nulos reales se preservan. La validación previa exige el esquema exacto y la receta vinculada; las fuentes source-backed se materializan como candidato con el límite existente y solo se publican tras completar la operación, con undo disponible. Cambios de esquema invalidan las decisiones. Una receta ambigua con conversiones repetidas omite la política derivada para no impedir guardar la tarea. JOIN, Consolidar y Resolver conflictos en Review comparten exclusión mutua y el token `reviewMutation`; la resolución por origen/celda prepara y publica un candidato privado eager/source-backed, con historial y descarte de comparación dentro del gate atómico. La nueva acción de exclusión quita la fila activa para la clave elegida sin incorporar la versión comparada. Ambas rutas conservan orden y tipos; la UI separa exclusión por clave de las decisiones por celda. La cancelación conserva comparación, historial y archivos previos; hay regresiones para cancelación en el gate de publicación eager y source-backed, además de staging, cancelación antes de COPY y exploración interrumpible por bloques de disco. Los lectores eager CSV/TSV/TXT y Parquet usan Polars Streaming con lotes configurados en 8.192 filas y observan cancelación entre lotes; JSON y JSONL la comprueban entre registros. XLSX/XLSB se recorre por celdas en dos pasadas y se acumula por bloques; XLS/ODS conserva un Range completo de Calamine, pero la conversión posterior a DataFrame/snapshot consulta cancelación entre filas, columnas y bloques. El parser de `worksheet_range` sigue siendo síncrono. El JOIN eager de Review procesa bloques de filas activas con cancelación entre bloques y vuelve a unir cada uno contra el dataset comparado, con un coste adicional por medir sobre datasets de trabajo. El selector nativo sigue siendo modal. Los fallbacks eager source-backed de JOIN, resolución/consolidación de Review, mutaciones y recetas de Preparar, y exportación local/ODBC ahora propagan la cancelación durante la materialización: CSV/TSV/TXT y Parquet entre lotes; JSON/JSONL/NDJSON entre registros. El dataset solo deja su representación diferida cuando la lectura termina. El hash SHA-256 del CSV temporal de Bundle comprueba cancelación entre lecturas de 64 KiB en las rutas eager y source-backed; cada lectura individual sigue siendo síncrona. La protección eager de privacidad también consulta el token entre filas; los escritores eager CSV, JSON y Parquet procesan bloques de 8.192 filas. CSV neutraliza fórmulas por bloque y JSON conserva un único arreglo. La comparación de archivos en Review ahora comparte el token `datasetComparison`: el panel ofrece cancelar, las copias Parquet, conversiones DuckDB y lecturas eager comprueban el token, y el cálculo Parquet interrumpe entre bloques y registros derramados. El resultado anterior se conserva si la cancelación gana el gate de publicación. El selector nativo sigue modal; la apertura y `worksheet_range` de XLS/ODS son monolíticas, aunque el análisis y conversión posterior consultan el token; el cierre final y sync_all de los escritores Parquet siguen siendo tramos síncronos. La paginación de conflictos eager y source-backed usa ahora el mismo token; Review ofrece cancelar la carga y conserva la página previa si la cancelación gana. La vista principal paginada también usa el token `datasetPage`: Polars Streaming cancela entre lotes, el fallback eager y el armado de filas comprueban la generación, y Review conserva visible la página anterior hasta completar o cancelar. Los tramos de `open_workbook_auto`/`worksheet_range` solo detectan la cancelación al regresar; el análisis y la conversión del rango consultan el token entre filas, celdas y bloques. El perfil de calidad cancelado conserva un estado visible en Revisar/Preparar y aclara que se descartó el resultado parcial; la acción principal ofrece reintentarlo sin reinicios automáticos. El gate local de reglas de calidad en Entregar usa el token qualityValidation y ofrece «Cancelar validación»; interrumpe la materialización DuckDB y revisa el token entre bloques Parquet y filas. Las operaciones vectorizadas solo pueden comprobar la cancelación al regresar. El preflight ODBC usa databasePreflight para cancelar materialización, protección de privacidad y análisis de filas; las llamadas síncronas al driver solo descartan el resultado al volver. La enumeración de hojas de Excel se ejecuta tras devolver la selección a Cargar; la interfaz permite cancelarla con la generación `load` y descarta la selección pendiente. `open_workbook_auto` y `sheet_names` siguen siendo síncronos, así que el token se observa al terminar la lectura. La auditoría de IPC no encontró transformaciones de filas sin token: los handlers restantes sin token se limitan a lectura/ajuste de recursos del sistema, diálogos y acciones nativas, el paso final del updater, o CRUD de contratos/catálogos acotados (recetas y calidad hasta 1 MiB, cada tarea hasta 2 MiB (el catálogo es cancelable) y el catálogo de presets hasta 100). Las llamadas síncronas de Calamine, SQLite, ODBC, escritores y limpieza de archivos solo observan cancelación cuando regresan. El corte anterior obtuvo 426/426 pruebas frontend, 21 E2E y 490 pruebas Rust ejecutables (5 ignoradas); esas cifras son previas a este cambio. La revisión anterior añadió cuatro regresiones Rust y `cargo check --tests` confirma que compilan, pero no se han ejecutado: `cargo test` termina antes del harness con `STATUS_ENTRYPOINT_NOT_FOUND` (`0xc0000139`), incluso con un manifiesto Common Controls v6 temporal. Los smokes nativos verifican flujos de archivo, no cancelación durante cada operación. La inspección de hojas se separa de la selección nativa y ofrece cancelar la lectura con la generación `load`; la llamada síncrona de calamine detecta cancelación al regresar. La vista previa `preview_delimited_header_review` también comparte `load`, comprueba el token entre su lectura acotada y sus dos parseos síncronos, y «Cancelar» invalida el trabajo y descarta la selección. `cargo fmt --check`, `cargo check --manifest-path src-tauri/Cargo.toml --lib`, `npm run build`, `npm run ipc:check`, el checker documental (4 pruebas internas aprobadas) y `git diff --check` pasan; no se ejecutaron pruebas de producto. Abrir un proyecto y restaurar una versión usan `projectOpen` para cancelar la validación del snapshot y la copia del historial. Restaurar valida el candidato antes del cambio persistente; el commit del catálogo y la activación comparten el gate. Si cancelar gana antes del commit, se conservan el catálogo y el dataset activo; si el commit gana, ambos se publican juntos. La interfaz ofrece «Cancelar apertura» y «Cancelar restauración». La interfaz conserva la operación si falla `cancel_operation`, muestra el error y permite reintentar para carga, análisis, exportación y comparación; la regresión App confirma que el dataset anterior sigue disponible. Las cancelaciones fallidas de una selección pendiente conservan el `selectionId` y las marcas de cancelación y liberación aún pendientes; la UI permite reintentar solo esos pasos y bloquea nuevas inspecciones hasta completarlos. La aceptación con datos reales/nativa y los tramos síncronos siguen abiertos. Deshacer/Rehacer consulta cancelación entre validaciones y durante la restauración eager/source-backed Parquet por lotes, antes de publicar el cursor. La comparación de revisiones puede cancelar la carga por lotes de ambas snapshots; Resolver conflictos/Consolidar y la consulta local con Polars cancelan la lectura eager del snapshot comparado. Si cancelar gana, no avanza el cursor ni entrega un resultado parcial. La vista previa source-backed de CSV/TSV/TXT/Parquet, JSON y Excel consulta cancelación por lotes. `read_parquet_frame` se mantiene como helper de tests; el flujo de producción usa la lectura cancelable.

### RV05

**Parcial — la configuración no mutadora se aplica automáticamente al importar con el perfil y esquema exactos.** Al preparar una tarea y cargar un archivo compatible, se restauran reglas, formato, privacidad y receta como borrador, sin volver a pedir confirmación. La receta no se ejecuta hasta que la persona la aplica; el esquema distinto o una importación con valores predeterminados conservan la revisión explícita. No se guardan credenciales ni permiso de sobrescritura. Al guardar una tarea con el dataset activo, el panel comprueba la compatibilidad del esquema y habilita aplicarla cuando el backend la declara lista. La prueba del panel pasa 7/7 y Playwright cubre guardado, revisión y aplicación; el smoke CDP nativo confirma receta, exportación, reapertura y restauración de fase con cleanup. El nuevo `npm run smoke:restart` confirma en dos sesiones reales que una tarea sintética se guarda, reaparece en el catálogo y puede abrirse tras reiniciar la app; valida su contenido y confirma la limpieza del registro (0→1→0). El nuevo `npm run smoke:native-selectors` recorre el panel con una tarea sintética y selectores Win32: el CSV con la columna `extra` exige confirmación y muestra la diferencia; el CSV compatible aplica la tarea y deja la receta en Preparar sin ejecutarla. La tarea se elimina al terminar. Evidencia: `.local/validation/webview2-cdp/20260922T022017Z/native-selectors.stdout.log` y `.local/validation/webview2-cdp/20260922T022017Z/summary.json`. La memoria privada del runtime debug excedió el presupuesto diagnóstico no aplicado (441.495.552 frente a 268.435.456 bytes). RV05 sigue parcial hasta la aceptación con tareas y archivos de trabajo reales.

### RV06

**Parcial — jerarquía, teclado y anuncios de progreso acotados; aceptación con lector de pantalla pendiente.** La comparación de revisiones queda plegada. En Preparar, el último resultado y Deshacer/Rehacer permanecen visibles; la lista de cambios inicia plegada y su resumen nativo la abre bajo demanda. El trap de foco ahora omite controles dentro de `details` cerrados e incluye su resumen, para que Tab no llegue a un control invisible; hay una regresión que recorre el diálogo de importación hasta «Cargar archivo». Los modales propios usan ahora el elemento HTML `dialog` con `showModal()`: el navegador aplica la modalidad nativa e inhabilita la interacción con el fondo, mientras Columnia conserva el trap de teclado y restaura el foco al cerrar. El fallo de la vista previa CSV/TSV se anuncia dentro del diálogo de revisión; se quitó la alerta global redundante que quedaba fuera de contexto. En Entregar, los campos de esquema y tabla ODBC toman el nombre de su etiqueta visible, y los errores de validación se enlazan con el control correspondiente mediante `aria-describedby`. La alerta de validación del contrato de calidad se enlaza con el fieldset de la regla afectada cuando el error identifica una regla. La auditoría de Entregar alineó los nombres accesibles de límites, referencias, orden y presets con sus etiquetas visibles. El E2E comprueba jerarquía a 200 %, colores forzados y avance con teclado a 320 CSS px; la suite actual pasa 22/22 E2E y `DeliveryPhase.test.tsx` pasa 32/32. La suite frontend actual pasa 449/449 en 51 archivos; la cobertura crítica por capa también aprueba `npm run test:coverage`. Falta aceptación nativa con lector de pantalla.

### RV12

**Completada para la matriz sintética v1 y ampliada con smoke nativo.** `perf:matrix:summary` valida los ocho cruces (1 y 100 MiB × standard, wide, low-cardinality y long-text), cada uno con 3 transformaciones, 2 actualizaciones de proyecto, RAM/disco, cancelación source-backed, salida previa intacta y limpieza confirmada. El benchmark WebView2 de 100 MiB pasa con 819.137 filas, carga en 2,74 s, paginación en 38 ms, transformación en 2,62 s, exportación en 2,78 s y memoria dentro de su presupuesto de benchmark; confirma cleanup. El exportador CSV usa un hilo para mantener el orden con el límite de memoria; las mediciones cubren el motor nativo y excluyen UI/IPC. Las formas observadas en beta pueden añadirse como nuevas corridas.

### RV16

**En curso — treinta y ocho módulos con responsabilidades extraídas.** Validación de workspace, perfiles de importación, consultas, ejecución local, automatización CLI, historial de proyectos, comparación, perfiles numéricos/categóricos/temporales, contratos y evaluación de calidad, recetas eager/lazy/source-backed, carga source-backed y materialización cancelable, inspección de libros y snapshots por bloques, exportación atómica con privacidad, receta eager en `dataset/recipe_eager.rs`, motor de comparación en `dataset/comparison_engine.rs`, ejecución de consultas en `dataset/query_execution.rs`, automatización CLI en `dataset/automation.rs`, historial de proyectos en `dataset/project_history.rs`, carga/persistencia temporal de fuentes comparadas, encabezados delimitados, seguridad CSV/Bundle, rutas seguras y lectura paginada viven en módulos internos separados. `dataset/query_execution.rs` concentra planes source-backed, lectura por bloques Parquet, agregaciones, paginación de consultas y JOIN local con cancelación. `dataset/automation.rs` concentra carga, inspección, transformación, validación y exportación para el CLI, incluidos los caminos source-backed. `dataset/project_history.rs` concentra captura, restauración y resúmenes de snapshots de proyectos. `dataset/profile_engine.rs` concentra inferencia de tipos, estadísticas de texto, perfilado eager/source-backed y agregaciones temporales. El módulo `dataset/page_reader.rs` concentra la paginación en memoria, lectura Parquet con slice pushdown y lectura source-backed CSV/TSV/TXT; preserva límites, cancelación y errores de fuente modificada. `dataset/profile_reader.rs` conserva el cache validado, orquesta perfiles eager/source-backed y calcula agregaciones temporales con el mismo sistema de cancelación. `dataset/comparison_reader.rs` prepara la comparación y pagina conflictos desde snapshots/fallbacks; cancela el trabajo y rechaza el resultado si cambia la comparación vigente. `dataset/snapshot_comparison.rs` calcula revisiones y orquesta `compare_history_snapshots` con progreso, cancelación y comprobación de vigencia. `dataset/import_schema_preview.rs` contiene el preflight del candidato detrás del mismo comando Tauri; `dataset/history.rs` reúne `HistoryManager`, sus contratos serializados y la navegación/restauración eager/source-backed cancelables de deshacer/rehacer; la lectura por lotes de las revisiones que compara `snapshot_comparison.rs` también observa cancelación. Las rutas de reexportación y los nombres JSON se conservan. `dataset/json_reader.rs` conserva el parseo cancelable de JSON/JSONL y la inspección de columnas sin cambiar errores ni contratos. `dataset/recipe_documents.rs` conserva validación, migración y persistencia atómica de recetas. `dataset/source_loading.rs` concentra carga source-backed, materialización cancelable, estimación de recursos y loaders por formato. `dataset/spreadsheet_io.rs` concentra inspección de libros, conversión tipada de rangos y snapshots por bloques con cancelación. `dataset/export_io.rs` concentra escritores CSV/JSON/Parquet/SQL/Excel/SQLite/Bundle, privacidad y publicación atómica cancelable. `dataset/import_source_inspection.rs` agrupa selector, drag/drop, hojas Excel y revisión de encabezados; `dataset/import_loading.rs` contiene carga final y descarte con publicación atómica. `dataset/page_reader.rs` contiene también la orquestación de `get_dataset_page` y sus rutas de lectura cancelables. Los comandos conservan nombres y parámetros Tauri. El corte actual pasa 494 pruebas con 5 ignoradas; además pasa `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check`, `cargo check --manifest-path src-tauri/Cargo.toml --lib`, `npm run ipc:check`, el checker documental y `git diff --check`. Los coordinadores de cancelación/publicación viven en `dataset/operation_cancellation.rs` y la coordinación de generaciones/locks en `dataset/operation_state.rs`; el trabajo continúa de forma gradual.
