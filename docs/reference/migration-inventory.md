# Inventario de migración DataPrep

Este documento define el contrato local para importar recetas JSON de
`dataprepv1.1` sin copiar su implementación Python ni exponer rutas, muestras
o valores del dataset.

## Artefactos reconocidos

Columnia acepta recetas nativas con la forma `StoredTransformRecipe` y también
los documentos DataPrep v1–v3 que contienen `transform` o `transform_config`.
La importación ocurre al seleccionar un archivo JSON desde Preparar; el
resultado siempre se normaliza a una receta Columnia v1 antes de mostrarla.

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
| `outliers` | `outlierTreatments` | Convertida para `cap`/`drop` |
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

## Límites pendientes

La importación de sesiones, reglas de calidad incrustadas, opciones de entrega,
artefactos de análisis y round-trip hacia proyectos requiere un contrato
separado. No se simula esa paridad durante la importación de una receta: esos
campos quedan para la siguiente entrega M1.
