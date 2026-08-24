# Paridad funcional con `dataprepv1.1`

Este documento separa la arquitectura nueva de las capacidades que una persona
usuaria espera conservar del producto original. La paridad no exige copiar el
código ni la interfaz: exige que la capacidad visible exista, tenga un contrato
claro y esté cubierta por una prueba o evidencia local.

## Matriz inicial

| Área | `dataprepv1.1` | Columnia | Estado | Siguiente decisión |
| --- | --- | --- | --- | --- |
| Entradas tabulares | CSV, TSV, JSON/JSONL, Excel/ODS, Parquet | CSV, TSV, JSON/JSONL, XLSX/XLS/XLSB/ODS, Parquet | Implementada | Mantener casos difíciles de libros en pruebas |
| Vista previa | Paginación y muestras acotadas | Páginas Rust de 50 filas, sin enviar el dataset completo a React | Implementada | Ampliar evidencia con datasets grandes |
| Perfilado | Esquema, nulos, duplicados, estadísticas y análisis | Esquema, nulos, duplicados, estadísticas, calidad, outliers y lectura visual accesible | Implementada | Ampliar análisis exploratorio visual interactivo |
| Transformaciones | Limpieza, tipos, filtros, columnas calculadas y operaciones compuestas | Recetas lazy/eager, historial, renombres, casts, filtros, texto, fechas, split/merge, outliers y agregación | Implementada | Ampliar operaciones multidataset |
| Comparación | Dataset secundario, consolidación y comparación por clave | Dataset secundario local, filas multivaluadas, diferencias de columnas y consolidación compatible con historial | Parcial | Añadir claves explícitas, conflictos y joins con DuckDB |
| Visualizaciones | Gráficos de análisis y diagnóstico | Barras accesibles de completitud y outliers, con tablas equivalentes | Parcial | Ampliar gráficos exploratorios, filtros e interacciones |
| Salidas | CSV, Excel, Parquet, JSON, SQL y destinos de base de datos | CSV, Parquet, JSON y script SQL | Parcial | Excel/conectores de base de datos |
| Proyectos | Sesiones, historial, caché, restauración y exportación | SQLite, snapshots Parquet, historial, reglas, recetas y CLI | Implementada | Comparar flujos avanzados del original |
| Privacidad | Redacción, PII y operación local | Sin telemetría; límites y contratos de privacidad; detección PII pendiente | Parcial | Inventario de PII y reglas explícitas |
| Escala | Lazy/incremental para entradas grandes | Lazy para recetas compatibles; benchmark CLI validado hasta 256 MiB, con RAM fuera del presupuesto | Parcial | Ejecución incremental real y presupuesto integral |

## Primera entrega de paridad

La primera capacidad añadida en esta fase es la exportación JSON de extremo a
extremo:

- botón de entrega en la UI con la misma compuerta de calidad que CSV/Parquet;
- escritura atómica desde Rust con arreglo JSON válido;
- soporte en `columnia-cli transform`, `batch` y `project-export` mediante
  `--format json`;
- prueba Rust del formato y pruebas Vitest del contrato UI/IPC.

La salida JSON conserva valores nulos y tipos serializables por Polars. La
protección contra fórmulas de hojas de cálculo se mantiene específicamente para
CSV, donde ese riesgo es relevante.

## Cuarta entrega de paridad: visualizaciones accesibles

Diagnóstico incorpora una lectura visual compacta del perfil por columna:

- barras de completitud para todas las columnas;
- barras de posibles outliers para las columnas numéricas, normalizadas contra
  el máximo observado;
- valores exactos visibles y regiones ARIA con nombres descriptivos;
- tablas de perfil existentes como equivalente completo para lector de pantalla,
  alto contraste y navegación sin depender del color.

La entrega cubre la visualización rápida del diagnóstico. Los gráficos
exploratorios interactivos y los filtros avanzados siguen pendientes.

## Segunda entrega de paridad: comparación y consolidación

Review ahora permite elegir una segunda fuente local compatible y conserva el
dataset activo mientras Rust calcula:

- filas compartidas y exclusivas como multiconjunto, respetando duplicados;
- columnas compartidas y columnas exclusivas de cada fuente;
- compatibilidad de esquema (nombres, orden y tipos);
- consolidación opt-in por `vstack` cuando el esquema es compatible.

La consolidación se publica como un cambio del historial, invalida el perfil y
las compuertas de entrega, y descarta el dataset comparado después de aplicar el
cambio. Los libros usan la primera hoja por defecto en este corte; la selección
explícita de hoja y la comparación por clave quedan en la siguiente iteración.

## Tercera entrega de paridad: script SQL

Entregar ahora ofrece un cuarto formato con la misma compuerta de calidad:

- genera un script `.sql` atómico con `BEGIN TRANSACTION` y `COMMIT`;
- crea una tabla portable llamada `dataset`, escapando identificadores y valores;
- conserva nulos, booleanos, números y texto con comillas simples duplicadas;
- está disponible también en `columnia-cli transform`, `batch` y `project-export`.

El script no conecta ni escribe directamente en una base de datos. Los
conectores, selección de tabla y credenciales siguen fuera de alcance para
mantener la frontera local y sin secretos.

## Evidencia del original

La matriz se contrastó con el bridge y las vistas existentes del original:

- `frontend/src/bridge/pywebviewBridge.ts` expone comparación, sesiones,
  historial y operaciones de exportación;
- `frontend/src/components/charts/DataChart.tsx` y las vistas de análisis
  muestran el contrato visual y accesible de gráficos;
- `src/dataprep/core/datasets.py` documenta la distinción entre operaciones
  materializadas y operaciones que requieren ejecución completa.

Esta matriz se actualizará con cada entrega de paridad y no sustituye los gates
de contratos, privacidad, accesibilidad y rendimiento.
