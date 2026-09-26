# Contexto de Columnia

> Punto de entrada técnico para personas y agentes que trabajen en este
> repositorio. Describe el sistema tal como es hoy. Lo pendiente está en
> [`ROADMAP.md`](ROADMAP.md) y lo entregado en [`CHANGELOG.md`](CHANGELOG.md).

`CONTEXTO.md` es el nombre español histórico del `CONTEXT.md` que pide el proceso
de revisión. El registro de sesiones, reauditorías y estados anteriores (hasta el
Tier 10, cerrado el 2026-09-25) está archivado sin cambios en
[`docs/archive/2026-09/CONTEXTO.md`](docs/archive/2026-09/CONTEXTO.md).

## Ficha

| Campo | Valor |
| --- | --- |
| Producto | Estación de escritorio local para revisar, limpiar, transformar y entregar datasets confiables |
| Versión | `1.26.0`, sincronizada en npm, Cargo y Tauri; la app lee `CARGO_PKG_VERSION` |
| Arquitectura | Tauri 2 + Rust + Polars + DuckDB + React 19 + TypeScript + Vite |
| Plataforma verificada | Windows x64; macOS y Linux son objetivos de diseño sin validar |
| Licencia y distribución | MIT; hoy solo se publica el código fuente |
| Red | No se necesita para trabajar con datos locales; la entrega ODBC opcional sale solo por acción explícita y confirmación nativa |
| Validación | Local con `tools/check.ps1` (perfiles Fast, Full, Release, Package); sin CI por decisión del proyecto |

## Modelo mental del sistema

```text
Persona
  |
  v
React / App.tsx
  |  estados de UI, formularios, navegación y confirmaciones
  v
bridge.ts
  |  contratos TypeScript + invoke() + Channel de progreso
  v
Comandos Tauri / lib.rs
  |  superficie permitida y estado administrado
  v
dataset.rs
|  validación, carga, perfil, recetas, historial y exportación
v
Polars + Calamine + filesystem local

projects.rs
|  catálogo, migración, snapshots y recuperación
v
SQLite + app_data_dir privado
```

No existe un servidor HTTP de aplicación. React pide casos de uso concretos mediante IPC de Tauri. Rust conserva la autoridad sobre rutas, archivos y datasets. El frontend recibe nombres, metadatos, filas de vista previa e identificadores opacos, no rutas locales.

La automatización sin interfaz entra por `columnia-cli`, que llama directamente al mismo motor Rust sin pasar por React ni IPC. Sus comandos de datasets `inspect`, `transform`, `validate` y `batch`, y sus comandos de proyectos `project-list`, `project-save`, `project-inspect`, `project-export` y `project-delete`, emiten contratos JSON versión 1, conservan los límites y la escritura atómica del escritorio y nunca incluyen rutas, filas ni muestras en la salida. Acepta CSV, TSV, JSON, Parquet, XLSX, XLS, XLSB y ODS; los libros exigen siempre una hoja por nombre exacto y un modo de encabezado explícito. El código 0 indica éxito, el 2 una validación de calidad reprobada o un trabajo batch fallido, y el 1 un error de uso, carga o almacenamiento.

`batch` admite de 1 a 64 transformaciones en un manifiesto JSON v1 estricto. Resuelve rutas relativas desde la carpeta canonicalizada del manifiesto, aplica presupuestos de texto, comprueba todos los inputs, recetas, formatos, hojas, destinos y colisiones antes de escribir, y publica cada salida de forma atómica. No es una transacción global: un fallo dependiente de los datos detiene el lote con código 2 y conserva las salidas anteriores; el JSON informa solo conteos y el ordinal 1-based del trabajo fallido. Un manifiesto o preflight inválido termina con código 1, sin stdout ni outputs.

Los cinco comandos CLI de proyectos exigen siempre `--store <directorio>`: no infieren ni reutilizan el `app_data_dir` del escritorio. Rust canonicaliza ese almacén explícito y mantiene allí el catálogo y los artefactos administrados. `project-save` crea o actualiza por ID un snapshot materializado desde una entrada y puede adjuntar receta, reglas y perfil; `project-list` devuelve resúmenes; `project-inspect` devuelve metadatos, presencia de perfil/receta, cantidad de reglas y estado agregado del historial; `project-export` valida el proyecto completo sin activarlo ni alterar su candidato de recuperación; y `project-delete` exige que `--confirm` coincida exactamente con `--id`. Las reglas guardadas deben aprobar siempre: `--allow-unvalidated` habilita únicamente proyectos sin reglas y nunca omite una validación reprobada. La exportación publica los destinos soportados de forma atómica y los contratos informan solo nombre de archivo, tamaño, formato, calidad y metadatos agregados de privacidad, nunca la ruta ni datos del dataset.

## Flujo de producto

La interfaz sigue cuatro fases declaradas en `src/App.tsx`:

1. **Cargar**: inspecciona una fuente local desde el selector nativo o el arrastre a la ventana, permite seleccionar una hoja cuando corresponde, muestra hasta cinco archivos recientes sin persistir rutas y materializa el dataset activo.
2. **Revisar**: calcula el perfil de calidad automáticamente al cargar, resume lo que hay que arreglar y pagina la vista previa.
3. **Preparar**: propone correcciones con su antes y después, aplica las elegidas o una receta estructural atómica y ofrece Deshacer/Rehacer.
4. **Entregar**: propone comprobaciones de calidad, valida el contrato y exporta a CSV, JSON, Parquet, SQL, Excel, SQLite, un paquete ZIP o una base de datos por ODBC.

Las fases distintas de Cargar se deshabilitan mientras no exista un dataset. Una operación activa bloquea la navegación que pueda competir con ella. Si el dataset cambia, cualquier validación de entrega previa queda obsoleta y debe ejecutarse de nuevo.

## Arquitectura por archivo

| Ruta | Responsabilidad |
| --- | --- |
| `src/main.tsx` | Monta `<App />` en modo estricto de React y publica la marca de bootstrap usada para separar compilación fría del primer render. |
| `src/App.tsx` | Coordina el flujo principal y los estados compartidos de la interfaz en 867 líneas. |
| `src/components/` | Componentes accesibles extraídos para diálogos, tabs de revisión y progreso cancelable. |
| `src/components/ResourceMonitor.tsx` | Monitor compacto de consumo de CPU/RAM del proceso y del equipo, con polling nativo, selector persistente de concurrencia Rayon y estado degradado para el shell web. |
| `src/features/load/` | Fase Cargar: vista y modelo de inspección, selección de hojas, arrastre nativo sin rutas en React, archivos recientes sin rutas, progreso, cancelación y recuperación. |
| `src/features/review/` | Fase Revisar: diagnóstico, perfil de calidad, tabs y vista previa paginada. |
| `src/features/prepare/` | Fase Preparar: vistas, editor de recetas, historial, modelo puro y controlador de IPC/invalidationes. |
| `src/features/projects/` | Catálogo, guardado, apertura, recuperación y eliminación accesible de proyectos locales. |
| `src/features/delivery/` | Fase Entregar: vista, métricas y modelo tipado de contrato, compuerta de calidad y exportación. |
| `src/bridge.ts` | Contrato TypeScript del IPC y única fachada de `invoke()` usada por la UI. |
| `src/styles.css` | Sistema visual y layout de la aplicación; incluye foco visible, targets mínimos, reducción de movimiento y una paleta explícita para `forced-colors: active`. |
| `playwright.config.ts` | Configuración de Playwright para E2E del shell web Vite, con Chromium/Edge local, preview de producción reutilizable, trazas y artefactos solo en fallos. |
| `e2e/` | Pruebas E2E del shell web, primer render, accesibilidad, preferencias responsive y ciclo de proyectos con IPC Tauri simulado; la ventana WebView2 nativa tiene un probe CDP opcional. |
| `src-tauri/src/main.rs` | Entrada mínima del ejecutable; delega en `columnia_lib::run()`. |
| `src-tauri/src/lib.rs` | Inicializa Tauri, instancia única, diálogo nativo, eventos nativos de arrastre, estados de dataset/proyectos y los comandos permitidos. |
| `src-tauri/src/resource.rs` | Obtiene CPU y memoria del proceso Columnia y del sistema mediante `sysinfo`, sin exponer rutas ni datos. |
| `src-tauri/src/dataset.rs` | Motor de datos principal. Contiene carga, tipos, perfiles, recetas, historial y exportación; el fingerprinting de duplicados parecidos vive en el módulo interno acotado `dataset_fingerprints.rs`. |
| `src-tauri/src/dataset_fingerprints.rs` | API interna para huellas exactas/normalizadas de filas, fast-path ASCII y normalización Unicode usada por perfilado y retiro de duplicados parecidos. |
| `src-tauri/src/projects.rs` | Catálogo SQLite v15 compatible con catálogos v1–v14; incluye snapshots Parquet durables, perfil, historial, actividad SQL agregada, vista/etapa de Revisar, página de muestra, motor SQL, cobertura de correlaciones, perfil de rendimiento y preferencias versionadas de Entregar y comparación. La migración v15 agrega un índice parcial para elegir el candidato de recuperación. |
| `src-tauri/src/automation.rs` | Parser estricto, contratos JSON y orquestación reutilizable de datasets, lotes y los cinco comandos CLI de proyectos. |
| `src-tauri/src/bin/columnia-cli.rs` | Ejecutable CLI mínimo que delega en el módulo de automatización. |
| `src-tauri/capabilities/main.json` | Capability mínima para la ventana `main`: solamente `core:default`. |
| `src-tauri/tauri.conf.json` | Ventana, build, bundle y CSP de producción/desarrollo. |
| `tools/check.ps1` | Entrada única para los gates locales Fast, Full y Release; genera evidencia JSON auditable en `.local/validation/` y en Release ejecuta supply chain/instalador. |
| `vitest.config.ts` / `tools/check-coverage.mjs` | Cobertura V8 global y por capa crítica: App, Entrega, Preparar y controller con umbrales 80% statements/lines y 75% branches/functions. |
| `tools/check-supply-chain.ps1` / `src-tauri/deny.toml` | npm audit, cargo audit, cargo-deny, secretos, avisos de terceros y política de red con excepciones upstream justificadas. |
| `tools/check-network-policy.mjs` / `docs/reference/network-privacy.md` | Inventario local de red, CSP productivo y política de telemetría desactivada por defecto. |
| `src-tauri/src/privacy.rs` | Serialización pública sanitizada para reportes, recetas y manifiestos: elimina rutas, valores, emails, secretos y referencias de filesystem, conservando identificadores, estados y conteos agregados. |
| `tools/check-installer-contract.ps1` / `tools/smoke-installed-artifact.ps1` | Contrato de NSIS currentUser, WebView2 bootstrapper, smoke del artefacto instalado y recursos legales reproducibles. |
| `tools/check-updater-key-policy.mjs` / `fixtures/updater/key-policy-v1.json` | Fingerprint de la clave pública embebida, release puente para rotación y recuperación fail-closed. |
| `tools/verify-published-assets.mjs` | Descarga posterior a publicación y verificación criptográfica local de manifiesto, tamaño, SHA-256 y firma minisign. |
| `tools/generate-sbom.ps1` / `tools/extract-package-lock-packages.mjs` | Generan offline un SBOM CycloneDX 1.6 reproducible desde ambos lockfiles, compatible con Windows PowerShell 5.1. |
| `docs/reference/feature-parity.md` | Matriz de paridad verificable con `sistema anterior`, con entregas CSV/JSON/Parquet/SQL/Excel/SQLite, comparación por columna y visualizaciones accesibles documentadas. |
| `tools/check-bundle.mjs` | Mide presupuestos JS/CSS e inventaría bundles de distribución nuevos o actualizados. |
| `tools/smoke-tauri.ps1` | Arranca `npm run tauri dev`, comprueba Vite y el ejecutable debug, registra hitos monotónicos de Vite/proceso/ventana, ejecuta un preflight de contrato de `ProjectsPanel` y limpia solo su Job Object con reintento acotado. |
| `tools/probe-webview2-cdp.ps1` | Arranca el comando real con `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` de loopback, verifica `/json/version` y `/json/list`, conecta Playwright al WebView2, atribuye el perfil por proceso/fase y aplica presupuestos observables de 512 MiB de working set, 256 MiB de memoria privada y transformaciones nativas sostenidas solo a procesos de su Job Object; admite un CSV temporal configurable para medir el recorrido grande; restaura el entorno y limpia su Job Object. |
| `tools/probe-webview2-restart.ps1` | Ejecuta las fases aisladas prepare/verify del reinicio real y eleva al resumen de cada fase el estado del presupuesto y el conteo/duración IPC, delegando el cleanup al probe CDP. |
| `tools/probe-webview2-playwright.mjs` | Conecta al endpoint CDP con Playwright, espera el shell listo y separa estado funcional de presupuesto de primer render relativo al bootstrap; ambos son necesarios para aprobar el probe. |
| `tools/probe-webview2-projects.mjs` | Conecta al endpoint CDP y verifica el contrato accesible de `ProjectsPanel`, repite transformaciones/exportaciones nativas sostenidas, y en el smoke debug guarda/abre/consulta/elimina un proyecto sintético con cleanup; registra duración por comando y total, pero no rutas ni datos del catálogo. |
| `tools/summarize-performance.ps1` | Lee únicamente `summary.json` dentro de `.local/validation/`, clasifica señales web/CDP/desktop y separa `cdp-large-dataset` de la muestra CDP normal; conserva perfil de memoria, presupuestos y duraciones nativas, calcula deltas y el intervalo proceso-listo→ventana-visible, y genera `summary.json`/`summary.csv` sin rutas absolutas ni datos sensibles. |
| `tools/capture-accessibility-evidence.mjs` | Construye el preview local, captura desktop/móvil/zoom CSS 125% y 200%/forced-colors y publica capturas más un resumen sanitizado de landmarks, foco, targets y overflow bajo `.local/validation/`. |
| `tools/check-accessibility-baseline.mjs` | Compara la evidencia visual más reciente con el contrato versionado de `fixtures/accessibility/`, verificando escenarios, landmarks, targets, foco, overflow y SHA-256 de cada captura. |
| `tools/capture-release-evidence.ps1` / `tools/capture-release-evidence.mjs` | Construyen el binario Tauri sin bundle, exigen árbol limpio, registran commit/rama/lockfiles y capturan desktop/móvil/zoom CSS 125% y 200%/forced-colors desde el ejecutable optimizado. |
| `tools/check-release-evidence.mjs` | Comprueba el sumario release contra el baseline de escenarios, contrato, versiones, commit limpio, hashes de lockfiles y ownership; solo `--update-baseline` acepta una diferencia visual intencional. |
| `tools/release.ps1` | Orquesta el pre-release local con toolchains, documentación, IPC, perfil Release/Package, CLI, evidencia visual, baseline y rendimiento; exige rama/árbol limpio y nunca crea tags ni publica servicios remotos. |
| `tools/generate-updater-manifest.mjs` / `tools/check-updater-manifest.mjs` / `tools/test-updater-manifest.mjs` | Generan y verifican el par manifiesto/inventario del updater; el contrato reproducible muta un fixture local para probar truncado, firma alterada, manifiesto incompleto/corrupto y URL insegura sin contactar la red. |
| `tools/check-documentation.mjs` | Valida el mapa Diátaxis, ADR/CHANGELOG, enlaces locales, UTF-8 sin BOM, coherencia de versiones y ownership de imágenes. |
| `tools/benchmark-datasets.ps1` | Genera un CSV sintético cercano al objetivo indicado, mide iteraciones sostenidas de transform CSV/Parquet, actualiza el mismo proyecto el número indicado de veces y verifica reapertura/exportación durable; conserva solo tiempos, conteos, estados y cleanup sin datos después de borrar el almacén temporal. |
| `tools/benchmark-webview2-dataset.ps1` | Genera un CSV temporal cercano a 100 MiB, lo entrega al selector Win32 del probe y exige dentro de WebView2 carga, paginación, transformación, exportación, memoria agregada y cleanup; elimina el dataset al terminar y publica solo evidencia sanitizada. |
| `tools/benchmark-datasets.ps1` / `tools/benchmark-datasets.ps1` | Benchmark cruzado de la inspección de 100 MiB contra `sistema anterior`, con selección del entorno Python, comparación de duración/working set, validación de conteos y cleanup. |
| `tools/check-performance-baseline.ps1` | Convierte el resumen CDP, el startup desktop, el benchmark de datasets, el recorrido WebView2 de dataset grande y el reporte Package en un gate contra `fixtures/performance/performance-baseline-v1.json`, incluyendo duración máxima por operación, con evidencia sanitizada y estado explícito. |
| `tools/verify-experience.ps1` | Ejecuta juntos `accessibility:check` y `perf:check` para verificar los contratos visual y de rendimiento después de generar evidencias. |
| `tools/verify-tier.ps1` | Orquesta el tier reproducible completo: tests, build, accesibilidad, benchmark sostenido, Package, smokes CLI/WebView2 y gates finales; permite omitir Package o native de forma explícita. |
| `tools/check-toolchains.mjs` / `rust-toolchain.toml` | Rechazan Node/npm/Rust fuera de las versiones exactas del entorno de release. |
| `tools/check-ipc-inventory.mjs` / `docs/reference/ipc-inventory.json` | Generan y verifican desde Rust el inventario de 68 comandos de producción, 4 debug y 60 estructuras compartidas; los tests de contrato consumen el inventario. |
| `docs/reference/legal-distribution-review.md` / `src/App.tsx` | Hacen descubribles MIT, notices y privacidad local; la revisión legal de canal/jurisdicción/contacto sigue pendiente antes de publicar. |
| `ACCESSIBILITY_MANUAL_CHECKLIST.md` | Checklist operativa para teclado, lector de pantalla, High Contrast, zoom y evidencia manual; no declara completada la auditoría sin una sesión real. |
| `fixtures/accessibility/visual-baseline-v1.json` | Contrato versionado de escenarios y mínimos visuales; no contiene imágenes ni datos de usuario. |
| `fixtures/performance/performance-baseline-v1.json` | Presupuestos versionados de memoria CDP, startup desktop después de proceso nativo listo, transformaciones nativas sostenidas, benchmark sostenido de 100 MiB, duración por operación y bundle frontend. |
| `tools/smoke-cli.ps1` | Verifica la CLI real con fixtures deterministas, libros, calidad, lotes, atomicidad por trabajo y errores seguros. |
| `fixtures/automation/` | Entradas, receta y resultados esperados del smoke de automatización. |
| `README.md` | Descripción funcional y guía de uso/desarrollo. |
| `THREAT_MODEL.md` | Activos, fronteras de confianza, amenazas, controles implementados y riesgos residuales. |
| `ROADMAP.md` | Pendientes vigentes y criterio de salida de V1. Lo entregado está en `CHANGELOG.md`. |
| `CONTRIBUTING.md` | Ramas, commits, revisión local y límites de alcance. |
| `docs/` | Tutoriales, how-to, referencias, explicaciones, ADRs, gobierno del repositorio y política de fixtures. La ficha de dependencias y controles está en `AUDITORIA.md`; el historial de revisiones, en `docs/archive/`. |
| `CHANGELOG.md` | Registro de cambios publicados y limitaciones conocidas por versión. |
| `fixtures/accessibility/release-evidence-baseline-v1.json` | Casos, contrato, owner, propósito, fecha de aprobación y hashes de capturas generadas desde el binario release. |
| `.codegraph/` | Índice semántico local del repositorio. Úsalo antes de búsquedas textuales para entender símbolos y rutas de llamadas. |
| `.agents/skills/` | Skills locales disponibles para tareas especializadas del repositorio. |

## Estado y ciclo de vida de los datos

### Estado de React

`App` coordina estados separados para runtime, dataset, perfil, cambios, historial, exportación, reglas de calidad y fase activa. Los estados importantes son uniones discriminadas (`loading`, `ready`, `error`, etc.), lo que hace explícitas las transiciones visibles.

- La vista previa usa páginas de 50 filas.
- Carga, perfil y exportación reciben progreso mediante `Channel<OperationProgress>`.
- La cancelación es cooperativa y se identifica por operación: `load`, `profile` o `export`.
- Durante una carga de reemplazo se conserva el dataset anterior para recuperarlo si la nueva selección se cancela o falla.
- Un perfil se invalida después de cualquier mutación.
- La compuerta de entrega se invalida cuando cambian el dataset o sus reglas de calidad.

### Estado de Rust

`DatasetState` administra:

- `current`: dataset activo protegido por `Mutex`;
- `pending_selection`: selección pendiente con ID opaco, ruta privada y hojas detectadas;
- contadores atómicos de generación para cancelar carga, perfil y exportación sin mezclar operaciones.

`LoadedDataset` conserva una ruta fuente privada opcional, el nombre/tamaño visibles, el `DataFrame`, un perfil opcional en caché y el historial. Separar la identidad visible de la ruta permite restaurar un snapshot aunque el archivo original ya no exista. El dataset activo se materializa en memoria, pero las recetas compatibles de I1 construyen y ejecutan un plan Polars lazy antes de publicar el candidato; esa colección y los lectores `LazyCsvReader`/`scan_parquet` pasan por una única frontera con el motor `streaming`, baja memoria y `rechunk` desactivado. Dentro de las calculadas, suma, resta, multiplicación, división, concatenación y extracción de año, mes o día sobre `Date`/`Datetime` sin zona horaria ya comparten esa ruta cuando no hay filtros previos; el preflight conserva la validación de fechas no representables. Las operaciones no compatibles conservan el camino eager para mantener sus validaciones estrictas. La apertura y validación de snapshots durables de proyectos y del historial temporal también usa `scan_parquet` por esa frontera, aunque al final produce el frame materializado que exige el contrato de sesión. La restauración del historial procesa cada snapshot una sola vez y conserva únicamente el frame del cursor durante la comprobación, no todos los frames históricos a la vez. Columnia no impone un límite fijo al tamaño del dataset. La capacidad efectiva depende de la RAM, el espacio temporal en disco, la CPU y la expansión propia del formato durante la lectura, el perfilado y las transformaciones. La interfaz sigue recibiendo un `DataFrame` activo para preservar el contrato actual. La paginación de la muestra activa y las consultas Polars simples sin comparación pueden leer el snapshot Parquet del cursor con lectura acotada por bloques cuando el historial está sano; las consultas cuentan coincidencias sin materializar otra copia y solo retienen la página o los acumuladores, y vuelven al `DataFrame` ante historial degradado o snapshot inconsistente. Cuando el historial está degradado, el dataset no ha sido mutado y la fuente original es CSV, TSV, TXT delimitado o Parquet, las consultas DuckDB registran el archivo directamente desde disco, incluso al unirlo con el snapshot comparado; una mutación invalida la referencia para no consultar el archivo obsoleto. `JOIN`, comparación, operaciones generales y ejecución integral fuera de memoria mantienen sus límites explícitos. La detección exacta de duplicados proyecta solo el conteo mediante `unique` lazy en streaming, y la detección normalizada recorre bloques en paralelo y derrama huellas XXH3-128 en 256 cubetas temporales para ordenar una sola cubeta en RAM; el temporal contiene fingerprints, no valores del dataset, y se elimina al terminar o cancelar. El perfilado por columna usa una cola acotada de hasta cuatro trabajadores, conserva el orden original y reporta progreso agregado por sub-etapa sin construir vectores numéricos completos. Los `JOIN` locales `INNER`/`LEFT` sin agregación procesan el lado `dataset` por bloques y conservan solo la página global y un bloque unido temporal; sus agregaciones fusionan estados por bloques sin conservar el resultado unido completo. `FULL` recorre el lado `dataset` por bloques y añade bloques de filas derechas no emparejadas mediante anti-join estable, aunque el frame derecho anti-join todavía se materializa dentro de los límites explícitos; joins/comparaciones grandes y la ejecución fuera de memoria general aún requieren la siguiente expansión incremental. Las recetas source-backed ejecutan en DuckDB las seis extracciones textuales actuales —tokens, dígitos, letras y segmentos antes/después de delimitadores— y normalizaciones de correo, teléfono y dirección sobre CSV/TSV/TXT delimitado o Parquet, conservando Unicode, nulos, coincidencias ausentes, resultados vacíos y conteos exactos sin cargar las filas en el `DataFrame` activo.

En v0.65.0, la apertura de CSV, TSV, TXT delimitado y Parquet de al menos
512 MiB es source-backed: conserva solo esquema y la primera página de 50 filas
en el `DataFrame`, calcula el total desde disco y materializa las filas completas
solo cuando una operación eager las necesita. Mientras la fuente no cambie,
la vista previa y las consultas compatibles pueden seguir leyendo directamente
desde disco; si cambia, desaparece o una operación no admite ese camino, se usa
el fallback materializado con validación de tamaño y conteo.

En v0.66.0, el perfilado de esas fuentes también conserva la frontera de
memoria: derrama firmas de duplicados por cubetas, procesa cada columna por
bloques, ordena estadísticas numéricas mediante corridas temporales y limita
las correlaciones a la muestra solicitada. Categorías y tendencias leen solo
la columna necesaria. El resultado mantiene los mismos campos del perfil
normal; el dataset solo se materializa cuando una operación posterior necesita
mutarlo o publicar todas sus filas.

En v0.67.0, la validación de calidad de reglas fila-a-fila y de esquema/conteo
recorre esos snapshots por bloques y acumula únicamente los conteos de cada
regla. La sesión source-backed conserva su esquema vacío, se comprueba el
tamaño de la fuente antes y después y las reglas globales que necesitan estado
completo mantienen el fallback materializado.

En v0.68.0, la exportación Parquet source-backed sin receta ni privacidad
adicional convierte la fuente directamente desde disco con DuckDB, publica
atómicamente y limpia el snapshot temporal sin materializar el `DataFrame`
activo. En v0.69.0, la misma ruta incorpora JSON para fuentes CSV, TSV, TXT
delimitado y Parquet, conservando orden, validación de cambios, publicación
atómica y cleanup. La validación compatible se ejecuta antes por bloques y los
demás formatos, recetas y protecciones mantienen su fallback materializado.

En v0.70.0, la validación de calidad source-backed amplía esa frontera a
`unique`, `unique_together`, `monotonic`, `aggregate_check`,
`aggregate_reconciliation` y `distribution_drift`: cada clave global se
derrama por cubetas o cada acumulador se fusiona por bloques, conservando
conteos, nulos, cancelación y comprobación del tamaño de la fuente. Las
recetas y transformaciones generales siguen materializando hasta completar
su ruta incremental.

En v0.80.0, las recetas source-backed que solo renombran y proyectan columnas
se convierten directamente desde la fuente a un Parquet privado administrado.
El dataset conserva esquema, conteo, orden y preview sin llenar el
`DataFrame`; una operación posterior que necesite valores completos todavía
materializa con validación de tamaño y conteo, y las recetas que transforman
valores mantienen el fallback eager.

En v0.92.0, esa ruta admitió hasta tres filtros combinados con selección y
renombrado. En v0.93.0, el mismo plan añade casts, fechas fijas `YMD`, `DMY` y
`MDY`, y columnas calculadas de suma, resta, multiplicación y concatenación.
En v0.94.0 también extrae año, mes y día tras un parseo de fecha fijo y sin
filtros previos. DuckDB conserva las etapas de renombrado, transformación,
filtro y proyección, publica solo el resultado Parquet y mantiene la paridad
eager; división, fechas ISO y conflictos de fecha siguen materializándose para
conservar sus validaciones estrictas.
En v0.95.0 el reemplazo literal también se ejecuta sobre la fuente después del
filtro, conserva nulos y calcula el conteo exacto de celdas cambiadas; las
expresiones regulares mantienen el fallback lazy/eager.

En v0.96.0 la unión de columnas de texto también se ejecuta sobre la fuente:
DuckDB conserva el orden de las fuentes, omite nulos sin crear separadores y
mantiene las cadenas vacías, permite casts numérico→texto validados y respeta
`keepColumns` y `dropSources` antes de publicar el snapshot.

En v0.97.0 la división literal también se ejecuta sobre la fuente: DuckDB
conserva delimitadores Unicode, segmentos vacíos, nulos y el resto en el último
destino, además de `keepColumns`, renombrados y `dropSource`; una unión posterior
puede consumir las columnas físicas dentro de la misma consulta.

En v0.98.0 las extracciones textuales también se ejecutan sobre la fuente:
DuckDB conserva tokens, dígitos ASCII, letras Unicode y segmentos antes/después
de delimitadores literales, incluidos nulos, coincidencias ausentes y resultados
vacíos; la regresión compara las seis variantes contra la ruta eager.

En v0.99.0 la normalización de contactos también se ejecuta sobre la fuente:
DuckDB conserva el recorte y minúsculas del correo, los prefijos y dígitos del
teléfono, los espacios Unicode de las direcciones y el conteo exacto de celdas;
las extracciones posteriores observan los valores ya normalizados.

En v0.100.0 los resúmenes por grupo también se ejecutan sobre la fuente:
DuckDB conserva el primer orden de aparición, agrupa claves nulas y publica
`sum`, `mean`, `min`, `max`, `count` y `count_unique` con validaciones de tipo,
precisión, overflow y finitud. El resultado source-backed mantiene separado el
conteo de grupos, filas colapsadas y filas eliminadas, y la regresión compara
salida y contadores contra eager.

En v0.101.0 los tratamientos IQR también se ejecutan sobre la fuente:
DuckDB calcula un baseline común de cuantiles por bloque lógico y soporta `cap`,
`drop` e `impute` sobre columnas numéricas, conservando nulos, tipos y el
baseline posterior a filtros. La ruta valida mínimo de valores, finitud,
precisión y umbrales, y separa celdas ajustadas, filas retiradas y filas
eliminadas por filtros.

En v0.102.0 las fechas ISO seguras también se ejecutan sobre la fuente:
DuckDB conserva fechas, horas sin offset y valores con sufijo UTC `Z`, mientras
los offsets distintos de UTC o valores inválidos vuelven al fallback eager
estricto.

### Historial y atomicidad

Las recetas IQR compatibles con lazy/streaming también pueden usar
`keepColumns` cuando la proyección conserva todas las columnas tratadas. Si una
proyección elimina una dependencia, se mantiene el fallback eager y la
validación cerrada.

En la importación de sesiones y en el ciclo durable de proyectos, los snapshots
que no son el cursor se copian byte a byte y solo consultan su footer/esquema;
el cursor se materializa para comprobar igualdad con el frame activo. Las filas
de las demás revisiones se leen bajo demanda al hacer undo/redo, evitando cargar
todo el historial en RAM durante la apertura o el guardado.
El bridge declara y valida los metadatos opcionales de sesión con enteros seguros,
banderas booleanas y categorías no portables como listas de texto; esos nombres
siguen siendo señales sanitizadas, no resultados ni cachés reanudables.

Cada revisión reversible de la sesión se guarda como snapshot Parquet en un directorio temporal:

- máximo normal: 12 entradas;
- presupuesto total: 1 GiB;
- se elimina al cerrar o reemplazar la sesión;
- si un snapshot individual excede el presupuesto, el cambio puede aplicarse, pero la reversión se desactiva y se informa el motivo;
- una receta completa publica un solo candidato o no publica nada;
- `publish_candidate` prepara la vista previa y registra el historial antes de sustituir el `DataFrame` activo;
- la exportación escribe y sincroniza un temporal antes de reemplazar el destino.

Los proyectos guardan el frame materializado como una nueva generación Parquet y actualizan después el puntero SQLite dentro de una transacción. El esquema SQLite v15 conserva reglas de calidad, borrador opcional de receta, perfil cacheado, historial con su cursor, actividad SQL agregada, vista y etapa de Revisar, página visible de la muestra del workspace, motor SQL elegido, cobertura de filas de correlaciones, perfil de rendimiento, formato de exportación, protección de datos, claves de comparación y tipo de JOIN; migra catálogos v1–v14 y crea un índice parcial por `last_opened_at` para seleccionar el candidato de recuperación. El historial durable mantiene los mismos límites de 12 revisiones y 1 GiB; la actividad SQL conserva como máximo cinco estados, duraciones y conteos de filas, sin consultas, rutas ni valores; la vista solo admite `diagnosis` y `preview` y vuelve a Diagnóstico cuando falta; la etapa solo admite `load`, `review`, `prepare` y `deliver` y vuelve a Revisar cuando falta; el motor SQL solo admite `polars` o `duckdb` y vuelve a la preferencia local cuando falta; el perfil de rendimiento solo admite `conservative`, `balanced` o `maximum` y vuelve a la preferencia local cuando falta; el formato, la protección y el JOIN usan listas cerradas y vuelven a sus valores locales cuando faltan; las claves se validan sin duplicados y se filtran contra el esquema restaurado; el offset de página se valida contra el snapshot y vuelve a cero si ya no representa una página real. Abrir valida todos los artefactos antes de sustituir el dataset activo y copia el perfil, historial y actividad guardados a estructuras temporales de sesión; una corrupción hace fallar la apertura completa. La recuperación es explícita desde Cargar y no abre datos silenciosamente.

## Contrato React ↔ Rust

La superficie pública está centralizada en `src/bridge.ts` y registrada en `src-tauri/src/lib.rs`.

### Runtime y carga

- `get_app_info`
- `get_resource_usage`
- `pick_dataset_source`
- `load_dataset_selection`
- `discard_dataset_selection`
- `get_dataset_page`

### Perfil, cancelación y entrega

- `get_dataset_profile`
- `validate_quality_rules`
- `cancel_operation`
- `export_dataset`

### Preparación e historial

- `remove_duplicates`
- `normalize_column_names`
- `trim_text_values`
- `normalize_text_values`
- `apply_safe_corrections`
- `apply_transform_recipe`
- `save_transform_recipe`
- `pick_transform_recipe`
- `get_history_state`
- `undo_last_change`
- `redo_last_change`

### Proyectos y recuperación

- `list_projects`
- `save_project`
- `open_project`
- `delete_project`

Regla de mantenimiento: cualquier cambio de nombre, argumentos, serialización o respuesta en Rust debe reflejarse en el bridge y en el inventario generado. `src/ipc-contract.test.ts` verifica automáticamente comandos registrados, argumentos serializados, tipos de retorno superiores y las 69 estructuras compartidas del inventario. Las subestructuras de `TransformRecipe` tienen interfaces nominales equivalentes a Rust; los alias públicos históricos se conservan para no romper consumidores.

## Capacidades implementadas

### Entrada

- CSV, TSV y TXT delimitado.
- JSON como arreglo de objetos y JSON Lines (`.jsonl`/`.ndjson`).
- Parquet.
- Excel y ODS mediante XLSX, XLS, XLSB y ODS.
- Selección de hoja y modo de encabezado para libros.
- UTF-8 estricto con BOM opcional.
- Detección conservadora de coma, punto y coma, tabulador o `|`; TSV fuerza tabulador.

CSV y otros formatos delimitados se conservan físicamente como texto para no inventar un esquema. El perfil puede detectar semántica numérica segura sin convertir identificadores con ceros iniciales o enteros que perderían precisión. Parquet conserva su esquema nativo compatible.

### Revisión

- esquema, dimensiones y vista previa paginada;
- nulos, completitud y cardinalidad;
- duplicados adicionales;
- métricas de texto y sugerencias conservadoras de tipos;
- mínimo, máximo, media, desviación muestral, cuartiles, mediana y outliers IQR para números compatibles;
- resumen acotado de grupos por columnas categóricas no sensibles, con top 8 y “Resto”, sin categorías raras;
- caché del perfil durante la sesión hasta que el dataset cambia.

### Preparación

- eliminación de duplicados;
- normalización determinista de encabezados;
- recorte y normalización de texto;
- correcciones recomendadas agrupadas;
- renombres, casts estrictos y parseo de fechas;
- hasta tres filtros AND y una columna calculada;
- buscar/reemplazar literal y selección/reordenamiento de columnas;
- división y combinación de columnas;
- tratamiento IQR por límite o eliminación;
- imputación IQR reversible por mediana, con preservación de tipos numéricos y `_cambios`;
- imputación categórica explícita de nulos textuales como `Desconocido`, sin tocar números ni `_cambios`;
- protección confirmable de valores no nulos en columnas personales detectadas mediante `[REDACTED]`, conservando columnas, números, nulos e `_cambios` y con reversión desde el historial;
- agrupación con agregaciones tipadas;
- consulta SQL local de solo lectura con motor Polars predeterminado o DuckDB opcional sobre snapshots Parquet temporales; ambos conservan filtros, agregaciones por bloques, `GROUP BY` compuesto de hasta ocho columnas y `JOIN` de hasta ocho pares de claves, con conteo exacto, paginación, orden estable, claves nulas, límites de cardinalidad y esquema de claves coalescidas;
- comparación completa de filas y por claves con índices temporales particionados, conflictos paginados y consolidación de nuevas claves sin retener mapas globales de firmas en memoria;
- normalización de correos, teléfonos y direcciones;
- extracciones textuales Unicode;
- recetas JSON versión 1 guardables y cargables;
- historial multinivel Deshacer/Rehacer.
- ejecución Polars lazy para renombres, casts, filtros, búsqueda/reemplazo
  literal, selección, unión y división de columnas, columnas calculadas
  numéricas compatibles, partes temporales lazy sin filtros previos y como claves
  de agrupación, resúmenes
  agrupados incluso con filtros previos (la búsqueda y reemplazo literal o regex
  segura se
  aplica antes dentro del plan y el preflight proyecta solo las columnas
  necesarias), normalizaciones de contactos y extracciones textuales incluso
  antes de resumir grupos; las columnas calculadas numéricas y concatenadas
  también pueden alimentar sus claves y fuentes de agregación; las columnas
  derivadas por split y merge también pueden alimentar la agrupación, con
  preflight posterior a las etapas estructurales; las operaciones restantes
  usan fallback eager atómico. Las recetas pueden combinar parseos de fecha con
  conversiones en columnas distintas y ejecutar split y merge juntos si ninguna
  etapa descarta una fuente todavía necesaria.

Las recetas se validan y ejecutan en orden determinista. Una entrada inválida, pérdida de precisión, división por cero o conflicto entre pasos revierte el lote completo.

### Entrega

- exportación atómica a CSV, JSON, Parquet, SQL, Excel y SQLite, con neutralización de fórmulas de texto en CSV; el bundle ZIP auditable añade `recipe.json` validada cuando existe un borrador y la referencia/hash correspondiente en `manifest.json`;
- contratos de hasta 16 reglas base y avanzadas: `allowed_values`, `regex`, `dtype`,
  unicidad compuesta, comparación, referencias, monotonía, agregados, drift,
  fechas, condiciones, esquema y conteo de filas;
- tolerancias por cantidad y/o porcentaje, resultados con conteos y confirmación
  explícita para exportar sin reglas;
- importación desde sistema anterior y guardado como `columnia-quality-rules` v1, con
  diálogos nativos y rutas privadas en Rust.

## Invariantes de seguridad y privacidad

No rompas estas reglas sin una decisión explícita documentada:

- El procesamiento de datasets ocurre localmente.
- React no recibe rutas de archivos ni autoridad general sobre el filesystem.
- Los diálogos nativos y las operaciones de archivos viven en Rust.
- Toda ruta de lectura elegida se canonicaliza, debe resolver a un archivo regular y rechaza enlaces simbólicos. Para escrituras se canonicaliza la carpeta, se exige un nombre de archivo y se rechazan destinos existentes, incluso enlaces colgantes, que no sean archivos regulares.
- La ventana principal conserva permisos mínimos; no habilites filesystem, shell, HTTP u opener por comodidad.
- La CSP de producción no permite CDN, navegación remota, objetos, frames ni conexiones web externas.
- Los errores y contratos de calidad no deben filtrar muestras de datos.
- Un fallo o cancelación de exportación no debe destruir un archivo previo.
- Una transformación compuesta debe ser atómica.
- No añadas CI, GitHub Actions, telemetría o servicios de pago como requisito sin revertir expresamente las decisiones vigentes.

La canonicalización cubre todos los puntos actuales de entrada por diálogo para datasets, recetas y exportaciones, con pruebas de segmentos `..`, directorios, symlinks en Unix y reparse points válidos o colgantes en Windows. La prueba Windows se omite limpiamente si el sistema no concede permiso para crear symlinks. Rust limita los payloads semánticos de receta a 4.096 caracteres por campo y 65.536 acumulados; las reglas de calidad conservan el máximo de 16 y admiten hasta 256 caracteres por columna y 2.048 acumulados. Estos presupuestos se aplican antes de validar/exportar o aplicar/guardar, pero no sustituyen un límite de memoria del transporte IPC.

## Desarrollo y validación

### Arranque local

```powershell
npm install
npm run tauri dev
```

`npm run dev` abre solamente el frontend Vite. En ese modo la interfaz muestra que el motor Rust no está conectado; los casos de uso de datos requieren Tauri.

Para ejecutar el E2E reproducible del shell web (sin IPC nativo) instala una vez el navegador y ejecuta:

```powershell
npm run test:e2e:install
npm run test:e2e
```

Playwright construye y sirve un preview Vite local en `http://127.0.0.1:4173` (en Windows usa el canal Edge instalado y en otros sistemas el Chromium de Playwright); las trazas, capturas y vídeos de fallos se guardan en directorios ignorados por Git. Esta cobertura no sustituye todavía el E2E de la ventana Tauri ni los flujos que dependen de comandos Rust.

### Gates locales

```powershell
.\tools\check-governance.ps1
.\tools\check.ps1 -Profile Fast
.\tools\check.ps1 -Profile Full
.\tools\check.ps1 -Profile Release
.\tools\check.ps1 -Profile Package
npm run smoke:desktop -- -TimeoutSeconds 120
npm run smoke:cli
npm run smoke:installer
npm run smoke:cdp
npm run updater:key:check
npm run updater:verify-published -- --manifest-url <https-url> --output-dir <evidence-dir> --target windows-x86_64 --expected-version <version>
npm run perf:summary
npm run perf:benchmark
npm run perf:i1
npm run perf:i1:check
npm run accessibility:visual
npm run accessibility:check
npm run docs:check
npm run accessibility:release
npm run accessibility:release:check
npm run perf:check
npm run verify:experience
npm run verify:tier
```

| Perfil | Incluye |
| --- | --- |
| Fast | `cargo fmt --check`, `cargo check`, Vitest, build TypeScript/Vite y presupuesto frontend |
| Full | Fast + Clippy con warnings como errores + pruebas Rust de biblioteca |
| Release | Full + SBOM CycloneDX reproducible + build Tauri optimizado sin bundle |
| Package | Release + MSI/NSIS en Windows + inventario diferencial con tamaño y SHA-256 + smoke del instalador NSIS |

Después de generar evidencia, `npm run accessibility:check` valida el contrato
visual versionado y el SHA-256 de cada captura. `npm run perf:check` compara la
última evidencia CDP, el benchmark y el reporte Package contra los presupuestos
versionados. `npm run verify:experience` ejecuta ambos gates juntos; requiere
que esas evidencias ya existan y no abre la aplicación ni conserva datos.

La evidencia de producto de I8 usa un flujo separado para que el contrato visual
también se pruebe contra el ejecutable Tauri optimizado: `npm run
accessibility:release` compila sin bundle, captura desktop, móvil, escala 125% y
`forced-colors` mediante CDP de loopback y deja únicamente artefactos sanitizados
en `.local/validation/release-evidence/`. `npm run
accessibility:release:check` compara el resultado con
`fixtures/accessibility/release-evidence-baseline-v1.json`; una actualización
intencional exige inspección y el comando explícito
`npm run accessibility:release:update-baseline`. El baseline no sustituye la
auditoría manual con lector de pantalla ni la validación de hardware real.

Cada ejecución escribe un reporte JSON en `.local/validation/` con perfil, estado, tiempos, commit, rama, indicador de árbol sucio, sistema operativo, arquitectura y versiones de PowerShell, Node, npm, Rust y Cargo. También registra SHA-256 de `package-lock.json` y `src-tauri/Cargo.lock`, sin incluir rutas absolutas ni contenido; un lockfile ausente queda marcado como `unavailable`. El directorio es local y está ignorado por Git. Usa `-ReportPath <ruta>` para elegir otro destino; las rutas relativas se resuelven desde la raíz del proyecto. El reporte también se intenta escribir si falla una etapa, conservando el último resultado y su error. Todos los perfiles registran métricas raw/gzip del frontend; Release añade el SBOM y Package añade únicamente instaladores producidos o actualizados en esa ejecución.

`check-governance.ps1` comprueba la licencia MIT, el ADR de contratos, las
políticas de ramas/commits, el inventario de dependencias incluido en
`AUDITORIA.md` y el manifest de fixtures sintéticas. Estos contratos son
independientes de la cobertura manual de accesibilidad, los benchmarks grandes y
la automatización Win32 pendientes en fases posteriores.

`npm run smoke:installer` ejecuta el instalador NSIS real como usuario sin
privilegios en una ruta temporal con espacios y Unicode. Mide instalación y
primera apertura, comprueba que la segunda invocación respete la instancia única,
desinstala, verifica que los datos de usuario sobrevivan y elimina únicamente
su sentinel temporal. Este smoke no sustituye una VM Windows limpia ni el
ejercicio de actualización contra un canal publicado.

Para probar además un upgrade local entre dos versiones, se puede pasar un
artefacto NSIS anterior explícito: `powershell -File
tools/smoke-installed-artifact.ps1 -InstallerPath
src-tauri/target/release/bundle/nsis/Columnia_0.57.0_x64-setup.exe
-PreviousInstallerPath <artefacto-anterior>`. El escenario instala la versión
anterior, instala la objetivo en la misma ruta, comprueba la versión del
ejecutable y verifica que el sentinel de datos sobreviva al upgrade y a la
desinstalación. No se ejecuta automáticamente en `Package` porque requiere
proporcionar un artefacto histórico compatible.

El presupuesto actual admite por archivo hasta 512 KiB raw/160 KiB gzip para JavaScript y 128 KiB raw/40 KiB gzip para CSS; el total JS+CSS no puede superar 800 KiB raw/240 KiB gzip. El límite raw subió de 768 KiB el 2026-09-23 por decisión explícita: el Tier 10 llevó el total a 788.352 bytes raw (194.082 gzip) y el margen previo era de 824 bytes; el gzip, que es lo que mide el coste real, sigue en el 81 % de su límite. El total verificado el 2026-09-23 antes de ese cambio fue 785.993 bytes raw y 193.427 gzip.

`npm run perf:benchmark` usa tres iteraciones sostenidas de transformaciones
CSV/Parquet y después mide `project-save`, `project-inspect`, `project-export`,
`project-list` y `project-delete` sobre un almacén temporal. Después actualiza dos
veces el mismo ID, inspecciona la reapertura durable y vuelve a exportar. La receta
cambia `amount` a `total`; las reglas, el perfil cacheado, el historial y el cleanup
se verifican antes de eliminar el almacén. El gate también limita la duración de
transformaciones, guardado, inspección y exportación. El resumen nunca conserva el
ID del proyecto, filas, rutas ni contenido de los archivos.

Las pruebas frontend verifican además que `package-lock.json` refleje exactamente la versión y las dependencias raíz de `package.json`, y que el paquete local de `Cargo.lock` coincida con `Cargo.toml`. No requieren red ni reescriben lockfiles.

Los gates estáticos verifican que la CSP de producción permanezca local, que desarrollo solo añada el servidor loopback configurado y que la ventana `main` conserve exclusivamente `core:default`, sin permisos de filesystem, shell, HTTP u opener.

Los gates de supply chain rechazan paquetes npm sin SRI fuerte o fuera del registro oficial, crates sin checksum o fuera de crates.io, fuentes Git e identidades contradictorias. Release genera el SBOM sin red, timestamps, UUID, rutas locales ni URLs de descarga.

La validación del 2026-08-28 registra 248 pruebas frontend, 234 pruebas Rust y 9
E2E; las ramas específicas de symlinks/reparse points dependen de la plataforma.
Son una fotografía orientativa ligada a `137520b`, no un umbral permanente.

## Riesgos y deuda técnica visibles

1. **Motor monolítico**: `dataset.rs` concentra casi todo el dominio. Un cambio puede afectar carga, receta, historial y exportación; usa CodeGraph y ejecuta pruebas Rust completas.
2. **Editor de recetas amplio**: las cuatro fases viven en módulos feature y el estado de Revisar y Entregar está en `useReviewController` y `useDeliveryController` (T10-23 y T10-24). `App.tsx` conserva la carga, la navegación y la coordinación entre fases. `TransformRecipeEditor.tsx` reúne muchos subdominios de receta. Cualquier división futura debe preservar el orden, dependencias y confirmaciones destructivas.
3. **Contratos duplicados con gate**: Rust y TypeScript todavía declaran contratos por separado, pero 69 estructuras tienen comparación automática de campos y tipos. Al añadir una estructura compartida nueva, debe incorporarse explícitamente a las listas del gate IPC.
4. **Memoria**: los datasets no tienen un tope fijo de tamaño. Polars materializa el dataset y algunas operaciones crean candidatos completos, por lo que la capacidad efectiva depende de la RAM, el espacio disponible y los demás recursos del equipo.
5. **Consumo de disco durable**: cada proyecto puede conservar generaciones e historial Parquet de hasta 12 revisiones/1 GiB; los límites por proyecto no forman un presupuesto global para todos los proyectos.
6. **Cobertura de plataforma**: arranque y empaquetado están verificados en Windows; macOS y Linux aún requieren validación local real.
7. **Sin CI por política**: la calidad depende de ejecutar y registrar correctamente los gates locales.
8. **Gates de publicación condicionados**: cobertura por capa, primer render,
   `SkipPackage`, zoom 125% y evidencia release ya tienen contratos ejecutables;
   la evidencia release aún exige un commit limpio y la revisión legal final.
9. **Rendimiento con alcance acotado**: el benchmark durable de 100 MiB y tres
    ciclos WebView2 cumplen sus presupuestos actuales, pero la capacidad global
    para datasets mayores y la interacción nativa requieren validación adicional.
10. **Persistencia recuperable en evolución**: los reintentos de inicialización y
    la reconciliación de generaciones ya están implementados; deben conservarse
    las pruebas de fallo y el margen de seguridad al ampliar el esquema.
11. **Distribución no promovible aún**: Package, notices, updater firmado y el
    orquestador local existen; instalación en VM, canal, rotación de clave y
    decisiones legales siguen abiertos.
12. **Estado envenenable** (2026-09-23, mitigado en T10-15): los mutex de estado
    se toman con `lock_recovering()` (`crash_report.rs`), que recupera el último
    valor publicado tras un pánico, y un hook escribe en `crash-reports/` un
    informe mínimo (versión, fecha, archivo:línea; nunca el mensaje). Decisión:
    se prefiere recuperar el último estado publicado a bloquear la sesión, porque
    las operaciones publican de forma atómica; la alternativa descartada era
    invalidar el dataset activo y perder el trabajo en memoria.

## Cómo trabajar en este repositorio

1. Comprueba `git status` y conserva cambios ajenos.
2. Si existe `.codegraph/`, usa primero `codegraph explore "<pregunta o símbolos>"` para localizar código, llamadas y radio de impacto.
3. Lee la skill pertinente en `.agents/skills/<skill>/SKILL.md` antes de aplicarla.
4. Traza el cambio desde `App.tsx` hacia `bridge.ts`, `lib.rs` y `dataset.rs` cuando cruce IPC.
5. Mantén las rutas y datos sensibles exclusivamente en Rust.
6. Añade o actualiza pruebas en la capa donde vive el comportamiento.
7. Ejecuta al menos el perfil Fast; usa Full para cambios Rust o de contratos y Release para trabajo de distribución.
8. Actualiza este archivo si el cambio altera arquitectura, contratos, comandos, límites, invariantes o forma de validar.

## Protocolo para mantener vivo `CONTEXTO.md`

Actualiza el documento en el mismo cambio cuando ocurra cualquiera de estos eventos:

- se agrega, elimina o renombra un comando Tauri;
- cambia un formato, límite, operación, regla de calidad o garantía de atomicidad;
- se incorpora una dependencia arquitectónica;
- cambia la persistencia, seguridad, permisos, CSP, red o manejo de rutas;
- cambia la forma oficial de ejecutar, probar, empaquetar o publicar;
- se toma una decisión duradera que condicionará trabajo futuro.

Al actualizarlo:

1. Verifica primero el comportamiento en código y pruebas.
2. Describe cómo es el sistema, no cómo llegó a serlo: sin registro de sesiones,
   cifras de pruebas ni estados de tareas. Lo entregado va en `CHANGELOG.md` y lo
   pendiente en `ROADMAP.md`.
3. Registra una decisión duradera con una frase concreta junto a la sección que
   afecta, y enlaza el commit o ADR cuando exista.

## Documentos relacionados

- [AUDITORIA.md](AUDITORIA.md): ficha de dependencias y controles verificados por los gates.
- [README.md](README.md): visión funcional y uso actual.
- [THREAT_MODEL.md](THREAT_MODEL.md): fronteras de confianza, amenazas, controles y riesgos residuales.
- [ROADMAP.md](ROADMAP.md): pendientes vigentes y criterio de salida de V1.
- [CHANGELOG.md](CHANGELOG.md): lo entregado, versión a versión.
- [docs/archive/2026-09/](docs/archive/2026-09/CONTEXTO.md): este documento, el roadmap y las auditorías hasta el Tier 10, sin cambios.
- [CONTRIBUTING.md](CONTRIBUTING.md): ramas, commits y revisión local.
- [docs/README.md](docs/README.md): ADRs, gobierno, dependencias y fixtures.
- [LICENSE](LICENSE): licencia MIT del proyecto.
- [package.json](package.json): scripts y dependencias frontend.
- [src-tauri/Cargo.toml](src-tauri/Cargo.toml): dependencias del motor nativo.
- [src-tauri/tauri.conf.json](src-tauri/tauri.conf.json): configuración de escritorio y CSP.
