# Contexto vivo de Columnia

> Punto de entrada técnico y operativo para personas y agentes que trabajen en este repositorio.
> Este archivo describe el código que existe hoy. `ROADMAP.md` describe también decisiones y trabajo futuro.

`CONTEXTO.md` es el nombre español histórico del `CONTEXT.md` solicitado por el
proceso de revisión. Se conserva como única fuente viva para no mantener dos
documentos equivalentes que puedan divergir.

## Reauditoría incremental vigente — 2026-09-07

Base `d0e00fd`, versión `0.167.0`. La reauditoría posterior al rediseño y a la
extracción modular no encontró fallos críticos o altos nuevos en el producto.
Pasan 284 pruebas frontend, 399 Rust (4 ignoradas), 10 E2E, build, clippy,
capturas de accesibilidad y el smoke WebView2/Tauri real. El recorrido nativo
verificó proyectos, mutaciones, reapertura y cleanup dentro de 512 MiB de working
set y 256 MiB privados.

Se abrió Tier 7 con cuatro pendientes de gates: CSS 234 B sobre su presupuesto
raw, inventario IPC desincronizado con `dataset::samples`, falso positivo de
`fetch()` ODBC en la política de red y ficha de dependencias con evidencia
anterior. Véanse [informe](AUDITORIA_PROFESIONAL_2026-09-07.md),
[ROADMAP](ROADMAP.md) y [CHANGELOG](CHANGELOG.md). T6-05, T5-18/T5-20,
asistencia real, VM limpia, updater real y plataformas no Windows siguen abiertos
sin duplicarse. Usuario, escala, canal, jurisdicción y normativa permanecen
pendientes por decisión del usuario.

T7-03 quedó cerrada el 2026-09-07: la política de red separa patrones de
frontend y Rust, mantiene bloqueados los clientes HTTP reales y permite el
cursor ODBC de las pruebas. Sus tres regresiones pasan, igual que
`network:check` y `supply-chain:check`. T7-01 y T7-02 ya están cerradas; T7-04
permanece abierta.

T7-02 quedó cerrada el 2026-09-07: el inventario IPC se regeneró con los
propietarios de `dataset/samples` y la fachada modular `src/bridge/*`. El checker
valida `sourceFiles` y su existencia; la prueba de paridad consume la misma lista.
Pasan `ipc:check` y los cinco tests de contrato IPC. T7-04 permanece abierta.

T7-01 quedó cerrada el 2026-09-07: se retiraron declaraciones responsive
redundantes sin elevar los límites. El CSS generado mide 130.758 B raw y
19.939 B gzip; pasan build, los 10 E2E y la matriz visual/accesibilidad.
T7-04 permanece abierta.

## Diseño vigente — 2026-09-06

Rediseño aplicado en `feat/diseno-integral`, sin cambio de versión. Consultar
primero esta sección y el [informe de diseño](AUDITORIA_DISENO_2026-09-06.md)
para retomar el trabajo sin repetir la exploración del frontend.

- `src/styles.css`: paleta mediante variables, navegación petróleo, superficies
  neutras, tipografía local Aptos/Bahnschrift con fallback, botones, tablas y
  espaciado compartidos por Cargar, Revisar, Preparar y Entregar. Se retiraron
  declaraciones duplicadas y estilos de elementos de carga que ya no existen.
- `src/features/load/LoadPhase.tsx`: selección junto a las instrucciones y los
  formatos, ilustración SVG local y conservación de estados de carga/error/hojas.
- `src/App.tsx`: marca de columnas y corrección del doble toggle de preferencias;
  el clic evita la acción nativa porque React ya controla `open`.
- `Sistema` aplica la paleta oscura con el atributo `data-theme="system"`, no
  únicamente cuando falta el atributo. La regresión de clic/teclado y cambio
  de tema está en `e2e/design-preferences.spec.ts`.
- Validación: 284 pruebas frontend, 10 E2E y `cargo check` aprobados. Matriz de
  48 estados (4 fases × 4 anchos × 3 temas) sin overflow ni errores JS; recorridos
  adicionales de vista previa, SQL, transformaciones, reglas y preferencias.
  Evidencia de zoom 125/200 %, móvil y colores forzados aprobada en
  `.local/validation/accessibility-visual/20260906T220235Z`.
- El gate Fast completo se detiene en un fallo de formato Rust preexistente en
  `src-tauri/src/dataset.rs`, dentro de una prueba de JOIN/trazabilidad. No se
  alteró backend ni esa prueba. Los recorridos con datos usan el bridge Tauri
  simulado; no equivalen a una nueva validación nativa del motor o del instalador.
- Capturas antes/después y scripts reproducibles: `.local/design-review/`.
  Implementación: `aeb8347` y `72bc681`; presupuesto frontend aprobado sin
  ampliar límites (586.492 bytes raw, 146.847 bytes gzip).
  Servidor de inspección: `http://127.0.0.1:5173` (Vite, motor nativo no conectado).

## Reauditoría vigente — 2026-09-05

Esta sección actualiza el estado de validación de la ficha histórica que sigue.
Base revisada: `7d837819c66d22d8c2c51f12cac84a8b8014206d`, versión `0.167.0`.
**Estado:** revisión terminada; la implementación técnica de T6-01–T6-11 está
aplicada. T6-03 queda aceptada con MariaDB real y T6-08 queda aceptada con
tres recorridos nativos dentro del presupuesto. T6-05 conserva únicamente la
aceptación SQL Server pendiente; T5-18/T5-20 siguen requiriendo revisión legal.
Véanse [informe](AUDITORIA_PROFESIONAL_2026-09-05.md),
[ROADMAP](ROADMAP.md) y [CHANGELOG](CHANGELOG.md).

- Alcance acordado: escritorio exhaustivo, sin CLI, workflows ni GitHub Actions.
- Pruebas posteriores: 284 frontend con cobertura crítica aprobada y 397 pruebas
  Rust de backend aprobadas (4 ignoradas por benchmark/integración externa). El
  harness ODBC real pasa PostgreSQL 17.11 y MariaDB 10.11 por las rutas frame y
  source-backed, incluyendo el modo `NO_BACKSLASH_ESCAPES`.
- La cobertura crítica de proyectos queda en 81,08 % de ramas, por encima del
  umbral de 75 %.
- El recorrido nativo de proyectos es funcional y limpia sus artefactos. Tras
  diferir el sondeo y el montaje visual del panel de recursos, tres recorridos
  aislados pasaron con 245,35, 243,92 y 250,76 MiB privados, y 458,98, 458,16 y
  462,56 MiB de working set.
- Un JOIN conserva ahora el cursor Parquet publicado como source-backed y la
  trazabilidad vuelve a materializar sus filas activas; guardar y reabrir siguen
  pendientes de validación en recorrido nativo completo.
- ODBC sincroniza el dialecto visible, descarta comprobaciones obsoletas,
  parametriza valores y bloquea el reemplazo MySQL inseguro. PostgreSQL y MariaDB
  tienen round-trip real; SQL Server permanece pendiente porque la instancia
  local está detenida y no puede iniciarse sin elevación.
- Sigue pendiente la definición de jurisdicción/canal, revisión jurídica,
  asistencia real y validación de plataformas/instalación fuera de esta estación.

La ficha y bitácora anteriores se conservan como evidencia histórica; sus frases
de aprobación no sustituyen los resultados rojos de esta reauditoría.

## Ficha rápida

| Campo | Estado verificado |
| --- | --- |
| Última actualización | 2026-09-07; reauditoría incremental cerrada sobre `d0e00fd` |
| Producto | Estación de escritorio local para revisar, limpiar, transformar y entregar datasets confiables |
| Versión | `0.167.0`, sincronizada en npm, Cargo y Tauri |
| Arquitectura implementada | Tauri 2 + Rust + Polars + React 19 + TypeScript + Vite |
| Licencia y distribución | MIT; distribución abierta inicial, sin telemetría ni servicio remoto obligatorio |
| Plataformas objetivo | Windows x64 como soporte inicial; macOS y Linux como objetivos de diseño hasta validación local |
| Plataforma verificada inicialmente | Windows x64 |
| Persistencia actual | Proyectos SQLite con dataset, reglas, borrador, perfil cacheado con huella SHA-256 del snapshot actual, historial/cursor, actividad SQL agregada, vista y etapa activa de Revisar, página visible de la muestra, motor SQL elegido, cobertura de correlaciones, perfil de rendimiento, formato de exportación, protección de datos, claves de comparación y tipo de JOIN durables; cada apertura crea copias temporales de sesión |
| Red y servicios externos | No requeridos para trabajar con datos locales; la entrega opcional a PostgreSQL, MySQL y SQL Server usa el controlador ODBC instalado y solo bajo acción explícita |
| Validación | Local mediante `tools/check.ps1`; no hay CI por decisión del proyecto |
| Pruebas observadas | La suite local actual mantiene 284 pruebas frontend y 397 pruebas Rust aprobadas, con 1 benchmark de escala ignorado explícitamente; `perf:benchmark`, `perf:webview2` y `perf:check` pasan con evidencia fresca de 100 MiB, tres corridas sostenidas, dos actualizaciones durables, 819.137 filas WebView2 y cleanup confirmado; E2E y Package históricos pasan en la estación auditada; smoke nativo Win32 y smoke NSIS instalado pasan con cleanup y presupuesto de memoria |
| Última revisión de este documento | 2026-09-04, rama `master`; implementación técnica de Tier 5 mayormente cerrada. Preparar incorpora imputación reversible de outliers por mediana, acciones IQR directas source-backed para limitar, imputar y eliminar filas atípicas, eliminación source-backed de duplicados parecidos, correcciones recomendadas source-backed que combinan trim y renombres, imputación categórica explícita como `Desconocido`, protección reversible de valores personales con `[REDACTED]`, interpretación conservadora de fechas, conversión numérica segura, separación source-backed de tipos incompatibles y corrección source-backed de secuencias mojibake inequívocas con fallback eager seguro; Cargar ofrece datasets de ejemplo locales sin exponer rutas; la superficie pública se mantiene limitada a capacidades nativas de Columnia; la entrega opcional ODBC cubre PostgreSQL, MySQL y SQL Server con prueba de conexión y políticas de tabla sin persistir credenciales, y transmite fuentes source-backed compatibles por bloques sin llenar el `DataFrame` activo; el inventario IPC registra 67 comandos de producción y 58 estructuras compartidas, y el gate de cobertura crítica por capa pasa sus cinco archivos. Los perfiles persistidos quedan ligados por SHA-256 al snapshot durable y se invalidan si `current.parquet` cambia; el workspace también restaura la vista y etapa activa de Revisar, la página visible de la muestra, el motor SQL elegido, la cobertura de correlaciones, el perfil de rendimiento, el formato de exportación, la protección de datos, las claves de comparación y el tipo de JOIN elegido por proyecto, con fallback seguro y migración SQLite v12. El benchmark formal de tres actualizaciones ya cumple 100 MiB y <60 s por guardado; la comparación inicial de `.xlsx` y `.xlsb` genera snapshots Parquet por bloques y conserva fallback para `.xls`/`.ods`; las comparaciones iniciales reutilizan el snapshot Parquet del activo o una fuente original Parquet/CSV/TSV/TXT intacta cuando es posible, sin clonar el `DataFrame`; las aperturas grandes de `JSON`, `JSONL` y `NDJSON` generan snapshots Parquet privados de DuckDB y dejan el frame activo en modo esquema-only; la restauración de proyectos durables también comprueba el presupuesto de RAM antes de leer `current.parquet` completo y conserva la sesión activa si la admisión falla; las lecturas eager indirectas de fuentes comparadas incompatibles, snapshots Parquet comparados, undo/redo y automatización pasan por la misma admisión antes de leer filas; Deshacer/Rehacer source-backed ya restaura esquema, conteo y primera página desde el cursor Parquet sin materializar la revisión completa; las consultas DuckDB fijan 512 MB, derrame privado de hasta 8 GB y cleanup por operación; las recetas source-backed ya pueden filtrar, seleccionar, renombrar, convertir tipos, parsear fechas fijas e ISO seguras, extraer partes de fecha, reemplazar texto literal, dividir y unir columnas de texto, calcular columnas simples y aplicar tratamientos IQR directamente sobre CSV/TSV/TXT delimitado o Parquet; las exportaciones source-backed de Bundle, Excel `.xlsx` y SQLite transfieren datos desde DuckDB sin materializar el `DataFrame` activo y conservan atomicidad, cancelación, validación de cambios y cleanup; los JOINs source-backed `INNER`/`LEFT`/`FULL` entre fuentes locales CSV/TSV/TXT/Parquet generan solo el resultado Parquet, limitan cardinalidad y dejan historial reversible con fallback eager para formatos incompatibles; conflictos paginados, resolución y consolidación también reutilizan snapshots Parquet durables del cursor actual de datasets materializados, con validación de cursor y fallback eager; `npm run brand:check` inspecciona el árbol activo para impedir regresiones de nomenclatura; updater firmado, política de rotación, contrato local de manifiesto, verificador de assets, selectores nativos y baseline release ligado a commit limpio pasan; el gate legal técnico y el inventario de avisos pasan, mientras la aprobación jurídica, la VM limpia y la validación del canal siguen pendientes |

### Estado verificable de Tier 5

La versión 0.167.0 conserva durante el recorrido inicial del perfil source-backed
los candidatos categóricos acotados y evita un recorrido de descubrimiento para
cada columna elegible. También acelera la normalización ASCII, descarta parseos
de fecha imposibles antes de intentar los formatos completos y configura el
lector Parquet para procesar columnas en paralelo, conservando la paridad, la
cancelación y los límites de memoria.

La versión 0.166.0 paraleliza por bloque el cálculo de huellas normalizadas y
la actualización de acumuladores de columnas en el perfilado source-backed.
La salida conserva el orden de columnas, la cancelación y la paridad del
perfil, pero ya puede usar el pool de concurrencia configurado en vez de
quedar limitada a un hilo.

La versión 0.165.0 elimina del perfilado source-backed el derrame temporal de
claves de fila completas: DuckDB calcula conjuntamente filas distintas y
distintos no nulos por columna en una sola agregación. Las huellas de
duplicados parecidos y los acumuladores semánticos siguen acotados y se
conserva la paridad del perfil, incluidos nulos y repeticiones exactas.

La versión 0.164.0 extiende la ruta source-backed a la automatización y al
guardado/exportación de proyectos grandes. La CLI evita materializar todas las
filas, los perfiles de proyecto se calculan por bloques y los distintos de
todas las columnas se obtienen con una sola agregación DuckDB, reduciendo la
E/S del análisis de datasets de varios gigabytes.

La versión 0.163.0 optimiza Deshacer/Rehacer source-backed: el cursor se valida
contra el Parquet y se prepara desde su esquema, conteo y primera página, sin
leer la revisión completa al `DataFrame` activo. La publicación de un error de
lectura deja intactos el cursor y la sesión.

La versión 0.162.0 reduce las lecturas repetidas del perfilado source-backed:
duplicados exactos, duplicados parecidos y perfiles de columnas comparten un
recorrido Parquet, y las correlaciones numéricas comparten otro recorrido para
su muestra. El recorrido de perfilado usa bloques de 65.536 filas, mientras las
consultas normales conservan bloques de 16.384 filas. El progreso comunica el
avance del análisis conjunto y las pruebas mantienen paridad con el perfil en
memoria.

La versión 0.161.0 añade cobertura de extremo a extremo para resolución de
conflictos y consolidación desde snapshots Parquet durables de datasets
materializados. Las pruebas verifican publicación reversible, orden estable y
un frame activo sin filas materializadas.

La versión 0.160.0 extiende la lectura desde snapshots Parquet durables a la
paginación de conflictos, la resolución de decisiones y la consolidación de
datasets materializados. Estas rutas reutilizan el cursor actual sin volver a
materializar el frame y rechazan resultados si la fuente o el cursor cambia
durante la lectura.

La versión 0.159.0 endurece el updater: compara el tamaño real de la descarga
con el manifest antes de conservar el payload como instalable, rechazando
truncados y excedentes. La firma criptográfica continúa verificándose durante
la instalación mediante el plugin oficial.

La versión 0.158.0 endurece las lecturas incrementales: la paginación, la
comparación y las consultas DuckDB validan el snapshot Parquet del cursor antes
de usarlo, y la validación de calidad comprueba que la fuente o snapshot siga
siendo el dataset activo al terminar. Un historial degradado conserva el
fallback seguro a fuente o `DataFrame`.

La versión 0.157.0 extiende la entrega remota ODBC a datasets materializados
con snapshot Parquet durable en el cursor actual. PostgreSQL, MySQL y SQL Server
reciben las filas por bloques desde DuckDB sin llenar el `DataFrame` activo; se
mantienen las compuertas incrementales de calidad y privacidad, la cancelación y
la comprobación de que el cursor siga vigente. Las reglas o formatos no
compatibles continúan usando el fallback eager.

La versión 0.156.0 extiende la frontera de disco a operaciones de lectura sobre
datasets materializados con un snapshot Parquet durable: perfilado, validación
de reglas incrementales y exportaciones locales compatibles reutilizan el cursor
actual por bloques. Las exportaciones verifican que el cursor no cambie mientras
se prepara y publica el destino; las reglas, formatos o historiales que no son
compatibles mantienen el fallback eager.

La versión 0.155.0 extiende los JOIN mutadores `INNER`, `LEFT` y `FULL` a
datasets activos ya materializados cuando conservan un snapshot Parquet durable
en el cursor actual. DuckDB lee ese snapshot y la comparación directamente desde
disco, publica solo el resultado como una revisión reversible y deja el frame
activo sin filas; si el historial está degradado o el cursor no es compatible,
la ruta eager existente sigue siendo la alternativa explícita.

La versión 0.154.0 lleva la guardia de RAM a las lecturas eager indirectas:
comparaciones incompatibles, snapshots Parquet comparados, restauración de
undo/redo y automatización verifican el tamaño antes de leer filas. Cuando la
admisión falla, la operación devuelve un error sin sustituir la sesión activa.

La versión 0.153.0 aplica la guardia de materialización también a la apertura
de proyectos durables: el tamaño de `current.parquet` se comprueba antes de
leer sus filas, y una admisión insuficiente devuelve un error sin reemplazar
la sesión activa. Los snapshots pequeños siguen restaurándose con el contrato
actual.

La versión 0.152.0 protege los fallbacks eager que necesitan materializar una
fuente source-backed grande: estima la expansión del dataset, reserva memoria
de seguridad y detiene la operación con un mensaje accionable cuando la RAM
disponible no alcanza. Las operaciones source-backed compatibles conservan su
ruta incremental.

La versión 0.151.0 añade una ruta source-backed para `Corregir codificación
UTF-8`: las secuencias mojibake inequívocas se reparan mediante una proyección
DuckDB y snapshot Parquet reversible, sin llenar el frame activo. Si el valor
contiene caracteres o secuencias fuera del conjunto seguro, la operación
mantiene el fallback eager para conservar la semántica existente.

La versión 0.150.0 extiende la ejecución source-backed a `Apartar tipos
incompatibles`: DuckDB calcula la inferencia sobre la fuente o snapshot Parquet
efectivo, aplica la misma confianza eager del 90 % para booleanos, enteros,
decimales y fechas, publica un snapshot reversible y conserva el frame activo
esquema-only. Los casos incompatibles mantienen fallback materializado.

La versión 0.149.0 corrige la validación de parseos ISO en sesiones
source-backed con snapshot: la comprobación usa el cursor Parquet efectivo y
conserva la ruta incremental después de filtros previos; la regresión cubre
que el frame activo siga en modo esquema-only.

La versión 0.148.0 refresca la evidencia de rendimiento: `perf:benchmark`
completa el ciclo durable CLI de 100 MiB con tres corridas sostenidas y dos
actualizaciones, mientras `perf:webview2` completa 100 MiB/819.137 filas con
cleanup. `perf:check` aprueba el baseline compuesto; aún queda decidir el
presupuesto global final y cerrar la optimización general fuera de RAM.

La versión 0.147.0 extiende la entrega ODBC a la frontera source-backed: las
fuentes compatibles se transmiten por bloques desde DuckDB y no se materializa
el `DataFrame` activo. La validación de calidad usa el contrato incremental y
`mask`/`hash` crea solo un snapshot Parquet temporal cuando debe proteger datos;
las fuentes o reglas incompatibles mantienen el fallback materializado explícito.

La versión 0.146.0 añade destinos remotos PostgreSQL, MySQL y SQL Server por
ODBC. La interfaz exige una conexión verificada con `SELECT 1`, valida esquema,
tabla y política (`create_only`, `append` o `replace`) y ejecuta inserciones por
lotes dentro de una transacción. La cadena ODBC no se persiste en el workspace,
las contraseñas no se imprimen en errores y la entrega remota se mantiene
separada de la CSP local: solo ocurre cuando el usuario la solicita.

La versión 0.145.0 conecta los hitos de `desktop-smoke` con el gate de
rendimiento: `perf:summary` calcula `desktopProcessToWindowVisibleMs` y
`perf:check` exige que sea como máximo 1.000 ms, con Vite, proceso, ventana y
cleanup confirmados. El tiempo anterior a que el proceso nativo esté listo se
conserva como coste de compilación/debug y no se presenta falsamente como
latencia de la aplicación.

La versión 0.144.1 corrige la redacción editorial de los documentos de release
y de la plantilla de notices. El inventario se regenera desde `package-lock.json`
y `src-tauri/Cargo.lock`, conserva 999 identidades únicas y el test de supply
chain verifica el encabezado `Versión`.

La versión 0.144.0 cierra la evidencia de la consulta source-backed con JOIN
para una fuente CSV de 537.286.551 bytes y 1.810.432 filas. DuckDB procesa
`INNER`, `LEFT` y `FULL`, conserva conteos, páginas y orden, deja el frame
activo sin filas y confirma cleanup con un working set máximo de 233.816.064
bytes. La evidencia está en
`.local/validation/duckdb-join-benchmark/20260902T013642Z`; la ejecución
incremental de todas las operaciones y el presupuesto global de la aplicación
siguen siendo límites separados.

La versión 0.143.0 amplía la resolución source-backed de conflictos por clave:
valida por bloques solo el índice y las columnas divergentes, sin construir en
memoria los valores de todos los conflictos. El límite explícito sube a 8.192;
las decisiones por fila o por columna se publican como un snapshot Parquet
reversible y el frame activo permanece esquema-only. Las entradas fuera de
presupuesto mantienen el fallback eager sin publicar una salida parcial.

La versión 0.142.0 completa la resolución source-backed acotada de conflictos
por clave cuando la comparación tiene hasta 2.048 conflictos y conserva un
esquema compatible. DuckDB aplica decisiones por fila o por columna sobre los
snapshots Parquet, publica un resultado reversible y mantiene el frame activo
esquema-only.

La versión 0.141.0 evita materializar el activo source-backed al abrir la
página de conflictos por clave. Para fuentes locales compatibles crea un
snapshot Parquet temporal en disco, calcula el índice de conflictos por bloques
y devuelve solo la página agregada; el frame activo conserva esquema-only y la
fuente se valida antes y después. La resolución completa se amplía en v0.142.0
con una ruta source-backed acotada y fallback eager para los casos fuera de
presupuesto o incompatibles.

La versión 0.140.0 amplía la consolidación source-backed por claves desde la
comparación existente: cuando el activo conserva una fuente CSV/TSV/TXT o
Parquet y la comparación está en su snapshot Parquet temporal, DuckDB valida
duplicados y conflictos, calcula solo las claves nuevas y publica el resultado
como Parquet reversible. El frame activo permanece en modo esquema-only, se
conservan las fuentes originales y los formatos incompatibles siguen usando
fallback eager.

La versión 0.139.0 amplía el JOIN source-backed desde la consulta paginada
hacia la mutación multidataset: `INNER`, `LEFT` y `FULL` pueden combinar un
activo source-backed con CSV, TSV, TXT delimitado o Parquet directamente en
DuckDB. Solo se materializa el resultado a Parquet, se comprueba la
cardinalidad antes de escribir, se preserva el orden de entrada y el cursor se
publica con historial reversible; formatos comparados incompatibles conservan
el fallback eager.

La versión 0.138.0 mantiene la exportación incremental de Bundles
source-backed cuando existe una receta activa. DuckDB transfiere el dataset
desde la fuente o snapshot privado y el paquete agrega `recipe.json` validado,
su referencia y su hash al manifest, sin materializar el `DataFrame` activo;
la fuente original permanece protegida por las comprobaciones de tamaño,
cancelación, atomicidad y cleanup.

La versión 0.137.0 permite exportar fuentes source-backed con protección
`mask`/`hash` sin materializar el `DataFrame` activo. DuckDB construye un
snapshot Parquet privado con las columnas personales protegidas, conserva los
nulos y las columnas restantes, y la exportación usa esa entrada para todos
los destinos locales; la fuente original se valida antes y después y el
directorio temporal se elimina al finalizar.

La versión 0.136.0 ejecuta source-backed la eliminación de duplicados parecidos
y las correcciones recomendadas. DuckDB construye claves normalizadas y
exactas, conserva repeticiones idénticas y el orden de entrada, o combina
trim y renombres en una sola proyección; ambas rutas publican snapshots
reversibles, mantienen el frame activo en modo esquema-only y conservan
fallback eager para fuentes incompatibles.

La versión 0.135.0 ejecuta source-backed las acciones directas de outliers
`cap`, `impute` y `drop`. DuckDB calcula cuantiles lineales, límites IQR y la
mediana compatible con la ruta eager, valida finitud y precisión, cuenta filas
y celdas afectadas y publica snapshots reversibles; `cap` conserva su salida
`Float64`, `impute` conserva los tipos numéricos y `drop` elimina el conjunto
unido de filas atípicas. El frame activo permanece en modo esquema-only y las
fuentes incompatibles mantienen el fallback eager.

La versión 0.134.0 ejecuta source-backed la imputación conservadora y la
imputación categórica. DuckDB obtiene modas de texto con desempate por primera
aparición y medianas inferiores para tipos numéricos, o usa `Desconocido` en
la operación categórica; conserva tipos, orden, conteos exactos, snapshots
reversibles y el frame activo en modo esquema-only. Las fuentes incompatibles
mantienen el fallback eager.

La versión 0.133.0 ejecuta source-backed la conversión numérica y la
interpretación de fechas detectadas. DuckDB calcula estadísticas agregadas y
proyecta tipos solo cuando se cumple la misma política eager: excluye
identificadores y códigos con ceros iniciales, rechaza pérdida de precisión,
exige más de 80% de parseo en la muestra, admite como máximo 1% de nulos
adicionales y limita los años a 1900–2100. Las operaciones cuentan cambios en
disco, publican snapshots reversibles y dejan el frame activo en modo
esquema-only; una fuente incompatible conserva el fallback eager.

La versión 0.132.0 ejecuta source-backed la normalización de booleanos. DuckDB
clasifica columnas con la misma regla eager del 90%, transforma alias
reconocidos y conserva el resto, conteos exactos, orden, snapshots reversibles
y el frame activo en modo esquema-only.

La versión 0.131.0 ejecuta source-backed el recorte de espacios, la
normalización de texto y la normalización de valores centinela. DuckDB calcula
los cambios por columna y fila, conserva el orden y deja el frame activo en
modo esquema-only; los snapshots son reversibles y el fallback eager permanece
para modos no equivalentes.

La versión 0.130.0 ejecuta source-backed la normalización de nombres de
columnas y la activación de `_cambios`. DuckDB proyecta nombres normalizados con
colisiones deterministas o añade la columna de auditoría sin cargar las filas;
ambas acciones publican snapshots Parquet reversibles y conservan fallback
eager para fuentes no compatibles.

La versión 0.129.0 ejecuta la máscara de valores personales source-backed
directamente en DuckDB. Cuenta únicamente celdas no nulas que aún no están
redactadas, conserva el esquema y las filas fuera del `DataFrame` activo,
actualiza `_cambios` y publica un snapshot Parquet reversible; si la fuente no
es compatible, conserva el fallback eager.

La versión 0.128.0 lleva a DuckDB el retiro source-backed de columnas
identificadoras y personales detectadas por categoría. La salida conserva el
orden y `_cambios`, publica un snapshot Parquet reversible y mantiene el
`DataFrame` activo en modo esquema-only; el contrato IPC de datos personales
continúa devolviendo solo el conteo, sin nombres ni valores.

La versión 0.127.0 amplía las limpiezas source-backed compatibles: DuckDB
elimina duplicados conservando la primera aparición y detecta columnas
constantes, completamente vacías o con alta nulidad mediante agregados, sin
materializar todas las filas. Cada cambio publica un snapshot Parquet privado,
actualiza `_cambios`, mantiene el límite de una columna utilizable y conserva
undo/redo cuando el presupuesto de historial lo permite; las fuentes no
compatibles vuelven al camino eager.

La versión 0.126.0 ejecuta `remove_empty_rows` sobre una fuente source-backed
compatible mediante DuckDB. Publica un snapshot Parquet, conserva el frame
activo en modo esquema-only, amplía `_cambios` y habilita un historial Parquet
reversible para undo/redo; si el presupuesto de historial no alcanza, conserva
el fallback eager seguro.

La versión 0.125.0 abre `JSON`, `JSONL` y `NDJSON` grandes con un snapshot
Parquet privado construido por DuckDB. La sesión conserva solo esquema y
preview, mientras conteo, paginación, consultas, proyectos y recetas
source-backed reutilizan el snapshot; la fuente original se valida antes y
después de la conversión.

La versión 0.124.0 extiende los filtros temporales a igualdad y desigualdad:
`Date` y `Datetime` aceptan literales ISO 8601 y comparan valores temporales
reales en eager, lazy y DuckDB, incluyendo unidades internas distintas.

La versión 0.123.0 mantiene el filtro temporal dentro del plan lazy cuando una
receta también parsea la fecha y extrae `year`, `month` o `day`; la validación
reconoce la conversión previa y conserva el orden parseo → filtro → cálculo.

La versión 0.122.0 añade filtros ordenados directos sobre columnas `Date` y
`Datetime` con literales ISO 8601. Eager, Polars lazy y DuckDB source-backed
validan las mismas fechas, conservan unidades temporales, nulos y límites
inclusivos/exclusivos; la regresión publica un snapshot Parquet y compara la
salida contra eager sin llenar el `DataFrame` activo.

La versión 0.121.0 amplía las recetas source-backed para que el parseo de una
fecha y la extracción de `year`, `month` o `day` puedan ejecutarse después de
filtros. DuckDB conserva el orden parseo → filtro → cálculo, publica el snapshot
Parquet y la regresión compara conteo, valores y estado source-backed con eager.

La versión 0.120.0 conserva el guardado de proyectos source-backed sin
materializar el dataset completo: CSV/TSV/TXT delimitado y Parquet se convierten
directamente a `current.parquet` mediante DuckDB y el conteo se verifica antes
de publicar la generación. El snapshot temporal se copia al catálogo y la
sesión activa mantiene su esquema vacío y su historial diferido.

La versión 0.119.0 extiende las recetas source-backed con divisiones
calculadas por operando literal o columna en DuckDB. La validación previa
rechaza división por cero antes de publicar, conserva nulos y mantiene el
`DataFrame` activo vacío; las entradas no compatibles conservan fallback eager.

La versión 0.118.0 extiende las recetas source-backed con reemplazos regex
globales y grupos numéricos `$1`–`$9` en DuckDB. Conserva nulos, conteo de
celdas modificadas, snapshot Parquet y `DataFrame` activo vacío; las formas de
sustitución no compatibles mantienen el fallback eager.

La versión 0.117.0 extiende la protección source-backed de exportaciones a
`mask` y `hash`: DuckDB proyecta las columnas detectadas a un snapshot Parquet
privado y los siete destinos locales reutilizan esa fuente sin materializar el
`DataFrame` activo. Se conservan nulos, columnas protegidas, cancelación y la
validación final del archivo original, incluidos libros XLSX/XLSB.

La versión 0.116.0 extiende la apertura source-backed a libros XLSX/XLSB
grandes: Calamine detecta esquema y tipos en streaming, escribe snapshots
Parquet temporales por bloques y deja el `DataFrame` activo vacío. Paginación,
perfilado, consultas DuckDB y exportaciones compatibles reutilizan el snapshot,
validan el tamaño del libro original y permiten materializar después; XLS/ODS
conservan el fallback materializado.

La implementación actual cerró técnicamente los gates de cobertura por capa,
`SkipPackage`, zoom/reflow de 125% y 200%, inventario IPC, toolchains, notices,
confinamiento batch, reintento del catálogo, reconciliación de generaciones y
endurecimiento Playwright y updater autenticado. El gate técnico legal/distribución
valida la ficha estructurada y bloquea los perfiles `Release`/`Package` mientras
falte aprobación jurídica. El fast-path de fingerprints normalizados mantiene
la semántica de duplicados parecidos y reduce el `project-save` de 100 MiB a
59.75 s en una corrida corta y el benchmark formal registra `project-save` en
52.09 s con tres actualizaciones entre 56.37 y 57.31 s
(`.local/validation/performance-benchmark/20260828T184531Z`).

Siguen siendo bloqueantes antes de publicar: validar instalación/actualización
en una VM Windows limpia y contra un canal real, y cerrar las decisiones legales de canal,
jurisdicción, responsable, contacto, retención y rotación del updater. La
evidencia release ya está ligada a un `HEAD` limpio: el baseline conserva el
commit aprobado, los dos lockfiles y los cinco hashes visuales; su commit
posterior solo modifica el propio baseline. El recorrido de selectores nativos
ya pasa en una sesión gráfica habilitada dentro de ambos presupuestos de memoria.
El orquestador local `tools/release.ps1` ya está implementado; solo ejecuta
gates locales y no publica ni etiqueta.

La consulta Polars simple sin comparación ya puede consumir el snapshot Parquet
del cursor por bloques de 16K filas: valida el esquema, cuenta coincidencias y
retiene solo la página o los acumuladores. Si el snapshot no coincide o no puede
leerse, la sesión vuelve al `DataFrame` activo. Todos los JOIN compatibles se
promueven automáticamente a DuckDB cuando existen snapshots o fuentes de disco
válidas de ambos lados, incluso por debajo del umbral de entradas. Desde
v0.107.0, si solo la comparación tiene un snapshot Parquet y el activo ya es
un `DataFrame` transformado, el mismo `JOIN` puede registrar el activo en una
vista temporal y leer la comparación directamente desde disco, evitando
materializar de nuevo todas sus filas. Cuando el
historial está degradado, el dataset no ha sido mutado y la fuente original es
CSV, TSV, TXT delimitado o
Parquet, DuckDB también puede leer el activo directamente desde disco y combinarlo
con el snapshot comparado. Si la fuente cambia o no es compatible, conserva el
fallback materializado seguro. La ejecución incremental general fuera de RAM
sigue siendo un límite explícito. En la ruta Polars `FULL`,
las filas derechas no emparejadas se recorren en bloques de 16K mediante un
índice temporal de claves, sin materializar el anti-join derecho completo; los
`DataFrame` fuente siguen teniendo los límites actuales. La paginación de
conflictos sobre el snapshot comparado usa la misma frontera por bloques de 16K:
derrama un índice global de claves para conservar duplicados entre bloques,
relee solo una página y un bloque de valores, y valida el conteo registrado antes
de publicar la respuesta; la comparación inicial de `.xls`/`.ods` y la ejecución
incremental general siguen pendientes. Cuando la fuente comparada es Parquet, CSV, TSV, TXT
delimitado o JSON, la comparación inicial conserva un snapshot temporal y
calcula sus métricas, claves y conflictos paginados por bloques; `.xlsx` y
`.xlsb` también generan el snapshot en bloques mediante el lector de celdas
secuencial de Calamine; `.xls` y `.ods` siguen el camino materializado.

Cuando el historial está degradado y la fuente original CSV, TSV, TXT delimitada
o Parquet permanece intacta, las consultas compatibles elegidas como Polars
intentan la ruta DuckDB desde disco antes de volver al `DataFrame`; esto incluye
JOINs con snapshots comparados y conserva el fallback seguro ante incompatibilidades.

El benchmark opt-in `npm run perf:duckdb:join` genera una fuente CSV temporal de
al menos 512 MiB y comprueba `INNER`, `LEFT` y `FULL JOIN` source-backed con
DuckDB. La corrida aprobada registra 537.286.551 bytes, 1.810.432 filas,
working set máximo de 233.816.064 bytes, conteos y páginas exactos, frame activo
vacío y cleanup confirmado. La evidencia mide esta ruta concreta; no declara
cerrada la ejecución incremental de todas las operaciones ni el presupuesto
global de la aplicación.

Desde v0.109.0, la apertura source-backed conserva el esquema y la primera
página mediante Polars, pero obtiene el conteo total con `COUNT(*)` de DuckDB
directamente sobre CSV/TSV/TXT delimitado o Parquet. El conteo comparte la
cancelación cooperativa de la operación, no crea un snapshot intermedio y
mantiene el `DataFrame` activo sin filas. Esto reduce la retención de la apertura
sin presentar todavía la ejecución integral del `DataFrame` fuera de RAM como
resuelta.

Desde v0.110.0, la exportación source-backed a CSV también usa DuckDB para
convertir directamente CSV/TSV/TXT delimitado o Parquet cuando no hay receta,
privacidad adicional ni reglas de calidad no incrementales. La salida se crea
en un temporal, neutraliza prefijos de fórmulas de hoja de cálculo, comprueba
cancelación y cambios de la fuente y se publica atómicamente; las rutas que
necesitan transformaciones o protecciones adicionales conservan materialización.

Desde v0.111.0, la exportación source-backed a SQL usa DuckDB sobre esas mismas
fuentes y escribe un script portable con esquema, literales escapados,
transacción y publicación atómica. La ruta conserva cancelación, comprueba que
la fuente no cambie y no materializa el `DataFrame` activo; recetas y reglas no
incrementales siguen usando materialización, mientras la privacidad adicional
se aplica desde v0.137.0 sobre un snapshot Parquet privado intermedio.

Desde v0.137.0, todas las exportaciones locales source-backed compatibles
(CSV, JSON, Parquet, SQL, Excel, SQLite y bundle) conservan la ruta DuckDB
cuando se solicita `mask` o `hash`. Las columnas personales se transforman en
un snapshot temporal, los nulos y columnas restantes se preservan, y la fuente
original se valida antes y después de la transferencia; el `DataFrame` activo
no se materializa.

Desde v0.112.0, las consultas source-backed compatibles mantienen `INNER`,
`LEFT` y `FULL JOIN` en DuckDB cuando la fuente está en disco, conservando solo
la página y los acumuladores necesarios. Si la consulta no es compatible, la
ruta devuelve un error explícito en lugar de materializar silenciosamente el
dataset source-backed. El benchmark de 512 MiB confirma conteos, paginación,
working set y cleanup; la ejecución integral fuera de RAM aún no está cerrada.

La preferencia del motor SQL de Revisar (`Polars`/`DuckDB`) se conserva por
proyecto en el workspace SQLite v12, con validación cerrada y fallback a la
preferencia local para catálogos anteriores. No cambia el contrato IPC.

El workspace conserva también el formato de exportación (`csv`, `json`,
`parquet`, `sql`, `excel`, `sqlite` o `bundle`), la protección (`none`, `mask`
o `hash`), hasta dieciséis columnas clave sin duplicados y el tipo de JOIN
(`inner`, `left` o `full`). Al reabrir, las claves se filtran contra el esquema
del snapshot restaurado; no se guardan muestras, consultas, rutas ni valores.

Los perfiles Cargo `dev` y `test` se configuran sin símbolos de depuración para
evitar `LNK1140` al enlazar el binario Tauri en Windows; por eso `npm run tauri
dev` se puede ejecutar directamente desde `Columnia` sin una variable temporal.
El perfil `release` conserva una política independiente.

La matriz de correlaciones numéricas permite seleccionar una muestra acotada de
10.000, 50.000 o 100.000 filas. El límite se valida en Rust, se conserva como
preferencia local y obliga a recalcular si el perfil cacheado tiene otra
cobertura; no cambia las estadísticas agregadas restantes.

### Validación de la implementación Tier 5

- Las suites locales actuales pasan: 271 tests frontend y 314 tests Rust; los
  últimos perfiles `Full`/`Release` históricos también aprobaron build, cobertura,
  clippy, supply chain, SBOM e instalador.
- El probe CDP funcional de ProjectsPanel mide 470.25 MiB de working set y
  253.48 MiB privados, dentro de presupuesto, ejecuta 3 ciclos sostenidos y
  confirma cleanup (`.local/validation/webview2-cdp/20260828T185215Z`). El
  recorrido nativo aislado histórico verificó abrir dataset, guardar/cargar
  receta y exportar con 521.79 MiB de working set y 265.98 MiB privados, por
  encima del límite privado de 256 MiB (`.local/validation/webview2-cdp/20260828T203917Z`).
- `npm run accessibility:visual`, `npm run accessibility:check` y, ejecutados en
  secuencia después de regenerar el resumen, `npm run perf:check` pasan; el
  benchmark `npm run perf:webview2` también pasa con un input sintético de 100
  MiB y 819.137 filas: carga 2,9 s, paginación 29 ms, transformación 1,7 s,
  exportación 1,4 s, 691.789.824 B de working set y 460.587.008 B privados,
  con cleanup confirmado. Evidencia visual:
  `.local/validation/accessibility-visual/20260828T185137Z`.
- La captura release desde un commit limpio y la aprobación del baseline visual
  ya pasan; el bundle
  firmado de prueba produjo MSI/NSIS y sus firmas `.sig`; la ruta reproducible
  está en `release:updater:dry-run` y requiere variables de entorno privadas.
  `updater:contract:test` cubre mutaciones estructurales locales y
  `updater:key:check` verifica el fingerprint/política de rotación, pero no cierra
  la validación de canal real, red o adopción de una release puente.

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

No presentes como implementada una tecnología solo porque aparece en el roadmap. DuckDB ya forma parte de las dependencias de Cargo y ofrece la primera ruta opcional de consulta SQL local: reutiliza snapshots Parquet administrados de la revisión activa y de la comparación cuando existen, lee solo el esquema del snapshot comparado durante la preparación, recibe automáticamente los JOINs Polars que superan el límite de entradas cuando ambos snapshots están disponibles y conserva fallbacks temporales para estados degradados; cuando el historial se degrada y la fuente original CSV/TSV/TXT delimitada o Parquet permanece intacta, las consultas compatibles elegidas como Polars también intentan DuckDB desde disco. Los JOINs mutadores `INNER`/`LEFT`/`FULL` entre un activo source-backed y CSV/TSV/TXT/Parquet ya publican el resultado como Parquet reversible; el `DataFrame` activo y la ejecución incremental general, la comparación inicial de fuentes `.xls`/`.ods` y el procesamiento completo fuera de la RAM del dataset siguen pendientes. Para fuentes comparadas Parquet, delimitadas, JSON y libros `.xlsx`/`.xlsb`, la comparación inicial ya conserva un snapshot secuencial y calcula sus métricas, claves y conflictos paginados por bloques; `.xls`/`.ods` mantienen fallback eager. Las recetas lazy también combinan parseos de fecha con conversiones en columnas distintas y `split` con `merge` cuando las dependencias se conservan; los conflictos de fuentes `.xls`/`.ods` siguen en fallback eager o rechazo explícito.

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

projects.rs
|  catálogo, migración, snapshots y recuperación
v
SQLite + app_data_dir privado
```

No existe un servidor HTTP de aplicación. React pide casos de uso concretos mediante IPC de Tauri. Rust conserva la autoridad sobre rutas, archivos y datasets. El frontend recibe nombres, metadatos, filas de vista previa e identificadores opacos, no rutas locales.

La automatización sin interfaz entra por `columnia-cli`, que llama directamente al mismo motor Rust sin pasar por React ni IPC. Sus comandos de datasets `inspect`, `transform`, `validate` y `batch`, y sus comandos de proyectos `project-list`, `project-save`, `project-inspect`, `project-export` y `project-delete`, emiten contratos JSON versión 1, conservan los límites y la escritura atómica del escritorio y nunca incluyen rutas, filas ni muestras en la salida. Acepta CSV, TSV, JSON, Parquet, XLSX, XLS, XLSB y ODS; los libros exigen siempre una hoja por nombre exacto y un modo de encabezado explícito. El código 0 indica éxito, el 2 una validación de calidad reprobada o un trabajo batch fallido, y el 1 un error de uso, carga o almacenamiento.

`batch` admite de 1 a 64 transformaciones en un manifiesto JSON v1 estricto. Resuelve rutas relativas desde la carpeta canonicalizada del manifiesto, aplica presupuestos de texto, comprueba todos los inputs, recetas, formatos, hojas, destinos y colisiones antes de escribir, y publica cada salida de forma atómica. No es una transacción global: un fallo dependiente de los datos detiene el lote con código 2 y conserva las salidas anteriores; el JSON informa solo conteos y el ordinal 1-based del trabajo fallido. Un manifiesto o preflight inválido termina con código 1, sin stdout ni outputs.

Los cinco comandos CLI de proyectos exigen siempre `--store <directorio>`: no infieren ni reutilizan el `app_data_dir` del escritorio. Rust canonicaliza ese almacén explícito y mantiene allí el catálogo y los artefactos administrados. `project-save` crea o actualiza por ID un snapshot materializado desde una entrada y puede adjuntar receta, reglas y perfil; `project-list` devuelve resúmenes; `project-inspect` devuelve metadatos, presencia de perfil/receta, cantidad de reglas y estado agregado del historial; `project-export` valida el proyecto completo sin activarlo ni alterar su candidato de recuperación; y `project-delete` exige que `--confirm` coincida exactamente con `--id`. Las reglas guardadas deben aprobar siempre: `--allow-unvalidated` habilita únicamente proyectos sin reglas y nunca omite una validación reprobada. La exportación publica los destinos soportados de forma atómica y los contratos informan solo nombre de archivo, tamaño, formato, calidad y metadatos agregados de privacidad, nunca la ruta ni datos del dataset.

## Flujo de producto

La interfaz sigue cuatro fases declaradas en `src/App.tsx`:

1. **Cargar**: inspecciona una fuente local desde el selector nativo o el arrastre a la ventana, permite seleccionar una hoja cuando corresponde, muestra hasta cinco archivos recientes sin persistir rutas y materializa el dataset activo.
2. **Revisar**: pagina la vista previa y calcula el perfil de calidad bajo demanda.
3. **Preparar**: aplica correcciones simples o una receta estructural atómica; ofrece Deshacer/Rehacer.
4. **Entregar**: valida un contrato de calidad y exporta CSV, JSON, Parquet, SQL, Excel o SQLite.

Las fases distintas de Cargar se deshabilitan mientras no exista un dataset. Una operación activa bloquea la navegación que pueda competir con ella. Si el dataset cambia, cualquier validación de entrega previa queda obsoleta y debe ejecutarse de nuevo.

## Arquitectura por archivo

| Ruta | Responsabilidad |
| --- | --- |
| `src/main.tsx` | Monta `<App />` en modo estricto de React y publica la marca de bootstrap usada para separar compilación fría del primer render. |
| `src/App.tsx` | Coordina el flujo principal y los estados compartidos de la interfaz en 867 líneas. |
| `src/components/` | Componentes accesibles extraídos para diálogos, tabs de revisión y progreso cancelable. |
| `src/components/ResourceMonitor.tsx` | Monitor compacto de consumo de CPU/RAM del proceso y del equipo, con polling nativo, selector persistente de concurrencia Rayon y estado degradado para el shell web. |
| `src/features/load/` | Fase Cargar: vista y modelo de inspección, selección de hojas, arrastre nativo sin rutas en React, archivos recientes sin rutas, progreso, cancelación y recuperación. |
| `src/features/review/` | Fase Revisar: diagnóstico, perfil de calidad, tabs y vista previa paginada. |
| `src/features/prepare/` | Fase Preparar: vistas, editor de recetas, historial, modelo puro y controlador de IPC/invalidationes. |
| `src/features/projects/` | Catálogo, guardado, apertura, recuperación y eliminación accesible de proyectos locales. |
| `src/features/delivery/` | Fase Entregar: vista, métricas y modelo tipado de contrato, compuerta de calidad y exportación. |
| `src/bridge.ts` | Contrato TypeScript del IPC y única fachada de `invoke()` usada por la UI. |
| `src/styles.css` | Sistema visual y layout de la aplicación; incluye foco visible, targets mínimos, reducción de movimiento y una paleta explícita para `forced-colors: active`. |
| `playwright.config.ts` | Configuración de Playwright para E2E del shell web Vite, con Chromium/Edge local, preview de producción reutilizable, trazas y artefactos solo en fallos. |
| `e2e/` | Pruebas E2E del shell web, primer render, accesibilidad, preferencias responsive y ciclo de proyectos con IPC Tauri simulado; la ventana WebView2 nativa tiene un probe CDP opcional. |
| `src-tauri/src/main.rs` | Entrada mínima del ejecutable; delega en `columnia_lib::run()`. |
| `src-tauri/src/lib.rs` | Inicializa Tauri, instancia única, diálogo nativo, eventos nativos de arrastre, estados de dataset/proyectos y los comandos permitidos. |
| `src-tauri/src/resource.rs` | Obtiene CPU y memoria del proceso Columnia y del sistema mediante `sysinfo`, sin exponer rutas ni datos. |
| `src-tauri/src/dataset.rs` | Motor de datos principal. Contiene carga, tipos, perfiles, recetas, historial y exportación; el fingerprinting de duplicados parecidos vive en el módulo interno acotado `dataset_fingerprints.rs`. |
| `src-tauri/src/dataset_fingerprints.rs` | API interna para huellas exactas/normalizadas de filas, fast-path ASCII y normalización Unicode usada por perfilado y retiro de duplicados parecidos. |
| `src-tauri/src/projects.rs` | Catálogo SQLite v12 compatible con v1/v2/v3/v4/v5/v6/v7/v8/v9/v10/v11, snapshots Parquet durables, perfil, historial, actividad SQL agregada, vista/etapa de Revisar, página de muestra, motor SQL, cobertura de correlaciones, perfil de rendimiento, preferencias de Entregar y preferencias de comparación versionados, y cinco comandos de proyectos. |
| `src-tauri/src/automation.rs` | Parser estricto, contratos JSON y orquestación reutilizable de datasets, lotes y los cinco comandos CLI de proyectos. |
| `src-tauri/src/bin/columnia-cli.rs` | Ejecutable CLI mínimo que delega en el módulo de automatización. |
| `src-tauri/capabilities/main.json` | Capability mínima para la ventana `main`: solamente `core:default`. |
| `src-tauri/tauri.conf.json` | Ventana, build, bundle y CSP de producción/desarrollo. |
| `tools/check.ps1` | Entrada única para los gates locales Fast, Full y Release; genera evidencia JSON auditable en `.local/validation/` y en Release ejecuta supply chain/instalador. |
| `vitest.config.ts` / `tools/check-coverage.mjs` | Cobertura V8 global y por capa crítica: App, Entrega, Preparar y controller con umbrales 80% statements/lines y 75% branches/functions. |
| `tools/check-supply-chain.ps1` / `src-tauri/deny.toml` | npm audit, cargo audit, cargo-deny, secretos, avisos de terceros y política de red con excepciones upstream justificadas. |
| `tools/check-network-policy.mjs` / `docs/reference/network-privacy.md` | Inventario local de red, CSP productivo y política de telemetría desactivada por defecto. |
| `src-tauri/src/privacy.rs` | Serialización pública sanitizada para reportes, recetas y manifiestos: elimina rutas, valores, emails, secretos y referencias de filesystem, conservando identificadores, estados y conteos agregados. |
| `tools/check-installer-contract.ps1` / `tools/smoke-installed-artifact.ps1` | Contrato de NSIS currentUser, WebView2 bootstrapper, smoke del artefacto instalado y recursos legales reproducibles. |
| `tools/check-updater-key-policy.mjs` / `fixtures/updater/key-policy-v1.json` | Fingerprint de la clave pública embebida, release puente para rotación y recuperación fail-closed. |
| `tools/verify-published-assets.mjs` | Descarga posterior a publicación y verificación criptográfica local de manifiesto, tamaño, SHA-256 y firma minisign. |
| `tools/generate-sbom.ps1` / `tools/extract-package-lock-packages.mjs` | Generan offline un SBOM CycloneDX 1.6 reproducible desde ambos lockfiles, compatible con Windows PowerShell 5.1. |
| `docs/reference/feature-parity.md` | Matriz de paridad verificable con `sistema anterior`, con entregas CSV/JSON/Parquet/SQL/Excel/SQLite, comparación por columna y visualizaciones accesibles documentadas. |
| `tools/check-bundle.mjs` | Mide presupuestos JS/CSS e inventaría bundles de distribución nuevos o actualizados. |
| `tools/smoke-tauri.ps1` | Arranca `npm run tauri dev`, comprueba Vite y el ejecutable debug, registra hitos monotónicos de Vite/proceso/ventana, ejecuta un preflight de contrato de `ProjectsPanel` y limpia solo su Job Object con reintento acotado. |
| `tools/probe-webview2-cdp.ps1` | Arranca el comando real con `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` de loopback, verifica `/json/version` y `/json/list`, conecta Playwright al WebView2, atribuye el perfil por proceso/fase y aplica presupuestos observables de 512 MiB de working set, 256 MiB de memoria privada y transformaciones nativas sostenidas solo a procesos de su Job Object; admite un CSV temporal configurable para medir el recorrido grande; restaura el entorno y limpia su Job Object. |
| `tools/probe-webview2-restart.ps1` | Ejecuta las fases aisladas prepare/verify del reinicio real y eleva al resumen de cada fase el estado del presupuesto y el conteo/duración IPC, delegando el cleanup al probe CDP. |
| `tools/probe-webview2-playwright.mjs` | Conecta al endpoint CDP con Playwright, espera el shell listo y separa estado funcional de presupuesto de primer render relativo al bootstrap; ambos son necesarios para aprobar el probe. |
| `tools/probe-webview2-projects.mjs` | Conecta al endpoint CDP y verifica el contrato accesible de `ProjectsPanel`, repite transformaciones/exportaciones nativas sostenidas, y en el smoke debug guarda/abre/consulta/elimina un proyecto sintético con cleanup; registra duración por comando y total, pero no rutas ni datos del catálogo. |
| `tools/summarize-performance.ps1` | Lee únicamente `summary.json` dentro de `.local/validation/`, clasifica señales web/CDP/desktop y separa `cdp-large-dataset` de la muestra CDP normal; conserva perfil de memoria, presupuestos y duraciones nativas, calcula deltas y el intervalo proceso-listo→ventana-visible, y genera `summary.json`/`summary.csv` sin rutas absolutas ni datos sensibles. |
| `tools/capture-accessibility-evidence.mjs` | Construye el preview local, captura desktop/móvil/zoom CSS 125% y 200%/forced-colors y publica capturas más un resumen sanitizado de landmarks, foco, targets y overflow bajo `.local/validation/`. |
| `tools/check-accessibility-baseline.mjs` | Compara la evidencia visual más reciente con el contrato versionado de `fixtures/accessibility/`, verificando escenarios, landmarks, targets, foco, overflow y SHA-256 de cada captura. |
| `tools/capture-release-evidence.ps1` / `tools/capture-release-evidence.mjs` | Construyen el binario Tauri sin bundle, exigen árbol limpio, registran commit/rama/lockfiles y capturan desktop/móvil/zoom CSS 125% y 200%/forced-colors desde el ejecutable optimizado. |
| `tools/check-release-evidence.mjs` | Comprueba el sumario release contra el baseline de escenarios, contrato, versiones, commit limpio, hashes de lockfiles y ownership; solo `--update-baseline` acepta una diferencia visual intencional. |
| `tools/release.ps1` | Orquesta el pre-release local con toolchains, documentación, IPC, perfil Release/Package, CLI, evidencia visual, baseline y rendimiento; exige rama/árbol limpio y nunca crea tags ni publica servicios remotos. |
| `tools/generate-updater-manifest.mjs` / `tools/check-updater-manifest.mjs` / `tools/test-updater-manifest.mjs` | Generan y verifican el par manifiesto/inventario del updater; el contrato reproducible muta un fixture local para probar truncado, firma alterada, manifiesto incompleto/corrupto y URL insegura sin contactar la red. |
| `tools/check-documentation.mjs` | Valida el mapa Diátaxis, ADR/CHANGELOG, enlaces locales, UTF-8 sin BOM, coherencia de versiones y ownership de imágenes. |
| `tools/benchmark-datasets.ps1` | Genera un CSV sintético cercano al objetivo indicado, mide iteraciones sostenidas de transform CSV/Parquet, actualiza el mismo proyecto el número indicado de veces y verifica reapertura/exportación durable; conserva solo tiempos, conteos, estados y cleanup sin datos después de borrar el almacén temporal. |
| `tools/benchmark-webview2-dataset.ps1` | Genera un CSV temporal cercano a 100 MiB, lo entrega al selector Win32 del probe y exige dentro de WebView2 carga, paginación, transformación, exportación, memoria agregada y cleanup; elimina el dataset al terminar y publica solo evidencia sanitizada. |
| `tools/benchmark-datasets.ps1` / `tools/benchmark-datasets.ps1` | Benchmark cruzado de la inspección de 100 MiB contra `sistema anterior`, con selección del entorno Python, comparación de duración/working set, validación de conteos y cleanup. |
| `tools/check-performance-baseline.ps1` | Convierte el resumen CDP, el startup desktop, el benchmark de datasets, el recorrido WebView2 de dataset grande y el reporte Package en un gate contra `fixtures/performance/performance-baseline-v1.json`, incluyendo duración máxima por operación, con evidencia sanitizada y estado explícito. |
| `tools/verify-experience.ps1` | Ejecuta juntos `accessibility:check` y `perf:check` para verificar los contratos visual y de rendimiento después de generar evidencias. |
| `tools/verify-tier.ps1` | Orquesta el tier reproducible completo: tests, build, accesibilidad, benchmark sostenido, Package, smokes CLI/WebView2 y gates finales; permite omitir Package o native de forma explícita. |
| `tools/check-toolchains.mjs` / `rust-toolchain.toml` | Rechazan Node/npm/Rust fuera de las versiones exactas del entorno de release. |
| `tools/check-ipc-inventory.mjs` / `docs/reference/ipc-inventory.json` | Generan y verifican desde Rust el inventario de 67 comandos de producción, 4 debug y 58 estructuras compartidas; los tests de contrato consumen el inventario. |
| `docs/reference/legal-distribution-review.md` / `src/App.tsx` | Hacen descubribles MIT, notices y privacidad local; la revisión legal de canal/jurisdicción/contacto sigue pendiente antes de publicar. |
| `ACCESSIBILITY_MANUAL_CHECKLIST.md` | Checklist operativa para teclado, lector de pantalla, High Contrast, zoom y evidencia manual; no declara completada la auditoría sin una sesión real. |
| `fixtures/accessibility/visual-baseline-v1.json` | Contrato versionado de escenarios y mínimos visuales; no contiene imágenes ni datos de usuario. |
| `fixtures/performance/performance-baseline-v1.json` | Presupuestos versionados de memoria CDP, startup desktop después de proceso nativo listo, transformaciones nativas sostenidas, benchmark sostenido de 100 MiB, duración por operación y bundle frontend. |
| `tools/smoke-cli.ps1` | Verifica la CLI real con fixtures deterministas, libros, calidad, lotes, atomicidad por trabajo y errores seguros. |
| `fixtures/automation/` | Entradas, receta y resultados esperados del smoke de automatización. |
| `README.md` | Descripción funcional y guía de uso/desarrollo. |
| `THREAT_MODEL.md` | Activos, fronteras de confianza, amenazas, controles implementados y riesgos residuales. |
| `ROADMAP.md` | Plan, decisiones históricas, fases y pendientes. No sustituye la inspección del código. |
| `CONTRIBUTING.md` | Ramas, commits, revisión local y límites de alcance. |
| `docs/` | Tutoriales, how-to, referencias, explicaciones, ADRs, gobierno del repositorio, auditoría de dependencias y política de fixtures. |
| `CHANGELOG.md` | Registro de cambios publicados y limitaciones conocidas por versión. |
| `fixtures/accessibility/release-evidence-baseline-v1.json` | Casos, contrato, owner, propósito, fecha de aprobación y hashes de capturas generadas desde el binario release. |
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

`LoadedDataset` conserva una ruta fuente privada opcional, el nombre/tamaño visibles, el `DataFrame`, un perfil opcional en caché y el historial. Separar la identidad visible de la ruta permite restaurar un snapshot aunque el archivo original ya no exista. El dataset activo se materializa en memoria, pero las recetas compatibles de I1 construyen y ejecutan un plan Polars lazy antes de publicar el candidato; esa colección y los lectores `LazyCsvReader`/`scan_parquet` pasan por una única frontera con el motor `streaming`, baja memoria y `rechunk` desactivado. Dentro de las calculadas, suma, resta, multiplicación, división, concatenación y extracción de año, mes o día sobre `Date`/`Datetime` sin zona horaria ya comparten esa ruta cuando no hay filtros previos; el preflight conserva la validación de fechas no representables. Las operaciones no compatibles conservan el camino eager para mantener sus validaciones estrictas. La apertura y validación de snapshots durables de proyectos y del historial temporal también usa `scan_parquet` por esa frontera, aunque al final produce el frame materializado que exige el contrato de sesión. La restauración del historial procesa cada snapshot una sola vez y conserva únicamente el frame del cursor durante la comprobación, no todos los frames históricos a la vez. Columnia no impone un límite fijo al tamaño del dataset. La capacidad efectiva depende de la RAM, el espacio temporal en disco, la CPU y la expansión propia del formato durante la lectura, el perfilado y las transformaciones. La interfaz sigue recibiendo un `DataFrame` activo para preservar el contrato actual. La paginación de la muestra activa y las consultas Polars simples sin comparación pueden leer el snapshot Parquet del cursor con lectura acotada por bloques cuando el historial está sano; las consultas cuentan coincidencias sin materializar otra copia y solo retienen la página o los acumuladores, y vuelven al `DataFrame` ante historial degradado o snapshot inconsistente. Cuando el historial está degradado, el dataset no ha sido mutado y la fuente original es CSV, TSV, TXT delimitado o Parquet, las consultas DuckDB registran el archivo directamente desde disco, incluso al unirlo con el snapshot comparado; una mutación invalida la referencia para no consultar el archivo obsoleto. `JOIN`, comparación, operaciones generales y ejecución integral fuera de memoria mantienen sus límites explícitos. La detección exacta de duplicados proyecta solo el conteo mediante `unique` lazy en streaming, y la detección normalizada recorre bloques en paralelo y derrama huellas XXH3-128 en 256 cubetas temporales para ordenar una sola cubeta en RAM; el temporal contiene fingerprints, no valores del dataset, y se elimina al terminar o cancelar. El perfilado por columna usa una cola acotada de hasta cuatro trabajadores, conserva el orden original y reporta progreso agregado por sub-etapa sin construir vectores numéricos completos. Los `JOIN` locales `INNER`/`LEFT` sin agregación procesan el lado `dataset` por bloques y conservan solo la página global y un bloque unido temporal; sus agregaciones fusionan estados por bloques sin conservar el resultado unido completo. `FULL` recorre el lado `dataset` por bloques y añade bloques de filas derechas no emparejadas mediante anti-join estable, aunque el frame derecho anti-join todavía se materializa dentro de los límites explícitos; joins/comparaciones grandes y la ejecución fuera de memoria general aún requieren la siguiente expansión incremental. Las recetas source-backed ejecutan en DuckDB las seis extracciones textuales actuales —tokens, dígitos, letras y segmentos antes/después de delimitadores— y normalizaciones de correo, teléfono y dirección sobre CSV/TSV/TXT delimitado o Parquet, conservando Unicode, nulos, coincidencias ausentes, resultados vacíos y conteos exactos sin cargar las filas en el `DataFrame` activo.

En v0.65.0, la apertura de CSV, TSV, TXT delimitado y Parquet de al menos
512 MiB es source-backed: conserva solo esquema y la primera página de 50 filas
en el `DataFrame`, calcula el total desde disco y materializa las filas completas
solo cuando una operación eager las necesita. Mientras la fuente no cambie,
la vista previa y las consultas compatibles pueden seguir leyendo directamente
desde disco; si cambia, desaparece o una operación no admite ese camino, se usa
el fallback materializado con validación de tamaño y conteo.

En v0.66.0, el perfilado de esas fuentes también conserva la frontera de
memoria: derrama firmas de duplicados por cubetas, procesa cada columna por
bloques, ordena estadísticas numéricas mediante corridas temporales y limita
las correlaciones a la muestra solicitada. Categorías y tendencias leen solo
la columna necesaria. El resultado mantiene los mismos campos del perfil
normal; el dataset solo se materializa cuando una operación posterior necesita
mutarlo o publicar todas sus filas.

En v0.67.0, la validación de calidad de reglas fila-a-fila y de esquema/conteo
recorre esos snapshots por bloques y acumula únicamente los conteos de cada
regla. La sesión source-backed conserva su esquema vacío, se comprueba el
tamaño de la fuente antes y después y las reglas globales que necesitan estado
completo mantienen el fallback materializado.

En v0.68.0, la exportación Parquet source-backed sin receta ni privacidad
adicional convierte la fuente directamente desde disco con DuckDB, publica
atómicamente y limpia el snapshot temporal sin materializar el `DataFrame`
activo. En v0.69.0, la misma ruta incorpora JSON para fuentes CSV, TSV, TXT
delimitado y Parquet, conservando orden, validación de cambios, publicación
atómica y cleanup. La validación compatible se ejecuta antes por bloques y los
demás formatos, recetas y protecciones mantienen su fallback materializado.

En v0.70.0, la validación de calidad source-backed amplía esa frontera a
`unique`, `unique_together`, `monotonic`, `aggregate_check`,
`aggregate_reconciliation` y `distribution_drift`: cada clave global se
derrama por cubetas o cada acumulador se fusiona por bloques, conservando
conteos, nulos, cancelación y comprobación del tamaño de la fuente. Las
recetas y transformaciones generales siguen materializando hasta completar
su ruta incremental.

En v0.80.0, las recetas source-backed que solo renombran y proyectan columnas
se convierten directamente desde la fuente a un Parquet privado administrado.
El dataset conserva esquema, conteo, orden y preview sin llenar el
`DataFrame`; una operación posterior que necesite valores completos todavía
materializa con validación de tamaño y conteo, y las recetas que transforman
valores mantienen el fallback eager.

En v0.92.0, esa ruta admitió hasta tres filtros combinados con selección y
renombrado. En v0.93.0, el mismo plan añade casts, fechas fijas `YMD`, `DMY` y
`MDY`, y columnas calculadas de suma, resta, multiplicación y concatenación.
En v0.94.0 también extrae año, mes y día tras un parseo de fecha fijo y sin
filtros previos. DuckDB conserva las etapas de renombrado, transformación,
filtro y proyección, publica solo el resultado Parquet y mantiene la paridad
eager; división, fechas ISO y conflictos de fecha siguen materializándose para
conservar sus validaciones estrictas.
En v0.95.0 el reemplazo literal también se ejecuta sobre la fuente después del
filtro, conserva nulos y calcula el conteo exacto de celdas cambiadas; las
expresiones regulares mantienen el fallback lazy/eager.

En v0.96.0 la unión de columnas de texto también se ejecuta sobre la fuente:
DuckDB conserva el orden de las fuentes, omite nulos sin crear separadores y
mantiene las cadenas vacías, permite casts numérico→texto validados y respeta
`keepColumns` y `dropSources` antes de publicar el snapshot.

En v0.97.0 la división literal también se ejecuta sobre la fuente: DuckDB
conserva delimitadores Unicode, segmentos vacíos, nulos y el resto en el último
destino, además de `keepColumns`, renombrados y `dropSource`; una unión posterior
puede consumir las columnas físicas dentro de la misma consulta.

En v0.98.0 las extracciones textuales también se ejecutan sobre la fuente:
DuckDB conserva tokens, dígitos ASCII, letras Unicode y segmentos antes/después
de delimitadores literales, incluidos nulos, coincidencias ausentes y resultados
vacíos; la regresión compara las seis variantes contra la ruta eager.

En v0.99.0 la normalización de contactos también se ejecuta sobre la fuente:
DuckDB conserva el recorte y minúsculas del correo, los prefijos y dígitos del
teléfono, los espacios Unicode de las direcciones y el conteo exacto de celdas;
las extracciones posteriores observan los valores ya normalizados.

En v0.100.0 los resúmenes por grupo también se ejecutan sobre la fuente:
DuckDB conserva el primer orden de aparición, agrupa claves nulas y publica
`sum`, `mean`, `min`, `max`, `count` y `count_unique` con validaciones de tipo,
precisión, overflow y finitud. El resultado source-backed mantiene separado el
conteo de grupos, filas colapsadas y filas eliminadas, y la regresión compara
salida y contadores contra eager.

En v0.101.0 los tratamientos IQR también se ejecutan sobre la fuente:
DuckDB calcula un baseline común de cuantiles por bloque lógico y soporta `cap`,
`drop` e `impute` sobre columnas numéricas, conservando nulos, tipos y el
baseline posterior a filtros. La ruta valida mínimo de valores, finitud,
precisión y umbrales, y separa celdas ajustadas, filas retiradas y filas
eliminadas por filtros.

En v0.102.0 las fechas ISO seguras también se ejecutan sobre la fuente:
DuckDB conserva fechas, horas sin offset y valores con sufijo UTC `Z`, mientras
los offsets distintos de UTC o valores inválidos vuelven al fallback eager
estricto.

| 2026-08-31 | Versión 0.95.0: las recetas source-backed ejecutan reemplazo literal en DuckDB después de filtros y antes de la proyección, con conteo exacto de celdas cambiadas y fallback para regex o caracteres no válidos. | `src-tauri/src/dataset.rs`, `src-tauri/src/duckdb_query.rs`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-08-31 | Versión 0.96.0: las recetas source-backed unen de dos a dieciséis columnas de texto en DuckDB, conservan orden, nulos, cadenas vacías, separador, casts numérico→texto, `keepColumns` y `dropSources`, con paridad contra eager. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-08-31 | Versión 0.97.0: las recetas source-backed dividen columnas de texto en DuckDB, conservan delimitadores Unicode, segmentos vacíos, nulos, resto final, `keepColumns`, renombrados y `dropSource`, y mantienen paridad con una unión posterior y la ruta eager. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-08-31 | Versión 0.98.0: las recetas source-backed extraen tokens, dígitos, letras Unicode y segmentos antes/después de delimitadores literales directamente en DuckDB, conservando nulos, coincidencias ausentes, resultados vacíos, renombrados y `keepColumns`, con paridad contra eager. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-08-31 | Versión 0.99.0: las recetas source-backed normalizan correo, teléfono y dirección directamente en DuckDB, conservando nulos, espacios Unicode, prefijos telefónicos y conteos exactos; las extracciones posteriores consumen los valores normalizados, con paridad contra eager. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-08-31 | Versión 0.94.0: las recetas source-backed extraen año, mes y día directamente en DuckDB después de parseos de fecha fijos, con validación de tipo, dependencias y paridad eager; filtros previos, fechas ISO y conflictos de conversión conservan fallback materializado. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |

### Historial y atomicidad

Las recetas IQR compatibles con lazy/streaming también pueden usar
`keepColumns` cuando la proyección conserva todas las columnas tratadas. Si una
proyección elimina una dependencia, se mantiene el fallback eager y la
validación cerrada.

En la importación de sesiones y en el ciclo durable de proyectos, los snapshots
que no son el cursor se copian byte a byte y solo consultan su footer/esquema;
el cursor se materializa para comprobar igualdad con el frame activo. Las filas
de las demás revisiones se leen bajo demanda al hacer undo/redo, evitando cargar
todo el historial en RAM durante la apertura o el guardado.
El bridge declara y valida los metadatos opcionales de sesión con enteros seguros,
banderas booleanas y categorías no portables como listas de texto; esos nombres
siguen siendo señales sanitizadas, no resultados ni cachés reanudables.

Cada revisión reversible de la sesión se guarda como snapshot Parquet en un directorio temporal:

- máximo normal: 12 entradas;
- presupuesto total: 1 GiB;
- se elimina al cerrar o reemplazar la sesión;
- si un snapshot individual excede el presupuesto, el cambio puede aplicarse, pero la reversión se desactiva y se informa el motivo;
- una receta completa publica un solo candidato o no publica nada;
- `publish_candidate` prepara la vista previa y registra el historial antes de sustituir el `DataFrame` activo;
- la exportación escribe y sincroniza un temporal antes de reemplazar el destino.

Los proyectos guardan el frame materializado como una nueva generación Parquet y actualizan después el puntero SQLite dentro de una transacción. El esquema SQLite v12 conserva reglas de calidad, borrador opcional de receta, perfil cacheado, historial con su cursor, actividad SQL agregada, vista y etapa de Revisar, página visible de la muestra del workspace, motor SQL elegido, cobertura de filas de correlaciones, perfil de rendimiento, formato de exportación, protección de datos, claves de comparación y tipo de JOIN, y migra catálogos v1/v2/v3/v4/v5/v6/v7/v8/v9/v10/v11 compatibles. El historial durable mantiene los mismos límites de 12 revisiones y 1 GiB; la actividad SQL conserva como máximo cinco estados, duraciones y conteos de filas, sin consultas, rutas ni valores; la vista solo admite `diagnosis` y `preview` y vuelve a Diagnóstico cuando falta; la etapa solo admite `load`, `review`, `prepare` y `deliver` y vuelve a Revisar cuando falta; el motor SQL solo admite `polars` o `duckdb` y vuelve a la preferencia local cuando falta; el perfil de rendimiento solo admite `conservative`, `balanced` o `maximum` y vuelve a la preferencia local cuando falta; el formato, la protección y el JOIN usan listas cerradas y vuelven a sus valores locales cuando faltan; las claves se validan sin duplicados y se filtran contra el esquema restaurado; el offset de página se valida contra el snapshot y vuelve a cero si ya no representa una página real. Abrir valida todos los artefactos antes de sustituir el dataset activo y copia el perfil, historial y actividad guardados a estructuras temporales de sesión; una corrupción hace fallar la apertura completa. La recuperación es explícita desde Cargar y no abre datos silenciosamente.

## Contrato React ↔ Rust

La superficie pública está centralizada en `src/bridge.ts` y registrada en `src-tauri/src/lib.rs`.

### Runtime y carga

- `get_app_info`
- `get_resource_usage`
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

### Proyectos y recuperación

- `list_projects`
- `get_recovery_candidate`
- `save_project`
- `open_project`
- `delete_project`

Regla de mantenimiento: cualquier cambio de nombre, argumentos, serialización o respuesta en Rust debe reflejarse en `bridge.ts` y quedar cubierto por pruebas. `src/ipc-contract.test.ts` verifica automáticamente comandos registrados, argumentos serializados, tipos de retorno superiores y las 58 estructuras compartidas del inventario generado. Las subestructuras de `TransformRecipe` tienen interfaces nominales equivalentes a Rust; los alias públicos históricos se conservan para no romper consumidores.

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
- resumen acotado de grupos por columnas categóricas no sensibles, con top 8 y “Resto”, sin categorías raras;
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
- imputación IQR reversible por mediana, con preservación de tipos numéricos y `_cambios`;
- imputación categórica explícita de nulos textuales como `Desconocido`, sin tocar números ni `_cambios`;
- protección confirmable de valores no nulos en columnas personales detectadas mediante `[REDACTED]`, conservando columnas, números, nulos e `_cambios` y con reversión desde el historial;
- agrupación con agregaciones tipadas;
- consulta SQL local de solo lectura con motor Polars predeterminado o DuckDB opcional sobre snapshots Parquet temporales; ambos conservan filtros, agregaciones por bloques, `GROUP BY` compuesto de hasta ocho columnas y `JOIN` de hasta ocho pares de claves, con conteo exacto, paginación, orden estable, claves nulas, límites de cardinalidad y esquema de claves coalescidas;
- comparación completa de filas y por claves con índices temporales particionados, conflictos paginados y consolidación de nuevas claves sin retener mapas globales de firmas en memoria;
- normalización de correos, teléfonos y direcciones;
- extracciones textuales Unicode;
- recetas JSON versión 1 guardables y cargables;
- historial multinivel Deshacer/Rehacer.
- ejecución Polars lazy para renombres, casts, filtros, búsqueda/reemplazo
  literal, selección, unión y división de columnas, columnas calculadas
  numéricas compatibles, partes temporales lazy sin filtros previos y como claves
  de agrupación, resúmenes
  agrupados incluso con filtros previos (la búsqueda y reemplazo literal o regex
  segura se
  aplica antes dentro del plan y el preflight proyecta solo las columnas
  necesarias), normalizaciones de contactos y extracciones textuales incluso
  antes de resumir grupos; las columnas calculadas numéricas y concatenadas
  también pueden alimentar sus claves y fuentes de agregación; las columnas
  derivadas por split y merge también pueden alimentar la agrupación, con
  preflight posterior a las etapas estructurales; las operaciones restantes
  usan fallback eager atómico. Las recetas pueden combinar parseos de fecha con
  conversiones en columnas distintas y ejecutar split y merge juntos si ninguna
  etapa descarta una fuente todavía necesaria.

Las recetas se validan y ejecutan en orden determinista. Una entrada inválida, pérdida de precisión, división por cero o conflicto entre pasos revierte el lote completo.

### Entrega

- exportación atómica a CSV, JSON, Parquet, SQL, Excel y SQLite, con neutralización de fórmulas de texto en CSV; el bundle ZIP auditable añade `recipe.json` validada cuando existe un borrador y la referencia/hash correspondiente en `manifest.json`;
- contratos de hasta 16 reglas base y avanzadas: `allowed_values`, `regex`, `dtype`,
  unicidad compuesta, comparación, referencias, monotonía, agregados, drift,
  fechas, condiciones, esquema y conteo de filas;
- tolerancias por cantidad y/o porcentaje, resultados con conteos y confirmación
  explícita para exportar sin reglas;
- importación desde sistema anterior y guardado como `columnia-quality-rules` v1, con
  diálogos nativos y rutas privadas en Rust.

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

Para ejecutar el E2E reproducible del shell web (sin IPC nativo) instala una vez el navegador y ejecuta:

```powershell
npm run test:e2e:install
npm run test:e2e
```

Playwright construye y sirve un preview Vite local en `http://127.0.0.1:4173` (en Windows usa el canal Edge instalado y en otros sistemas el Chromium de Playwright); las trazas, capturas y vídeos de fallos se guardan en directorios ignorados por Git. Esta cobertura no sustituye todavía el E2E de la ventana Tauri ni los flujos que dependen de comandos Rust.

### Gates locales

```powershell
.\tools\check-governance.ps1
.\tools\check.ps1 -Profile Fast
.\tools\check.ps1 -Profile Full
.\tools\check.ps1 -Profile Release
.\tools\check.ps1 -Profile Package
npm run smoke:desktop -- -TimeoutSeconds 120
npm run smoke:cli
npm run smoke:installer
npm run smoke:cdp
npm run updater:key:check
npm run updater:verify-published -- --manifest-url <https-url> --output-dir <evidence-dir> --target windows-x86_64 --expected-version <version>
npm run perf:summary
npm run perf:benchmark
npm run perf:i1
npm run perf:i1:check
npm run accessibility:visual
npm run accessibility:check
npm run docs:check
npm run accessibility:release
npm run accessibility:release:check
npm run perf:check
npm run verify:experience
npm run verify:tier
```

| Perfil | Incluye |
| --- | --- |
| Fast | `cargo fmt --check`, `cargo check`, Vitest, build TypeScript/Vite y presupuesto frontend |
| Full | Fast + Clippy con warnings como errores + pruebas Rust de biblioteca |
| Release | Full + SBOM CycloneDX reproducible + build Tauri optimizado sin bundle |
| Package | Release + MSI/NSIS en Windows + inventario diferencial con tamaño y SHA-256 + smoke del instalador NSIS |

Después de generar evidencia, `npm run accessibility:check` valida el contrato
visual versionado y el SHA-256 de cada captura. `npm run perf:check` compara la
última evidencia CDP, el benchmark y el reporte Package contra los presupuestos
versionados. `npm run verify:experience` ejecuta ambos gates juntos; requiere
que esas evidencias ya existan y no abre la aplicación ni conserva datos.

La evidencia de producto de I8 usa un flujo separado para que el contrato visual
también se pruebe contra el ejecutable Tauri optimizado: `npm run
accessibility:release` compila sin bundle, captura desktop, móvil, escala 125% y
`forced-colors` mediante CDP de loopback y deja únicamente artefactos sanitizados
en `.local/validation/release-evidence/`. `npm run
accessibility:release:check` compara el resultado con
`fixtures/accessibility/release-evidence-baseline-v1.json`; una actualización
intencional exige inspección y el comando explícito
`npm run accessibility:release:update-baseline`. El baseline no sustituye la
auditoría manual con lector de pantalla ni la validación de hardware real.

Cada ejecución escribe un reporte JSON en `.local/validation/` con perfil, estado, tiempos, commit, rama, indicador de árbol sucio, sistema operativo, arquitectura y versiones de PowerShell, Node, npm, Rust y Cargo. También registra SHA-256 de `package-lock.json` y `src-tauri/Cargo.lock`, sin incluir rutas absolutas ni contenido; un lockfile ausente queda marcado como `unavailable`. El directorio es local y está ignorado por Git. Usa `-ReportPath <ruta>` para elegir otro destino; las rutas relativas se resuelven desde la raíz del proyecto. El reporte también se intenta escribir si falla una etapa, conservando el último resultado y su error. Todos los perfiles registran métricas raw/gzip del frontend; Release añade el SBOM y Package añade únicamente instaladores producidos o actualizados en esa ejecución.

`check-governance.ps1` comprueba la licencia MIT, el ADR de contratos, las
políticas de ramas/commits, el inventario local de dependencias y el manifest de
fixtures sintéticas. Estos contratos son independientes de la cobertura manual
de accesibilidad, los benchmarks grandes y la automatización Win32 pendientes
en fases posteriores.

`npm run smoke:installer` ejecuta el instalador NSIS real como usuario sin
privilegios en una ruta temporal con espacios y Unicode. Mide instalación y
primera apertura, comprueba que la segunda invocación respete la instancia única,
desinstala, verifica que los datos de usuario sobrevivan y elimina únicamente
su sentinel temporal. Este smoke no sustituye una VM Windows limpia ni el
ejercicio de actualización contra un canal publicado.

Para probar además un upgrade local entre dos versiones, se puede pasar un
artefacto NSIS anterior explícito: `powershell -File
tools/smoke-installed-artifact.ps1 -InstallerPath
src-tauri/target/release/bundle/nsis/Columnia_0.57.0_x64-setup.exe
-PreviousInstallerPath <artefacto-anterior>`. El escenario instala la versión
anterior, instala la objetivo en la misma ruta, comprueba la versión del
ejecutable y verifica que el sentinel de datos sobreviva al upgrade y a la
desinstalación. No se ejecuta automáticamente en `Package` porque requiere
proporcionar un artefacto histórico compatible.

El presupuesto actual admite por archivo hasta 512 KiB raw/160 KiB gzip para JavaScript y 128 KiB raw/40 KiB gzip para CSS; el total JS+CSS no puede superar 768 KiB raw/240 KiB gzip. El baseline verificado es aproximadamente 308 KiB raw y 88 KiB gzip (315,829/90,324 bytes).

`npm run perf:benchmark` usa tres iteraciones sostenidas de transformaciones
CSV/Parquet y después mide `project-save`, `project-inspect`, `project-export`,
`project-list` y `project-delete` sobre un almacén temporal. Después actualiza dos
veces el mismo ID, inspecciona la reapertura durable y vuelve a exportar. La receta
cambia `amount` a `total`; las reglas, el perfil cacheado, el historial y el cleanup
se verifican antes de eliminar el almacén. El gate también limita la duración de
transformaciones, guardado, inspección y exportación. El resumen nunca conserva el
ID del proyecto, filas, rutas ni contenido de los archivos.

Las pruebas frontend verifican además que `package-lock.json` refleje exactamente la versión y las dependencias raíz de `package.json`, y que el paquete local de `Cargo.lock` coincida con `Cargo.toml`. No requieren red ni reescriben lockfiles.

Los gates estáticos verifican que la CSP de producción permanezca local, que desarrollo solo añada el servidor loopback configurado y que la ventana `main` conserve exclusivamente `core:default`, sin permisos de filesystem, shell, HTTP u opener.

Los gates de supply chain rechazan paquetes npm sin SRI fuerte o fuera del registro oficial, crates sin checksum o fuera de crates.io, fuentes Git e identidades contradictorias. Release genera el SBOM sin red, timestamps, UUID, rutas locales ni URLs de descarga.

La validación del 2026-08-28 registra 248 pruebas frontend, 234 pruebas Rust y 9
E2E; las ramas específicas de symlinks/reparse points dependen de la plataforma.
Son una fotografía orientativa ligada a `137520b`, no un umbral permanente.

## Estado real frente a arquitectura objetivo

### Implementado ahora

- Shell Tauri, frontend React y motor Rust/Polars.
- Instancia única en escritorio: una segunda apertura muestra, desminimiza y enfoca la ventana `main` existente.
- Flujo Cargar → Revisar → Preparar → Entregar.
- Formatos, perfiles, transformaciones, historial de sesión, contratos y exportación descritos arriba.
- CSP restrictiva, capability mínima y validación local centralizada.
- Threat model vivo y gates de regresión para CSP, permisos, payloads semánticos y fórmulas CSV.
- Navegación por teclado inicial con skip link, pestañas ARIA, foco visible, regiones anunciables y diálogos con ciclo/restauración de foco.
- SBOM CycloneDX 1.6 reproducible y gates offline de integridad/procedencia para npm y Cargo.
- Cobertura V8 global sobre `src` con gate 80/75/75/80; la cobertura real por
  capa está abierta en `T5-04`. Supply
  chain local con npm audit, cargo-audit 0.22.2, cargo-deny 0.20.2, secret scan,
  avisos de terceros y política de red/telemetría.
- Instalador declarado `NSIS currentUser`, licencia MIT y avisos de terceros
  incluidos como recursos, además de política explícita de WebView2.
- Smoke automatizado del runtime de desarrollo con aislamiento y cleanup de procesos propios.
- Monitor nativo compacto de CPU/RAM integrado al lateral, con polling de 2 s,
  fallback explícito en el shell web y contratos de accesibilidad.
- Fase I1 cerrada: recetas compatibles con plan lazy/fallback eager, benchmark
  cruzado de 100 MiB contra `sistema anterior` y revisión visual release en cuatro
  escenarios.
- Presupuestos medibles del frontend y empaquetado Windows verificado en MSI/NSIS con evidencia criptográfica.
- CLI local con `inspect`, `transform` y `validate`, contratos JSON versionados y reutilización del motor, libros, reglas, recetas y exportación atómica del escritorio.
- Smoke CLI determinista que cubre CSV, Parquet, XLSX, dos modos de encabezado, calidad aprobada/reprobada, neutralización de fórmulas y fallos sin outputs parciales.
- Fases Cargar y Revisar extraídas de `App.tsx` a módulos con transiciones tipadas y pruebas propias.
- Fase Entregar extraída de `App.tsx` a un módulo con estados discriminados y pruebas propias.
- Fase Preparar extraída a vistas, editor, historial, modelo y controlador; `App.tsx` queda como coordinador de las cuatro fases.
- CLI batch v1 para 1–64 transformaciones, con preflight sin escrituras, colisiones rechazadas y atomicidad individual explícita.
- Proyectos locales con catálogo SQLite v10 compatible con v1/v2/v3/v4/v5/v6/v7/v8/v9, snapshots Parquet durables, reglas de calidad, borrador opcional, perfil cacheado, historial/cursor, actividad SQL agregada, vista y etapa de Revisar, página de muestra, motor SQL, cobertura de correlaciones y recuperación explícita aunque desaparezca la fuente original.
- CLI de proyectos con almacén `--store` explícito, guardado/listado/inspección/exportación/borrado, contratos JSON v1 privados, compuerta de calidad y confirmación destructiva exacta.
- Fase P1 activa: matriz de paridad con `sistema anterior`, exportación local atómica,
  comparación/joins, resolución visible por columna/valor, consulta restringida,
  paginación de conflictos, privacidad visible, visualizaciones accesibles y reglas de calidad versionadas;
  siguen pendientes el análisis exploratorio amplio, políticas sin equivalencia
  segura, conectores remotos, bundles auditables, escala fuera de memoria y
  privacidad de artefactos operativos.
 - M1 conserva en el informe de migración un resumen estructural de los manifiestos
   de sesión sistema anterior: hoja, etapa, conteos de operaciones/reglas/análisis y
   presencia de referencias de origen/snapshot sin copiar rutas. Proyectos puede
   restaurar un `snapshot_path` local y compatible como dataset temporal cuando
   falta la fuente original; el contrato explícito `history_snapshots` v1 también
   restaura hasta doce Parquet locales con cursor validado como historial durable
   del proyecto. Los historiales ambiguos o referencias ausentes siguen siendo
   explícitamente manuales. El bridge nativo ofrece un preflight local sanitizado
   antes de importar, sin crear proyectos ni modificar el dataset activo; la
   superficie visual de Proyectos permanece enfocada en el catálogo. La fixture
   representativa de round-trip cubre además fallback a fuente, hoja/etapa,
   operaciones deterministas, reglas y artefactos operativos redactados.
- La migración de reglas reconoce aliases snake/camel, severidades históricas,
  tolerancias y condiciones heredadas; referencias externas, políticas no
  equivalentes y contradicciones se conservan como omisiones explícitas.
- Integración frontend del ciclo guardar/abrir/eliminar: Vitest cubre el guardado, la confirmación/cancelación destructiva y la conservación del dataset activo; el smoke de escritorio valida el contrato de `ProjectsPanel` y el arranque de la ventana/WebView2.
- Accesibilidad WCAG 2.2 de bajo riesgo: targets interactivos mínimos de 24 px, reducción global de movimiento y prueba de regresión CSS para ambos contratos.
- Baseline local de rendimiento medido: Vite listo en 278–283 ms, Cargo debug en 0.86–0.91 s y startup total del smoke en 6.33–6.98 s, con mediana aproximada de 6.71 s; bundle v0.40.0 verificado en 314,827 bytes raw/90,154 gzip.
- Contratos automatizados de accesibilidad para landmarks, skip link, `aria-current`, `aria-busy`, acciones de proyectos y `alertdialog` modal.
- Smoke desktop instrumentado con hitos: la ejecución fría v0.30.0 registró Vite en 4,307 ms, proceso debug en 71,321 ms, ventana visible en 71,337 ms y cleanup confirmado en 1,133 ms/1 intento; el cleanup admite un segundo intento de 3 s tras uno inicial de 4 s.
- Playwright configurado con preview Vite y E2E del shell web para landmarks, runtime, navegación accesible, skip link y primer render; conserva trazas/capturas/vídeos solo cuando una prueba falla.
- Playwright con mock de `__TAURI_INTERNALS__` cubre cargar dataset, guardar, abrir y eliminar proyectos con confirmación, sin filesystem ni datos reales.
- `App` publica la marca `columnia:app-render`; Playwright verifica el primer render del shell por debajo de 3 segundos en el preview local.
- Playwright añade cobertura E2E de landmarks, foco visible, targets mínimos y ciclo de foco/restauración del `alertdialog`.
- En Windows, `npm run smoke:cdp` verifica un endpoint CDP de loopback de WebView2 y usa `chromium.connectOverCDP` para medir primer render, landmarks, skip link y foco principal, además del contrato de solo lectura de `ProjectsPanel`, sin mutar datos.
- Playwright añade cobertura E2E responsive de viewport móvil/desktop, `prefers-reduced-motion`, targets mínimos y ausencia de overflow horizontal.
- La última medición CDP nativa registró `columnia:app-render` en 42,097.2 ms durante un arranque debug frío; el dato queda como señal de rendimiento y no bloquea los contratos de landmarks/foco. El E2E web conserva el gate estricto de 3 s.
- Validación v0.37.0 completada en Windows: Vitest 129/129, Rust 127/127, Playwright 9/9, probe CDP combinado con IPC nativo de proyectos y cleanup confirmado, smoke desktop, CLI, `npm run perf:summary` y Package aprobados. Evidencias: CDP `.local/validation/webview2-cdp/20260823T013419Z`, desktop `.local/validation/desktop-smoke/20260823T012904Z`, CLI `.local/validation/cli-smoke/20260823T012943Z`, Package `.local/validation/20260823T013521Z-7e06675-package.json` y resumen `.local/validation/performance-summary/summary.json` (12 muestras CDP, 14 desktop, 60 filas CSV; shell web todavía no observado por este agregador).
- Benchmark local v0.38.0 completado con entrada sintética de 104,963,092 bytes (100.11 MiB), 876,544 filas y cuatro columnas: `inspect` 512.37 ms/247.1 MiB, `validate` 502.64 ms/248.2 MiB, `transform-csv` 2,329.89 ms/394.6 MiB y `transform-parquet` 2,502.10 ms/248.3 MiB; las salidas fueron 104,086,546 y 659,729 bytes respectivamente. Es una muestra CLI debug, no un presupuesto final de la ventana Tauri.
- Validación v0.38.0 completada en Windows: Vitest 129/129, Rust 127/127, Playwright 9/9, probe CDP combinado con IPC nativo de proyectos y cleanup confirmado, smoke desktop, CLI, `npm run perf:summary`, benchmark de 100 MiB y Package aprobados. Evidencias: benchmark `.local/validation/performance-benchmark/20260823T015011Z`, CDP `.local/validation/webview2-cdp/20260823T015220Z`, desktop `.local/validation/desktop-smoke/20260823T015325Z`, CLI `.local/validation/cli-smoke/20260823T015325Z`, Package `.local/validation/20260823T015338Z-9a8c52f-package.json` y resumen `.local/validation/performance-summary/summary.json` (13 muestras CDP, 15 desktop). El primer render nativo frío quedó en 39,995.6 ms, fuera del presupuesto de 3 s, pero no rompió los contratos de UI/IPC; el perfilado Tauri y el presupuesto global de RAM siguen pendientes.
- v0.39.0 añade al probe CDP un perfil de memoria del proceso debug (`sampleCount`, working set inicial/máximo/final y memoria privada máxima) y lo conserva en el resumen de rendimiento sin rutas ni datos de usuario. La señal sirve para comparar arranques, pero no constituye todavía un presupuesto global de RAM ni una medición de transformaciones dentro de la ventana.
- Validación v0.39.0 completada en Windows: Vitest 129/129, Rust 127/127, Playwright 9/9, CDP/ProjectsPanel, smoke desktop, CLI, benchmark, `npm run perf:summary` y Package aprobados. Evidencias: CDP `.local/validation/webview2-cdp/20260823T020433Z`, desktop `.local/validation/desktop-smoke/20260823T020544Z`, CLI `.local/validation/cli-smoke/20260823T020544Z`, benchmark `.local/validation/performance-benchmark/20260823T020544Z`, Package `.local/validation/20260823T020620Z-3fc7e17-package.json` y resumen `.local/validation/performance-summary/summary.json` (16 muestras CDP, 16 desktop). El perfil CDP registró 3 muestras, 7 procesos, working set máximo de 366,428,160 bytes (~349.4 MiB) y memoria privada máxima de 147,488,768 bytes; el primer render frío fue 43,412 ms y permanece como señal fuera del presupuesto de 3 s.
- v0.40.0 añade un recorrido nativo de proyectos al probe CDP: en el build debug crea un dataset sintético en memoria mediante un comando `#[cfg(debug_assertions)]`, guarda un proyecto temporal, lo lista, abre, pagina y elimina; valida que el catálogo vuelva a su conteo inicial y nunca conserva IDs, nombres, filas ni rutas en la evidencia.
- Validación v0.40.0 completada en Windows: Vitest 129/129, Rust 127/127, Playwright 9/9, CDP con mutaciones IPC nativas y cleanup confirmado, smoke desktop, CLI, benchmark, `npm run perf:summary` y Package aprobados. Evidencias: CDP `.local/validation/webview2-cdp/20260823T022759Z`, desktop `.local/validation/desktop-smoke/20260823T022332Z`, CLI `.local/validation/cli-smoke/20260823T022332Z`, benchmark `.local/validation/performance-benchmark/20260823T022332Z`, Package `.local/validation/20260823T022448Z-3aa09f2-package.json` y resumen `.local/validation/performance-summary/summary.json` (18 muestras CDP, 17 desktop). El ciclo nativo dejó el catálogo en 0→0, sin campos de ruta; el perfil registró 3 muestras, 7 procesos, working set máximo de 365,252,608 bytes (~348.4 MiB) y memoria privada máxima de 145,301,504 bytes; el primer render frío continúa fuera del presupuesto de 3 s.
- v0.41.0 extiende el ciclo nativo bajo debug con `probe_save_transform_recipe`, `apply_transform_recipe` y `probe_export_dataset`: persiste una receta JSON en un directorio temporal, la reaplica sobre el dataset sintético y exporta CSV atómicamente con una regla de calidad aprobada. El proyecto guarda y restaura también el borrador de receta y la regla; la evidencia conserva solo estados, conteos y nombres de comandos, nunca rutas ni datos.
- Validación v0.41.0 completada en Windows: Vitest 129/129, Rust 127/127, Playwright 9/9, CDP WebView2 con receta/exportación/workspace y cleanup 0→0, smoke desktop, CLI, benchmark, `npm run perf:summary` y Package aprobados. Evidencias: CDP `.local/validation/webview2-cdp/20260823T023521Z`, desktop `.local/validation/desktop-smoke/20260823T023722Z`, CLI `.local/validation/cli-smoke/20260823T023722Z`, benchmark `.local/validation/performance-benchmark/20260823T023722Z`, Package `.local/validation/20260823T023806Z-2b173b8-package.json` y resumen `.local/validation/performance-summary/summary.json` (19 muestras CDP, 18 desktop). El ciclo nativo dejó el catálogo en 0→0; el perfil registró 1 muestra, 7 procesos, working set máximo de 333,901,824 bytes (~318.5 MiB) y memoria privada máxima de 140,701,696 bytes; el primer render frío continúa fuera del presupuesto de 3 s.
- v0.42.0 añade `probe_reopen_project` bajo debug: crea un `ProjectStore` fresco contra el almacén de aplicación, reabre SQLite y el snapshot durable, valida recovery, dimensiones, reglas y borrador antes de que el ciclo IPC vuelva a abrir y elimine el proyecto. La comprobación no registra IDs, nombres, rutas ni filas en la evidencia.
- Validación v0.42.0 completada en Windows: Vitest 129/129, Rust 127/127, Playwright 9/9, CDP WebView2 con receta, exportación, reapertura durable, workspace y cleanup 0→0, smoke desktop, CLI, benchmark, `npm run perf:summary` y Package aprobados. Evidencias: CDP `.local/validation/webview2-cdp/20260823T024449Z`, desktop `.local/validation/desktop-smoke/20260823T024650Z`, CLI `.local/validation/cli-smoke/20260823T024650Z`, benchmark `.local/validation/performance-benchmark/20260823T024650Z`, Package `.local/validation/20260823T024737Z-96e6792-package.json` y resumen `.local/validation/performance-summary/summary.json` (20 muestras CDP, 19 desktop). El probe marcó `persistenceReopenVerified=true`, sin campos de ruta, con 7 procesos y working set máximo de 340,135,936 bytes (~324.4 MiB); el primer render frío continúa fuera del presupuesto de 3 s.
- v0.43.0 añade `npm run smoke:restart`: ejecuta dos procesos Tauri/WebView2 independientes. La primera fase persiste un proyecto sintético y termina; la segunda obtiene el recovery candidate, valida SQLite/snapshot/workspace, abre, pagina y elimina solo el proyecto del probe. Cada fase conserva evidencia privada y el wrapper solo publica estados, conteos y directorios de evidencia.
- Validación inicial v0.43.0 del reinicio real: `restart-prepare` dejó el catálogo 0→1 y `restart-verify` 1→0, ambas fases con cleanup confirmado. Evidencia `.local/validation/webview2-restart/20260823T030656Z`; CDP prepare `.local/validation/webview2-cdp/20260823T030656Z`, verify `.local/validation/webview2-cdp/20260823T030750Z`.
- Validación v0.43.0 completada en Windows: Vitest 129/129, Rust 127/127, Playwright 9/9, CDP normal y reinicio real, smoke desktop, CLI, benchmark, `npm run perf:summary` y Package aprobados. Evidencias: CDP normal `.local/validation/webview2-cdp/20260823T030941Z`, restart `.local/validation/webview2-restart/20260823T030656Z`, desktop `.local/validation/desktop-smoke/20260823T031037Z`, CLI `.local/validation/cli-smoke/20260823T030919Z`, benchmark `.local/validation/performance-benchmark/20260823T030919Z`, Package `.local/validation/20260823T031047Z-a66fe3d-package.json` y resumen `.local/validation/performance-summary/summary.json` (29 muestras CDP, 20 desktop). El reinicio confirmó prepare 0→1 y verify 1→0 con cleanup; la compilación fría continúa como señal observacional fuera del presupuesto de 3 s.
- v0.44.0 añade telemetría sanitizada por operación al ciclo nativo de `ProjectsPanel`: el runner conserva conteo, duración total y muestras por comando IPC para receta, exportación, persistencia, reapertura y cleanup, sin IDs, rutas, filas ni nombres de usuario.
- v0.44.0 convierte el perfil CDP en un gate de presupuesto de ventana: toma muestras después de los probes y antes del cleanup, publica `performanceBudget` con límites configurables (512 MiB working set y 256 MiB memoria privada por defecto) y falla una ejecución soportada si excede alguno. `npm run perf:summary` conserva ese bloque junto al perfil y las duraciones nativas.
- Validación v0.44.0 completada en Windows: Vitest 129/129, Playwright 9/9, smoke CDP normal y reinicio real, smoke desktop, CLI, benchmark, `npm run perf:summary` y Package aprobados. Evidencias: CDP normal `.local/validation/webview2-cdp/20260823T032048Z`, reinicio final `.local/validation/webview2-restart/20260823T032932Z` (prepare `.local/validation/webview2-cdp/20260823T032932Z`, verify `.local/validation/webview2-cdp/20260823T033028Z`), desktop/CLI/benchmark `.local/validation/*/20260823T032441Z`, Package `.local/validation/20260823T032502Z-d8ddd90-package.json` y resumen `.local/validation/performance-summary/summary.json` (34 muestras CDP, 21 desktop). El CDP normal observó 448,970,752 bytes de working set y 251,916,288 bytes privados, ambos dentro de sus límites; el reinicio final observó 429.89/235.36 MiB en prepare y 428.11/236.12 MiB en verify, con 9 operaciones IPC por fase y cleanup confirmado. El primer render frío continúa como señal observacional fuera de 3 s.
- v0.45.0 añade soporte visual para `forced-colors: active`, conserva foco/controles con colores del sistema y agrega `npm run accessibility:visual`, que captura cuatro escenarios reproducibles (desktop, móvil, escala de dispositivo 125% y alto contraste) con un resumen sin datos.
- La evidencia v0.45.0 verificó en los cuatro escenarios los landmarks `main`/`nav`/`aside`, foco visible, targets mínimos de 24 px y ausencia de overflow horizontal; el lector de pantalla manual sigue siendo una validación externa pendiente.
- Validación v0.45.0 completada en Windows: Vitest 130/130, Rust 127/127, Playwright 9/9, evidencia visual 4/4, CDP normal, reinicio real, desktop, CLI, benchmark, `npm run perf:summary` y Package aprobados. Evidencias: visual `.local/validation/accessibility-visual/20260823T035524Z`, CDP final `.local/validation/webview2-cdp/20260823T035854Z`, reinicio final `.local/validation/webview2-restart/20260823T035959Z` (prepare `.local/validation/webview2-cdp/20260823T035959Z`, verify `.local/validation/webview2-cdp/20260823T040108Z`), desktop/CLI/benchmark `.local/validation/*/20260823T034915Z`, Package `.local/validation/20260823T035550Z-08e35ed-package.json` y resumen `.local/validation/performance-summary/summary.json` (40 muestras CDP, 22 desktop). El CDP final observó 428.30/235.87 MiB y el reinicio 430.60/237.35 MiB en prepare, 419.52/233.23 MiB en verify, dentro del presupuesto y con cleanup.
- v0.46.0 añade un baseline visual estructural versionado: cada captura conserva SHA-256 y `accessibility:check` valida los cuatro escenarios contra un fixture sin versionar imágenes ni datos.
- v0.46.0 añade un gate de rendimiento que cruza CDP, benchmark de 100 MiB y bundle Package contra presupuestos versionados (512 MiB working set, 256 MiB memoria privada, 768/240 KiB raw/gzip) y publica evidencia local sanitizada. `verify:experience` ejecuta ambos gates.
- Validación v0.46.0 completada en Windows: Vitest 130/130, Rust 127/127, Playwright 9/9, captura visual 4/4, baseline visual, benchmark de 100 MiB, CDP normal, reinicio real, smoke desktop/CLI, `perf:summary`, baseline de rendimiento y Package aprobados. Evidencias: visual `.local/validation/accessibility-visual/20260823T041648Z`, baseline visual `.local/validation/accessibility-baseline/20260823T042831Z`, benchmark `.local/validation/performance-benchmark/20260823T041752Z`, CDP `.local/validation/webview2-cdp/20260823T042258Z`, reinicio `.local/validation/webview2-restart/20260823T042438Z`, desktop `.local/validation/desktop-smoke/20260823T042811Z`, CLI `.local/validation/cli-smoke/20260823T042810Z`, Package `.local/validation/20260823T041915Z-72c81a2-package.json`, baseline de rendimiento `.local/validation/performance-baseline/20260823T042916Z` y resumen `.local/validation/performance-summary/summary.json` (43 muestras CDP, 23 desktop). El CDP observó 449,486,848 bytes de working set y 246,984,704 bytes privados, dentro del presupuesto; el benchmark alcanzó 104,963,092 bytes con cleanup confirmado.
- v0.47.0 amplía el benchmark CLI a tres iteraciones sostenidas de CSV/Parquet y a un ciclo durable de proyecto con receta, reglas, perfil cacheado, inspección, exportación, catálogo y borrado; el gate conserva el presupuesto de 512 MiB y exige cleanup.
- Validación v0.47.0 completada en Windows: Vitest 130/130, Rust 127/127, Playwright 9/9, captura/baseline visual 4/4, benchmark sostenido y ciclo de proyecto, CDP normal, reinicio real, smoke desktop/CLI, `perf:summary`, baseline de rendimiento y Package aprobados. Evidencias: visual `.local/validation/accessibility-visual/20260823T045124Z`, baseline visual `.local/validation/accessibility-baseline/20260823T045957Z`, benchmark `.local/validation/performance-benchmark/20260823T044929Z`, CDP `.local/validation/webview2-cdp/20260823T045557Z`, reinicio `.local/validation/webview2-restart/20260823T045709Z`, desktop/CLI `.local/validation/*/20260823T045920Z`, Package `.local/validation/20260823T045200Z-48206a7-package.json`, baseline de rendimiento `.local/validation/performance-baseline/20260823T045957Z` y resumen `.local/validation/performance-summary/summary.json` (46 muestras CDP, 24 desktop). El gate observó 451,100,672 bytes de working set y 242,634,752 bytes privados; el benchmark ejecutó tres iteraciones, alcanzó 104,963,092 bytes, tuvo pico CLI de 476,659,712 bytes y confirmó cleanup.
- v0.48.0 endurece el benchmark con presupuestos de duración por operación, actualiza dos veces el mismo proyecto y verifica una reapertura/exportación posterior; añade `verify:tier` para ejecutar el conjunto reproducible completo y una checklist manual de asistencia.
- v0.49.0 amplía el probe Tauri/WebView2 a transformaciones y exportaciones nativas sostenidas; `perf:check` exige tres iteraciones nativas y aplica sus duraciones junto al presupuesto global de memoria del árbol WebView2.
- 2026-08-29 añade `perf:webview2`: un CSV temporal cercano a 100 MiB atraviesa el selector Win32, `load_dataset_selection`, paginación, transformación y exportación dentro de WebView2. La evidencia conserva dimensiones, duraciones, picos agregados y cleanup; el benchmark específico pasa con 105,446,485 bytes, 819,137 filas, 686,817,280 bytes de working set y 456,114,176 bytes privados. El gate global de 512/256 MiB sigue pendiente para el caso grande.
- 2026-08-29 M1 conserva en `migrationReport.session` los nombres estructurales acotados de operaciones aplicadas y comprobaciones de análisis cuando son tokens seguros, además de sus conteos; también conserva metadatos agregados de muestreo cuando están disponibles, sin copiar filas ni valores. El proyecto importado los mantiene al persistir y reabrir su receta. No se guardan resultados, cachés ni rutas resueltas, por lo que la restauración completa de sesiones sistema anterior sigue pendiente.
  - 2026-08-29 la importación de sesiones prioriza un `snapshot_path` local y compatible cuando existe, porque conserva el estado materializado exacto y no requiere inventar parámetros para `applied_ops`; sin snapshot, la fuente se valida, las operaciones deterministas `drop_duplicates`, `drop_fuzzy_duplicates`, `drop_high_null_cols`, `drop_id_cols`, `drop_empty_cols`, `drop_constant_cols`, `drop_empty_rows`, `normalize_sentinels`, `impute_numeric`, `impute_categorical`, `parse_dates`, `trim_text`, `normalize_text`, `fix_encoding`, `cast_numeric`, `cap_outliers`, `impute_outliers`, `drop_outliers`, `normalize_booleans`, `mask_pii`, `normalize_columns` y `add_cambios_col` se reproducen en orden fijo y la receta estructural se reaplica. `parse_dates` solo convierte texto con formatos cerrados, cobertura segura, años entre 1900 y 2100 y como máximo 1% de literales ilegibles; las columnas ambiguas se dejan intactas. `mask_pii` usa solo la máscara predeterminada y conservadora `[REDACTED]`; `drop_fuzzy_duplicates` usa el fingerprint normalizado local hasta 5.000 filas y conserva copias exactas; los modos hash/clave explícita no se inventan y datasets mayores conservan el guard de rendimiento de sistema anterior. Las eliminaciones de columnas conservan al menos una columna utilizable, `drop_empty_rows` solo retira filas completamente nulas, `impute_numeric` usa la mediana sobre columnas físicas `Int64`/`Float64` después de normalizar centinelas y promueve a `Float64` cuando es fraccionaria, `trim_text` recorta espacios exteriores en texto y protege `_cambios`, `normalize_text` colapsa espacios, pasa a minúsculas y retira acentos, `cast_numeric` exige más de 90% de valores numéricos y rechaza conversiones con pérdida de precisión, las estrategias IQR cap/impute/drop requieren al menos cuatro valores numéricos válidos, `drop_outliers` elimina una fila si cualquier columna numérica excede sus límites, `normalize_booleans` exige un vocabulario cerrado con ambos valores y `normalize_columns` resuelve nombres con la misma regla Unicode, no texto en blanco. `add_cambios_col` recupera solo la estructura reservada; el historial, las cachés y los resultados de análisis continúan fuera del contrato.
 - 2026-08-29 `selected_cleaning_operations` se normaliza mediante aliases canónicos: las limpiezas deterministas, incluido `normalize_text` con espacios colapsados, minúsculas y acentos retirados, se reproducen desde la fuente y quedan registradas en `migrationReport.session`; `mask_pii` usa en fallback solo la máscara predeterminada y conservadora `[REDACTED]`, `drop_fuzzy_duplicates` usa el fingerprint normalizado local hasta 5.000 filas, y los modos hash/clave explícita y operaciones sin equivalente reversible conservan una advertencia específica, sin copiar contenido ni ejecutarse silenciosamente.
- 2026-08-29 el preflight de sesiones clasifica bloques reconocibles de resultados, historial y cachés como `analysis_results`, `history` y `caches` no portables; conserva únicamente las categorías sanitizadas para orientar la revisión manual, sin copiar contenido ni rutas.
- 2026-08-29 el probe CDP limita el perfil y el cleanup a procesos realmente
  pertenecientes al `Job Object`; una réplica anterior había contado un proceso
  externo descendiente (`DriverBooster`) y elevó artificialmente el pico a
  332,472,320 bytes. La corrida posterior pasó con 503,644,160 bytes de working
  set, 267,456,512 bytes privados y cleanup confirmado; la variación restante
  del runtime WebView2 queda como señal a vigilar, no como permiso para relajar
  el presupuesto (`.local/validation/webview2-cdp/20260829T044540Z`).
 - 2026-08-29 P1 incorpora `fix_encoding`: el perfil cuenta por columna secuencias comunes de doble codificación UTF-8 y Preparar ofrece una reparación reversible solo cuando la conversión es inequívoca; los tipos no textuales, `_cambios` y valores no decodificables quedan intactos.
 - 2026-08-29 M1 recalcula y persiste el perfil agregado durante la importación de sesiones sistema anterior, de modo que el proyecto abre con caché de calidad verificable; los resultados de análisis originales y cachés reanudables no se inventan.
 - 2026-08-30 P1 incorpora un motor DuckDB opcional para la consulta SQL local restringida: el bridge selecciona el motor, Rust conserva el contrato de solo lectura, ejecuta sobre snapshots Parquet temporales, interrumpe la consulta nativa al cancelar y mantiene conteo, paginación y orden estable sin publicar columnas auxiliares. Cuando existe un snapshot administrado, la preparación de JOIN lee solo el esquema de la comparación y no vuelve a materializar sus filas; el `DataFrame` activo y la ejecución incremental general quedan pendientes.
 - 2026-08-30 P1 pagina los conflictos de Revisar directamente desde el snapshot Parquet comparado: recorre bloques de 16K, mantiene duplicados globales con un índice temporal, conserva solo una página y un bloque de valores, y rechaza conteos obsoletos o cambios durante la lectura; la ejecución fuera de RAM general y los formatos Excel sin lector secuencial siguen pendientes.
 - 2026-08-30 M1 restaura el historial portable `history_snapshots` v1 cuando la sesión aporta hasta doce referencias locales Parquet, etiquetas y cursor: Rust valida cada archivo regular, comprueba que el cursor coincide con el estado actual, copia las revisiones a la generación administrada y conserva Deshacer/Rehacer tras reiniciar. Los historiales ambiguos, referencias ausentes y cachés/resultados de análisis continúan requiriendo revisión manual.
 - 2026-08-30 M1 añade al bridge nativo la operación `migration` con progreso por etapas para validar, cargar, reproducir, restaurar historial, recalcular el perfil y publicar la sesión sistema anterior. La cancelación usa una generación aislada y se comprueba antes de cada fase; si ocurre antes de la publicación atómica, el catálogo queda sin proyecto parcial y el dataset activo no se modifica. La CLI conserva el wrapper síncrono sin exponer rutas.
 - 2026-08-30 M1 reconoce el campo real `selected` de los pipelines sistema anterior persistidos, además de `selected_cleaning_operations` y `selectedCleaningOperations`, y conserva sus operaciones deterministas al importar la receta; una prueba Rust evita que la selección se pierda por usar el contrato de pipeline en vez del de sesión.
 - 2026-08-30 M1 completa el mapeo de pipelines al catálogo: `project-save --recipe` exige un `--input` explícito porque el pipeline no transporta su fuente, reproduce las limpiezas seleccionadas antes de la receta estructural y valida el resultado antes del snapshot; el proyecto reabierto conserva el informe de migración.
 - 2026-08-30 P1/M1 cierra el catálogo de limpieza sugerida: las 22 operaciones registradas por `sistema anterior` comparten una lista canónica con el replay de sesiones; una prueba Rust evita que futuras operaciones queden sin mapping. Operaciones desconocidas y modos PII no equivalentes siguen requiriendo revisión manual.
- 2026-08-30 M1 alinea la actividad de sesiones con el historial real de sistema anterior: normaliza `completed`/`failed`, redondea duraciones decimales y usa `rows_out`/`rowsOut` como filas agregadas; las entradas desconocidas o con datos no portables se descartan.
 - 2026-08-30 M1 alinea el fallback de `normalize_text` con sistema anterior: omite columnas de texto con más de 50% de valores distintos, conserva nulos y aplica título a nombres propios sugeridos por el encabezado; la acción manual de normalización de Columnia conserva su contrato explícito por columnas.
 - 2026-08-30 M1 conserva el perfil inicial para `drop_high_null_cols` y `drop_id_cols` durante el replay sistema anterior: una deduplicación previa no puede convertir artificialmente una columna en candidata por cambiar su porcentaje de nulos o cardinalidad.
 - 2026-08-30 M1 reconoce `filename`, el campo de nombre que emite sistema anterior, como fallback de referencia local cuando falta `source_path`; solo lo usa junto al manifiesto, valida el archivo antes de publicar y nunca transporta la ruta por IPC.
 - 2026-08-30 M1 añade una fixture v3 con la forma real de `SessionRecipe`: la importación conserva etapa, reglas y metadatos agregados de muestra, normaliza la actividad `ExecutionHistory` y la restaura al reabrir el proyecto; resultados, cachés, consultas y rutas siguen siendo señales no portables.
 - 2026-08-30 M1 añade la fixture `sistema anterior-session-v1-history-roundtrip.json`: importa una sesión con fuente, snapshot actual, dos revisiones Parquet, cursor, etapa, actividad y artefactos no portables; reabre el proyecto y verifica Deshacer/Rehacer sin copiar resultados ni cachés.
 - 2026-08-30 Configuración de desarrollo fija los perfiles Cargo `dev`/`test` sin símbolos de depuración: `npm run tauri dev` enlaza el binario Windows sin `LNK1140`, mientras `release` mantiene su política separada.
 - 2026-08-29 P1 añade una acción confirmada para apartar como nulos los valores de texto que no coinciden con una sugerencia semántica con al menos 90% de confianza; no muestra celdas, conserva vacíos y tipos no textuales, y puede revertirse desde el historial.
  - 2026-08-29 P1 persiste en el workspace de cada proyecto las últimas cinco ejecuciones SQL como estado, duración y filas; las restaura al abrir y rechaza entradas corruptas o sobredimensionadas, sin guardar consultas, rutas ni valores.
  - 2026-08-29 P1 conserva también la vista y etapa activa del flujo, además de la página visible de la muestra de Revisar, en el workspace durable; `diagnosis`, Revisar y la primera página son fallbacks seguros para catálogos anteriores, valores no soportados u offsets fuera de rango, y la migración SQLite avanza a v8.
  - 2026-08-29 I3 confirma la cobertura crítica por capa: `npm run test:coverage` pasa los cinco archivos orquestadores con los umbrales declarados, incluyendo las ramas de confirmación y desmarcado de Preparar.
 - 2026-08-29 Entregar permite abrir la carpeta del último output local: Rust retiene temporalmente el destino de una exportación exitosa, lo revalida como archivo regular y ejecuta el explorador nativo; la ruta nunca cruza el IPC hacia React.
 - 2026-08-29 una nueva corrida normal de `smoke:cdp` aprobó Playwright, ProjectsPanel, mutaciones IPC y cleanup, con 501,563,392 bytes de working set dentro de 512 MiB, pero 272,379,904 bytes privados sobre 256 MiB; junto a diagnósticos previos de 256.06–258.06 MiB, la señal apunta a variabilidad del árbol WebView2 y mantiene abierto el gate privado estable.
 - Validación v0.48.0 completada en Windows con `npm run verify:tier`: Vitest 130/130, Rust 127/127, Playwright 9/9, build, evidencia/baseline visual 4/4, benchmark sostenido, Package, smoke CLI, smoke desktop, CDP, reinicio y gates finales aprobados. Evidencias: visual `.local/validation/accessibility-visual/20260823T052032Z`, baseline visual `.local/validation/accessibility-baseline/20260823T053241Z`, benchmark `.local/validation/performance-benchmark/20260823T053019Z`, CDP `.local/validation/webview2-cdp/20260823T052546Z`, reinicio `.local/validation/webview2-restart/20260823T052647Z`, desktop `.local/validation/desktop-smoke/20260823T052530Z`, CLI `.local/validation/cli-smoke/20260823T052526Z`, Package `.local/validation/20260823T052258Z-3b7950b-package.json`, baseline de rendimiento `.local/validation/performance-baseline/20260823T053242Z` y resumen `.local/validation/performance-summary/summary.json` (49 muestras CDP, 25 desktop). El gate CDP observó 438,829,056 bytes de working set y 242,012,160 bytes privados; el benchmark alcanzó 104,963,092 bytes, ejecutó tres iteraciones y dos actualizaciones, tuvo pico CLI de 467,546,112 bytes, duraciones máximas de 2,578.97/26,264.97/11,458.07/13,141.88 ms (transform/guardado/inspección/exportación) y confirmó cleanup.
- Validación v0.49.0 completada en Windows con `npm run verify:tier` (9.09 minutos): Vitest 130/130, Rust 127/127, Playwright 9/9, build, evidencia/baseline visual 4/4, benchmark sostenido, Package, smoke CLI, smoke desktop, CDP sostenido, reinicio y gates finales aprobados. Evidencias: visual `.local/validation/accessibility-visual/20260823T055019Z`, baseline visual `.local/validation/accessibility-baseline/20260823T055904Z`, benchmark `.local/validation/performance-benchmark/20260823T055027Z`, CDP final `.local/validation/webview2-cdp/20260823T055801Z`, reinicio `.local/validation/webview2-restart/20260823T055558Z`, desktop `.local/validation/desktop-smoke/20260823T055552Z`, CLI `.local/validation/cli-smoke/20260823T055548Z`, Package `.local/validation/20260823T055253Z-d865c7a-package.json`, baseline de rendimiento `.local/validation/performance-baseline/20260823T055904Z` y resumen `.local/validation/performance-summary/summary.json` (59 muestras CDP, 26 desktop). El gate CDP observó 453,664,768 bytes de working set y 249,978,880 bytes privados; la señal nativa ejecutó tres ciclos con máximos de 9.8 ms de transformación y 4.7 ms de exportación. El benchmark alcanzó 104,963,092 bytes, ejecutó tres iteraciones y dos actualizaciones, tuvo pico CLI de 492,957,696 bytes, duraciones máximas de 6,243.26/26,306.45/11,436.06/13,285.38 ms (transform/guardado/inspección/exportación) y confirmó cleanup.
 - 2026-08-30 P1 reduce el pico de los `JOIN` SQL locales `INNER`/`LEFT` sin agregación: el lado `dataset` se divide en bloques, cada bloque se une y consulta de forma cancelable, y solo se conserva la página global, el conteo y un bloque temporal. La ruta `FULL` y las agregaciones también recorren bloques dentro de sus límites explícitos; DuckDB y la ejecución incremental general permanecen pendientes.
 - 2026-08-30 P1 extiende esa ruta por bloques a las agregaciones `INNER`/`LEFT` y `FULL`: `COUNT`, `SUM`, `AVG`, `MIN` y `MAX` fusionan sus estados y grupos en el orden de primera aparición, sin conservar el `DataFrame` unido completo. DuckDB y la ejecución incremental general permanecen pendientes.
 - 2026-08-30 P1 elimina la materialización completa del anti-join derecho en `FULL JOIN` local: un índice temporal de claves del activo y bloques de 16K recorren solo las filas no emparejadas, preservando `NULL` como no igual, duplicados y orden de entrada. Los `DataFrame` fuente y la ejecución general fuera de RAM permanecen pendientes.

- Fase I8 completada: la documentación está separada en tutorial, how-to, referencia y explicación; `CHANGELOG.md` y el índice de ADRs tienen entradas verificables; `docs:check` valida 14 Markdown, enlaces locales, UTF-8 sin BOM, versiones y ownership de imágenes. `accessibility:release` construyó el binario optimizado y capturó cuatro escenarios desde Tauri/WebView2 (`.local/validation/release-evidence/20260823T185353Z`); `accessibility:release:check` aprobó el baseline `.local/validation/release-evidence-check/20260823T185503Z` con fixture sintética de 44 bytes, controles legibles en `forced-colors` y contratos de landmarks, foco, targets y overflow. Las imágenes permanecen fuera de Git y sus hashes/owner/propósito viven en `fixtures/accessibility/release-evidence-baseline-v1.json`; la auditoría manual de lector de pantalla sigue siendo I3.

- Cierre I1 verificado el 2026-08-23: `cargo test --lib` pasa 128 pruebas, el benchmark cruzado `.local/validation/i1-benchmark/20260823T193942Z` procesó 104,963,092 bytes y 876,544 filas, la inspección lazy de Columnia tardó 529.68 ms frente a 3,689.39 ms de `sistema anterior`, y la evidencia release `.local/validation/release-evidence/20260823T195225Z` aprobó desktop, móvil, zoom 125% y `forced-colors`. La comparación de memoria es direccional porque cada herramienta mide su propio proceso.

### Planeado o pendiente

- ampliar la ejecución lazy/incremental a datasets mayores que la memoria y a
  operaciones que todavía requieren el camino eager;
- DuckDB embebido para ampliar esta primera ruta a datasets que excedan la RAM,
  consultas SQL más amplias y un presupuesto incremental integral;
- conectores remotos y destinos de bases de datos adicionales; los joins,
  comparación de datasets y destinos locales Excel/SQLite ya están cubiertos;
- auditoría manual con lector de pantalla y validación en hardware de Windows High Contrast; `npm run accessibility:visual` ya cubre capturas reproducibles de desktop, móvil, escala 125% y `forced-colors` sin reemplazar una sesión manual de asistencia;
- ampliar la comparación contra `sistema anterior` a datasets grandes y a RAM
  integral de dataset+historial; I1 ya cubre la inspección cruzada reproducible
  de 100 MiB y el benchmark CLI `.local/validation/performance-benchmark/20260823T223949Z`
  validó 256 MiB/2,220,032 filas con tres ciclos y dos actualizaciones, pero
  observó hasta 1.12 GiB de working set;
- el smoke `npm run smoke:native-selectors` ya es un gate de `verify:tier` cuando no se usa `-SkipNative`: recorre abrir dataset, guardar/cargar receta y exportar con fixtures sintéticos, variantes de editor Abrir/Guardar como, cleanup y evidencia sanitizada; el driver filtra el diálogo por proceso/owner y tiene timeout propio; la auditoría manual de lector de pantalla/High Contrast sigue pendiente;
- presupuesto integral global de dataset+historial para entradas grandes y comparación contra `sistema anterior`; v0.49 mide tres ciclos nativos sobre el dataset de probe y aplica el presupuesto global del árbol, pero todavía no prueba datasets grandes desde WebView2;
- escaneo de vulnerabilidades y firma Authenticode de Windows siguen fuera de
  alcance; updater autenticado, SBOM, gates offline y empaquetado Windows básico
  ya existen;
- verificación real en macOS y Linux.
- restauración segura de sesiones sistema anterior y mapeo al catálogo de proyectos;
  el informe estructural ya está disponible, pero no activa snapshots ni escribe
  artefactos durables.

Consulta `ROADMAP.md` para el detalle, pero verifica cada casilla contra el código antes de afirmar que una fase está completa.

## Riesgos y deuda técnica visibles

1. **Motor monolítico**: `dataset.rs` concentra casi todo el dominio. Un cambio puede afectar carga, receta, historial y exportación; usa CodeGraph y ejecuta pruebas Rust completas.
2. **Editor de recetas amplio**: las cuatro fases ya viven en módulos feature y `App.tsx` es un coordinador pequeño, pero `TransformRecipeEditor.tsx` reúne muchos subdominios de receta. Cualquier división futura debe preservar el orden, dependencias y confirmaciones destructivas.
3. **Contratos duplicados con gate**: Rust y TypeScript todavía declaran contratos por separado, pero 58 estructuras tienen comparación automática de campos y tipos. Al añadir una estructura compartida nueva, debe incorporarse explícitamente a las listas del gate IPC.
4. **Memoria**: los datasets no tienen un tope fijo de tamaño. Polars materializa el dataset y algunas operaciones crean candidatos completos, por lo que la capacidad efectiva depende de la RAM, el espacio disponible y los demás recursos del equipo.
5. **Consumo de disco durable**: cada proyecto puede conservar generaciones e historial Parquet de hasta 12 revisiones/1 GiB; los límites por proyecto no forman un presupuesto global para todos los proyectos.
6. **Cobertura de plataforma**: arranque y empaquetado están verificados en Windows; macOS y Linux aún requieren validación local real.
7. **Roadmap acumulativo**: contiene decisiones propuestas, aprobadas e implementadas; no todas reflejan dependencias presentes.
8. **Sin CI por política**: la calidad depende de ejecutar y registrar correctamente los gates locales.
9. **Gates de publicación condicionados**: cobertura por capa, primer render,
   `SkipPackage`, zoom 125% y evidencia release ya tienen contratos ejecutables;
   la evidencia release aún exige un commit limpio y la revisión legal final.
10. **Rendimiento con alcance acotado**: el benchmark durable de 100 MiB y tres
    ciclos WebView2 cumplen sus presupuestos actuales, pero la capacidad global
    para datasets mayores y la interacción nativa requieren validación adicional.
11. **Persistencia recuperable en evolución**: los reintentos de inicialización y
    la reconciliación de generaciones ya están implementados; deben conservarse
    las pruebas de fallo y el margen de seguridad al ampliar el esquema.
12. **Distribución no promovible aún**: Package, notices, updater firmado y el
    orquestador local existen; instalación en VM, canal, rotación de clave y
    decisiones legales siguen abiertos.

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
| 2026-09-07 | Reauditoría incremental posterior al rediseño y la modularización: producto, suites y recorrido nativo aprobados; Tier 7 registra presupuesto CSS excedido, inventario IPC desincronizado, falso positivo del gate de red y evidencia documental anterior. Usuario, escala, canal, jurisdicción y normativa quedan pendientes. | `AUDITORIA_PROFESIONAL_2026-09-07.md`, `ROADMAP.md`, `CHANGELOG.md`, `.local/validation/accessibility-visual/20260907T223346Z`, `.local/validation/webview2-cdp/20260907T223458Z` |
| 2026-09-07 | T7-03: el gate de red distingue patrones por lenguaje y conserva cobertura de cursor ODBC permitido, `fetch` web bloqueado y cliente HTTP Rust bloqueado; red y supply chain pasan. | `tools/check-network-policy.mjs`, `tools/check-network-policy.test.mjs`, `package.json`, `ROADMAP.md`, `CHANGELOG.md` |
| 2026-09-07 | T7-02: inventario IPC regenerado con módulos reales; checker y prueba de paridad comparten `sourceFiles` y validan su existencia. | `docs/reference/ipc-inventory.json`, `tools/check-ipc-inventory.mjs`, `src/ipc-contract.test.ts`, `ROADMAP.md`, `CHANGELOG.md` |
| 2026-09-07 | T7-01: declaraciones responsive redundantes retiradas; CSS raw/gzip bajo presupuesto y build, E2E y matriz visual aprobados. | `src/styles.css`, `.local/validation/audit-20260907-bundle.json`, `.local/validation/accessibility-visual/20260907T224737Z`, `ROADMAP.md`, `CHANGELOG.md` |
| 2026-09-04 | Versión 0.167.0: el perfilado source-backed conserva los candidatos categóricos durante el recorrido inicial, acelera la clasificación de texto ASCII y la detección conservadora de fechas, y lee columnas Parquet en paralelo sin alterar paridad ni límites source-backed. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `ROADMAP.md` |
| 2026-09-04 | Versión 0.166.0: el perfilado source-backed usa el pool de concurrencia por bloque para calcular huellas normalizadas y actualizar columnas en paralelo sin cambiar orden ni semántica. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `ROADMAP.md` |
| 2026-09-04 | Versión 0.165.0: el perfilado source-backed cuenta filas distintas y distintos por columna en una sola agregación DuckDB, eliminando el derrame temporal de una clave completa por registro y manteniendo paridad con nulos/repeticiones. | `src-tauri/src/dataset.rs`, `src-tauri/src/duckdb_query.rs`, `CHANGELOG.md`, `ROADMAP.md` |
| 2026-09-04 | Versión 0.164.0: automatización, perfiles y proyectos grandes reutilizan source-backed; el perfil de columnas calcula distintos en una sola agregación DuckDB y conserva fallback cerrado para recetas incompatibles. | `src-tauri/src/dataset.rs`, `src-tauri/src/automation.rs`, `src-tauri/src/duckdb_query.rs`, `CHANGELOG.md`, `ROADMAP.md` |
| 2026-09-04 | Versión 0.163.0: Deshacer/Rehacer source-backed restaura esquema, conteo y primera página desde el cursor Parquet, sin materializar la revisión completa y con fallo cerrado ante snapshots inválidos. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `ROADMAP.md` |
| 2026-09-03 | Versión 0.162.0: el perfilado source-backed comparte recorridos Parquet para duplicados, columnas y correlaciones, con progreso continuo y paridad verificada. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `ROADMAP.md` |
| 2026-09-03 | Versión 0.161.0: resolución de conflictos y consolidación sobre snapshots Parquet durables quedan cubiertas de extremo a extremo, con publicación reversible, orden estable y frame activo sin filas materializadas. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `ROADMAP.md` |
| 2026-09-03 | Versión 0.160.0: conflictos paginados, resolución y consolidación reutilizan snapshots Parquet durables de datasets materializados, con comprobación de cursor durante la lectura y fallback eager cuando no existe un snapshot compatible. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-09-02 | Versión 0.159.0: el updater valida el tamaño del payload descargado contra el manifest y rechaza descargas truncadas o sobredimensionadas antes de preparar la instalación. | `src-tauri/src/updater.rs`, `CHANGELOG.md`, `ROADMAP.md` |
| 2026-09-02 | Versión 0.158.0: las lecturas incrementales validan snapshots Parquet actuales y la validación de calidad rechaza resultados obsoletos si cambia la fuente o el cursor durante el recorrido. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-09-02 | Versión 0.157.0: la entrega remota ODBC reutiliza snapshots Parquet durables de datasets materializados y transmite por bloques desde DuckDB, conservando calidad, privacidad, cancelación, validación de cursor y fallback eager para incompatibilidades. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-09-02 | Versión 0.156.0: perfilado, reglas incrementales y exportaciones locales compatibles reutilizan por bloques el snapshot Parquet durable del cursor actual de datasets materializados, con validación de cursor y fallback eager seguro. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-09-02 | Versión 0.155.0: los JOIN mutadores `INNER`/`LEFT`/`FULL` reutilizan el snapshot Parquet durable del cursor actual para datasets materializados, publican solo el resultado como revisión reversible y conservan fallback eager sin snapshot compatible. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.145.0: `perf:summary` calcula el intervalo entre proceso nativo listo y ventana visible; `perf:check` lo valida contra 1.000 ms junto con hitos, estado y cleanup desktop, separando compilación debug de latencia de aplicación. | `tools/summarize-performance.ps1`, `tools/check-performance-baseline.ps1`, `fixtures/performance/performance-baseline-v1.json`, `CHANGELOG.md`, `ROADMAP.md` |
| 2026-09-01 | Versión 0.144.0: el benchmark source-backed de 512 MiB procesa `INNER`/`LEFT`/`FULL` desde DuckDB, conserva conteos y páginas, deja el frame activo vacío, observa 233.816.064 bytes de working set y confirma cleanup; la ejecución integral fuera de RAM sigue separada. | `tools/benchmark-duckdb-join.ps1`, `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.143.0: la validación de decisiones source-backed recorre conflictos por bloques y conserva solo índices y columnas divergentes, sin materializar los valores completos; el límite explícito sube a 8.192 y las entradas mayores mantienen fallback eager sin salida parcial. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.142.0: la página de conflictos por clave evita materializar el activo source-backed y la resolución acotada publica resultados reversibles desde DuckDB para hasta 2.048 conflictos, con orden, integridad, cleanup y fallback eager. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.140.0: la consolidación por claves entre un activo source-backed y el snapshot Parquet comparado valida duplicados/conflictos en DuckDB, materializa solo las claves nuevas, conserva orden y fuentes, publica un cursor Parquet reversible y actualiza el nombre visible del dataset; formatos incompatibles mantienen fallback eager. | `src-tauri/src/dataset.rs`, `src-tauri/src/duckdb_query.rs`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.139.0: los JOINs source-backed `INNER`/`LEFT`/`FULL` entre fuentes locales compatibles ejecutan en DuckDB, publican solo el resultado Parquet con límite de cardinalidad, conservan orden, fuentes intactas e historial reversible y mantienen fallback eager para formatos incompatibles. | `src-tauri/src/dataset.rs`, `src-tauri/src/duckdb_query.rs`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.138.0: los Bundles source-backed con receta activa conservan la ruta DuckDB incremental e incorporan `recipe.json`, su referencia y hash en `manifest.json`; se preservan fuente original, privacidad, calidad incremental, cancelación, atomicidad y cleanup sin materializar el frame activo. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.137.0: las exportaciones source-backed con protección `mask`/`hash` generan snapshots privados en DuckDB y transfieren todos los destinos locales sin materializar el frame activo; conservan nulos, columnas no personales, validación de cambios, atomicidad y cleanup. | `src-tauri/src/dataset.rs`, `src-tauri/src/duckdb_query.rs`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.136.0: la eliminación de duplicados parecidos y las correcciones recomendadas se ejecutan source-backed con DuckDB; conservan claves normalizadas/exactas, repeticiones idénticas, trim, renombres, orden, conteos, snapshots reversibles y fallback eager. | `src-tauri/src/dataset.rs`, `src-tauri/src/duckdb_query.rs`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.135.0: las acciones directas de outliers `cap`, `impute` y `drop` se ejecutan source-backed con DuckDB; conservan cuantiles/límites IQR y mediana de la ruta eager, tipos y nulos, conteos exactos, snapshots reversibles y fallback eager. | `src-tauri/src/dataset.rs`, `src-tauri/src/duckdb_query.rs`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.134.0: las imputaciones conservadora y categórica se ejecutan source-backed con DuckDB, conservan moda/mediana de la ruta eager, `Desconocido`, conteos exactos, snapshots reversibles y fallback eager. | `src-tauri/src/dataset.rs`, `src-tauri/src/duckdb_query.rs`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.133.0: la conversión numérica y la interpretación de fechas detectadas se ejecutan source-backed con DuckDB, conservan las reglas eager de seguridad, conteos exactos, snapshots reversibles y fallback eager. | `src-tauri/src/dataset.rs`, `src-tauri/src/duckdb_query.rs`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.132.0: la normalización de booleanos se ejecuta source-backed con DuckDB, conserva el umbral eager, alias reconocidos, conteos exactos, snapshots reversibles y fallback eager. | `src-tauri/src/dataset.rs`, `src-tauri/src/duckdb_query.rs`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.131.0: recorte, normalización de texto y valores centinela se ejecutan source-backed con DuckDB, conservan conteos exactos, snapshots reversibles y fallback eager para modos no equivalentes. | `src-tauri/src/dataset.rs`, `src-tauri/src/duckdb_query.rs`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.130.0: la normalización de nombres y la activación de `_cambios` se ejecutan source-backed con DuckDB, conservan colisiones, snapshots reversibles y fallback eager. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.129.0: la máscara de valores personales source-backed usa DuckDB, cuenta celdas no nulas aún no redactadas, conserva `_cambios`, snapshots reversibles, el contrato IPC agregado y fallback eager. | `src-tauri/src/dataset.rs`, `src-tauri/src/duckdb_query.rs`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.128.0: el retiro source-backed de columnas identificadoras y personales detectadas usa DuckDB, conserva `_cambios`, snapshots reversibles, el contrato IPC agregado y fallback eager. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.127.0: duplicados y limpiezas de columnas constantes, vacías o con alta nulidad se ejecutan source-backed con DuckDB, snapshots Parquet reversibles, orden estable, `_cambios` y fallback eager. | `src-tauri/src/dataset.rs`, `src-tauri/src/duckdb_query.rs`, `CHANGELOG.md`, `ROADMAP.md`, `CONTEXTO.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.126.0: la eliminación de filas completamente vacías usa DuckDB sobre la fuente source-backed, publica snapshot Parquet, conserva `_cambios`, activa historial reversible y mantiene fallback eager ante presupuesto insuficiente. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.125.0: `JSON`, `JSONL` y `NDJSON` grandes se abren mediante snapshot Parquet privado de DuckDB, con esquema, preview, conteo, cancelación, verificación de integridad y frame activo sin filas; las recetas source-backed reutilizan el snapshot y validan la fuente original. | `src-tauri/src/dataset.rs`, `src-tauri/src/duckdb_query.rs`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.124.0: los filtros temporales `Eq`/`Neq` comparan fechas ISO sobre `Date` y `Datetime` en eager, Polars lazy y DuckDB source-backed, con paridad y snapshot verificados. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.123.0: las recetas lazy/streaming combinan parseo de fecha, filtros ISO 8601 y extracción de `year`, `month` o `day` sin materialización eager; una regresión verifica orden, conteo y tipos. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.122.0: los filtros ordenados sobre `Date`/`Datetime` aceptan literales ISO 8601 y se ejecutan en eager, Polars lazy y DuckDB source-backed, con paridad de límites, nulos, unidades temporales y snapshot Parquet sin llenar el frame activo. | `src-tauri/src/dataset.rs`, `src/features/prepare/TransformRecipeEditor.tsx`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.121.0: las recetas source-backed parsean fechas y extraen `year`, `month` o `day` después de filtros en DuckDB, conservando conteo, paridad eager, snapshot Parquet y frame activo vacío. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.120.0: el guardado de proyectos source-backed convierte directamente CSV/TSV/TXT delimitado y Parquet a `current.parquet` con DuckDB, verifica el conteo y conserva el `DataFrame` activo vacío; la regresión confirma que la sesión no se materializa. | `src-tauri/src/dataset.rs`, `src-tauri/src/projects.rs`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.119.0: las recetas source-backed ejecutan divisiones calculadas con operandos literales o columnas mediante DuckDB, validan división por cero antes de publicar y conservan nulos, paridad eager y frame activo vacío. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.117.0: las exportaciones source-backed con `mask`/`hash` generan un snapshot Parquet protegido mediante DuckDB y reutilizan la transmisión para los siete destinos locales sin materializar el `DataFrame` activo; la regresión cubre nulos, hashes, columnas protegidas y fuente intacta. | `src-tauri/src/dataset.rs`, `src-tauri/src/duckdb_query.rs`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.109.0: la apertura source-backed conserva esquema y primera página en Polars, cuenta filas con DuckDB y cancelación cooperativa, y la regresión end-to-end confirma conteo exacto, frame activo vacío y cleanup en una fuente CSV de al menos 512 MiB. La ejecución integral fuera de RAM sigue pendiente. | `src-tauri/src/dataset.rs`, `src-tauri/src/duckdb_query.rs`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.110.0: la exportación source-backed a CSV usa DuckDB sobre fuentes delimitadas o Parquet, conserva cancelación, neutralización de fórmulas, validación de cambios y publicación atómica sin materializar el `DataFrame` activo en la ruta compatible. | `src-tauri/src/dataset.rs`, `src-tauri/src/duckdb_query.rs`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.118.0: las recetas source-backed ejecutan reemplazos regex globales con grupos numéricos `$1`–`$9` mediante DuckDB, conservando paridad eager, nulos, conteo de celdas y frame activo vacío; las sustituciones no compatibles mantienen fallback. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.116.0: los libros XLSX/XLSB grandes se abren source-backed; Calamine detecta esquema y tipos en streaming, publica un snapshot Parquet temporal por bloques y deja el `DataFrame` activo vacío. Paginación, perfilado, consultas DuckDB y exportaciones compatibles reutilizan el snapshot, validan el libro original y permiten materializar después; la regresión comprueba tipos, fuente intacta y cleanup. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.115.0: las exportaciones source-backed compatibles a Excel `.xlsx` y SQLite recorren las filas desde DuckDB, conservan esquema y tipos, publican atómicamente y validan cancelación, cambios de la fuente y cleanup sin materializar el `DataFrame` activo; las regresiones reabren ambos destinos. | `src-tauri/src/dataset.rs`, `src-tauri/src/duckdb_query.rs`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.114.0: `npm run brand:check` inspecciona el árbol activo y los archivos no ignorados para impedir el regreso de referencias a la marca retirada; pasa sobre el repositorio actual y no reescribe el historial Git. | `tools/check-retired-brand.mjs`, `package.json`, `tools/check.ps1`, `CHANGELOG.md`, `ROADMAP.md` |
| 2026-09-01 | Versión 0.113.0: la exportación source-backed a Bundle ZIP escribe `dataset.csv` desde DuckDB, calcula tipos y nulos del diccionario sin materializar el `DataFrame` activo, conserva el reporte de calidad incremental opcional y publica hashes, cancelación y atomicidad; la regresión confirma fuente intacta y cleanup. | `src-tauri/src/dataset.rs`, `src-tauri/src/duckdb_query.rs`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.112.0: las consultas source-backed compatibles mantienen `INNER`, `LEFT` y `FULL JOIN` en DuckDB desde la fuente en disco; si la consulta no es compatible, se rechaza la materialización implícita. El benchmark de 512 MiB confirma conteos, paginación, frame vacío, working set y cleanup. | `src-tauri/src/dataset.rs`, `src-tauri/src/duckdb_query.rs`, `tools/benchmark-duckdb-join.ps1`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.111.0: la exportación source-backed a SQL usa DuckDB sobre fuentes delimitadas o Parquet, conserva esquema y literales escapados, cancelación, validación de cambios y publicación atómica sin materializar el `DataFrame` activo en la ruta compatible. | `src-tauri/src/dataset.rs`, `src-tauri/src/duckdb_query.rs`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.108.0: se añade el benchmark opt-in `perf:duckdb:join` para una fuente CSV temporal de 512 MiB; DuckDB conserva el frame source-backed vacío, comprueba conteo/paginación y el working set queda bajo 512 MiB con cleanup confirmado. La ejecución integral fuera de RAM sigue pendiente. | `src-tauri/src/dataset.rs`, `tools/benchmark-duckdb-join.ps1`, `package.json`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.107.0: los JOINs DuckDB compatibles pueden combinar un `DataFrame` activo con el snapshot Parquet comparado, evitando materializar de nuevo sus filas; Polars promueve la ruta automáticamente, y una regresión verifica filas, nulos y orden. La ejecución completa fuera de RAM sigue pendiente. | `src-tauri/src/dataset.rs`, `src-tauri/src/duckdb_query.rs`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.106.0: el workspace SQLite v12 persiste formato de exportación, protección de datos, claves de comparación y tipo de JOIN; Rust valida listas cerradas y la reapertura filtra claves contra el snapshot restaurado sin guardar muestras ni valores. | `src-tauri/src/projects.rs`, `src-tauri/src/automation.rs`, `src/App.tsx`, `src/features/delivery/DeliveryPhase.tsx`, `src/bridge.ts`, `src/App.test.tsx`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.103.0: la cobertura de filas de correlaciones se persiste por proyecto con validación cerrada, migración SQLite v9 y fallback local seguro para catálogos anteriores; la reapertura restaura la preferencia junto con la sesión de Revisar. | `src-tauri/src/projects.rs`, `src/App.tsx`, `src/App.test.tsx`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.104.0: el motor SQL elegido se persiste por proyecto con validación cerrada, migración SQLite v10 y fallback local seguro para catálogos anteriores; la reapertura restaura el motor junto con la sesión de Revisar. | `src-tauri/src/projects.rs`, `src-tauri/src/automation.rs`, `src/App.tsx`, `src/features/review/ReviewPhase.tsx`, `src/App.test.tsx`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.105.1: se renombra el componente interno de vista previa a `DatasetPreviewPanel` para eliminar coincidencias textuales con la marca retirada, sin cambiar el contrato visible ni la funcionalidad. | `src/features/review/ReviewPhase.tsx`, `src/features/review/ReviewPhase.test.tsx`, `CHANGELOG.md`, `ROADMAP.md` |
| 2026-09-01 | Versión 0.105.2: la preparación DuckDB transporta el conteo real del dataset activo para ordenar correctamente los `FULL JOIN` source-backed cuando el esquema en memoria está vacío; la regresión ejecuta ambos lados desde snapshots Parquet sin materializar el activo. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-09-01 | Versión 0.105.0: el perfil de rendimiento se persiste por proyecto con categorías cerradas, migración SQLite v11 y fallback local seguro; la UI restaura el perfil al reabrir y vuelve a la preferencia local al cambiar de dataset o desvincular el proyecto. | `src-tauri/src/projects.rs`, `src-tauri/src/automation.rs`, `src/App.tsx`, `src/components/ResourceMonitor.tsx`, `src/features/projects/useProjectsController.ts`, `src/App.test.tsx`, `src/components/ResourceMonitor.test.tsx`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-08-31 | Versión 0.93.0: las recetas source-backed amplían la ejecución directa en DuckDB a casts, fechas fijas `YMD`/`DMY`/`MDY` y cálculos de suma, resta, multiplicación o concatenación; la regresión Rust comprueba paridad con la ruta eager y división, partes de fecha, ISO y operaciones no compatibles conservan fallback materializado. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-08-30 | M1 expone la importación de sesiones sistema anterior desde Proyectos con selector nativo, progreso por etapas y cancelación cooperativa; el controlador abre el proyecto publicado solo después del commit atómico. | `src/features/projects/ProjectsPanel.tsx`, `src/features/projects/useProjectsController.ts`, `docs/reference/feature-parity.md` |
| 2026-08-29 | P1 añade protección reversible de valores personales detectados: una confirmación sustituye por `[REDACTED]` los valores no nulos de correo, teléfono, dirección y nombre, conserva columnas, números, nulos e `_cambios`, publica solo conteos agregados y deja los identificadores como acción separada. El inventario IPC queda en 64 comandos de producción y 58 estructuras. | `src-tauri/src/dataset.rs`, `src-tauri/src/lib.rs`, `src/bridge.ts`, `src/features/prepare/PreparePhase.tsx`, `src/features/prepare/usePrepareController.ts`, `docs/reference/ipc-inventory.json` |
| 2026-08-29 | P1 expone `parse_dates` como acción directa de Preparar: las columnas de texto con formato dominante cerrado pasan a `Datetime`, las ambiguas se conservan, el impacto es agregado y la mutación queda disponible para Deshacer/Rehacer. El inventario IPC queda en 65 comandos de producción y la suite frontend en 284 pruebas. | `src-tauri/src/dataset.rs`, `src-tauri/src/lib.rs`, `src/bridge.ts`, `src/features/prepare/PreparePhase.tsx`, `src/features/prepare/usePrepareController.ts`, `docs/reference/ipc-inventory.json` |
| 2026-08-29 | M1 amplía el round-trip de sesiones con un `.xlsx` real generado por el exportador nativo: valida la hoja `dataset`, aplica la receta y reabre el proyecto comprobando esquema, conteos y etapa activa; la restauración completa de artefactos históricos sigue pendiente. | `src-tauri/src/projects.rs`, `src-tauri/src/dataset.rs` |
| 2026-08-29 | M1 importa hasta cinco entradas de historial de ejecución solo cuando sus metadatos agregados son seguros (estado, duración y filas), las persiste en la actividad SQL del proyecto con IDs locales y descarta consultas, rutas, valores y entradas inválidas. | `src-tauri/src/dataset.rs`, `src-tauri/src/projects.rs` |
| 2026-08-29 | M1 rechaza antes de publicar una sesión que combine estrategias IQR de outliers mutuamente excluyentes (`cap`, `impute` y `drop`), preservando la semántica de sistema anterior y el catálogo existente. | `src-tauri/src/dataset.rs`, `src-tauri/src/projects.rs` |
| 2026-08-29 | M1/P1 incorpora `mask_pii`, `drop_fuzzy_duplicates` y `parse_dates` al fallback de sesiones sistema anterior: sin snapshot compatible se aplica la máscara predeterminada y conservadora `[REDACTED]`, el fingerprint normalizado local hasta 5.000 filas o el parseo cerrado de fechas con validación de cobertura, preservando nulos y `_cambios`; snapshots compatibles siguen teniendo prioridad y los modos hash/clave explícita o fechas ambiguas requieren revisión. Las etiquetas de etapa conocidas también restauran la etapa activa del workspace y las desconocidas vuelven a Revisar. | `src-tauri/src/dataset.rs`, `src-tauri/src/projects.rs`, `docs/reference/feature-parity.md` |
| 2026-08-28 | T5-06 queda cerrado: la evidencia release se captura desde un árbol limpio, registra commit/rama/lockfiles, cubre desktop, móvil, zoom 125%, zoom 200% y forced-colors, y el checker exige que el único commit posterior sea el del baseline. | `tools/capture-release-evidence.mjs`, `tools/check-release-evidence.mjs`, `tools/capture-release-evidence.ps1`, `fixtures/accessibility/release-evidence-baseline-v1.json` |
| 2026-08-29 | P1/M1 liga los perfiles cacheados del catálogo a la huella SHA-256 de `current.parquet`: al reabrir un proyecto se invalida solo la caché si cambia el snapshot, se conserva compatibilidad con catálogos anteriores y SQLite migra a v5 de forma transaccional. | `src-tauri/src/projects.rs`, `src-tauri/src/dataset.rs`, `ROADMAP.md` |
| 2026-08-28 | I6 añade un contrato updater estructural reproducible y lo integra al perfil Release/Package: un fixture temporal aprueba el par válido y falla ante truncado, firma alterada, manifiesto incompleto/corrupto y URL HTTP. La prueba no contacta la red ni sustituye la validación del canal real. | `tools/test-updater-manifest.mjs`, `tools/check.ps1`, `.local/validation/20260828T214048Z-6ec7bae-release.json` |
| 2026-08-28 | T5-02 queda respaldado por un smoke nativo aislado: los cuatro diálogos Win32 pasan, el driver rechaza ventanas residuales por PID/owner, espera el cierre modal y corta drivers bloqueados; el recorrido oficial registra 521.79 MiB working set, 265.98 MiB privados y cleanup confirmado. El smoke nativo se separa del runner Playwright para medir el presupuesto de WebView2 sin retención del runner. | `tools/automate-native-file-dialog.ps1`, `tools/probe-webview2-cdp.ps1`, `tools/probe-webview2-native-selectors.mjs`, `.local/validation/webview2-cdp/20260828T203917Z/summary.json` |
| 2026-08-28 | Reauditoría profesional exhaustiva sobre `137520b`: 248 tests frontend, 234 Rust y 9 E2E aprobaron; Full/Release/Package pasaron y Package produjo MSI/NSIS. Se verificaron regresiones de rendimiento, verdes falsos de cobertura/tiers/evidencia, deuda de seguridad/arquitectura y bloqueos legales de distribución. Se abrió Tier 5 con 20 tareas; no se corrigió código ni se aprobó un baseline. | `AUDITORIA_PROFESIONAL_2026-08-28.md`, `ROADMAP.md`, `.local/validation/20260828T054125Z-137520b-package.json`, `.local/validation/performance-baseline/20260828T053153Z/summary.json` |
| 2026-08-26 | Review incorpora cobertura temporal agregada para Date/Datetime/Timestamp y fechas detectadas: muestra rango, filas con valor y porcentaje con tabla accesible equivalente sin enviar celdas a React. | `src/features/review/ReviewPhase.tsx`, `src/features/review/ReviewPhase.test.tsx`, `src/styles.css`, `ROADMAP.md` |
| 2026-08-26 | M1 incorpora un mapeo seguro de sesiones sistema anterior al catálogo de proyectos mediante selector nativo. La fuente, hoja, esquema y receta se validan en un estado temporal y el snapshot solo se publica después de pasar todas las comprobaciones; el bridge no recibe rutas. | `src-tauri/src/projects.rs`, `src-tauri/src/lib.rs`, `src/bridge.ts`, `ROADMAP.md` |
| 2026-08-26 | La migración de reglas acepta aliases y números finitos serializados como texto, y omite con warning tolerancias negativas o políticas no representables para evitar conversiones inválidas. | `src-tauri/src/dataset.rs`, `ROADMAP.md` |
| 2026-08-26 | La vista de progreso muestra el tiempo transcurrido de operaciones largas y explica que los datasets grandes pueden tardar varios minutos, manteniendo estados de cancelación y estilos de alto contraste. | `src/components/OperationProgressView.tsx`, `src/components/OperationProgressView.test.tsx`, `src/styles.css` |
| 2026-08-26 | El filtro SQL local se ejecuta por bloques en Rayon: calcula conteos en paralelo y reescanea únicamente los bloques necesarios para una página, preservando el orden y la exactitud de las agregaciones. La vista de progreso ahora comunica operación, etapa, estado de cancelación y porcentaje normalizado con semántica accesible. | `src-tauri/src/dataset.rs`, `src/components/OperationProgressView.tsx`, `src/styles.css`, `ROADMAP.md` |
| 2026-08-26 | Entregar añade el formato `bundle`: un ZIP atómico y cancelable con dataset CSV protegido, diccionario tipado, reporte de calidad opcional y manifest con SHA-256 por archivo; CLI y batch aceptan `bundle`/`zip`. Review añade validación visual de formatos con tabla accesible equivalente. | `src-tauri/src/dataset.rs`, `src-tauri/src/automation.rs`, `src/bridge.ts`, `src/features/delivery/DeliveryPhase.tsx`, `src/features/review/ReviewPhase.tsx`, `ROADMAP.md` |
| 2026-08-26 | P1 añade eliminación difusa explícita y reversible: agrupa por fingerprint XXH3 normalizado, confirma el impacto agregado, conserva primera fila/orden y copias exactas. M1 acepta aliases de sesión sistema anterior `snake_case`/`camelCase` y comprobaciones como objeto o arreglo sin publicar rutas. Review añade ranking y tabla accesible de patrones de nulos. | `src-tauri/src/dataset.rs`, `src-tauri/src/lib.rs`, `src/bridge.ts`, `src/features/prepare/PreparePhase.tsx`, `src/features/review/ReviewPhase.tsx`, `ROADMAP.md` |
| 2026-08-26 | El explorador SQL local limita la memoria de consultas proyectadas: recorre el filtro, cuenta coincidencias y conserva solo la página solicitada; los `GROUP BY` y agregados siguen reteniendo el conjunto necesario para preservar exactitud. | `src-tauri/src/dataset.rs`, `ROADMAP.md` |
| 2026-08-26 | El conteo de duplicados exactos del perfil usa `unique` lazy con motor streaming y proyecta solo el total; si el backend no puede ejecutar el plan, conserva un fallback eager exacto. El perfil numérico comparte una única vista `Float64` entre histograma y atípicos para evitar conversiones duplicadas. | `src-tauri/src/dataset.rs`, `ROADMAP.md` |
| 2026-08-26 | Diagnóstico incorpora una matriz de correlaciones de Pearson acotada a doce columnas numéricas y hasta 100.000 filas muestreadas; conserva solo nombres, coeficientes y conteos de pares, omite columnas constantes y permite cancelar durante el muestreo. Los perfiles antiguos sin esta señal se recalculan al abrirse cuando corresponde. | `src-tauri/src/dataset.rs`, `src/bridge.ts`, `src/features/review/ReviewPhase.tsx`, `src/styles.css`, `ROADMAP.md` |
| 2026-08-26 | La comparación por filas y la agrupación de claves usan reducciones Rayon y ordenan los índices resultantes para conservar determinismo; la mejora aprovecha CPU sin cambiar el contrato ni materializar firmas globales adicionales. | `src-tauri/src/dataset.rs`, `ROADMAP.md` |
| 2026-08-26 | Diagnóstico incorpora histogramas numéricos de 12 intervalos con límites estables y tabla de frecuencias equivalente; el campo es opcional para abrir perfiles antiguos sin invalidarlos. | `src-tauri/src/dataset.rs`, `src/bridge.ts`, `src/features/review/ReviewPhase.tsx`, `src/styles.css`, `ROADMAP.md` |
| 2026-08-26 | Preparar muestra un informe de migración accesible para recetas sistema anterior: métricas de conversión, advertencias y acciones manuales plegables, opciones de entrega y contexto de sesión reducido a estados/conteos; no publica rutas, hashes ni valores originales. | `src/features/prepare/TransformRecipeEditor.tsx`, `src/features/prepare/PreparePhase.test.tsx`, `src/styles.css`, `ROADMAP.md` |
| 2026-08-26 | Preparar incorpora un asesor previo de recetas que explica filas/columnas antes-después, riesgo, confianza y recuperación; las operaciones dependientes de la muestra se presentan como estimaciones. | `src/features/prepare/transformAdvisor.ts`, `src/features/prepare/TransformRecipeEditor.tsx`, `ROADMAP.md` |
| 2026-08-27 | La frontera de privacidad de la CLI ahora sanitiza reportes, recetas y manifiestos completos: elimina rutas, nombres de archivo, valores, emails, secretos y errores largos, pero conserva identificadores, estados y conteos agregados; los conectores remotos siguen desactivados. | `src-tauri/src/privacy.rs`, `src-tauri/src/bin/columnia-cli.rs`, `ROADMAP.md` |
| 2026-08-27 | Cargar conserva hasta cinco entradas recientes como historial local de nombre, formato, fecha e ID opaco; elegir una entrada vuelve a abrir el selector nativo y nunca reutiliza ni persiste rutas. | `src/features/load/recentFilesModel.ts`, `src/features/load/LoadPhase.tsx`, `src/App.tsx` |
| 2026-08-27 | Cargar admite arrastre nativo de archivos: Tauri conserva la ruta en estado Rust, emite un evento opaco y React solicita solo la inspección validada; el selector de hoja y la carga siguen usando el mismo contrato. | `src-tauri/src/dataset.rs`, `src-tauri/src/lib.rs`, `src/bridge.ts`, `src/App.tsx`, `src/features/load/LoadPhase.tsx` |
| 2026-08-27 | SQL local añade cancelación cooperativa por bloques, presupuesto de filas coincidentes para agregaciones y preflight de cardinalidad para rechazar joins many-to-many peligrosos antes de materializar Polars; Revisar muestra además una actividad de las últimas cinco ejecuciones sin persistir consultas; DuckDB y lazy/incremental completo siguen pendientes. | `src-tauri/src/dataset.rs`, `src/features/review/ReviewPhase.tsx`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-08-29 | La comparación por filas y claves fusiona firmas en bloques de 16K filas, conserva determinismo y evita `HashSet` auxiliares redundantes; el mapa global exacto sigue limitado por la memoria disponible y los límites de JOIN/comparación continúan vigentes. | `src-tauri/src/dataset.rs`, `ROADMAP.md`, `CHANGELOG.md` |
| 2026-08-29 | Los JOIN locales por claves ejecutan el plan Polars con motor `streaming` después del preflight de cardinalidad; el resultado sigue materializándose solo dentro de los límites explícitos de entradas y filas, con comprobación posterior antes de publicar. | `src-tauri/src/dataset.rs`, `ROADMAP.md`, `CHANGELOG.md` |
| 2026-08-29 | La agrupación y los resúmenes tipados pueden ejecutarse mediante `group_by_stable` en el plan streaming incluso con filtros previos; la búsqueda/reemplazo literal se aplica antes dentro del plan, el preflight proyecta solo las columnas necesarias y se conservan orden de primera aparición, claves nulas, conteo de filas, `count_unique` y validaciones numéricas. | `src-tauri/src/dataset.rs`, `ROADMAP.md`, `CHANGELOG.md` |
| 2026-08-30 | `find_replace` admite modo regex seguro en recetas eager/lazy y en la migración sistema anterior; conserva grupos de captura, nulos y conteos de celdas, y valida el patrón con el motor Rust antes de modificar o publicar el dataset. | `src-tauri/src/dataset.rs`, `src/bridge.ts`, `src/features/prepare/TransformRecipeEditor.tsx`, `docs/reference/feature-parity.md`, `ROADMAP.md`, `CHANGELOG.md` |
| 2026-08-30 | La normalización lazy de correo, teléfono y dirección puede preceder a la agrupación; el preflight valida claves y agregaciones sobre los valores ya normalizados y conserva los conteos de celdas modificadas. | `src-tauri/src/dataset.rs`, `ROADMAP.md`, `CHANGELOG.md` |
| 2026-08-30 | Las extracciones textuales lazy pueden preceder a la agrupación; tokens, runs y delimitadores literales se validan en la proyección posterior y sus columnas derivadas pueden ser claves o fuentes de agregación sin perder nulos ni orden. | `src-tauri/src/dataset.rs`, `ROADMAP.md`, `CHANGELOG.md` |
| 2026-08-30 | Las columnas calculadas numéricas y concatenadas pueden preceder a la agrupación; el preflight valida la proyección posterior al cálculo y conserva tipos, nulos, orden y conteos de agregación. | `src-tauri/src/dataset.rs`, `ROADMAP.md`, `CHANGELOG.md` |
| 2026-08-30 | Las columnas derivadas por split y merge pueden preceder a la agrupación; el preflight valida la proyección posterior a las etapas estructurales y conserva nulos, orden y conteos de columnas descartadas. | `src-tauri/src/dataset.rs`, `ROADMAP.md`, `CHANGELOG.md` |
| 2026-08-30 | La extracción calculada de año, mes y día usa las funciones temporales del plan streaming para `Date` y `Datetime` sin zona horaria y sus columnas pueden ser claves de agrupación cuando no hay filtros; el preflight conserva el rechazo de fechas fuera de rango y deja fallback eager para zonas horarias o filtros. | `src-tauri/src/dataset.rs`, `ROADMAP.md`, `CHANGELOG.md` |
| 2026-08-30 | La consulta SQL local admite hasta ocho columnas en `GROUP BY`; forma claves compuestas con orden de primera aparición, conserva combinaciones nulas, rechaza claves duplicadas y mantiene el presupuesto de materialización de agregaciones. Los joins fuera de memoria y la ejecución incremental integral siguen pendientes. | `src-tauri/src/dataset.rs`, `ROADMAP.md`, `CHANGELOG.md` |
| 2026-08-30 | Los `JOIN` SQL locales admiten hasta ocho pares de claves entre `dataset` y `compared`; cada par debe cruzar ambos lados, no repetir columnas y conservar tipos compatibles. El preflight de cardinalidad, el orden izquierdo y los límites de materialización siguen vigentes; DuckDB ya dispone de una ruta opcional sobre snapshots, mientras los joins fuera de memoria permanecen pendientes. | `src-tauri/src/dataset.rs`, `ROADMAP.md`, `CHANGELOG.md` |
| 2026-08-30 | Las agregaciones SQL locales recorren los bloques coincidentes en una segunda pasada y conservan solo estados de agregación y grupos; ya no retienen todos los índices de filas, pero mantienen el límite explícito de coincidencias, el orden estable y los resultados exactos. | `src-tauri/src/dataset.rs`, `ROADMAP.md`, `CHANGELOG.md` |
| 2026-08-30 | La consulta SQL local `FULL` procesa el lado activo por bloques y añade por bloques las filas derechas no emparejadas mediante anti-join estable; conserva paginación, agregaciones y orden lógico sin acumular el resultado unido completo, pero el frame derecho anti-join mantiene los límites de materialización actuales. | `src-tauri/src/dataset.rs`, `src-tauri/Cargo.toml`, `ROADMAP.md`, `CHANGELOG.md` |
| 2026-08-30 | P1 incorpora la primera ruta DuckDB opcional para SQL local: valida el contrato restringido, ejecuta sobre snapshots Parquet temporales, conserva conteo/paginación/orden estable y reproduce el esquema coalescido de JOIN `INNER`/`LEFT`/`FULL`; el `DataFrame` activo y la ejecución incremental fuera de RAM permanecen como límites explícitos. | `src-tauri/src/duckdb_query.rs`, `src-tauri/src/dataset.rs`, `src-tauri/Cargo.toml`, `src/bridge.ts`, `src/features/review/ReviewPhase.tsx`, `CHANGELOG.md`, `CONTEXTO.md` |
| 2026-08-30 | P1 hace que DuckDB reutilice el snapshot Parquet administrado de la revisión actual, añadiendo la columna de orden solo en la vista temporal y evitando serializar otra vez el `DataFrame`; la ruta conserva fallback para historiales degradados y no afirma todavía ejecución fuera de RAM. | `src-tauri/src/duckdb_query.rs`, `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `CONTEXTO.md` |
| 2026-08-30 | P1 conserva la fuente comparada en un snapshot Parquet temporal mientras la comparación está activa: conflictos y consolidación leen bajo demanda, los JOIN DuckDB pueden registrar ambos snapshots sin reserializar el frame comparado y el dueño temporal garantiza cleanup al descartar la comparación. | `src-tauri/src/dataset.rs`, `src-tauri/src/duckdb_query.rs`, `CHANGELOG.md`, `CONTEXTO.md` |
| 2026-08-30 | P1 reduce la materialización de JOIN DuckDB: la preparación lee solo el esquema Parquet del snapshot comparado administrado y evita volver a cargar sus filas; la preparación DuckDB tampoco aplica el límite de entradas Polars, mientras el `DataFrame` activo y la ejecución completa fuera de RAM siguen pendientes. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `ROADMAP.md` |
| 2026-08-30 | P1 amplía DuckDB para consultar directamente desde disco la fuente original CSV/TSV/TXT delimitada o Parquet cuando el historial está degradado y el dataset sigue intacto; los JOINs grandes pueden combinarla con el snapshot comparado sin reserializar el activo, y `publish_candidate` invalida la referencia tras cualquier mutación. | `src-tauri/src/duckdb_query.rs`, `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-08-30 | P1 habilita la comparación inicial de fuentes Parquet, delimitadas y JSON por bloques: conserva el snapshot secuencial, cuenta filas por streaming y calcula métricas, claves y conflictos sin materializar otra copia completa; Excel conserva el camino materializado. | `src-tauri/src/dataset.rs`, `src-tauri/src/duckdb_query.rs`, `src-tauri/Cargo.toml`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-08-30 | P1 pagina conflictos desde snapshots Parquet comparados por bloques de 16K, conserva duplicados globales con un índice temporal y valida conteos/cambios del snapshot antes de responder; Excel y la ejecución general fuera de RAM permanecen pendientes. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-08-31 | Versión 0.58.0: la comparación inicial de libros XLSX/XLSB usa el lector secuencial de celdas de Calamine en dos pasadas y escribe snapshots Parquet por bloques de 16K; XLS/ODS conservan fallback porque Calamine no ofrece lectura lazy para esos formatos. | `src-tauri/src/dataset.rs`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-08-31 | Versión 0.59.0: las consultas compatibles elegidas como Polars caen automáticamente a DuckDB sobre la fuente original CSV/TSV/TXT delimitada o Parquet cuando el historial se degrada; los JOINs pueden combinar esa fuente con snapshots comparados sin reconstruir el activo desde el `DataFrame`. | `src-tauri/src/dataset.rs`, `src-tauri/src/duckdb_query.rs`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-08-31 | Versión 0.60.0: la promoción automática a DuckDB cubre todos los JOIN compatibles elegidos como Polars cuando el activo y la comparación tienen snapshots o fuentes de disco válidas, evitando la materialización eager innecesaria y manteniendo fallback seguro. | `src-tauri/src/dataset.rs`, `src-tauri/src/duckdb_query.rs`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-08-31 | Versión 0.61.0: la vista previa de un dataset intacto con historial degradado lee solo la página solicitada desde la fuente original Parquet o CSV/TSV/TXT, valida el tamaño de la fuente y la cantidad esperada de filas de la página, y conserva fallback al frame ante cambios o formatos no compatibles. | `src-tauri/src/dataset.rs`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-08-31 | Versión 0.62.0: la comparación inicial reutiliza el snapshot Parquet administrado del activo o una fuente original Parquet/CSV/TSV/TXT intacta cuando está disponible, compara ambos lados por bloques e índices temporales sin clonar el `DataFrame` activo y conserva fallback materializado ante inconsistencias. | `src-tauri/src/dataset.rs`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-08-31 | Versión 0.67.0: la validación source-backed recorre por bloques las reglas fila-a-fila y de esquema/conteo, comprueba cambios de tamaño y conserva fallback materializado para reglas globales; la paridad con memoria queda cubierta por regresión Rust. | `src-tauri/src/dataset.rs`, `src-tauri/Cargo.toml`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-08-31 | Versión 0.66.0: el perfilado source-backed derrama firmas exactas y normalizadas por cubetas, perfila columnas por bloques, ordena corridas numéricas en disco y limita categorías, tendencias y correlaciones a las columnas/muestras necesarias; la paridad con el perfil en memoria queda cubierta por regresión Rust. | `src-tauri/src/dataset.rs`, `src-tauri/Cargo.toml`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-08-31 | Versión 0.65.0: fuentes CSV/TSV/TXT delimitadas y Parquet de al menos 512 MiB abren source-backed con esquema, primera página y conteo desde disco; paginación/consultas compatibles evitan el `DataFrame` completo y las operaciones eager materializan bajo demanda con validación de cambios. | `src-tauri/src/dataset.rs`, `src-tauri/Cargo.toml`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-08-31 | Versión 0.64.0: las consultas y la conversión source-backed a snapshots Parquet comparten 512 MB de memoria, hasta 8 GB de derrame temporal privado y cleanup automático; la regresión delimitada verifica que el snapshot siga siendo legible. | `src-tauri/src/duckdb_query.rs`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-08-30 | P1 sirve la paginación de la muestra activa desde el snapshot Parquet del cursor con `slice` y colección streaming; los estados degradados conservan el fallback al `DataFrame` cuando la fuente original no puede leerse directamente. | `src-tauri/src/dataset.rs`, `ROADMAP.md`, `CHANGELOG.md` |
| 2026-08-30 | P1 evita una segunda clonación completa al guardar proyectos: la copia aislada del dataset se entrega directamente al escritor `Parquet`, conservando la publicación atómica y la recuperación ante fallos; el benchmark fija el perfil de compilación reproducible y restaura el entorno del proceso. | `src-tauri/src/projects.rs`, `tools/benchmark-datasets.ps1`, `ROADMAP.md`, `CHANGELOG.md` |
| 2026-08-30 | La receta lazy/streaming incorpora parseos explícitos `Ymd`, `Dmy` y `Mdy`, y `Iso8601` sin offset o con sufijo UTC `Z`, para objetivos `Date`/`Datetime`, con trim y nulos preservados; offsets distintos de UTC, zonas horarias y operaciones avanzadas mantienen fallback eager. | `src-tauri/src/dataset.rs`, `ROADMAP.md`, `CHANGELOG.md` |
| 2026-08-30 | La receta lazy/streaming incorpora tratamientos IQR aislados (`cap`, `impute`, `drop`) sobre columnas numéricas, calculando umbrales y conteos después de filtros compatibles y preservando nulos; las etapas previas que alteran valores mantienen fallback eager. | `src-tauri/src/dataset.rs`, `ROADMAP.md`, `CHANGELOG.md` |
| 2026-08-30 | La receta lazy/streaming combina parseos de fecha con conversiones en columnas distintas y permite `split` junto con `merge` cuando se conservan las fuentes; dependencias incompatibles mantienen rechazo o fallback eager. | `src-tauri/src/dataset.rs`, `ROADMAP.md`, `CHANGELOG.md` |
| 2026-08-30 | M1/P1 reduce el pico temporal de restauración de `history_snapshots`: cada Parquet histórico se lee y publica secuencialmente, conserva validación de etiquetas/cursor/cancelación y no acumula todos los `DataFrame` antes del commit; la ejecución lazy del dataset activo y los presupuestos globales de datasets grandes continúan pendientes. | `src-tauri/src/dataset.rs`, `src-tauri/src/projects.rs`, `docs/reference/feature-parity.md`, `CHANGELOG.md`, `CONTEXTO.md` |
| 2026-08-30 | M1/P1 copia los snapshots históricos byte a byte en importación, guardado y apertura; valida footer/esquema en revisiones no cursor, materializa solo el cursor para la comprobación de consistencia y deja la lectura completa restante para undo/redo bajo demanda. | `src-tauri/src/dataset.rs`, `src-tauri/src/projects.rs`, `ROADMAP.md`, `CHANGELOG.md` |
| 2026-08-30 | M1 endurece el contrato frontend de metadatos: `nonPortableArtifacts` queda tipado y los contadores/banderas opcionales se validan antes de aceptar una receta migrada. | `src/bridge.ts`, `src/features/prepare/prepareModel.ts`, `src/features/prepare/prepareModel.test.ts`, `ROADMAP.md`, `CHANGELOG.md` |
| 2026-08-30 | La comparación por claves particiona el índice exacto en 256 cubetas temporales y procesa resumen, nuevas claves y conflictos por cubeta; conserva el orden de conflictos, duplicados y resultados sin retener todos los índices de ambos datasets en memoria. | `src-tauri/src/dataset.rs`, `ROADMAP.md`, `CHANGELOG.md` |
| 2026-08-30 | La comparación completa calcula el multiconjunto de filas comunes procesando las firmas por cubetas temporales y conserva los conteos exactos sin mapas globales de firmas. | `src-tauri/src/dataset.rs`, `ROADMAP.md`, `CHANGELOG.md` |
| 2026-08-30 | El preflight de cardinalidad de los `JOIN` locales derrama las claves de ambos datasets y calcula los productos de duplicidad por cubeta con cancelación cooperativa, sin cambiar los límites ni el rechazo many-to-many. | `src-tauri/src/dataset.rs`, `ROADMAP.md`, `CHANGELOG.md` |
| 2026-08-26 | Parquet se incorpora al mismo camino de lectura streaming mediante `scan_parquet`, con `parallel: None`, baja memoria y `rechunk` desactivado. La carga sigue publicando un `DataFrame` activo para mantener el perfilado, las transformaciones y el historial actuales. | `src-tauri/src/dataset.rs`, `ROADMAP.md` |
| 2026-08-26 | CSV, TSV y TXT delimitado usan `LazyCsvReader` con el motor streaming de Polars, baja memoria y `rechunk` desactivado. La carga sigue publicando un `DataFrame` activo para mantener compatibilidad con el perfilado, las transformaciones y el historial actuales. | `src-tauri/src/dataset.rs`, `ROADMAP.md` |
| 2026-08-26 | El perfilado de datasets grandes distribuye las columnas entre hasta cuatro trabajadores, conserva el orden de resultados y emite progreso ponderado para que la interfaz no parezca detenida entre el 40 % y el 100 %. La optimización reduce presión temporal de memoria, pero no sustituye todavía la materialización inicial del `DataFrame`. | `src-tauri/src/dataset.rs`, `ROADMAP.md` |
| 2026-08-26 | El conteo de duplicados parecidos derrama fingerprints XXH3-128 en 256 cubetas temporales y ordena una cubeta a la vez. El resultado conserva conteo, orden de filas y cancelación; el temporal no incluye valores del dataset y la materialización del `DataFrame` sigue siendo el límite de escala. | `src-tauri/src/dataset.rs`, `ROADMAP.md` |
| 2026-08-27 | Diagnóstico agrega tendencia temporal diaria para rangos de hasta 90 días, conserva días sin filas y limita el payload al mismo presupuesto de periodos; la vista usa la tabla accesible existente y distingue día, mes y año sin transportar celdas. | `src-tauri/src/dataset.rs`, `src/bridge.ts`, `src/features/review/ReviewPhase.tsx`, `src/features/review/ReviewPhase.test.tsx`, `ROADMAP.md` |
| 2026-08-27 | Review muestra un calendario diario cuando el perfil usa granularidad `day`: la intensidad representa filas, los días sin observaciones permanecen visibles y la tabla equivalente conserva valores exactos para teclado y lectores de pantalla. | `src/features/review/ReviewPhase.tsx`, `src/features/review/ReviewPhase.test.tsx`, `src/styles.css`, `ROADMAP.md` |
| 2026-08-27 | Preparar permite revisar y retirar columnas cuyo encabezado se clasifica como identificador; exige confirmación, no expone celdas, conserva al menos una columna y registra el cambio en el historial reversible. Email, teléfono, dirección y nombre permanecen como señales para decidir el tratamiento de salida. | `src-tauri/src/dataset.rs`, `src-tauri/src/lib.rs`, `src/bridge.ts`, `src/features/prepare/usePrepareController.ts`, `src/features/prepare/PreparePhase.tsx`, `ROADMAP.md` |
| 2026-08-27 | La migración de sesiones sistema anterior se puede ejecutar por CLI con `project-save --recipe`; valida antes de escribir, prefiere la fuente y usa un snapshot compatible como respaldo, crea un proyecto nuevo y emite solo un resumen JSON sanitizado. | `src-tauri/src/automation.rs`, `src-tauri/src/projects.rs`, `src-tauri/src/bin/columnia-cli.rs`, `docs/reference/cli.md`, `README.md`, `ROADMAP.md` |
| 2026-08-25 | M1 conserva un resumen estructural sanitizado de sesiones sistema anterior dentro del informe de migración: hoja, etapa, conteos y presencia de referencias de origen/snapshot; las rutas no cruzan el bridge y la restauración histórica completa sigue siendo manual. | `src-tauri/src/dataset.rs`, `src/bridge.ts`, `src/features/prepare/prepareModel.ts`, `src/features/prepare/TransformRecipeEditor.tsx`, `docs/reference/feature-parity.md` |
| 2026-08-24 | I3 estabiliza los selectores nativos Win32 y los incorpora a `verify:tier`: el driver usa UI Automation para el modelo del diálogo, soporta editores Abrir `1148` y Guardar como `1001`, conserva fallback Win32/Unicode y valida los cuatro recorridos sin exponer rutas. | `.local/validation/webview2-cdp/20260824T233922Z`, `tools/automate-native-file-dialog.ps1`, `tools/probe-webview2-native-selectors.mjs`, `tools/verify-tier.ps1` |
| 2026-08-24 | Versión 0.50.0: Review resuelve conflictos por columna/valor dentro del preview y conserva decisiones legacy por fila; Entregar protege texto e identificadores numéricos detectados en CSV, JSON, Parquet, SQL, Excel y SQLite, e informa solo el conteo/nombre de columnas protegidas. | `src-tauri/src/dataset.rs`, `src/bridge.ts`, `src/features/review/ReviewPhase.tsx`, `src/features/review/ReviewPhase.test.tsx`, `src/bridge.test.ts`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-08-24 | Validación final del corte 0.50.0: 177 pruebas Rust y 202 frontend, build Vite, Clippy estricto, formato, documentación, gobernanza, diff limpio, smoke CLI y smoke WebView2 con selectores nativos Win32 aprobados. | `.local/validation/cli-smoke/20260825T000531Z`, `.local/validation/webview2-cdp/20260825T000531Z`, `src-tauri/src/dataset.rs`, `src/bridge.ts`, `src/features/review/ReviewPhase.tsx` |
| 2026-08-24 | Versión 0.51.0: Review pagina conflictos en bloques de 50, conserva decisiones con índices globales y permite resolver todos los conflictos visibles y no visibles sin reducir el dataset; la operación conserva historial reversible. | `src-tauri/src/dataset.rs`, `src-tauri/src/lib.rs`, `src/bridge.ts`, `src/App.tsx`, `src/features/review/ReviewPhase.tsx`, `src/styles.css` |
| 2026-08-24 | Validación final de 0.51.0: 177 pruebas Rust, 204 frontend, build Vite, Clippy estricto, formato, documentación, gobernanza, diff limpio, smoke CLI y smoke WebView2 con los cuatro selectores nativos aprobados. | `.local/validation/cli-smoke/20260825T002214Z`, `.local/validation/webview2-cdp/20260825T002400Z`, `src-tauri/src/dataset.rs`, `src/bridge.ts`, `src/features/review/ReviewPhase.test.tsx` |
| 2026-08-24 | Versión 0.52.0: el shell incorpora tema `Sistema`/`Claro`/`Oscuro` persistente, aplicación temprana antes de montar React, selector accesible en la barra lateral y una capa visual refinada; se preservan los contratos de foco, reduced-motion y `forced-colors`. | `src/components/ThemeSwitcher.tsx`, `src/features/settings/themeModel.ts`, `src/styles.css`, `index.html` |
| 2026-08-24 | Validación de 0.52.0: 209 pruebas frontend, sincronización de versión, build Vite y contratos de accesibilidad del selector de tema aprobados. | `src/components/ThemeSwitcher.test.tsx`, `src/features/settings/themeModel.test.ts`, `src/version-sync.test.ts` |
| 2026-08-24 | Versión 0.53.0: el cargador nativo de recetas acepta el núcleo representable de pipelines sistema anterior v1–v3, normaliza operaciones compatibles y falla cerrado ante regex/booleanos personalizados; se añadió anuncio accesible de etapa activa. | `src-tauri/src/dataset.rs`, `src/features/prepare/TransformRecipeEditor.tsx`, `src/App.tsx`, `docs/reference/feature-parity.md` |
| 2026-08-24 | Validación de 0.53.0: 180 pruebas Rust, 209 frontend, build Vite, formato Rust, documentación y gobernanza aprobados. | `src-tauri/src/dataset.rs`, `src/version-sync.test.ts`, `docs/reference/feature-parity.md` |
| 2026-08-24 | Versión 0.54.0 optimiza el arranque: Review/Preparar/Entregar se dividen en chunks y se precargan al enfocar o pasar el cursor; Tauri muestra el shell sin esperar `get_app_info`; la migración SQLite se difiere hasta la primera operación y el monitor de recursos espera 250 ms. | `src/App.tsx`, `src/components/ResourceMonitor.tsx`, `src-tauri/src/projects.rs`, `src/styles.css` |
| 2026-08-24 | Validación de 0.54.0: 181 pruebas Rust, 209 frontend, build Vite, Clippy estricto, formato Rust, smoke desktop, smoke CDP, resumen y baseline de rendimiento aprobados. El bundle inicial pasó de 359.93 a 253.10 KB raw y de 100.26 a 77.31 KB gzip; el smoke desktop observó Vite listo en 2.31 s, aunque el arranque debug total continúa dominado por la compilación nativa. | `.local/validation/desktop-smoke/20260825T011202Z`, `.local/validation/webview2-cdp/20260825T011234Z`, `.local/validation/performance-summary/summary.json`, `.local/validation/performance-baseline/20260825T011434Z` |
| 2026-08-24 | Versión 0.55.0 completa la vertical de informe de migración de contratos: Rust calcula el SHA-256 de los bytes leídos y publica conteos, warnings, omisiones y acciones manuales; React muestra el informe sin recibir rutas ni datos de filas. | `docs/reference/feature-parity.md`, `src-tauri/src/dataset.rs`, `src/bridge.ts`, `src/features/delivery/DeliveryPhase.tsx`, `.local/validation/release-evidence/20260825T014418Z`, `.local/validation/release-evidence-check/20260825T014616Z` |
| 2026-08-24 | Versión 0.56.0 completa la vertical de opciones de entrega M1: el pipeline conserva formatos compatibles, columnas seleccionadas y privacidad; el informe marca semánticas CSV/ZIP/SQL omitidas y se conserva al guardar la receta. | `docs/reference/feature-parity.md`, `src-tauri/src/dataset.rs`, `src/bridge.ts`, `src/features/prepare/TransformRecipeEditor.tsx`, `src/features/prepare/PreparePhase.test.tsx` |
| 2026-08-24 | Versión 0.57.0 completa el inventario de contratos y las fixtures sintéticas de validación; los documentos se revisan antes de publicarse y no exponen rutas ni datos. | `docs/reference/feature-parity.md`, `fixtures/manifest.json`, `src-tauri/src/dataset.rs` |
| 2026-08-24 | P1 añade destinos locales Excel/SQLite, consulta SQL restringida con filtros, `GROUP BY` y agregaciones seguras, privacidad de exportación con máscara/hash y señales agregadas de limpieza; joins/DuckDB, conectores remotos, catálogo completo de PII y sesión operativa siguen pendientes. | `ROADMAP.md`, `docs/reference/feature-parity.md`, `src-tauri/src/dataset.rs`, `src-tauri/src/automation.rs`, `src/bridge.ts`, `src/features/review/ReviewPhase.tsx`, `src/features/prepare/PreparePhase.tsx`, `src/features/delivery/DeliveryPhase.tsx` |
| 2026-08-24 | P1 amplía el contrato de calidad con `column_compare`, `date_range`, `conditional`, `schema_contract`, `referential_integrity`, `monotonic`, agregados y `distribution_drift`; todos comparten migración sistema anterior, tolerancias, bridge tipado, editor accesible, evaluación Rust/UI/CLI y resultados basados en conteos. | `ROADMAP.md`, `docs/reference/feature-parity.md`, `src-tauri/src/dataset.rs`, `src/bridge.ts`, `src/ipc-contract.test.ts`, `src/features/delivery/deliveryModel.ts`, `src/features/delivery/DeliveryPhase.tsx`, `../sistema anterior/docs/reference/feature-parity.md` |
| 2026-08-24 | P1 cierra el versionado del documento de calidad: formato canónico `columnia-quality-rules` v1, guardado atómico, importación Columnia/sistema anterior v1–v3/legado, rechazo cerrado de contratos futuros o ambiguos, CLI retrocompatible y estado accesible sin rutas en React. | `ROADMAP.md`, `docs/reference/feature-parity.md`, `docs/reference/cli.md`, `src-tauri/src/dataset.rs`, `src-tauri/src/automation.rs`, `src/bridge.ts`, `src/features/delivery/DeliveryPhase.tsx` |
| 2026-08-24 | Validación del documento de calidad versionado: 177 pruebas Rust y 202 frontend, build Vite, Clippy estricto, formato, documentación, diff limpio, smoke CLI con contratos/calidad/proyectos y smoke WebView2 con IPC nativo, foco, landmarks y cleanup aprobados. | `.local/validation/cli-smoke/20260824T222619Z`, `.local/validation/webview2-cdp/20260824T222724Z`, `src-tauri/src/dataset.rs`, `src/bridge.test.ts`, `src/ipc-contract.test.ts`, `src/features/delivery/DeliveryPhase.test.tsx` |
| 2026-08-24 | Validación de `monotonic`: 171 pruebas Rust y 191 frontend, build Vite, contrato IPC, Clippy con `-D warnings`, formato, documentación, diff limpio y smoke WebView2 CDP con IPC nativo, foco, landmarks y cleanup aprobados. | `.local/validation/webview2-cdp/20260824T211555Z`, `src-tauri/src/dataset.rs`, `src/bridge.ts`, `src/ipc-contract.test.ts`, `src/features/delivery/deliveryModel.test.ts`, `src/features/delivery/DeliveryPhase.test.tsx` |
| 2026-08-24 | Validación de `referential_integrity`: 169 pruebas Rust y 188 frontend, build Vite, contrato IPC, Clippy con `-D warnings`, formato, documentación y smoke WebView2 CDP con IPC nativo, foco, landmarks y cleanup aprobados. | `.local/validation/webview2-cdp/20260824T190500Z`, `src-tauri/src/dataset.rs`, `src/bridge.ts`, `src/ipc-contract.test.ts`, `src/features/delivery/deliveryModel.ts`, `src/features/delivery/DeliveryPhase.tsx` |
| 2026-08-24 | Validación de `schema_contract`: 166 pruebas Rust y 184 frontend, build Vite, paridad IPC, Clippy con `-D warnings`, formato, diff limpio y smoke WebView2 CDP con IPC nativo, foco, landmarks y cleanup aprobados. | `.local/validation/webview2-cdp/20260824T184643Z`, `src-tauri/src/dataset.rs`, `src/bridge.ts`, `src/ipc-contract.test.ts`, `src/features/delivery/DeliveryPhase.tsx` |
| 2026-08-24 | Validación de `conditional`: 164 pruebas Rust y 181 frontend, build Vite, paridad IPC, Clippy con `-D warnings`, formato, diff limpio y smoke WebView2 CDP con IPC nativo, foco, landmarks y cleanup aprobados. | `.local/validation/webview2-cdp/20260824T182246Z`, `src-tauri/src/dataset.rs`, `src/bridge.ts`, `src/ipc-contract.test.ts`, `src/features/delivery/DeliveryPhase.tsx` |
| 2026-08-24 | Validación de `date_range`: 161 pruebas Rust y 178 frontend, build Vite, contrato IPC concreto, Clippy con `-D warnings`, formato, diff limpio y smoke WebView2 CDP con Playwright/ProjectsPanel, foco, landmarks, IPC nativo y cleanup aprobados. | `.local/validation/webview2-cdp/20260824T174529Z`, `src-tauri/src/dataset.rs`, `src/features/delivery/DeliveryPhase.tsx`, `src/ipc-contract.test.ts` |
| 2026-08-24 | Validación de `column_compare`: 159 pruebas Rust y 175 frontend, build Vite, IPC concreto, Clippy con `-D warnings`, formato, diff limpio y smoke WebView2 CDP con Playwright/ProjectsPanel, foco, landmarks, IPC nativo y cleanup aprobados. | `.local/validation/webview2-cdp/20260824T172704Z`, `src-tauri/src/dataset.rs`, `src/features/delivery/DeliveryPhase.tsx`, `src/ipc-contract.test.ts` |
| 2026-08-24 | P1 añade eliminación explícita de filas completamente vacías desde Preparar, con detección de nulos/blancos, impacto contado, orden estable, historial reversible, bridge tipado y pruebas Rust/React. | `ROADMAP.md`, `src-tauri/src/dataset.rs`, `src-tauri/src/lib.rs`, `src/bridge.ts`, `src/features/prepare/usePrepareController.ts`, `src/features/prepare/PreparePhase.tsx` |
| 2026-08-24 | P1 añade eliminación explícita y reversible de columnas constantes desde Preparar: usa métricas agregadas, reporta nombres/impacto, excluye columnas totalmente nulas, conserva al menos una columna, invalida perfil/entrega y cubre Rust, bridge y React. | `ROADMAP.md`, `docs/reference/feature-parity.md`, `src-tauri/src/dataset.rs`, `src-tauri/src/lib.rs`, `src/bridge.ts`, `src/features/prepare/usePrepareController.ts`, `src/features/prepare/PreparePhase.tsx` |
| 2026-08-24 | P1 añade eliminación explícita y reversible de columnas completamente vacías desde Preparar, separada de constantes, con detección 100% nula, nombres/impacto, conservación de al menos una columna, bridge tipado y pruebas Rust/React. | `ROADMAP.md`, `docs/reference/feature-parity.md`, `src-tauri/src/dataset.rs`, `src-tauri/src/lib.rs`, `src/bridge.ts`, `src/features/prepare/usePrepareController.ts`, `src/features/prepare/PreparePhase.tsx` |
| 2026-08-24 | P1 añade tratamiento explícito y reversible de columnas con al menos 80% de valores nulos: excluye columnas 100% nulas, muestra el umbral, reporta nombres/impacto, conserva al menos una columna y cubre Rust, bridge y React. | `ROADMAP.md`, `docs/reference/feature-parity.md`, `src-tauri/src/dataset.rs`, `src-tauri/src/lib.rs`, `src/bridge.ts`, `src/features/prepare/usePrepareController.ts`, `src/features/prepare/PreparePhase.tsx` |
| 2026-08-24 | P1 añade detección agregada y normalización reversible de centinelas textuales conocidos: cuenta por columna, convierte solo texto a nulos, preserva tipos no textuales y `_cambios`, invalida perfil/entrega y cubre Rust, bridge y React. | `ROADMAP.md`, `docs/reference/feature-parity.md`, `src-tauri/src/dataset.rs`, `src-tauri/src/lib.rs`, `src/bridge.ts`, `src/features/prepare/usePrepareController.ts`, `src/features/prepare/PreparePhase.tsx` |
| 2026-08-24 | P1 añade normalización reversible de alias booleanos, clasificación agregada de señales de privacidad por nombre y activación de `_cambios` con etiquetas de operaciones posteriores, conservación ante mutaciones e historial reversible. | `ROADMAP.md`, `docs/reference/feature-parity.md`, `src-tauri/src/dataset.rs`, `src-tauri/src/lib.rs`, `src/bridge.ts`, `src/features/prepare/usePrepareController.ts`, `src/features/prepare/PreparePhase.tsx` |
| 2026-08-24 | P1 añade conteo agregado de duplicados parecidos (normalización de mayúsculas, espacios y acentos, excluyendo exactos) e imputación conservadora reversible de nulos textuales/números, con bridge, historial, acciones accesibles y pruebas Rust/React. | `ROADMAP.md`, `docs/reference/feature-parity.md`, `src-tauri/src/dataset.rs`, `src-tauri/src/lib.rs`, `src/bridge.ts`, `src/features/prepare/usePrepareController.ts`, `src/features/prepare/PreparePhase.tsx` |
| 2026-08-24 | Validación de duplicados parecidos e imputación conservadora: 158 pruebas Rust, 172 frontend, contrato IPC, build Vite, Clippy con `-D warnings`, formato, diff limpio y smoke WebView2 CDP con Playwright/ProjectsPanel, foco, landmarks, IPC nativo y cleanup aprobados. | `.local/validation/webview2-cdp/20260824T164340Z`, `src-tauri/src/dataset.rs`, `src/features/prepare/PreparePhase.tsx`, `src/bridge.ts` |
| 2026-08-24 | Validación de las verticales booleanos/privacidad/auditoría: 156 pruebas Rust, 171 frontend, contrato IPC, build Vite, Clippy con `-D warnings`, formato, diff limpio y smoke WebView2 CDP con Playwright/ProjectsPanel, foco, landmarks, IPC nativo y cleanup aprobados. | `.local/validation/webview2-cdp/20260824T053845Z`, `src-tauri/src/dataset.rs`, `src/features/prepare/PreparePhase.tsx`, `src/bridge.ts` |
| 2026-08-24 | Validación posterior a la slice de centinelas: 154 pruebas Rust, 168 frontend, contrato IPC, build Vite, clippy con `-D warnings`, formato, diff limpio y smoke WebView2 CDP con Playwright/ProjectsPanel, foco, landmarks, IPC nativo y cleanup aprobados. | `.local/validation/webview2-cdp/20260824T050852Z`, `src-tauri/src/dataset.rs`, `src/features/prepare/PreparePhase.tsx`, `src/bridge.ts` |
| 2026-08-24 | Validación posterior a la slice de alta nulidad: 153 pruebas Rust, 167 frontend, build Vite, clippy con `-D warnings`, formato, diff limpio y smoke WebView2 CDP con Playwright/ProjectsPanel, foco, landmarks, IPC nativo y cleanup aprobados. | `.local/validation/webview2-cdp/20260824T045913Z`, `src-tauri/src/dataset.rs`, `src/features/prepare/PreparePhase.tsx`, `src/styles.css` |
| 2026-08-24 | Validación posterior a la slice de columnas completamente vacías: 152 pruebas Rust, 165 frontend, build Vite, clippy con `-D warnings`, formato, diff limpio y smoke WebView2 CDP con Playwright/ProjectsPanel, foco, landmarks, IPC nativo y cleanup aprobados. | `.local/validation/webview2-cdp/20260824T044319Z`, `src-tauri/src/dataset.rs`, `src/features/prepare/PreparePhase.tsx`, `src/styles.css` |
| 2026-08-24 | Validación posterior a la slice de columnas constantes: 151 pruebas Rust, 163 frontend, build Vite, clippy con `-D warnings`, formato, diff limpio y smoke WebView2 CDP con Playwright/ProjectsPanel, foco, landmarks, IPC nativo y cleanup aprobados. | `.local/validation/webview2-cdp/20260824T043701Z`, `src-tauri/src/dataset.rs`, `src/features/prepare/PreparePhase.tsx`, `src/styles.css` |
| 2026-08-24 | P1 añade resolución interactiva acotada de conflictos por clave: Review muestra celdas divergentes, exige una elección activa/comparada por conflicto y Rust construye el resultado con historial reversible; los previews sobre el límite se bloquean y la combinación independiente por columna sigue pendiente. | `ROADMAP.md`, `docs/reference/feature-parity.md`, `src-tauri/src/dataset.rs`, `src-tauri/src/lib.rs`, `src/bridge.ts`, `src/App.tsx`, `src/features/review/ReviewPhase.tsx` |
| 2026-08-24 | Validación final de este corte P1: 150 pruebas Rust, 161 frontend, build Vite, clippy con `-D warnings`, formato, diff limpio y smoke WebView2 CDP con Playwright/ProjectsPanel, foco, landmarks, IPC nativo y cleanup aprobados. | `.local/validation/webview2-cdp/20260824T042223Z`, `src-tauri/src/dataset.rs`, `src/features/review/ReviewPhase.tsx`, `src/styles.css` |
| 2026-08-24 | Auditoría M1 y primera vertical de migración desde `sistema anterior`: se separó la paridad funcional pendiente de la compatibilidad de artefactos, y Entregar ya importa parcialmente contratos JSON de reglas representables con tolerancias, límites y warnings/omitidas; pipelines, sesiones y round-trip siguen pendientes. | `ROADMAP.md`, `docs/reference/feature-parity.md`, `src-tauri/src/dataset.rs`, `src-tauri/src/lib.rs`, `src/bridge.ts`, `src/features/delivery/DeliveryPhase.tsx`, `../sistema anterior/docs/reference/feature-parity.md` |
| 2026-08-24 | M1 amplía la vertical de contratos de calidad con un informe estructurado: conteos de entradas/convertidas/omitidas, advertencias, acciones manuales y SHA-256 del archivo original; el hash se calcula dentro de Rust y la UI nunca recibe rutas ni valores del dataset. | `docs/reference/feature-parity.md`, `src-tauri/src/dataset.rs`, `src/bridge.ts`, `src/features/delivery/DeliveryPhase.tsx` |
| 2026-08-24 | Validación de la quinta entrega P1: 135 pruebas Rust, 145 frontend, cobertura V8, build Vite, clippy, documentación y smoke WebView2 CDP con IPC nativo, foco, landmarks y cleanup aprobados. | `.local/validation/webview2-cdp/20260824T011617Z`, `src-tauri/src/dataset.rs`, `src/App.tsx`, `src/features/review/ReviewPhase.tsx` |
| 2026-08-24 | P1 añade joins multidataset `Inner`, `Left` y `Full` por claves explícitas: valida existencia/tipos, conserva la relación seleccionada, sufija columnas compartidas y registra la unión en historial; la resolución interactiva de conflictos sigue pendiente. | `src-tauri/src/dataset.rs`, `src/bridge.ts`, `src/App.tsx`, `src/features/review/ReviewPhase.tsx`, `docs/reference/feature-parity.md` |
| 2026-08-23 | P1 amplía la calidad v3 con `allowed_values`, `regex`, `dtype`, unicidad compuesta y `row_count`; el mismo contrato se evalúa en Rust para UI, CLI y exportación, con límites de payload, controles accesibles y pruebas avanzadas. | `src-tauri/src/dataset.rs`, `src/bridge.ts`, `src/features/delivery/DeliveryPhase.tsx`, `src/features/delivery/deliveryModel.ts`, `docs/reference/feature-parity.md` |
| 2026-08-24 | P1 añade comparación por claves explícitas: valida presencia y tipos, informa claves coincidentes/exclusivas/duplicadas/conflictivas y consolida solo claves nuevas cuando la operación es segura; los joins multidataset siguen pendientes. | `src-tauri/src/dataset.rs`, `src/bridge.ts`, `src/App.tsx`, `src/features/review/ReviewPhase.tsx`, `docs/reference/feature-parity.md` |
| 2026-08-24 | Validación de la cuarta entrega P1: 145 pruebas frontend, cobertura V8, build Vite, documentación y smoke WebView2 CDP con Playwright/ProjectsPanel, foco, landmarks, IPC nativo y cleanup aprobados. | `.local/validation/webview2-cdp/20260824T010540Z`, `src/features/review/ReviewPhase.tsx`, `src/features/review/ReviewPhase.test.tsx` |
| 2026-08-24 | P1 añade visualizaciones accesibles en Diagnóstico: barras de completitud y posibles outliers con valores exactos, soporte responsive/forced-colors y tablas equivalentes para lector de pantalla; los gráficos exploratorios interactivos siguen pendientes. | `src/features/review/ReviewPhase.tsx`, `src/styles.css`, `src/features/review/ReviewPhase.test.tsx`, `docs/reference/feature-parity.md` |
| 2026-08-24 | Validación de la tercera entrega P1: 144 pruebas frontend, 133 Rust, build, cobertura, clippy, documentación y smoke WebView2 CDP con Playwright/ProjectsPanel y cleanup aprobados. | `.local/validation/webview2-cdp/20260824T005528Z`, `src-tauri/src/dataset.rs`, `src/features/delivery/DeliveryPhase.tsx` |
| 2026-08-24 | P1 añade script SQL portable: Entregar, CLI, batch y proyectos publican una tabla `dataset` con transacción, escape de identificadores/valores y escritura atómica; no hay conexión directa ni secretos de base de datos. | `src-tauri/src/dataset.rs`, `src-tauri/src/automation.rs`, `src/features/delivery/DeliveryPhase.tsx`, `docs/reference/feature-parity.md` |
| 2026-08-24 | Validación posterior a la segunda entrega P1: 142 pruebas frontend, 131 Rust, build, cobertura V8, clippy, documentación y smoke WebView2 CDP con Playwright/ProjectsPanel y cleanup aprobados. | `.local/validation/webview2-cdp/20260824T004109Z`, `src/features/review/ReviewPhase.test.tsx`, `src-tauri/src/dataset.rs` |
| 2026-08-24 | P1 añade comparación/consolidación básica: el dataset secundario se lee localmente, se comparan filas multivaluadas y columnas, y la consolidación compatible se registra en historial sin exponer rutas. Excel usa la primera hoja; claves explícitas, conflictos y joins siguen pendientes. | `docs/reference/feature-parity.md`, `src-tauri/src/dataset.rs`, `src/bridge.ts`, `src/features/review/ReviewPhase.tsx` |
| 2026-08-23 | I3/I4/I5 avanzan con 137 tests frontend, cobertura V8, smoke CDP real, auditorías npm/Cargo con excepciones transitivas explícitas, secret scan, notices, política de red sin telemetría y contrato NSIS/WebView2 con recursos legales; Release y Package pasan con MSI/NSIS 0.49.0. Quedan pendientes el selector nativo estable, la auditoría manual y la validación desde VM limpia. | `vitest.config.ts`, `.local/validation/20260823T224857Z`, `.local/validation/20260823T225136Z-ac3c0a1-release.json`, `.local/validation/20260823T225800Z-ac3c0a1-package.json`, `tools/check-supply-chain.ps1`, `src-tauri/deny.toml`, `tools/check-network-policy.mjs`, `tools/check-installer-contract.ps1`, `docs/reference/network-privacy.md` |
| 2026-08-23 | Se cerró la Fase I0 con licencia MIT, Windows x64 como soporte inicial, ADR de frontera Rust/UI, política local de ramas/commits, inventario de dependencias y manifest de fixtures sintéticas; `governance:check`, Fast y Full pasan. | `LICENSE`, `CONTRIBUTING.md`, `docs/adr/0001-contratos-del-repositorio.md`, `docs/reference/`, `tools/check-governance.ps1`, `src/governance.test.ts` |
| 2026-08-23 | Se añadió el probe de selectores nativos: PowerShell conduce los diálogos Win32 y WebView2 espera los resultados IPC para abrir dataset, guardar/cargar receta y exportar sin exponer rutas; el cierre se estabilizó y se elevó a gate en I3 el 2026-08-24. | `tools/probe-webview2-native-selectors.mjs`, `tools/automate-native-file-dialog.ps1`, `tools/probe-webview2-cdp.ps1` |
| 2026-08-22 | El smoke desktop separa hitos monotónicos de Vite, proceso debug y ventana visible, y confirma cleanup con hasta dos intentos acotados; se añadieron contratos de landmarks, estados ARIA y alertdialog. | `tools/smoke-tauri.ps1`, `src/components/AccessibilityContracts.test.tsx` |
| 2026-08-22 | Playwright 1.62 quedó configurado contra un preview Vite local (Edge en Windows, Chromium en otros sistemas) y E2E del shell web; los artefactos de diagnóstico quedan ignorados y la cobertura nativa Tauri/IPC sigue pendiente. | `playwright.config.ts`, `e2e/shell.spec.ts`, `package.json` |
| 2026-08-22 | El E2E añade un mock aislado de `__TAURI_INTERNALS__` para recorrer cargar, guardar, abrir y eliminar proyectos sin tocar filesystem; la ventana WebView2 real sigue fuera de alcance. | `e2e/projects.spec.ts` |
| 2026-08-22 | El shell marca `columnia:app-render` y Playwright comprueba el primer render en menos de 3 s sobre el preview local; la medición nativa queda pendiente de CDP. | `src/App.tsx`, `e2e/performance.spec.ts` |
| 2026-08-22 | El probe Windows levanta `npm run tauri dev` con CDP de loopback aislado, verifica endpoints y conecta Playwright para lectura DOM; no ejecuta mutaciones IPC y restaura la variable de entorno al limpiar. | `tools/probe-webview2-cdp.ps1`, `tools/probe-webview2-playwright.mjs` |
| 2026-08-22 | Playwright cubre landmarks, foco visible, targets mínimos y el ciclo de foco/restauración del diálogo destructivo en el shell web. | `e2e/accessibility.spec.ts` |
| 2026-08-22 | El probe CDP mide primer render nativo con `columnia:app-render` bajo 3 s y verifica skip link → foco de `main`; el E2E responsive cubre reduced-motion y viewports sin overflow horizontal. | `tools/probe-webview2-playwright.mjs`, `e2e/preferences.spec.ts`, `src/styles.css` |
| 2026-08-22 | El smoke de escritorio valida el contrato estático de `ProjectsPanel` y confirma que la ventana debug/WebView2 inició; no simula clics porque UI Automation solo expone paneles del WebView2 y no el DOM React sin ampliar la configuración de depuración. | `tools/smoke-tauri.ps1`, `src/features/projects/ProjectsPanel.tsx` |
| 2026-08-22 | El smoke CDP combinado añade una lectura DOM de solo lectura del contrato de `ProjectsPanel` y un resumen local de rendimiento con deltas; las acciones que abren diálogos o escriben proyectos siguen fuera del probe. | `tools/probe-webview2-projects.mjs`, `tools/probe-webview2-cdp.ps1`, `tools/summarize-performance.ps1` |
| 2026-08-22 | v0.39.0 añade al probe CDP un perfil del árbol de procesos propio de Columnia/WebView2 (muestras, nombres, working set inicial/máximo/final y memoria privada máxima), conservado en el resumen sin rutas ni datos; el presupuesto global de RAM sigue pendiente. | `tools/probe-webview2-cdp.ps1`, `tools/summarize-performance.ps1` |
| 2026-08-22 | v0.40.0 añade un recorrido nativo temporal de `probe_seed_dataset` → `save_project` → `open_project` → `get_dataset_page` → `delete_project` bajo debug; el runner verifica cleanup y no registra datos ni rutas. | `src-tauri/src/dataset.rs`, `tools/probe-webview2-projects.mjs`, `tools/probe-webview2-cdp.ps1` |
| 2026-08-22 | v0.41.0 añade persistencia temporal de receta, aplicación nativa y exportación CSV atómica con quality gate dentro del mismo probe WebView2; el workspace del proyecto restaura regla y borrador sin exponer rutas. | `src-tauri/src/dataset.rs`, `src-tauri/src/lib.rs`, `tools/probe-webview2-projects.mjs` |
| 2026-08-22 | v0.42.0 añade reapertura durable nativa bajo debug: una instancia fresca de `ProjectStore` valida SQLite, snapshot, recovery y workspace antes de la apertura IPC y cleanup. | `src-tauri/src/projects.rs`, `src-tauri/src/lib.rs`, `tools/probe-webview2-projects.mjs` |
| 2026-08-22 | v0.43.0 añade `smoke:restart` con dos procesos Tauri/WebView2 independientes: preparar→cerrar→reiniciar→reabrir→eliminar, con cleanup delegado a cada fase y evidencia sin datos ni rutas. | `tools/probe-webview2-restart.ps1`, `tools/probe-webview2-cdp.ps1`, `tools/probe-webview2-projects.mjs` |
| 2026-08-22 | v0.44.0 añade duraciones sanitizadas por operación IPC y un presupuesto CDP de 512 MiB working set/256 MiB memoria privada, con muestras posteriores a los probes y resumen de rendimiento persistente. | `tools/probe-webview2-projects.mjs`, `tools/probe-webview2-cdp.ps1`, `tools/summarize-performance.ps1` |
| 2026-08-22 | v0.45.0 añade estilos `forced-colors` y captura reproducible de evidencia visual en cuatro escenarios, con validación de landmarks, foco, targets y overflow sin datos de usuario. | `src/styles.css`, `src/components/AccessibilityStyles.test.ts`, `tools/capture-accessibility-evidence.mjs` |
| 2026-08-23 | v0.46.0 versiona los contratos de baseline visual y rendimiento; añade hashes de capturas, gates de memoria/benchmark/bundle y una verificación compuesta sin tocar el motor de datasets. | `fixtures/accessibility/`, `fixtures/performance/`, `tools/check-accessibility-baseline.mjs`, `tools/check-performance-baseline.ps1`, `tools/verify-experience.ps1` |
| 2026-08-23 | v0.47.0 convierte el benchmark CLI en una señal sostenida y añade el ciclo durable de proyecto con receta, reglas, perfil, exportación y cleanup; no declara completada la medición nativa ni lazy/incremental. | `tools/benchmark-datasets.ps1`, `fixtures/performance/performance-baseline-v1.json`, `tools/check-performance-baseline.ps1` |
| 2026-08-23 | v0.48.0 añade presupuestos de duración, stress de actualización/reapertura durable, `verify:tier` y checklist manual de accesibilidad; lector de pantalla real, medición WebView2 y lazy/incremental siguen pendientes. | `tools/benchmark-datasets.ps1`, `tools/check-performance-baseline.ps1`, `tools/verify-tier.ps1`, `ACCESSIBILITY_MANUAL_CHECKLIST.md` |
| 2026-08-23 | v0.49.0 mide tres ciclos de transformación/exportación nativos dentro de WebView2 y los incorpora al gate junto al presupuesto global de memoria; datasets grandes, lector real y lazy/incremental siguen pendientes. | `tools/probe-webview2-projects.mjs`, `tools/probe-webview2-cdp.ps1`, `tools/summarize-performance.ps1`, `tools/check-performance-baseline.ps1` |
| 2026-08-23 | I8 separa la documentación por Diátaxis, mantiene CHANGELOG/ADR indexados y convierte la evidencia visual del release en un contrato reproducible desde `columnia.exe`; las capturas permanecen locales y el baseline conserva hashes, ownership y propósito. | `docs/`, `CHANGELOG.md`, `tools/check-documentation.mjs`, `tools/capture-release-evidence.ps1`, `tools/check-release-evidence.mjs`, `fixtures/accessibility/release-evidence-baseline-v1.json` |
| 2026-08-23 | I1 queda completa: las recetas compatibles usan Polars lazy con fallback eager seguro, se añade `get_resource_usage` con monitor compacto en el lateral y el benchmark cruzado de 100 MiB contra `sistema anterior` queda aprobado con evidencia visual release 4/4. | `src-tauri/src/resource.rs`, `src/components/ResourceMonitor.tsx`, `src-tauri/src/dataset.rs`, `tools/benchmark-datasets.ps1`, `tools/benchmark-datasets.ps1`, `fixtures/performance/i1-benchmark-baseline-v1.json` |
| 2026-08-27 | P1 añade perfiles persistentes de concurrencia Rayon (`conservative`, `balanced`, `maximum`) con límite de 64 hilos, aplicación opt-in antes de la primera operación y estado honesto cuando el pool global ya fue inicializado; la GPU continúa explícitamente no disponible. | `src-tauri/src/resource.rs`, `src-tauri/src/lib.rs`, `src/bridge.ts`, `src/components/ResourceMonitor.tsx`, `src/features/settings/performanceModel.ts` |
| 2026-08-21 | La CLI administra proyectos en un `--store` obligatorio y canonicalizado mediante cinco comandos; exportar respeta las reglas guardadas y borrar exige confirmar el ID exacto, sin exponer rutas ni muestras en JSON. | `src-tauri/src/automation.rs`, `src-tauri/src/projects.rs`, `README.md`, `THREAT_MODEL.md` |
| 2026-08-21 | SQLite v3 migra catálogos v1/v2 y conserva perfil cacheado e historial/cursor; abrir valida todo y crea una copia temporal de sesión, manteniendo 12 revisiones/1 GiB. | `src-tauri/src/projects.rs`, `src-tauri/src/dataset.rs`, `src/features/projects/` |
| 2026-08-21 | El esquema SQLite v2 conserva reglas de calidad y borrador opcional de receta en cada proyecto, migra catálogos v1 y mantiene perfil e historial como estado temporal. | `src-tauri/src/projects.rs`, `src-tauri/src/dataset.rs`, `src/features/projects/` |
| 2026-08-21 | Proyectos v1 persisten un catálogo SQLite y generaciones Parquet privadas; guardado, apertura, recuperación y borrado no exponen rutas a React. | `src-tauri/src/projects.rs`, `src/features/projects/`, `src/bridge.ts` |
| 2026-08-21 | `LoadedDataset` separa identidad visible y ruta fuente opcional para que un proyecto siga funcionando después de borrar la fuente original. | `src-tauri/src/dataset.rs`, `src-tauri/src/projects.rs` |
| 2026-08-21 | La CLI ejecuta manifiestos batch v1 de hasta 64 trabajos, con preflight completo, outputs atómicos individuales y fallo parcial explícito por ordinal. | `src-tauri/src/automation.rs`, `tools/smoke-cli.ps1`, `fixtures/automation/` |
| 2026-08-21 | Preparar e Historial se extrajeron a vistas, modelo y controlador IPC; `App.tsx` bajó de 1,645 a 455 líneas en ese corte y hoy ronda 504 tras integrar proyectos. | `src/features/prepare/`, `src/features/projects/`, `src/App.tsx` |
| 2026-08-21 | La CLI admite libros mediante hoja exacta y encabezado explícito, y valida contratos de calidad con salida JSON de conteos y códigos 0/2/1. | `src-tauri/src/automation.rs`, `src-tauri/src/dataset.rs`, `tools/smoke-cli.ps1` |
| 2026-08-21 | Cargar y Revisar se extrajeron a módulos tipados y probados; `App.tsx` se redujo en otras 520 líneas. | `src/features/load/`, `src/features/review/`, `src/App.tsx` |
| 2026-08-21 | La automatización local incorpora `columnia-cli inspect/transform` sobre el mismo motor Rust, con contratos JSON v1, rutas no expuestas y exportación atómica. | `src-tauri/src/automation.rs`, `src-tauri/src/bin/columnia-cli.rs` |
| 2026-08-21 | Un smoke con fixtures deterministas valida CSV, Parquet, neutralización de fórmulas y errores sin outputs parciales. | `tools/smoke-cli.ps1`, `fixtures/automation/` |
| 2026-08-21 | La fase Entregar se extrajo a un módulo tipado y probado; `App.tsx` se redujo en 302 líneas. | `src/features/delivery/`, `src/App.tsx` |
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

- [AUDITORIA_PROFESIONAL_2026-08-28.md](AUDITORIA_PROFESIONAL_2026-08-28.md): informe exhaustivo, evidencia, puntuaciones y trazabilidad hacia Tier 5.
- [README.md](README.md): visión funcional y uso actual.
- [THREAT_MODEL.md](THREAT_MODEL.md): fronteras de confianza, amenazas, controles y riesgos residuales.
- [ROADMAP.md](ROADMAP.md): planificación, decisiones históricas y pendientes.
- [CONTRIBUTING.md](CONTRIBUTING.md): ramas, commits y revisión local.
- [docs/README.md](docs/README.md): ADRs, gobierno, dependencias y fixtures.
- [LICENSE](LICENSE): licencia MIT del proyecto.
- [package.json](package.json): scripts y dependencias frontend.
- [src-tauri/Cargo.toml](src-tauri/Cargo.toml): dependencias del motor nativo.
- [src-tauri/tauri.conf.json](src-tauri/tauri.conf.json): configuración de escritorio y CSP.
| 2026-08-31 | Versión 0.80.0: recetas source-backed de renombres y selección/orden de columnas se convierten directamente desde CSV/TSV/TXT delimitado o Parquet a un snapshot Parquet privado; el esquema, conteo, orden y preview se conservan sin llenar el `DataFrame`, y las demás transformaciones mantienen fallback eager. | `src-tauri/src/dataset.rs`, `src-tauri/src/duckdb_query.rs`, `src-tauri/Cargo.toml`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-08-31 | Versión 0.70.0: la validación source-backed amplía la ejecución por bloques a unicidad simple/compuesta, monotonicidad, agregados y deriva de distribución; la regresión cubre duplicados que cruzan bloques sin materializar la fuente. | `src-tauri/src/dataset.rs`, `src-tauri/Cargo.toml`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-08-31 | Versión 0.69.0: la exportación JSON source-backed sin receta ni privacidad adicional convierte directamente CSV/TSV/TXT delimitado o Parquet desde la fuente con límites de DuckDB, publicación atómica, validación de cambios y cleanup; una regresión confirma la salida JSON sin materializar el `DataFrame` activo. | `src-tauri/src/dataset.rs`, `src-tauri/src/duckdb_query.rs`, `src-tauri/Cargo.toml`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |
| 2026-08-31 | Versión 0.68.0: la exportación Parquet source-backed sin receta ni privacidad adicional convierte directamente desde la fuente, conserva límites de DuckDB, publicación atómica, validación de cambios y cleanup; una regresión confirma la salida sin materializar el `DataFrame` activo. | `src-tauri/src/dataset.rs`, `src-tauri/Cargo.toml`, `CHANGELOG.md`, `ROADMAP.md`, `docs/reference/feature-parity.md` |

## Optimización arquitectónica — 2026-09-07

La frontera Tauri del frontend conserva `src/bridge.ts` como fachada pública para
evitar migraciones en sus consumidores. Sus responsabilidades internas quedan
separadas en `src/bridge/`: `contracts.ts` y `client.ts` son barrels internos;
los contratos se distribuyen entre `system-contracts.ts`, `dataset-contracts.ts`,
`recipe-contracts.ts`, `delivery-contracts.ts` y `project-contracts.ts`; y los
clientes viven en `system.ts`, `datasets.ts`, `delivery.ts`, `prepare.ts` y
`projects.ts`. `progress.ts` concentra la adaptación de canales. La prueba de
paridad IPC lee estas fuentes explícitamente y mantiene la comparación de comandos,
argumentos, retornos y tipos.

El catálogo y los comandos de datasets de ejemplo salen del motor monolítico hacia
`src-tauri/src/dataset/samples.rs`. `dataset.rs` conserva el procesamiento central y
`lib.rs` registra los comandos desde su módulo propietario. Esta extracción no
cambia payloads, rutas públicas, persistencia ni comportamiento de ejecución.

Las 13,735 líneas de pruebas unitarias del motor se trasladan desde el archivo de
producción a `src-tauri/src/dataset/tests.rs`. El módulo continúa siendo hijo de
`dataset`, por lo que conserva acceso a sus detalles internos y ejecuta exactamente
los mismos casos, mientras `dataset.rs` queda dedicado al código de producción.


## Sesión de reauditoría — 2026-09-05

Se retomó la revisión iniciada el 2026-09-04 tras interrupciones de ejecución.
Se conservaron resultados y base de código; el trabajo termina en diagnóstico y
planificación, sin correcciones de producto. Los casos aislados bajo `.local/`
permitieron detectar fallos que las suites existentes no ejercen en secuencia.

**Decisión:** expresar estado/revisión explícitos y verificar continuidad entre
operaciones en T6-01/T6-07. La alternativa de dar por válida la optimización porque
su prueba aislada pasa se descarta: JOIN → trazabilidad pierde filas activas.
**Decisión:** la aprobación de una conexión debe pertenecer al destino exacto
(T6-06), y el formato visible debe coincidir con el dialecto enviado (T6-02).
**Decisión:** mantener controles de memoria/cobertura; no borrar su incumplimiento.

Pendiente para la siguiente sesión: aprobación y ejecución de T6-01–T6-11,
aceptación jurídica T5-18/T5-20 y verificaciones externas expresamente enumeradas
en el informe. No crear CONTEXT.md ni tareas de CLI/workflows/Actions.

Mantenimiento: actualizar contexto en el mismo commit que el cambio; usar fechas
absolutas. Si una decisión cambia, marcarla superada con su razón sin borrarla.
El qué publicado pertenece al changelog; el porqué y lo aprendido, a este archivo.
