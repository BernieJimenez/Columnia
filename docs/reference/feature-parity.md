# Matriz de capacidades vigente

Este documento describe las capacidades que Columnia mantiene actualmente. La
matriz es una referencia de producto y no depende de herramientas externas ni
de rutas fuera del repositorio.

## Matriz

| Área | Capacidad actual | Estado | Próximo límite conocido |
| --- | --- | --- | --- |
| Entradas tabulares | CSV, TSV, JSON/JSONL, Parquet, XLSX, XLS, XLSB y ODS | Implementada | Ampliar casos difíciles de libros |
| Vista previa | Paginación de 50 filas, muestreo acotado y fallback seguro | Implementada | Ampliar evidencia con datasets grandes |
| Perfilado | Esquema, nulos, duplicados exactos y parecidos, estadísticas, calidad, outliers, grupos categóricos y cobertura temporal | Implementada | Ampliar análisis exploratorio |
| Calidad | Documento `columnia-quality-rules` v1, reglas base y avanzadas, tolerancias, severidad y validación previa a entrega | Implementada | Añadir políticas para destinos remotos |
| Transformaciones | Recetas lazy/eager, historial, renombres, casts, filtros, texto, fechas, split/merge, reemplazo literal o regex segura, outliers, imputación y agregación; parseo, filtros temporales (`Eq`/`Neq` y rangos) y extracción de componentes pueden compartir el plan lazy | Implementada | Seguir ampliando ejecución incremental |
| Comparación | Dataset secundario local, comparación por clave, conflictos paginados sin materializar activos source-backed, resolución acotada por fila/columna, consolidación segura por claves nuevas y joins `INNER`/`LEFT`/`FULL`, incluidos resultados source-backed reversibles para CSV/TSV/TXT/Parquet | Implementada | Ampliar análisis comparativo |
| Visualizaciones | Completitud, outliers, patrones de nulos, formatos, grupos, calendario, tendencia temporal y correlaciones, con tablas equivalentes | Implementada | Ampliar interacciones |
| Salidas | CSV, JSON, Parquet, SQL, Excel, SQLite y bundle ZIP auditable | Implementada | Añadir destinos de base de datos |
| Proyectos | Catálogo SQLite, snapshots Parquet, historial, reglas, recetas, CLI, archivos recientes, reapertura segura, cobertura de correlaciones, motor SQL, perfil de rendimiento, formato de exportación, protección, claves de comparación y tipo de JOIN por proyecto | Implementada | Muestras y resultados derivados no portables |
| Privacidad | Sin telemetría, sanitización de contratos e informes, detección agregada de datos personales y máscara/hash local | Implementada | Extender contratos equivalentes |
| Escala | Lazy para recetas compatibles, apertura source-backed de JSON/JSONL/NDJSON/XLSX/XLSB grandes mediante snapshots Parquet privados por bloques, limpiezas source-backed de filas vacías, duplicados exactos y parecidos, columnas con historial reversible, correcciones recomendadas que combinan trim/renombres, retiro y máscara de identificadores y datos personales detectados desde DuckDB, normalización de nombres y activación de `_cambios` source-backed, recorte/normalización de texto/valores centinela y booleanos source-backed con umbrales y conteos exactos, inferencia source-backed de números y fechas con validación conservadora, imputaciones source-backed conservadora y categórica, acciones directas IQR source-backed `cap`/`impute`/`drop`, recetas source-backed de proyección/filtros/casts/fechas/cálculos simples/reemplazo literal y regex con grupos `$1`–`$9`/división calculada/división de texto/unión/extracción de texto/normalización de contactos/resúmenes por grupo/tratamientos IQR, protección `mask`/`hash` mediante snapshot DuckDB, lectura por bloques, snapshots administrados, conteo de apertura con DuckDB y cancelación, exportación CSV/JSON/Parquet/SQL/Excel/SQLite/Bundle source-backed con privacidad incremental y Bundle con `recipe.json` validado, transferencia por filas de Excel y SQLite, diccionario Bundle con nulos agregados en disco, consultas y mutaciones source-backed `INNER`/`LEFT`/`FULL JOIN`, consolidación y resolución acotada de conflictos por claves, paginación de conflictos desde disco con resultados Parquet reversibles, DuckDB opcional, `JOIN` con frame activo y snapshot Parquet comparado, benchmark source-backed de 512 MiB, orden global por conteo real, rechazo de materialización implícita, cancelación y presupuestos explícitos | Parcial | Ejecución integral fuera de RAM y benchmark sostenido |

## Contrato de calidad

El formato durable es:

```json
{
  "format": "columnia-quality-rules",
  "version": 1,
  "rules": []
}
```

Columnia valida el formato antes de guardarlo o aplicarlo. También puede leer el
documento histórico sin campo `format` cuando contiene `version: 1` y `rules`.
Las versiones futuras, campos ambiguos y reglas no representables fallan de
forma cerrada.

Las reglas admitidas incluyen valores permitidos, expresiones regulares, tipos,
unicidad compuesta, comparación entre columnas, integridad referencial,
monotonía, agregados, reconciliación, drift de distribución, rangos de fecha,
condiciones, contratos de esquema y conteo de filas. La evaluación pública solo
expone estados, conteos, porcentajes y acciones de revisión.

## Recetas y ejecución

Las recetas se validan antes de modificar el dataset. El historial conserva
revisiones reversibles y la publicación de un resultado es atómica. Las rutas,
filas y valores permanecen en Rust; React recibe únicamente metadatos acotados,
identificadores opacos y resultados agregados.

Las operaciones compatibles con el plan lazy se ejecutan sobre fuentes y
snapshots sin clonar innecesariamente el dataset completo. Las recetas
source-backed que combinan selección/renombrado con hasta tres filtros, casts,
fechas fijas (`YMD`, `DMY`, `MDY`), partes de fecha (`Year`, `Month`, `Day`),
reemplazo literal y cálculos simples (`Add`, `Subtract`, `Multiply`, `Concat`)
también se ejecutan directamente sobre la fuente mediante DuckDB y publican un
snapshot Parquet privado, conservando conteo, orden y nulos. El reemplazo se
aplica después de los filtros y publica su conteo exacto de celdas modificadas.
La división source-backed de una columna de texto en dos a dieciséis destinos
conserva delimitadores Unicode, segmentos vacíos, nulos y el resto en el último
destino; también respeta `keepColumns`, renombrados y `dropSource`.
La unión source-backed de dos a dieciséis columnas de texto también conserva el
orden de las fuentes, nulos, cadenas vacías, separador y `dropSources`, incluso
cuando una fuente numérica se convierte explícitamente a texto.
Las extracciones source-backed de texto ejecutan en DuckDB las variantes de
primer/último token, dígitos ASCII, letras Unicode y segmentos antes/después de
delimitadores literales; conservan coincidencias ausentes como nulos y los
segmentos vacíos, y validan dependencias con renombrados y `keepColumns`.
Las normalizaciones source-backed de correo, teléfono y dirección se aplican
después de las etapas estructurales compatibles, conservan nulos, espacios
Unicode y prefijos telefónicos, y calculan el conteo exacto de celdas cambiadas;
las extracciones posteriores observan esos valores normalizados.
Los resúmenes source-backed por grupo se ejecutan después de esas etapas cuando
la receta es compatible: conservan el primer orden de aparición, agrupan claves
nulas y soportan `sum`, `mean`, `min`, `max`, `count` y `count_unique`, con
validación de tipos, precisión, overflow y valores no finitos. El snapshot
publicado mantiene contadores separados para grupos y filas colapsadas.
La eliminación source-backed de duplicados parecidos calcula claves normalizadas
y exactas en DuckDB, conserva repeticiones idénticas y el orden de entrada, y
publica un snapshot reversible sin llenar el `DataFrame` activo. Las
correcciones recomendadas source-backed combinan el recorte de espacios y los
renombres deterministas en una sola proyección con conteos exactos.
Las acciones directas IQR source-backed calculan cuantiles sobre la fuente y
aplican `cap`, `drop` o `impute` a columnas numéricas sin llenar el `DataFrame`
activo, conservando conteos exactos y snapshots reversibles. Los tratamientos
IQR source-backed de receta calculan cuantiles sobre la fuente y aplican
`cap`, `drop` o `impute` a columnas `Int64`/`Float64`, con baseline común,
nulos preservados, validación de mínimo de valores, finitud y precisión, y
contadores separados para celdas ajustadas, filas retiradas y filtros.
Las exportaciones source-backed con privacidad `mask` o `hash` generan primero
un snapshot Parquet privado en DuckDB, conservan nulos y columnas no personales
y transfieren CSV, JSON, Parquet, SQL, Excel, SQLite o bundle sin materializar
el `DataFrame` activo; la fuente original se valida antes y después.
Las consultas `JOIN` compatibles de Revisar pueden registrar un `DataFrame`
activo junto con el snapshot Parquet de la comparación. Así se evita cargar de
nuevo todas las filas comparadas cuando el activo ya está transformado; si la
fuente no es compatible, se conserva el fallback materializado.

Las mutaciones `INNER`, `LEFT` y `FULL` también pueden combinar un activo
source-backed con una fuente CSV, TSV, TXT delimitada o Parquet directamente en
DuckDB. El motor cuenta la cardinalidad antes de escribir, publica solo el
resultado como Parquet administrado, conserva el orden de entrada y activa el
historial reversible con undo/redo; las fuentes permanecen intactas y los
formatos comparados incompatibles usan el fallback eager. La ejecución
integral fuera de RAM de todas las demás operaciones sigue siendo un límite
explícito.

La consolidación por claves nuevas reutiliza el snapshot Parquet temporal de la
comparación: DuckDB valida duplicados y conflictos de payload con comparación
null-safe, ejecuta el anti-join contra la fuente activa y escribe solo las
filas nuevas en otro Parquet administrado. Se conserva el orden del activo y
de la comparación, se comprueba la cardinalidad antes de escribir, el cursor
queda reversible y el frame activo sigue en modo esquema-only; fuentes o
errores incompatibles vuelven al camino eager existente.

La página de conflictos por clave también conserva la frontera diferida: si el
activo es source-backed, CSV/TSV/TXT delimitado se copia temporalmente a Parquet
en disco y tanto ese snapshot como la comparación se recorren por bloques. La
respuesta contiene únicamente la página de conflictos y sus valores necesarios;
el frame activo no se materializa ni cambia de estado. Cuando la comparación
completa tiene como máximo 2.048 conflictos y un esquema compatible, la
resolución aplica las decisiones por fila o columna directamente en DuckDB,
publica un Parquet reversible y limpia la comparación después de publicar. Si
supera ese límite, falla la compatibilidad o no puede respetar el presupuesto,
usa el camino eager existente.

Las fechas ISO sin offset o con sufijo UTC `Z` también se convierten sobre la
fuente, y las partes de fecha pueden seguir a filtros dentro de la misma receta;
los filtros ordenados sobre columnas `Date` y `Datetime` aceptan literales ISO
8601 y mantienen límites inclusivos/exclusivos tanto en eager como en DuckDB;
los offsets distintos de UTC, valores inválidos, expresiones regulares y
combinaciones no seguras usan un fallback eager atómico y mantienen
la recuperación del dataset anterior ante errores o cancelación.

La exportación Bundle source-backed compatible escribe `dataset.csv` desde
DuckDB y calcula `dictionary.json` sin conservar las filas en el `DataFrame`
activo. El manifest incluye hashes de los archivos, puede incluir el reporte
de calidad incremental y, cuando hay una receta activa, `recipe.json` validado
con su referencia y hash. Las reglas no incrementales continúan usando la ruta
materializada, mientras `mask`/`hash` se aplican mediante un snapshot privado
intermedio.

Las exportaciones Excel `.xlsx` y SQLite source-backed compatibles leen las
fuentes delimitadas o Parquet con DuckDB y transfieren cada fila al destino sin
llenar el `DataFrame` activo. Excel conserva el texto que parece fórmula como
texto literal; SQLite crea la tabla con el esquema detectado y confirma los
insertos en una transacción. Ambas rutas validan el tamaño de la fuente,
cancelación, publicación atómica y cleanup. Las recetas y reglas no
incrementales mantienen la ruta materializada, mientras `mask`/`hash` se
aplican mediante el snapshot privado intermedio.

Los libros XLSX/XLSB grandes también pueden abrirse source-backed. Calamine
recorre la hoja seleccionada en dos pasadas secuenciales: primero detecta
encabezados, tipos y dimensiones; después escribe un snapshot Parquet temporal
por bloques de 16K. La sesión conserva solo el esquema y la primera página en
el `DataFrame` activo, mientras paginación, perfilado, consultas y exportaciones
reutilizan el snapshot y verifican que el libro original mantenga su tamaño.
XLS/ODS continúan usando el fallback materializado porque su lector no ofrece
la misma lectura secuencial.

Los archivos `JSON`, `JSONL` y `NDJSON` grandes también se abren
source-backed. DuckDB los convierte a un snapshot Parquet privado mediante una
proyección completa en disco; la sesión conserva solo el esquema y la primera
página, mientras el conteo, la paginación, las consultas, los proyectos y las
recetas compatibles reutilizan el snapshot. La apertura es cancelable y valida
que el archivo original no cambie durante la conversión.

La acción `remove_empty_rows` también puede ejecutarse sobre una fuente
source-backed compatible: DuckDB filtra filas cuyos valores son nulos o texto
en blanco, publica un snapshot Parquet y conserva `_cambios` con la etiqueta de
la operación. El historial se inicializa copiando la fuente actual por disco,
por lo que undo/redo sigue disponible sin llenar el `DataFrame` activo.

Las acciones `remove_duplicates`, `remove_constant_columns`,
`remove_empty_columns` y `remove_high_null_columns` comparten esa frontera:
DuckDB conserva la primera aparición de cada fila, calcula conteos agregados de
valores distintos y nulos, y publica solo el snapshot Parquet resultante. El
orden, `_cambios`, el mínimo de una columna utilizable y el historial reversible
se mantienen; si la fuente o el presupuesto no son compatibles, se conserva el
fallback eager.

Las acciones `remove_identifier_columns` y `remove_personal_columns` también
pueden retirar columnas por señal de privacidad directamente desde la fuente.
La proyección conserva el orden y `_cambios`, publica un snapshot reversible y
el contrato de datos personales devuelve únicamente el conteo de columnas
retiradas.

La acción `mask_personal_values` también puede proteger valores personales
source-backed en DuckDB. Conserva nulos, evita contar valores ya `[REDACTED]`,
publica los cambios como snapshot reversible y devuelve únicamente los conteos
de celdas y columnas modificadas.

Las acciones `normalize_column_names` y `enable_row_audit` también admiten
fuentes source-backed: la primera conserva la normalización y resolución de
colisiones de la ruta eager, y la segunda añade `_cambios` como texto nulo. Las
dos publican snapshots reversibles y dejan el `DataFrame` activo en modo
esquema-only.

## Proyectos y almacenamiento

Cada proyecto conserva un snapshot Parquet administrado y un catálogo SQLite.
Abrir un proyecto valida el perfil, las reglas, el esquema, el cursor y las
revisiones antes de activar la sesión. El historial está limitado a doce
revisiones y el registro SQL conserva solo estado, duración y filas afectadas.

Desde v0.120, guardar un dataset source-backed delimitado o Parquet convierte
la fuente directamente a `current.parquet` con DuckDB y verifica el conteo antes
de publicar la generación. La sesión conserva el esquema vacío y el historial
diferido, mientras que los proyectos materializados mantienen la ruta eager.

La interfaz no recibe rutas internas. El workspace SQLite v12 conserva también
el formato de exportación, la protección de datos, las columnas clave de
comparación y el tipo de JOIN con validación cerrada; las claves se filtran
contra el esquema restaurado y no se guardan muestras ni valores. La cobertura
de correlaciones se conserva
por proyecto con opciones cerradas de 10.000, 50.000 o 100.000 filas; el motor
SQL por proyecto acepta únicamente `polars` o `duckdb`; el perfil de rendimiento
por proyecto acepta únicamente `conservative`, `balanced` o `maximum`; y los
catálogos anteriores usan la preferencia local segura para ambos ajustes. Los selectores nativos, la
canonicalización de archivos y la política de escritura atómica viven en la
capa Rust.

## Verificación

Los contratos se cubren por capas:

- `npm run test` valida la UI y los modelos TypeScript.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib` valida el motor nativo.
- `npm run build` valida tipos y bundle de producción.
- `npm run docs:check` valida enlaces, versiones y ownership documental.
- `npm run brand:check` evita que regresen referencias de marca retiradas al árbol activo.
- `npm run ipc:check` compara el inventario IPC con `lib.rs` y el bridge.
- `npm run verify:tier` orquesta los gates reproducibles del tier completo.
