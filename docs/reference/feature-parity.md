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
| Perfilado | Esquema, nulos, duplicados, estadísticas y análisis | Esquema, nulos, duplicados exactos y parecidos, estadísticas, calidad, outliers, grupos categóricos acotados, cobertura temporal y lectura visual accesible | Parcial | Migrar análisis exploratorio, calendario completo, tendencias y series temporales |
| Calidad | Reglas v3, tolerancias, formatos, severidad y validación previa a entrega | Reglas base más `allowed_values`, `regex`, `dtype`, unicidad compuesta, `column_compare`, `referential_integrity`, `monotonic`, `aggregate_check`, `aggregate_reconciliation`, `distribution_drift`, `date_range`, `conditional`, `schema_contract` y `row_count`; documento Columnia v1, compatibilidad DataPrep v1–v3, límites de payload y gate Rust | Parcial | Conservar severidad y políticas avanzadas sin degradarlas |
| Transformaciones | Limpieza, tipos, filtros, columnas calculadas y operaciones compuestas | Recetas lazy/eager, historial, renombres, casts, filtros, texto, fechas, split/merge, outliers con límite/eliminación/imputación por mediana, imputación categórica explícita como `Desconocido`, eliminación reversible de casi duplicados por fingerprint normalizado y agregación; importación del núcleo representable de pipelines DataPrep v1–v3, incluido el catálogo determinista `selected_cleaning_operations`; reparación reversible de doble codificación UTF-8 y apartado confirmado de valores incompatibles con sugerencias semánticas | Parcial | Migrar las reglas avanzadas restantes del catálogo de limpieza sugerida, opciones de exportación y optimización no destructiva |
| Comparación | Dataset secundario, consolidación y comparación por clave | Dataset secundario local, comparación por clave, consolidación segura, resolución por columna/valor paginada y joins Inner/Left/Full con historial | Parcial | Ampliar análisis exploratorio y equivalencias remotas |
| Visualizaciones | Gráficos de análisis y diagnóstico | Barras accesibles de completitud, outliers, patrones de nulos, validación de formatos, grupos categóricos acotados, cobertura, calendario diario y tendencia temporal diaria/mensual/anual acotada y matriz de correlaciones numéricas, con tablas equivalentes | Parcial | Ampliar gráficos exploratorios, series temporales completas, filtros e interacciones |
| Salidas | CSV, Excel, Parquet, JSON, SQL y destinos de base de datos | CSV, Parquet, JSON, SQL, Excel `.xlsx`, SQLite y bundle ZIP auditable locales, con publicación atómica y receta validada opcional dentro del bundle | Parcial | PostgreSQL/MySQL/SQL Server, políticas de tabla |
| Proyectos | Sesiones, historial, caché, restauración y exportación | SQLite, snapshots Parquet, historial, reglas, recetas, CLI, archivos recientes locales sin rutas, arrastre nativo sin rutas en React y primera importación segura de sesiones DataPrep con validación previa | Parcial | Completar restauración de sesiones, round-trip, caché y actividad |
| Privacidad | Redacción, PII y operación local | Sin telemetría; detección agregada de PII, máscara/hash en los seis destinos locales, sanitización de recetas/reports/manifests y confirmación visible de columnas protegidas | Parcial | Extender contratos equivalentes a conectores remotos |
| Escala | Lazy/incremental para entradas grandes | Lazy para recetas compatibles; benchmark CLI validado hasta 256 MiB; perfiles Rayon persistentes; SQL local con cancelación cooperativa, presupuesto de agregación y preflight de cardinalidad JOIN | Parcial | Ejecución incremental real, DuckDB y presupuesto integral |

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
- histogramas numéricos de 12 intervalos con límites estables al persistir un
  perfil;
- matriz de correlaciones de Pearson para hasta 12 columnas numéricas y 100.000
  filas muestreadas, con coeficientes y conteos de pares en una tabla accesible;
- resúmenes de frecuencia para hasta cuatro columnas categóricas no sensibles,
  con ocho grupos principales, umbral de tres filas y “Resto” para categorías
  omitidas; cada gráfico conserva una tabla equivalente;
- valores exactos visibles y regiones ARIA con nombres descriptivos;
- tablas de perfil y frecuencias como equivalente completo para lector de pantalla,
  alto contraste y navegación sin depender del color.

La entrega cubre la visualización rápida del diagnóstico. Los gráficos
exploratorios interactivos y los filtros avanzados siguen pendientes. Las
tendencias temporales incluyen ahora una serie de línea con área, selector de
filas/porcentaje, escala y puntos accesibles; la tabla inferior conserva la
equivalencia exacta y el calendario diario se mantiene como vista complementaria.

## Privacidad accionable en Preparar

El catálogo de limpieza separa los identificadores de otras señales personales.
Cuando el perfil detecta correo electrónico, teléfono, dirección o nombre por el
encabezado, Preparar ofrece una revisión agregada y una confirmación explícita.
La acción retira esas columnas de forma reversible, excluye `_cambios`, conserva
al menos una columna utilizable y no envía nombres ni valores por IPC; el
historial permite deshacerla. La privacidad de recetas, reportes, manifiestos y
conectores remotos sigue siendo una brecha independiente.

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
compuertas de entrega, y no expone rutas del sistema al frontend. Los conflictos
que superan la primera página se consultan mediante el contrato paginado
descrito abajo, sin exponer rutas ni celdas fuera de la decisión visible.

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

## Duodécima entrega de paridad: integridad referencial local

El contrato admite `referential_integrity` para comprobar que una clave simple
o compuesta pertenezca a un catálogo explícito de referencias. Las claves
simples usan valores escalares de texto, número o booleano; las compuestas usan
un arreglo JSON por referencia, por ejemplo `["DO", 1]`. Los nulos y las claves
ausentes en el catálogo cuentan como inválidos y respetan las tolerancias.

La evaluación ocurre completamente en Rust sobre el snapshot activo y devuelve
solo conteos. Entregar muestra checkboxes nativos para la clave y un textarea
etiquetado para el catálogo, con ayuda sobre el formato compuesto. La migración
reconoce `reference_values`/`referenceValues` y `reference`, además de aliases
`referential` y `key_columns`; entradas malformadas, nulas o complejas se
omiten con advertencia visible. No se conectan tablas remotas ni se incluyen
valores de filas en los resultados.

## Decimotercera entrega de paridad: monotonía de secuencias

El contrato admite `monotonic` para comprobar que una columna conserve un orden
no decreciente (`increasing`) o no creciente (`decreasing`). La comparación es
no estricta, cuenta cada inversión como un inválido y reutiliza las tolerancias
por conteo y porcentaje del contrato. Los nulos reinician la cadena y no se
convierten en muestras ni valores expuestos; el resultado mantiene solo los
conteos agregados.

La evaluación compartida soporta texto, fechas, datetimes, booleanos y columnas
numéricas. Entregar muestra un selector accesible con las opciones “No
decreciente” y “No creciente”. La migración DataPrep reconoce `direction` y
`order`, además de aliases como `asc`, `ascending`, `desc` y `descending`; si
falta la dirección se conserva el comportamiento compatible de DataPrep y se
usa `increasing`. Direcciones inválidas se omiten con advertencia visible.

## Decimocuarta entrega de paridad: agregados y reconciliaciones

El contrato admite `aggregate_check` para comparar una agregación de una columna
contra `expected` o una lista de `referenceValues`. Soporta `count`, `sum`,
`min` y `max`; por defecto usa `sum`. También admite
`aggregate_reconciliation`, que suma dos columnas y comprueba que sus totales
coincidan. Ambas reglas aceptan `toleranceAbs` y `toleranceRel`, con límites
inclusivos; el resultado cuenta como máximo una inversión agregada por regla y
no expone el valor observado.

Los valores numéricos se leen de columnas numéricas, booleanas o texto
numérico. Nulos y textos no numéricos quedan fuera de la observación agregada,
manteniendo el conteo de filas revisadas para calcular la tolerancia porcentual.
Entregar muestra selectores accesibles para la agregación, columnas de
reconciliación, valor esperado, referencias y tolerancias absoluta/relativa.
La migración reconoce `aggregate_check`/`aggregate_reconciliation`, aliases de
agregación (`total`, `minimum`, `maximum`) y variantes snake_case/camelCase de
`expected`, `toleranceAbs` y `toleranceRel`; configuraciones incompatibles se
omiten con advertencia visible.

## Decimoquinta entrega de paridad: drift de distribución

El contrato admite `distribution_drift` para comparar la media numérica observada
de una columna contra la media de una línea base (`baseline`). El umbral inclusivo
se expresa con `threshold`; `toleranceAbs` se conserva como alias de compatibilidad
y tiene prioridad cuando ambos están presentes. Los nulos y textos no numéricos
quedan fuera de ambas medias, pero el conteo de filas revisadas permanece completo
para aplicar las tolerancias de la compuerta.

Entregar muestra un editor accesible para la línea base y el umbral absoluto. La
evaluación devuelve únicamente conteos, porcentajes y estado, sin publicar la media
observada ni valores del dataset. La migración DataPrep reconoce `distribution_drift`
y `drift`, `baseline`/`baselineValues`, `reference_values`/`referenceValues`,
`threshold` y `tolerance_abs`/`toleranceAbs`; las reglas incompatibles se omiten con
advertencia visible.

## Decimosexta entrega de paridad: documento de calidad versionado

Columnia guarda el contrato de calidad en un documento canónico, independiente
del contrato JSON v1 que usa la salida de la CLI:

```json
{
  "format": "columnia-quality-rules",
  "version": 1,
  "rules": []
}
```

Entregar usa diálogos nativos para importar y guardar, publica en React solo el
documento o el resultado de conversión —nunca una ruta— y reemplaza el destino
de forma atómica. El lector limita cada archivo a 1 MiB y el contrato a 16
reglas. Los documentos Columnia son estrictos: un `format` distinto, campos
desconocidos o una versión futura se rechazan sin intentar una conversión
permisiva.

La matriz de compatibilidad acepta Columnia v1, DataPrep v1–v3 y documentos
legados con una lista directa o un objeto `rules`/`quality_rules` sin versión.
La CLI también conserva la lectura del documento histórico de Columnia
`{"version":1,"rules":[...]}`. El resultado visible identifica origen y versión;
una versión DataPrep posterior a v3 falla de forma cerrada.

## Primera vertical de migración de reglas DataPrep

Entregar permite importar un contrato JSON de DataPrep mediante el selector
nativo y guardar el resultado como documento Columnia v1. Acepta una lista
directa o un objeto con `rules`/`quality_rules`, informa origen y versión, y
convierte de forma segura las reglas representables por Columnia:
`not_null`, `non_empty`, `unique`, `numeric_range`, `allowed_values`, `regex`,
`dtype`, `unique_together`, `column_compare`, `referential_integrity`, `monotonic`,
`aggregate_check`, `aggregate_reconciliation`, `distribution_drift`, `date_range`, `conditional`, `schema_contract` y
`row_count`. Reconoce campos snake_case y
camelCase, conserva tolerancias por conteo y porcentaje, y aplica el límite de
16 reglas y 1 MiB por archivo.

La importación es deliberadamente parcial: reglas desconocidas, severidades no
bloqueantes, políticas `on_missing`/`null_policy` incompatibles y parámetros
malformados se omiten con un informe visible por regla. Se reconocen aliases
snake/camel, severidades históricas (`error`, `critical`, `fatal`), referencias
escalares y condiciones heredadas; una referencia externa, `nullable: true` o
una contradicción entre `severity` y `blocking` queda omitida con warning. Una
regla sin tolerancia se importa como bloqueante con máximo de inválidos igual a
cero; nunca se convierte silenciosamente una política no equivalente en una
aprobación.
Los pipelines JSON todavía no se convierten automáticamente en proyectos. Las
sesiones guardadas ya se reconocen y dejan un resumen estructural sanitizado; el
bridge nativo dispone de un preflight que puede revisarse antes de crear un
proyecto sin modificar el dataset activo. La primera vertical también importa
una sesión sintética, la reabre, la valida y la exporta sin publicar rutas. La
restauración completa y el round-trip con fixtures representativas siguen
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
Excel, SQLite y bundle. Excel se publica como un libro `.xlsx` real con una hoja
`dataset`; SQLite se publica con una transacción atómica, columnas tipadas y la
misma tabla lógica `dataset`. El bundle publica `dataset.csv`,
`dictionary.json`, un `quality-report.json` opcional, `recipe.json` cuando hay
una receta validada y `manifest.json` con hashes SHA-256 por archivo. Estos
destinos también están disponibles en CLI, batch y
exportación de proyectos, sin entregar rutas al frontend.

Antes de publicar se puede elegir no proteger, enmascarar o aplicar SHA-256 a
columnas cuyos nombres sugieren correo, teléfono, dirección, nombre o
identificadores personales. La protección se aplica también a identificadores
numéricos detectados y conserva nulos; el resultado informa el conteo y nombres
de columnas protegidas, nunca sus valores. La calidad se valida sobre el dataset
preparado y la protección se aplica solo al snapshot de salida en CSV, JSON,
Parquet, SQL, Excel y SQLite. El catálogo completo de PII para recetas,
manifests, reports y conectores remotos sigue pendiente.

Después de una exportación local exitosa, Entregar ofrece **Abrir carpeta de
exportación**. Rust conserva únicamente durante la sesión el destino recién
publicado, lo vuelve a validar como archivo regular y abre su carpeta mediante
el explorador nativo; React recibe solo éxito o error y no puede proporcionar
una ruta arbitraria. La retención desaparece al reiniciar la aplicación o al
comenzar una sesión nueva.

## Decimoctava entrega de paridad: conflictos paginados y privacidad visible

Review ya puede resolver cada celda divergente de una clave con una decisión
independiente de la misma celda activa o comparada. La operación exige cubrir
todas las celdas visibles, conserva el esquema, registra una sola revisión
reversible y mantiene compatibilidad con decisiones legacy que elegían una fila
completa. Cuando hay más conflictos que la primera página, Review los solicita en
bloques de 50, conserva el índice global de cada conflicto y solo habilita la
resolución después de completar todas las páginas. Rust vuelve a calcular el
conjunto completo antes de publicar, de modo que una página omitida o una
decisión repetida falla sin mutar el dataset.

La exportación reutiliza un único snapshot protegido para todos los destinos
locales existentes. La detección usa el mismo catálogo agregado de señales de
perfil, transforma a texto los identificadores numéricos cuando se solicita
protección y devuelve únicamente metadatos de columnas protegidas. No se
incluyen valores, hashes de filas ni rutas en el contrato IPC.

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

La detección y retirada explícita de identificadores ya está disponible; la
eliminación automática de duplicados difusos sigue formando parte del catálogo
pendiente. La señal agregada de duplicados parecidos y la imputación conservadora
también están disponibles para revisión.

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

## Reparación segura de doble codificación UTF-8

El perfil cuenta por columna secuencias heredadas comunes como `Ã©` y `â€™` sin
mostrar celdas. Preparar ofrece una acción reversible que convierte únicamente
valores de texto cuya secuencia puede reinterpretarse y decodificarse como UTF-8
válido; los tipos no textuales, `_cambios` y valores ambiguos permanecen intactos.

## Valores incompatibles con tipos sugeridos

El perfil detecta sugerencias semánticas de booleano, entero, decimal o fecha cuando
al menos el 90% de los valores no vacíos coincide. Preparar permite confirmar una
acción que convierte únicamente los valores no vacíos que contradicen esa sugerencia
en nulos. No se muestran celdas, los valores vacíos y las columnas no textuales se
conservan, y el cambio queda disponible para deshacer desde el historial. La acción
no cambia todavía el tipo físico de la columna ni completa todas las reglas avanzadas
del catálogo.

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

## Tratamiento reversible de outliers

Preparar muestra únicamente el conteo agregado de columnas con outliers según la
regla IQR × 1.5. Las acciones directas confirmables pueden limitar cada valor al
límite correspondiente o eliminar filas con al menos un valor fuera de rango;
conservan los nulos, excluyen `_cambios` y publican una sola revisión reversible en
el historial. La acción directa de imputación reemplaza cada valor atípico por la
mediana observada de su columna y conserva los tipos `Int64`/`Float64`. Las recetas
ofrecen las tres semánticas como acciones `cap`, `drop` e `impute`.

## Imputación categórica explícita

Preparar ofrece una acción optativa separada para completar únicamente nulos de
columnas textuales con la categoría `Desconocido`. No infiere una moda, no cambia
columnas numéricas ni la columna reservada `_cambios`, publica solo filas/celdas y
columnas afectadas como impacto agregado y crea una revisión reversible en el
historial.

## Señales agregadas de privacidad e identificadores

Columnia clasifica nombres de columnas con señales conservadoras de correo,
teléfono, dirección, identificador o nombre. Solo publica la categoría y el nombre
de la columna en el perfil; no inspecciona ni devuelve muestras para esta señal.
Preparar permite revisar y retirar únicamente las columnas clasificadas como
identificadoras, con confirmación, historial reversible y conservación de al menos
una columna. Para email, teléfono, dirección y nombre ofrece además una acción
confirmable que sustituye los valores no nulos por `[REDACTED]`, conserva las
columnas y publica solo el número de celdas/columnas afectadas; no toca números,
nulos ni `_cambios`. La acción puede deshacerse desde el historial. La privacidad
de entrega conserva sus semánticas independientes de máscara o hash.

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
explícitos. Cuando Review ya tiene una comparación cargada, también permite
`INNER`, `LEFT` o `FULL JOIN` contra la tabla lógica `compared`, usando una
condición `ON` de igualdad entre una columna de cada tabla. Las claves deben
existir y compartir tipo; un preflight estima la cardinalidad antes de
materializar y la entrada y el resultado del JOIN tienen un límite de filas
para evitar materializaciones accidentales. Las agregaciones cuentan las filas
coincidentes antes de retenerlas y rechazan consultas fuera de presupuesto.
Toda la ejecución comprueba cancelación por bloques, incluida la ordenación y
la unión. No se aceptan rutas, tablas externas, escrituras ni consultas contra
una comparación no cargada. DuckDB sigue fuera de esta vertical.

Revisar conserva una actividad de las últimas cinco ejecuciones SQL: estado,
duración y filas afectadas. Al guardar un proyecto, ese resumen se serializa en
su workspace y se restaura al abrirlo; un proyecto todavía no guardado conserva
la actividad solo durante la sesión actual. No almacena el texto de la consulta,
rutas ni valores del dataset. El workspace también conserva la vista activa de
Revisar (`diagnosis` o `preview`), la etapa activa del flujo (`load`, `review`,
`prepare` o `deliver`) y el desplazamiento de la página visible de la muestra;
proyectos anteriores, etapas desconocidas y offsets inválidos vuelven a Revisar,
la primera página o Diagnóstico según corresponda. Las muestras de análisis y otras preferencias
de sesión siguen pendientes dentro de la paridad completa; la apertura segura
del último output local ya está cubierta por Entregar con revalidación en Rust.

## Brecha de migración desde `dataprepv1.1`

La migración tiene dos capas distintas:

1. **Paridad funcional:** ya existe una primera slice de calidad v3, pero todavía
   faltan reglas avanzadas del catálogo de limpieza
   sugerida, la optimización global del plan y el análisis exploratorio
   (distribuciones, correlaciones, grupos, nulos, centinelas, casi duplicados,
   series temporales completas), la severidad y las políticas de calidad que
   aún no tienen equivalencia segura, los conectores PostgreSQL/MySQL/SQL Server,
   los bundles auditables y el procesamiento
   fuera de memoria; Excel y SQLite locales ya están cubiertos en la primera
   vertical de entrega.
2. **Compatibilidad de artefactos:** Columnia ya importa parcialmente contratos
   JSON de reglas de calidad de DataPrep v1–v3, guarda el documento canónico
   Columnia v1 y produce una conversión segura con informe de omitidas. Los
   manifiestos de sesión dejan un resumen estructural sin rutas ni snapshots
   activables. `mask_pii` ya se representa y se reproduce en fallback como
   máscara local predeterminada y conservadora; los modos hash/clave explícita
   siguen requiriendo revisión. Una fixture representativa ya verifica fallback a fuente,
   metadatos de hoja/etapa, operaciones deterministas, reglas y artefactos no
   portables sin copiar valores; la restauración completa de sesiones y el
   round-trip con snapshots de libro e historiales representativos siguen
   pendientes en la Fase M1 del roadmap.

Columnia ya tiene una representación nativa distinta —Tauri/Rust/Polars,
proyectos SQLite/Parquet y comandos estrechos—, por lo que la migración no
debe copiar módulos Python ni prometer compatibilidad binaria de archivos
internos. La unidad de compatibilidad será el resultado observable y un
informe claro de cualquier operación no convertida.
