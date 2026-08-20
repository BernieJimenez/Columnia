# Contexto vivo de Columnia

> Punto de entrada técnico y operativo para personas y agentes que trabajen en este repositorio.
> Este archivo describe el código que existe hoy. `ROADMAP.md` describe también decisiones y trabajo futuro.

## Ficha rápida

| Campo | Estado verificado |
| --- | --- |
| Producto | Estación de escritorio local para revisar, limpiar, transformar y entregar datasets confiables |
| Versión | `0.24.0`, sincronizada en npm, Cargo y Tauri |
| Arquitectura implementada | Tauri 2 + Rust + Polars + React 19 + TypeScript + Vite |
| Plataformas objetivo | Windows, macOS y Linux |
| Plataforma verificada inicialmente | Windows |
| Persistencia actual | Dataset y perfil en memoria; historial en snapshots Parquet temporales |
| Red y servicios externos | No requeridos para trabajar con datos; la CSP de producción bloquea conexiones remotas |
| Validación | Local mediante `tools/check.ps1`; no hay CI por decisión del proyecto |
| Última revisión de este documento | 2026-08-20, rama `master`, commit base `8fdcb3d` |

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

No presentes como implementada una tecnología solo porque aparece en el roadmap. Por ejemplo, DuckDB y SQLite forman parte de la arquitectura objetivo, pero no están entre las dependencias actuales de Cargo.

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
```

No existe un servidor HTTP de aplicación. React pide casos de uso concretos mediante IPC de Tauri. Rust conserva la autoridad sobre rutas, archivos y datasets. El frontend recibe nombres, metadatos, filas de vista previa e identificadores opacos, no rutas locales.

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
| `src/App.tsx` | Flujo completo de interfaz, estados, formularios y coordinación de casos de uso. Es actualmente un archivo grande de unas 2,491 líneas. |
| `src/bridge.ts` | Contrato TypeScript del IPC y única fachada de `invoke()` usada por la UI. |
| `src/styles.css` | Sistema visual y layout de la aplicación. |
| `src-tauri/src/main.rs` | Entrada mínima del ejecutable; delega en `columnia_lib::run()`. |
| `src-tauri/src/lib.rs` | Inicializa Tauri, el diálogo nativo, `DatasetState` y los 20 comandos permitidos. |
| `src-tauri/src/dataset.rs` | Motor de datos completo. Contiene carga, tipos, perfiles, recetas, historial y exportación en unas 7,983 líneas. |
| `src-tauri/capabilities/main.json` | Capability mínima para la ventana `main`: solamente `core:default`. |
| `src-tauri/tauri.conf.json` | Ventana, build, bundle y CSP de producción/desarrollo. |
| `tools/check.ps1` | Entrada única para los gates locales Fast, Full y Release; genera evidencia JSON auditable en `.local/validation/`. |
| `README.md` | Descripción funcional y guía de uso/desarrollo. |
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

`LoadedDataset` conserva la ruta privada, el `DataFrame`, un perfil opcional en caché y el historial. El dataset se materializa actualmente en memoria. El límite provisional de archivo es 500 MiB, pero el consumo real puede ser mayor durante lectura, perfilado y transformaciones.

### Historial y atomicidad

Cada revisión reversible se guarda como snapshot Parquet en un directorio temporal:

- máximo normal: 12 entradas;
- presupuesto total: 1 GiB;
- se elimina al cerrar o reemplazar la sesión;
- si un snapshot individual excede el presupuesto, el cambio puede aplicarse, pero la reversión se desactiva y se informa el motivo;
- una receta completa publica un solo candidato o no publica nada;
- `publish_candidate` prepara la vista previa y registra el historial antes de sustituir el `DataFrame` activo;
- la exportación escribe y sincroniza un temporal antes de reemplazar el destino.

No hay todavía proyectos persistentes, SQLite, recuperación de sesión ni historial durable entre aperturas.

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

Regla de mantenimiento: cualquier cambio de nombre, argumentos, serialización o respuesta en Rust debe reflejarse en `bridge.ts` y quedar cubierto por pruebas. `src/ipc-contract.test.ts` verifica automáticamente que los comandos registrados en `tauri::generate_handler!`, sus argumentos serializados y sus tipos de retorno superiores coincidan con la fachada; reconoce `AppHandle` y `State` como inyecciones internas de Tauri, normaliza `snake_case` a `camelCase` y resuelve los alias de receta conocidos. Los tipos de cada argumento y la estructura interna de objetos y respuestas todavía se mantienen manualmente.

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
- La ventana principal conserva permisos mínimos; no habilites filesystem, shell, HTTP u opener por comodidad.
- La CSP de producción no permite CDN, navegación remota, objetos, frames ni conexiones web externas.
- Los errores y contratos de calidad no deben filtrar muestras de datos.
- Un fallo o cancelación de exportación no debe destruir un archivo previo.
- Una transformación compuesta debe ser atómica.
- No añadas CI, GitHub Actions, telemetría o servicios de pago como requisito sin revertir expresamente las decisiones vigentes.

Pendientes de seguridad ya reconocidos en el roadmap: canonicalización exhaustiva de rutas, pruebas negativas de traversal/symlinks/fórmulas/payloads grandes, threat model actualizado e instancia única.

## Desarrollo y validación

### Arranque local

```powershell
npm install
npm run tauri dev
```

`npm run dev` abre solamente el frontend Vite. En ese modo la interfaz muestra que el motor Rust no está conectado; los casos de uso de datos requieren Tauri.

### Gates locales

```powershell
.\tools\check.ps1 -Profile Fast
.\tools\check.ps1 -Profile Full
.\tools\check.ps1 -Profile Release
```

| Perfil | Incluye |
| --- | --- |
| Fast | `cargo fmt --check`, `cargo check`, Vitest y build TypeScript/Vite |
| Full | Fast + Clippy con warnings como errores + pruebas Rust de biblioteca |
| Release | Full + build Tauri optimizado sin bundle |

Cada ejecución escribe un reporte JSON en `.local/validation/` con perfil, estado, tiempos, commit, rama, indicador de árbol sucio y versiones de PowerShell, Node, npm, Rust y Cargo. El directorio es local y está ignorado por Git. Usa `-ReportPath <ruta>` para elegir otro destino; las rutas relativas se resuelven desde la raíz del proyecto. El reporte también se intenta escribir si falla una etapa, conservando el último resultado y su error.

Al revisar este documento había 51 pruebas frontend y 81 pruebas Rust. Son una fotografía orientativa, no un umbral: actualiza el número si cambia de forma material o elimina el conteo si deja de ser útil.

## Estado real frente a arquitectura objetivo

### Implementado ahora

- Shell Tauri, frontend React y motor Rust/Polars.
- Flujo Cargar → Revisar → Preparar → Entregar.
- Formatos, perfiles, transformaciones, historial temporal, contratos y exportación descritos arriba.
- CSP restrictiva, capability mínima y validación local centralizada.

### Planeado o pendiente

- ejecución lazy/incremental y datasets mayores que la memoria;
- DuckDB embebido;
- SQLite, proyectos y recuperación de sesión;
- CLI y automatización sin interfaz;
- joins, comparación de datasets y destinos de bases de datos;
- instancia única;
- tipos de argumentos y estructuras internas Rust/TypeScript generados o verificados automáticamente (comandos, argumentos y retornos superiores ya tienen paridad automática);
- E2E, accesibilidad, pruebas visuales y presupuestos medibles; los reportes básicos de gates locales ya existen;
- supply chain, SBOM, empaquetado Windows y updater autenticado;
- verificación real en macOS y Linux.

Consulta `ROADMAP.md` para el detalle, pero verifica cada casilla contra el código antes de afirmar que una fase está completa.

## Riesgos y deuda técnica visibles

1. **Motor monolítico**: `dataset.rs` concentra casi todo el dominio. Un cambio puede afectar carga, receta, historial y exportación; usa CodeGraph y ejecuta pruebas Rust completas.
2. **UI monolítica**: `App.tsx` concentra coordinación y muchos editores. Los refactors deben preservar las uniones de estado y las confirmaciones de acciones destructivas.
3. **Contratos parcialmente duplicados**: comandos, argumentos y retornos superiores tienen un gate de paridad, pero Rust y TypeScript aún definen los tipos de argumentos y los campos internos manualmente; persiste riesgo de deriva estructural.
4. **Memoria**: el límite de 500 MiB no equivale a un presupuesto de RAM. Polars materializa el dataset y algunas operaciones crean candidatos completos.
5. **Persistencia efímera**: cerrar la aplicación pierde dataset, perfil e historial.
6. **Cobertura de plataforma**: el diseño es multiplataforma, pero soporte declarado requiere validación local en cada sistema.
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
| 2026-08-20 | El gate IPC compara los tipos de retorno Rust con los genéricos `invoke<T>` y normaliza `Result`, `Option`, `void` y los alias de recetas conocidos. | `src/ipc-contract.test.ts` |
| 2026-08-20 | Los perfiles de `tools/check.ps1` generan evidencia JSON local con entorno, commit, tiempos y resultados por etapa. | `tools/check.ps1`, `.local/validation/` |
| 2026-08-20 | El gate IPC ahora compara también los argumentos serializados, excluyendo las inyecciones internas `AppHandle` y `State`; los tipos y respuestas siguen pendientes. | `src/ipc-contract.test.ts` |
| 2026-08-20 | La primera versión del gate IPC comparó todos los nombres registrados con las llamadas de `bridge.ts`; en ese corte todavía no comparaba argumentos. | `src/ipc-contract.test.ts` |
| 2026-08-20 | Se creó este documento vivo a partir del código, las pruebas, `README.md`, `ROADMAP.md` y el índice CodeGraph. | Estado de `master` en `8fdcb3d` |

## Documentos relacionados

- [README.md](README.md): visión funcional y uso actual.
- [ROADMAP.md](ROADMAP.md): planificación, decisiones históricas y pendientes.
- [package.json](package.json): scripts y dependencias frontend.
- [src-tauri/Cargo.toml](src-tauri/Cargo.toml): dependencias del motor nativo.
- [src-tauri/tauri.conf.json](src-tauri/tauri.conf.json): configuración de escritorio y CSP.
