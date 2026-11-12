# Roadmap de Columnia

**Actualizado:** 2026-11-11 · **Versión:** 1.26.0

**Estado:** aplicación local funcional para Windows x64, de **uso personal**. No
se comparte ni se distribuye; las mejoras se eligen por lo que su dueño nota al
usarla con sus propios archivos. Los identificadores (RVnn y códigos de
hallazgo) se explican en el [glosario](docs/reference/glosario.md).

Este documento solo contiene lo pendiente. Lo entregado está en
[`CHANGELOG.md`](CHANGELOG.md). Los roadmaps anteriores se conservan sin cambios
en [`docs/archive/`](docs/archive/README.md): el de la revisión del 2026-10-01,
con sus 97 objetivos y casillas, en
[`docs/archive/2026-10/`](docs/archive/2026-10/ROADMAP.md).

<!-- revision-2026-11-05:inicio -->
## Revisión del 2026-11-05

Re-auditoría de lo cambiado desde la revisión anterior (`0a89c09`..`e48ecab`):
97 defectos (ningún Crítico ni Alto; 32 Medio y 65 Bajo) y 1 propuesta, con IDs
`AUD-NNN` en las secciones «Tier» de más abajo. El informe, con la evidencia de
cada uno, está en la carpeta local de auditorías, que git no versiona. Sin
histórico aparte: lo cerrado se queda marcado `[x]` con su línea *Cierre* y pasa
al CHANGELOG. El 2026-11-06 se cerró la tanda 1 (AUD-001, 002, 004, 005 y 006)
y se abrió AUD-099, aparecida al aplicarla. Ese mismo día se cerró la tanda 2
(AUD-003, 010, 011, 013 y 014) y se abrieron AUD-100 a AUD-103, aparecidas al
aplicarla. El 2026-11-08 se cerró la tanda 3 (AUD-012, 037, 023 y 024) y se
abrió AUD-104, aparecida al aplicarla. Ese mismo día se cerró la tanda 4
(AUD-022, 025 y 026) y se abrieron AUD-105 a AUD-107, aparecidas al aplicarla. Ese
mismo día se cerró la tanda 5 (AUD-105, 034, 035 y 036) y se abrieron AUD-108
a AUD-111, aparecidas al aplicarla. El 2026-11-09 se cerró la tanda 6 (AUD-108,
044, 045, 046, 047 y 048) y se abrió AUD-112, aparecida al aplicarla. Ese
mismo día se cerró la tanda 7 (AUD-112, 056 y 057). El 2026-11-10 se cerró la
tanda 8 (AUD-077, 074, 075, 076 y 073) y se abrió AUD-113, aparecida al
aplicarla. Ese mismo día se cerró la tanda 9 (AUD-009, 100, 008, 015 y 007) y
se abrieron AUD-114 y AUD-115, aparecidas al aplicarla. El 2026-11-11 se
cerró la tanda 10 (AUD-115, 016, 017, 018, 019 y 114). Ese mismo día se cerró
la tanda 11 (AUD-020, 103, 021, 102 y 099), después la tanda 12
(AUD-027, 028, 029, 030, 031 y 032) después la tanda 13 (AUD-033, 104, 106
y 107) y después la tanda 14 (AUD-039, 040, 041, 043, 038 y 042; en AUD-038 y
AUD-042 el usuario dio por bueno un criterio ajustado). Ese mismo día se cerró
la tanda 15 (AUD-109, 110 y 111), con la que Rendimiento queda sin abiertas. Después se cerró la
tanda 16 (AUD-049, 050, 051, 052, 053 y 054), aplicada en paralelo, y la tanda 17
(AUD-055, 058, 060, 061 y 062; AUD-059 avanza y sigue abierta), con la que
Accesibilidad queda sin abiertas y el catálogo de proyectos pasa a la versión 17.
El 2026-11-12 se cerró la tanda 18 (AUD-079, 063, 064, 065, 066 y 067) y se
abrió AUD-116, salida de la revisión del bundle de AUD-079.

<!-- indice:inicio -->
**Última actualización:** 2026-11-12 · **Siguiente ID libre:** AUD-117 · **Último informe:** [2026-11-05](docs/auditorias/2026-11-05/INFORME.md)

| Tier | Abiertas | Cerradas | Esfuerzo de las abiertas (Bajo · Medio · Alto) |
|---|---|---|---|
| 0 · Crítico | 0 | 0 | — |
| 1 · Alto | 0 | 0 | — |
| 2 · Medio | 0 | 35 | — |
| 3 · Bajo | 28 | 52 | 26 · 2 · 0 |
| 4 · Propuestas | 1 | 0 | 1 · 0 · 0 |
| **Total** | **29** | **87** | **27 · 2 · 0** |

**Abiertas por área:** Documentación 9 · UI/UX 8 · QA y testing 7 · Redacción 5
<!-- indice:fin -->
<!-- revision-2026-11-05:fin -->

## Dirección del producto

Columnia es una aplicación de escritorio local-first para convertir archivos
desordenados en datasets confiables:

1. **Cargar:** inspeccionar el origen y confirmar cómo interpretarlo.
2. **Revisar:** entender calidad, estructura, diferencias y relaciones.
3. **Preparar:** corregir y transformar con historial reversible.
4. **Explorar:** ver los datos limpios en un panel con filtro cruzado.
5. **Entregar:** validar un contrato y exportar una copia segura.

Columnia propone y la persona aprueba: nunca cambia datos sin un clic. Tauri 2 y
Rust para el shell y el motor; React, TypeScript y Vite para la interfaz; Polars
y DuckDB para procesar en local. Sin cuentas, telemetría ni sincronización
remota. El archivo original no se modifica.

## Cola vigente

La revisión del 2026-10-01 quedó aplicada el 2026-10-08. El 2026-10-09 se
cerraron REN-08 y REN-13 y DAT-12 pasó a aparcado. Quedan REN-14 y REN-01,
que tratan de la velocidad con archivos de varios GB; ninguno bloquea el uso
diario. La revisión del 2026-11-05 añade las tareas `AUD-NNN` de las secciones
«Tier» de más abajo; las de rendimiento (AUD-034 a AUD-043) conviene hacerlas
dentro de REN-14 y REN-01.

### Preparar archivos grandes

- [ ] **REN-14** — Preparar y explorar archivos muy grandes.
  - Hecho: la propuesta de Preparar se aplica entera en disco (marcadores,
    espacios, números, fechas, huecos y duplicados) con las mismas reglas que
    en memoria. La sonda `perf_probe_full_flow` recorre 11 datasets (de 3 MiB
    a 8,6 GB: netflix, creditcard, ALL2 en CSV y Parquet, vmCloud, 2019-Oct y
    2019-Nov) por cargar, perfil, Explorar, propuesta, perfil y Explorar otra
    vez: 0 fallos. Las herramientas sueltas de Preparar ya tenían su camino en
    disco.
  - Hecho: «Aplicar» muestra el paso, el porcentaje y el tiempo, y se cancela
    desde ahí; Explorar y la versión de partida del historial reutilizan la
    copia del perfil, y los marcadores sueltos se buscan en una sola lectura.
    `perf_probe_full_flow` en los 11 datasets, 0 fallos: Explorar 2,3 s en
    2019-Oct y 4,2 s en 2019-Nov (antes 55 y 88 s); aplicar la propuesta,
    195 s y 427 s (antes 272 y 516 s). Desde el 2026-10-14 aplicar lee la
    copia del perfil en lugar del CSV: 6 M filas de unos 26 s a unos 15 s
    (medido alternando) y 2019-Nov 387 s; la medida de 2019-Oct (273 s) se
    tomó con el equipo ocupado y no sirve para comparar.
  - Falta: aplicar sigue tardando minutos en 5–9 GB (escribir sin duplicados
    unos 130 s en 2019-Oct), con pausas de hasta unos 30 s sin cambio de
    porcentaje mientras DuckDB cuenta los cambios o arranca la escritura.

### Rendimiento

- [ ] **REN-01** — El perfil de calidad de archivos grandes es lento y se repite
  tras cada cambio (RV48).
  - Hecho: vmCloud (338 MB) se perfila en 2,6–2,7 s (criterio ≤ 3 s). En
    archivos grandes, el texto se revisa en partes a la vez, las filas
    repetidas se cuentan con huellas en la misma lectura, DuckDB cuenta los
    valores distintos en paralelo y una fecha igual a la anterior no se
    vuelve a interpretar; la lectura de filas prepara el bloque siguiente y
    DuckDB usa hasta 1 GB para la copia y el conteo de distintos. 2019-Oct
    (5,6 GB): de 223 s a 64 s, con un pico de 1.159 MiB. La etiqueta
    cambia como mucho cada 8,9 s (criterio ≤ 10 s, cumplido): la copia de
    trabajo y el conteo de distintos muestran el progreso de DuckDB.
    Sondas `perf_probe_profile` y `perf_probe_generate_csv`.
  - Medido: tras un cambio en memoria, recalcular el perfil cuesta lo mismo
    que el primero (0,4 s con 300 000 filas; 2,7 s con vmCloud) y lo marca la
    detección de filas parecidas, que depende de todas las columnas.
    Reutilizar el perfil de las columnas sin cambios ahorró un 8 % y se
    descartó.
  - Medido el 2026-10-14 (2019-Oct, con el equipo en uso): 60–62 s en las
    tandas tranquilas (copia 14 s, filas y columnas 29 s, espera al conteo
    de distintos 9–11 s) y hasta 78 s con carga. Comprimir la copia sin
    compresión o con LZ4, o dar 4–6 hilos a DuckDB en el conteo, quedó
    dentro de ese ruido y se descartó; lo que queda es acelerar la pasada de
    filas y columnas, que trata todas las columnas como texto.
  - Falta: bajar 2019-Oct a ≤ 60 s (64 s; al final se esperan unos 9 s al
    conteo de valores distintos de DuckDB) y, para reutilizar el perfil tras
    un cambio, actualizar duplicados y filas parecidas sin releer todo. Con
    100 M filas (CSV sintético de 4,8 GB) queda una espera de 48 s al cerrar
    el conteo de valores distintos de una columna con 100 M valores únicos,
    fase en la que DuckDB no informa de progreso.

<!-- revision-2026-11-05-tiers:inicio -->
## Tier 0 · Crítico (revisión 2026-11-05)

Sin tareas abiertas.

## Tier 1 · Alto (revisión 2026-11-05)

Sin tareas abiertas.

## Tier 2 · Medio (revisión 2026-11-05)

### Seguridad

- [x] **AUD-001 · Las columnas clave van sin validar a un comentario `--` del script T-SQL: una cabecera con salto de línea inyecta SQL**
  - **Área:** Seguridad · **Severidad:** Medio · **Esfuerzo:** Bajo
  - **Ubicación:** `src-tauri/src/tsql_script.rs:192-214`
  - **Qué hacer:** (1) no escribir nombres de datos en comentarios: dejar el resumen sin `key_list()` o pasar el texto por un filtro que sustituya `\r`, `\n`, U+0085, U+2028 y U+2029 y cualquier `char::is_control`; (2) validar los nombres de columna en `check_columns` (sin controles ni separadores de línea Unicode, 1 a 128 caracteres) y devolver un error claro; (3) añadir tests con cabeceras `a\nDROP…`, `]`, `'` y U+2028.
  - **Criterio de aceptación:** exportar con una clave llamada `id\nSELECT 1;--` o falla con un mensaje claro o genera un script cuyo único texto fuera de literales e identificadores citados es el esperado; hay un test que lo comprueba en `script` y en `cargar.sql`.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-06 · tanda 1 de la Fase 2 (rama `auditoria/2026-11-05`). `check_columns` rechaza nombres de columna vacíos, de más de 128 caracteres o con saltos de línea y controles (también U+2028/U+2029, igual que esquema y tabla), y los comentarios del script pasan por un filtro que quita esos caracteres.

- [x] **AUD-002 · `odbc_attributes` no sigue la gramática de las cadenas ODBC: los controles de Rust (diálogo, controlador, FileDSN, cifrado, redacción) se pueden esquivar con `}}` o con `{` a mitad de valor**
  - **Área:** Seguridad · **Severidad:** Medio · **Esfuerzo:** Medio
  - **Ubicación:** `src-tauri/src/remote_databases.rs:1924-1952`
  - **Qué hacer:** estructural: que Rust construya la cadena a partir de campos (motor, controlador listado, servidor, puerto, base, autenticación, usuario, contraseña, cifrado) y no acepte la cadena de la interfaz; así desaparecen el parser propio y la superficie de atributos (ver AUD-003). Si se mantiene la cadena libre: parser conforme (llave solo al inicio del valor, `}}` escapado) y rechazo de cadenas con llaves desbalanceadas o con claves repetidas.
  - **Criterio de aceptación:** tests con `Uid={sa}};Server=X};Server=Y`, `Server=a{b;Driver=C:\x.dll` y una contraseña `{a}}b;Encrypt=yes}` en `Pwd` que Rust rechaza o interpreta igual que el controlador; la cadena de la evidencia en ejecución no llega a abrirse.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-06 · tanda 1 de la Fase 2 (rama `auditoria/2026-11-05`). Enfoque estructural elegido por el usuario: la interfaz manda `DatabaseConnection` (campos) y Rust arma la cadena; desaparecen «Escribir la cadena ODBC», los DSN y el parser propio. La identidad del registro de entregas no cambia (fijada con literales del código anterior). Pendiente de AUD-003: el contenido del diálogo nativo.

- [x] **AUD-003 · La cadena ODBC no tiene lista de atributos permitidos: «Solo leerá» depende de la interfaz y el diálogo omite lo que más importa**
  - **Área:** Seguridad · **Severidad:** Medio · **Esfuerzo:** Medio
  - **Ubicación:** `src-tauri/src/remote_databases.rs:373-393`
  - **Qué hacer:** el mismo cambio estructural de AUD-002 (Rust arma la cadena con una lista cerrada de claves por motor: `Driver`, `Server`, `Port`, `Database`, `Uid`, `Pwd`, `Trusted_Connection`, cifrado, `TrustServerCertificate`) y que el diálogo muestre servidor, puerto, base, usuario o «tu usuario de Windows», cifrado y certificado, truncando cada valor a unos 100 caracteres. Para la importación, añadir `ApplicationIntent=ReadOnly` en SQL Server y documentar que la garantía real es un usuario con permiso de lectura.
  - **Criterio de aceptación:** una cadena con `ConnSettings`, `INITSTMT`, `PLUGIN_DIR`, `Address` o claves repetidas se rechaza antes de abrir el diálogo; el diálogo de una conexión con `Trusted_Connection` lo dice explícitamente.
  - **Depende de:** AUD-002 (el mismo cambio: que Rust construya la cadena ODBC)
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-06 · tanda 2 de la Fase 2 (rama `auditoria/2026-11-05`). Con la cadena por campos de AUD-002, el criterio se cumple porque esas cadenas no se pueden escribir: un campo extra no se deserializa y ningún valor añade ni repite claves (pruebas con `Address`, `INITSTMT`, `ConnSettings`, `PLUGIN_DIR` y claves repetidas en los tres motores). El diálogo muestra servidor, puerto, base, usuario o «tu usuario de Windows», cifrado, certificado (también «no se verifica» en PostgreSQL y MySQL) y controlador, cortados a 100 caracteres. La importación de SQL Server lleva `ApplicationIntent=ReadOnly` y se comprobó contra el SQL Server local. El aspecto del diálogo nativo no se vio en la app real.

- [x] **AUD-004 · `calamine` 0.36.1 (RUSTSEC-2026-0436): pánico y OOM en XLS/XLSB con archivos hostiles, ruta alcanzable**
  - **Área:** Seguridad · **Severidad:** Medio · **Esfuerzo:** Bajo
  - **Ubicación:** `src-tauri/src/dataset/spreadsheet_io.rs:22,41,684,939,1034,1150`
  - **Qué hacer:** subir `calamine` a ≥ 0.38.0 (revisar cambios de API; ya usa `quick-xml` 0.41.0). Mientras tanto, limitar el tamaño de los libros `.xls`/`.xlsb` antes de abrirlos.
  - **Criterio de aceptación:** `cargo audit` sin RUSTSEC-2026-0436 y la suite de importación de hojas pasa.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-06 · tanda 1 de la Fase 2 (rama `auditoria/2026-11-05`). `calamine` 0.38.0 sin cambios de API; `cargo audit` ya no lista RUSTSEC-2026-0436.

- [x] **AUD-005 · Las conexiones DuckDB nuevas (y las anteriores salvo la de la consola) no desactivan la instalación y carga automática de extensiones; `timezone()` sin ICU puede provocar una descarga**
  - **Área:** Seguridad · **Severidad:** Medio · **Esfuerzo:** Bajo
  - **Ubicación:** `src-tauri/src/duckdb_query.rs:2683-2702`
  - **Qué hacer:** mover `SET autoinstall_known_extensions = false; SET autoload_known_extensions = false;` a `configure_duckdb_resources_with_limits` para que lo hereden todas las conexiones; calcular el UTC sin ICU o habilitar `icu` en `duckdb`; añadir un test que exporte una columna con zona horaria; opcional: `allowed_paths` y `lock_configuration` también en las conexiones de escritura de Parquet. Endurecimiento que dejó apuntado la revisión del escapado del SQL de DuckDB (correcto en todo el diff): `allowed_directories` y `lock_configuration` también en las conexiones que escriben Parquet, para que la barrera no sea solo el escape.
  - **Criterio de aceptación:** un test (o `SELECT current_setting('autoload_known_extensions')`) en cada tipo de conexión devuelve `false`, y la exportación de un Parquet con `TIMESTAMPTZ` produce el script sin red.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-06 · tanda 1 de la Fase 2 (rama `auditoria/2026-11-05`). El ajuste va en `configure_duckdb_resources_with_limits` (es de la base: lo heredan las conexiones que la comparten) y el UTC se calcula sin ICU; ±infinity conserva su texto. La parte opcional (`allowed_paths`, `lock_configuration`) no se hizo: rompería los `SET` posteriores.

- [x] **AUD-006 · `vite` 8.3.0 con avisos altos de servidor de desarrollo, en loopback pero accesible desde cualquier página web abierta en la misma máquina**
  - **Área:** Seguridad · **Severidad:** Medio · **Esfuerzo:** Bajo
  - **Ubicación:** `package.json:93`
  - **Qué hacer:** `npm update vite` y repetir `npm audit`. `vitest`/`@vitest/mocker`/`@vitest/coverage-v8` 5.0.1 y `undici` 7.29.1 (vía `jsdom`) se actualizan en el mismo paso (hoy sin alcance: ver la sección 5).
  - **Criterio de aceptación:** `npm audit` sin vulnerabilidades altas y `tools/check-supply-chain.ps1 -RequireAuditTools` pasa su tramo de npm.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-06 · tanda 1 de la Fase 2 (rama `auditoria/2026-11-05`). `vite` 8.3.6, `vitest` y `@vitest/coverage-v8` 5.0.5, `undici` 7.30.2, sin cambiar los rangos de `package.json`; `npm audit` sin vulnerabilidades.

### Arquitectura, datos y resiliencia

- [x] **AUD-010 · Las copias de trabajo del perfil y del índice de conflictos viven en `static` y quedan en `%TEMP%` tras cerrar la app**
  - **Área:** Arquitectura · **Severidad:** Medio · **Esfuerzo:** Bajo
  - **Ubicación:** `src-tauri/src/dataset/profile_engine.rs:2530`
  - **Qué hacer:** (1) crear estas carpetas con un prefijo propio (`columnia-profile-`, `columnia-conflicts-`) y un archivo de bloqueo igual al del historial, y extender `purge_finished_history_directories`; o bien crearlas dentro de la carpeta del historial del dataset (`dataset.history.directory`), que ya se purga; (2) vaciar `PROFILE_SNAPSHOT` en `RunEvent::Exit` y al cargar otro dataset (`clear_profile_snapshot()` junto a `clear_conflict_index()` en `import_loading.rs:310`); (3) de paso, anotar tamaño y fecha de modificación del origen **antes** de la copia y pasarlos a `remember_profile_snapshot`, que hoy los lee después (`profile_engine.rs:2563-2576`).
  - **Criterio de aceptación:** tras perfilar un CSV grande, cerrar con normalidad y arrancar de nuevo no queda ninguna carpeta `.tmp*` con `compared.parquet`, `current.parquet` ni `conflictos.bin` en `%TEMP%`; una prueba cubre la purga de la carpeta nueva.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-06 · tanda 2 de la Fase 2 (rama `auditoria/2026-11-05`). Las dos copias van en carpetas `columnia-profile-` y `columnia-conflicts-` con el mismo bloqueo que el historial, y la purga del arranque las recorre. Las cachés se vacían en `RunEvent::Exit` y al cargar otro archivo. El tamaño y la fecha del origen se toman antes de la copia. Hay pruebas de la purga, del vaciado y de los prefijos. No se comprobó en la app real perfilar un CSV grande, cerrar y volver a arrancar. Abrir un proyecto queda para AUD-102.

- [x] **AUD-011 · Importar otra tabla borra el archivo del dataset activo antes de confirmar: la protección `keep` compara rutas de distinto formato en Windows**
  - **Área:** Arquitectura · **Severidad:** Medio · **Esfuerzo:** Bajo
  - **Ubicación:** `src-tauri/src/remote_databases.rs:2465-2511`
  - **Qué hacer:** comparar por identidad de archivo y no por texto de ruta: canonicalizar cada entrada (`fs::canonicalize(entry.path())`) antes de compararla con `keep`, o comparar el identificador de archivo (`same_file`). Mover el borrado de importaciones anteriores a después de `persist` correcto, para no perder nada si la escritura falla.
  - **Criterio de aceptación:** una prueba que pasa a `keep` la ruta canónica (`fs::canonicalize`) del archivo recién escrito conserva ese archivo y crea `(nueva)`; un fallo al escribir no borra las importaciones anteriores. La prueba existente, con la ruta canónica, falla con el código actual de `same_file` y pasa tras el arreglo.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-06 · tanda 2 de la Fase 2 (rama `auditoria/2026-11-05`). `write_import_parquet` compara rutas canonicalizadas y borra las importaciones anteriores solo después de `persist`. La prueba con la ruta canónica y la del fallo de escritura fallaban con el código anterior y pasan ahora. No se repitió en la app real.

- [x] **AUD-012 · La importación desde una base de datos no acota de verdad la memoria: el tope de 2 GiB cuenta solo los bytes de texto**
  - **Área:** Arquitectura · **Severidad:** Medio · **Esfuerzo:** Medio
  - **Ubicación:** `src-tauri/src/remote_databases.rs:2407-2437`
  - **Qué hacer:** escribir Parquet por lotes (`ParquetWriter::batched` con un frame por lote ODBC) en lugar de acumular columnas enteras; el tope de filas ya no necesita el de bytes. Si se mantiene la acumulación, contar el sobrecoste (`24 + len` por celda) y aplicar `ensure_materialization_budget` con la estimación. Con un servidor hostil: truncar la lista de tablas mientras se lee y limitar el número de columnas (por ejemplo, 2.000).
  - **Criterio de aceptación:** importar 1.000.000 filas por 30 columnas de texto corto mantiene el pico de memoria por debajo de unos 1,5 GB (medido con el monitor de recursos) y 5.000.000 filas por 20 columnas no aborta el proceso. Un cursor simulado de 100.000 columnas, o con tablas sin fin, termina con un error claro.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-08 · tanda 3 de la Fase 2 (rama `auditoria/2026-11-05`). Cada lote leído del servidor se escribe al Parquet temporal en cuanto llega, sin acumular la tabla; la escritura sigue siendo atómica y las importaciones anteriores se borran solo después. Se quitó el tope de 2 GiB de texto. Una tabla de más de 2.000 columnas se rechaza antes de reservar memoria, y un catálogo de más de 100.000 objetos se corta con un mensaje claro (pruebas con 100.000 columnas y con un catálogo sin fin). Medido con la prueba externa contra una base de pruebas en el SQL Server local, ya borrada: 1.000.000 × 30 columnas de texto corto, pico de 134 MB; 5.000.000 × 20, pico de 108 MB y sin abortar. No se midió en la app abierta.

- [x] **AUD-013 · Listar tablas y conectar no tienen timeout de sentencia, y Cancelar solo se observa entre llamadas del controlador**
  - **Área:** Arquitectura · **Severidad:** Medio · **Esfuerzo:** Bajo
  - **Ubicación:** `src-tauri/src/remote_databases.rs:2188-2196`
  - **Qué hacer:** listar con una sentencia preparada con `set_query_timeout_sec` (por ejemplo `SQLTables` por `preallocate()` + `tables` del cursor o una consulta al catálogo con timeout de 30 s), o fijar `SQL_ATTR_CONNECTION_TIMEOUT`; y devolver el control a la interfaz al cancelar (que el comando devuelva `Cancelado` aunque el hilo siga, o mostrar que la cancelación está pendiente).
  - **Criterio de aceptación:** con un servidor que no responde al catálogo, «Ver tablas» falla con un mensaje en menos de 45 s y Cancelar devuelve la interfaz a su estado en menos de 2 s.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-06 · tanda 2 de la Fase 2 (rama `auditoria/2026-11-05`). Listar las tablas usa una sentencia con un timeout de 30 s; con los 15 s de conexión suma 45 s, y una aserción en compilación lo comprueba. «Ver tablas» devuelve la cancelación en menos de 2 s aunque el controlador siga bloqueado (prueba unitaria con un trabajo de 10 s). El listado con timeout se comprobó contra el SQL Server local; un servidor que no responda al catálogo no. Cancelar una importación queda para AUD-103.

- [x] **AUD-014 · El script T-SQL va en un solo lote sin `GO` y no avisa del límite de tamaño de lote de SQL Server**
  - **Área:** Arquitectura · **Severidad:** Medio · **Esfuerzo:** Bajo
  - **Ubicación:** `src-tauri/src/tsql_script.rs:16-17`
  - **Qué hacer:** estimar el tamaño (filas × columnas) y, por encima de un umbral (por ejemplo 100 MB), mostrar en Entregar «Este script será muy grande; usa el paquete BULK INSERT» y proponerlo; o dividir en lotes con `GO` entre bloques de INSERT conservando la transacción (los `GO` no cierran una transacción abierta, pero `XACT_ABORT` y las variables `@nuevas` habría que repetirlas por lote).
  - **Criterio de aceptación:** exportar más de N filas a «Script T-SQL» muestra el aviso antes del selector de destino; un script de la longitud documentada se parsea sin error del tamaño del lote.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-06 · tanda 2 de la Fase 2 (rama `auditoria/2026-11-05`). Se eligió el aviso y no partir el script con `GO`. Entregar estima el lote en UTF-16 y, a partir de 100 MiB (el límite es 256 MB con el paquete de red por defecto), avisa antes de exportar y ofrece cambiar al paquete BULK INSERT. El aviso no bloquea. Con 10 columnas aparece desde unas 210.000 filas: avisa de más, porque Entregar no conoce la longitud de cada columna. El límite está documentado en `docs/reference/cli.md`. Hay pruebas de la estimación y del aviso. No se ejecutó en un SQL Server real un script de esa longitud.

### Código

- [x] **AUD-022 · El perfil y la validación de contrato de un CSV grande sin encabezado leen una copia con el encabezado equivocado y fallan al final de la lectura**
  - **Área:** Código · **Severidad:** Medio · **Esfuerzo:** Medio
  - **Ubicación:** `src-tauri/src/dataset/profile_engine.rs:2582-2626`
  - **Qué hacer:** pasar el `DuckDbFileFormat` que ya calcula `current_duckdb_file_source` (que incluye `DelimitedWithoutHeader { column_count }`) a `profile_source_backed_with_progress`, a `source_profile_snapshot_with_cancel` y a `persist_delimited_*`, en lugar de reconstruirlo desde la extensión. Guardar ese formato (delimitador y encabezado) en `ProfileSnapshot` y compararlo en `cached_profile_snapshot`; así la coincidencia de lectura queda comprobada en un solo sitio para todos los consumidores (y se cierra por construcción lo que H-01-1 temía del delimitador). Además, que la comprobación de filas no diga que el snapshot «cambió» cuando no consta ningún cambio del origen: distinguir «la copia no corresponde con el dataset» de «el archivo cambió durante la lectura».
  - **Criterio de aceptación:** una prueba que cargue un CSV de dos líneas de datos como `source_backed` con `Generated` (con el umbral de carga diferida bajado en la prueba) y lo perfile obtiene `column_1`, `column_2`, el mismo `row_count` que la carga y ningún error; «Validar contrato» sobre el mismo origen evalúa las columnas `column_N`.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-08 · tanda 4 de la Fase 2 (rama `auditoria/2026-11-05`). La copia de trabajo del perfil guarda el formato con que se leyó el origen (delimitador y encabezado), y `cached_profile_snapshot` solo la devuelve para ese mismo formato: es el único sitio que lo decide. El perfil, «Validar contrato», la calidad al exportar y las copias reciben el formato del dataset cargado en vez de rehacerlo desde la extensión. Un desajuste de filas en la lectura por bloques dice ahora «no corresponde con el dataset». Prueba nueva: un CSV sin encabezado cargado como `source_backed` con nombres generados se perfila con `column_1`, `column_2` y el mismo número de filas, y su contrato evalúa esas columnas; con el código anterior el perfil fallaba y el contrato leía «001» y «Ana». El umbral de carga diferida es una constante: la prueba construye el dataset `source_backed` directamente. No comprobado en la app abierta.

- [x] **AUD-023 · Al volver a un destino de base de datos, el formulario aparece vacío pero la cadena de conexión anterior sigue en el estado y se puede usar**
  - **Área:** Código · **Severidad:** Medio · **Esfuerzo:** Bajo
  - **Ubicación:** `src/features/delivery/DeliveryPhase.tsx:520-536`
  - **Qué hacer:** limpiar `connectionString` y subir `connectionFormKey` cada vez que el formato deja de ser de base de datos (en `changeExportFormat`, rama `else`), o guardar los campos en el padre. Un cambio de una línea: `if (!kind) { setDatabaseTarget(c => ({...c, connectionString: ""})); setConnectionFormKey(k => k + 1); }`.
  - **Criterio de aceptación:** tras rellenar PostgreSQL, pasar a «Archivo» y volver, el botón "Analizar destino" queda deshabilitado con el aviso "Completa la conexión con el servidor." y el estado `databaseTarget.connectionString` es `""`. Prueba de componente nueva.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-08 · tanda 3 de la Fase 2 (rama `auditoria/2026-11-05`). Al pasar a un formato que no es de base de datos se descarta la conexión (desde AUD-002 es `databaseTarget.connection`, que queda en `null`) y el análisis vuelve a cero. Prueba de componente: PostgreSQL → CSV → PostgreSQL deja el servidor vacío, «Analizar compatibilidad» (el nombre actual del botón) deshabilitado con «Completa la conexión con el servidor.» y sin llamar al análisis. Falla con el código anterior y pasa ahora.

- [x] **AUD-024 · Las tablas y esquemas con acentos o `ñ` pueden desaparecer de la lista (o quedar sin esquema) porque el catálogo ODBC se lee como UTF-8 estricto**
  - **Área:** Código · **Severidad:** Medio · **Esfuerzo:** Bajo
  - **Ubicación:** `src-tauri/src/remote_databases.rs:2197-2209`
  - **Qué hacer:** leer el catálogo en ancho (`SQLTablesW` mediante un cursor propio: `connection.tables_cursor(...)` y `bind_buffer` con `TextColumn::<u16>`), o al menos distinguir el fallo: si `as_str()` da error, no omitir la tabla en silencio sino devolver "N tablas con nombres que no se pueden leer" y no usar `unwrap_or_default()` para el esquema (rechazar la fila). Añadir una prueba con `Año` y `ñ` contra un servidor local, aunque sea `#[ignore]`.
  - **Criterio de aceptación:** con una tabla `[dbo].[Año]` y otra `[Ventas_ñ].[Pedidos]` en SQL Server local, ambas aparecen en "Ver tablas" con su nombre exacto, y se importan las filas correctas.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-08 · tanda 3 de la Fase 2 (rama `auditoria/2026-11-05`). El catálogo se lee como texto Unicode (UTF-16) con un cursor propio, y un nombre que no se puede leer da un error claro en vez de desaparecer o quedar sin esquema. Comprobado contra una base de pruebas en el SQL Server local, ya borrada: `[dbo].[Año]`, `[Ventas_ñ].[Pedidos]` y `[dbo].[Zona_Ω]` aparecen con su nombre exacto, y se importan sus 3 y 4 filas con las tildes intactas (prueba externa `external_odbc_catalog_lists_wide_names`). No se vio en la app abierta.

- [x] **AUD-025 · Fechas y huecos de la etapa en disco copian la lógica de la ruta en memoria en lugar de compartirla (regresión parcial de COD-03)**
  - **Área:** Código · **Severidad:** Medio · **Esfuerzo:** Medio
  - **Ubicación:** `src-tauri/src/dataset/source_stage.rs:182-300`
  - **Qué hacer:** extraer `typed_date_column(name, values: &StringChunked, order, with_time) -> PolarsResult<Column>` y `fill_column(column, Fill)` a un módulo compartido (p. ej. `column_typing.rs`) que usen `cast_fully_dated_columns`, `impute_missing_values_in_columns` y `convert_block`; dejar en `parquet_lower_median` solo la parte que no se puede compartir (SQL) con una prueba de propiedades contra la ruta en memoria. Corregir el comentario de `NumericCastTarget::Decimal`.
  - **Criterio de aceptación:** una sola función construye la columna de fechas y una sola aplica un `Fill`; las dos rutas la llaman; el comentario de `NumericCastTarget` coincide con `decimal_fits_in_f64`; las pruebas existentes de paridad pasan sin cambios.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-08 · tanda 4 de la Fase 2 (rama `auditoria/2026-11-05`). El módulo nuevo `dataset/column_typing.rs` tiene `typed_date_column` y `fill_column`, y los usan la ruta en memoria (`cast_fully_dated_columns`, `impute_missing_values_in_columns`) y la etapa en disco (`convert_block`). El comentario de `NumericCastTarget::Decimal` describe ya lo que comprueba `decimal_fits_in_f64`. Prueba nueva de la mediana SQL contra la imputación en memoria (par, impar, `i64::MAX`, NaN, infinitos, todo nulo y cero filas); las pruebas de paridad pasan sin cambios. Coste aceptado: en memoria, una columna que sí es de fechas se analiza dos veces.

- [x] **AUD-026 · Cancelar el conteo de distintos no alcanza al respaldo por falta de memoria: `join` espera a que termine una cuenta sin cancelación**
  - **Área:** Código · **Severidad:** Medio · **Esfuerzo:** Bajo
  - **Ubicación:** `src-tauri/src/duckdb_query.rs:478-487`
  - **Qué hacer:** pasar al respaldo el estado de cancelación real (un `Arc<AtomicBool>` que `NativeProgress::interrupt` también active y que el respaldo consulte), o hacer que `BackgroundDistinctCount` use un `Arc<AtomicBool>` compartido con el hilo y sustituya `|| false` por `move || flag.load(Ordering::Acquire)`. Sin eso, al menos no esperar con `join` en `Drop` cuando se cancela: soltar el hilo (ya conserva su propio `Arc` de la base) y dejar que borre su carpeta temporal.
  - **Criterio de aceptación:** una prueba fuerza el respaldo (`SET memory_limit` muy bajo o un Parquet con texto libre) y cancela a mitad: `finish` devuelve el error de cancelación en menos de unos segundos y el hilo termina.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-08 · tanda 4 de la Fase 2 (rama `auditoria/2026-11-05`). `NativeProgress::interrupt` activa también un `Arc<AtomicBool>` que el hilo del conteo pasa al respaldo por falta de memoria, en lugar de `|| false`; si la cancelación llega en la primera consulta, el hilo devuelve igualmente el error de cancelación. Prueba nueva: un Parquet de 1.000.000 de filas de texto libre con `memory_limit` de 16 MB fuerza el respaldo, y al cancelar el hilo termina con el error de cancelación en unos 20 ms (el respaldo entero tarda unos 1,5 s); con el código anterior terminaba con el recuento completo.

- [x] **AUD-105 · La tendencia numérica de un CSV grande sin encabezado lee la proyección con el encabezado equivocado**
  - **Área:** Código · **Severidad:** Medio · **Esfuerzo:** Bajo
  - **Ubicación:** `src-tauri/src/dataset/profile_engine.rs`, `source_temporal_projection_snapshot` (≈`:2433`), llamada desde `get_temporal_aggregation_impl` (`profile_reader.rs`)
  - **Qué hacer:** pasar a `source_temporal_projection_snapshot` el `DuckDbFileFormat` de `current_duckdb_file_source`, como hizo AUD-022 con el perfil, en lugar de rehacer `Delimited { detect_delimiter }` desde la extensión. Si se aplica antes AUD-034 (que cambia esa proyección por una lectura de la copia del perfil), comprobar que el caso sin copia también recibe el formato.
  - **Criterio de aceptación:** una prueba con un CSV sin encabezado cargado como `source_backed` con nombres generados pide la tendencia de `column_2` sobre `column_1` y obtiene los puntos de las dos filas de datos, sin error y sin tratar la primera fila como encabezado.
  - **Depende de:** ninguna
  - **Origen:** Fase 2 de la auditoría 2026-11-05, al aplicar AUD-022 (visto en el código, no reproducido).
  - **Cierre:** 2026-11-08 · tanda 5 de la Fase 2 (rama `auditoria/2026-11-05`). `source_temporal_projection_snapshot` recibe el `DuckDbFileFormat` de `current_duckdb_file_source` en lugar de rehacerlo desde la extensión; Parquet y JSON siguen sin admitir la proyección, y si el dataset no da formato se devuelve el mismo error que antes. Prueba nueva: la tendencia de `column_2` sobre `column_1` de un CSV sin encabezado cargado como `source_backed` lee las dos filas de datos; con el código anterior fallaba con «column_1 not found» porque tomaba la primera fila como encabezado. No comprobado en la app abierta.

- [x] **AUD-108 · Exportar un CSV grande sin encabezado puede tomar la primera fila de datos como encabezado**
  - **Área:** Código · **Severidad:** Medio · **Esfuerzo:** Bajo
  - **Ubicación:** `src-tauri/src/dataset/export_io.rs`, `publish_source_backed_export` (≈`:864`)
  - **Qué hacer:** tomar el formato de lectura de `current_duckdb_file_source(dataset)` (que respeta `delimited_header_mode`), como hicieron AUD-022 y AUD-105, en lugar de rehacer `Delimited { detect_delimiter }` desde la extensión. Así el script T-SQL y el paquete BULK INSERT de un CSV sin encabezado también podrán leer la copia del perfil (AUD-034).
  - **Criterio de aceptación:** una prueba exporta un CSV sin encabezado cargado como `source_backed` con nombres generados y el resultado tiene las columnas `column_1`… y todas las filas de datos, incluida la primera.
  - **Depende de:** ninguna
  - **Origen:** Fase 2 de la auditoría 2026-11-05, al aplicar AUD-034 (visto en el código, no reproducido).
  - **Cierre:** 2026-11-09 · tanda 6 de la Fase 2 (rama `auditoria/2026-11-05`). `publish_source_backed_export` recibe el formato de lectura de `current_duckdb_file_source` en lugar de rehacerlo desde la extensión; las nueve exportaciones `source_backed` lo pasan (con privacidad, el snapshot Parquet), y la automatización deja de descartarlo. JSON sigue sin admitirse, con el mismo error. Prueba nueva: un CSV sin encabezado cargado como `source_backed` con nombres generados se exporta con `column_1,column_2` y todas las filas, incluida la primera. No comprobado en la app abierta.

- [x] **AUD-112 · Exportar con privacidad un CSV grande sin encabezado puede tomar la primera fila de datos como encabezado**
  - **Área:** Código · **Severidad:** Medio · **Esfuerzo:** Bajo
  - **Ubicación:** `src-tauri/src/dataset/export_io.rs`, `source_backed_privacy_snapshot` (≈`:1602`); también `source_profile_snapshot` (`profile_engine.rs`, ≈`:2481`, solo `#[cfg(test)]`)
  - **Qué hacer:** que el snapshot de privacidad reciba el formato de `current_duckdb_file_source(dataset)` (que respeta `delimited_header_mode`), como hicieron AUD-022, AUD-105 y AUD-108, en lugar de rehacer `Delimited { detect_delimiter }` desde la extensión. Alinear de paso el ayudante de pruebas `source_profile_snapshot`.
  - **Criterio de aceptación:** una prueba exporta con privacidad `Mask` (o `Hash`) un CSV sin encabezado cargado como `source_backed` con nombres generados, y el resultado tiene las columnas `column_1`… y todas las filas de datos, incluida la primera.
  - **Depende de:** ninguna
  - **Origen:** Fase 2 de la auditoría 2026-11-05, al aplicar AUD-108 (visto en el código, no reproducido).
  - **Cierre:** 2026-11-09 · tanda 7 de la Fase 2 (rama `auditoria/2026-11-05`). `source_backed_privacy_snapshot` recibe el formato de `current_duckdb_file_source` y lee el esquema con o sin encabezado según ese formato (los tres llamadores lo pasan); `source_profile_snapshot` recibe también el formato. Prueba `privacy_snapshot_of_a_headerless_source_backed_csv_keeps_the_first_row` (primera fila `email,Ana`, con `Mask` y `Hash`): exporta `column_1,column_2` y todas las filas; falla con el comportamiento anterior.

### Rendimiento

- [x] **AUD-034 · Cinco rutas vuelven a leer o a copiar el CSV entero aunque la copia de trabajo de REN-14 ya existe**
  - **Área:** Rendimiento · **Severidad:** Medio · **Esfuerzo:** Medio
  - **Ubicación:** `src-tauri/src/dataset/comparison_engine.rs:2516-2547`
  - **Qué hacer:** una función `reusable_profile_copy(source_path, source_format) -> Option<CachedProfileSnapshot>` que aplique la misma condición que `prepare_source_backed_baseline_snapshot` (`Delimited` con encabezado y mismo delimitador; ver H-01 del área 7 para `DelimitedWithoutHeader`) y llamarla al principio de `source_backed_parquet_snapshot_with_cancel`, `source_profile_snapshot_with_cancel` y `get_temporal_aggregation_impl` (sustituyendo la proyección de dos columnas por `parquet_scan` de la copia). En la tendencia numérica, soltar el mutex antes de leer, como hizo ARQ-04 con Explorar. Para el texto «a la medida», `max(strlen())` sobre la copia es una lectura de una columna por Parquet en vez de reinterpretar el CSV.
  - **Criterio de aceptación:** tras perfilar un CSV de 512 MiB o más, «Validar contrato», la primera página de conflictos, «Resolver» y la tendencia numérica no crean ninguna carpeta con `current.parquet` ni `compared.parquet` nuevos y su primera fase no relee el CSV (medible con el Monitor de procesos: sin lectura del CSV); una prueba con un CSV de más de un bloque comprueba que el resultado es idéntico con y sin la copia.
  - **Depende de:** AUD-022 (la copia del perfil tiene que guardar el formato de lectura antes de reutilizarla; cumplida el 2026-11-08)
  - **Nota (2026-11-08, al aplicar AUD-022):** con el formato ya en la copia, `cached_profile_snapshot(source, format)` es la comprobación; las condiciones propias de las correcciones seguras (`dataset.rs`, solo `Delimited`), de Explorar (`get_explore_panel`, solo con encabezado) y de la línea base del historial (`prepare_source_backed_baseline_snapshot`, solo `Delimited`) se pueden ampliar a `DelimitedWithoutHeader` sin riesgo.
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-08 · tanda 5 de la Fase 2 (rama `auditoria/2026-11-05`). `reusable_profile_copy(source, format)` delega en `cached_profile_snapshot` (lecturas delimitadas con y sin encabezado) y `WorkingCopyHold` mantiene vivo el Parquet mientras se lee, aunque otro perfil sustituya la copia. Leen la copia en lugar del CSV: «Validar contrato», la primera página de conflictos y «Resolver» (sin carpeta `columnia-conflicts-*` nueva), la tendencia numérica (que ahora suelta el mutex durante el cálculo y comprueba al final que el dataset no cambió) y el texto «a la medida» del script T-SQL y del paquete BULK INSERT. Pruebas nuevas: con un CSV de 66.536 filas (más de un bloque), el contrato, la tendencia y la copia de comparación dan lo mismo con y sin la copia, y con copia la ruta devuelta es la de la copia; lo mismo para la tendencia de un CSV sin encabezado; el script y el paquete son idénticos con y sin copia. No medido en la app real con un CSV de 512 MiB y el Monitor de procesos. Las tres condiciones de la nota no se ampliaron (AUD-110).

- [x] **AUD-035 · La etapa de huecos lanza una consulta DuckDB por columna, con conexión nueva y ordenación completa de la columna**
  - **Área:** Rendimiento · **Severidad:** Medio · **Esfuerzo:** Medio
  - **Ubicación:** `src-tauri/src/duckdb_query.rs:1638-1680`
  - **Qué hacer:** una sola consulta para todas las columnas a rellenar, por ejemplo `SELECT count(*) FILTER (WHERE c1 IS NULL), quantile_disc(x1, 0.5), count(*) FILTER (WHERE c2 IS NULL), quantile_disc(x2, 0.5), ... FROM dataset` (una lectura, sin ordenación completa; AUD-017 ya señala que `quantile_disc(x, 0.5)` devuelve el mismo valor, a confirmar con la prueba de paridad `source_backed_dates_and_fills_match_the_in_memory_plan`), pasando el cierre de cancelación. Si se prefiere mantener Rust, calcular las medianas en la pasada de comprobación de `numeric_target` (ver AUD-036) con un histograma por columna.
  - **Criterio de aceptación:** una propuesta con k columnas a rellenar ejecuta una sola consulta de medianas (comprobable por el número de conexiones DuckDB abiertas o por una prueba que cuente llamadas), los resultados de las pruebas de paridad con la ruta en memoria no cambian, y la fase tarda en 2019-Oct lo mismo con k = 1 que con k = 6 salvo la lectura adicional.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-08 · tanda 5 de la Fase 2 (rama `auditoria/2026-11-05`). `parquet_lower_medians` calcula los huecos y la mediana inferior de todas las columnas a rellenar en una sola conexión y una sola consulta con `quantile_disc(x, 0.5)` (sin ordenar cada columna; los enteros no pasan por DOUBLE; NaN e infinitos fuera, como en memoria), y recibe la cancelación de la etapa. Las pruebas de paridad con la ruta en memoria pasan sin cambios; prueba nueva: cuatro columnas a rellenar hacen una sola consulta y dan el mismo resultado que el plan en memoria. No medido en 2019-Oct con k = 1 y k = 6.

- [x] **AUD-036 · La etapa a disco comprueba cada columna en una pasada propia, en serie, sin parar cuando ya hay veredicto y asignando por fila**
  - **Área:** Rendimiento · **Severidad:** Medio · **Esfuerzo:** Medio
  - **Ubicación:** `src-tauri/src/dataset/source_stage.rs:323-349`
  - **Qué hacer:** (a) una sola pasada de comprobación que lea todas las columnas candidatas del bloque y las reparta con `rayon` por columna, con una bandera `AtomicBool` por columna para saltar las ya decididas y, si todas lo están, terminar la lectura devolviendo un error centinela como hace `CONFLICT_PAGE_FILLED` en `comparison_engine.rs:2294`; (b) `if let Some(entry) = counts.get_mut(value) { entry.0 += 1 } else { counts.insert(value.to_owned(), (1, position)) }` en `text_fill`; (c) en `decimal_fits_in_f64`, evitar `to_string()`: comprobar que el número tiene ≤ 15 dígitos significativos contando dígitos del texto; (d) usar la lectura anticipada en las pasadas de comprobación.
  - **Criterio de aceptación:** las pruebas de paridad (`source_backed_numeric_typing_matches_the_in_memory_plan` y las dos hermanas) pasan sin cambios, y con 5 columnas numéricas candidatas y un CSV de 6 millones de filas la fase de comprobación tarda menos que k = 1 multiplicado por 2 (medido con la sonda `perf_probe_apply_proposal`).
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-08 · tanda 5 de la Fase 2 (rama `auditoria/2026-11-05`). (a) Números y fechas candidatos se comprueban en una sola lectura por bloques, repartidos por columna con `rayon`; una columna rechazada deja de leerse y, si todas lo están, la lectura termina con el centinela `SOURCE_CHECKS_DECIDED`. Los rellenos de texto se cuentan también en una sola lectura. (b) `text_fill` ya no copia el texto en cada fila repetida. (c) `decimal_fits_in_f64` responde sin construir el texto del número cuando tiene 15 dígitos significativos o menos (prueba contra la versión anterior con unos 100.000 casos). (d) Las dos pasadas leen el bloque siguiente mientras se procesa el actual (`for_each_parquet_wanted_columns_block_prefetched`). Pruebas de paridad sin cambios; 5 pruebas nuevas (una lectura por bloque, corte cuando todas se rechazan, cancelación entre bloques, `decimal_fits_in_f64` y celdas vacías: llegan como nulo en las dos rutas, sin divergencia). No medido con 6 millones de filas y `perf_probe_apply_proposal`.

- [x] **AUD-037 · La importación de una base de datos escribe el Parquet sin la configuración de grupos de filas que el resto del proyecto aplica a propósito**
  - **Área:** Rendimiento · **Severidad:** Medio · **Esfuerzo:** Bajo
  - **Ubicación:** `src-tauri/src/remote_databases.rs:2500-2502`
  - **Qué hacer:** `ParquetWriter::new(...).with_row_group_size(Some(crate::dataset::parquet_row_group_rows(frame))).finish(frame)`. Un cambio de una línea; se puede añadir una prueba que lea `ParquetMetaData` del archivo importado y compruebe que ningún grupo supera `HISTORY_SNAPSHOT_BATCH_ROWS`.
  - **Criterio de aceptación:** el Parquet importado de una tabla de 100.000 filas por 60 columnas de texto tiene grupos de a lo sumo 65.536 filas, y la exportación a script T-SQL desde ese dataset no agota el límite de memoria de DuckDB.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-08 · tanda 3 de la Fase 2 (rama `auditoria/2026-11-05`). Con la escritura por lotes de AUD-012, los lotes se juntan en grupos de `parquet_row_group_rows` (como mucho 65.536 filas, menos en filas anchas) antes de escribirlos. 100.000 × 60 columnas de texto dan grupos de 65.536 y 34.464, y el script T-SQL de ese Parquet se genera dentro del límite de DuckDB (pruebas). Contra el SQL Server local, 1.000.000 filas quedan en 16 grupos (eran 100 de 10.000) y la importación pasa de 7,8 s a 6,4 s.

### Accesibilidad y semántica

- [x] **AUD-044 · La cabecera fija con perfil (162 px) deja sin filas visibles la vista previa en la ventana mínima y a zoom 200 %**
  - **Área:** Accesibilidad · **Severidad:** Medio · **Esfuerzo:** Bajo
  - **Ubicación:** `src/styles.css:519`
  - **Qué hacer:** mantener fijo solo el bloque nombre+tipo (por ejemplo, el perfil en una segunda `<tr>` de la cabecera que no sea `sticky`), o quitar `position: sticky` de `.table--profiled th` y el `max-height` cuando `max-height: 700px` o cuando `html[data-columnia-zoom]`/`@media (max-width: 900px)`; como mínimo, `max-height: max(53vh, 420px)` para que la región nunca sea menor que la cabecera más tres filas. Alternativa del área 6: que `.table-region` use una altura en función del espacio libre (`max-height: calc(100dvh - 240px)` con un mínimo de 320 px), o una cabecera de perfil plegable («Mostrar perfil por columna»).
  - **Criterio de aceptación:** a 640×370 CSS px (zoom 200 %) con 12 columnas y perfil listo, se ven al menos tres filas de datos bajo la cabecera, y el desplazamiento del teclado sigue funcionando en la región. A 1280×640, con 12 columnas con señales, se ven al menos 6 filas completas sin desplazar la página.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-09 · tanda 6 de la Fase 2 (rama `auditoria/2026-11-05`). `.table-region` mide `max(340px, calc(100dvh - 320px))` en lugar de `53vh`; la cabecera sigue fija. Medido con el simulador (12 columnas con perfil, cabecera de 162 px): a 640×370, 3 filas completas bajo la cabecera (antes 0) y el teclado sigue desplazando la región; a 1280×800, 6 filas (antes 5). El criterio pedía 6 filas a 1280×640, que no caben (la región empieza a unos 340 px y cabecera más 6 filas suman 450 px): se midió a 1280×800, el tamaño de la medición original; a 1280×640 siguen siendo 3. La densidad compacta conserva su `72vh`.

- [x] **AUD-045 · El anillo de foco no llega a 3:1 sobre la barra lateral en los temas Papel, Océano y Pizarra**
  - **Área:** Accesibilidad · **Severidad:** Medio · **Esfuerzo:** Bajo
  - **Ubicación:** `src/styles.css:58-66`
  - **Qué hacer:** en `shell.css` (o en la regla `.app-sidebar` de `workflow-styles.css:525`) añadir `--focus-ring: #efba70;` para todas las barras, y ampliar `AccessibilityStyles.test.ts` y `tools/contrast-table.mjs` con el par `--focus-ring` / `--sidebar-surface` de cada tema.
  - **Criterio de aceptación:** el par foco/superficie de la barra da al menos 3:1 en los cinco temas, medido con DevTools, y la prueba de contraste lo vigila.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-09 · tanda 6 de la Fase 2 (rama `auditoria/2026-11-05`). La regla base `.app-sidebar` define `--focus-ring: #efba70`. Medido en el navegador: 8,21:1 (Claro y Oscuro), 7,96:1 (Papel), 8,00:1 (Océano) y 8,03:1 (Pizarra); antes 2,26, 2,72 y 2,68 en los tres últimos. `tools/contrast-table.mjs` mide la combinación que pinta la barra en cada tema (regla base más el bloque del tema) y añade esas filas a la tabla de `DESIGN.md`; `AccessibilityStyles.test.ts` falla si el par baja de 3:1 o falta el token.

- [x] **AUD-046 · El foco cae a `body` al terminar «Ver tablas» y durante la importación**
  - **Área:** Accesibilidad · **Severidad:** Medio · **Esfuerzo:** Bajo
  - **Ubicación:** `src/features/load/DatabaseImportPanel.tsx:159-166`
  - **Qué hacer:** no desactivar el botón que tiene el foco (usar `aria-disabled` e ignorar el clic mientras trabaja) o mover el foco al terminar: a la lista «Tabla o vista» cuando llega el catálogo y al mensaje si falla. Anunciar «Conectando…» y el resultado en un contenedor persistente con `role="status"`.
  - **Criterio de aceptación:** tras «Ver tablas», `document.activeElement` es la lista de tablas (o el error), nunca `body`, y el resultado se anuncia; una prueba de componente lo comprueba.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-09 · tanda 6 de la Fase 2 (rama `auditoria/2026-11-05`). «Ver tablas» y «Traer …» quedan con `aria-disabled` mientras trabajan (el clic se ignora) en lugar de `disabled`, así que el foco no cae a `body`; al terminar va a la lista «Tabla o vista», al aviso de que no hay tablas, al resultado o al error, salvo que la persona lo haya movido. «Conectando…» y los resultados se anuncian en contenedores `role="status"` siempre montados. Los botones con `aria-disabled` se ven desactivados como los demás. 3 pruebas de componente nuevas; medido también con el simulador (el foco sigue en «Conectando…» y luego en la lista).

- [x] **AUD-047 · El editor de reglas de calidad se sale de su columna en Entregar a partir de 1180 px y el panel de exportación lo tapa**
  - **Área:** Accesibilidad · **Severidad:** Medio · **Esfuerzo:** Bajo
  - **Ubicación:** `src/shell.css:291`
  - **Qué hacer:** que `.quality-rule` se pliegue por contenedor: `.delivery-layout .quality-rule { grid-template-columns: repeat(auto-fit, minmax(150px, 1fr)); }` y `.quality-contract { min-width: 0; container-type: inline-size; }`; o repartir 1fr/1fr (o un máximo de 3fr con `min-content`) para el editor. De paso, `grid-template-columns: minmax(0, 1fr)` explícito en la rama de una columna de `.delivery-layout`.
  - **Criterio de aceptación:** a 1280×800 y a 1180×800, con el editor abierto y una regla con tolerancia, ningún control de la regla queda fuera de su columna ni bajo el panel de exportación.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-09 · tanda 6 de la Fase 2 (rama `auditoria/2026-11-05`). Dentro de Entregar, `.quality-rule` usa `repeat(auto-fit, minmax(150px, 1fr))` y `.quality-contract` tiene `min-width: 0`; `.delivery-layout` declara `minmax(0, 1fr)` en una columna. Medido con el simulador, editor abierto y tolerancia por cantidad y porcentaje: a 1280×800 y 1180×800 los 6 controles de la regla quedan dentro de su columna y ninguno bajo el panel de exportación (antes, 3 bajo el panel a 1280).

- [x] **AUD-048 · El encabezado fijo del cajón de consulta SQL puede tapar por completo el elemento que recibe el foco**
  - **Área:** Accesibilidad · **Severidad:** Medio · **Esfuerzo:** Bajo
  - **Ubicación:** `src/shell.css:241-253`
  - **Qué hacer:** `.sheet-dialog--drawer > .sheet-dialog__panel { scroll-padding-top: 8rem; }` (o el alto real más 16 px), o quitar `position: sticky` del encabezado.
  - **Criterio de aceptación:** con un resultado de 200 filas, Mayús+Tab desde «Ejecutar consulta» hasta el campo de consulta nunca deja el elemento enfocado bajo el encabezado; verificado con DevTools (rectángulo del foco frente al del encabezado).
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-09 · tanda 6 de la Fase 2 (rama `auditoria/2026-11-05`). El panel del cajón tiene `scroll-padding-top: 8rem`. Medido con el simulador (200 filas de resultado, panel desplazado al final): Mayús+Tab desde «Ejecutar consulta» hasta el campo de consulta no deja ningún elemento bajo el encabezado (antes «Columnas del dataset» quedaba tapado entero y «Motor de consulta», 9 px).

### Diseño responsivo y UI/UX

- [x] **AUD-056 · Una columna de textos largos no tiene tope de ancho: la vista previa se vuelve ilegible en tablas anchas**
  - **Área:** UI/UX · **Severidad:** Medio · **Esfuerzo:** Bajo
  - **Ubicación:** `src/styles.css:516-517`
  - **Qué hacer:** limitar la celda del dataset (`.table-region td { max-width: 36ch; overflow: hidden; text-overflow: ellipsis }`) con el valor completo en `title` desde `renderCellValue`, y una forma de ver la celda entera (clic o foco que la expande, o panel con la fila). Mantener `nowrap` solo para números y fechas.
  - **Criterio de aceptación:** con un dataset de 12 columnas y una de textos de 2.000 caracteres, ninguna columna de la vista previa supera unos 320 CSS px, el valor completo se puede leer (tooltip o expansión) y las cabeceras de perfil siguen alineadas con su columna.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-09 · tanda 7 de la Fase 2 (rama `auditoria/2026-11-05`). Los textos de más de 36 caracteres van en un `span.cell-clip` con tope de 36ch, el valor completo en `title` y expansión al recibir el foco (teclado o clic, hasta 16rem de alto con desplazamiento). Se aplica también a la muestra importada y al resultado de la consulta SQL. Medido con el simulador a 1280×800 (12 columnas, una de 2.000 caracteres): columna larga de 324 px (antes 13.567), `title` completo y cabeceras de perfil alineadas.

- [x] **AUD-057 · Preparar: el resultado de cada cambio y Deshacer/Rehacer viven solo en la columna del historial, que bajo 1180 px queda al final de la página y encima puede ser más alta que la ventana**
  - **Área:** UI/UX · **Severidad:** Medio · **Esfuerzo:** Bajo
  - **Ubicación:** `src/features/prepare/PreparePhase.tsx:253-254`
  - **Qué hacer:** mostrar también el resultado (una línea con «Deshacer») donde se aplica el cambio, junto al `ChangeFeedback`; dar al `aside` fijo `max-height: calc(100dvh - 104px); overflow-y: auto`, y poner los botones Deshacer/Rehacer arriba, bajo el encabezado, antes de la línea de versiones.
  - **Criterio de aceptación:** con 12 versiones a 1280×640, Deshacer y Rehacer se ven sin desplazar la página; a 900 px de ancho, tras aplicar una herramienta aparece el resultado y Deshacer en pantalla sin desplazarse al final.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-09 · tanda 7 de la Fase 2 (rama `auditoria/2026-11-05`). Deshacer y Rehacer pasan arriba del historial, bajo el encabezado; el panel fijo tiene `max-height` y desplazamiento propio; tras aplicar un cambio aparece, fija arriba en `.prepare-layout__main`, una línea con el resultado, «Deshacer» y «Cerrar» (sin anuncio duplicado; se oculta cuando la propuesta muestra su propio resultado). Medido con el simulador: con 12 versiones a 1280×640, Deshacer y Rehacer en y=317 sin desplazar la página (antes 954); a 900×800, tras «Eliminar filas vacías», la línea con Deshacer queda en pantalla (y=19; antes ningún Deshacer visible).

### QA y testing

- [x] **AUD-073 · Full lleva cinco semanas sin ejecutarse y está en rojo (cobertura de 3 archivos críticos y resumen de cobertura de Rust)**
  - **Área:** QA y testing · **Severidad:** Medio · **Esfuerzo:** Medio
  - **Ubicación:** `tools/check.ps1:220-221`
  - **Qué hacer:** (1) añadir `"tsql_script": 95` al JSON de umbrales; (2) cubrir en `LoadPhase`/`App` las ramas nuevas (líneas `App.tsx` 1651-1791 y `LoadPhase.tsx` 107, 120-122, 461 salen sin cubrir) y devolver a `DatasetComparisonSection` las pruebas que perdió, o bajar el umbral con motivo escrito; (3) ejecutar `check.ps1 -Profile Full` antes de cerrar cada bloque de roadmap (o añadir al `pre-push` una variante que compruebe que existe un informe Full `passed` para un commit reciente). Es decisión del dueño bajar umbrales o no (el uso es personal y no hay CI).
  - **Criterio de aceptación:** `check.ps1 -Profile Full` pasa en el commit de cierre y el informe `*-full.json` queda en `.local/validation/`.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-10 · tanda 8 de la Fase 2 (rama `auditoria/2026-11-05`). `tools/check.ps1 -Profile Full` pasa entero (23 pasos, informe `*-full.json` en `.local/validation/`). Sin bajar umbrales: pruebas nuevas en `App`, `LoadPhase`, `DatasetComparisonSection` y `ExplorePhase` (que también estaba por debajo en ramas); Vitest 772 → 786; total 89,72 / 93,34 / 85,69 / 91,53 (antes 88,4 / 92,88 / 85,21 / 90,45). `tsql_script` mide 98,15 % y su umbral es 95. El archivo de umbrales de Rust nunca se había versionado (`.gitignore` ignoraba cualquier `coverage/`): la regla pasa a `/coverage/` y `fixtures/coverage/rust-coverage-thresholds.json` entra en git. `CONTRIBUTING.md` pide ejecutar Full antes de cerrar cada bloque del roadmap; el pre-push no cambia. La intermitencia de `App.test.tsx` no se reprodujo en cinco pasadas completas.

- [x] **AUD-074 · El núcleo de la importación desde bases de datos no tiene ninguna prueba automática (lectura del cursor, lista de tablas, cancelación a mitad, PostgreSQL y MySQL)**
  - **Área:** QA y testing · **Severidad:** Medio · **Esfuerzo:** Medio
  - **Ubicación:** `src-tauri/src/remote_databases.rs:2347`
  - **Qué hacer:** extraer `rows_to_frame(names, columns_values, ...)` (o una función que acepte un iterador de lotes `Vec<Vec<Option<String>>>`) y probarla con límite, recorte y cancelación; añadir `external_odbc_import_postgresql` y `external_odbc_import_mysql` ignoradas que usen las variables existentes; documentarlas en `docs/reference/environment-variables.md`. Si el servidor SQL local (`MSSQLSERVER`, con ODBC 17/18) está disponible, la prueba de importación de SQL Server puede ejecutarse a mano como paso del cierre de cada versión.
  - **Criterio de aceptación:** `remote_databases` sube por encima del 60 % con pruebas que fallan si se quita el tope de filas, el recorte o la comprobación de cancelación; existe una prueba ignorada por motor.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-10 · tanda 8 de la Fase 2 (rama `auditoria/2026-11-05`). El bucle de la importación pasa a `stream_batches_to_parquet` (lotes desde un closure; el lector ODBC lo alimenta igual que antes) con cuatro pruebas: tope exacto con y sin fila extra, recorte, cancelación tras el primer lote sin archivo y error del driver a mitad; cada una falla si se quita el tope, el recorte o la comprobación de cancelación. Ignoradas nuevas `external_odbc_import_postgresql` y `external_odbc_import_mysql` (crean, importan y borran su tabla), documentadas; solo compiladas, porque este equipo no tiene esos servidores. `remote_databases` ya estaba en 68 % por las tandas anteriores; umbral de 56 a 67. Comprobado contra el SQL Server local, solo lectura: la importación de `dbo.Ventas` da 25 filas y, con tope 500, 500 filas con «más filas».

- [x] **AUD-075 · La exportación T-SQL solo se comprueba por subcadenas del texto generado, nunca ejecutada, y con pocos valores hostiles**
  - **Área:** QA y testing · **Severidad:** Medio · **Esfuerzo:** Medio
  - **Ubicación:** `src-tauri/src/tsql_script.rs:777-1005`
  - **Qué hacer:** una prueba `#[ignore]` (con variable `COLUMNIA_ODBC_SQLSERVER` ya existente) que ejecute en una base de pruebas los cuatro scripts (crear, añadir, clave, paquete con `BULK INSERT` desde ruta local) sobre una tabla con nombres y valores hostiles (`]`, `'`, `"`, `\n`, `\r\n`, emoji, texto largo, vacíos y nulos) y compare filas y conteos; añadir a las pruebas de texto un caso con columna `a]b`, `x'y` y salto de línea.
  - **Criterio de aceptación:** existe una prueba ejecutable por entorno que crea, añade y actualiza una tabla real y la compara con el DataFrame de origen; las pruebas de texto incluyen nombres de columna hostiles.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-10 · tanda 8 de la Fase 2 (rama `auditoria/2026-11-05`). Dos pruebas ignoradas (`external_sql_server_runs_the_script_deliveries_on_a_real_table` y `..._bulk_package_deliveries_on_a_real_table`) generan el script y el paquete por la ruta real de exportación y los ejecutan por ODBC en una base de pruebas: crear 8 filas, recrear falla sin tocar la tabla, añadir 2 y actualizar por una clave con nombre hostil; comparan cada celda y los conteos con el DataFrame (valores con `]`, comillas, saltos de línea, emoji, texto de 5.000 caracteres, vacíos y nulos). Prueba de texto nueva con columnas `a]b`, `x'y` y `c"d`. Pasan contra el SQL Server local en una base desechable, borrada al terminar; quitar el escape de `]` o de `'` las rompe.

- [x] **AUD-076 · Sin pruebas de cancelación ni de fallo a mitad en la propuesta a disco (REN-14) ni en las exportaciones nuevas**
  - **Área:** QA y testing · **Severidad:** Medio · **Esfuerzo:** Medio
  - **Ubicación:** `src-tauri/src/dataset/source_stage.rs:305-421`
  - **Qué hacer:** (a) una prueba que cancele tras el primer bloque (contador, como la de `tests.rs:2540`) y compruebe que el dataset no cambia y que el directorio queda con las mismas entradas; (b) una con más filas que `SOURCE_STAGE_BLOCK_ROWS` y una columna que rompa a la mitad; (c) lo mismo para `export_source_backed_tsql_atomic` y el paquete BULK: cancelación y destino inexistente al terminar.
  - **Criterio de aceptación:** pruebas que cancelan o fallan a mitad en la etapa de disco y en las dos exportaciones, comprobando que no quedan `.columnia-*` ni destino a medias.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-10 · tanda 8 de la Fase 2 (rama `auditoria/2026-11-05`). Siete pruebas en `dataset/disk_cancellation_tests.rs`: la etapa a disco cancelada al empezar el segundo bloque y con un error real de lectura en el cuarto deja dataset, historial y carpetas idénticos; el script T-SQL cancelado a medias o con un valor no finito al final, y el paquete BULK cancelado durante el CSV o el ZIP o con un fallo al escribir `cargar.sql`, no dejan temporales y conservan el destino anterior. Cada una falla al quitar la cancelación o el borrado del temporal correspondiente. Sin bugs.

- [x] **AUD-077 · Las estructuras y enums nuevos del IPC de importación y T-SQL no están inscritos en las pruebas de paridad de contratos**
  - **Área:** QA y testing · **Severidad:** Medio · **Esfuerzo:** Bajo
  - **Ubicación:** `src/ipc-contract.test.ts:485-493`
  - **Qué hacer:** añadir las cuatro estructuras y `SqlServerLoadMode` a `sharedStructures` y a `SHARED_ENUMS`, y una prueba que parsee un JSON de ejemplo de TypeScript con `DatabaseSource`/`SqlServerScriptOptions` (la prueba de Rust `options_arrive_from_the_interface_in_camel_case` ya lo hace por un lado).
  - **Criterio de aceptación:** renombrar un campo de `DatabaseImportResult` o una variante de `SqlServerLoadMode` rompe `npm test`.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-10 · tanda 8 de la Fase 2 (rama `auditoria/2026-11-05`). `DatabaseSource`, `DatabaseSourceTable`, `DatabaseImportResult` y `SqlServerScriptOptions` entran en las estructuras compartidas (inventario y su generador, con `tsql_script.rs` como fuente) y `SqlServerLoadMode` y `SqlServerTextSize` en los enums; prueba nueva con el JSON de ejemplo de `DatabaseSource` y `SqlServerScriptOptions`. Renombrar `rowsImported` o la variante `upsert` rompe `npm test`. Quedan fuera otras estructuras del mismo IPC (AUD-113).

## Tier 3 · Bajo (revisión 2026-11-05)

### Seguridad

- [x] **AUD-007 · `zstd` 0.13.3 / `zstd-safe` 7.3.0 se usan al leer Parquet comprimido con ZSTD (hostil), con impacto práctico acotado**
  - **Área:** Seguridad · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `polars-parquet-0.55.2/src/parquet/compression.rs:225-231`
  - **Qué hacer:** `cargo update -p zstd -p zstd-safe -p zstd-sys` y repetir `cargo audit`.
  - **Criterio de aceptación:** `cargo audit` sin los RUSTSEC de `zstd`.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-10 · tanda 9 de la Fase 2 (rama `auditoria/2026-11-05`). `cargo update -p zstd -p zstd-safe -p zstd-sys`: zstd 0.13.4, zstd-safe 7.4.0 y zstd-sys 2.1.2; `Cargo.lock` solo cambia esas tres entradas y `THIRD_PARTY_NOTICES.md` sus tres filas (`-Check` pasa; `zstd` declara ahora BSD-3-Clause en lugar de MIT, licencia ya aceptada para `zstd-safe` y `zstd-sys`; el gate legal pasa). `cargo audit` ya no lista ningún RUSTSEC de `zstd*`. No hay prueba específica de lectura de Parquet con ZSTD.

- [x] **AUD-008 · La política de cifrado de la conexión de importación solo la aplica la interfaz**
  - **Área:** Seguridad · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src-tauri/src/remote_databases.rs:2095-2110`
  - **Qué hacer:** que `list_database_tables` e `import_database_table` reutilicen `transport_encryption_issue` y bloqueen el caso «blocking» (con la misma salida para servidores locales), o mostrar el estado de cifrado en el diálogo (ver AUD-003).
  - **Criterio de aceptación:** `import_database_table` rechaza una cadena sin cifrado a un servidor remoto, con test.
  - **Depende de:** ninguna; conviene hacerla junto con AUD-002
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-10 · tanda 9 de la Fase 2 (rama `auditoria/2026-11-05`). Con la alternativa de «Qué hacer», no con el rechazo del criterio, que es anterior a AUD-002 y AUD-003: Rust compone la conexión desde los campos (no hay cadena libre por IPC) y conectar sin cifrado es una casilla explícita de la interfaz. El diálogo nativo de importación ya mostraba «Cifrado» y «Certificado», y ahora añade «Atención: …» con el mismo texto que el preflight de entrega cuando el canal remoto puede ir sin cifrar o SQL Server acepta cualquier certificado; en un servidor local o con conexión estricta no añade nada. Prueba `the_import_dialog_warns_about_an_unencrypted_or_unchecked_channel`. El texto de la casilla se revisa en AUD-115.

- [x] **AUD-009 · `DatabaseSource` y `DatabaseTarget` derivan `Debug` con la cadena de conexión (contraseña incluida)**
  - **Área:** Seguridad · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src-tauri/src/remote_databases.rs:2061-2067`
  - **Qué hacer:** `Debug` manual que sustituya `connection_string` por `[REDACTED]` y un test.
  - **Criterio de aceptación:** `format!("{:?}", source)` no contiene la contraseña.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-10 · tanda 9 de la Fase 2 (rama `auditoria/2026-11-05`), sin cambios: ya estaba resuelta desde la tanda 1 (AUD-002). `DatabaseSource` y `DatabaseTarget` guardan un `DatabaseConnection` por campos cuyo `Debug` manual muestra `password` como `[REDACTED]` (`remote_databases.rs:579-594`), y la cadena ODBC no se guarda en ninguna estructura. La prueba `debug_output_never_shows_the_password` lo cubre.

- [x] **AUD-100 · Los campos de conexión aceptan caracteres de formato bidireccional, que pueden invertir lo que muestra el diálogo nativo**
  - **Área:** Seguridad · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src-tauri/src/remote_databases.rs:619`
  - **Qué hacer:** `field_text_issue` ya rechaza los caracteres de control y U+2028/U+2029; que rechace también los de formato bidireccional (U+200E, U+200F, U+202A-U+202E, U+2066-U+2069) en servidor, base, usuario y controlador, con el mismo mensaje.
  - **Criterio de aceptación:** un servidor con U+202E se rechaza antes de abrir el diálogo, con una prueba; los nombres con tildes o `ñ` siguen valiendo.
  - **Depende de:** ninguna
  - **Origen:** Fase 2 de la auditoría 2026-11-05, al aplicar AUD-003
  - **Cierre:** 2026-11-10 · tanda 9 de la Fase 2 (rama `auditoria/2026-11-05`). `field_text_issue` rechaza U+200E, U+200F, U+202A-U+202E y U+2066-U+2069 en controlador, servidor, puerto, base y usuario, con el mismo mensaje que los caracteres de control; la contraseña queda fuera porque ningún diálogo la muestra. Las tres órdenes con diálogo nativo validan antes de confirmar. La prueba cubre los 11 caracteres en los cinco campos, y los nombres con tildes y `ñ` siguen valiendo.

- [x] **AUD-114 · `libsqlite3-sys` 0.38.2 empaqueta SQLite 3.53.2, afectado por RUSTSEC-2026-0459 (memoria sin inicializar con un `-wal` de otro tamaño de página)**
  - **Área:** Seguridad · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src-tauri/Cargo.toml:58` (`rusqlite` 0.40.2 con `bundled`)
  - **Qué hacer:** subir `rusqlite` a 0.41 (`libsqlite3-sys` ≥ 0.39.0, SQLite ≥ 3.54.0), revisar los cambios de API en los catálogos (`delivery_presets.rs`, `reusable_tasks.rs`, `projects.rs`, `automation.rs`) y en la exportación a SQLite (`dataset/export_io.rs`), y regenerar `THIRD_PARTY_NOTICES.md`. Ataque local: hace falta que la persona abra una base preparada junto a su `-wal`.
  - **Criterio de aceptación:** `cargo audit` sin RUSTSEC-2026-0459 y las pruebas de catálogos y de exportación a SQLite pasan.
  - **Depende de:** ninguna
  - **Origen:** Fase 2 de la auditoría 2026-11-05, al aplicar AUD-007 (el aviso es del 2026-10-30 y no estaba en la base de avisos que usó la Fase 1)
  - **Cierre:** 2026-11-11 · tanda 10 de la Fase 2 (en `master`). `rusqlite` 0.41.1 con `libsqlite3-sys` 0.39.1, que empaqueta SQLite 3.54.1; el lockfile solo cambia esas dos entradas y `cargo tree -i libsqlite3-sys` da una sola versión. Sin cambios de código: catálogos y exportación a SQLite compilan y sus pruebas pasan. `cargo audit` ya no lista RUSTSEC-2026-0459. `THIRD_PARTY_NOTICES.md` regenerado: mismas filas salvo las dos versiones, pero reordenado entero, porque `-Check` compara el orden y hasta el archivo anterior lo fallaba (AUD-099).

- [x] **AUD-115 · La casilla «Conectar sin cifrado si el servidor no lo ofrece» promete que el cifrado se usa si el servidor lo ofrece, y con `prefer` alguien en la red puede impedirlo**
  - **Área:** Seguridad · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src/features/database/ServerConnectionForm.tsx:126-127` y `src-tauri/src/remote_databases.rs` (atributos `SSLmode=prefer` y `SSLMODE=PREFERRED`)
  - **Qué hacer:** que el texto diga que, con la casilla marcada, alguien con acceso a la red puede forzar una conexión sin cifrar (el cifrado es oportunista), y mantener «Solo en servidores propios o de prueba». Revisar que el aviso de `transport_encryption_issue` dice lo mismo.
  - **Criterio de aceptación:** el texto de la casilla y el aviso del diálogo no afirman que el cifrado se use siempre que el servidor lo ofrezca; prueba de Vitest con el texto nuevo.
  - **Depende de:** ninguna
  - **Origen:** Fase 2 de la auditoría 2026-11-05, al aplicar AUD-008
  - **Cierre:** 2026-11-11 · tanda 10 de la Fase 2 (en `master`). La casilla dice «Solo en servidores propios o de prueba. El cifrado es oportunista: alguien con acceso a la red puede forzar una conexión sin cifrar.», y el aviso de `transport_encryption_issue` (que también sale en el diálogo de importación) dice lo mismo. Prueba nueva en `ServerConnectionForm.test.tsx`.

### Arquitectura, datos y resiliencia

- [x] **AUD-015 · Los `FLOAT` se escriben como literales decimales de hasta cientos de dígitos y SQL Server los rechaza fuera de 38 dígitos**
  - **Área:** Arquitectura · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src-tauri/src/dataset/export_io.rs:2568-2569`
  - **Qué hacer:** para `Float32`/`Float64` escribir en notación científica (`format!("{value:e}")`, que T-SQL admite como literal `float`) o cuando el valor está fuera de, por ejemplo, 1e-15 a 1e15.
  - **Criterio de aceptación:** una prueba con `1e-40`, `1.5e300` y `5e-324` genera literales válidos (`1e-40`, ...) y `SET PARSEONLY ON` no marca error.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-10 · tanda 9 de la Fase 2 (rama `auditoria/2026-11-05`). `tsql_float_literal`: 0 y los valores con magnitud entre 1e-15 y 1e15 siguen en decimal (`3.5`); el resto va con exponente (`1e-40`, `1.5e300`, `5e-324`, `1e40`). `Float32` se escribe con su forma corta, sin pasar por f64. Contra el SQL Server local, `SET PARSEONLY ON` acepta los 19 literales de la prueba, y el decimal antiguo daba Msg 1007. SQL Server lee un subnormal como `5e-324` como 0, con aviso: es un límite de su `float`.

- [x] **AUD-016 · Preparar a disco publica una versión idéntica y la etiqueta como si hubiera convertido tipos cuando la etapa no hace nada**
  - **Área:** Arquitectura · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src-tauri/src/dataset.rs:5876-5879`
  - **Qué hacer:** calcular `force_publish` con el resultado real de la etapa: ejecutar la etapa antes de decidir, o devolver `Ok(false)` y no publicar cuando ni DuckDB ni la etapa cambian nada (`changed_cell_count == 0`, sin renombrados, sin filas quitadas y `stage_outcome` a cero).
  - **Criterio de aceptación:** una propuesta con una columna con ceros a la izquierda marcada para convertir, sobre un archivo grande sin otros cambios, no añade versión al historial.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-11 · tanda 10 de la Fase 2 (en `master`). `force_publish` ya no cuenta la etapa planificada: la etapa devuelve `Ok(true)` solo si tipó, fechó o rellenó algo, y si no borra su archivo y se queda la salida de DuckDB. Sin cambios, no se publica versión. Prueba `a_stage_that_changes_nothing_adds_no_version_on_disk` (ceros a la izquierda marcados para convertir; falla sin el arreglo).

- [x] **AUD-017 · Las medianas de la etapa de huecos no se pueden cancelar**
  - **Área:** Arquitectura · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src-tauri/src/duckdb_query.rs:1638-1660`
  - **Qué hacer:** pasar el cierre de cancelación de la etapa a `parquet_lower_median` (como `count_file_rows`) o usar la versión con `NativeDuckDb` y `interrupt`; de paso `quantile_disc(x, 0.5)` devuelve el mismo valor sin ordenar todo el archivo.
  - **Criterio de aceptación:** cancelar durante el cálculo de una mediana en un archivo grande devuelve el control en menos de 2 s.
  - **Depende de:** ninguna; AUD-035 la resuelve si se hace antes
  - **Origen:** auditoría 2026-11-05
  - **Nota (2026-11-08, al aplicar AUD-035):** la consulta de medianas recibe ya el cierre de cancelación de la etapa y usa `quantile_disc(x, 0.5)` en una sola consulta para todas las columnas. Falta comprobar el criterio (cancelar en menos de 2 s con un archivo grande) antes de cerrarla.
  - **Cierre:** 2026-11-11 · tanda 10 de la Fase 2 (en `master`). Ya cumplía desde AUD-035: `parquet_lower_medians` corre en `execute_duckdb_operation`, que interrumpe la consulta en curso, y es una sola sentencia. Prueba `cancelling_the_lower_medians_returns_control_quickly` (`#[ignore]`, 40 M filas): 6,7 s sin cancelar; cancelando a los 200 ms vuelve en 221 ms.

- [x] **AUD-018 · La mediana de una columna con espacios Unicode en los bordes falla, aunque la etapa la considera numérica**
  - **Área:** Arquitectura · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src-tauri/src/duckdb_query.rs:1618-1633`
  - **Qué hacer:** que `NumericCastCheck::observe` use el mismo recorte que el SQL (solo ASCII) o que el SQL use `trim` Unicode equivalente; una prueba de paridad con `\u00A0`.
  - **Criterio de aceptación:** una columna `"\u00A05"`, `"7"`, vacía, con impute y cast se comporta igual en memoria y a disco (ambas la dejan como texto, o ambas la convierten y rellenan).
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-11 · tanda 10 de la Fase 2 (en `master`). El SQL recorta como `str::trim` de Rust (`[\s\x0B\x{85}\p{Z}]` en `trimmed_text_sql`), así que la columna se convierte y rellena igual en memoria y a disco. Prueba de paridad `source_backed_median_and_cast_trim_unicode_spaces_like_the_in_memory_plan` con U+00A0, U+2003, U+3000 y U+0085 (falla sin el arreglo).

- [x] **AUD-019 · Un fallo de DuckDB al escribir la propuesta se descarta sin dejar rastro y se intenta la ruta en memoria**
  - **Área:** Arquitectura · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src-tauri/src/dataset.rs:2962-2965`
  - **Qué hacer:** registrar el error con `source_backed_fallback("preparar a disco", &error)` y, si es de espacio o de memoria, devolverlo a la interfaz en lugar de `Ok(None)`; comprobar `fs4::available_space` antes de empezar contra el tamaño del origen × 3 y avisar.
  - **Criterio de aceptación:** una escritura que falla por espacio muestra «No hay espacio en disco suficiente...» y queda registrada en el diagnóstico.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-11 · tanda 10 de la Fase 2 (en `master`). El fallo de DuckDB queda en el diagnóstico (`source_backed_fallback("preparar a disco", …)`); si es de espacio o de memoria llega a la interfaz («No hay espacio en disco suficiente para preparar este archivo…» o «No hay memoria suficiente…») y el resto sigue con la ruta en memoria. Antes de escribir se comprueba el espacio libre contra el origen × 3. Prueba de la clasificación y del umbral. No comprobado con el disco lleno de verdad: el texto exacto de DuckDB en Windows y el recorrido hasta la interfaz.

- [x] **AUD-020 · La última importación de base de datos queda en claro en `%APPDATA%` sin purga, y el temporal de escritura se pierde si el proceso muere**
  - **Área:** Arquitectura · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src-tauri/src/remote_databases.rs:2465-2511`
  - **Qué hacer:** borrar el contenido de `database-imports/` al arrancar (la carga ya hizo su copia al historial) o al descargar el dataset, y crear el temporal con sufijo `.parquet.partial` para que la limpieza lo incluya.
  - **Criterio de aceptación:** tras cerrar y abrir, `database-imports/` está vacía y no hay `.tmp*`.
  - **Depende de:** AUD-011 (mismo `write_import_parquet`)
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-11 · tanda 11 de la Fase 2 (en `master`). Al arrancar se vacía `database-imports/` (`purge_database_imports`, junto a la purga de copias UTF-8; la instancia única garantiza que nadie la usa, y la ruta del dataset no se guarda en proyectos ni tareas). El temporal de escritura lleva el sufijo `.parquet.partial`, así que la purga lo incluye si el proceso muere. Prueba de la purga con un temporal abandonado. No comprobado cerrando y abriendo la app real.

- [x] **AUD-021 · La actualización por clave acepta como clave una columna que la privacidad enmascara o seudonimiza**
  - **Área:** Arquitectura · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src-tauri/src/dataset.rs:10197`
  - **Qué hacer:** al exportar, si alguna clave está entre las columnas protegidas (`protected_columns`), devolver un error o avisar antes de abrir el selector.
  - **Criterio de aceptación:** exportar con `mask` o `hash` y una clave personal muestra el error con el nombre de la columna.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-11 · tanda 11 de la Fase 2 (en `master`). Con Enmascarar o Seudonimizar, el script T-SQL y el paquete BULK INSERT con actualización por clave se rechazan antes del diálogo de guardado si una clave es una columna que la privacidad protege («La columna clave «email» está protegida por la privacidad…»). Usa el mismo criterio por nombre que la exportación (`privacy_signal`), que también incluye `id` o `codigo`: esas claves ya llegaban cambiadas al destino. Prueba con `mask`, `hash` y sin privacidad. No comprobado en la app.

- [x] **AUD-102 · Abrir un proyecto no vacía las copias de trabajo, y las copias de comparación siguen en carpetas `.tmp*` que la purga no ve**
  - **Área:** Arquitectura · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src-tauri/src/dataset.rs:12813`
  - **Qué hacer:** que `commit_project_open_with_candidate` y `activate_project_candidate` (`dataset.rs:13207`) vacíen el índice de conflictos y la copia del perfil, como `import_loading.rs:311`, y comprobar si dejan la comparación pendiente (allí `comparison = None`). Que las copias de comparación (`comparison_io.rs`, funciones `persist_*`, y `dataset.rs:2059`) usen `locked_directory` con su prefijo. Cuidado: las pruebas llaman a esas funciones en paralelo, y vaciar la caché global sin más vuelve inestables las que comprueban aciertos de caché.
  - **Criterio de aceptación:** tras abrir un proyecto no queda la copia del perfil del dataset anterior; matar el proceso con una comparación abierta y volver a arrancar no deja `compared.parquet` en `%TEMP%`.
  - **Depende de:** ninguna
  - **Origen:** Fase 2 de la auditoría 2026-11-05, al aplicar AUD-010
  - **Cierre:** 2026-11-11 · tanda 11 de la Fase 2 (en `master`). Abrir un proyecto descarta la comparación abierta, el índice de conflictos y la copia del perfil del dataset anterior (`release_replaced_dataset_state`, por origen para no tocar la caché compartida de las pruebas). Las copias de comparación van a carpetas `columnia-comparison-*` con `locked_directory`, que la purga de arranque borra si el proceso murió y salta mientras siguen en uso. Tres pruebas nuevas. No comprobado matando el proceso con una comparación abierta.

- [x] **AUD-103 · Cancelar una importación desde base de datos espera al controlador, y no puede volver antes sin riesgo**
  - **Área:** Arquitectura · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src-tauri/src/remote_databases.rs:2711`
  - **Qué hacer:** comprobar la cancelación justo antes de `persist` en `ImportParquetWriter::finish` (era `write_import_parquet` antes de AUD-012), para que un hilo cancelado no escriba ni borre nada, y después usar `run_blocking_unless_cancelled` (AUD-013) también en `import_database_table`.
  - **Criterio de aceptación:** con un servidor que no responde, Cancelar una importación devuelve la interfaz a su estado en menos de 2 s, y el hilo que vuelve después no deja ningún Parquet nuevo ni borra los anteriores (prueba con un trabajo simulado).
  - **Depende de:** AUD-012 (cambia el mismo camino de escritura)
  - **Origen:** Fase 2 de la auditoría 2026-11-05, al aplicar AUD-013
  - **Cierre:** 2026-11-11 · tanda 11 de la Fase 2 (en `master`). `import_database_table` usa `run_blocking_unless_cancelled`: Cancelar vuelve enseguida aunque el controlador siga bloqueado. El escritor comprueba la cancelación justo antes de `persist`, así que el hilo que vuelve tarde no deja Parquet ni borra los anteriores, y el dataset activo no cambia. Prueba con un trabajo bloqueado (vuelve en menos de 2 s; falla sin el arreglo). No comprobado con un servidor real que no responde.

### Código

- [x] **AUD-027 · Si no se pueden listar los controladores ODBC, el formulario dice «no hay un controlador instalado»**
  - **Área:** Código · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src/features/database/ServerConnectionForm.tsx:36-43`
  - **Qué hacer:** guardar `drivers: null | string[] | {error: string}` y mostrar `errorMessage(error)` (de `bridge/errors`); en Rust, mapear con un texto en español sin `Debug`.
  - **Criterio de aceptación:** con `listDatabaseDrivers` rechazado, el formulario muestra el motivo del fallo y no el mensaje de «instala el controlador». Ningún mensaje de `list_database_drivers` contiene `{`, `Diagnostics` ni `State:`.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-11 · tanda 12 de la Fase 2 (en `master`). `ServerConnectionForm` guarda la lista como `null | string[] | {error}` y, si listar falla, muestra el motivo con `errorMessage` en un aviso `role="alert"` en lugar de «instala el controlador». En Rust, `list_database_drivers` pasa por `driver_list_error` (frase con `readable_driver_detail`, sin `Debug`, `Diagnostics` ni `State:`). Pruebas nuevas en Vitest y Rust.

- [x] **AUD-028 · Los controladores se cargan después de que el usuario puede haber escrito, y el padre no se entera de que ya hay controlador**
  - **Área:** Código · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src/features/database/ServerConnectionForm.tsx:36-43`
  - **Qué hacer:** dentro del `then`, calcular el estado resultante y llamar a `onChange(problem === null ? build(...) : null)` en el mismo paso (o deshabilitar los campos mientras `drivers === null`).
  - **Criterio de aceptación:** una prueba que escribe los campos con `listDatabaseDrivers` pendiente y luego lo resuelve termina con `onChange` recibiendo la cadena.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-11 · tanda 12 de la Fase 2 (en `master`). `update` solo cambia los campos y un efecto sobre `kind` y los campos avisa al padre (con `onChange` en una referencia), así que el controlador que llega después de escribir también le llega. Al montar avisa `null`, que es lo correcto para un formulario vacío (se remonta con `key={kind}`). Prueba nueva con el listado pendiente; falla sin el arreglo.

- [x] **AUD-029 · La cabecera de columna oculta el histograma de las columnas numéricas continuas con todos los valores distintos**
  - **Área:** Código · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src/components/ColumnProfileHeader.tsx:60`
  - **Qué hacer:** `const identifierLike = allDistinct && !(numeric && !isIntegerType(column.dataType))` o decidir con `profile.suggestedType`; el histograma se oculta solo para enteros/texto.
  - **Criterio de aceptación:** una columna `Float64` de 100 valores distintos muestra su histograma; una columna `Int64` 1..100 (identificador) no.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-11 · tanda 12 de la Fase 2 (en `master`). `identifierLike` solo oculta el histograma de enteros y texto con todos los valores distintos; float y decimal (importes, medidas) lo conservan. Pruebas: Int64 1..100 sin histograma; Float64, f64 y Decimal(10, 2) con él.

- [x] **AUD-030 · Decisiones por el texto de un mensaje: errores de DuckDB en inglés y un aviso propio en Entregar**
  - **Área:** Código · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src-tauri/src/duckdb_query.rs:479`
  - **Qué hacer:** una función `is_duckdb_out_of_memory(&str)` en un solo lugar con una prueba que provoque el error con `SET memory_limit='1MB'` (así un cambio de redacción rompe la prueba y no el flujo), y llamar a una única función de respaldo desde las dos rutas. En Entregar, que el problema de conexión llegue como un valor con tipo (un código o una variante) y no como texto.
  - **Criterio de aceptación:** existe una sola función de clasificación, cubierta por una prueba que falla si DuckDB cambia el texto. Ninguna decisión de `DeliveryPhase.tsx` depende del texto de un mensaje.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-11 · tanda 12 de la Fase 2 (en `master`). `is_duckdb_out_of_memory` es la única clasificación del error de memoria de DuckDB (los dos respaldos de conteo y `source_backed_resource_error`), con una prueba que provoca el error real con `memory_limit='1MB'`. En Entregar, el campo de destino con problema sale de `databaseTargetIssue`, la regla marcada de `invalidQualityRuleIndex` y el «Resultado incierto» de `exportState.uncertain`, que se fija al capturar el error (`isUncertainCommitError`, reconocido una sola vez en `toBridgeError` con una prueba que compara el texto con la constante de Rust). Sin cambios del contrato IPC.

- [x] **AUD-031 · Código y comentarios obsoletos en las rutas nuevas de T-SQL y de textos de entrega**
  - **Área:** Código · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src-tauri/src/duckdb_query.rs:918-923`
  - **Qué hacer:** sustituir los tres brazos por el error genérico de «tipo no admitido» (o `unreachable`-con-mensaje), corregir el comentario, y separar `exportFormatLabel` de un `exportDestinationLabel` para la leyenda.
  - **Criterio de aceptación:** la leyenda dice «Conexión · SQL Server» y el botón «Exportar a SQL Server»; no quedan brazos inalcanzables en `duckdb_tsql_value`.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-11 · tanda 12 de la Fase 2 (en `master`). Fuera los brazos `Date32`, `Time64` y `Timestamp` de `duckdb_tsql_value`: la única llamada lee la proyección de `tsql_text_projection`, que ya pasa esos tipos a texto; si llegaran, dan el error genérico de tipo no admitido. La leyenda usa `exportDestinationLabel` («Conexión · SQL Server») y el botón sigue con «Exportar a SQL Server». El comentario obsoleto ya no estaba.

- [x] **AUD-032 · Las interrupciones a DuckDB se envían una sola vez y DuckDB reinicia la marca al empezar cada sentencia**
  - **Área:** Código · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src-tauri/src/duckdb_query.rs:329-343`
  - **Qué hacer:** repetir la interrupción mientras la sentencia siga viva: en `run_native_with_progress`, llamar a `native.interrupt()` en cada vuelta mientras `cancelled` sea verdadero y el hilo no haya terminado; en `finish` y `Drop`, reemplazar `interrupt(); join()` por un bucle `while !handle.is_finished() { interrupt(); sleep(10 ms) }` y luego `join`.
  - **Criterio de aceptación:** una prueba que dispara la cancelación inmediatamente después de `spawn_parquet_distinct_count` sobre un Parquet grande (o con una consulta artificialmente larga) termina en tiempo acotado en 100 ejecuciones seguidas.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-11 · tanda 12 de la Fase 2 (en `master`). La interrupción se repite mientras la sentencia sigue viva en los tres vigilantes: `run_native_with_progress`, `NativeProgress::interrupt_and_join` (en `finish` y `Drop` de `BackgroundDistinctCount`) y el de `execute_duckdb_operation_with_cancel_state`, que antes también interrumpía una sola vez. Pruebas nuevas: 10 cancelaciones inmediatas (siempre) y 100 sobre 20 M filas (`#[ignore]`): la más lenta, 45 ms; con una sola interrupción, la de 100 falló en la vuelta 13 con 2,3 s.

- [x] **AUD-033 · El comentario `SAFETY` de `unsafe impl Sync for NativeDuckDb` cubre menos de lo que el tipo permite**
  - **Área:** Código · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src-tauri/src/duckdb_query.rs:522-525`
  - **Qué hacer:** separar el tipo en un asa compartible (`Arc<NativeHandle>` con `progress`/`interrupt`, `Send + Sync`) y un ejecutor exclusivo (`NativeDuckDb` con `execute(&mut self)`, `Send` solamente). O ampliar el comentario `SAFETY` con la restricción «solo el hilo propietario llama a `execute`».
  - **Criterio de aceptación:** el compilador impide llamar a `execute` desde un `Arc` compartido, o el comentario nombra la restricción.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-11 · tanda 13 de la Fase 2 (en `master`). Por la segunda vía del criterio: el comentario `SAFETY` de `NativeDuckDb` dice ahora que `Sync` solo cubre `progress` e `interrupt` y que `execute` (y abrir conexiones sobre `database`) lo llama un único hilo a la vez. No se separan los tipos: `run_native_with_progress` y `spawn_parquet_distinct_count` comparten la conexión entre hilos y el cambio crecía; los usos actuales se comprobaron y cumplen la restricción.

- [x] **AUD-104 · Importar una celda de texto con un sustituto UTF-16 suelto hace entrar en pánico la lectura y la importación termina con un error genérico**
  - **Área:** Código · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src-tauri/src/remote_databases.rs`, `read_cursor_to_parquet` (`slice.iter()`, ≈`:2789`)
  - **Qué hacer:** leer cada celda con `get(row)` (que devuelve `&[u16]` sin pánico), como ya hace el catálogo desde AUD-024, y decidir qué hacer con un valor que no es UTF-16 válido: sustituir el carácter con `String::from_utf16_lossy` y nombrar la columna en el resultado, igual que `truncated_columns`, o rechazar la importación con un mensaje que diga la columna. `TextColumnView::<u16>::iter()` de odbc-api 29.1.0 hace `expect("Wide character encoding must be UTF-16")` (`text_column.rs:582`). SQL Server guarda en `nvarchar` sustitutos sueltos sin quejarse.
  - **Criterio de aceptación:** importar una tabla con una celda `NCHAR(0xD800) + N'x'` no entra en pánico: termina con las filas importadas y la columna señalada, o con un error que la nombra. Prueba unitaria de la decodificación y prueba externa `#[ignore]`.
  - **Depende de:** ninguna
  - **Origen:** Fase 2 de la auditoría 2026-11-05, al aplicar AUD-024. Reproducido el 2026-11-08 contra la base de pruebas: la prueba externa de importación entra en pánico en `text_column.rs:582`. Ya pasaba antes de la tanda 3; en la app, el pánico dentro de `spawn_blocking` llega como «La importación se interrumpió inesperadamente.» (perfil `panic = "unwind"`).
  - **Cierre:** 2026-11-11 · tanda 13 de la Fase 2 (en `master`). `read_cursor_to_parquet` lee cada celda con `get(row)` y `decode_wide_cell`: un sustituto suelto pasa a U+FFFD y la columna se señala en `replacedColumns` (IPC nuevo), con aviso en el panel de importación, igual que las truncadas. Pruebas de la decodificación y del aviso, y la externa `external_odbc_import_survives_lone_surrogates` (`COLUMNIA_ODBC_IMPORT_SURROGATE_TABLE`), ejecutada contra una base temporal del SQL Server local ya borrada: 2 filas y la columna señalada; sin el arreglo, pánico `LoneSurrogate`.

- [x] **AUD-106 · Los errores de la comparación dicen que el snapshot «cambió durante la lectura» cuando solo no coincide el número de filas**
  - **Área:** Código · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src-tauri/src/dataset/comparison_engine.rs` (≈`:1941`, `:2093`, `:2461` y `:2473`)
  - **Qué hacer:** usar la redacción que dejó AUD-022 en `query_execution.rs` («no corresponde con el dataset») cuando lo que falla es el recuento de filas de un bloque, y reservar «cambió durante la lectura» para cuando consta un cambio del origen (tamaño o fecha).
  - **Criterio de aceptación:** ningún mensaje de desajuste de filas en `comparison_engine.rs` dice «cambió»; las pruebas que comprueban esos errores siguen pasando (ajustando solo el texto esperado).
  - **Depende de:** ninguna
  - **Origen:** Fase 2 de la auditoría 2026-11-05, al aplicar AUD-022.
  - **Cierre:** 2026-11-11 · tanda 13 de la Fase 2 (en `master`). Los cuatro desajustes de filas de `comparison_engine.rs` dicen «no corresponde con la comparación» o «no corresponde con el dataset», con las filas esperadas y obtenidas. Los de la lectura de conflictos que comparan tamaño de la fuente o ruta, tamaño y filas del activo siguen diciendo «cambió». Ningún código dependía del texto.

- [x] **AUD-107 · La barra «Contando valores distintos» se queda quieta mientras corre el respaldo por falta de memoria**
  - **Área:** Código · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src-tauri/src/duckdb_query.rs`, `NativeProgress::fraction`; `BackgroundDistinctCount::finish` (`dataset/profile_engine.rs`)
  - **Qué hacer:** que el respaldo informe de su avance (por ejemplo, columnas terminadas sobre el total, en un `AtomicUsize` compartido como la bandera de cancelación de AUD-026) y que `fraction` lo use cuando la conexión nativa ya no está contando.
  - **Criterio de aceptación:** con el respaldo forzado (límite de memoria bajo, como en la prueba de AUD-026), la fracción que devuelve `NativeProgress` sube a medida que el respaldo termina columnas.
  - **Depende de:** ninguna
  - **Origen:** Fase 2 de la auditoría 2026-11-05, al aplicar AUD-026.
  - **Cierre:** 2026-11-11 · tanda 13 de la Fase 2 (en `master`). `NativeProgress` comparte con el hilo un `FallbackProgress` (columnas terminadas sobre el total) y `fraction` lo usa cuando corre el respaldo; además nunca devuelve menos de lo ya mostrado, así que el cambio de DuckDB al respaldo puede parar la barra pero no la hace retroceder. Prueba nueva con `memory_limit` de 16 MB: la fracción pasa por 1/3, 2/3 y 1 sin bajar; sin el arreglo se queda en 0.

### Rendimiento

- [x] **AUD-038 · Cada página de conflictos abre un escaneo Parquet por fila del archivo comparado (hasta 50 por página)**
  - **Área:** Rendimiento · **Severidad:** Bajo · **Esfuerzo:** Medio
  - **Ubicación:** `src-tauri/src/dataset/comparison_engine.rs:2403-2500`
  - **Qué hacer:** ordenar los pares de la página por `compared_row_index` y leer los grupos de filas implicados una sola vez (un `slice` por bloque en vez de por fila), o agrupar las filas comparadas de la página en un `IN` de DuckDB; envolver `conflict_markers` y `conflict_rows` en `BufWriter`/`BufReader` o escribir los marcadores en un `Vec<u8>` por cubo.
  - **Criterio de aceptación:** una página de 50 conflictos con filas comparadas dispersas en un Parquet de 20 millones de filas y 40 columnas de texto tarda lo mismo (± 20 %) que una con filas contiguas.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-11 · tanda 14 de la Fase 2 (en `master`). Las filas comparadas de una página se leen ordenadas, en tramos de hasta 1.024 filas, un `slice` por tramo y hasta 4 en paralelo, y la página conserva su orden. Medido con 3 M filas × 40 columnas de texto (solo las 50 filas comparadas): contiguas 2,5 s → 0,05 s; dispersas 2,4 s → 0,75 s. Los marcadores siguen sin `BufWriter`: son escrituras con `seek` por conflicto. **Criterio ajustado por decisión del usuario (2026-11-11):** se da por bueno que las páginas con filas dispersas tarden más que las contiguas (0,75 s frente a 0,05 s en la medición), porque igualarlas exige rehacer el índice de conflictos con un mapa de grupos, y 0,75 s por página es aceptable para el uso personal.

- [x] **AUD-039 · El hash exacto de filas formatea con `Debug` cada celda no textual de cada fila**
  - **Área:** Rendimiento · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src-tauri/src/dataset_fingerprints.rs:278-310`
  - **Qué hacer:** escribir a `hasher` los bytes nativos del valor (`write_u64`, `to_bits()` para floats, `i128` para decimales) con una etiqueta de tipo, en lugar de `Debug`.
  - **Criterio de aceptación:** `exact_row_fingerprint` no asigna memoria por celda (una prueba con un asignador contador, o `cargo bench` simple) y el recuento de duplicados exactos no cambia en las pruebas existentes.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-11 · tanda 14 de la Fase 2 (en `master`). `exact_row_fingerprint` escribe una etiqueta de tipo y los bytes nativos de cada valor no textual (con NaN canónico, unidad y zona en fechas), sin `format!`; listas, structs y tipos raros siguen con `Debug`. Mismas equivalencias que antes, y el hash no se guarda en disco más allá de la pasada del perfil. Sin asignación por celda por construcción (no se contaron asignaciones); las pruebas de duplicados siguen pasando.

- [x] **AUD-040 · El generador de T-SQL asigna varias cadenas por celda y una por cada byte binario**
  - **Área:** Rendimiento · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src-tauri/src/tsql_script.rs:339-358`
  - **Qué hacer:** `render_value(&TsqlValue, &mut impl Write)` que escribe directamente (cifras con `write!(out, "{value}")`, texto escapando comillas sobre la marcha, hexadecimal con una tabla de 16 bytes), y `push_row` escribiendo sin `Vec<String>`.
  - **Criterio de aceptación:** el script generado es byte a byte idéntico al actual en las pruebas existentes de `tsql_script.rs`, y un blob de 1 MB se renderiza sin asignaciones por byte.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-11 · tanda 14 de la Fase 2 (en `master`). `render_value` escribe directo en la salida (texto con las comillas duplicadas sobre la marcha, binario con una tabla de 16 dígitos) y `push_row` ya no arma un `Vec<String>`. Una prueba de referencia escrita antes del cambio (todos los tipos, Unicode, los 256 bytes y 3.000 filas que cruzan dos cortes de `INSERT`) confirma que el script es idéntico byte a byte; un blob de 1 MB se renderiza sin asignar por byte.

- [x] **AUD-041 · El paquete BULK INSERT escribe el CSV con un solo hilo y comprime todo el CSV en la fase final sin avance de progreso**
  - **Área:** Rendimiento · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src-tauri/src/duckdb_query.rs:1012`
  - **Qué hacer:** quitar `SET threads = 1` o documentar el motivo; guardar el CSV con `CompressionMethod::Stored` cuando supere un umbral (el CSV ya es texto, y el ZIP deja de ser un paso de CPU), o emitir `report` por bytes copiados en `copy_file_with_cancel`.
  - **Criterio de aceptación:** exportar un paquete BULK desde un CSV de 6 millones de filas no deja la barra más de 10 s en el mismo porcentaje y el tiempo total baja al menos a la mitad (medido con el mismo dataset).
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-11 · tanda 14 de la Fase 2 (en `master`). Fuera `SET threads = 1` (sin motivo documentado; el orden lo conserva `preserve_insertion_order`, con prueba de orden), la copia del CSV al ZIP mueve la barra de 70 a 87 % («Empaquetando el CSV») y, desde 128 MiB, el CSV va sin comprimir. Medido en `--release` con 3 M filas × 10 columnas (367 MiB de CSV): total 6,77 s → 1,86 s y mayor tramo sin avance 3,47 s → 1,29 s. Contrapartida: el ZIP de un CSV grande pesa casi lo que el CSV (367 MiB frente a 64 MiB).

- [x] **AUD-042 · `ColumnProfileHeader` y `format.ts` crean un `Intl.NumberFormat` por cada llamada, en la vista previa de Revisar de cada columna y en cada render**
  - **Área:** Rendimiento · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src/format.ts:10-25`
  - **Qué hacer:** crear los formateadores una vez por módulo (`const decimalFormat = new Intl.NumberFormat(...)` y un caché por número de decimales) y envolver `ColumnProfileHeader` en `React.memo` (sus propiedades `column`, `profile` y `rowCount` son estables mientras el perfil no cambie).
  - **Criterio de aceptación:** con un dataset de 200 columnas, el render del panel desde el Profiler de React tarda menos de la mitad que ahora y el número de construcciones de `Intl.NumberFormat` por render es 0.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-11 · tanda 14 de la Fase 2 (en `master`). `formatDecimal` y `formatNumber` reutilizan un formateador por configuración y `ColumnProfileHeader` va en `memo` (sus propiedades son estables); una prueba con 200 columnas comprueba 0 construcciones de `Intl.NumberFormat` al renderizar y re-renderizar. **Criterio ajustado por decisión del usuario (2026-11-11):** basta con 0 construcciones por render; la cifra de «la mitad» era una estimación sin medir, y el montaje en frío lo domina React.

- [x] **AUD-043 · Los bloques de lectura no están alineados con los grupos de filas de la copia Parquet: cada trozo de columna se descomprime entre 2,75 y 8 veces**
  - **Área:** Rendimiento · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src-tauri/src/duckdb_query.rs:264-266`
  - **Qué hacer:** que la copia de trabajo se escriba con un tamaño de grupo igual o divisor del bloque más pequeño que la lee de forma habitual: `COPY (...) TO '...' (FORMAT PARQUET, ROW_GROUP_SIZE 65536)` en `copy_to_parquet_statement` y en las demás copias que luego se leen por bloques (`duckdb_query.rs:399`, `:619`), y, para filas muy anchas, el mismo cálculo que `parquet_row_group_rows` (que ya limita por bytes). Para las lecturas de 16.384 filas, subirlas a 65.536 (o leer con el bloque igual al grupo) si el consumo de memoria del bloque lo permite. Para la importación, `with_row_group_size(Some(parquet_row_group_rows(frame)))` (ver AUD-037). Contrapartida: grupos más pequeños duplican el tamaño del pie del Parquet y reducen algo el paralelismo de lectura de DuckDB; hay que medirlo.
  - **Criterio de aceptación:** con `POLARS_PARQUET_METRICS=1` en una pasada del perfil sobre un Parquet de la copia, la suma de `num_decompressed_pages` por columna es igual al número de grupos (1 visita por grupo) y el tiempo de «Analizando filas y columnas» en 2019-Oct con la copia alineada no empeora respecto al actual; `parquet_metadata()` de DuckDB muestra grupos de 65.536 filas.
  - **Depende de:** ninguna; AUD-037 aplica la misma regla a la importación
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-11 · tanda 14 de la Fase 2 (en `master`). Las 6 copias de trabajo que escribe DuckDB usan `ROW_GROUP_SIZE 65536` (las de Polars ya lo hacían desde AUD-037), con prueba sobre `parquet_metadata()`. Con `POLARS_PARQUET_METRICS=1` en un Parquet sintético de 6 M filas (2019-Oct no está en disco): con el bloque del perfil, 134 → 92 descompresiones por columna (una por grupo) y 0,51 → 0,34 s; con el bloque de 16.384, 1,78 → 1,17 s; archivo +0,65 %.

- [x] **AUD-109 · Las demás exportaciones de un CSV grande vuelven a leer el archivo aunque exista la copia del perfil**
  - **Área:** Rendimiento · **Severidad:** Bajo · **Esfuerzo:** Medio
  - **Ubicación:** `src-tauri/src/dataset/export_io.rs` (exportaciones `source_backed` a CSV, JSON, Excel, SQL, SQLite, Parquet y paquete)
  - **Qué hacer:** leer `reusable_profile_copy(source, format)` cuando exista, como ya hacen el script T-SQL y el paquete BULK INSERT desde AUD-034, y comprobar formato por formato que el resultado no cambia (tipos de las columnas de la copia frente a la lectura del CSV).
  - **Criterio de aceptación:** tras perfilar un CSV de más de un bloque, cada exportación da el mismo archivo con y sin la copia, y con la copia no relee el CSV.
  - **Depende de:** AUD-108 (formato de lectura de la exportación)
  - **Origen:** Fase 2 de la auditoría 2026-11-05, al aplicar AUD-034.
  - **Cierre:** 2026-11-11 · tanda 15 de la Fase 2 (en `master`). `export_read_of(source)` (`export_io.rs`) devuelve la copia del perfil, con su `CachedProfileSnapshot` vivo, o el CSV si no la hay; lo usan todas las exportaciones source-backed: CSV, CSV para Excel, JSON, SQL, Parquet, Excel, SQLite, paquete (filas y nulos), script T-SQL y paquete BULK INSERT (que dejan su bloque propio de AUD-034). La lectura del CSV es `all_varchar` y la copia es el Parquet de esa misma lectura, así que ningún formato cambia. El esquema con tipos de Excel, SQLite y el paquete sigue saliendo de Polars sobre el CSV (lectura corta de esquema). Prueba nueva: con un CSV de más de un bloque, los 10 formatos dan lo mismo con y sin copia, y tras sustituir el CSV por otro texto del mismo tamaño y fecha la salida no cambia (no se relee). No medido en la app real.

- [x] **AUD-110 · Correcciones seguras, Explorar y la línea base del historial no reutilizan la copia del perfil de un CSV sin encabezado**
  - **Área:** Rendimiento · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src-tauri/src/dataset.rs`, `source_backed_safe_corrections_with_cancellation`; `get_explore_panel`; `dataset/comparison_engine.rs`, `prepare_source_backed_baseline_snapshot`
  - **Qué hacer:** sustituir sus condiciones propias (solo `Delimited`, o solo con encabezado) por `reusable_profile_copy(source, format)`, que ya admite `DelimitedWithoutHeader` y decide la validez en un solo sitio.
  - **Criterio de aceptación:** con un CSV sin encabezado perfilado, las tres rutas leen la copia (la ruta devuelta es la de la copia) y dan el mismo resultado que sin ella.
  - **Depende de:** ninguna
  - **Origen:** Fase 2 de la auditoría 2026-11-05, nota de AUD-022 no aplicada en AUD-034.
  - **Cierre:** 2026-11-11 · tanda 15 de la Fase 2 (en `master`). Las correcciones seguras, `get_explore_panel` y `prepare_source_backed_baseline_snapshot` usan `reusable_profile_copy(source, format)` en lugar de sus condiciones propias (solo `Delimited` o solo con encabezado), así que también reutilizan la copia de un CSV sin encabezado (`column_N`). Prueba nueva con un CSV sin encabezado: la versión de partida es byte a byte la copia, y Explorar y las correcciones seguras dan lo mismo con y sin ella. Explorar se comprueba con las mismas piezas, no con el comando (necesita `AppHandle`).

- [x] **AUD-111 · Las estadísticas de imputación lanzan una subconsulta por columna y calculan la mediana pasando por DOUBLE**
  - **Área:** Rendimiento · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src-tauri/src/duckdb_query.rs`, `count_file_imputation_stats` (≈`:2081-2146`), usada por `source_backed_imputation_with_cancellation` (`dataset.rs`)
  - **Qué hacer:** calcular cada mediana con `quantile_disc(x, 0.5) FILTER (...)` dentro de la misma agregación, como `parquet_lower_medians` desde AUD-035, en lugar de `(SELECT quantile_disc(CAST(col AS DOUBLE), 0.5) FROM dataset WHERE ...)` por columna, y sin pasar los enteros por DOUBLE.
  - **Criterio de aceptación:** k columnas cuestan una sola lectura del archivo, y un entero mayor que 2^53 da la misma mediana que la ruta en memoria.
  - **Depende de:** ninguna
  - **Origen:** Fase 2 de la auditoría 2026-11-05, al aplicar AUD-035 (visto en el código).
  - **Cierre:** 2026-11-11 · tanda 15 de la Fase 2 (en `master`). `count_file_imputation_stats` recibe `FileImputationKind` (`Text`, `Integer`, `Float`) y calcula la mediana inferior dentro de la agregación única: `quantile_disc(col, 0.5)` en BIGINT para enteros y `... FILTER (WHERE isfinite(col))` para floats, sin subconsulta por columna ni paso por DOUBLE. Una columna `Int64`/`Float64` del dataset source-backed siempre viene de un Parquet tipado (un CSV cargado es todo texto), así que el `CAST` ya no hacía falta. Prueba nueva: 9007199254740993, …995, nulo y …997 dan …995 como la ruta en memoria (antes, …996). «Una sola lectura» por construcción, sin `EXPLAIN`. La moda de las columnas de texto sigue con una subconsulta por columna (sin tarea).

### Accesibilidad y semántica

- [x] **AUD-049 · Entregar: el motivo del bloqueo no se anuncia ni queda asociado al botón, que además sale del orden de tabulación**
  - **Área:** Accesibilidad · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src/features/delivery/DeliveryPhase.tsx:322-336`
  - **Qué hacer:** `aria-disabled="true"` en lugar de `disabled` en la acción principal (manteniendo el borde discontinuo con `[aria-disabled="true"]`), `aria-describedby` en el propio botón y `role="status"` en el contenedor persistente del motivo (que exista siempre y cambie de texto, no que se monte con el texto). Guardar el `onClick` para ignorar el clic cuando haya bloqueo.
  - **Criterio de aceptación:** con NVDA/Narrador, al marcar «Actualizar por clave» sin columnas se anuncia el motivo; Tab llega a «Exportar» y la descripción lo explica.
  - **Depende de:** ninguna; hacerla junto con AUD-060 (mismo botón)
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-11 · tanda 16 de la Fase 2 (en `master`). `DeliveryPhase.tsx`: las dos acciones principales (exportar y Power BI) llevan `aria-disabled` en lugar de `disabled`, `aria-describedby="export-blocked-reason"` y un `onClick` que ignora el clic con bloqueo; el motivo es un `<p role="status">` siempre montado que solo cambia de texto (`:empty` compensa el hueco). Las tres reglas `:hover:not(:disabled)` de `.primary-action` en `workflow-styles.css` excluyen también `[aria-disabled="true"]`. Comprobado en el navegador con el mock de Tauri (T-SQL, «Actualizar por clave» sin columnas): el botón recibe el foco, borde discontinuo, el hover no lo rellena y el motivo («Para exportar, elige al menos una columna clave…») está en su `role="status"`. No probado con NVDA ni Narrador. AUD-060 sigue abierta.

- [x] **AUD-050 · Formularios de conexión: campos obligatorios sin marcar y sin asociar con el motivo por el que «Ver tablas» sigue desactivado**
  - **Área:** Accesibilidad · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src/features/database/ServerConnectionForm.tsx:70-95`
  - **Qué hacer:** `required` + «(obligatorio)» en la etiqueta de Servidor y Usuario; un único contenedor persistente `role="status"` con el motivo y `aria-describedby` hacia él en el campo que falta (o `aria-invalid` en ese campo); `aria-required` y mensaje en el grupo de claves.
  - **Criterio de aceptación:** con lector de pantalla se oye qué campo falta y por qué «Ver tablas» no está disponible, en los tres motores.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-11 · tanda 16 de la Fase 2 (en `master`). `ServerConnectionForm.tsx` y `SqlServerConnectionForm.tsx`: Servidor y Usuario (este, solo cuando hace falta) con `required` y la marca «(obligatorio)» por CSS con texto alternativo vacío (no cambia el nombre accesible ni rompe `getByLabel("Servidor")`); un único `<p role="status">` siempre montado con el motivo, y `aria-describedby` hacia él en el campo que falta (servidor, puerto o usuario); `aria-invalid` solo tras tocar el formulario o si Entregar señala el error. **Principal:** sin controlador ODBC no se marca ningún campo, y el puerto se detecta por su propia regla. Comprobado en el navegador (PostgreSQL con servidor y sin usuario: «Indica el usuario de PostgreSQL.» como descripción del campo). El grupo de claves de la tarea no existe en estos formularios: nada que marcar. No probado con NVDA ni Narrador; «Ver tablas» sigue con `disabled` real (fuera de esta tarea).

- [x] **AUD-051 · El `aria-label` de cada `<th>` de la vista previa tapa el perfil de columna que pinta `ColumnProfileHeader`**
  - **Área:** Accesibilidad · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src/features/review/ReviewPhase.tsx:707-714`
  - **Qué hacer:** quitar el `aria-label` y dejar que el nombre sea el contenido, con el perfil dentro de un `<span class="visually-hidden">` etiquetado como «Perfil:» ; o conservar el nombre y añadir `aria-describedby` hacia el bloque del perfil, que se anuncia una sola vez por columna sin repetirse en las celdas. Como `DESIGN.md:129-131` lo declara a propósito, arreglarlo es revisar esa decisión; si se mantiene, basta con dejarlo escrito en `DESIGN.md` y cerrar la tarea sin cambiar código.
  - **Criterio de aceptación:** con NVDA o Narrador, al situarse en una cabecera de columna se oyen nombre, tipo y la línea de completitud y distintos.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-11 · tanda 16 de la Fase 2 (en `master`). el `<th>` conserva el nombre corto (nombre y tipo) y gana `aria-describedby` hacia su bloque `.col-profile` (id por posición con `useId`); `DESIGN.md` describe la decisión. Descripción calculada por el navegador: «sin vacíos · todos distintos Distribución: de 10 a 20, la mayoría entre 15 y 20. 10 – 20». No probado con NVDA ni Narrador.

- [x] **AUD-052 · El histograma de la cabecera no tiene equivalente textual y su contraste calculado es de 2,2-2,4:1 en los temas claros**
  - **Área:** Accesibilidad · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src/data-views.css:33`
  - **Qué hacer:** subir a `color-mix(… 80%)` (3,3-3,8:1) y, o bien un `<title>` con «Distribución: x–y, la mayoría entre …», o una línea de texto con el cuantil más poblado.
  - **Criterio de aceptación:** el relleno da al menos 3:1 contra la cabecera en los cinco temas y la forma tiene una descripción corta accesible.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-11 · tanda 16 de la Fase 2 (en `master`). relleno al 80 % (`data-views.css`): 3,60 (claro), 3,52 (paper), 3,45 (ocean), 3,30 (slate) y 5,70:1 (oscuro) contra el fondo de la cabecera; antes 2,15-2,28 en los claros. `histogramDescription` añade un texto oculto («Distribución: de x a y, la mayoría entre a y b») dentro del bloque del perfil, que entra en la descripción de AUD-051; el SVG sigue `aria-hidden`. El par no está en `tools/contrast-table.mjs`.

- [x] **AUD-053 · Semántica menor: `aria-label` en `div` sin rol, nombres repetidos en el bloque «Historial» y saltos de nivel de encabezado en el cajón SQL**
  - **Área:** Accesibilidad · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src/features/prepare/HistoryBar.tsx:81`
  - **Qué hacer:** `role="region"` (o `<section>`) donde hay `aria-label` y foco por teclado; un solo nombre para el `aside` (el `section` interior puede quitar su `aria-label`); `LocalQueryPanel standalone` con `h3` y sin repetir el título.
  - **Criterio de aceptación:** el árbol de accesibilidad de Preparar muestra un único landmark de historial y el cajón SQL una jerarquía h2 → h3.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-11 · tanda 16 de la Fase 2 (en `master`). el `aside` de Preparar se llama «Historial de cambios» y el bloque de `HistoryBar` deja de ser `section` con nombre; «Uso y retención» y el resultado del último cambio son `role="group"`, y la tabla «Antes y después» `role="region"`. `LocalQueryPanel standalone` usa h3/h4 (h4/h5 fuera del cajón); **principal:** los selectores de `styles.css` atados a `h4`/`h5` pasan a `:is(...)`, para que el cajón se vea igual. Comprobado en el navegador: un único `complementary` de historial, ninguna región con «historial», y h2 «Consulta SQL» → h3. Quedan `div` con `aria-label` sin rol en Revisar y en `TransformRecipeEditor` (sin tarea).

- [x] **AUD-054 · Textos con elipsis y solo `title` para ver el valor completo**
  - **Área:** Accesibilidad · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src/shell.css:173-180`
  - **Qué hacer:** permitir dos líneas (`line-clamp: 2` o `overflow-wrap: anywhere`) en las claves, y mostrar el nombre del archivo completo con `overflow-wrap: anywhere` en el cajón/Revisar.
  - **Criterio de aceptación:** a 320 CSS px se lee el nombre completo de un dataset y de las columnas clave sin pasar el ratón.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-11 · tanda 16 de la Fase 2 (en `master`). `.dataset-bar__name`, `.comparison-key-options strong`, las claves del script SQL Server y el nombre de los recientes cambian la elipsis por `overflow-wrap: anywhere` con `min-width: 0` (el `title` se conserva). Comprobado en el navegador a 320 px con un nombre de 73 caracteres: se lee entero, sin recorte ni desplazamiento horizontal. Las columnas clave a 320 px no se miraron en el navegador (prueba de estilos).

- [x] **AUD-055 · A 320 CSS px, Entregar y Preparar se salen de la pantalla y las etapas se parten a media palabra**
  - **Área:** Accesibilidad · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src/shell.css:291`
  - **Qué hacer:** `grid-template-columns: minmax(0, 1fr)` explícito en la rama de una columna de `.delivery-layout`, `.prepare-layout` y `.database-import`; `min-width: 0` y `flex-wrap` en los `fieldset` de Preparar; `overflow-wrap: normal` (o `hyphens: auto` con `lang="es"`) en las etapas, o etiquetas abreviadas por debajo de 400 px; `@media (max-width: 480px) { .connection-form__row--server { grid-template-columns: minmax(0, 1fr) } }`.
  - **Criterio de aceptación:** a 320 CSS px, `document.documentElement.scrollWidth` es igual a `clientWidth` en las cinco etapas (también en Entregar con SQL Server › Actualizar por clave), ninguna etapa parte una palabra y el campo Servidor mide al menos 200 px o va en su propia fila.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-11 · tanda 17 de la Fase 2 (en `master`). `minmax(0, 1fr)` en la base de `.prepare-layout` y `.database-import` (`.delivery-layout` ya lo tenía); `min-width: 0` en los `fieldset` de Preparar (`.text-cleaning-options`, `.transform-recipe`); las etapas pasan de `overflow-wrap: anywhere` a `normal` y, por debajo de 400 px, letra de 11 px con menos relleno; `.connection-form__row--server` a una columna por debajo de 480 px. Comprobado en el navegador (mock de Tauri, dataset de 14 columnas y nombre de 84 caracteres) a 320 px: `scrollWidth` igual a `clientWidth` en las cinco etapas, en Cargar › Base de datos y en Entregar › SQL Server › Actualizar por clave; ninguna etiqueta de etapa ocupa más de una línea ni desborda; el campo Servidor mide al menos 200 px. Prueba de estilos `ResponsiveLayoutCss.test.ts`.

### Diseño responsivo y UI/UX

- [x] **AUD-058 · Alturas fijadas a mano (88 px, 260 px) que dependen de una barra superior de alto variable**
  - **Área:** UI/UX · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src/shell.css:346`
  - **Qué hacer:** una variable `--topbar-height` actualizada con `ResizeObserver` (o `scroll-padding` calculado en JS) y usarla en los tres sitios.
  - **Criterio de aceptación:** a 1000×800 con un archivo de nombre largo, tabular por cualquier campo no lo deja bajo la barra ni bajo el resumen de Entregar.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-11 · tanda 17 de la Fase 2 (en `master`). `useTopbarHeight()` publica la altura real de la barra como `--topbar-height` con `ResizeObserver` (88 px de reserva en `:root`); la usan `scroll-padding-top` y el `top`/`max-height` del panel lateral pegajoso de Preparar. Comprobado en el navegador a 1000×800 con un nombre de 84 caracteres: la barra mide 118 px (antes se suponían 88) y, tabulando 60 veces en Preparar y en Entregar, ningún control con foco queda bajo ella. El `scroll-padding-bottom: 260px` del resumen de Entregar no depende de la barra y el resumen solo es pegajoso desde 1180 px: sin cambio.

- [ ] **AUD-059 · Las banderas de ocupado desactivan 56 controles sin decir por qué; «Guardar proyecto» es el caso más visible**
  - **Área:** UI/UX · **Severidad:** Bajo · **Esfuerzo:** Medio
  - **Ubicación:** `src/App.tsx:1418-1426`
  - **Qué hacer:** cambiar el estado a «Sin guardar · disponible al terminar el análisis» (o «Guardando…» / «Esperando…») según el motivo de `disabled`. Para el patrón: un solo texto de motivo derivado de las banderas (por ejemplo, junto a `busyReasons` de `App.tsx:1121`, que hoy solo llega a un atributo `data-busy-reasons`), que cada grupo muestre junto al control o asocie con `aria-describedby`.
  - **Criterio de aceptación:** mientras el botón esté desactivado, la barra superior dice por qué. Cada grupo de controles de la lista muestra o asocia el motivo mientras está desactivado.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Avance:** 2026-11-11 · tanda 17 de la Fase 2 (en `master`). Primera frase del criterio cumplida: con «Guardar proyecto» desactivado, la barra dice «Sin guardar · disponible al terminar el análisis de calidad» (o la operación que toque), y el botón lo asocia con `aria-describedby`. `src/busyReasons.ts` deriva un solo motivo de las banderas (las mismas de `data-busy-reasons`, que no cambia), en un nodo `#busy-reason` sin rol vivo (cambia en cada carga de página), al que apuntan las etapas, «Volver a…» y «Continuar…». **Queda abierta:** los grupos de controles de `LoadPhase`, `DatabaseImportPanel`, `ReusableTaskPanel`, `ProjectsPanel`, los formularios de conexión, `SqlServerScriptTarget`, `PrepareProposal` y Revisar no asocian todavía el motivo; y en los botones con `disabled` real el `aria-describedby` no lo lee un lector de pantalla al tabular (habría que pasar los críticos a `aria-disabled`, como en AUD-049). No se comprobó en el navegador (el mock no deja el análisis en curso).

- [x] **AUD-060 · El motivo del botón «Exportar» omite dos de sus bloqueos**
  - **Área:** UI/UX · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src/features/delivery/DeliveryPhase.tsx:325-335`
  - **Qué hacer:** añadir a `exportBlockers` `!databaseReady && "analiza la compatibilidad con el destino"` y `exceedsExcel && "elige otro formato: el dataset no cabe en Excel"`.
  - **Criterio de aceptación:** cada condición que desactiva el botón principal produce una frase bajo el botón; hay una prueba que recorre las combinaciones.
  - **Depende de:** ninguna; hacerla junto con AUD-049 (mismo botón)
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-11 · tanda 17 de la Fase 2 (en `master`). Las condiciones y sus frases pasan a `exportBlockers()` (`src/features/delivery/exportBlockers.ts`), y el `aria-disabled` del botón principal se deriva de esa lista, así que no se puede añadir un bloqueo sin frase. Nuevas: «analiza la compatibilidad con el destino», «elige otro formato: el dataset no cabe en Excel» y «espera a que termine la operación en curso» (`busy`, visto en la tanda 16). La prueba recorre las 128 combinaciones de las 7 condiciones.

- [x] **AUD-061 · Cuando la importación es parcial (límite de filas, textos recortados), el aviso solo se ve en Cargar**
  - **Área:** UI/UX · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src/features/load/DatabaseImportPanel.tsx:233-244`
  - **Qué hacer:** guardar en el dataset (o en el nombre) «primeras 100.000 de la tabla» y los textos recortados, mostrarlo en la barra superior y como línea del resumen de Entregar («Origen: primeras 100.000 filas de dbo.Ventas»).
  - **Criterio de aceptación:** tras importar con límite y llegar a Entregar, el resumen dice que el dataset es una parte de la tabla; tras reabrir el proyecto sigue diciéndolo. Tras importar una tabla con un texto de 9.000 caracteres, Entregar muestra «Columnas con textos recortados: Notas».
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-11 · tanda 17 de la Fase 2 (en `master`). Esfuerzo real Medio: con la aprobación del usuario, el catálogo de proyectos sube a la versión 17 (columna `import_origin_json`, migración idempotente como la de UX-09, con la copia `.bak` de siempre). `ImportOrigin` (tabla, filas, `partial` = `moreRowsAvailable`, columnas recortadas y con caracteres sustituidos) va de `DatabaseImportPanel` a `App` por su `selectionId`, se guarda en el workspace y se restaura con el proyecto y con sus versiones; se borra al cargar otro dataset y al unir, consolidar o resolver conflictos. La barra dice «Parte de la tabla: primeras 100.000 filas de dbo.Ventas» y el resumen de Entregar añade «Origen», «Columnas con textos recortados» y «Columnas con caracteres sustituidos». Un JSON dañado se descarta sin impedir abrir el proyecto. Comprobado en el navegador: tras guardar y abrir un proyecto con origen, la barra y Entregar lo dicen. El límite real de un servidor y el texto de 9.000 caracteres se comprobaron con pruebas (Rust y jsdom), no contra SQL Server.

- [x] **AUD-062 · Mientras se importa de una base de datos aparece «Esperando la selección y verificando el formato local…»**
  - **Área:** UI/UX · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src/App.tsx:729-730`
  - **Qué hacer:** que el aviso dependa de un origen («file» o «database») o se oculte mientras `DatabaseImportPanel` muestra su propio progreso.
  - **Criterio de aceptación:** durante una importación solo se ve el progreso de la importación.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-11 · tanda 17 de la Fase 2 (en `master`). El estado `inspecting` lleva `origin`; con `"database"`, Cargar no muestra «Esperando la selección y verificando el formato local…». Comprobado en el navegador con un `MutationObserver` durante la importación de `dbo.Ventas`: el aviso no aparece en ningún momento.

- [x] **AUD-063 · Los paneles «Preferencias y recursos» y «Licencia y privacidad» (ventanas de más de 900 px) no se cierran con Esc ni con clic fuera, y bloquean el scroll de la página**
  - **Área:** UI/UX · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src/styles.css:1411-1432`
  - **Qué hacer:** `keydown` de Esc y clic fuera cierran el panel y devuelven el foco al `summary`; o convertirlos en `ModalDialog variant="drawer"`.
  - **Criterio de aceptación:** Esc cierra ambos paneles y devuelve el foco a su botón.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-12 · tanda 18 de la Fase 2 (en `master`). `useDismissablePanel` (`src/useDismissablePanel.ts`) en los dos `<details>` de la barra lateral: Esc cierra el panel y devuelve el foco a su `summary`; un clic fuera lo cierra y solo devuelve el foco si estaba dentro del panel y el clic cayó en algo no enfocable. El bloqueo del scroll (`html:has(details[open])`, panel `position: fixed`) se conserva: con Esc y clic fuera deja de ser una trampa. Comprobado en el navegador a 1280 px con los dos paneles (Esc, foco, clic en el área de trabajo, scroll liberado al cerrar; al abrir uno con el otro abierto queda solo el segundo). Pruebas en `App.test.tsx`. No comprobado: NVDA ni Narrador.

- [x] **AUD-064 · Textos que mandan a «Continuar un proyecto», una sección que el rediseño de Cargar eliminó**
  - **Área:** UI/UX · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src/features/load/LoadPhase.tsx:192`
  - **Qué hacer:** cambiar los dos textos por «en la pestaña Proyecto de más abajo» (o «en la tarjeta Proyecto») y actualizar `CONTEXTO.md:295`.
  - **Criterio de aceptación:** ningún texto de la interfaz ni de la documentación viva nombra un control que no existe; el texto de recuperación coincide con la etiqueta de la tarjeta.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-12 · tanda 18 de la Fase 2 (en `master`). Los dos avisos de Cargar dicen «en la tarjeta «Proyecto» de más abajo» (el control es una tarjeta de opción, no una pestaña); `CONTEXTO.md` y cuatro menciones de `docs/how-to/` (`usar-columnia.md`, `respaldar-proyectos.md`) dicen «la tarjeta **Proyecto** de Cargar». Sin menciones restantes de «Continuar un proyecto» en `src/`, `e2e/`, `README.md` ni la documentación viva.

- [x] **AUD-065 · En la vista compacta desaparece la etiqueta «Revisar de nuevo», y bajo 700 px también los números y las marcas ✓**
  - **Área:** UI/UX · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src/shell.css:376-377`
  - **Qué hacer:** en compacto, mostrar un punto o un subrayado discontinuo junto al nombre de «Revisar» cuando haga falta revisarla de nuevo, y conservar el ✓ de las completadas; ajustar `DESIGN.md` si se decide no hacerlo.
  - **Criterio de aceptación:** a 900 px, tras cambiar datos en Preparar, la etapa Revisar se distingue visualmente de una no visitada.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-12 · tanda 18 de la Fase 2 (en `master`). Solo CSS (`shell.css`): hasta 900 px la etiqueta sigue oculta y la sustituyen un punto de 6 px en la esquina y un subrayado discontinuo bajo el nombre, en `--accent` (unos 9:1 sobre la barra); bajo 700 px el ✓ de las completadas queda como marca en la esquina, sin restar anchura. Las mismas reglas con zoom al 200 % y `CanvasText` en `forced-colors`. `DESIGN.md` y `WorkspaceShellStyles.test.ts` actualizados. Comprobado en el navegador a 900, 699 y 320 px (punto, subrayado, ✓ y sin desbordamiento horizontal), reproduciendo el marcado del estado «Revisar de nuevo» sobre la app cargada; no se llegó al estado mutando datos en Preparar con el simulador.

- [x] **AUD-066 · El error del motor solo se ve como una línea pequeña en la barra superior, y Cargar sigue diciendo «desde la aplicación de escritorio»**
  - **Área:** UI/UX · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src/App.tsx:1390-1401`
  - **Qué hacer:** un `notice--error` en Cargar con el mensaje y los pasos («Cierra y vuelve a abrir Columnia; si se repite, crea un informe de diagnóstico en Preferencias»).
  - **Criterio de aceptación:** con `get_app_info` fallando, Cargar muestra el motivo y la acción.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-12 · tanda 18 de la Fase 2 (en `master`). `LoadRuntimeState` lleva el mensaje en `{ kind: "unavailable", error }`; con el motor en error, Cargar muestra un `notice--error` («El motor de Columnia no responde: …» y «Cierra y vuelve a abrir Columnia. Si se repite, crea un informe de diagnóstico en Preferencias.»), y «desde la aplicación de escritorio» queda solo para la vista previa en navegador. Comprobado en el navegador con `get_app_info` fallando. Pruebas en `App.test.tsx`. Queda como observación: el error se anuncia dos veces (`role="alert"` en la barra y en Cargar).

- [x] **AUD-067 · Un valor ausente y una celda con la palabra «vacío» se ven casi iguales**
  - **Área:** UI/UX · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src/components/MissingValue.tsx:9`
  - **Qué hacer:** rodear el nulo de un marcador no textual (por ejemplo «∅ vacío» o «(vacío)» con borde tenue) y `title="Sin valor"`.
  - **Criterio de aceptación:** una celda de texto «vacío» y un nulo se distinguen sin depender solo del color.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-12 · tanda 18 de la Fase 2 (en `master`). `MissingValue` pinta «∅ vacío» dentro de una caja con borde `--border-subtle` y `title="Sin valor"`; el símbolo y la palabra son `aria-hidden` y el nombre accesible sigue siendo «valor ausente». Comprobado en el navegador (temas claro y oscuro): la fila con el nulo mide lo mismo que las demás (47,5 px) y una celda con el texto «vacío» no lleva marcador. Prueba nueva `MissingValue.test.tsx`.

- [ ] **AUD-068 · El perfil de las cabeceras provoca un salto de diseño en la tabla**
  - **Área:** UI/UX · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src/features/review/ReviewPhase.tsx:148`
  - **Qué hacer:** reservar la altura final con un esqueleto del bloque `.col-profile` mientras carga.
  - **Criterio de aceptación:** al llegar el perfil, la primera fila de datos no cambia de posición vertical.
  - **Depende de:** ninguna; hacerla junto con AUD-044 (misma cabecera)
  - **Origen:** auditoría 2026-11-05

- [ ] **AUD-069 · La barra lateral es oscura en todos los temas, y ni `DESIGN.md` ni el comentario de `shell.css` lo dicen**
  - **Área:** UI/UX · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src/workflow-styles.css:525`
  - **Qué hacer:** documentar en `DESIGN.md` que la barra es siempre oscura, con sus tokens, y añadir sus pares a la tabla; o quitar el token claro muerto.
  - **Criterio de aceptación:** `DESIGN.md` describe la paleta real de la barra en cada tema.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05

- [ ] **AUD-070 · Las etiquetas de las barras de Explorar se cortan con «…» sin mostrar el valor completo**
  - **Área:** UI/UX · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src/features/explore/explore.css:38-45`
  - **Qué hacer:** `title={valueLabel(bar.value)}` en el botón o en la etiqueta, y/o permitir 2 líneas en la etiqueta.
  - **Criterio de aceptación:** pasar el cursor por una etiqueta cortada muestra su valor entero.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05

- [ ] **AUD-071 · Franja de cifras de Explorar: con un número de cifras que no llena la última fila, las celdas vacías se pintan del color del borde**
  - **Área:** UI/UX · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src/features/explore/explore.css:21-26`
  - **Qué hacer:** separadores con `border-left` en cada cifra en lugar del fondo del contenedor, o `grid-auto-flow: dense` con un relleno.
  - **Criterio de aceptación:** a 450 CSS px con 3 cifras no se ve ningún bloque gris.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05

- [ ] **AUD-072 · `DESIGN.md` no describe partes del rediseño que sí se construyeron**
  - **Área:** UI/UX · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `DESIGN.md:113-134`
  - **Qué hacer:** añadir tres párrafos (Preparar, Explorar, Cargar) y los puntos de corte de AUD-065.
  - **Criterio de aceptación:** `DESIGN.md` describe todas las pantallas del flujo y coincide con `shell.css`.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05

- [ ] **AUD-101 · El selector ofrece el controlador antiguo «SQL Server», que no puede conectar con el cifrado obligatorio**
  - **Área:** UI/UX · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src-tauri/src/remote_databases.rs:2850`
  - **Qué hacer:** no listar el controlador antiguo de Windows («SQL Server», `sqlsrv32`), o avisar al elegirlo. Traducir su error («Error de seguridad de SSL», SQLSTATE 08001) a «Este controlador no admite el cifrado; elige ODBC Driver 17 o 18».
  - **Criterio de aceptación:** con ODBC Driver 17 instalado, el selector no ofrece «SQL Server», o al elegirlo explica por qué no conecta. Hoy falla contra el SQL Server local con «Error de seguridad de SSL» aunque se marque «Aceptar cualquier certificado» (comprobado el 2026-11-06), porque no admite `TrustServerCertificate`.
  - **Depende de:** ninguna
  - **Origen:** Fase 2 de la auditoría 2026-11-05, al aplicar AUD-003

### QA y testing

- [ ] **AUD-078 · La comprobación de higiene del CSS no revisa `shell.css` ni `data-views.css`**
  - **Área:** QA y testing · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src/components/StylesheetHygiene.test.ts:7`
  - **Qué hacer:** construir la lista con todas las hojas de `src/` (por ejemplo, con `readdirSync` recursivo) en lugar de una lista escrita a mano, y corregir lo que salga.
  - **Criterio de aceptación:** la prueba recorre todas las hojas de `src/`; añadir una hoja nueva no obliga a tocar la prueba; con un token inexistente en `shell.css`, la prueba falla.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05

- [x] **AUD-079 · El presupuesto del bundle queda al 0,5 % y da un falso rojo si se compila sin `TAURI_ENV_PLATFORM`**
  - **Área:** QA y testing · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `tools/check-bundle.mjs:8-19`
  - **Qué hacer:** que `tools/check-bundle.mjs budget` avise cuando detecte el target `safari13` (o que `npm run build` fije la variable), y revisar qué pesa más en `index-*.js` (430 kB) antes del siguiente aumento.
  - **Criterio de aceptación:** `node tools/check-bundle.mjs budget` sobre un build manual da el mismo veredicto que `check.ps1`, o avisa de por qué no.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
  - **Cierre:** 2026-11-12 · tanda 18 de la Fase 2 (en `master`). `vite.config.ts` deja fuera de `dist` una marca con el target del build (`.local/validation/frontend-build-target.json`) y `tools/check-bundle.mjs budget` avisa (sin cambiar el veredicto) si falta, si es de otro directorio o si el target no es `chrome105`; el aviso va también a `warnings` de la evidencia. Build manual sin la variable: avisa y mide 948.515 bytes; con `TAURI_ENV_PLATFORM=windows`, sin aviso y 920.940, como `check.ps1`. Revisión del contenido: `index-*.js` son unos 429 kB, de los que `react-dom` ocupa 207 kB; lo propio que más pesa es `App.tsx` (34 kB), `LoadPhase.tsx` (32 kB) y `deliveryModel.ts` (22 kB, importado entero por `App.tsx`). Reducción posible sin cambiar dependencias: unos 60-80 kB (ver AUD-116).

- [ ] **AUD-116 · `index-*.js` carga de entrada código que solo usan Entregar, los paneles secundarios de Cargar y los diálogos**
  - **Área:** QA y testing · **Severidad:** Bajo · **Esfuerzo:** Medio
  - **Ubicación:** `src/App.tsx:6`
  - **Qué hacer:** antes de volver a subir el presupuesto del bundle: (1) que `App.tsx` no importe `deliveryModel.ts` entero (unos 22 kB) por `excelLimitIssues`, `personalDataColumnNames` y `suggestQualityRules`; (2) cargar a demanda `DatabaseImportPanel`, `ReusableTaskPanel` y `ProjectsPanel` (unos 27 kB); (3) lo mismo con `DiagnosticsDialog`, `UpdatePanel` y `ResourceMonitor` (unos 15 kB). `react-dom` (207 kB) no se toca.
  - **Criterio de aceptación:** `index-*.js` baja al menos 40 kB sin comprimir con `TAURI_ENV_PLATFORM=windows`, la suite y Playwright pasan, y Cargar no muestra un parpadeo al abrir sus tarjetas.
  - **Depende de:** ninguna
  - **Origen:** Fase 2 de la auditoría 2026-11-05, al aplicar AUD-079 (medición con sourcemap del 2026-11-12, cifras aproximadas)

- [ ] **AUD-080 · La expulsión de la caché de perfil nunca se prueba: capacidad 64 en pruebas y 1 en producción**
  - **Área:** QA y testing · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src-tauri/src/dataset/profile_engine.rs:2534`
  - **Qué hacer:** hacer la capacidad un parámetro de `remember_profile_snapshot` (o una función `capacity()` que una prueba pueda fijar a 1) y probar la expulsión con dos orígenes, comprobando que quien ya tenía un `CachedProfileSnapshot` sigue leyéndolo.
  - **Criterio de aceptación:** una prueba con capacidad 1 pasa y falla si se rompe la expulsión o el conteo del `Arc`.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05

- [ ] **AUD-081 · El simulador E2E devuelve resultados sin tipar para los comandos nuevos y no hay E2E de entrega a SQL Server ni de importación desde PostgreSQL/MySQL**
  - **Área:** QA y testing · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `e2e/support/tauri-mock.ts:149-175`
  - **Qué hacer:** tipar los resultados con los tipos de `src/bridge` (`satisfies DatabaseImportResult`, `DatabaseSourceTable[]`) y añadir un E2E que elija el destino SQL Server y compruebe que `export_dataset` recibe `sqlServer` con modo y claves.
  - **Criterio de aceptación:** cambiar un campo de `DatabaseImportResult` rompe `tsc -p tsconfig.e2e.json`; existe un E2E de entrega a SQL Server.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05

- [ ] **AUD-082 · Esperas reales y topes de tiempo inflados en pruebas, con dos commits de estabilización en tres semanas**
  - **Área:** QA y testing · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src/components/ResourceMonitor.test.tsx:95`
  - **Qué hacer:** sustituir las esperas reales por `vi.useFakeTimers()` (Vitest) y por `expect(...).toBeVisible()`/`toHaveCSS` con auto-espera (Playwright); fijar `testTimeout` global en lugar de un tope por prueba.
  - **Criterio de aceptación:** ninguna espera fija en `src/**/*.test.*` ni `e2e/*.spec.ts`; el `pre-push` pasa dos veces seguidas con el equipo cargado.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05

- [ ] **AUD-083 · Calidad de algunas pruebas nuevas: regex sobre CSS, una tabla de verdad y salida de depuración**
  - **Área:** QA y testing · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src-tauri/src/dataset/tests.rs:19661-19674`
  - **Qué hacer:** convertir las más importantes (zoom 200 %, `max-width: 900px`) en comprobaciones de Playwright con medidas, o aceptar que son guardas de texto y dejarlas marcadas; quitar el `println!`.
  - **Criterio de aceptación:** las reglas de layout críticas del shell tienen al menos una comprobación medida en E2E.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05

- [x] **AUD-099 · El generador de `THIRD_PARTY_NOTICES.md` no ordena: cada cambio de un lockfile reordena el archivo entero**
  - **Área:** QA y testing · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `tools/generate-third-party-notices.ps1:111`
  - **Qué hacer:** construir las filas como `[pscustomobject]@{…}` en lugar de `[ordered]@{…}`: con diccionarios, `Sort-Object ecosystem, name, version, source` no ordena por clave (comprobado en Windows PowerShell 5.1 y en PowerShell 7), así que el orden depende de la agrupación. Regenerar el archivo una vez y añadir una prueba que compruebe que la tabla sale ordenada.
  - **Criterio de aceptación:** regenerar dos veces seguidas no cambia nada, y tras subir un solo paquete solo cambian sus filas; la tabla queda ordenada por ecosistema, nombre y versión. (Al subir `calamine` en AUD-004, 7 filas de contenido dieron unas 590 líneas de diff.)
  - **Depende de:** ninguna
  - **Origen:** Fase 2 de la auditoría 2026-11-05, al aplicar AUD-004
  - **Cierre:** 2026-11-11 · tanda 11 de la Fase 2 (en `master`). Filas como `[pscustomobject]` y orden ordinal (`[string]::CompareOrdinal` por ecosistema, nombre, versión y fuente), igual en Windows PowerShell 5.1 y en PowerShell 7 (salidas idénticas byte a byte; dos regeneraciones sin cambios). `THIRD_PARTY_NOTICES.md` regenerado una vez: mismo conjunto de filas, solo cambia el orden. Prueba nueva en `src/supply-chain.test.ts` (tabla ordenada y sin duplicados; falla con el archivo anterior). Las versiones se ordenan como texto.

- [ ] **AUD-113 · Las estructuras de conexión y entrega a bases de datos siguen fuera de las pruebas de paridad del IPC**
  - **Área:** QA y testing · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `docs/reference/ipc-inventory.json` (`sharedStructures`), `tools/check-ipc-inventory.mjs` y `src/ipc-contract.test.ts` (`SHARED_ENUMS`)
  - **Qué hacer:** inscribir `DatabaseConnection`, `DatabaseTarget`, `DatabaseConnectionResult` y `RemoteExportPreflight` en las estructuras compartidas, y `DatabaseTablePolicy` y `DatabaseAuthentication` en los enums, como se hizo en AUD-077. Regenerar el inventario con `node tools/check-ipc-inventory.mjs --write` en lugar de editar el JSON a mano (el JSON y las listas del generador tienen que coincidir).
  - **Criterio de aceptación:** renombrar un campo de `DatabaseTarget` o una variante de `DatabaseAuthentication` rompe `npm test`.
  - **Depende de:** ninguna
  - **Origen:** Fase 2 de la auditoría 2026-11-05, al aplicar AUD-077

### Ortografía y redacción

- [ ] **AUD-084 · Cuatro nombres distintos para los marcadores que Columnia convierte en vacíos**
  - **Área:** Redacción · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src/components/ColumnProfileHeader.tsx:11`
  - **Qué hacer:** un único término para el marcador (por ejemplo «sin dato», que ya es el más usado en la propuesta) y otro para el nulo real («nulo» o «celda sin valor», sin mezclar). Aplicarlo en el perfil, la propuesta y el texto de `PreparePhase.tsx`. Documentarlo en `docs/reference/glosario.md`.
  - **Criterio de aceptación:** en las pantallas de Revisar y Preparar, un mismo concepto aparece con un solo nombre; el glosario lo define.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05

- [ ] **AUD-085 · La consulta SQL tiene tres nombres, y uno de ellos es el de la fase Explorar**
  - **Área:** Redacción · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src/features/review/ReviewPhase.tsx:633`
  - **Qué hacer:** llamarla «Consulta SQL» en los tres sitios (en el botón de Revisar, en el panel y en el encabezado) y reservar «Explorar» solo para la fase. Si el botón de Revisar se mantiene, su texto debe ser «Consulta SQL».
  - **Criterio de aceptación:** la palabra «Explorar» solo aparece como nombre de fase; la consulta tiene un único nombre en toda la interfaz.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05

- [ ] **AUD-086 · Redacción confusa o desigual en ayudas de campos y en el LEEME del paquete BULK INSERT**
  - **Área:** Redacción · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src/features/delivery/SqlServerScriptTarget.tsx:104`
  - **Qué hacer:** - «Si los dejas vacíos, se usan `dbo` y el nombre del archivo que guardes.» - Campo de base de datos, en los tres motores: «Si lo dejas vacío, se usa la base por defecto del usuario.» (y en MySQL, mantener su variante con su sentido propio). - LEEME: «En BULK INSERT, una celda vacía y un texto vacío se cargan como NULL; si necesitas distinguirlos, exporta el script T-SQL.»
  - **Criterio de aceptación:** las tres ayudas se leen completas y usan la misma forma para el mismo campo.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05

- [ ] **AUD-087 · El resumen legal dice que Columnia no inicia conexiones de red por sí sola, pero no menciona la conexión de lectura de Cargar**
  - **Área:** Redacción · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src/App.tsx:1357`
  - **Qué hacer:** por ejemplo: «Columnia procesa los datos en este equipo. Solo se conecta a un servidor cuando lo eliges: para leer una tabla en Cargar (conexión de solo lectura, con confirmación) o para escribir en Entregar (exportación explícita). Solo esas exportaciones envían filas fuera de este equipo.»
  - **Criterio de aceptación:** el resumen menciona la conexión de lectura y la de escritura, y no contradice `THREAT_MODEL.md` (fila de importación ODBC).
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05

- [ ] **AUD-088 · El mensaje de tiempo de espera usa «lote» y «sentencia», términos internos**
  - **Área:** Redacción · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src-tauri/src/remote_databases.rs:1989`
  - **Qué hacer:** «…la base remota no respondió a tiempo (límites: 15 s para conectar, 30 s por bloque de filas y 300 s por consulta)…».
  - **Criterio de aceptación:** el mensaje no contiene «lote» ni «sentencia» y los tres límites se entienden sin leer el código.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05

### Documentación

- [ ] **AUD-089 · El modelo de amenazas dice que `style-src` permite `unsafe-inline`, y solo ocurre en desarrollo**
  - **Área:** Documentación · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `THREAT_MODEL.md:80`
  - **Qué hacer:** «En producción `style-src` es solo `'self'`. `'unsafe-inline'` solo figura en la CSP de desarrollo (`devCsp`), para Vite.»
  - **Criterio de aceptación:** la fila distingue producción y desarrollo y coincide con `tauri.conf.json`.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05

- [ ] **AUD-090 · El modelo de amenazas dice que la importación comparte la validación de cadena de la entrega, sin aclarar el cifrado**
  - **Área:** Documentación · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `THREAT_MODEL.md:85`
  - **Qué hacer:** «Misma validación de cadena que la entrega en FileDSN y `Driver=` (controlador instalado). A diferencia de la entrega, la importación no comprueba la política de cifrado (ver AUD-008).»
  - **Criterio de aceptación:** la fila no atribuye a la importación la comprobación de cifrado, y enlaza el hallazgo de área 2.
  - **Depende de:** AUD-008 (la frase depende de si la importación aplica la política de cifrado)
  - **Origen:** auditoría 2026-11-05

- [ ] **AUD-091 · El README dice que Cargar trae tablas solo de SQL Server**
  - **Área:** Documentación · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `README.md:19`
  - **Qué hacer:** «…o trae una tabla o vista de SQL Server, PostgreSQL o MySQL, en solo lectura.»
  - **Criterio de aceptación:** la lista del README coincide con los motores que acepta Cargar.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05

- [ ] **AUD-092 · El lema del README promete que los datos no salen del equipo, y la exportación ODBC sí puede sacarlos**
  - **Área:** Documentación · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `README.md:4`
  - **Qué hacer:** «…sin sacar los datos de tu equipo salvo que tú los envíes a una base de datos.»
  - **Criterio de aceptación:** el lema no contradice el texto legal de la aplicación ni el modelo de amenazas.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05

- [ ] **AUD-093 · El README no indica cómo ejecutar las pruebas Rust ni el gate oficial**
  - **Área:** Documentación · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `README.md:112-119`
  - **Qué hacer:** añadir en el README: «Validación local: `.\tools\check.ps1 -Profile Fast`; para cambios en Rust, `-Profile Full` y `cargo test --manifest-path src-tauri/Cargo.toml --lib`. Detalle en CONTRIBUTING.md.»
  - **Criterio de aceptación:** el README indica un comando de validación que cubre Rust y frontend, y enlaza CONTRIBUTING.md.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05

- [ ] **AUD-094 · El DESIGN.md da 640 × 400 como ventana mínima, pero la aplicación tiene 900 × 640**
  - **Área:** Documentación · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `DESIGN.md:180`
  - **Qué hacer:** «…también en la ventana mínima de 900 × 640 (UX-02).»
  - **Criterio de aceptación:** la cifra de `DESIGN.md` coincide con `minWidth` y `minHeight` de `tauri.conf.json`.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05

- [ ] **AUD-095 · La ayuda de `project-export` no lista los formatos T-SQL que sí acepta**
  - **Área:** Documentación · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src-tauri/src/automation.rs:27`
  - **Qué hacer:** añadir `sqlserver-script|sqlserver-bulk` a `PROJECT_EXPORT_HELP` y, si es posible, generar la lista desde el mismo origen que el parser.
  - **Criterio de aceptación:** la ayuda, `cli.md` y el parser listan los mismos formatos; un test lo comprueba.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05

- [ ] **AUD-096 · El JSDoc de `nonFiniteColumns` quedó encima de `profileDuration`**
  - **Área:** Documentación · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `src/features/review/reviewModel.ts:105-108`
  - **Qué hacer:** mover el bloque a `nonFiniteColumns` (`:137`) y dar a `profileDuration` su propio comentario de una línea (formato de duración: menos de 1 s, segundos, o minutos y segundos).
  - **Criterio de aceptación:** cada función tiene su JSDoc y ninguno describe otra.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05

- [ ] **AUD-097 · Las notas de `[Unreleased]` nombran tres veces la tarjeta de Cargar, de tres formas distintas**
  - **Área:** Documentación · **Severidad:** Bajo · **Esfuerzo:** Bajo
  - **Ubicación:** `CHANGELOG.md:16`
  - **Qué hacer:** consolidar en una sola entrada final sobre la tarjeta «Base de datos» y retirar las de «Cargar desde SQL Server» y «Archivo, SQL Server, Ejemplo o Proyecto».
  - **Criterio de aceptación:** ninguna entrada de `[Unreleased]` nombra la tarjeta de Cargar de forma distinta de la interfaz.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05

## Tier 4 · Propuestas (revisión 2026-11-05)

- [ ] **AUD-098 · Avisar en Entregar si la clave elegida para «actualizar por clave» tiene vacíos o repetidos**
  - **Área:** UI/UX · **Tipo:** Propuesta · **Retorno:** Medio · **Esfuerzo:** Bajo
  - **Ubicación:** `src/features/delivery/SqlServerScriptTarget.tsx:105-124`
  - **Qué hacer:** al marcar una clave, si el perfil dice que la columna (o la combinación) tiene vacíos o repetidos, mostrar bajo el recuadro «La columna X tiene N vacíos: el script se detendrá», y no bloquear.
  - **Criterio de aceptación:** al marcar como clave una columna con vacíos o repetidos según el perfil, aparece el aviso con el número de casos y la exportación no se bloquea.
  - **Depende de:** ninguna
  - **Origen:** auditoría 2026-11-05
<!-- revision-2026-11-05-tiers:fin -->

## Aparcado: solo si Columnia se comparte

Estos objetivos dejaron la cola porque la aplicación es de uso personal o
porque dependen de algo que su dueño no usa. Se retoman solo si eso cambia.

| ID | Criterio de cierre | Se retoma si |
| --- | --- | --- |
| RV07 — Beta | Tres participantes distintos sobre el mismo release candidate, dos casos reales por sesión y al menos tres datasets; 24 de 30 tareas sin ayuda; guardar, reabrir y entrega verificados; sin P0/P1; resumen sanitizado validado con `npm run beta:check-summary`. Cada fallo dependiente de datos se reduce a una fixture sintética con su regresión antes de corregirlo. Guía: [sesión beta](docs/how-to/run-beta-validation.md) e [invitación](docs/templates/beta-invitation.md). | Otras personas van a usarla |
| RV09 — Accesibilidad nativa | Cargar → Entregar en Windows solo con teclado y NVDA, incluidos diálogos, tablas, progreso, zoom y alto contraste. Guía: [probar con NVDA](docs/how-to/run-nvda-check.md). | La usa alguien con lector de pantalla |
| DAT-12 — «Crear» en MySQL | Ya hecho: si la entrega falla, se borra la tabla que creó. Falta la prueba `external_odbc_round_trip_mysql_with_and_without_no_backslash_escapes` con un servidor y un controlador MySQL reales (aparcado el 2026-10-09: su dueño no usa MySQL). SQL Server y PostgreSQL no lo necesitan. | Se entrega a MySQL |
| RV11 — Distribución | Aprobación jurídica, prueba en VM limpia (instalación, reapertura, updater, fallos y recuperación), hashes y firmas de los artefactos descargados, la sección `## [versión]` del CHANGELOG al cortar la versión, la firma Authenticode de los ejecutables (OPS-13) y los textos de licencia de terceros (LEG-01). | Se reparten binarios |

La publicación vigente autoriza **código fuente únicamente**. No autoriza
instaladores, updater público ni comercialización. ONAPI sigue siendo requisito
previo a comercializar.

## Fuera de la cola

Entran cuando su dueño las necesite en su trabajo: lotes gráficos, diccionario
de negocio, catálogos de equivalencias, reanudación avanzada de lotes,
vigilancia de carpetas, macOS/Linux y conectores nuevos.

Modularizar `dataset.rs` no es un objetivo: se extrae una responsabilidad cuando
se toca esa área, sin cambiar contratos.

## Criterio de salida de V1

Para uso personal no hace falta declarar una V1 soportada. Si Columnia se
comparte, V1 puede declararse soportada cuando RV07 y RV09 estén cerradas sobre
un candidato identificable; no queden defectos P0/P1; pasen la suite completa y
los gates de privacidad, seguridad y rendimiento; y, si se distribuyen binarios,
RV11 tenga autorización y evidencia reproducible.

## Cómo mantener este documento

- Una entrada por pendiente: qué está hecho y qué falta para cerrarlo. Nada de
  registros de sesiones.
- Al cerrar algo, se quita de aquí y se describe en `CHANGELOG.md`.
- Una revisión nueva añade su propia sección de pendientes; cuando queda
  aplicada, el documento completo se archiva en `docs/archive/` y aquí solo
  quedan los abiertos.
- Una decisión duradera de arquitectura o de trabajo va en `CONTEXTO.md`.
- Las tareas `AUD-NNN` siguen el formato de la auditoría (área, severidad,
  esfuerzo, ubicación, qué hacer, criterio, dependencias y origen). Al cerrar
  una, se quita de aquí, se describe en `CHANGELOG.md` con su ID y se
  actualizan los totales del índice de la revisión.
