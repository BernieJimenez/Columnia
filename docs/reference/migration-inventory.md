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
| `impute_categorical` | Acción directa de Preparar e importación de sesión | Convertida/reproducida cuando la semántica es determinista: nulos textuales a `Desconocido`; es reversible, agregada y no modifica números ni `_cambios` |
| `group_summary` | `groupSummary` | Convertida para agregaciones conocidas |
| `normalize_contacts` | `contactNormalizations` | Convertida |
| `extract_text` | `textExtractions` | Convertida |

## Conversión segura

- Las versiones futuras a v3 se rechazan antes de convertirlas.
- Expresiones regulares en `find_replace`, booleanos personalizados y
  operaciones sin equivalente no se descartan: la carga falla con una razón
  accionable para revisión manual.
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
análisis o calidad incrustadas: esas superficies se migran por contratos
separados y no se descartan silenciosamente.

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
del manifiesto. Cuando no hay snapshot, se usa la fuente disponible y se
reaplica la receta estructural. La acción no se expone en el panel de Proyectos; la CLI la ofrece como
`project-import-dataprep --store DIR --session FILE [--name NAME]` para
migraciones autorizadas. Una cancelación o error no modifica el dataset activo
ni reemplaza proyectos existentes.

Cuando el manifiesto aporta identificadores estructurales, el informe conserva
hasta 64 nombres de operaciones aplicadas y comprobaciones de análisis, con un
límite de 96 caracteres por nombre y sin separadores de ruta. Los elementos que
no cumplen ese contrato se omiten, pero sus conteos originales permanecen. No
se guardan resultados originales, celdas, cachés reanudables ni rutas resueltas.
La operación determinista `impute_categorical` se reproduce como parte de la
importación cuando solo queda la fuente; si existe un snapshot compatible, se
usa ese estado materializado y la operación no se reaplica.
Al publicar un proyecto importado, Columnia recalcula y persiste su propio
perfil agregado para que Revisar tenga una caché verificable; esto no afirma que
un análisis de DataPrep pueda reanudarse automáticamente.

## Límites pendientes

La restauración completa de sesiones, historial de ejecuciones, cachés reanudables,
artefactos de análisis originales y round-trip hacia proyectos requiere contratos
separados. La slice actual conserva la receta, las reglas representables y un
perfil agregado recalculado en el workspace nuevo, pero no reconstruye snapshots
históricos ni afirma que los análisis originales puedan reanudarse automáticamente.
