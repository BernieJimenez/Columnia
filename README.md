# Columnia

Columnia será una estación local multiplataforma para revisar, limpiar,
transformar y entregar datasets confiables.

Para entender rápidamente la arquitectura, el estado implementado, los riesgos
y las reglas de trabajo, consulta el [contexto vivo del proyecto](CONTEXTO.md).
Las fronteras de confianza, amenazas y controles se mantienen en el
[threat model vivo](THREAT_MODEL.md).

El proyecto está en su primer hito técnico. Actualmente contiene el shell Tauri
2, una interfaz React/TypeScript y el primer corte vertical del motor Polars:
selección nativa, carga local y vista previa de CSV, TSV, TXT delimitado, JSON, Parquet, Excel y ODS de hasta
500 MB. Los libros con varias hojas muestran un selector antes de cargar y React
solo recibe un identificador opaco, nunca la ruta local. Este límite es provisional:
el dataset aún se materializa en memoria y un archivo
grande puede requerir bastante más RAM durante perfiles y transformaciones.
Parquet conserva su esquema nativo, incluidos tipos temporales compatibles,
nulos y texto Unicode, y se lee con una configuración conservadora de memoria.

## Plataformas objetivo

- Windows
- macOS
- Linux

El diseño evita APIs exclusivas de un sistema operativo. Sin embargo, cada
plataforma se considerará soportada únicamente después de compilar, instalar y
probar Columnia localmente en ese sistema.

## Requisitos de desarrollo

- Node.js LTS.
- Rust estable instalado mediante `rustup`.
- Dependencias nativas de Tauri para el sistema operativo correspondiente.

La máquina de desarrollo actual ya dispone de Rust/Cargo, WebView2 y las Build
Tools de Visual Studio. El shell nativo se compila localmente en Windows.

## Proyectos locales

En **Cargar**, Columnia permite guardar el dataset materializado como un proyecto,
actualizarlo, abrirlo más tarde o recuperar explícitamente la última sesión. El
catálogo usa SQLite y cada versión se publica como un snapshot Parquet privado
en el directorio de datos de la aplicación. React recibe únicamente IDs opacos y
metadatos; nunca las rutas internas.

El esquema SQLite v3 conserva exactamente el dataset y su nombre visible aunque
la fuente original haya desaparecido, además de las reglas de calidad, el
borrador opcional de receta, el perfil cacheado y el historial Deshacer/Rehacer
con su cursor. Los catálogos v1 y v2 se migran de forma compatible al abrirse.
Columnia valida el conjunto durable antes de activarlo; si un perfil o snapshot
está corrupto, la apertura falla sin reemplazar el dataset actual.

Al abrir correctamente, el perfil y las revisiones durables se copian a una
sesión temporal para continuar trabajando sin modificar directamente los
artefactos guardados. El historial conserva el máximo de doce revisiones y el
presupuesto de 1 GiB. Eliminar un proyecto no descarta el dataset que ya está
abierto en memoria.

## Desarrollo local

```powershell
npm install
npm run tauri dev
```

El E2E del shell web usa Playwright contra un preview Vite local (Edge instalado
en Windows o Chromium local en otros sistemas). Instala el navegador una vez y
ejecuta las pruebas:

```powershell
npm run test:e2e:install
npm run test:e2e
```

Estas pruebas cubren la carcasa web, la navegación accesible, reduced-motion,
viewports móvil/desktop y el ciclo de proyectos con IPC simulado, además de un
presupuesto de primer render de 3 s; los comandos Rust reales siguen validándose
con Tauri y los smokes locales.

Para generar evidencia visual reproducible ejecuta `npm run accessibility:visual`.
El comando construye el preview y captura desktop, móvil, escala de dispositivo
125% y `forced-colors`, validando landmarks, foco, targets mínimos y overflow; las
imágenes y el resumen quedan en `.local/validation/accessibility-visual/`.

En Windows, `npm run smoke:cdp` levanta el comando real `npm run tauri dev` con
un puerto CDP de loopback aislado, verifica `/json/version` y `/json/list`, y
usa `chromium.connectOverCDP` para medir primer render, landmarks y foco del
WebView2, además de comprobar el contrato accesible de `ProjectsPanel` y los
comandos IPC nativos. En el build debug, el ciclo temporal siembra un dataset,
persiste/aplica una receta, exporta CSV con quality gate y guarda/abre/elimina
un proyecto con ese workspace; la evidencia conserva solo estados y conteos.
El probe restaura la variable de entorno y termina únicamente los procesos que
creó; reporta el primer render aunque el arranque debug frío supere el
presupuesto. Para resumir las evidencias locales por categoría y comparar
deltas entre ejecuciones usa `npm run perf:summary`.

Para medir una entrada sintética cercana a 100 MiB sin conservarla en el árbol
de trabajo ejecuta `npm run perf:benchmark`. El benchmark usa la CLI local para
`inspect`, `validate` y transformaciones CSV/Parquet, registra duración y pico
de working set, y deja solo un resumen en `.local/validation/`.

`npm run smoke:cdp` añade un perfil acotado del proceso debug (working set y
memoria privada inicial, máxima y final) y ejecuta, solo en el build debug del
probe, un ciclo temporal nativo de dataset/receta/exportación/proyecto. La
reapertura durable crea un `ProjectStore` fresco y valida SQLite, snapshot,
recovery y workspace antes del cleanup. La evidencia conserva conteos/estados,
no IDs, datos ni rutas; esta señal no habilita CDP en el arranque normal. Desde
v0.44.0 aplica por defecto un presupuesto de 512 MiB de working set y 256 MiB
de memoria privada al árbol de procesos propio; si una ejecución soportada lo
excede, falla. El mismo ciclo conserva duración total y muestras por comando
IPC (solo milisegundos), que `npm run perf:summary` agrega junto al perfil de
memoria.

Para comprobar continuidad entre procesos ejecuta `npm run smoke:restart`. El
comando corre dos fases aisladas: prepara y persiste el proyecto, termina la
primera instancia `npm run tauri dev`, inicia una segunda, recupera el proyecto
desde SQLite/snapshot y finalmente lo elimina. Cada fase conserva su cleanup y
evidencia separada.

## Automatización por CLI

La CLI reutiliza el motor Rust y entrega resultados JSON versión 1 para poder
integrarse en scripts sin abrir la interfaz:

```powershell
cargo run --release --manifest-path src-tauri/Cargo.toml --bin columnia-cli -- inspect --input datos.csv
cargo run --release --manifest-path src-tauri/Cargo.toml --bin columnia-cli -- transform --input datos.csv --recipe receta.json --output resultado.parquet --format parquet
cargo run --release --manifest-path src-tauri/Cargo.toml --bin columnia-cli -- validate --input datos.csv --rules calidad.json
cargo run --release --manifest-path src-tauri/Cargo.toml --bin columnia-cli -- inspect --input libro.xlsx --sheet Datos --header first-row
cargo run --release --manifest-path src-tauri/Cargo.toml --bin columnia-cli -- batch --manifest lote.json
cargo run --release --manifest-path src-tauri/Cargo.toml --bin columnia-cli -- project-list --store .\almacen-columnia
cargo run --release --manifest-path src-tauri/Cargo.toml --bin columnia-cli -- project-save --store .\almacen-columnia --name Ventas --input datos.csv --profile
cargo run --release --manifest-path src-tauri/Cargo.toml --bin columnia-cli -- project-inspect --store .\almacen-columnia --id <id>
cargo run --release --manifest-path src-tauri/Cargo.toml --bin columnia-cli -- project-export --store .\almacen-columnia --id <id> --output entrega.parquet --format parquet
cargo run --release --manifest-path src-tauri/Cargo.toml --bin columnia-cli -- project-delete --store .\almacen-columnia --id <id> --confirm <id>
```

`inspect`, `transform` y `validate` aceptan CSV, TSV, JSON, Parquet, XLSX, XLS,
XLSB y ODS. Para libros son obligatorios `--sheet <nombre exacto>` y `--header
first-row|generated`; esas opciones se rechazan para otros formatos. Las rutas y
los valores del dataset no aparecen en el JSON ni en los errores. La salida se
publica de forma atómica y CSV conserva la protección contra fórmulas. `validate`
devuelve 0 cuando el contrato pasa, 2 cuando falla y 1 ante errores de uso/carga.

Un manifiesto batch v1 contiene entre 1 y 64 trabajos `input`, `recipe`,
`output` y `format`, más `sheet`/`header` para libros. Las rutas relativas se
resuelven desde la carpeta del manifiesto. Columnia valida todo el lote antes de
escribir y rechaza destinos repetidos o que sobrescriban inputs, recetas o el
propio manifiesto. Cada trabajo es atómico, pero el lote no es una transacción
global: un fallo tardío conserva los trabajos anteriores, informa su ordinal y
termina con código 2. Un manifiesto inválido termina con código 1 sin outputs.

Los cinco comandos de proyectos requieren un almacén explícito y canonicalizado
mediante `--store <directorio>`; no usan implícitamente el directorio privado de
la aplicación de escritorio:

- `project-save --store DIR --name NAME --input FILE [--id ID] [--sheet NAME --header first-row|generated] [--recipe FILE] [--rules FILE] [--profile]` crea un proyecto o actualiza el ID indicado. La receta, las reglas y el cálculo de perfil son opcionales.
- `project-list --store DIR` lista resúmenes ordenados del catálogo.
- `project-inspect --store DIR --id ID` inspecciona metadatos y estado durable sin activar el proyecto ni abrir una sesión de escritorio.
- `project-export --store DIR --id ID --output FILE --format csv|parquet [--allow-unvalidated]` valida y exporta el snapshot completo de forma atómica, sin activarlo ni cambiar la recuperación del escritorio. Las reglas guardadas siempre deben aprobar; `--allow-unvalidated` solo autoriza un proyecto que no tenga reglas.
- `project-delete --store DIR --id ID --confirm ID` borra únicamente cuando la confirmación coincide exactamente con el ID.

Todos emiten JSON v1 por stdout sin rutas, filas ni muestras. Sus contratos son:

| Comando | Campos de respuesta |
| --- | --- |
| `project-list` | `schemaVersion`, `command`, `projects` con resúmenes de ID, nombre, archivo visible, dimensiones y fechas |
| `project-save` | `schemaVersion`, `command`, `created`, `project` |
| `project-inspect` | `schemaVersion`, `command`, `project`, `profileCached`, `qualityRuleCount`, `recipeDraftPresent`, `history` con conteo, cursor, disponibilidad de undo/redo y estado degradado |
| `project-export` | `schemaVersion`, `command`, `status`, `format`, `quality`; añade `fileName` y `fileSizeBytes` solo cuando publica |
| `project-delete` | `schemaVersion`, `command`, `id`, `deleted` |

El código 0 indica éxito, 2 indica una exportación bloqueada por reglas
reprobadas y 1 indica error de uso, carga o almacenamiento.

La validación permanece completamente local. Desde PowerShell:

```powershell
.\tools\check.ps1 -Profile Fast
.\tools\check.ps1 -Profile Full
.\tools\check.ps1 -Profile Release
```

`Fast` comprueba formato, compilación Rust, pruebas frontend y build web. `Full`
añade Clippy con warnings como errores y las pruebas Rust. `Release` agrega el
binario Tauri optimizado sin crear instaladores ni usar servicios externos.

El flujo principal replica el orden de `dataprepv1.1`: **Cargar → Revisar →
Preparar → Entregar**. Cada acción aparece únicamente en la etapa que le
corresponde.

En **Cargar**, usa **Seleccionar dataset**. Rust abre el diálogo nativo para CSV,
TSV, TXT delimitado, JSON/JSON Lines, Parquet, XLSX, XLS, XLSB u ODS. En cada libro permite
elegir la hoja y decidir si la primera fila contiene encabezados o debe conservarse
como datos generando `column_1`, `column_2`, etc. Rust valida y
conserva el dataset en la sesión; React recibe solamente el esquema,
los metadatos y las primeras 50 filas.
Durante la carga se muestra el avance por fases. El perfil de calidad informa el
porcentaje conforme termina cada columna; ambos canales permanecen dentro del
equipo mediante IPC de Tauri.
Las operaciones activas se pueden cancelar. El perfil se detiene entre columnas;
la lectura del dataset se descarta después de terminar la fase que Polars tenga en curso.
Cancelar una selección de hoja o una sustitución conserva el dataset que ya estaba activo.
TSV usa tabuladores estrictos y conserva valores léxicos como ceros iniciales y
decimales formateados. Excel/ODS conserva booleanos, enteros, decimales, fechas y
duraciones cuando una columna es compatible; las mezclas inseguras quedan como texto.
CSV también conserva todas sus columnas físicamente como texto porque el formato
no contiene un esquema confiable. Calidad calcula estadísticas numéricas semánticas
cuando todos los valores no vacíos son números seguros; identificadores con ceros
iniciales y enteros que perderían precisión quedan excluidos de esa interpretación.
CSV y TXT detectan de forma conservadora coma, punto y coma, tabulador o `|`,
respetando delimitadores dentro de campos entrecomillados. TSV fuerza tabulador.
Se acepta UTF-8 con o sin BOM; bytes inválidos se rechazan sin sustituir caracteres.
XLSX, XLSB y ODS advierten que su tamaño comprimido puede requerir bastante más RAM.
JSON admite un arreglo de objetos o un objeto por línea (`.jsonl`/`.ndjson`).
Los campos ausentes quedan como nulos y los objetos o arreglos anidados se conservan
como texto JSON, sin aplanarlos ni descartar su contenido silenciosamente.

Desde **Entregar**, el dataset activo puede exportarse a CSV o Parquet. Rust abre
el selector nativo y escribe primero un archivo temporal en la carpeta elegida.
El destino se reemplaza únicamente después de completar y sincronizar la
escritura; cancelar o fallar conserva cualquier archivo anterior.
Antes de exportar puede definirse un **contrato de calidad** de hasta dieciséis
reglas exactas: no nulo, texto no vacío, unicidad o rango numérico inclusivo.
Cada regla admite una tolerancia por cantidad o porcentaje y el resultado solo
expone conteos, nunca muestras de los datos. Rust vuelve a evaluar el contrato
sobre el mismo snapshot que escribirá antes de abrir el selector. Si no existen
reglas, la entrega no validada requiere una confirmación explícita.
En **Revisar**, la vista previa permite recorrer el dataset en páginas de 50 filas sin volver a abrir
el archivo ni enviar su ruta al frontend.
El botón **Analizar calidad** calcula en Rust los nulos, la completitud y los
valores únicos no nulos de cada columna. Para columnas numéricas también muestra
mínimo, máximo y promedio; el resultado se reutiliza durante la sesión.
El mismo análisis cuenta filas duplicadas adicionales y, para texto, cadenas
vacías y longitudes mínima, máxima y promedio. Las cadenas compuestas solo por
espacios se consideran vacías.
Para columnas de texto con al menos tres valores, Columnia sugiere tipos
booleano, entero, decimal o fecha cuando al menos el 90% coincide. La sugerencia
es informativa: en este hito no transforma el dataset.
Para columnas numéricas, el perfil añade desviación estándar muestral, Q1,
mediana, Q3 y posibles outliers mediante la regla IQR de 1.5. Los nulos y
valores no finitos se excluyen; no se señalan outliers con menos de cuatro datos.

En **Preparar**, cuando el perfil encuentra duplicados exactos, **Eliminar duplicados** conserva
la primera aparición y elimina las repeticiones posteriores de la sesión activa.
El archivo original no se modifica. La operación queda registrada en el historial
de **Deshacer/Rehacer** y el perfil debe recalcularse sobre el resultado.

La corrección **Normalizar nombres de columnas** sigue las reglas del proyecto
de referencia: minúsculas, eliminación de acentos, `_` para espacios y guiones,
prefijo `col_` cuando el encabezado comienza con un número y sufijos estables
cuando dos encabezados producen el mismo nombre. También puede deshacerse.

Columnia también puede **Recortar espacios** exteriores en todas las columnas
de texto sin modificar su contenido interno. Para una limpieza más profunda,
**Normalizar texto** permite elegir columnas concretas, convertir a minúsculas,
compactar espacios y decidir si se eliminan acentos. La interfaz informa cuántas
celdas y filas cambiaron, conserva los nulos y permite deshacer el resultado.

La barra **Continuidad de trabajo** permite recorrer hasta doce revisiones con
Deshacer y Rehacer. Durante la sesión, cada revisión vive como un snapshot Parquet
temporal con un presupuesto total de 1 GiB. Guardar el dataset como proyecto
conserva durablemente esa cadena y su cursor; abrirla crea una nueva copia temporal
de trabajo. Si un único snapshot excede ese presupuesto, Columnia aplica el cambio,
desactiva honestamente la reversión y muestra el motivo. **Aplicar recomendadas**
agrupa el recorte exterior y la normalización de encabezados en una sola operación
atómica: ambos cambios se publican juntos o el dataset permanece intacto.

La pestaña **Transformaciones** permite construir una receta estructural con
varios renombres, conversiones de tipo y parseos de fecha. La receta se aplica
una sola vez y en orden determinista: renombres, tipos y fechas. Las conversiones
son estrictas —un valor inválido cancela el lote completo—, los nulos se preservan
y un resultado correcto ocupa una única revisión de Deshacer/Rehacer. El borrador
puede guardarse y cargarse como una receta JSON v1 mediante selectores nativos;
la ruta nunca llega a React y una receta cargada no se aplica automáticamente.
La ejecución sigue siendo inmediata y no lazy.

La receta también admite hasta tres **filtros AND** y una **columna calculada**.
Como los filtros pueden eliminar filas, Columnia muestra una confirmación antes
de ejecutar. Los cálculos aceptan otra columna o un valor fijo de forma
explícita, con suma, resta, multiplicación, división, concatenación y extracción
de año, mes o día. El motor preserva nulos y cancela el lote completo ante
conversiones imprecisas, división por cero, infinitos o fechas no representables.

**Buscar y reemplazar** trabaja de forma literal y sensible a mayúsculas sobre
una columna o sobre todas las columnas físicas de texto; no interpreta patrones
regulares ni convierte silenciosamente columnas numéricas. También puedes elegir
qué columnas conservar. El descarte solicita confirmación y la receta rechaza
eliminar columnas que todavía necesita un cálculo posterior.

**Dividir columna** usa un separador literal y nombres de salida explícitos; la
última salida conserva cualquier resto. **Combinar columnas** une entre dos y
dieciséis campos de texto en el orden seleccionado, omitiendo nulos sin confundirlos
con cadenas vacías. La eliminación opcional de las columnas fuente requiere
confirmación y permanece incluida en la misma operación reversible.

El tratamiento de **outliers** utiliza Q1, Q3 e IQR × 1.5 sobre un mismo estado
previo. Puedes limitar valores o eliminar filas en columnas diferentes; los
nulos se conservan y cualquier acción requiere confirmación. Para evitar pérdida
silenciosa, el motor rechaza infinitos, NaN y enteros fuera del rango exacto que
puede representar durante el cálculo.

**Agrupar y resumir** reemplaza la granularidad del dataset por grupos estables,
ordenados según su primera aparición. Admite claves nulas y agregaciones de suma,
promedio, mínimo, máximo, conteo de filas y valores únicos. El motor conserva los
tipos compatibles, controla overflow y precisión, y solicita confirmación antes
de sustituir el dataset por el resumen.

La receta puede normalizar **correos, teléfonos y direcciones** mediante reglas
explícitas y extraer tokens, dígitos, letras o segmentos antes/después de un
delimitador literal. Las operaciones preservan nulos y admiten Unicode. Las
direcciones no cambian automáticamente de capitalización y la extracción no
acepta expresiones regulares libres en este hito.

Validación rápida y completamente local:

```powershell
.\tools\check.ps1 -Profile Fast
```

La validación completa añade Clippy y las pruebas Rust:

```powershell
.\tools\check.ps1 -Profile Full
```

El perfil de distribución genera un SBOM CycloneDX reproducible y compila el
binario Tauri optimizado sin crear instaladores:

```powershell
.\tools\check.ps1 -Profile Release
```

En Windows, el perfil de empaquetado produce MSI y NSIS e inventaría cada
artefacto con tamaño y SHA-256. La primera ejecución puede descargar herramientas
oficiales de Tauri/NSIS y valida sus hashes:

```powershell
.\tools\check.ps1 -Profile Package
```

Para comprobar automáticamente el shell web y el mismo arranque que se usa durante desarrollo:

```powershell
npm run test:e2e
npm run smoke:desktop -- -TimeoutSeconds 120
npm run smoke:cli
```

No se utilizan CI, GitHub Actions ni workflows. Consulta [ROADMAP.md](ROADMAP.md)
para conocer las decisiones y fases previstas.
