# Changelog

Todos los cambios visibles de Columnia se registran aquí. Las versiones siguen
SemVer y el estado real del prototipo se contrasta con el código, los tests y
los artefactos de validación locales.

## [Unreleased]

## [0.138.0] - 2026-09-01

### Mejorado

- Los Bundles source-backed con una receta activa conservan la ruta incremental:
  DuckDB transfiere el dataset y el Bundle incorpora `recipe.json` validado,
  su referencia y su hash en `manifest.json`, sin materializar el `DataFrame`
  activo.
- La combinación de privacidad `mask`/`hash`, contrato de calidad incremental y
  receta de Bundle mantiene atomicidad, cancelación, validación de cambios de
  la fuente y limpieza de snapshots temporales.

### Verificado

- Una regresión dedicada confirma que un Bundle source-backed con receta
  conserva la fuente original y publica `recipe.json` con hash verificable.

## [0.137.0] - 2026-09-01

### Mejorado

- Las exportaciones source-backed ya conservan la ruta incremental cuando se
  solicita protección `mask` o `hash`: DuckDB genera un snapshot Parquet
  privado con las columnas personales protegidas y luego lo transfiere a CSV,
  JSON, Parquet, SQL, Excel, SQLite o bundle sin materializar todo el dataset.
- La validación de calidad continúa ejecutándose sobre la fuente original y la
  protección se mantiene aislada, temporal y con comprobación de cambios de la
  fuente antes y después de exportar.

### Verificado

- La regresión existente de snapshots source-backed confirma máscara y hash,
  preservación de nulos, columnas no personales, fuente intacta y limpieza de
  artefactos temporales.

## [0.136.0] - 2026-09-01

### Mejorado

- `Eliminar filas duplicadas parecidas` ya puede calcular sus claves
  normalizadas y exactas directamente en DuckDB sobre fuentes source-backed,
  conservando la primera fila, las repeticiones exactas y el orden original
  sin materializar todas las filas en el `DataFrame` activo.
- `Aplicar correcciones recomendadas` ya combina el recorte de espacios y la
  normalización determinista de nombres en una sola proyección source-backed,
  con conteos exactos de celdas y publicación reversible.
- Ambas rutas conservan fallback eager para fuentes incompatibles y dejan el
  frame activo en modo esquema-only cuando publican un snapshot Parquet.

### Verificado

- Las regresiones comparan duplicados parecidos y correcciones recomendadas
  source-backed con eager, incluyendo repeticiones exactas, renombres, trim,
  conteos, orden y snapshot publicado.

## [0.135.0] - 2026-09-01

### Mejorado

- Las acciones directas `cap`, `impute` y `drop` de outliers ya pueden
  calcular IQR, límites y medianas directamente sobre datasets source-backed
  mediante DuckDB, sin materializar todas las filas en el `DataFrame` activo.
- Las tres acciones conservan nulos, tipos cuando corresponde, orden y
  fallback eager para fuentes incompatibles; `cap` mantiene la salida
  `Float64` de la semántica existente y `impute` conserva `Int64`/`Float64`.
- Los conteos de filas afectadas y celdas modificadas se calculan en disco,
  las mutaciones publican snapshots Parquet reversibles y actualizan
  `_cambios` cuando está activo.

### Verificado

- La regresión compara los tres modos directos source-backed con eager,
  incluyendo tipos, nulos, conteos y permanencia del frame activo en
  esquema-only.

## [0.134.0] - 2026-09-01

### Mejorado

- `Imputación conservadora` ya puede calcular modas de texto y medianas
  numéricas directamente sobre datasets source-backed mediante DuckDB, sin
  materializar todas las filas en el `DataFrame` activo.
- `Imputación categórica` ya rellena nulos source-backed con `Desconocido`
  directamente sobre la fuente, conservando tipos, orden y columnas no
  afectadas.
- Ambas operaciones cuentan filas y celdas afectadas en disco, actualizan
  `_cambios` cuando está activo, publican snapshots Parquet reversibles y
  mantienen fallback eager si la fuente no es compatible.

### Verificado

- La regresión cubre la imputación categórica, la mediana numérica inferior,
  snapshots sucesivos y permanencia del frame activo en modo esquema-only.

## [0.133.0] - 2026-09-01

### Mejorado

- `Convertir números detectados` ya puede analizar y convertir columnas
  numéricas source-backed directamente en DuckDB, sin materializar todas las
  filas en el `DataFrame` activo.
- `Interpretar fechas detectadas` ya puede inferir formatos fijos e ISO segura
  sobre fuentes source-backed, con el mismo umbral conservador de la ruta
  eager, tolerancia de nulos y validación de años 1900–2100.
- Ambas operaciones conservan identificadores y códigos con ceros iniciales,
  cuentan filas y celdas afectadas en disco, publican snapshots Parquet
  reversibles y mantienen fallback eager si la fuente no es compatible.

### Verificado

- La regresión cubre conversión numérica y temporal source-backed, tipos
  publicados, preservación de códigos con ceros iniciales y permanencia del
  frame activo en modo esquema-only.

## [0.132.0] - 2026-09-01

### Mejorado

- `Normalizar booleanos` ya puede detectar y transformar columnas candidatas
  directamente sobre datasets source-backed mediante DuckDB, sin materializar
  todas las filas en el `DataFrame` activo.
- La detección conserva el umbral eager del 90%, reconoce alias Unicode y de
  mayúsculas, mantiene los valores no reconocidos, actualiza `_cambios` y
  publica snapshots Parquet reversibles.

### Verificado

- La regresión cubre el umbral de candidatos, alias `Sí`, valores no
  booleanos, conteos exactos y permanencia del frame en modo esquema-only.

## [0.131.0] - 2026-09-01

### Mejorado

- `Recortar espacios`, `Normalizar texto` y `Normalizar valores centinela` ya
  pueden ejecutarse sobre datasets source-backed mediante DuckDB, sin
  materializar todas las filas en el `DataFrame` activo.
- Las tres limpiezas conservan nulos, orden y columnas no seleccionadas,
  calculan celdas y filas afectadas en disco, actualizan `_cambios` cuando está
  activo y publican snapshots Parquet reversibles.

### Verificado

- La regresión cubre recorte seleccionado, centinelas, normalización Unicode,
  eliminación de acentos, conteos exactos y permanencia del frame en modo
  esquema-only.

## [0.130.0] - 2026-09-01

### Mejorado

- `Normalizar nombres de columnas` y `Activar trazabilidad por fila` ya pueden
  ejecutarse sobre datasets source-backed mediante DuckDB, sin materializar
  todas las filas en el `DataFrame` activo.
- La normalización conserva el orden, resuelve colisiones de nombres con la
  misma regla eager y publica snapshots Parquet reversibles; la trazabilidad
  añade `_cambios` como columna de texto nula y mantiene fallback seguro.

### Verificado

- La regresión cubre nombres normalizados, colisiones, snapshot source-backed,
  activación de `_cambios` y ciclos undo/redo.

## [0.129.0] - 2026-09-01

### Mejorado

- `Proteger valores personales detectados` ya puede ejecutarse sobre datasets
  source-backed mediante DuckDB, sin cargar todas las filas en el `DataFrame`
  activo.
- La máscara conserva nulos, ignora valores ya `[REDACTED]`, actualiza
  `_cambios`, publica un snapshot Parquet reversible y mantiene el contrato IPC
  agregado de celdas y columnas modificadas.

### Verificado

- La regresión cubre conteo exacto, columnas personales, valores redactados,
  trazabilidad, snapshot legible y undo.

## [0.128.0] - 2026-09-01

### Mejorado

- `Retirar columnas identificadoras` y `Retirar datos personales detectados`
  ya pueden ejecutarse sobre datasets source-backed sin materializar todas las
  filas en el `DataFrame` activo.
- Ambas acciones publican snapshots Parquet privados, conservan `_cambios`,
  el límite de una columna utilizable y el historial reversible, y mantienen
  el fallback eager para fuentes no compatibles.

### Verificado

- La regresión cubre la detección por categorías, la eliminación de `customer_id`
  y `email`, la trazabilidad acumulada y undo sobre el snapshot publicado.

## [0.127.0] - 2026-09-01

### Mejorado

- `Eliminar filas duplicadas` y las limpiezas de columnas constantes,
  completamente vacías o con alta nulidad ya pueden ejecutarse sobre datasets
  source-backed mediante DuckDB, sin cargar todas las filas en el `DataFrame`
  activo.
- Las limpiezas publican snapshots Parquet privados, conservan el orden de
  aparición, `_cambios` y el límite de una columna utilizable, con fallback
  eager cuando la fuente o el presupuesto no son compatibles.

### Verificado

- La regresión cubre duplicados, las tres políticas de columnas, la etiqueta
  acumulada de `_cambios`, snapshots legibles y undo del último cambio.

## [0.126.0] - 2026-09-01

### Mejorado

- `Eliminar filas completamente vacías` ya puede ejecutarse sobre datasets
  source-backed desde DuckDB, publicando un snapshot Parquet sin cargar todas
  las filas en el `DataFrame` activo.
- La operación conserva la semántica de nulos, blancos y `_cambios`, y activa
  snapshots de historial para mantener undo/redo reversible incluso después
  de una apertura diferida.

### Verificado

- La regresión comprueba una fuente JSON, el snapshot limpio, el frame activo
  sin filas y los ciclos undo/redo.
- Si el snapshot no cabe en el presupuesto local, se mantiene el fallback
  eager existente.

## [0.125.0] - 2026-09-01

### Mejorado

- La apertura diferida de `JSON`, `JSONL` y `NDJSON` grandes crea un snapshot
  Parquet privado mediante DuckDB, conservando solo el esquema y la primera
  página en el `DataFrame` activo.
- El snapshot permite contar filas y reutilizar paginación, consultas,
  proyectos y recetas source-backed sin cargar todas las filas del archivo
  estructurado en memoria.

### Verificado

- Una regresión cubre los tres sufijos, preview, conteo, snapshot legible y
  cancelación sin residuos.
- La carga valida que el archivo original conserve su tamaño durante la
  creación del snapshot.

## [0.124.0] - 2026-09-01

### Mejorado

- Los filtros `Eq` y `Neq` sobre columnas `Date` y `Datetime` aceptan
  literales ISO 8601 y comparan el valor temporal real en eager, Polars lazy y
  DuckDB source-backed, sin depender de la representación textual o de la
  unidad interna del timestamp.

### Verificado

- La regresión cubre igualdad y desigualdad sobre una fecha parseada a
  `Datetime`, compara la salida source-backed con eager y confirma que el
  `DataFrame` activo permanece vacío.

## [0.123.0] - 2026-09-01

### Mejorado

- Las recetas lazy/streaming ya pueden parsear una fecha, filtrarla con
  límites ISO 8601 y extraer `year`, `month` o `day` en la misma ejecución.
  El filtro se aplica antes del cálculo y evita degradar innecesariamente a la
  ruta eager.

### Verificado

- Una regresión cubre parseo DMY, dos límites temporales, extracción de año,
  conteo de filas y tipos del resultado dentro del plan lazy.

## [0.122.0] - 2026-09-01

### Mejorado

- Los filtros ordenados sobre columnas `Date` y `Datetime` aceptan literales
  ISO 8601 y se ejecutan directamente en la ruta source-backed de DuckDB.
  La ruta eager y el plan lazy usan la misma semántica temporal, incluidos
  parseos de fecha previos y unidades de tiempo compatibles.
- Preparar ya documenta el formato esperado para filtrar fechas sin obligar a
  convertirlas previamente en columnas de año, mes o día.

### Verificado

- Una regresión compara filtros inclusivos y exclusivos de rango temporal,
  conteo, tipo `Date`, orden y snapshot Parquet contra la ruta eager.

## [0.121.0] - 2026-09-01

### Mejorado

- Las recetas source-backed ya pueden parsear fechas y extraer `year`, `month`
  o `day` después de aplicar filtros, manteniendo el orden de etapas de la ruta
  eager sin materializar las filas.

### Verificado

- La regresión compara la combinación de parseo, filtro y componente de fecha
  source-backed con eager, incluyendo conteo de filas y snapshot Parquet.

## [0.120.0] - 2026-09-01

### Mejorado

- Guardar proyectos source-backed ahora convierte CSV/TSV/TXT delimitado y
  Parquet directamente a un snapshot Parquet administrado con DuckDB, sin
  materializar todas las filas en el `DataFrame` activo.
- La validación conserva el conteo real de filas, la integridad del archivo y
  la sesión source-backed; los proyectos normales mantienen la ruta eager y su
  historial existente.

### Verificado

- La regresión comprueba que el snapshot contiene todas las filas mientras el
  estado activo conserva el esquema vacío y `source_backed`.

## [0.119.0] - 2026-09-01

### Mejorado

- Las recetas source-backed ya ejecutan divisiones calculadas con operandos
  literales o columnas directamente en DuckDB, incluyendo filtros y
  reemplazos previos.
- La división por cero se valida antes de publicar el snapshot; los nulos,
  el orden, el conteo de columnas y el `DataFrame` activo vacío se conservan.

### Verificado

- La regresión compara la división por columna source-backed con eager y
  confirma que una división por cero no publica ni materializa el dataset.

## [0.118.0] - 2026-09-01

### Mejorado

- Las recetas source-backed ya ejecutan reemplazos regex seguros en DuckDB
  usando la misma proyección por bloques que los reemplazos literales.
- Se conservan reemplazos globales, grupos de captura `$1`–`$9`, nulos,
  conteo de celdas modificadas y `DataFrame` activo vacío; las sustituciones
  con sintaxis no compatible mantienen fallback eager.

### Verificado

- La regresión compara regex source-backed y eager, comprueba captura,
  conteo, Parquet resultante y ausencia de materialización del frame activo.

## [0.117.0] - 2026-09-01

### Mejorado

- Las exportaciones source-backed con protección `mask` o `hash` ya generan
  un snapshot Parquet protegido mediante DuckDB y transmiten desde él a los
  siete destinos locales, sin materializar el `DataFrame` activo.
- La protección conserva nulos, devuelve las columnas protegidas y mantiene
  la misma semántica de detección que las exportaciones materializadas.
- Las exportaciones source-backed vuelven a comprobar la fuente original al
  terminar, incluso cuando el trabajo utiliza un snapshot derivado de un libro
  XLSX/XLSB.

### Verificado

- La regresión de privacidad source-backed cubre máscara, SHA-256, nulos,
  columnas no protegidas, snapshot Parquet y fuente original intacta.

## [0.116.0] - 2026-09-01

### Mejorado

- Los libros XLSX/XLSB grandes ahora se abren source-backed: Calamine detecta
  el esquema en streaming y escribe un snapshot Parquet temporal por bloques,
  dejando el `DataFrame` activo vacío mientras se conservan la hoja, los tipos
  y la primera página.
- Paginación, perfilado, consultas DuckDB y exportaciones compatibles reutilizan
  ese snapshot y vuelven a validar el tamaño del libro original; al materializar
  una operación se lee el snapshot administrado, no se vuelve a cargar la hoja
  completa de forma implícita.

### Verificado

- La regresión de XLSX comprueba snapshot legible, tipos, preview, acceso como
  fuente Parquet, perfilado, materialización posterior, fuente intacta y
  cleanup del almacenamiento temporal.

## [0.115.0] - 2026-09-01

### Mejorado

- La exportación source-backed compatible a Excel escribe `.xlsx` por filas
  desde DuckDB para fuentes CSV/TSV/TXT delimitadas o Parquet, conservando el
  esquema y los tipos tabulares sin materializar el `DataFrame` activo.
- La exportación source-backed compatible a SQLite crea la tabla y sus tipos
  desde el esquema detectado, inserta en una transacción y publica el archivo
  de forma atómica; ambas salidas conservan cancelación, validación de cambios
  de la fuente y cleanup ante errores.

### Verificado

- Las regresiones Rust reabren el `.xlsx` y el SQLite generados, comprueban
  tipos, filas, texto que parece fórmula, fuente intacta y ausencia de
  artefactos temporales.

## [0.114.0] - 2026-09-01

### Mejorado

- Se añade `npm run brand:check`, un gate reproducible que inspecciona el árbol
  activo y evita que reaparezcan referencias a la marca retirada en código,
  documentación o archivos no ignorados.
- La política distingue el árbol vigente del historial Git: los commits antiguos
  no se reescriben ni se alteran con este cambio.

### Verificado

- El gate pasa sobre todos los archivos activos del repositorio y queda incluido
  en la validación de esta versión.

## [0.113.0] - 2026-09-01

### Mejorado

- La exportación source-backed compatible a Bundle ZIP usa DuckDB para escribir
  `dataset.csv` directamente desde CSV/TSV/TXT delimitado o Parquet, sin
  materializar el `DataFrame` activo.
- El Bundle construye `dictionary.json` con tipos y conteos de nulos calculados
  por agregaciones en disco, conserva el reporte de calidad incremental cuando
  existe y publica hashes del dataset, diccionario y reporte en un manifest.

### Verificado

- La regresión Rust confirma dataset, diccionario, conteos de nulos, publicación
  atómica, fuente intacta y ausencia de artefactos temporales privados.

## [0.112.0] - 2026-09-01

### Mejorado

- Las consultas locales source-backed compatibles ya mantienen la ejecución en
  DuckDB cuando la fuente supera el umbral de apertura: `INNER`, `LEFT` y
  `FULL JOIN` leen desde disco y solo conservan la página solicitada y los
  acumuladores necesarios.
- El camino de Polars deja de materializar silenciosamente un dataset
  source-backed cuando la consulta DuckDB no puede ejecutarse; devuelve un
  error explícito para proteger el presupuesto de memoria y la integridad de
  la fuente.

### Verificado

- El benchmark reproducible de 512 MiB cubre los tres tipos de JOIN, conteos
  exactos, paginación, frame activo vacío, working set de 200.925.184 bytes y
  cleanup confirmado.

## [0.111.0] - 2026-09-01

### Mejorado

- La exportación source-backed a SQL usa DuckDB directamente sobre CSV, TSV,
  TXT delimitado o Parquet cuando no hay receta, privacidad adicional ni reglas
  de calidad no incrementales, sin materializar el `DataFrame` activo.
- El script SQL conserva el esquema inferido, escapa valores textuales,
  neutraliza tipos complejos mediante texto y publica `DROP`/`CREATE`/`INSERT`
  dentro de una transacción portable.

### Verificado

- La salida SQL se genera con cancelación cooperativa, validación de cambios de
  la fuente, scratch privado, copia temporal y publicación atómica; las rutas
  que necesitan transformaciones o protección adicional conservan el camino
  materializado.

## [0.110.0] - 2026-09-01

### Mejorado

- La exportación source-backed a CSV usa DuckDB directamente sobre CSV, TSV,
  TXT delimitado o Parquet, conserva la publicación atómica y evita cargar el
  `DataFrame` completo cuando no hay receta, privacidad adicional ni reglas no
  incrementales.

### Verificado

- La ruta CSV aplica cancelación cooperativa durante la conversión y copia
  temporal, valida que la fuente no cambie y neutraliza prefijos de fórmulas de
  hoja de cálculo antes de publicar el archivo.

## [0.109.0] - 2026-09-01

### Mejorado

- La apertura source-backed conserva el esquema y la primera página con Polars,
  pero calcula el conteo total directamente desde el archivo mediante DuckDB,
  con cancelación cooperativa y sin materializar las filas en el `DataFrame`
  activo.

### Verificado

- El benchmark end-to-end de `perf:duckdb:join` mantiene una fuente CSV temporal
  de al menos 512 MiB, comprueba el conteo exacto, un `LEFT JOIN`, la paginación,
  el frame source-backed vacío y el cleanup dentro del presupuesto de 512 MiB.

## [0.108.0] - 2026-09-01

### Verificado

- Se incorpora `perf:duckdb:join`, un benchmark opt-in que genera una fuente
  CSV temporal de al menos 512 MiB, ejecuta un `LEFT JOIN` source-backed desde
  DuckDB, conserva el frame activo sin filas y valida conteo, paginación,
  working set de 512 MiB y cleanup sin guardar datos del benchmark.

## [0.107.0] - 2026-09-01

### Mejorado

- Los `JOIN` DuckDB con una comparación Parquet activa pueden combinar el
  `DataFrame` actual con el snapshot en disco, evitando materializar de nuevo
  todas las filas comparadas.

### Verificado

- Polars promueve los `JOIN` compatibles cuando existe snapshot comparado y
  conserva el fallback seguro si el contrato o la fuente no son válidos.
- Una regresión comprueba paridad de filas, nulos y orden en un `LEFT JOIN`
  entre un frame activo y un snapshot Parquet.

## [0.106.0] - 2026-09-01

### Mejorado

- El workspace durable de cada proyecto conserva y restaura el formato de
  exportación, la protección de datos personales, las columnas clave de
  comparación y el tipo de `JOIN`.
- Las claves restauradas se filtran contra las columnas del snapshot activo,
  sin reactivar selecciones obsoletas ni guardar muestras o valores privados.

### Verificado

- SQLite migra el catálogo a v12 y Rust valida los cuatro contratos con listas
  cerradas antes de publicar snapshots o reemplazar el proyecto.
- Los catálogos anteriores usan los valores locales seguros y la reapertura
  conserva compatibilidad con sus workspaces existentes.

## [0.105.2] - 2026-09-01

### Corregido

- Las consultas DuckDB source-backed conservan el orden global de un `FULL
  JOIN` cuando el esquema activo se mantiene sin filas en memoria; las filas
  exclusivas del dataset comparado quedan después de las filas activas.

### Verificado

- La regresión ejecuta el `FULL JOIN` desde snapshots Parquet con esquemas
  vacíos y comprueba conteo, nulos, columnas y orden sin materializar el
  dataset activo para preparar la consulta.

## [0.105.1] - 2026-09-01

### Limpieza

- El componente interno de vista previa ahora usa el nombre neutral
  `DatasetPreviewPanel`, evitando coincidencias con la marca retirada en
  búsquedas del repositorio.

## [0.105.0] - 2026-09-01

### Mejorado

- El perfil de rendimiento (`Ahorro`, `Equilibrado` o `Máximo`) se guarda por
  proyecto y se restaura al abrirlo; los proyectos antiguos mantienen la
  preferencia local como fallback.

### Verificado

- SQLite migra el catálogo a v11 y Rust rechaza perfiles desconocidos antes de
  publicar snapshots o sustituir el proyecto activo.
- El cambio de dataset o el borrado del proyecto devuelve el monitor a la
  preferencia local, sin sobrescribirla desde un workspace ajeno.

## [0.104.0] - 2026-09-01

### Mejorado

- El motor SQL elegido en Revisar (`Polars` o `DuckDB`) se guarda por proyecto
  y se restaura al abrirlo, manteniendo la preferencia local como fallback para
  catálogos anteriores.

### Verificado

- SQLite migra el catálogo a v10 y Rust rechaza motores desconocidos antes de
  publicar snapshots o sustituir el proyecto activo.
- La reapertura conserva el motor junto con la pestaña, etapa, página y
  cobertura de correlaciones del workspace.

## [0.103.0] - 2026-09-01

### Mejorado

- La cobertura de filas usada por las correlaciones (`10.000`, `50.000` o
  `100.000`) se guarda y restaura dentro del workspace durable de cada proyecto.
- Los catálogos anteriores conservan compatibilidad: cuando no tienen esta
  preferencia, Columnia usa el valor local seguro y lo incorpora al guardar.

### Verificado

- SQLite migra el catálogo a v9 y rechaza valores de cobertura fuera del
  conjunto permitido sin sustituir el proyecto activo.
- La reapertura desde el catálogo restaura la preferencia junto con la pestaña,
  etapa y página visible de Revisar.

## [0.102.0] - 2026-09-01

### Mejorado

- Las recetas source-backed interpretan fechas ISO seguras (`YYYY-MM-DD`,
  fecha-hora sin offset y sufijo UTC `Z`) directamente sobre CSV, TSV, TXT
  delimitado o Parquet mediante DuckDB.
- Los valores con offsets distintos de UTC o ISO inválidos conservan el
  fallback eager para mantener la semántica estricta y no convertir errores en
  nulos.

### Verificado

- Regresiones comparan fechas ISO ingenuas/UTC con eager y comprueban el
  fallback para offsets no UTC.

## [0.101.0] - 2026-09-01

### Mejorado

- Los tratamientos IQR source-backed (`cap`, `drop` e `impute`) se ejecutan
  directamente sobre CSV, TSV, TXT delimitado o Parquet mediante DuckDB,
  después de filtros y etapas compatibles.
- La ruta calcula un baseline común, conserva nulos y tipos cuando corresponde,
  valida mínimo de valores, finitud, precisión y umbrales, y separa los
  contadores de celdas ajustadas, filas retiradas y filas eliminadas por filtros.

### Verificado

- Regresiones comparan los tres modos IQR con eager y comprueban el baseline
  posterior a filtros y la separación de contadores.

## [0.100.0] - 2026-09-01

### Mejorado

- Las recetas source-backed pueden agrupar y resumir directamente sobre CSV,
  TSV, TXT delimitado o Parquet mediante DuckDB, después de filtros y etapas
  estructurales compatibles.
- Los resúmenes conservan el primer orden de aparición, agrupan valores nulos
  juntos y soportan `sum`, `mean`, `min`, `max`, `count` y `count_unique`, con
  validación de precisión, overflow y valores numéricos no finitos.
- El resultado se publica como snapshot Parquet privado sin llenar el
  `DataFrame` activo y reporta grupos, agregaciones, filas eliminadas y filas
  colapsadas con contadores separados.

### Verificado

- Una regresión compara la salida source-backed con la ruta eager, incluyendo
  grupos estables, clave nula, agregaciones mixtas y contadores de resultado.

## [0.99.0] - 2026-08-31

### Mejorado

- Las recetas source-backed pueden normalizar correo, teléfono y dirección
  directamente en DuckDB, después de filtros y etapas estructurales compatibles,
  y publicar el snapshot Parquet resultante.
- La normalización conserva nulos, espacios Unicode, prefijos telefónicos,
  renombrados y `keepColumns`, y las extracciones textuales posteriores pueden
  consumir los valores normalizados.

### Verificado

- Una regresión compara los tres tipos de contacto y una extracción posterior
  con la ruta eager, incluyendo conteo exacto de celdas modificadas.

## [0.98.0] - 2026-08-31

### Mejorado

- Las recetas source-backed pueden extraer tokens, dígitos, letras y segmentos
  antes o después de un delimitador directamente en DuckDB, después de las
  etapas compatibles de la receta, y publicar el snapshot Parquet resultante.
- La extracción conserva la semántica eager para Unicode, nulos, coincidencias
  ausentes, delimitadores Unicode y resultados vacíos, además de renombrados y
  `keepColumns`.

### Verificado

- Una regresión compara las seis variantes de extracción source-backed con la
  ruta eager usando una fuente Parquet con texto Unicode, nulos y segmentos
  vacíos.

## [0.97.0] - 2026-08-31

### Mejorado

- Las recetas source-backed pueden dividir una columna de texto en entre dos y
  dieciséis destinos directamente en DuckDB, después de filtros, casts,
  reemplazos y cálculos compatibles, y publicar el snapshot Parquet resultante.
- La división conserva delimitadores Unicode, segmentos vacíos, nulos, el resto
  en el último destino, `keepColumns`, renombrados y `dropSource`; también puede
  alimentar una unión posterior dentro de la misma consulta.

### Verificado

- Una regresión compara la división y unión source-backed con la ruta eager
  usando una fuente Parquet con resto, valores nulos y segmentos vacíos.

## [0.96.0] - 2026-08-31

### Mejorado

- Las recetas source-backed pueden unir entre dos y dieciséis columnas de
  texto directamente en DuckDB, después de filtros, casts y reemplazos
  compatibles, y publican el resultado en un snapshot Parquet privado.
- La unión conserva el orden de las fuentes, nulos, cadenas vacías, separador,
  renombrados, `keepColumns` y el conteo de columnas eliminadas por
  `dropSources`.

### Verificado

- Una regresión compara la unión source-backed con la ruta eager usando una
  fuente Parquet con valores nulos, cadenas vacías y una columna numérica
  convertida a texto.

## [0.95.0] - 2026-08-31

### Mejorado

- Las recetas source-backed ejecutan búsqueda y reemplazo literal sobre la
  fuente mediante DuckDB después de aplicar filtros y antes de proyectar o
  calcular columnas.
- La ruta conserva renombrados, `keepColumns`, nulos y columnas convertidas a
  texto, y obtiene el conteo exacto de celdas modificadas con una consulta
  acotada adicional.

### Verificado

- Una regresión compara filtro, renombrado y reemplazo contra la receta eager,
  incluyendo un literal con apóstrofe y el orden de las operaciones.
- Las expresiones regulares y los literales con caracteres no válidos mantienen
  el fallback existente para no cambiar su semántica.

## [0.94.0] - 2026-08-31

### Mejorado

- Las recetas source-backed pueden extraer año, mes y día después de un
  parseo de fecha fijo (`YMD`, `DMY` o `MDY`) directamente en DuckDB.
- La ruta valida que la fuente sea una fecha compatible, conserve la
  dependencia en `keepColumns` y mantenga el fallback materializado para
  filtros, conversiones conflictivas o fechas no representables.

### Verificado

- Una regresión compara año, mes y día source-backed con la ruta eager y
  confirma que el resultado publicado mantiene el snapshot Parquet privado.

## [0.93.0] - 2026-08-31

### Mejorado

- Las recetas source-backed amplían su ejecución directa en DuckDB a casts,
  parseo de fechas `YMD`, `DMY` y `MDY`, y columnas calculadas de suma, resta,
  multiplicación y concatenación.
- La ruta conserva renombres, filtros, orden, `keepColumns`, nulos y la
  publicación atómica en un snapshot Parquet privado; las divisiones, partes
  de fecha y formatos ISO continúan en el fallback materializado para preservar
  sus validaciones estrictas.

### Verificado

- La regresión nueva compara la combinación de renombrado, cast, fecha y cálculo
  contra la receta eager sin materializar las filas del dataset source-backed.
- La suite Rust completa queda en 316 pruebas aprobadas.

## [0.92.0] - 2026-08-31

### Mejorado

- Las recetas source-backed pueden combinar hasta tres filtros con selección y
  renombrado de columnas. DuckDB ejecuta el plan directamente sobre CSV/TSV/TXT
  delimitado o Parquet y publica un snapshot Parquet privado sin materializar
  todas las filas en el `DataFrame` activo.
- La nueva ruta valida operadores, valores numéricos, nombres finales, columnas
  duplicadas y cambios de la fuente, y conserva el conteo exacto de filas
  eliminadas, el orden y la semántica de nulos.

### Verificado

- La regresión source-backed compara filtros numéricos, texto con apóstrofes y
  nulos contra la receta eager; la suite Rust completa queda en 314 pruebas.

## [0.91.0] - 2026-08-31

### Añadido

- Se incorpora una ficha estructurada de decisiones legales y de distribución,
  con un gate técnico que exige completar la aprobación antes de los perfiles
  `Release` y `Package`.
- El panel accesible de licencia y privacidad expone una región etiquetada para
  que el contenido sea descubrible por teclado y tecnologías de asistencia.

### Mejorado

- El inventario de avisos consolida paquetes repetidos por identidad y conserva
  todas sus fuentes, rechazando filas incompletas, licencias desconocidas y
  contradicciones.
- La auditoría documental queda alineada con 995 dependencias, 65 comandos de
  producción, 4 de depuración y 58 estructuras compartidas.

## [0.90.0] - 2026-08-31

### Eliminado

- Se retiró completamente la compatibilidad con sesiones, recetas, comandos,
  fixtures y documentación del sistema externo.
- El bridge IPC, la interfaz y el catálogo de proyectos ya exponen únicamente
  contratos nativos de Columnia.

### Mejorado

- El inventario IPC queda sincronizado en 65 comandos de producción, 4 de
  depuración y 58 estructuras compartidas.
- Se conserva la carga de contratos legacy de calidad sin mantener una
  integración de sesiones externa.

## [0.80.0] - 2026-08-31

### Mejorado

- Las recetas source-backed compuestas por renombres y selección/orden de
  columnas ahora se convierten directamente desde CSV, TSV, TXT delimitado o
  Parquet a un snapshot Parquet privado, sin materializar todas las filas en el
  `DataFrame` activo.
- El resultado conserva conteo, esquema, orden, preview y validación de la
  fuente; las recetas que requieren transformar valores mantienen el fallback
  eager explícito.
- La suite Rust queda en 363 pruebas aprobadas, incluida la regresión de
  paridad de la receta source-backed.

## [0.70.0] - 2026-08-31

### Mejorado

- La validación de calidad de fuentes source-backed ya no materializa el
  dataset completo para reglas globales de unicidad simple/compuesta,
  monotonicidad, agregación y deriva de distribución.
- Las claves y acumuladores se procesan por bloques con derrama temporal
  acotada, conservando la semántica de nulos, orden y cancelación cooperativa.
- La suite Rust queda en 362 pruebas aprobadas, incluida una regresión que
  verifica duplicados que cruzan el límite de un bloque.

## [0.69.0] - 2026-08-31

### Mejorado

- La exportación JSON de fuentes source-backed puede leer directamente CSV,
  TSV, TXT delimitado o Parquet desde disco cuando no se solicita receta,
  privacidad ni reglas globales, sin materializar el \`DataFrame\` activo.
- La salida conserva la conversión controlada por DuckDB, el orden de origen,
  la publicación atómica, la validación de cambios y el cleanup de temporales.
- La suite Rust queda en 361 pruebas aprobadas, incluida la regresión de
  exportación JSON source-backed.

## [0.68.0] - 2026-08-31

### Mejorado

- La exportación Parquet de fuentes source-backed puede leer directamente la
  fuente desde disco y publicar un archivo temporalmente aislado, sin
  materializar el \`DataFrame\` activo, cuando no se solicita receta ni
  protección adicional. La validación de calidad compatible se ejecuta antes
  por bloques y la conversión respeta la frontera de recursos de DuckDB.
- Se conserva la publicación atómica, la comprobación de cambios de la fuente,
  la cancelación cooperativa entre etapas y el cleanup del snapshot temporal.
- La suite Rust queda en 360 pruebas aprobadas, incluida la regresión de
  exportación Parquet source-backed.

## [0.67.0] - 2026-08-31

### Mejorado

- La validación de calidad de fuentes source-backed evalúa por bloques las
  reglas fila-a-fila y las comprobaciones de esquema/conteo, sin materializar
  el dataset completo ni mutar la sesión activa; comprueba el tamaño de la
  fuente antes y después y conserva el fallback materializado para reglas
  globales.
- La suite Rust queda en 359 pruebas aprobadas, incluyendo la regresión de
  paridad entre validación source-backed y validación en memoria.

## [0.66.0] - 2026-08-31

### Mejorado

- El perfilado de fuentes source-backed ya no materializa el dataset completo:
  las filas duplicadas se indexan por cubetas temporales, cada columna se
  procesa por bloques y las estadísticas numéricas usan corridas ordenadas en
  disco para cuantiles, histogramas y atípicos.
- Los resúmenes categóricos y temporales recorren solo la columna necesaria y
  las correlaciones leen una muestra acotada directamente desde Parquet; se
  conserva la semántica del perfil normal, cancelación cooperativa y validación
  del tamaño de la fuente.
- La suite Rust queda en 358 pruebas aprobadas, incluyendo la regresión de
  paridad entre perfil source-backed y perfil en memoria.

## [0.65.0] - 2026-08-31

### Mejorado

- Los CSV, TSV, TXT delimitados y Parquet de al menos 512 MiB abren ahora en
  modo source-backed: Columnia conserva el esquema y la primera página de 50
  filas, calcula el total sin retener el dataset completo y sirve la
  paginación directamente desde disco mientras la fuente permanece intacta.
- Perfilado, calidad, exportación, recetas, limpiezas, joins y demás operaciones
  que necesitan todas las filas materializan la fuente bajo demanda, validando
  que no haya cambiado; después de una mutación la referencia source-backed se
  invalida para no consultar datos obsoletos.
- La carga diferida mantiene un historial explícitamente degradado hasta la
  primera materialización y conserva el fallback seguro para formatos o rutas
  incompatibles. La suite Rust queda en 357 pruebas aprobadas.

## [0.64.0] - 2026-08-31

### Mejorado

- La conversión source-backed de CSV, TSV, TXT delimitado y JSON a snapshots
  Parquet reutiliza la frontera de recursos de DuckDB: 512 MB de memoria,
  hasta 8 GB de derrame temporal privado y cleanup automático. Así la
  preparación de snapshots no evade el presupuesto aplicado a las consultas.
- La suite Rust cubre también la publicación y reapertura de un snapshot creado
  desde una fuente delimitada bajo esa frontera.

## [0.63.0] - 2026-08-31

### Mejorado

- La ruta DuckDB de consultas locales configura por consulta un límite de
  memoria de 512 MB y un directorio privado de derrame temporal de hasta 8 GB.
  Esto permite que filtros, agregaciones y joins source-backed cedan memoria a
  disco cuando el plan lo necesita, manteniendo cancelación, aislamiento y
  cleanup automático.

## [0.62.0] - 2026-08-31

### Mejorado

- La comparación inicial reutiliza el snapshot Parquet administrado del dataset
  activo cuando existe y, si no, convierte una fuente original Parquet o
  CSV/TSV/TXT intacta a un snapshot temporal. Ambos lados se comparan por
  bloques e índices temporales sin clonar el `DataFrame` activo; si una fuente
  cambia o el camino source-backed falla, se conserva el fallback materializado
  seguro, incluido el soporte compatible de XLS/ODS.

## [0.61.0] - 2026-08-31

### Mejorado

- La vista previa de un dataset intacto con historial degradado puede leer
  únicamente la página solicitada desde la fuente original Parquet o CSV/TSV/TXT,
  validando el tamaño de la fuente y la cantidad esperada de filas de la página
  antes de responder; si la fuente no coincide o el formato no es compatible,
  conserva el fallback seguro al frame.

## [0.60.0] - 2026-08-31

### Mejorado

- Las consultas Polars con `JOIN` compatible usan automáticamente DuckDB cuando
  el activo y la comparación tienen snapshots o fuentes de disco válidas,
  incluso por debajo del umbral de datasets grandes. Se conserva el fallback
  local seguro si alguna fuente no puede registrarse.

## [0.59.0] - 2026-08-31

### Añadido

- Cuando el historial se degrada por presupuesto y la fuente original CSV, TSV,
  TXT delimitada o Parquet permanece intacta, una consulta compatible elegida
  como Polars intenta automáticamente DuckDB desde disco, incluidos los JOINs
  con snapshots comparados. Si la consulta o la fuente no son compatibles,
  conserva el fallback materializado seguro.

## [0.58.0] - 2026-08-31

### Añadido

- El benchmark `perf:webview2` valida carga, paginación, transformación y
  exportación de un dataset sintético de 100 MiB en WebView2, con memoria dentro
  del presupuesto contractual y cleanup confirmado; la ejecución general fuera
  de RAM continúa explícitamente pendiente.
- DuckDB puede consultar directamente desde disco la fuente original CSV, TSV,
  TXT delimitada o Parquet cuando el historial se degrada por presupuesto y el
  dataset sigue intacto; los JOINs grandes pueden combinarla con el snapshot
  comparado sin reserializar el activo. Una mutación invalida la referencia y
  conserva el fallback materializado seguro.
- La paginación de conflictos de Revisar lee el snapshot Parquet comparado en
  bloques de 16K: mantiene un índice temporal global de claves para respetar
  duplicados entre bloques y conserva solo el bloque de valores necesario para
  construir la página visible. También valida el conteo registrado y rechaza
  snapshots que cambien durante la lectura.
- Los perfiles Cargo `dev` y `test` omiten símbolos de depuración para que
  `npm run tauri dev` y las pruebas nativas puedan enlazar de forma reproducible
  en Windows sin alcanzar `LNK1140`; el perfil `release` conserva su política
  independiente de símbolos.
- Revisar permite elegir el límite de filas usado por la matriz de correlaciones
  numéricas (10.000, 50.000 o 100.000) y conserva la preferencia localmente;
  Rust valida el rango de 1.000 a 100.000, invalida cachés con una cobertura
  distinta y mantiene el resto del perfil agregado sin cambios.
- Revisar recuerda localmente el motor SQL elegido (`Polars` o `DuckDB`) entre
  aperturas, valida cualquier valor persistido y vuelve a `Polars` si el
  almacenamiento no está disponible o contiene una opción desconocida.
- Revisar expone la cobertura agregada de análisis conservada al importar una
  sesión sistema anterior: indica si el análisis fue muestreado y sus conteos de filas
  cuando están disponibles. El perfil actual se recalcula sobre el dataset
  activo y la UI deja explícito que no se restauran filas, valores ni resultados
  originales.
- La migración de recetas sistema anterior reconoce el campo `selected` que emiten los
  pipelines persistidos, además de `selected_cleaning_operations` y su alias
  camelCase; las operaciones se normalizan al catálogo determinista canónico y
  no se pierden al importar un pipeline real.
- `project-save --recipe` reproduce ahora las limpiezas deterministas
  seleccionadas en un pipeline sistema anterior antes de aplicar su receta estructural,
  validando el resultado completo antes de publicar el snapshot del proyecto.
- El catálogo de limpieza sugerida queda alineado con las 22 operaciones
  registradas por `sistema anterior`; la lista canónica comparte el orden del replay
  de sesiones y una regresión evita que futuras operaciones queden sin mapping.
- La importación de sesiones sistema anterior reconoce también el campo real `filename`
  como referencia local reproducible cuando el manifiesto no incluye
  `source_path`, incluyendo su validación nativa y el nombre seguro del dataset.
- La actividad de sesiones sistema anterior acepta los estados reales `completed` y
  `failed`, además de sus aliases compatibles, y convierte `rows_out`/`rowsOut`
  en el conteo agregado de filas; las duraciones decimales se redondean de forma
  segura y nunca se conservan consultas, rutas ni valores.
- Se añade una fixture v3 con la forma emitida por `SessionRecipe` de sistema anterior
  y una regresión de proyecto que verifica metadatos agregados de muestreo,
  etapa, reglas y actividad `ExecutionHistory` tras reabrir; los resultados,
  cachés, consultas y rutas privadas siguen descartándose.
- Se añade una fixture de round-trip con historial Parquet explícito que verifica
  importación, cursor, etapa, actividad y Deshacer/Rehacer después de reabrir;
  resultados de análisis y cachés reanudables continúan marcados como no
  portables y no se copian al workspace.
- El replay de limpiezas sistema anterior dependientes del perfil (`drop_high_null_cols`
  y `drop_id_cols`) conserva las métricas del dataset inicial aunque antes se
  ejecute `drop_duplicates`; evita eliminar columnas solo porque la deduplicación
  cambió su cardinalidad o porcentaje de nulos.
- El replay de sesiones sistema anterior aplica la semántica de `normalize_text` del
  limpiador original: omite columnas de texto con más de 50% de valores
  distintos, conserva nulos y aplica título a columnas cuyo nombre sugiere
  nombres propios. La normalización directa de Columnia mantiene su contrato
  independiente, sin cambiar el comportamiento de la acción manual.
- La importación, guardado y apertura de historiales `history_snapshots` copia
  cada Parquet de forma secuencial y conserva sus bytes originales; las
  revisiones que no son el cursor validan solo footer/esquema y se leen bajo
  demanda al hacer undo/redo. Únicamente el cursor se materializa para
  comprobar igualdad con el dataset activo, reduciendo el pico de RAM sin
  relajar los límites de disco, ruta, etiqueta, cancelación y publicación
  atómica. La ejecución incremental del dataset activo y el presupuesto global
  de datasets grandes siguen fuera de este bloque.
- El bridge declara ahora `nonPortableArtifacts` y valida estrictamente los
  metadatos opcionales de sesión sistema anterior: contadores enteros seguros no
  negativos, banderas booleanas y listas de categorías sin valores arbitrarios.
  Los artefactos no portables siguen siendo señales sanitizadas y no se
  convierten en datos operativos.
- La consulta SQL local ofrece un motor DuckDB explícito además de Polars:
  ejecuta el contrato restringido de solo lectura sobre el snapshot Parquet
  administrado de la revisión actual cuando está disponible, evitando
  serializar otra vez el `DataFrame`; conserva conteo exacto, paginación de
  hasta 200 filas, orden estable, agregaciones y JOIN `INNER`/`LEFT`/`FULL` con
  el mismo esquema coalescido. La cancelación interrumpe la consulta nativa y
  las columnas auxiliares de orden nunca se publican; el fallback a snapshot
  temporal se mantiene para historiales degradados. Esta ruta aún no completa
  la carga inicial ni la ejecución incremental fuera de memoria.
- La ruta Polars de consultas simples sin comparación puede leer el snapshot
  Parquet del cursor actual por bloques de 16K filas: cuenta coincidencias sin
  materializar una segunda copia del dataset y solo relee los bloques que
  contienen la página solicitada o el estado agregado. Verifica que el
  snapshot conserve exactamente el conteo activo, respeta cancelación y hace
  fallback al `DataFrame` materializado ante un snapshot inválido; `JOIN`,
  comparación, historial degradado y ejecución integral fuera de RAM conservan
  sus límites explícitos.
- La comparación local conserva la segunda fuente en un snapshot Parquet
  temporal mientras está activa: conflictos y consolidación lo leen bajo
  demanda, DuckDB puede registrar directamente ambos snapshots en un JOIN y
  el directorio se elimina al descartar la comparación. Cuando la fuente
  comparada ya es Parquet, el archivo se copia secuencialmente y la comparación
  calcula conteos, claves y la primera página de conflictos por bloques de 16K,
  sin materializar otra copia completa; los formatos Excel sin lector secuencial
  conservan su camino materializado.
- La comparación inicial de fuentes CSV, TSV, TXT delimitadas y JSON usa el
  lector DuckDB bundled para crear el snapshot Parquet temporal de forma
  secuencial y después reutiliza la comparación por bloques; la proyección
  conserva el orden de columnas y serializa campos anidados con el mismo texto
  JSON que el lector nativo. No crea un `DataFrame` completo de la fuente
  secundaria; `.xls` y `.ods` mantienen el camino materializado compatible.
- La comparación inicial de libros `.xlsx` y `.xlsb` usa el lector secuencial
  de celdas de Calamine en dos pasadas: detecta el esquema y escribe el
  snapshot Parquet en bloques de 16K, sin crear un `DataFrame` completo de la
  fuente comparada. `.xls` y `.ods` conservan el fallback compatible porque
  Calamine no ofrece lectura lazy para esos formatos.
- La preparación de JOIN para DuckDB ya lee solo el esquema del snapshot
  comparado cuando existe un snapshot activo administrado, y no vuelve a
  materializar sus filas para validar el contrato. La ruta DuckDB tampoco
  hereda el límite de entradas del plan Polars; el `DataFrame` activo y la
  ejecución incremental completa fuera de RAM siguen siendo límites abiertos.
- Cuando Polars detecta un JOIN que supera el límite de entradas y existen los
  snapshots Parquet administrados del activo y la comparación, Revisar lo
  promueve automáticamente al camino DuckDB sobre esos archivos. Así se evita
  recargar la segunda fuente completa y se conserva el límite del resultado;
  sin ambos snapshots permanece el rechazo seguro de la ruta Polars.
- La paginación de la muestra activa reutiliza el snapshot Parquet del cursor
  actual con `slice` y colección streaming cuando el historial está disponible;
  los estados degradados conservan el fallback al `DataFrame` activo.
- Guardar un proyecto entrega la copia ya aislada del dataset directamente al
  escritor Parquet, eliminando una segunda clonación completa del `DataFrame`
  durante la publicación durable.
- Las recetas y la migración sistema anterior admiten `find_replace` con expresiones
  regulares seguras, grupos de captura en el reemplazo y validación nativa del
  patrón antes de modificar el dataset; los patrones inválidos siguen fallando
  cerrado sin perder la semántica del pipeline.
- El benchmark de datasets compila el CLI con el perfil debug sin símbolos y
  restaura la configuración del proceso, evitando el fallo de enlace MSVC
  `LNK1140` sin alterar las mediciones de runtime.
- Las recetas compatibles con Polars lazy/streaming también convierten fechas
  con formatos explícitos `Ymd`, `Dmy` y `Mdy`, respetando espacios exteriores,
  nulos y objetivos `Date`/`Datetime`; `Iso8601` sin offset o con sufijo UTC
  `Z` también usa streaming, mientras offsets distintos de UTC, zonas horarias
  y operaciones avanzadas conservan el fallback eager seguro.
- Los tratamientos IQR aislados de recetas (`cap`, `impute` y `drop`) ahora
  pueden ejecutarse en lazy/streaming con conteos exactos de celdas y filas,
  calculando sus umbrales después de filtros previos; las etapas que alteran
  valores antes del cálculo conservan fallback eager.
- Los tratamientos IQR lazy/streaming también pueden combinarse con
  `keepColumns` cuando la proyección conserva todas sus columnas tratadas; si
  una proyección elimina una dependencia, se conserva el fallback eager y la
  validación cerrada.
- Las recetas lazy pueden combinar parseos de fecha con conversiones en columnas
  distintas, y también dividir y combinar columnas en la misma ejecución; las
  dependencias incompatibles, como descartar antes una fuente que aún necesita
  un `merge`, conservan el fallback o rechazo explícito.
- La migración M1 de sesiones sistema anterior restaura un historial explícito de hasta
  doce snapshots Parquet locales mediante el contrato versionado
  `history_snapshots`: valida etiquetas, referencias regulares, presupuesto de
  disco y cursor contra el dataset actual, y publica las revisiones dentro de
  la generación durable del proyecto sin exponer rutas.
- La importación nativa de sesiones sistema anterior publica progreso por etapas de
  validación, carga, replay, restauración de historial, perfil y publicación;
  la operación `migration` admite cancelación cooperativa aislada y nunca
  publica un proyecto parcial cuando se cancela antes del commit atómico.
- Proyectos expone la importación de sesiones sistema anterior con progreso visible y
  cancelación desde la interfaz, además del selector nativo y la CLI existentes.
- Los `JOIN` locales `INNER` y `LEFT` sin agregación procesan el lado `dataset`
  por bloques y conservan el conteo, orden y `OFFSET`/`LIMIT` globales sin
  acumular el `DataFrame` unido completo.
- Las agregaciones sobre `JOIN` locales `INNER` y `LEFT` también procesan bloques
  y fusionan estados de `COUNT`/`SUM`/`AVG`/`MIN`/`MAX` y grupos sin acumular el
  `DataFrame` unido completo.
- Los `JOIN` locales `FULL` procesan el lado `dataset` por bloques y recorren las
  filas derechas no emparejadas por bloques de 16K usando un índice temporal de
  claves, sin materializar el anti-join derecho completo. La paginación y las
  agregaciones cubren ambos lados sin acumular el resultado unido completo,
  conservando `NULL` como no emparejado, duplicados y orden de entrada; los
  `DataFrame` fuente y la ejecución general fuera de RAM mantienen sus límites.
- El preflight de cardinalidad de los `JOIN` locales usa índices temporales de
  claves particionados y cuenta por cubeta los productos de duplicidad, con
  cancelación cooperativa y el mismo rechazo explícito de resultados excesivos.
- La comparación completa de filas calcula el multiconjunto de filas comunes por
  cubetas temporales, conservando los conteos exactos sin retener mapas globales
  de firmas de ambos datasets.
- La comparación por claves particiona su índice exacto en 256 cubetas temporales
  y procesa una cubeta a la vez para el resumen, las nuevas claves y los
  conflictos paginados, conservando orden, duplicados y resultados sin retener
  todos los índices de ambos datasets en memoria.
- Las agregaciones SQL locales procesan su segunda pasada por bloques y conservan
  solo estados de `COUNT`/`SUM`/`AVG`/`MIN`/`MAX` y grupos, sin retener índices de
  todas las filas coincidentes; el presupuesto explícito y los resultados exactos
  se mantienen.
- Los `JOIN` de la consulta SQL local admiten hasta ocho pares de columnas clave
  entre `dataset` y `compared`; rechazan claves repetidas, conservan el preflight
  de tipos/cardinalidad y mantienen el orden estable y los límites de materialización.
- La consulta SQL local admite `GROUP BY` compuesto de hasta ocho columnas, con
  orden estable de primera aparición, claves nulas, rechazo de claves duplicadas
  y el mismo presupuesto acotado de filas para agregaciones.
- Preparar detecta outliers numéricos mediante IQR y permite reemplazarlos de
  forma reversible por la mediana observada, conservando tipos `Int64`/`Float64`,
  `_cambios` y un impacto agregado. La misma regla está disponible en recetas
  como `impute`; el inventario IPC pasa a 60 comandos de producción.
- Preparar ofrece además una imputación categórica explícita y reversible que
  completa nulos textuales como `Desconocido`, sin tocar números ni `_cambios`,
  con impacto agregado; el inventario IPC actual pasa a 61 comandos de producción.
- Preparar ofrece acciones IQR confirmables para limitar valores atípicos o
  eliminar las filas que excedan los límites, con historial reversible, impacto
  agregado y sin mostrar celdas; el inventario IPC pasa a 63 comandos de producción.
- Preparar ofrece una acción confirmable para proteger valores no nulos de
  correo, teléfono, dirección y nombre detectados, sustituyéndolos por
  `[REDACTED]` sin tocar identificadores, números, nulos ni `_cambios`; conserva
  las columnas, reporta solo conteos agregados y permite deshacer desde el
  historial. El inventario IPC pasa a 64 comandos de producción y 58 estructuras.
- Preparar ofrece una acción reversible para interpretar columnas de texto con
  fechas detectadas: solo usa un formato dominante cerrado, omite columnas
  ambiguas y publica únicamente el impacto agregado. El inventario IPC pasa a
  65 comandos de producción.
- Preparar ofrece una acción reversible para convertir texto numérico con más
  de 90% de coincidencia: rechaza pérdida de precisión y conserva
  identificadores y códigos con ceros iniciales. El inventario IPC pasa a 66
  comandos de producción.
- La cobertura M1 añade un round-trip de sesión contra un libro `.xlsx` real:
  genera el libro con el exportador nativo, valida la hoja registrada, importa
  la receta, reabre el proyecto y comprueba esquema, conteos y etapa activa.
- La importación M1 conserva hasta cinco entradas de historial de ejecución solo
  cuando contienen estado, duración y filas agregadas; asigna IDs locales y no
  copia consultas, rutas, valores ni entradas inválidas.
- La importación de sesiones sistema anterior reproduce `drop_duplicates`,
   `drop_fuzzy_duplicates`,
   `drop_high_null_cols`, `drop_id_cols`, `drop_empty_cols`, `drop_constant_cols`,
   `drop_empty_rows`, `normalize_sentinels`, `impute_numeric`, `impute_categorical`, `parse_dates`, `trim_text`, `fix_encoding`, `cast_numeric`, `cap_outliers`, `impute_outliers` y `drop_outliers`,
   `normalize_text`, `normalize_booleans`, `mask_pii`, `normalize_columns` y `add_cambios_col` durante el fallback a la fuente
   cuando no hay snapshot compatible, respetando el orden fijo de limpieza; un snapshot disponible
   conserva prioridad para evitar reaplicar operaciones sobre un estado
   materializado.
- El replay M1 rechaza antes de publicar una sesión que combine las estrategias
  IQR incompatibles `cap_outliers`, `impute_outliers` y `drop_outliers`, igualando
  la exclusividad del catálogo sistema anterior.
- El catálogo sistema anterior `selected_cleaning_operations` se migra por aliases canónicos:
  las limpiezas deterministas, incluido `mask_pii` en su modo `mask` predeterminado,
  se reproducen desde la fuente y se conservan en el informe de sesión. Los modos
  hash/clave explícita no se inventan y las operaciones sin equivalente quedan como
  advertencias accionables, sin descartarse silenciosamente.
- La importación de sesiones sistema anterior convierte etiquetas de etapa conocidas
  (`Cargar`, `Revisar`, `Preparar`, `Entregar` y equivalentes de análisis,
  transformación o exportación) en la etapa activa del workspace; etiquetas
  desconocidas conservan el fallback seguro a Revisar.
- El preflight de sesiones sistema anterior clasifica bloques reconocibles de resultados,
  historial y cachés como artefactos no portables, conservando solo sus categorías
  sanitizadas y una acción manual; no copia contenido ni rutas de esos artefactos.
- El probe WebView2 atribuye memoria por proceso y fase, y limita el perfil y el
  cleanup a procesos pertenecientes al `Job Object`; así evita contar o terminar
  descendientes externos con un `ParentProcessId` coincidente.
- Benchmark reproducible de datasets grandes dentro de WebView2: `perf:webview2`
  genera un CSV temporal de 100 MiB y verifica selección nativa, carga,
  paginación, transformación y exportación con evidencia agregada de duración,
  memoria y cleanup. `perf:check` y `verify:tier` exigen este recorrido además
  del benchmark CLI.
- La migración de sesiones sistema anterior conserva en el artefacto de receta los
  identificadores estructurales acotados de operaciones aplicadas y comprobaciones
  de análisis, además de sus conteos; no guarda resultados, cachés ni rutas.
- Cargar ofrece dos datasets de ejemplo locales para explorar señales de calidad
  y series temporales sin descargar datos ni exponer rutas; la selección usa un
  identificador opaco y el inventario IPC queda en 68 comandos de producción y
  59 estructuras.
- La migración de sesiones sistema anterior conserva metadatos agregados de muestras de
  análisis —estado muestreado y conteos de filas— cuando están disponibles, sin
  copiar filas, valores ni resultados originales; el informe los muestra como
  contexto de compatibilidad.
- La ruta lazy de recetas compatibles y los lectores CSV/TSV/TXT delimitados y
  Parquet comparten una colección Polars con el motor `streaming`; así las
  operaciones de carga y transformación compatibles no vuelven a una
  colección eager silenciosa, mientras las recetas no compatibles conservan su
  fallback eager explícito.
- La apertura y validación de snapshots Parquet de proyectos y del historial
  temporal reutiliza la misma frontera streaming de baja memoria, sin cambiar
  el formato durable ni exponer rutas; el `DataFrame` activo continúa siendo
  materializado para preservar el contrato actual de sesión.
- La restauración del historial ya valida y copia cada snapshot de forma
  incremental: conserva en memoria solo el frame del cursor mientras procesa
  las entradas restantes una por una, reduciendo el pico de RAM al reabrir
  proyectos con varias revisiones.
- La comparación por filas y claves fusiona firmas en bloques acotados y evita
  conjuntos auxiliares duplicados durante el resumen de claves; conserva los
  conteos exactos, el orden estable de los conflictos y los límites actuales
  de JOIN/comparación.
- Los JOIN locales por claves ejecutan ahora el plan de unión de Polars con el
  motor `streaming` después del preflight de cardinalidad; se mantienen el
  límite de entradas, el límite de resultado y la comprobación posterior antes
  de publicar cualquier cambio.
- La selección y reordenación `keep_columns` se incorpora a la familia de
  recetas lazy: la proyección ocurre dentro del plan `streaming`, conserva el
  conteo de columnas descartadas y valida dependencias de columnas calculadas
  antes de materializar el candidato.
- La búsqueda y reemplazo literal sobre columnas de texto también usa el plan
  lazy/streaming, cuenta celdas modificadas con una agregación separada y
  conserva el comportamiento para nulos, renombres y casts a texto.
- La unión de columnas de texto puede ejecutarse dentro del plan lazy/streaming,
  conserva el orden de las fuentes, omite nulos como antes y vuelve a nulo una
  fila sin ningún valor; los casts numérico→texto se validan antes de publicar.
- La división de texto literal también puede ejecutarse de forma lazy: conserva
  el resto en la última columna, rellena destinos ausentes con nulos y mantiene
  las validaciones de nombres, colisiones, casts a texto y descarte reversible
  de la fuente.
- La agrupación y los resúmenes tipados pueden ejecutarse dentro del plan
  lazy/streaming incluso con filtros previos; la búsqueda/reemplazo literal se
  aplica antes de agrupar dentro del mismo plan y el preflight proyecta solo las
  columnas necesarias:
  conserva el orden estable de primera aparición, claves nulas, conteos de
  filas, `count_unique`, tipos numéricos/temporales y validaciones de finitos,
  precisión y desbordamiento antes de publicar el candidato.
- La normalización de contactos puede ejecutarse dentro del plan lazy/streaming:
  correo, teléfono y dirección conservan nulos, espacios Unicode, prefijos y
  conteos exactos de celdas modificadas; también puede ejecutarse antes de
  resumir grupos, con validación sobre los valores ya normalizados.
- Las extracciones textuales también pueden ejecutarse dentro del plan
  lazy/streaming y alimentar resúmenes agrupados: tokens, runs Unicode y
  búsquedas antes/después de delimitadores literales preservan nulos,
  coincidencias ausentes y el límite de 16 columnas.
- Las columnas calculadas numéricas y concatenadas también pueden alimentar
  claves y fuentes de agregación de resúmenes dentro del plan lazy/streaming;
  se conservan tipos, nulos, orden estable y validaciones previas.
- Las columnas derivadas por `split` y `merge` también pueden alimentar claves y
  fuentes de agregación de resúmenes dentro del plan lazy/streaming; el preflight
  valida la proyección posterior a las etapas estructurales y conserva nulos,
  orden estable y conteos de columnas descartadas.
- Las partes temporales calculadas de año, mes y día también pueden usarse como
  claves de agrupación lazy sobre `Date` y `Datetime` sin zona horaria; el
  preflight conserva nulos, orden estable y el fallback eager para filtros o
  zonas horarias.
- Las columnas calculadas de suma, resta, multiplicación, división, concatenación
  y extracción de año, mes o día sobre fechas sin zona horaria también pueden
  ejecutarse dentro del plan lazy/streaming; los operandos y rangos se validan
  antes de publicar, se preservan nulos y se rechazan división por cero, infinitos
  y fechas no representables. Fechas con zona horaria o filtros previos mantienen
  el fallback eager.
- La importación de sesiones sistema anterior prioriza un snapshot local compatible para
  conservar el estado materializado exacto; solo reaplica la receta sobre el
  origen cuando no existe snapshot, dejando explícito el límite de paridad de
  operaciones cuyos parámetros no están en el manifiesto.
- La fixture de round-trip de sesiones cubre fuente, hoja/etapa, operaciones
  deterministas, reglas y análisis/historial/caché no portables; la prueba importa
  y reabre el proyecto sin copiar valores privados ni artefactos operativos.
- Preparar detecta secuencias comunes de doble codificación UTF-8 (`Ã©`, `â€™`)
  por columna y ofrece una reparación reversible, limitada a valores de texto
  que pueden decodificarse inequívocamente sin tocar números, `_cambios` ni
  valores ambiguos.
- Preparar permite confirmar y apartar como nulos los valores de texto que no
  coincidan con una sugerencia semántica con al menos 90% de confianza; no muestra
  celdas, conserva tipos no textuales y ofrece reversión desde el historial.
- La importación de sesiones sistema anterior recalcula y persiste el perfil agregado
  del dataset importado antes de publicar el proyecto, para que Revisar abra
  con una caché de calidad válida sin conservar filas, celdas, rutas ni muestras.
- Los proyectos con perfil cacheado guardan la huella SHA-256 del
   `current.parquet` durable y descartan automáticamente solo ese perfil si el
   snapshot cambia al reabrir; los catálogos anteriores siguen siendo legibles y
   se actualizan al próximo guardado mediante la migración SQLite v6.
- El workspace durable conserva la última vista de Revisar (`diagnosis` o
  `preview`) y la restaura al abrir el proyecto; los catálogos anteriores usan
  Diagnóstico por defecto y las vistas inválidas se rechazan sin reemplazar la
  sesión activa. También conserva el desplazamiento de la página visible de la
  muestra, normaliza offsets inválidos y vuelve a la primera página si la página
  guardada ya no está disponible. La migración SQLite pasa a v7.
- La cobertura crítica por capa vuelve a pasar con `npm run test:coverage`:
  `App`, `DeliveryPhase`, `PreparePhase`, `usePrepareController` y
  `useProjectsController` mantienen sus umbrales propios, incluyendo las ramas
  de confirmación y desmarcado de correcciones de Preparar.
- El workspace durable conserva también la etapa activa del flujo (`load`,
  `review`, `prepare` o `deliver`) y la restaura al abrir proyectos; los
  catálogos anteriores vuelven a Revisar y las etapas desconocidas se rechazan
  sin sustituir la sesión activa. La migración SQLite pasa a v8.
- Updater autenticado de Tauri 2 con consulta explícita, metadatos visibles,
  descarga con progreso/cancelación, instalación nativa y clave pública
  embebida; la frontera Rust rechaza versiones semver iguales, anteriores o
  inválidas; el flujo firmado genera `.sig`, manifiesto estático e inventario
  SHA-256 sin guardar la clave privada en el repositorio.
- Contrato reproducible `updater:contract:test` para el gate Release/Package:
  genera una clave Ed25519 efímera, valida criptográficamente el par válido y
  confirma fallo cerrado ante artefacto truncado, firma alterada, manifiesto
  incompleto/corrupto y URL HTTP. Este fixture no reemplaza el ejercicio contra
  un canal real.
- `release:updater:dry-run` amplía el orquestador de release con configuración
  temporal de endpoint, firma local, SBOM, gates completos y verificación
  fail-closed de instalador/firma/manifiesto.
- El orquestador de release reutiliza el binario `Release/Package` para capturar
  la evidencia visual, evitando reconstruir un ejecutable distinto después del
  gate de empaquetado.
- Los gates de release, SBOM e instalador calculan sus fingerprints SHA-256 con
  la API criptográfica del runtime, manteniendo el flujo compatible con los
  hosts PowerShell usados por npm y por la ejecución directa.
- El perfil `Release` incorpora y aprueba el contrato updater junto con sus
  gates de documentación, IPC, toolchains, cobertura, supply chain, instalador,
  SBOM y binario Tauri sin bundle; la corrida local no implica un árbol limpio
  ni autoriza publicación.
- Cargar admite arrastrar un dataset a la ventana sin entregar su ruta a React
  y mantiene hasta cinco referencias recientes sanitizadas que vuelven a abrir
  el selector nativo.
- Revisar incorpora actividad SQL agregada para las últimas cinco ejecuciones y
  la conserva en el workspace del proyecto al guardarlo; se restauran estado,
  duración y filas, nunca la consulta, rutas ni valores. También mantiene la
  tendencia temporal diaria para rangos cortos, con días vacíos y tabla accesible
  equivalente.
- Entregar ofrece abrir la carpeta del último output local después de una
  exportación exitosa; Rust conserva y revalida temporalmente el destino, usa el
  explorador nativo y no expone la ruta a React.
- Preparar permite retirar columnas identificadoras de forma explícita,
  confirmada y reversible, sin publicar sus valores.
- La CLI sanitiza reportes, recetas y manifiestos en una frontera común, y las
  consultas/joins locales incorporan cancelación y preflight de cardinalidad.
- El smoke CLI valida la redacción de nombres de archivo en stdout sin dejar de
  comprobar que las salidas locales se publiquen completas y de forma atómica.
- Los manifiestos de sesión sistema anterior importados conservan un resumen estructural
  sanitizado en el informe de migración: hoja, etapa, conteos de operaciones,
  reglas y análisis, además de señales booleanas para referencias de origen y
  snapshot. No se restauran sesiones ni se escriben proyectos automáticamente.
- Contrato de calidad versionado `columnia-quality-rules` v1, con guardado
  atómico, importación de Columnia/sistema anterior v1–v3 y compatibilidad con el
  documento legado v1; versiones futuras y formatos ambiguos fallan cerrados.
- Selector nativo Win32 estabilizado para Abrir/Guardar como, con soporte de
  editores `1148`/`1001`, fallback de UI Automation/Win32/Unicode y entrada al
  gate `verify:tier` mediante `npm run smoke:native-selectors`; el driver filtra
  ventanas por PID/owner, espera el cierre del modal y corta procesos bloqueados.
- Entregar permite importar y guardar contratos mediante diálogos nativos,
  muestra el origen/versión y mantiene las rutas fuera de React; la CLI acepta
  el formato canónico y el legado.
- Fase I1 completa: recetas compatibles con renombres, casts, filtros y
  columnas calculadas ejecutadas mediante planes Polars lazy, con fallback eager
  atómico para operaciones que requieren validaciones específicas.
- Monitor compacto de consumo en el lateral, con CPU/RAM del proceso y del
  equipo, actualización nativa periódica y estado accesible para el shell web.
- Benchmark reproducible de 100 MiB contra `sistema anterior`, con comparación de
  duración, working set, conteos, fixture sintética y cleanup sin conservar datos.
- Gate de cobertura V8 global para `src` (80% statements/lines, 75% branches y
  functions), más umbrales por capa crítica para App, Entrega, Preparar y su
  controller; la suite frontend actual tiene 267 tests.
- Supply chain local con `npm audit`, `cargo-audit`, `cargo-deny`, secret scan,
  inventario reproducible de `THIRD_PARTY_NOTICES` y verificación de red sin
  telemetría.
- Contrato de instalador NSIS `currentUser`, recursos MIT/third-party notices y
  política WebView2 `downloadBootstrapper`; Polars actualizado a `0.55.2`.
- Primera entrega de paridad funcional: exportación JSON atómica en UI, Rust,
  CLI, batch y proyectos, con matriz comparativa frente a `sistema anterior`.
- Segunda entrega de paridad funcional: comparación local de dos datasets,
  diferencias multivaluadas de filas/columnas y consolidación opt-in con
  historial cuando el esquema es compatible.
- Tercera entrega de paridad funcional: exportación SQL como script portable y
  atómico en Entregar, CLI, batch y proyectos, con escape de identificadores,
  valores y transacción explícita.
- Cuarta entrega de paridad funcional: visualizaciones compactas y accesibles de
  completitud y posibles outliers en Diagnóstico, con valores exactos y tablas
  equivalentes para lector de pantalla.
- Quinta entrega de paridad funcional: comparación opcional por claves explícitas,
  detección de duplicados/conflictos y consolidación segura de claves nuevas,
  manteniendo el dataset activo hasta confirmar la operación.
- Sexta entrega de paridad funcional: joins locales `Inner`, `Left` y `Full` por
  claves explícitas, con validación de tipos, columnas compartidas sufijadas,
  historial e invalidación de perfil/entrega.
- Benchmark CLI de 256 MiB con 2,220,032 filas, tres transformaciones sostenidas
  y dos actualizaciones durables; el flujo pasa, pero su working set máximo es
  aproximadamente 1.12 GiB y queda fuera del presupuesto de 512 MiB.
- Tier 5 endurece la automatización batch: las salidas quedan confinadas al
  `outputRoot` del manifiesto por defecto y `--force` es obligatorio para
  destinos externos o existentes.
- Tier 5 añade inventario IPC generado desde `generate_handler!`, contrato de
  58 comandos de producción, 4 debug y 56 estructuras compartidas, toolchains
  exactas Node/npm/Rust, notices offline sin `UNKNOWN` y una revisión legal de
  distribución pendiente de completar por canal/jurisdicción.
- Tier 5 hace durable la recuperación del catálogo: los fallos de inicialización
  se pueden reintentar sin reiniciar y las generaciones huérfanas antiguas se
  reconcilian sin tocar las activas. El perfilado de duplicados normalizados
  incorpora un fast-path ASCII; la evidencia corta de 100 MiB registra
  `project-save` en 59.75 s y su actualización en 58.84 s.
- Tier 5 abre el primer límite modular del motor Rust en
  `dataset_fingerprints.rs`, con API interna acotada y cobertura equivalente
  para duplicados exactos y parecidos; quality/recipe/export/persistence
  conservan extracciones posteriores como trabajo incremental.
- Tier 5 incorpora `tools/release.ps1` y `npm run release:dry-run` para
  orquestar los gates locales con rama y árbol limpios; el flujo no etiqueta,
  publica ni contacta servicios remotos.
- Tier 5 liga la evidencia visual de release a un commit limpio: el baseline
  versionado conserva commit, rama, hashes de `package-lock.json` y `Cargo.lock`,
  además de los cinco escenarios desktop, móvil, zoom 125%, zoom 200% y
  forced-colors. El checker solo tolera el commit posterior que modifica
  exclusivamente el propio baseline.
- Smoke reproducible del instalador NSIS con usuario sin privilegios, ruta
  Unicode/con espacios, primera apertura, segunda invocación con instancia única,
  desinstalación y retención controlada de datos de usuario; `Package` lo ejecuta
  después de construir e inventariar el bundle.
- El smoke del instalador acepta un NSIS anterior explícito para probar un
  upgrade real en la misma ruta, con comprobación de versión y supervivencia de
  datos antes de la desinstalación.
- Política ejecutable de rotación/recuperación del updater con release puente
  firmada por la clave anterior, preservación de la versión instalada ante fallo
  y gate de fingerprint; `updater:verify-published` vuelve a descargar el
  manifiesto/instalador HTTPS y verifica tamaño, SHA-256 y firma minisign. Ante
  compromiso de la clave, el contrato congela el canal y prohíbe firmar otra
  release puente con la clave comprometida.

### Validación histórica de la reauditoría

- En la reauditoría del 2026-08-28 aprobaron 234 pruebas Rust, 248 frontend y 9
  E2E, además de build Vite, contratos IPC, cobertura y los perfiles Full,
  Release y Package; Package produjo MSI y NSIS para `137520b`.
- Las capturas visuales web y release frescas cumplieron sus contratos. Los
  recorridos CDP/selectores fueron funcionales, pero excedieron memoria; el gate
  de rendimiento y la comparación del baseline visual release quedaron fallidos
  y trazados en Tier 5.

### Validación de la implementación Tier 5

- `tools/check.ps1 -Profile Full` y `tools/check.ps1 -Profile Release` pasan:
  build, cobertura, clippy, supply chain, SBOM, instalador, 267 tests frontend
  y 244 tests Rust.
- El benchmark formal final de tres actualizaciones durables pasa en 100 MiB:
  `project-save` en 52.09 s y sus tres actualizaciones en 56.37–57.31 s, con
  reapertura, exportación y cleanup confirmados. Evidencia:
  `.local/validation/performance-benchmark/20260828T184531Z/summary.json`.
- La medición nativa de memoria privada WebView2 ya pasa en el smoke aislado:
  521.79 MiB de working set, 265.98 MiB privados y cleanup confirmado, con los
  cuatro diálogos Win32 aprobados. La captura release desde un commit limpio y
  la revisión visual del baseline ya están aprobadas; la revisión legal final
  y la validación del canal siguen siendo requisitos de publicación.
- El probe CDP funcional de ProjectsPanel midió 470.25 MiB de working set y
  253.48 MiB privados, dentro de presupuesto, ejecutó 3 ciclos sostenidos y
  confirmó cleanup. El smoke nativo aislado aprobó abrir dataset, guardar/cargar
  receta y exportar sin exponer rutas; Playwright y selectores se ejecutan como
  gates separados para no mezclar sus perfiles de memoria.
- La nueva corrida estricta de `smoke:cdp` aprobó Playwright, ProjectsPanel,
  mutaciones IPC, persistencia, reapertura y cleanup, con 501,563,392 bytes de
  working set dentro de 512 MiB, pero 272,379,904 bytes de memoria privada sobre
  el límite de 256 MiB. Las corridas diagnósticas previas quedaron en 256.06–258.06
  MiB sin crecimiento monotónico; el gate privado estable sigue pendiente y no se
  presenta esta señal como una fuga confirmada. Evidencia:
  `.local/validation/webview2-cdp/20260829T023923Z/summary.json`.
- Las pruebas Rust del updater cubren versiones estables, downgrade, igualdad,
  prerelease e inputs inválidos; el ejercicio contra un canal real sigue siendo
  una validación de I6 pendiente.
- `release:dry-run` valida el preflight de distribución y exige rama/árbol
  limpios; la captura release aprobada queda ligada al commit de evidencia y el
  baseline solo se guarda en un commit posterior exclusivo de ese archivo.
- El smoke NSIS local pasó desde un usuario no administrador: instalación en
  5.3 s, primera ventana en 711 ms, segunda invocación sin proceso duplicado,
  desinstalación en 1.3 s y sentinel de datos de usuario conservado durante la
  desinstalación y limpiado después. La VM limpia, el canal real y la
  aceptación legal siguen abiertos.
- La rotación de claves ya tiene contrato versionado y gate local; la descarga
  posterior a publicación tiene verificador criptográfico, pero no se ejecutó
  contra un canal real porque todavía no existe un canal público configurado.
- Accesibilidad visual (desktop/móvil, zoom 125%/200% y forced-colors) y
  `perf:check` pasan con la evidencia renovada.

### Interno

- Reauditoría profesional exhaustiva sobre `137520b`: se añadió
  `AUDITORIA_PROFESIONAL_2026-08-28.md` y se abrió Tier 5 con 20 tareas
  trazables (`T5-01`–`T5-20`). No se modificó comportamiento del producto.
- `Full`, `Release` y `Package` aprobaron; Package produjo MSI y NSIS ligados al
  commit auditado. Los gates de rendimiento y baseline visual release fallaron
  y se conservaron como fallos, sin elevar presupuestos ni aprobar hashes.
- El estado operativo posterior se documenta en la sección de validación de la
  implementación Tier 5; el texto de la reauditoría anterior se conserva como
  histórico del commit auditado.

## [0.57.0] - 2026-08-24

### Añadido

- Inventario y fixtures sintéticas para pipelines, sesiones, reglas de calidad
  y recetas legacy sistema anterior, declaradas en `fixtures/manifest.json`.
- La importación de manifiestos de sesión reconoce origen, snapshot, hoja,
  etapa, operaciones aplicadas, calidad y análisis, y los publica como warnings
  sanitizados sin afirmar una restauración automática.

### Validación

- 183 pruebas Rust, 211 pruebas frontend, build Vite, contrato IPC, formato
  Rust y Clippy estricto aprobados.
- Documentación, gobernanza y baseline de rendimiento aprobados; evidencia
  release desktop/móvil/zoom 125%/`forced-colors` en
  `.local/validation/release-evidence/20260825T023814Z` y puerta visual
  confirmada en `.local/validation/release-evidence-check/20260825T023954Z`.

## [0.56.0] - 2026-08-24

### Añadido

- La migración de pipelines sistema anterior v1–v3 conserva las opciones de entrega
  compatibles: formatos locales, columnas seleccionadas y privacidad.
- `xlsx` se normaliza a `excel`; reportes, CSV/ZIP y parámetros SQL sin
  equivalente generan warnings estructurados en el informe de migración.
- Preparar muestra el informe con conversiones, omisiones, acciones manuales y
  SHA-256 del artefacto, y conserva sus metadatos al guardar la receta.

### Validación

- 181 pruebas Rust, 211 pruebas frontend, build Vite, contrato IPC, formato
  Rust y Clippy estricto aprobados.
- Documentación, gobernanza y baseline de rendimiento aprobados; evidencia
  release desktop/móvil/zoom 125%/`forced-colors` en
  `.local/validation/release-evidence/20260825T022756Z` y puerta visual
  confirmada en `.local/validation/release-evidence-check/20260825T023005Z`.

## [0.55.0] - 2026-08-24

### Añadido

- La importación de contratos de calidad sistema anterior ahora entrega un informe
  estructurado con total de entradas, reglas convertidas, omisiones,
  advertencias, acciones manuales y SHA-256 del artefacto original.
- Entregar muestra el resumen del informe y las acciones recomendadas sin
  publicar rutas, filas ni valores del dataset.

### Validación

- 181 pruebas Rust, 209 frontend, build Vite, formato Rust, Clippy estricto,
  documentación y gobernanza aprobados para la vertical de migración.

## [0.54.0] - 2026-08-24

### Mejorado

- El arranque frontend carga inicialmente solo el shell, Cargar y Proyectos;
  Review, Preparar y Entregar se separan en chunks y se precargan al enfocar o
  pasar el cursor por su etapa.
- Tauri ya puede mostrar la ventana y el shell local sin esperar la consulta
  secundaria de información de versión/plataforma.
- La preparación del catálogo SQLite de proyectos se difiere hasta la primera
  operación de proyecto; se conservan la migración segura y la inicialización
  eager para CLI y automatizaciones.
- El monitor de consumo se inicia después del primer paint para no competir con
  la apertura de la interfaz.

### Validación

- 181 pruebas Rust, 209 frontend, build Vite, Clippy estricto, formato Rust,
  smoke desktop, smoke CDP, resumen y baseline de rendimiento aprobados.
- El bundle inicial bajó de 359.93 a 253.10 KB raw y de 100.26 a 77.31 KB gzip.
  El smoke debug continúa condicionado por la compilación nativa de desarrollo.

## [0.53.0] - 2026-08-24

### Añadido

- Importación segura de recetas JSON sistema anterior v1–v3 desde el selector nativo de
  Preparar: renombres, casts, fechas, filtros, reemplazos literales, columnas
  conservadas, cálculos, split/merge, outliers, grupos, contactos y extracciones
  se normalizan a una receta Columnia v1.
- Rechazo explícito de expresiones regulares, booleanos personalizados y
  operaciones sin equivalente para evitar perder semántica durante la
  migración.
- Inventario de migración documentado y anuncio accesible de la etapa activa.

### Validación

- 180 pruebas Rust, 209 frontend, build Vite, formato Rust, documentación y
  gobernanza aprobados.

## [0.52.0] - 2026-08-24

### Añadido

- Selector visible de apariencia en la barra lateral con los modos `Sistema`,
  `Claro` y `Oscuro`, botones con estado accesible y persistencia local segura.
- Aplicación temprana del tema antes de montar React para evitar saltos visuales,
  con overrides explícitos para que `Claro` y `Oscuro` funcionen aunque el modo
  del sistema sea el contrario.
- Refinamiento visual del shell: panel de apariencia, jerarquía de navegación,
  fondos con profundidad, tarjetas redondeadas y estados de interacción más
  distinguibles.

### Validación

- 209 pruebas frontend, build Vite y prueba de sincronización entre npm, Cargo,
  Cargo.lock, package-lock y Tauri.

## [0.51.0] - 2026-08-24

### Añadido

- Paginación de conflictos por clave en bloques de 50, con índices globales,
  navegación accesible y bloqueo de la resolución hasta completar todas las
  páginas.
- Resolución completa de conflictos fuera del primer preview mediante una
  validación backend del conjunto total, manteniendo historial reversible y
  rechazo de decisiones repetidas o incompletas.
- Versionado sincronizado `0.51.0` en npm, Cargo, Cargo.lock, package-lock y
  configuración Tauri.

### Validación

- 177 pruebas Rust y 204 Vitest, build Vite, Clippy estricto, formato,
  documentación, gobernanza, diff limpio, smoke CLI y smoke WebView2 con
  selectores nativos Win32 aprobados. Evidencia: `.local/validation/cli-smoke/20260825T002214Z`
  y `.local/validation/webview2-cdp/20260825T002400Z`.

## [0.50.0] - 2026-08-24

### Añadido

- Resolución independiente por columna/valor para conflictos visibles por clave,
  con cobertura obligatoria de cada celda, historial reversible y compatibilidad
  con decisiones legacy por fila.
- Privacidad visible en los seis destinos locales actuales: máscara/hash para
  señales de correo, teléfono, dirección, nombre e identificadores, incluidos
  identificadores numéricos, con conteo/nombres protegidos en el resultado sin
  exponer valores.
- Versionado sincronizado `0.50.0` en npm, Cargo, Cargo.lock, package-lock y
  configuración Tauri.

### Validación

- 177 pruebas Rust y 202 pruebas Vitest pasan, junto con build Vite, formato,
  Clippy estricto, documentación, gobernanza, diff limpio, smoke CLI y smoke
  WebView2 con selectores nativos Win32. Evidencia: `.local/validation/cli-smoke/20260825T000531Z`
  y `.local/validation/webview2-cdp/20260825T000531Z`.

## [0.49.0] - 2026-08-23

### Añadido

- Catálogo durable de proyectos SQLite v3 con snapshots Parquet, recuperación
  explícita, reglas de calidad, receta y cursor de historial.
- CLI local con `inspect`, `transform`, `validate`, `batch` y operaciones de
  proyectos con contratos JSON v1.
- Evidencia local de accesibilidad, rendimiento, SBOM y empaquetado Windows.
- Documentación Diátaxis, ADRs, política de contribución y política de fixtures
  sintéticas.

### Validación

- 132 pruebas frontend Vitest.
- 127 pruebas Rust.
- Gates locales `Fast`, `Full`, `Release` y `Package` disponibles; los smokes
  que necesitan Windows interactivo conservan evidencia bajo `.local/`.

### Corregido

- Los controles de la interfaz conservan texto legible y foco visible bajo
  `forced-colors: active`, incluidos los controles deshabilitados.

### Limitaciones conocidas

- El dataset se materializa en memoria y la ejecución de recetas todavía no es
  lazy/incremental.
- La auditoría manual con lector de pantalla y High Contrast sigue separada de
  los gates automáticos.
- El release actual es un prototipo local; updater, publicación y firma del
  artefacto son fases posteriores.
