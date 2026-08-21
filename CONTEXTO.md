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
| Última revisión de este documento | 2026-08-20, rama `master`, commit base `dbf54e4` |

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
| `src/App.tsx` | Flujo completo de interfaz, estados, formularios y coordinación de casos de uso. Es actualmente un archivo grande de unas 2,467 líneas. |
| `src/components/` | Componentes accesibles extraídos para diálogos, tabs de revisión y progreso cancelable. |
| `src/bridge.ts` | Contrato TypeScript del IPC y única fachada de `invoke()` usada por la UI. |
| `src/styles.css` | Sistema visual y layout de la aplicación. |
| `src-tauri/src/main.rs` | Entrada mínima del ejecutable; delega en `columnia_lib::run()`. |
| `src-tauri/src/lib.rs` | Inicializa Tauri, instancia única, diálogo nativo, `DatasetState` y los 20 comandos permitidos. |
| `src-tauri/src/dataset.rs` | Motor de datos completo. Contiene carga, tipos, perfiles, recetas, historial y exportación en unas 7,983 líneas. |
| `src-tauri/capabilities/main.json` | Capability mínima para la ventana `main`: solamente `core:default`. |
| `src-tauri/tauri.conf.json` | Ventana, build, bundle y CSP de producción/desarrollo. |
| `tools/check.ps1` | Entrada única para los gates locales Fast, Full y Release; genera evidencia JSON auditable en `.local/validation/`. |
| `tools/generate-sbom.ps1` | Genera offline un SBOM CycloneDX 1.6 reproducible desde ambos lockfiles. |
| `tools/check-bundle.mjs` | Mide presupuestos JS/CSS e inventaría bundles de distribución nuevos o actualizados. |
| `tools/smoke-tauri.ps1` | Arranca `npm run tauri dev`, comprueba Vite y el ejecutable debug, y limpia solo su Job Object. |
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

Regla de mantenimiento: cualquier cambio de nombre, argumentos, serialización o respuesta en Rust debe reflejarse en `bridge.ts` y quedar cubierto por pruebas. `src/ipc-contract.test.ts` verifica automáticamente comandos registrados, argumentos serializados, tipos de retorno superiores, nombres de campos y tipos concretos de 39 estructuras compartidas. Normaliza referencias, números, `Vec`/arrays, `Option`/campos opcionales, herencia, literales y alias conocidos. Las 14 subestructuras de `TransformRecipe` tienen interfaces nominales equivalentes a Rust; los alias públicos históricos se conservan para no romper consumidores.

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

### Gates locales

```powershell
.\tools\check.ps1 -Profile Fast
.\tools\check.ps1 -Profile Full
.\tools\check.ps1 -Profile Release
.\tools\check.ps1 -Profile Package
npm run smoke:desktop -- -TimeoutSeconds 120
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

Al revisar este documento había 65 pruebas frontend y 87 pruebas Rust; las ramas específicas de symlinks/reparse points dependen de la plataforma. Son una fotografía orientativa, no un umbral: actualiza el número si cambia de forma material o elimina el conteo si deja de ser útil.

## Estado real frente a arquitectura objetivo

### Implementado ahora

- Shell Tauri, frontend React y motor Rust/Polars.
- Instancia única en escritorio: una segunda apertura muestra, desminimiza y enfoca la ventana `main` existente.
- Flujo Cargar → Revisar → Preparar → Entregar.
- Formatos, perfiles, transformaciones, historial temporal, contratos y exportación descritos arriba.
- CSP restrictiva, capability mínima y validación local centralizada.
- Threat model vivo y gates de regresión para CSP, permisos, payloads semánticos y fórmulas CSV.
- Navegación por teclado inicial con skip link, pestañas ARIA, foco visible, regiones anunciables y diálogos con ciclo/restauración de foco.
- SBOM CycloneDX 1.6 reproducible y gates offline de integridad/procedencia para npm y Cargo.
- Smoke automatizado del runtime de desarrollo con aislamiento y cleanup de procesos propios.
- Presupuestos medibles del frontend y empaquetado Windows verificado en MSI/NSIS con evidencia criptográfica.

### Planeado o pendiente

- ejecución lazy/incremental y datasets mayores que la memoria;
- DuckDB embebido;
- SQLite, proyectos y recuperación de sesión;
- CLI y automatización sin interfaz;
- joins, comparación de datasets y destinos de bases de datos;
- E2E de flujos reales con datasets, auditoría manual con lector de pantalla/zoom/alto contraste y pruebas visuales; el smoke de arranque ya existe;
- escaneo de vulnerabilidades, firma de instaladores y updater autenticado; SBOM, gates offline y empaquetado Windows básico ya existen;
- verificación real en macOS y Linux.

Consulta `ROADMAP.md` para el detalle, pero verifica cada casilla contra el código antes de afirmar que una fase está completa.

## Riesgos y deuda técnica visibles

1. **Motor monolítico**: `dataset.rs` concentra casi todo el dominio. Un cambio puede afectar carga, receta, historial y exportación; usa CodeGraph y ejecuta pruebas Rust completas.
2. **UI monolítica**: `App.tsx` concentra coordinación y muchos editores. Los refactors deben preservar las uniones de estado y las confirmaciones de acciones destructivas.
3. **Contratos duplicados con gate**: Rust y TypeScript todavía declaran contratos por separado, pero 39 estructuras tienen comparación automática de campos y tipos. Al añadir una estructura compartida nueva, debe incorporarse explícitamente a las listas del gate IPC.
4. **Memoria**: el límite de 500 MiB no equivale a un presupuesto de RAM. Polars materializa el dataset y algunas operaciones crean candidatos completos.
5. **Persistencia efímera**: cerrar la aplicación pierde dataset, perfil e historial.
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
