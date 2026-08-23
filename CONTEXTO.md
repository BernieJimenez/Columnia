# Contexto vivo de Columnia

> Punto de entrada técnico y operativo para personas y agentes que trabajen en este repositorio.
> Este archivo describe el código que existe hoy. `ROADMAP.md` describe también decisiones y trabajo futuro.

## Ficha rápida

| Campo | Estado verificado |
| --- | --- |
| Producto | Estación de escritorio local para revisar, limpiar, transformar y entregar datasets confiables |
| Versión | `0.36.0`, sincronizada en npm, Cargo y Tauri |
| Arquitectura implementada | Tauri 2 + Rust + Polars + React 19 + TypeScript + Vite |
| Plataformas objetivo | Windows, macOS y Linux |
| Plataforma verificada inicialmente | Windows |
| Persistencia actual | Proyectos SQLite con dataset, reglas, borrador, perfil cacheado e historial/cursor durables; cada apertura crea copias temporales de sesión |
| Red y servicios externos | No requeridos para trabajar con datos; la CSP de producción bloquea conexiones remotas |
| Validación | Local mediante `tools/check.ps1`; no hay CI por decisión del proyecto |
| Última revisión de este documento | 2026-08-22, rama `master`, base `4900599` con cambios locales pendientes |

## Para qué existe este documento

`CONTEXTO.md` reduce el tiempo necesario para entender el proyecto y evita decisiones basadas en información desactualizada. Debe responder rápidamente:

- qué problema resuelve Columnia;
- qué está implementado y qué está solamente planificado;
- dónde vive cada responsabilidad;
- cómo circulan los datos entre React y Rust;
- qué invariantes de privacidad, seguridad y calidad no deben romperse;
- cómo validar un cambio;
- qué contexto debe actualizarse después de una decisión o modificación importante.

### Fuentes de verdad

Usa esta prioridad cuando dos documentos parezcan contradecirse:

1. El código y las pruebas actuales definen el comportamiento implementado.
2. `CONTEXTO.md` resume el estado técnico y las reglas de trabajo vigentes.
3. `README.md` explica el producto y su uso actual.
4. `ROADMAP.md` registra decisiones históricas, arquitectura objetivo y trabajo pendiente.

No presentes como implementada una tecnología solo porque aparece en el roadmap. Por ejemplo, DuckDB forma parte de la arquitectura objetivo, pero no está entre las dependencias actuales de Cargo.

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

Los cinco comandos CLI de proyectos exigen siempre `--store <directorio>`: no infieren ni reutilizan el `app_data_dir` del escritorio. Rust canonicaliza ese almacén explícito y mantiene allí el catálogo y los artefactos administrados. `project-save` crea o actualiza por ID un snapshot materializado desde una entrada y puede adjuntar receta, reglas y perfil; `project-list` devuelve resúmenes; `project-inspect` devuelve metadatos, presencia de perfil/receta, cantidad de reglas y estado agregado del historial; `project-export` valida el proyecto completo sin activarlo ni alterar su candidato de recuperación; y `project-delete` exige que `--confirm` coincida exactamente con `--id`. Las reglas guardadas deben aprobar siempre: `--allow-unvalidated` habilita únicamente proyectos sin reglas y nunca omite una validación reprobada. La exportación publica CSV o Parquet atómicamente y el contrato informa solo nombre de archivo, tamaño, formato y conteos de calidad, no la ruta ni datos del dataset.

## Flujo de producto

La interfaz sigue cuatro fases declaradas en `src/App.tsx`:

1. **Cargar**: inspecciona una fuente local, permite seleccionar una hoja cuando corresponde y materializa el dataset activo.
2. **Revisar**: pagina la vista previa y calcula el perfil de calidad bajo demanda.
3. **Preparar**: aplica correcciones simples o una receta estructural atómica; ofrece Deshacer/Rehacer.
4. **Entregar**: valida un contrato de calidad y exporta CSV o Parquet.

Las fases distintas de Cargar se deshabilitan mientras no exista un dataset. Una operación activa bloquea la navegación que pueda competir con ella. Si el dataset cambia, cualquier validación de entrega previa queda obsoleta y debe ejecutarse de nuevo.

## Arquitectura por archivo

| Ruta | Responsabilidad |
| --- | --- |
| `src/main.tsx` | Monta `<App />` en modo estricto de React. |
| `src/App.tsx` | Coordina el flujo principal y los estados compartidos de la interfaz en unas 504 líneas. |
| `src/components/` | Componentes accesibles extraídos para diálogos, tabs de revisión y progreso cancelable. |
| `src/features/load/` | Fase Cargar: vista y modelo de inspección, selección de hojas, progreso, cancelación y recuperación. |
| `src/features/review/` | Fase Revisar: diagnóstico, perfil de calidad, tabs y vista previa paginada. |
| `src/features/prepare/` | Fase Preparar: vistas, editor de recetas, historial, modelo puro y controlador de IPC/invalidationes. |
| `src/features/projects/` | Catálogo, guardado, apertura, recuperación y eliminación accesible de proyectos locales. |
| `src/features/delivery/` | Fase Entregar: vista, métricas y modelo tipado de contrato, compuerta de calidad y exportación. |
| `src/bridge.ts` | Contrato TypeScript del IPC y única fachada de `invoke()` usada por la UI. |
| `src/styles.css` | Sistema visual y layout de la aplicación; incluye foco visible, targets mínimos y reducción de movimiento respetando `prefers-reduced-motion`. |
| `playwright.config.ts` | Configuración de Playwright para E2E del shell web Vite, con Chromium/Edge local, preview de producción reutilizable, trazas y artefactos solo en fallos. |
| `e2e/` | Pruebas E2E del shell web, primer render, accesibilidad, preferencias responsive y ciclo de proyectos con IPC Tauri simulado; la ventana WebView2 nativa tiene un probe CDP opcional. |
| `src-tauri/src/main.rs` | Entrada mínima del ejecutable; delega en `columnia_lib::run()`. |
| `src-tauri/src/lib.rs` | Inicializa Tauri, instancia única, diálogo nativo, estados de dataset/proyectos y los 25 comandos permitidos. |
| `src-tauri/src/dataset.rs` | Motor de datos completo. Contiene carga, tipos, perfiles, recetas, historial y exportación en unas 7,983 líneas. |
| `src-tauri/src/projects.rs` | Catálogo SQLite v3 compatible con v1/v2, snapshots Parquet durables, perfil e historial versionados y cinco comandos de proyectos. |
| `src-tauri/src/automation.rs` | Parser estricto, contratos JSON y orquestación reutilizable de datasets, lotes y los cinco comandos CLI de proyectos. |
| `src-tauri/src/bin/columnia-cli.rs` | Ejecutable CLI mínimo que delega en el módulo de automatización. |
| `src-tauri/capabilities/main.json` | Capability mínima para la ventana `main`: solamente `core:default`. |
| `src-tauri/tauri.conf.json` | Ventana, build, bundle y CSP de producción/desarrollo. |
| `tools/check.ps1` | Entrada única para los gates locales Fast, Full y Release; genera evidencia JSON auditable en `.local/validation/`. |
| `tools/generate-sbom.ps1` | Genera offline un SBOM CycloneDX 1.6 reproducible desde ambos lockfiles. |
| `tools/check-bundle.mjs` | Mide presupuestos JS/CSS e inventaría bundles de distribución nuevos o actualizados. |
| `tools/smoke-tauri.ps1` | Arranca `npm run tauri dev`, comprueba Vite y el ejecutable debug, registra hitos monotónicos de Vite/proceso/ventana, ejecuta un preflight de contrato de `ProjectsPanel` y limpia solo su Job Object con reintento acotado. |
| `tools/probe-webview2-cdp.ps1` | Arranca el comando real con `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` de loopback, verifica `/json/version` y `/json/list`, puede conectar Playwright al WebView2 y ejecutar probes DOM de solo lectura, y deja evidencia; restaura el entorno y limpia su Job Object. |
| `tools/probe-webview2-playwright.mjs` | Conecta al endpoint CDP con Playwright, espera el shell listo y registra primer render, landmarks, `ProjectsPanel`, skip link, foco principal y cantidad de controles sin mutar datos. |
| `tools/probe-webview2-projects.mjs` | Conecta al endpoint CDP y verifica el contrato accesible de solo lectura de `ProjectsPanel`: región, etiqueta, botón deshabilitado sin dataset, ausencia de rutas y nombres de acciones; nunca hace clic ni abre diálogos. |
| `tools/summarize-performance.ps1` | Lee únicamente `summary.json` dentro de `.local/validation/`, clasifica señales web/CDP/desktop, calcula deltas y genera `summary.json`/`summary.csv` sin rutas absolutas ni datos sensibles. |
| `tools/smoke-cli.ps1` | Verifica la CLI real con fixtures deterministas, libros, calidad, lotes, atomicidad por trabajo y errores seguros. |
| `fixtures/automation/` | Entradas, receta y resultados esperados del smoke de automatización. |
| `README.md` | Descripción funcional y guía de uso/desarrollo. |
| `THREAT_MODEL.md` | Activos, fronteras de confianza, amenazas, controles implementados y riesgos residuales. |
| `ROADMAP.md` | Plan, decisiones históricas, fases y pendientes. No sustituye la inspección del código. |
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

`LoadedDataset` conserva una ruta fuente privada opcional, el nombre/tamaño visibles, el `DataFrame`, un perfil opcional en caché y el historial. Separar la identidad visible de la ruta permite restaurar un snapshot aunque el archivo original ya no exista. El dataset se materializa actualmente en memoria. El límite provisional de archivo es 500 MiB, pero el consumo real puede ser mayor durante lectura, perfilado y transformaciones.

### Historial y atomicidad

Cada revisión reversible de la sesión se guarda como snapshot Parquet en un directorio temporal:

- máximo normal: 12 entradas;
- presupuesto total: 1 GiB;
- se elimina al cerrar o reemplazar la sesión;
- si un snapshot individual excede el presupuesto, el cambio puede aplicarse, pero la reversión se desactiva y se informa el motivo;
- una receta completa publica un solo candidato o no publica nada;
- `publish_candidate` prepara la vista previa y registra el historial antes de sustituir el `DataFrame` activo;
- la exportación escribe y sincroniza un temporal antes de reemplazar el destino.

Los proyectos guardan el frame materializado como una nueva generación Parquet y actualizan después el puntero SQLite dentro de una transacción. El esquema SQLite v3 conserva reglas de calidad, borrador opcional de receta, perfil cacheado y el historial con su cursor, y migra catálogos v1/v2 compatibles. El historial durable mantiene los mismos límites de 12 revisiones y 1 GiB. Abrir valida todos los artefactos antes de sustituir el dataset activo y copia el perfil e historial guardados a estructuras temporales de sesión; una corrupción hace fallar la apertura completa. La recuperación es explícita desde Cargar y no abre datos silenciosamente.

## Contrato React ↔ Rust

La superficie pública está centralizada en `src/bridge.ts` y registrada en `src-tauri/src/lib.rs`.

### Runtime y carga

- `get_app_info`
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
- `get_recovery_candidate`
- `save_project`
- `open_project`
- `delete_project`

Regla de mantenimiento: cualquier cambio de nombre, argumentos, serialización o respuesta en Rust debe reflejarse en `bridge.ts` y quedar cubierto por pruebas. `src/ipc-contract.test.ts` verifica automáticamente comandos registrados, argumentos serializados, tipos de retorno superiores, nombres de campos y tipos concretos de 42 estructuras compartidas. Normaliza referencias, números, `Vec`/arrays, `Option`/campos opcionales, herencia, literales y alias conocidos. Las 14 subestructuras de `TransformRecipe` tienen interfaces nominales equivalentes a Rust; los alias públicos históricos se conservan para no romper consumidores.

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
- agrupación con agregaciones tipadas;
- normalización de correos, teléfonos y direcciones;
- extracciones textuales Unicode;
- recetas JSON versión 1 guardables y cargables;
- historial multinivel Deshacer/Rehacer.

Las recetas se validan y ejecutan en orden determinista. Una entrada inválida, pérdida de precisión, división por cero o conflicto entre pasos revierte el lote completo.

### Entrega

- exportación atómica a CSV o Parquet;
- neutralización de fórmulas en columnas de texto al exportar CSV; Parquet conserva los valores originales;
- cancelación cooperativa;
- contratos de hasta 16 reglas: no nulo, texto no vacío, unicidad y rango numérico inclusivo;
- tolerancia por cantidad y/o porcentaje;
- resultados con conteos, nunca muestras de celdas;
- confirmación explícita para exportar sin reglas.

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
.\tools\check.ps1 -Profile Fast
.\tools\check.ps1 -Profile Full
.\tools\check.ps1 -Profile Release
.\tools\check.ps1 -Profile Package
npm run smoke:desktop -- -TimeoutSeconds 120
npm run smoke:cli
npm run smoke:cdp
npm run perf:summary
```

| Perfil | Incluye |
| --- | --- |
| Fast | `cargo fmt --check`, `cargo check`, Vitest, build TypeScript/Vite y presupuesto frontend |
| Full | Fast + Clippy con warnings como errores + pruebas Rust de biblioteca |
| Release | Full + SBOM CycloneDX reproducible + build Tauri optimizado sin bundle |
| Package | Release + MSI/NSIS en Windows + inventario diferencial con tamaño y SHA-256 |

Cada ejecución escribe un reporte JSON en `.local/validation/` con perfil, estado, tiempos, commit, rama, indicador de árbol sucio, sistema operativo, arquitectura y versiones de PowerShell, Node, npm, Rust y Cargo. También registra SHA-256 de `package-lock.json` y `src-tauri/Cargo.lock`, sin incluir rutas absolutas ni contenido; un lockfile ausente queda marcado como `unavailable`. El directorio es local y está ignorado por Git. Usa `-ReportPath <ruta>` para elegir otro destino; las rutas relativas se resuelven desde la raíz del proyecto. El reporte también se intenta escribir si falla una etapa, conservando el último resultado y su error. Todos los perfiles registran métricas raw/gzip del frontend; Release añade el SBOM y Package añade únicamente instaladores producidos o actualizados en esa ejecución.

El presupuesto actual admite por archivo hasta 512 KiB raw/160 KiB gzip para JavaScript y 128 KiB raw/40 KiB gzip para CSS; el total JS+CSS no puede superar 768 KiB raw/240 KiB gzip. El baseline verificado es aproximadamente 301 KiB raw y 86 KiB gzip.

Las pruebas frontend verifican además que `package-lock.json` refleje exactamente la versión y las dependencias raíz de `package.json`, y que el paquete local de `Cargo.lock` coincida con `Cargo.toml`. No requieren red ni reescriben lockfiles.

Los gates estáticos verifican que la CSP de producción permanezca local, que desarrollo solo añada el servidor loopback configurado y que la ventana `main` conserve exclusivamente `core:default`, sin permisos de filesystem, shell, HTTP u opener.

Los gates de supply chain rechazan paquetes npm sin SRI fuerte o fuera del registro oficial, crates sin checksum o fuera de crates.io, fuentes Git e identidades contradictorias. Release genera el SBOM sin red, timestamps, UUID, rutas locales ni URLs de descarga.

Al revisar este documento había 129 pruebas frontend y 127 pruebas Rust; las ramas específicas de symlinks/reparse points dependen de la plataforma. Son una fotografía orientativa, no un umbral: actualiza el número si cambia de forma material o elimina el conteo si deja de ser útil.

## Estado real frente a arquitectura objetivo

### Implementado ahora

- Shell Tauri, frontend React y motor Rust/Polars.
- Instancia única en escritorio: una segunda apertura muestra, desminimiza y enfoca la ventana `main` existente.
- Flujo Cargar → Revisar → Preparar → Entregar.
- Formatos, perfiles, transformaciones, historial de sesión, contratos y exportación descritos arriba.
- CSP restrictiva, capability mínima y validación local centralizada.
- Threat model vivo y gates de regresión para CSP, permisos, payloads semánticos y fórmulas CSV.
- Navegación por teclado inicial con skip link, pestañas ARIA, foco visible, regiones anunciables y diálogos con ciclo/restauración de foco.
- SBOM CycloneDX 1.6 reproducible y gates offline de integridad/procedencia para npm y Cargo.
- Smoke automatizado del runtime de desarrollo con aislamiento y cleanup de procesos propios.
- Presupuestos medibles del frontend y empaquetado Windows verificado en MSI/NSIS con evidencia criptográfica.
- CLI local con `inspect`, `transform` y `validate`, contratos JSON versionados y reutilización del motor, libros, reglas, recetas y exportación atómica del escritorio.
- Smoke CLI determinista que cubre CSV, Parquet, XLSX, dos modos de encabezado, calidad aprobada/reprobada, neutralización de fórmulas y fallos sin outputs parciales.
- Fases Cargar y Revisar extraídas de `App.tsx` a módulos con transiciones tipadas y pruebas propias.
- Fase Entregar extraída de `App.tsx` a un módulo con estados discriminados y pruebas propias.
- Fase Preparar extraída a vistas, editor, historial, modelo y controlador; `App.tsx` queda como coordinador de las cuatro fases.
- CLI batch v1 para 1–64 transformaciones, con preflight sin escrituras, colisiones rechazadas y atomicidad individual explícita.
- Proyectos locales con catálogo SQLite v3 compatible con v1/v2, snapshots Parquet durables, reglas de calidad, borrador opcional, perfil cacheado, historial/cursor y recuperación explícita aunque desaparezca la fuente original.
- CLI de proyectos con almacén `--store` explícito, guardado/listado/inspección/exportación/borrado, contratos JSON v1 privados, compuerta de calidad y confirmación destructiva exacta.
- Integración frontend del ciclo guardar/abrir/eliminar: Vitest cubre el guardado, la confirmación/cancelación destructiva y la conservación del dataset activo; el smoke de escritorio valida el contrato de `ProjectsPanel` y el arranque de la ventana/WebView2.
- Accesibilidad WCAG 2.2 de bajo riesgo: targets interactivos mínimos de 24 px, reducción global de movimiento y prueba de regresión CSS para ambos contratos.
- Baseline local de rendimiento medido: Vite listo en 278–283 ms, Cargo debug en 0.86–0.91 s y startup total del smoke en 6.33–6.98 s, con mediana aproximada de 6.71 s; bundle v0.36.0 verificado en 314,827 bytes raw/90,154 gzip.
- Contratos automatizados de accesibilidad para landmarks, skip link, `aria-current`, `aria-busy`, acciones de proyectos y `alertdialog` modal.
- Smoke desktop instrumentado con hitos: la ejecución fría v0.30.0 registró Vite en 4,307 ms, proceso debug en 71,321 ms, ventana visible en 71,337 ms y cleanup confirmado en 1,133 ms/1 intento; el cleanup admite un segundo intento de 3 s tras uno inicial de 4 s.
- Playwright configurado con preview Vite y E2E del shell web para landmarks, runtime, navegación accesible, skip link y primer render; conserva trazas/capturas/vídeos solo cuando una prueba falla.
- Playwright con mock de `__TAURI_INTERNALS__` cubre cargar dataset, guardar, abrir y eliminar proyectos con confirmación, sin filesystem ni datos reales.
- `App` publica la marca `columnia:app-render`; Playwright verifica el primer render del shell por debajo de 3 segundos en el preview local.
- Playwright añade cobertura E2E de landmarks, foco visible, targets mínimos y ciclo de foco/restauración del `alertdialog`.
- En Windows, `npm run smoke:cdp` verifica un endpoint CDP de loopback de WebView2 y usa `chromium.connectOverCDP` para medir primer render, landmarks, skip link y foco principal, además del contrato de solo lectura de `ProjectsPanel`, sin mutar datos.
- Playwright añade cobertura E2E responsive de viewport móvil/desktop, `prefers-reduced-motion`, targets mínimos y ausencia de overflow horizontal.
- La última medición CDP nativa registró `columnia:app-render` en 40,757.8 ms durante un arranque debug frío; el dato queda como señal de rendimiento y no bloquea los contratos de landmarks/foco. El E2E web conserva el gate estricto de 3 s.
- Validación v0.36.0 completada en Windows: Vitest 129/129, Rust 127/127, Playwright 9/9, probe CDP combinado con `ProjectsPanel` y cleanup confirmado, smoke desktop, CLI, `npm run perf:summary` y Package aprobados. Evidencias: CDP `.local/validation/webview2-cdp/20260822T212619Z`, desktop `.local/validation/desktop-smoke/20260822T212231Z`, CLI `.local/validation/cli-smoke/20260822T212240Z`, Package `.local/validation/20260822T212259Z-4900599-package.json` y resumen `.local/validation/performance-summary/summary.json` (10 muestras CDP, 13 desktop, 52 filas CSV; shell web todavía no observado por este agregador).

### Planeado o pendiente

- ejecución lazy/incremental y datasets mayores que la memoria;
- DuckDB embebido;
- joins, comparación de datasets y destinos de bases de datos;
- flujos IPC nativos de proyectos dentro de WebView2, auditoría manual con lector de pantalla/zoom/alto contraste y pruebas visuales; el probe CDP verifica ahora el contrato accesible de solo lectura, pero no reemplaza todavía la interacción de negocio;
- acciones completas de proyectos sobre CDP; el objetivo operativo provisional sigue siendo startup total menor de 8 s y cleanup 100 % repetible;
- escaneo de vulnerabilidades, firma de instaladores y updater autenticado; SBOM, gates offline y empaquetado Windows básico ya existen;
- verificación real en macOS y Linux.

Consulta `ROADMAP.md` para el detalle, pero verifica cada casilla contra el código antes de afirmar que una fase está completa.

## Riesgos y deuda técnica visibles

1. **Motor monolítico**: `dataset.rs` concentra casi todo el dominio. Un cambio puede afectar carga, receta, historial y exportación; usa CodeGraph y ejecuta pruebas Rust completas.
2. **Editor de recetas amplio**: las cuatro fases ya viven en módulos feature y `App.tsx` es un coordinador pequeño, pero `TransformRecipeEditor.tsx` reúne muchos subdominios de receta. Cualquier división futura debe preservar el orden, dependencias y confirmaciones destructivas.
3. **Contratos duplicados con gate**: Rust y TypeScript todavía declaran contratos por separado, pero 42 estructuras tienen comparación automática de campos y tipos. Al añadir una estructura compartida nueva, debe incorporarse explícitamente a las listas del gate IPC.
4. **Memoria**: el límite de 500 MiB no equivale a un presupuesto de RAM. Polars materializa el dataset y algunas operaciones crean candidatos completos.
5. **Consumo de disco durable**: cada proyecto puede conservar generaciones e historial Parquet de hasta 12 revisiones/1 GiB; los límites por proyecto no forman un presupuesto global para todos los proyectos.
6. **Cobertura de plataforma**: arranque y empaquetado están verificados en Windows; macOS y Linux aún requieren validación local real.
7. **Roadmap acumulativo**: contiene decisiones propuestas, aprobadas e implementadas; no todas reflejan dependencias presentes.
8. **Sin CI por política**: la calidad depende de ejecutar y registrar correctamente los gates locales.

## Cómo trabajar en este repositorio

1. Comprueba `git status` y conserva cambios ajenos.
2. Si existe `.codegraph/`, usa primero `codegraph explore "<pregunta o símbolos>"` para localizar código, llamadas y radio de impacto.
3. Lee la skill pertinente en `.agents/skills/<skill>/SKILL.md` antes de aplicarla.
4. Traza el cambio desde `App.tsx` hacia `bridge.ts`, `lib.rs` y `dataset.rs` cuando cruce IPC.
5. Mantén las rutas y datos sensibles exclusivamente en Rust.
6. Añade o actualiza pruebas en la capa donde vive el comportamiento.
7. Ejecuta al menos el perfil Fast; usa Full para cambios Rust o de contratos y Release para trabajo de distribución.
8. Actualiza este archivo si el cambio altera arquitectura, contratos, comandos, límites, invariantes, estado del roadmap o forma de validar.

## Protocolo para mantener vivo `CONTEXTO.md`

Actualiza el documento en el mismo cambio cuando ocurra cualquiera de estos eventos:

- se agrega, elimina o renombra un comando Tauri;
- cambia un formato, límite, operación, regla de calidad o garantía de atomicidad;
- se incorpora una dependencia arquitectónica como DuckDB o SQLite;
- cambia la persistencia, seguridad, permisos, CSP, red o manejo de rutas;
- se completa un pendiente listado aquí;
- cambia la forma oficial de ejecutar, probar, empaquetar o publicar;
- se toma una decisión duradera que condicionará trabajo futuro.

Al actualizarlo:

1. Verifica primero el comportamiento en código y pruebas.
2. Cambia la fecha y el commit base de la ficha rápida.
3. Mueve elementos entre “Planeado” e “Implementado”; no los dupliques.
4. Actualiza el mapa de archivos si cambia la propiedad de una responsabilidad.
5. Registra decisiones duraderas abajo con una frase concreta y un enlace al PR, commit o ADR cuando exista.
6. Evita convertir este archivo en changelog. Conserva solo contexto que ayude a la siguiente decisión.

## Registro de contexto

| Fecha | Cambio de contexto | Evidencia |
| --- | --- | --- |
| 2026-08-22 | El smoke desktop separa hitos monotónicos de Vite, proceso debug y ventana visible, y confirma cleanup con hasta dos intentos acotados; se añadieron contratos de landmarks, estados ARIA y alertdialog. | `tools/smoke-tauri.ps1`, `src/components/AccessibilityContracts.test.tsx` |
| 2026-08-22 | Playwright 1.62 quedó configurado contra un preview Vite local (Edge en Windows, Chromium en otros sistemas) y E2E del shell web; los artefactos de diagnóstico quedan ignorados y la cobertura nativa Tauri/IPC sigue pendiente. | `playwright.config.ts`, `e2e/shell.spec.ts`, `package.json` |
| 2026-08-22 | El E2E añade un mock aislado de `__TAURI_INTERNALS__` para recorrer cargar, guardar, abrir y eliminar proyectos sin tocar filesystem; la ventana WebView2 real sigue fuera de alcance. | `e2e/projects.spec.ts` |
| 2026-08-22 | El shell marca `columnia:app-render` y Playwright comprueba el primer render en menos de 3 s sobre el preview local; la medición nativa queda pendiente de CDP. | `src/App.tsx`, `e2e/performance.spec.ts` |
| 2026-08-22 | El probe Windows levanta `npm run tauri dev` con CDP de loopback aislado, verifica endpoints y conecta Playwright para lectura DOM; no ejecuta mutaciones IPC y restaura la variable de entorno al limpiar. | `tools/probe-webview2-cdp.ps1`, `tools/probe-webview2-playwright.mjs` |
| 2026-08-22 | Playwright cubre landmarks, foco visible, targets mínimos y el ciclo de foco/restauración del diálogo destructivo en el shell web. | `e2e/accessibility.spec.ts` |
| 2026-08-22 | El probe CDP mide primer render nativo con `columnia:app-render` bajo 3 s y verifica skip link → foco de `main`; el E2E responsive cubre reduced-motion y viewports sin overflow horizontal. | `tools/probe-webview2-playwright.mjs`, `e2e/preferences.spec.ts`, `src/styles.css` |
| 2026-08-22 | El smoke de escritorio valida el contrato estático de `ProjectsPanel` y confirma que la ventana debug/WebView2 inició; no simula clics porque UI Automation solo expone paneles del WebView2 y no el DOM React sin ampliar la configuración de depuración. | `tools/smoke-tauri.ps1`, `src/features/projects/ProjectsPanel.tsx` |
| 2026-08-22 | El smoke CDP combinado añade una lectura DOM de solo lectura del contrato de `ProjectsPanel` y un resumen local de rendimiento con deltas; las acciones que abren diálogos o escriben proyectos siguen fuera del probe. | `tools/probe-webview2-projects.mjs`, `tools/probe-webview2-cdp.ps1`, `tools/summarize-performance.ps1` |
| 2026-08-21 | La CLI administra proyectos en un `--store` obligatorio y canonicalizado mediante cinco comandos; exportar respeta las reglas guardadas y borrar exige confirmar el ID exacto, sin exponer rutas ni muestras en JSON. | `src-tauri/src/automation.rs`, `src-tauri/src/projects.rs`, `README.md`, `THREAT_MODEL.md` |
| 2026-08-21 | SQLite v3 migra catálogos v1/v2 y conserva perfil cacheado e historial/cursor; abrir valida todo y crea una copia temporal de sesión, manteniendo 12 revisiones/1 GiB. | `src-tauri/src/projects.rs`, `src-tauri/src/dataset.rs`, `src/features/projects/` |
| 2026-08-21 | El esquema SQLite v2 conserva reglas de calidad y borrador opcional de receta en cada proyecto, migra catálogos v1 y mantiene perfil e historial como estado temporal. | `src-tauri/src/projects.rs`, `src-tauri/src/dataset.rs`, `src/features/projects/` |
| 2026-08-21 | Proyectos v1 persisten un catálogo SQLite y generaciones Parquet privadas; guardado, apertura, recuperación y borrado no exponen rutas a React. | `src-tauri/src/projects.rs`, `src/features/projects/`, `src/bridge.ts` |
| 2026-08-21 | `LoadedDataset` separa identidad visible y ruta fuente opcional para que un proyecto siga funcionando después de borrar la fuente original. | `src-tauri/src/dataset.rs`, `src-tauri/src/projects.rs` |
| 2026-08-21 | La CLI ejecuta manifiestos batch v1 de hasta 64 trabajos, con preflight completo, outputs atómicos individuales y fallo parcial explícito por ordinal. | `src-tauri/src/automation.rs`, `tools/smoke-cli.ps1`, `fixtures/automation/` |
| 2026-08-21 | Preparar e Historial se extrajeron a vistas, modelo y controlador IPC; `App.tsx` bajó de 1,645 a 455 líneas en ese corte y hoy ronda 504 tras integrar proyectos. | `src/features/prepare/`, `src/features/projects/`, `src/App.tsx` |
| 2026-08-21 | La CLI admite libros mediante hoja exacta y encabezado explícito, y valida contratos de calidad con salida JSON de conteos y códigos 0/2/1. | `src-tauri/src/automation.rs`, `src-tauri/src/dataset.rs`, `tools/smoke-cli.ps1` |
| 2026-08-21 | Cargar y Revisar se extrajeron a módulos tipados y probados; `App.tsx` se redujo en otras 520 líneas. | `src/features/load/`, `src/features/review/`, `src/App.tsx` |
| 2026-08-21 | La automatización local incorpora `columnia-cli inspect/transform` sobre el mismo motor Rust, con contratos JSON v1, rutas no expuestas y exportación atómica. | `src-tauri/src/automation.rs`, `src-tauri/src/bin/columnia-cli.rs` |
| 2026-08-21 | Un smoke con fixtures deterministas valida CSV, Parquet, neutralización de fórmulas y errores sin outputs parciales. | `tools/smoke-cli.ps1`, `fixtures/automation/` |
| 2026-08-21 | La fase Entregar se extrajo a un módulo tipado y probado; `App.tsx` se redujo en 302 líneas. | `src/features/delivery/`, `src/App.tsx` |
| 2026-08-20 | Fast/Full/Release/Package aplican presupuestos JS/CSS; Package produjo MSI y NSIS y registró tamaño/hash sin atribuir bundles viejos. | `tools/check.ps1`, `tools/check-bundle.mjs`, `src-tauri/tauri.conf.json` |
| 2026-08-20 | `npm run smoke:desktop` verifica el comando real de desarrollo y termina únicamente procesos propios mediante un Job Object de Windows. | `package.json`, `tools/smoke-tauri.ps1` |
| 2026-08-20 | Diálogo, tabs y progreso se extrajeron como componentes accesibles con pruebas unitarias; `App.tsx` se redujo en 136 líneas. | `src/components/`, `src/App.tsx` |
| 2026-08-20 | Release genera un SBOM CycloneDX 1.6 reproducible y reporta su hash/cantidad; gates offline exigen procedencia e integridad de npm y Cargo. | `tools/generate-sbom.ps1`, `tools/check.ps1`, `src/supply-chain.test.ts` |
| 2026-08-20 | La UI incorpora una primera base WCAG verificable para landmarks, tabs, estados, foco y diálogos; queda pendiente validación manual con tecnologías de asistencia. | `src/App.tsx`, `src/styles.css`, `src/App.test.tsx` |
| 2026-08-20 | La exportación CSV neutraliza fórmulas únicamente en texto; los tipos no textuales y Parquet conservan sus valores. | `src-tauri/src/dataset.rs` |
| 2026-08-20 | Rust aplica presupuestos semánticos explícitos a recetas y reglas de calidad en todos sus comandos de entrada. | `src-tauri/src/dataset.rs` |
| 2026-08-20 | Se incorporó un threat model vivo y gates que impiden ampliar silenciosamente CSP o capabilities. | `THREAT_MODEL.md`, `src/tauri-assets.test.ts` |
| 2026-08-20 | El escritorio usa el plugin oficial de instancia única; una segunda apertura restaura y enfoca `main` sin ampliar capabilities. | `src-tauri/src/lib.rs`, `src-tauri/Cargo.toml` |
| 2026-08-20 | `TransformRecipe` y sus 14 subestructuras tienen contratos TypeScript nominales; 39 estructuras comparan campos y tipos con Rust. | `src/bridge.ts`, `src/ipc-contract.test.ts` |
| 2026-08-20 | Los gates locales comprueban que ambos lockfiles representen las versiones y dependencias raíz de sus manifiestos. | `src/version-sync.test.ts` |
| 2026-08-20 | Los reportes locales incluyen OS, arquitectura y SHA-256 de los lockfiles sin registrar rutas ni contenido. | `tools/check.ps1` |
| 2026-08-20 | Windows rechaza cualquier reparse point en fuentes y destinos, incluidos enlaces válidos y colgantes. | `src-tauri/src/dataset.rs` |
| 2026-08-20 | Se añadió la primera comparación de tipos concretos para 24 contratos IPC; posteriormente se amplió a `TransformRecipe` y sus subestructuras, como registra la entrada superior. | `src/ipc-contract.test.ts` |
| 2026-08-20 | Las rutas de datasets, recetas y exportaciones se canonicalizan en Rust; fuentes y destinos no regulares o simbólicos se rechazan antes de operar. | `src-tauri/src/dataset.rs` |
| 2026-08-20 | El gate IPC compara los campos de 25 estructuras compartidas y encontró/corrigió la ausencia de `TransformRecipeResult.changed` en TypeScript. | `src/ipc-contract.test.ts`, `src/bridge.ts` |
| 2026-08-20 | El gate IPC compara los tipos de retorno Rust con los genéricos `invoke<T>` y normaliza `Result`, `Option`, `void` y los alias de recetas conocidos. | `src/ipc-contract.test.ts` |
| 2026-08-20 | Los perfiles de `tools/check.ps1` generan evidencia JSON local con entorno, commit, tiempos y resultados por etapa. | `tools/check.ps1`, `.local/validation/` |
| 2026-08-20 | El gate IPC ahora compara también los argumentos serializados, excluyendo las inyecciones internas `AppHandle` y `State`; los tipos y respuestas siguen pendientes. | `src/ipc-contract.test.ts` |
| 2026-08-20 | La primera versión del gate IPC comparó todos los nombres registrados con las llamadas de `bridge.ts`; en ese corte todavía no comparaba argumentos. | `src/ipc-contract.test.ts` |
| 2026-08-20 | Se creó este documento vivo a partir del código, las pruebas, `README.md`, `ROADMAP.md` y el índice CodeGraph. | Estado de `master` en `8fdcb3d` |

## Documentos relacionados

- [README.md](README.md): visión funcional y uso actual.
- [THREAT_MODEL.md](THREAT_MODEL.md): fronteras de confianza, amenazas, controles y riesgos residuales.
- [ROADMAP.md](ROADMAP.md): planificación, decisiones históricas y pendientes.
- [package.json](package.json): scripts y dependencias frontend.
- [src-tauri/Cargo.toml](src-tauri/Cargo.toml): dependencias del motor nativo.
- [src-tauri/tauri.conf.json](src-tauri/tauri.conf.json): configuración de escritorio y CSP.
