# Inventario de migración DataPrep

Este documento define el contrato local para importar recetas JSON de
`dataprepv1.1` sin copiar su implementación Python ni exponer rutas, muestras
o valores del dataset.

## Artefactos reconocidos

Columnia acepta recetas nativas con la forma `StoredTransformRecipe` y también
los documentos DataPrep v1–v3 que contienen `transform` o `transform_config`,
además de manifiestos de sesión que incluyan una de esas transformaciones. La
importación de recetas ocurre al seleccionar un archivo JSON desde Preparar; el
resultado siempre se normaliza a una receta Columnia v1 antes de mostrarla.
Desde Proyectos existe además una acción separada para mapear una sesión guardada
al catálogo cuando su fuente local puede validarse de forma segura.

| Semántica DataPrep | Receta Columnia | Estado |
| --- | --- | --- |
| `rename_text` | `renames` | Convertida |
| `dtype_col` + `dtype_type` | `casts` | Convertida para tipos explícitos |
| `parse_date_cols` | `dateParses` | Convertida con ISO 8601 por defecto |
| `filters` (`col`, `op`, `val`) | `filters` (`column`, `operator`, `value`) | Convertida |
| `find_replace` literal | `findReplace` | Convertida |
| `keep_columns` | `keepColumns` | Convertida |
| `calc` | `calculatedColumn` | Convertida para operaciones equivalentes |
| `split_column`, `merge_columns` | `splitColumn`, `mergeColumns` | Convertida |
| `outliers` | `outlierTreatments` | Convertida para `cap`/`drop`; `impute` está disponible en recetas nativas Columnia |
| `drop_duplicates` | Acción directa de Preparar e importación de sesión | Reproducida en el fallback a la fuente con todas las columnas y conserva la primera aparición/orden; es reversible y agregada |
| `drop_fuzzy_duplicates` | Acción directa de Preparar e importación de sesión | Reproducida en el fallback hasta 5.000 filas con el fingerprint normalizado de la fila completa de Columnia; conserva la primera fila y las copias exactas, es reversible y agregada; por encima de ese límite se conserva la protección de rendimiento de DataPrep |
| `drop_high_null_cols` | Acción directa de Preparar e importación de sesión | Reproducida en el fallback con el umbral estricto de DataPrep (>80% nulos), conservando al menos una columna utilizable |
| `drop_id_cols` | Acción directa de Preparar e importación de sesión | Reproducida en el fallback para columnas 100% únicas no numéricas ni temporales; conserva al menos una columna utilizable |
| `drop_empty_cols` | Acción directa de Preparar e importación de sesión | Reproducida en el fallback para columnas completamente nulas, conservando al menos una columna utilizable |
| `drop_constant_cols` | Acción directa de Preparar e importación de sesión | Reproducida en el fallback para columnas con un único valor no nulo, sin retirar columnas completamente nulas y conservando al menos una columna utilizable |
| `drop_empty_rows` | Acción directa de Preparar e importación de sesión | Reproducida en el fallback a la fuente solo para filas completamente nulas, como DataPrep; es reversible y agregada |
| `impute_numeric` | Acción directa de Preparar e importación de sesión | Reproducida en el fallback a la fuente para columnas físicas `Int64`/`Float64` seguras; usa la mediana de DataPrep, conserva `Int64` si es entera y promueve a `Float64` si es fraccionaria |
| `normalize_sentinels` | Acción directa de Preparar e importación de sesión | Reproducida en el fallback a la fuente con el vocabulario cerrado de DataPrep; convierte solo texto centinela a nulo, es reversible, agregada y no modifica números ni `_cambios` |
| `fix_encoding` | Acción directa de Preparar e importación de sesión | Reproducida en el fallback a la fuente cuando cada valor puede repararse inequívocamente; es reversible, agregada y no modifica números, `_cambios` ni valores ambiguos |
| `impute_categorical` | Acción directa de Preparar e importación de sesión | Convertida/reproducida cuando la semántica es determinista: nulos textuales a `Desconocido`; es reversible, agregada y no modifica números ni `_cambios` |
| `parse_dates` | Acción directa de Preparar e importación de sesión | Reproducida en el fallback para columnas textuales con un formato dominante cerrado y cobertura segura; convierte a `Datetime`, conserva nulos y omite columnas ambiguas o con demasiadas fechas ilegibles |
| `trim_text` | Acción directa de Preparar e importación de sesión | Reproducida en el fallback a la fuente para columnas textuales, recortando espacios exteriores de forma determinista y protegiendo `_cambios` |
| `normalize_text` | Acción directa de Preparar e importación de sesión | Reproducida en el fallback a la fuente con espacios colapsados, minúsculas y eliminación de acentos; protege `_cambios` |
| `cast_numeric` | Acción directa de Preparar e importación de sesión | Reproducida en el fallback para texto con más de 90% de valores numéricos; convierte a `Int64`/`Float64`, conserva nulos no convertibles y rechaza conversiones inseguras |
| `cap_outliers` | Acción directa de Preparar, receta de Transformaciones e importación de sesión | Reproducida en Preparar y en el fallback después de `cast_numeric`, usando límites IQR ×1.5 sobre columnas numéricas con al menos cuatro valores válidos; promueve el resultado a `Float64` cuando los límites son fraccionarios |
| `impute_outliers` | Acción directa de Preparar e importación de sesión | Reproducida en el fallback con IQR ×1.5 y mediana observada, conservando el tipo numérico cuando la mediana es representable |
| `drop_outliers` | Acción directa de Preparar, receta de Transformaciones e importación de sesión | Reproducida en Preparar y en el fallback después de `cast_numeric`, eliminando una fila si alguna columna numérica excede sus límites IQR ×1.5 y conservando nulos |
| `normalize_booleans` | Acción directa de Preparar e importación de sesión | Reproducida en el fallback a la fuente cuando una columna de texto usa solo tokens booleanos conocidos y contiene ambos valores; se convierte al tipo booleano, sin modificar `_cambios` |
| `mask_pii` | Acción directa de Preparar e importación de sesión | Reproducida en el fallback con el modo `mask` conservador: sustituye valores no nulos de columnas personales detectadas por encabezado (`email`, `phone`, `address`, `name`) con `[REDACTED]`, preserva nulos, columnas no detectadas y `_cambios`; los snapshots compatibles conservan prioridad y los modos hash/clave explícita requieren revisión manual |
| `normalize_columns` | Acción directa de Preparar e importación de sesión | Reproducida en el fallback a la fuente con normalización Unicode y colisiones deterministas; se ejecuta antes de la receta estructural |
| `add_cambios_col` | Acción directa de Preparar e importación de sesión | Reproduce la estructura reservada `_cambios` con estado inicial nulo; no reconstruye anotaciones históricas que no formen parte del snapshot |
| `group_summary` | `groupSummary` | Convertida para agregaciones conocidas |
| `normalize_contacts` | `contactNormalizations` | Convertida |
| `extract_text` | `textExtractions` | Convertida |

## Conversión segura

- Las versiones futuras a v3 se rechazan antes de convertirlas.
- Expresiones regulares en `find_replace`, booleanos personalizados y
  operaciones estructurales sin equivalente no se descartan: la carga falla con
  una razón accionable para revisión manual. En `selected_cleaning_operations`,
  una limpieza desconocida se conserva como advertencia específica para poder
  revisar el catálogo sin ejecutar una aproximación insegura.
- Los campos desconocidos de una receta nativa Columnia continúan fallando por
  `deny_unknown_fields`.
- El nombre y la fecha de la receta se conservan cuando están presentes; si la
  fecha no existe se genera una fecha local de importación.
- El archivo permanece bajo control del diálogo nativo. React recibe solo la
  receta normalizada y nunca recibe la ruta seleccionada.

## Informe estructurado de contratos de calidad

La importación de contratos JSON de calidad produce un informe junto con las
reglas convertidas. El informe incluye el total de entradas, convertidas,
omitidas, advertencias y acciones manuales recomendadas. Cuando la importación
proviene de un archivo, también incluye el SHA-256 de sus bytes originales para
que el resultado pueda auditarse sin conservar la ruta ni valores del dataset.

El informe diferencia una advertencia de una omisión y siempre recomienda
validar el contrato convertido antes de exportar. Si hay reglas omitidas,
indica que deben revisarse y recrearse manualmente; las tolerancias ajustadas o
asumidas también quedan señaladas. La UI muestra el resumen, el hash y las
acciones sin publicar rutas administradas, filas ni celdas.

## Opciones de entrega de pipelines

Los pipelines DataPrep v1–v3 conservan un bloque `export`. Columnia migra al
artefacto de receta las opciones que ya tienen representación local segura:

- `csv`, `json`, `parquet`, `sql` y `xlsx` (`xlsx` se normaliza a `excel`);
- columnas seleccionadas, cuando vienen como nombres de columna válidos;
- políticas de privacidad `none`, `mask` y `hash`.

La receta incluye `exportOptions` y un `migrationReport` con las operaciones
convertidas, omitidas, advertencias, acciones manuales y SHA-256 del archivo
original. Reportes `md`/`html`, separador o codificación CSV, formato de fecha,
ZIP y parámetros específicos de tabla/dialecto SQL no se aplican
automáticamente porque todavía no tienen un equivalente completo en el
selector de Entregar; quedan señalados para revisión antes de exportar.

También se informa cuando el pipeline trae operaciones de limpieza,
análisis o calidad incrustadas. `selected_cleaning_operations` reconoce aliases
de las limpiezas deterministas y los normaliza a nombres canónicos. `parse_dates`
solo convierte columnas textuales con formatos cerrados, cobertura de al menos
80%, años entre 1900 y 2100 y como máximo 1% de literales no interpretables;
las columnas ambiguas se dejan intactas. `mask_pii` se reproduce únicamente como
la máscara local predeterminada y conservadora;
los modos hash o con clave explícita no se inventan y requieren revisión manual.
Una operación sin equivalente reversible conserva una advertencia específica
para revisión manual y no se ejecuta como si fuera equivalente.

## Fixtures y formatos auditados

El inventario mínimo se prueba con fixtures sintéticas versionadas en
`fixtures/migration/` y declaradas en `fixtures/manifest.json`:

| Fixture | Artefacto | Cobertura |
| --- | --- | --- |
| `dataprep-pipeline-v3.json` | Pipeline DataPrep v3 | Transformación y entrega compatibles |
| `dataprep-session-v1.json` | Manifiesto de sesión | Origen, snapshot, hoja, etapa, operaciones, calidad y análisis como warnings |
| `dataprep-quality-v3.json` | Reglas DataPrep v3 | Forma de contrato de calidad sin datos de usuario |
| `legacy-recipe-v1.json` | Receta antigua | Campos legacy sin versión explícita |

La revisión de manifiestos de sesión desde Preparar no restaura un dataset: sus
rutas y snapshots se convierten en señales booleanas sanitizadas, y la hoja,
etapa y conteos de operaciones, reglas y análisis se conservan en el informe
estructurado. La capa nativa conserva una slice de mapeo segura para
migraciones controladas: mantiene la ruta fuera del bridge, valida fuente,
hoja, esquema y receta en un estado temporal, y solo después puede publicar un
snapshot y abrir el proyecto resultante. Si existe un `snapshot_path` local y
compatible, se restaura primero porque representa el estado materializado
actual y evita perder operaciones de limpieza cuyos parámetros no forman parte
del manifiesto. En el proyecto importado, las etiquetas de etapa conocidas se
convierten a la etapa activa de Columnia; una etiqueta desconocida conserva el
fallback a Revisar. Cuando no hay snapshot, se usa la fuente disponible y se
reaplica la receta estructural. La acción no se expone en el panel de Proyectos; la CLI la ofrece como
`project-import-dataprep --store DIR --session FILE [--name NAME]` para
migraciones autorizadas. Una cancelación o error no modifica el dataset activo
ni reemplaza proyectos existentes.

Cuando el manifiesto aporta identificadores estructurales, el informe conserva
hasta 64 nombres de operaciones aplicadas y comprobaciones de análisis, con un
límite de 96 caracteres por nombre y sin separadores de ruta. Los elementos que
no cumplen ese contrato se omiten, pero sus conteos originales permanecen. No
se guardan resultados originales, celdas, cachés reanudables ni rutas resueltas.
Si un bloque de análisis aporta solo metadatos agregados de muestreo, se pueden
conservar el estado muestreado y los conteos de filas de muestra/total bajo
`migrationReport.session`; se descartan sus filas, valores y resultados. Los
conteos se validan contra un límite local y una muestra nunca puede superar el
total registrado.
Si el manifiesto trae bloques reconocibles de resultados, historial o caché,
el informe los clasifica como `analysis_results`, `history` o `caches` no
portables; solo conserva esos nombres de categoría y una acción manual, nunca
su contenido ni la referencia de archivo.
Las operaciones deterministas `drop_duplicates`, `drop_high_null_cols`,
`drop_id_cols`, `drop_empty_cols`, `drop_constant_cols`, `drop_empty_rows`,
`normalize_sentinels`, `impute_numeric`, `impute_categorical`, `parse_dates`,
`trim_text`, `normalize_text`, `fix_encoding`, `cast_numeric`, `cap_outliers`,
`impute_outliers`, `drop_outliers`, `normalize_booleans`, `mask_pii`,
`drop_fuzzy_duplicates`, `normalize_columns` y `add_cambios_col` se reproducen como
parte de la importación cuando solo queda la fuente, en el orden fijo de
limpieza de DataPrep. `drop_empty_rows` conserva la semántica original de filas
completamente nulas y no elimina por sí sola texto en blanco. Si existe un
snapshot compatible, se usa ese estado materializado y las operaciones no se
reaplican.
Cuando una sesión necesita conservar revisiones históricas, Columnia acepta el
contrato local explícito `history_snapshots` (o `historySnapshots`): un objeto
de versión `1` con `entries` y `cursor`. Cada entrada debe aportar una etiqueta
imprimible y una referencia local a un archivo `.parquet`; se aceptan hasta doce
entradas y el tamaño total queda sujeto al presupuesto de 1 GiB del historial
durable. El cursor debe apuntar a una entrada cuyo frame sea idéntico al dataset
actual importado después del replay. Las referencias se resuelven y validan solo
en Rust, se rechazan enlaces simbólicos/reparse points y no cruzan el bridge. Al
publicar el proyecto, las entradas se copian a la generación administrada como
`history-*.parquet`, por lo que Deshacer/Rehacer queda disponible tras reabrirlo.
Un `history` ambiguo, `execution_history` o una referencia que no cumpla este
contrato continúa siendo no portable y requiere revisión manual.
Las estrategias IQR `cap_outliers`, `impute_outliers` y `drop_outliers` son
mutuamente excluyentes: si una sesión selecciona más de una, el replay se
rechaza antes de publicar el proyecto y no combina sus efectos.
Al publicar un proyecto importado, Columnia recalcula y persiste su propio
perfil agregado junto con la huella SHA-256 de `current.parquet`, para que
Revisar tenga una caché verificable y pueda invalidarla si el snapshot cambia;
esto no afirma que un análisis de DataPrep pueda reanudarse automáticamente.

## Límites pendientes

Si `execution_history` contiene únicamente estado (`success`, `error` o
`cancelled`), duración acotada y filas opcionales, la importación de proyecto
conserva como máximo cinco entradas agregadas en la actividad SQL, con IDs
locales. Las consultas, rutas, valores, estados desconocidos y entradas fuera
de presupuesto se descartan; el artefacto `history` continúa marcado para
revisión manual porque su contenido completo no es portable.

La restauración completa de sesiones, historial de ejecuciones, cachés reanudables
y artefactos de análisis originales requiere contratos separados. La slice
`history_snapshots` reconstruye únicamente revisiones Parquet explícitamente
referenciadas y compatibles con el cursor; no convierte resultados de análisis,
cachés reanudables ni una historia ambigua en estado operativo automáticamente.
