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
| Perfilado | Esquema, nulos, duplicados, estadísticas y análisis | Esquema, nulos, duplicados exactos y parecidos, estadísticas, calidad, outliers y lectura visual accesible | Parcial | Migrar análisis exploratorio, calendario y series temporales |
| Calidad | Reglas v3, tolerancias, formatos, severidad y validación previa a entrega | Reglas base más `allowed_values`, `regex`, `dtype`, unicidad compuesta, `column_compare`, `date_range`, `conditional`, `schema_contract` y `row_count`; límites de payload y gate Rust | Parcial | Versionar documentos y añadir referencias, agregados y drift |
| Transformaciones | Limpieza, tipos, filtros, columnas calculadas y operaciones compuestas | Recetas lazy/eager, historial, renombres, casts, filtros, texto, fechas, split/merge, outliers y agregación | Parcial | Migrar catálogo de limpieza sugerida y optimización no destructiva |
| Comparación | Dataset secundario, consolidación y comparación por clave | Dataset secundario local, comparación por clave, consolidación segura, resolución acotada por fila y joins Inner/Left/Full con historial | Parcial | Completar combinación independiente por columna y conflictos fuera del preview |
| Visualizaciones | Gráficos de análisis y diagnóstico | Barras accesibles de completitud y outliers, con tablas equivalentes | Parcial | Ampliar gráficos exploratorios, filtros e interacciones |
| Salidas | CSV, Excel, Parquet, JSON, SQL y destinos de base de datos | CSV, Parquet, JSON y script SQL | Parcial | Excel, conectores, bundles auditables y privacidad |
| Proyectos | Sesiones, historial, caché, restauración y exportación | SQLite, snapshots Parquet, historial, reglas, recetas y CLI | Parcial | Importar sesiones/pipelines y completar caché/actividad |
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
explícita de hoja queda en la siguiente iteración.

## Quinta entrega de paridad: comparación por clave

La comparación ahora permite seleccionar una o varias columnas clave antes de
elegir la segunda fuente:

- informa claves coincidentes, exclusivas, duplicadas y con conflictos de valores;
- valida que cada clave exista en ambas fuentes y conserve el mismo tipo;
- bloquea la consolidación si hay conflictos, duplicados o esquemas incompatibles;
- cuando es segura, conserva el dataset activo y agrega solo las filas con claves
  nuevas, registrando la operación en el historial.

La comparación sin claves mantiene el comportamiento multivaluado anterior.

Cuando una comparación por clave encuentra valores divergentes, Review muestra un
preview acotado de las celdas diferentes y exige elegir entre conservar la fila
activa o usar la fila comparada. La resolución queda bloqueada si faltan decisiones
o si el preview superó su límite; al confirmar, Rust reemplaza únicamente las filas
seleccionadas, registra la operación en el historial y descarta la comparación
pendiente. La combinación independiente por columna sigue siendo una brecha
explícita.

## Sexta entrega de paridad: joins multidataset

Review permite unir el dataset activo con una segunda fuente local por las
claves seleccionadas explícitamente:

- `Inner` conserva solo las claves presentes en ambos datasets;
- `Left` conserva todas las filas del dataset activo;
- `Full` conserva las filas de ambos datasets;
- las claves deben existir en ambas fuentes y mantener el mismo tipo;
- columnas compartidas no clave del dataset comparado reciben el sufijo
  `_right` de Polars, mientras las columnas de ambas fuentes se publican en el
  preview resultante.

La unión reemplaza el dataset activo únicamente después de calcular el frame,
registra `Unir datasets (...)` en el historial, invalida el perfil y las
compuertas de entrega, y no expone rutas del sistema al frontend. La brecha
restante es combinar conflictos de forma independiente por columna/valor y
resolver previews que superen el límite visible.

## Séptima entrega de paridad: primera slice de calidad v3

El contrato de calidad de Entregar ahora admite cinco comprobaciones avanzadas
que también recorren CLI y exportación porque se evalúan en el mismo motor Rust:

- `allowed_values` para catálogos textuales;
- `regex` con compilación segura y rechazo de patrones inválidos;
- `dtype` para verificar el tipo físico de una columna;
- `unique_together` para detectar duplicados de una clave compuesta;
- `row_count` para límites inclusivos sobre el tamaño del dataset.

Cada regla conserva la tolerancia por conteo o porcentaje, rechaza parámetros que
pertenecen a otro tipo, limita el texto y el número de valores/columnas, y no
devuelve muestras ni celdas. La UI muestra solo los controles relevantes, con
etiquetas y ayudas aptas para teclado y lector de pantalla. La compatibilidad
restante del contrato v3 sigue en P1/M1.

## Octava entrega de paridad: comparación entre columnas

Los contratos de calidad ahora admiten `column_compare` con dos columnas del
mismo tipo físico y seis operadores (`eq`, `ne`, `lt`, `lte`, `gt`, `gte`). Los
operadores de orden solo se habilitan para texto y tipos numéricos; nulos en
cualquiera de las columnas cuentan como inválidos.

La regla conserva las tolerancias por conteo o porcentaje y se evalúa en Rust,
por lo que la misma semántica protege la UI, la CLI y las exportaciones. La UI
ofrece selectores etiquetados para columna izquierda, operador y columna derecha.
La migración reconoce `column_compare`/`column_comparison`, `other_column` y
alias seguros de operadores; reglas sin dos columnas, operador o equivalencia
de severidad se omiten con advertencia visible.

## Novena entrega de paridad: rango de fechas

El contrato admite `date_range` con límites inclusivos `minDate` y `maxDate`.
La evaluación acepta columnas de texto con fechas ISO, RFC3339 o formatos locales
seguros, además de columnas físicas `Date` y `Datetime`. Los valores nulos,
vacíos, ilegibles o fuera de los límites cuentan como inválidos y respetan las
tolerancias del contrato.

Entregar muestra controles nativos de fecha con etiquetas asociadas. Rust valida
el orden de los límites y mantiene la misma regla para UI, CLI y exportaciones.
La migración reconoce `min_value`/`max_value`, `minDate`/`maxDate` y omite con
advertencia límites ausentes, invertidos o no interpretables.

## Décima entrega de paridad: reglas condicionales

El contrato admite `conditional` con una condición `when` sobre una columna y
los operadores `eq`, `ne`, `lt`, `lte`, `gt` y `gte`. Cuando la condición se
cumple, la regla aplica una subregla `then` fila-a-fila: `not_null`, `non_empty`,
`numeric_range`, `allowed_values`, `regex` o `dtype`. Los nulos de la condición
no activan la subregla; la tolerancia se mantiene en la regla exterior y se
evalúa en Rust para UI, CLI y exportaciones.

La UI muestra controles etiquetados para columna, operador, valor, columna
objetivo y parámetros de `then`. La migración reconoce el objeto `when` (con
alias `op`/`val` y operador `eq` por defecto) y `then`; omite con advertencia
subreglas globales o formas que no tengan una equivalencia segura.

## Undécima entrega de paridad: contrato de esquema

El contrato admite `schema_contract` para comprobar la estructura completa del
dataset. Define `columns` o `requiredColumns`, puede rechazar columnas
adicionales con `allowAdditional: false` y puede exigir un orden exacto con
`requiredOrder`. La evaluación estructural usa `checkedCount = 1` y cuenta
columnas faltantes, adicionales y un eventual desorden; no intenta tratar el
esquema como una regla fila-a-fila.

Entregar muestra un editor accesible con una columna requerida por línea, una
casilla para permitir adicionales y un orden opcional. La migración reconoce
`schema` como alias de `schema_contract`, conserva tolerancias y omite con
advertencia listas vacías, nombres duplicados o parámetros malformados.

## Primera vertical de migración de reglas DataPrep

Entregar permite importar un contrato JSON de DataPrep mediante el selector
nativo. Acepta una lista directa o un objeto con `rules`/`quality_rules`, y
convierte de forma segura las reglas representables por Columnia:
`not_null`, `non_empty`, `unique`, `numeric_range`, `allowed_values`, `regex`,
`dtype`, `unique_together`, `column_compare`, `date_range`, `conditional`,
`schema_contract` y `row_count`. Reconoce campos snake_case y
camelCase, conserva tolerancias por conteo y porcentaje, y aplica el límite de
16 reglas y 1 MiB por archivo.

La importación es deliberadamente parcial: reglas desconocidas, severidades no
bloqueantes, políticas `on_missing`/`null_policy` incompatibles y parámetros
malformados se omiten con un informe visible por regla. Una regla sin tolerancia
se importa como bloqueante con máximo de inválidos igual a cero; nunca se
convierte silenciosamente una política no equivalente en una aprobación.
Pipelines JSON, sesiones guardadas y el round-trip hacia proyectos siguen
pendientes en M1.

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

## Destinos locales y privacidad de entrega

Entregar conserva la misma compuerta de calidad para CSV, JSON, Parquet, SQL,
Excel y SQLite. Excel se publica como un libro `.xlsx` real con una hoja
`dataset`; SQLite se publica con una transacción atómica, columnas tipadas y la
misma tabla lógica `dataset`. Ambos destinos también están disponibles en CLI,
batch y exportación de proyectos, sin entregar rutas al frontend.

Antes de publicar se puede elegir no proteger, enmascarar o aplicar SHA-256 a
columnas de texto cuyos nombres sugieren correo, teléfono, dirección o
identificadores personales. La calidad se valida sobre el dataset preparado y
la protección se aplica solo al snapshot de salida. El catálogo completo de PII
para recetas, manifests, reports y conectores remotos sigue pendiente.

## Limpieza segura de filas vacías

Preparar ofrece una acción explícita para eliminar únicamente filas cuyos
valores son todos nulos o texto en blanco. Conserva el orden de las filas,
reporta el impacto y registra una revisión reversible en el historial; no
elimina filas parcialmente incompletas ni decide imputaciones automáticamente.

## Limpieza segura de columnas constantes

Preparar también permite retirar columnas que tienen un único valor no nulo entre
las filas. La operación usa las métricas agregadas del perfil, reporta nombres e
impacto, excluye columnas completamente nulas, conserva el orden y deja al menos
una columna para que el dataset siga siendo utilizable. El cambio queda registrado
en el historial, invalida el perfil y puede deshacerse.

La detección de identificadores y la eliminación automática de duplicados difusos
siguen formando parte del catálogo pendiente; la señal agregada de duplicados
parecidos y la imputación conservadora ya están disponibles para revisión.

## Limpieza segura de columnas completamente vacías

Preparar distingue las columnas 100% nulas de las columnas constantes y ofrece una
acción independiente para retirarlas. Reporta los nombres y el impacto, conserva
el orden y deja al menos una columna aunque todo el esquema esté vacío; la acción
se registra en historial, invalida el perfil y puede deshacerse. Las columnas con
solo parte de sus valores nulos no se eliminan automáticamente.

## Tratamiento seguro de columnas con alta nulidad

Preparar identifica columnas con al menos 80% de valores nulos, sin incluir las
columnas 100% nulas que tienen una acción separada. El umbral aparece en la UI,
la operación reporta nombres e impacto, conserva el orden y deja al menos una
columna; el cambio se registra en historial y puede deshacerse. Las columnas bajo
el umbral no se eliminan automáticamente.

## Normalización segura de valores centinela

El perfil cuenta, sin mostrar valores, tokens textuales ausentes conocidos como
`N/A`, `null`, `unknown`, `missing`, `sin datos` y equivalentes normalizados. Preparar
permite convertir esos tokens a nulos en columnas de texto, conservando otros tipos
y `_cambios`; el impacto por celda/columna se informa y el cambio queda en historial
reversible. La lista es deliberadamente conservadora: no convierte números
centinela ni valores arbitrarios sin una regla explícita.

## Normalización segura de booleanos

El perfil identifica columnas de texto con una coincidencia booleana de al menos
90%. Preparar puede canonicalizar únicamente `yes`/`no`, `sí`/`no` y `true`/`false`
como `true`/`false`, conserva los valores no reconocidos y registra el cambio en el
historial. La operación no convierte identificadores numéricos ni altera columnas
que no sean de texto.

## Detección conservadora de duplicados parecidos

El perfil cuenta filas adicionales que coinciden después de normalizar
mayúsculas/minúsculas, espacios repetidos y acentos. El conteo excluye los
duplicados exactos ya informados, no devuelve claves ni valores y solo aparece
como una señal de revisión manual en Preparar. No se eliminan filas difusas sin
una decisión explícita de la persona usuaria.

## Imputación conservadora de nulos

Preparar ofrece un intento reversible para completar únicamente valores nulos.
En texto usa el valor no vacío más repetido cuando aparece al menos dos veces;
en columnas numéricas usa la mediana observada inferior para conservar el tipo
físico. No modifica blancos, centinelas, columnas sin evidencia suficiente ni
la columna reservada `_cambios`, y reporta celdas/filas afectadas antes de
invalidar el perfil y las compuertas de entrega.

## Señales agregadas de privacidad e identificadores

Columnia clasifica nombres de columnas con señales conservadoras de correo,
teléfono, dirección, identificador o nombre. Solo publica la categoría y el nombre
de la columna en el perfil; no inspecciona ni devuelve muestras para esta señal. La
protección de salida con máscara/hash sigue siendo una decisión independiente y
explícita.

## Trazabilidad local por fila

Preparar permite activar la columna reservada `_cambios`. Se crea como texto nulo y
las mutaciones posteriores conservan la columna y añaden una etiqueta breve de la
operación por fila, con límite de longitud y registro reversible en historial. La
limpieza textual general no modifica `_cambios`; deshacer/rehacer restaura también
la trazabilidad. Esta vertical no pretende sustituir un log de auditoría externo.

## Consulta local restringida

Revisar ofrece una consulta SQL de solo lectura sobre la tabla lógica `dataset`.
La primera vertical permite seleccionar columnas existentes y paginar con
`LIMIT`/`OFFSET` hasta 200 filas, con presupuesto de 2 KiB. También acepta
filtros simples (`=`, desigualdad, comparaciones numéricas, `IS NULL` e
`IS NOT NULL`), `GROUP BY` de una columna y agregaciones acotadas (`COUNT`,
`SUM`, `AVG`, `MIN`, `MAX`).
Rechaza escrituras, comentarios, separadores, tablas externas y operaciones no
representadas, y devuelve una tabla accesible con tipos y valores nulos
explícitos. Joins y DuckDB quedan fuera de esta vertical.

## Brecha de migración desde `dataprepv1.1`

La migración tiene dos capas distintas:

1. **Paridad funcional:** ya existe una primera slice de calidad v3, pero todavía
   faltan el catálogo completo de limpieza
   sugerida, el optimizador de transformaciones, el análisis exploratorio
   (distribuciones, correlaciones, grupos, nulos, centinelas, casi duplicados,
   calendario y series temporales), las reglas de calidad versionadas restantes, los
   conectores PostgreSQL/MySQL/SQL Server, los bundles auditables y el procesamiento
   fuera de memoria; Excel y SQLite locales ya están cubiertos en la primera
   vertical de entrega.
2. **Compatibilidad de artefactos:** Columnia ya importa parcialmente contratos
   JSON de reglas de calidad de DataPrep, con conversión segura e informe de
   omitidas. Todavía no importa pipelines JSON ni sesiones guardadas, y la
   verificación de round-trip sigue pendiente en la Fase M1 del roadmap.

Columnia ya tiene una representación nativa distinta —Tauri/Rust/Polars,
proyectos SQLite/Parquet y comandos estrechos—, por lo que la migración no
debe copiar módulos Python ni prometer compatibilidad binaria de archivos
internos. La unidad de compatibilidad será el resultado observable y un
informe claro de cualquier operación no convertida.
