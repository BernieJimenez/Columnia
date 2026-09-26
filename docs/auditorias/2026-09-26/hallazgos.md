# Hallazgos de trabajo — re-auditoría 2026-09-26

Archivo de trabajo: se escribe según aparecen los hallazgos. El informe consolidado está en `informe.md`.

**Alcance:** áreas 1 (corrección funcional), 2 (código), 3 (seguridad), 4 (datos y persistencia), 8 (UI/UX) y 10 (QA y testing). Exhaustiva en los flujos críticos; el resto, por muestreo declarado. Con la app corriendo y copias locales de datasets de trabajo (no versionadas).

**Flujos críticos:** (F1) cargar CSV/Excel · (F2) Preparar: imputación, correcciones recomendadas y transformaciones · (F3) deshacer/rehacer · (F4) guardar, reabrir y recuperar proyecto · (F5) Entregar: exportar a archivo y ODBC.

## Hallazgos

#### [FUN-01] Un CSV con fin de línea solo CR (Mac clásico) no se puede cargar, y el error culpa al tamaño
**Severidad:** Alto · **Confianza:** reproducido · **Esfuerzo:** medio · **Áreas:** Corrección funcional, UI/UX
**Ubicación:** `src-tauri/src/dataset/delimited_header_import.rs:42-52` (solo `b'\n'` cierra un registro); la lectura Polars posterior tampoco configura `eol_char`.
**Evidencia:** `capturas/02-carga-onlineretail.png`. `OnlineRetail.csv` (41,9 MiB, del escritorio del usuario) tiene 790 `\r` y 0 `\n` en los primeros 64 KiB. La app muestra: «No se pudo previsualizar el archivo: La primera fila supera la muestra segura de 64 KiB» y deja «Revisar esquema» deshabilitado.
**Problema:** los archivos con terminador `\r` (Excel «CSV (Macintosh)», exportaciones de sistemas antiguos) se leen como una sola línea.
**Impacto:** el flujo crítico F1 se bloquea para esos archivos sin salida dentro de la app, y el mensaje orienta a un problema de tamaño que no existe; la persona no puede saber que basta con convertir los saltos de línea.
**Cómo reproducirlo:** Seleccionar dataset → `OnlineRetail.csv` (o cualquier CSV con `\r` como único separador de línea).
**Solución propuesta:** en la muestra, detectar CR sin LF y (a) leer con `eol_char = b'\r'` o normalizar a `\n` en el snapshot de importación, o como mínimo (b) mostrar «El archivo usa saltos de línea de Mac clásico (CR)» en lugar del error de tamaño. Añadir una fixture sintética CR-only con su regresión.
**Criterio de aceptación:** un CSV CR-only se previsualiza y carga con las mismas filas que su equivalente LF; o, si se opta por (b), el mensaje nombra la causa real.

#### [FUN-02] Un CSV guardado por Excel en español (Windows-1252) no se puede abrir, y la app no ofrece salida
**Severidad:** Alto · **Confianza:** reproducido · **Esfuerzo:** medio · **Áreas:** Corrección funcional, UI/UX
**Ubicación:** `src-tauri/src/dataset.rs:6428-6441` (muestra: UTF-8 estricto) y `src-tauri/src/dataset.rs:6652,6669` (lectura completa).
**Evidencia:** `capturas/05-csv-ansi-espanol.png`: un CSV de 4 filas con `Población;Año;Importe (€)` en Windows-1252 (lo que produce Excel en Windows con «CSV (delimitado por comas)») muestra «El archivo delimitado no contiene UTF-8 válido. Columnia no sustituye caracteres ni aplica codificaciones heredadas automáticamente.» `capturas/04-esquema-onlineretail.png`: si el primer byte inválido está después de los 64 KiB de muestra (`OnlineRetail`, byte `0xA3` «£» en la línea 38.250 de 541.909), la vista previa pasa y el fallo llega en «Revisar esquema» con texto técnico en inglés: «…como UTF-8: invalid utf-8 sequence».
**Problema:** solo se acepta UTF-8 estricto (`CONTEXTO.md:343`). Es una capacidad documentada, no una decisión cerrada con motivo; y la negativa a *convertir automáticamente* es correcta, pero la app tampoco *propone* la conversión.
**Impacto:** para personas hispanohablantes en Windows es probablemente el primer archivo que intentan abrir. El mensaje dice lo que Columnia no hace, no lo que la persona puede hacer, y en el segundo caso filtra texto técnico del motor, contra el criterio de T10 de no mostrarlo.
**Cómo reproducirlo:** `printf 'Año;Población\r\n2022;59544\r\n' | iconv -t CP1252 > a.csv` y abrirlo.
**Solución propuesta:** detectar en la muestra una codificación heredada probable (Windows-1252/Latin-1 cuando falla UTF-8) y **proponer** «Este archivo parece Windows-1252 (Excel en Windows). ¿Leerlo convirtiéndolo a UTF-8?» con vista previa antes/después; la conversión va a la copia de trabajo y el original no se toca. Coherente con «Columnia propone y la persona aprueba». Como mínimo, traducir el error de `dataset.rs:6652/6669` y decir cómo resolverlo («guárdalo como CSV UTF-8 en Excel»).
**Criterio de aceptación:** el CSV de la evidencia se abre tras un clic de aprobación con `Año`, `Población` y `€` correctos; ningún mensaje de carga contiene texto del motor en inglés.

#### [FUN-03] La propuesta por defecto de Preparar rellena identificadores con la moda y falsea el dataset
**Severidad:** Crítico · **Confianza:** reproducido · **Esfuerzo:** medio · **Áreas:** Corrección funcional, Datos, UI/UX
**Ubicación:** `src/features/prepare/proposalModel.ts:34-40` (`isImputable`: cualquier columna de texto con algún valor repetido es imputable), `:94-106` (título y pista de la propuesta) y `:114-121` (`defaultProposalSelection` marca todo). El motor ya tiene una estrategia segura sin usar aquí: `categorical_imputation_uses_desconocido_without_touching_row_audit` (`src-tauri/src/dataset/tests.rs:14948`), que rellena con «Desconocido» y deja columna de auditoría; el test `imputes_repeated_text_and_lower_median_numeric_nuls_only` (`:11071`) fija el comportamiento actual.
**Evidencia:** `capturas/09-preparar-propuesta.png`, `10-antes-despues.png`, `12-tras-aplicar.png`; export `onlineretail_preparado.csv`. Con `OnlineRetail` (541.909 filas), la propuesta llega **marcada por defecto**; al aplicarla, los 135.037 `CustomerID` vacíos (compras sin cliente identificado, el 25 % del dataset) pasan a ser del cliente **17841**, que salta de 7.812 a **142.849** filas y del 0,4 % al **15,3 %** de los ingresos. Los 1.454 `Description` vacíos pasan a «WHITE HANGING HEART T-LIGHT HOLDER». La pantalla final lo presenta como éxito: «Valores vacíos 136,534 → 0».
Con el dataset sintético `hostil_20k.csv` la misma propuesta asigna el nombre **«José Ñúñez»** a los 2.222 registros sin nombre y la fecha «02/11/2026» a las 400 filas sin fecha (`capturas/20-hostil-propuesta.png`, export `hostil_preparado.csv`). Con `vmCloud_data.csv` (2.000.000 filas) propone rellenar los 200.666 `timestamp` vacíos con la marca de tiempo más frecuente.
**Problema:** la imputación trata un identificador como una medida. Rellenar un ID o una descripción con el valor más frecuente no «corrige» un vacío: fabrica un dato falso indistinguible de uno real. El «antes y después» solo dice «valor ausente → valor más frecuente», sin el valor ni el número de filas por columna.
**Impacto:** cualquier análisis por cliente o producto posterior queda falseado sin señal visible. Contradice la promesa del producto («datasets confiables») y va en el camino recomendado de un clic.
**Cómo reproducirlo:** cargar `OnlineRetail` (UTF-8, LF) → Ver cambios propuestos → Aplicar 4 cambios → exportar CSV → contar `CustomerID`.
**Solución propuesta:** no proponer imputación, o dejarla **desmarcada**, en columnas que parezcan identificadores (nombre `*id`, `*code`, `*no`, cardinalidad alta, formato de clave) o con un porcentaje de vacíos alto (> 5-10 %); para texto libre, no imputar. En el antes/después, mostrar por columna el valor concreto y el número de celdas que lo recibirán («CustomerID: 135.037 vacíos → 17841»). Alternativa segura: proponer «marcar como desconocido» (columna indicadora) en vez de rellenar.
**Criterio de aceptación:** con `OnlineRetail`, la propuesta por defecto no modifica `CustomerID` ni `Description`; una fixture sintética con una columna ID con vacíos lo cubre como regresión.

#### [FUN-04] Exportar a CSV antepone un apóstrofo a todos los números negativos guardados como texto
**Severidad:** Alto · **Confianza:** reproducido · **Esfuerzo:** bajo · **Áreas:** Corrección funcional, Datos, Seguridad
**Ubicación:** `src-tauri/src/dataset/csv_formula_safety.rs:5-18` (`-` y `+` al inicio disparan la neutralización sin mirar si el valor es un número); se aplica a toda columna `String` (`:33-50`).
**Evidencia:** el export de `OnlineRetail` contiene `'-1`, `'-12`… en **10.587** valores de `Quantity` (todas las devoluciones) y `'-11062.06` en `UnitPrice`. La carga CSV deja esas columnas como texto y la propuesta por defecto no convierte tipos, así que la premisa de `THREAT_MODEL.md:96` («sin modificar columnas tipadas») no se cumple en el flujo principal.
**Problema:** un número negativo no es una fórmula; neutralizarlo convierte una columna numérica en texto para cualquier consumidor (pandas, Power BI, `BULK INSERT`, Excel con el apóstrofo visible en la celda).
**Impacto:** corrupción silenciosa de datos numéricos en la exportación por defecto (CSV) de cualquier dataset con negativos: devoluciones, saldos, temperaturas, coordenadas.
**Cómo reproducirlo:** cargar un CSV con una columna `-1,-2.5` → Entregar → Exportar CSV → el archivo contiene `'-1`.
**Solución propuesta:** no neutralizar valores que sean un número válido completo (`^[+-]?\d+([.,]\d+)?([eE][+-]?\d+)?$`); mantener la neutralización para `=`, `@`, tabulador, CR/LF y para `+`/`-` seguidos de algo que no sea un número. Añadir el caso al test `csv_export_neutralizes_spreadsheet_formulas_and_parquet_preserves_values` (`dataset/tests.rs:8856`).
**Criterio de aceptación:** `-1`, `-11062.06` y `+3` se exportan tal cual; `-2+3`, `=1+1`, `@SUM(1)` y `+1+1` siguen neutralizados.

#### [FUN-06] Lo que anuncia la propuesta de Preparar no es lo que aplica
**Severidad:** Alto · **Confianza:** reproducido · **Esfuerzo:** medio · **Áreas:** Corrección funcional, UI/UX
**Ubicación:** `src/features/prepare/proposalModel.ts:94-99`: el total del anuncio suma los `nullCount` del perfil previo, sin contar los vacíos que crea el cambio «sentinels» de la misma propuesta; el motor aplica los cambios encadenados.
**Evidencia:** `hostil_20k.csv` → propuesta «Convertir 5,094 marcadores de «sin dato» en 2 columnas» + «Rellenar **5,528** valores vacíos en 5 columnas» (`capturas/20-hostil-propuesta.png`). Diferencia del export frente al origen:
- `categoria`: se rellenan **7.500** celdas (2.500 vacías + 5.000 `NULL`/`n/a` convertidas por el primer cambio), no las 2.500 vacías que contaba el anuncio; «A» pasa de 2.500 a 12.500 filas.
- `importe`: sus 301 vacíos (207 + 94 `N/A` convertidos) **siguen vacíos**, aunque figuraba entre las «5 columnas».
**Problema:** los recuentos se calculan sobre el dataset antes de aplicar los cambios anteriores de la misma propuesta, pero se aplican encadenados; además, una de las columnas anunciadas no se rellena.
**Impacto:** la persona aprueba un número y se aplica otro. Contradice el principio «Columnia propone, muestra el cambio y la persona aprueba»: lo que muestra no es lo que pasa.
**Cómo reproducirlo:** cargar `hostil_20k.csv` → Ver cambios propuestos → Aplicar 3 cambios → exportar y comparar con el origen.
**Solución propuesta:** calcular el antes/después y los recuentos **simulando la cadena completa** sobre el mismo plan que se ejecutará (o aplicar sobre una copia y enseñar el diff real antes de confirmar); test de contrato «recuento anunciado = celdas cambiadas» por cada cambio propuesto.
**Criterio de aceptación:** para `hostil_20k.csv`, la suma de celdas cambiadas por columna en el export coincide con lo anunciado cambio por cambio.

#### [FUN-07] El flujo por defecto nunca tipa las columnas numéricas: llegan como texto a todos los destinos
**Severidad:** Medio · **Confianza:** reproducido · **Esfuerzo:** medio · **Áreas:** Corrección funcional, Datos
**Ubicación:** carga CSV con perfil léxico (`ImportExceptionPolicy.baseline: "lexical"`, `src/bridge/reusable-task-contracts.ts:32`) + propuesta de Preparar sin conversión de tipos; `create_table_sql` (`src-tauri/src/remote_databases.rs:1513-1553`) mapea texto a `NVARCHAR(MAX)`.
**Evidencia:** entrega ODBC real a SQL Server local (`columnia_auditoria.dbo.hostil_auditoria`, 20.000 filas): las 7 columnas son `nvarchar(max)`, incluidas `id`, `importe` y `cantidad`. En CSV provoca FUN-04. Revisar sí detecta «Valores con tipo incompatible» (94 `N/A` en `importe`), pero la propuesta no ofrece convertir la columna.
**Problema:** la decisión de no convertir automáticamente es correcta, pero ninguna propuesta de un clic ofrece convertir a número o fecha las columnas que lo son casi por completo.
Además, la promesa «Mediana en números» de la propuesta no se cumple nunca en CSV: `CustomerID` y `num_executed_instructions` reciben «valor más frecuente» porque son texto.
**Impacto:** la «copia confiable» llega sin tipos: no se puede indexar ni agregar sin `CAST` en SQL, y es la causa raíz de FUN-04.
**Solución propuesta:** incluir en la propuesta «Convertir `cantidad` a número entero (20.000 de 20.000 valores válidos)» cuando ≥ 99 % de los valores no vacíos lo son, con los inválidos a revisión. Con eso FUN-04 deja de producirse en el flujo por defecto (aunque su corrección sigue siendo necesaria para columnas mixtas).
**Criterio de aceptación:** con `OnlineRetail`, la propuesta ofrece tipar `Quantity`, `UnitPrice` y `CustomerID`, y la entrega a SQL Server crea columnas numéricas.

#### [FUN-05] La exportación a Excel produce archivos que Excel no abre, sin avisar
**Severidad:** Alto · **Confianza:** reproducido · **Esfuerzo:** medio · **Áreas:** Corrección funcional, Datos
**Ubicación:** `src-tauri/src/dataset/export_io.rs:294-301` (`xml_escape` no filtra caracteres de control prohibidos en XML 1.0) y `:315-358` (`xlsx_cell` sin límite de 32.767 caracteres); el escritor de filas no comprueba el máximo de 1.048.576 filas por hoja.
**Evidencia:**
- `excel_control.csv` (4 filas: `\x01`, `\x0B` y una celda de 40.000 caracteres) → `control.xlsx`: `xl/worksheets/sheet1.xml` no es XML bien formado («invalid token», línea 1, columna 607); `openpyxl` falla y **Excel (COM) no puede abrirlo**.
- `excel_1m1.csv` (1.100.000 filas) → `filas_1m1.xlsx` (12,4 MB, 81 s) con `<row r="1100001">`: **Excel no puede abrirlo**.
- La app muestra la exportación como correcta en ambos casos; no hay aviso previo.
**Problema:** el exportador asume que cualquier dataset cabe en una hoja de Excel y que cualquier texto es XML válido.
**Impacto:** la entrega falla donde más duele, en el equipo de quien la recibe. Los caracteres de control aparecen en datos reales copiados de otros sistemas (ERP, PDFs, terminales).
**Cómo reproducirlo:** los dos archivos de la evidencia (se generan con los comandos de «Cómo reproducirlo» de cada caso; no se versionan) → Entregar → Excel.
**Solución propuesta:** (1) en `xml_escape` para XLSX, eliminar o sustituir los caracteres no válidos en XML 1.0 (`\x00-\x08`, `\x0B`, `\x0C`, `\x0E-\x1F`) e informar del número de celdas afectadas; (2) en la comprobación previa de Entregar, bloquear o proponer alternativas (CSV/Parquet, o varias hojas) si hay más de 1.048.575 filas de datos o celdas de más de 32.767 caracteres; (3) test que abra el XLSX generado con un parser XML estricto.
**Criterio de aceptación:** los dos archivos de la evidencia producen un aviso previo o un XLSX que un parser XML estricto acepta; nunca un XLSX inválido presentado como correcto.

#### [DAT-01] Tras un cierre inesperado se pierden en silencio los cambios posteriores al último guardado
**Severidad:** Medio · **Confianza:** reproducido · **Esfuerzo:** medio · **Áreas:** Datos y persistencia, UI/UX
**Ubicación:** arranque de la fase Cargar (`src/features/projects/ProjectsPanel.tsx`) y preferencia de autoguardado por proyecto (`src/features/projects/useProjectsController.ts:30-38`, desactivada por defecto).
**Evidencia:** `capturas/16-relanzado.png`, `17-recuperacion.png`. Proyecto guardado con el recorte (cursor 1) → aplicar «Quitar duplicados» (cursor 2) → `Stop-Process -Force` → relanzar: la pantalla de inicio es idéntica a una sesión normal; «Recuperar proyecto» (dentro de un grupo plegado) devuelve el cursor 1, y el cambio del cursor 2 desaparece sin aviso. Un dataset nunca guardado como proyecto se pierde entero.
**Problema:** la recuperación explícita es una decisión documentada (`CONTEXTO.md:289`) y es correcta; lo que falta es **informar**: no se detecta el cierre anómalo ni se dice qué se perdió.
**Impacto:** la persona cree que «Recuperar» devolvió su trabajo y sigue sobre un estado anterior sin saberlo.
**Cómo reproducirlo:** los pasos de la evidencia.
**Solución propuesta:** marcador de sesión limpia (se borra al salir bien); si falta al arrancar, mostrar arriba en Cargar «Columnia se cerró inesperadamente. Último guardado: 11:16 (Auditoría…). Los cambios posteriores no se guardaron.» con la acción «Recuperar» a la vista. Considerar autoguardado activado por defecto tras el primer guardado.
**Criterio de aceptación:** tras un `Stop-Process -Force`, el aviso aparece con la hora del último guardado; tras un cierre normal, no.

#### [CODE-01] Los informes de fallo guardan solo `mod.rs:2136`: no se puede saber qué crate falló
**Severidad:** Medio · **Confianza:** reproducido · **Esfuerzo:** bajo · **Áreas:** Código (observabilidad)
**Ubicación:** `src-tauri/src/crash_report.rs:49-57`
**Evidencia:** los tres informes existentes en `%APPDATA%\app.columnia.desktop\crash-reports\` (versión 1.26.0, 2026-09-24T02:45Z) dicen `"source": "mod.rs:2136"`; hay cientos de `mod.rs` en Polars, Tokio y Tauri.
**Problema:** para no filtrar la carpeta del usuario se descarta toda la ruta, incluida la parte que identifica el crate.
**Impacto:** sin telemetría por decisión del proyecto, este informe es la única pista de un fallo en el equipo de otra persona (beta RV07), y así no sirve para localizarlo.
**Solución propuesta:** conservar la ruta desde el crate: recortar hasta después de `registry/src/<índice>/` (→ `polars-core-0.55.2/src/…/mod.rs:2136`) o, para código propio, desde `src-tauri/`. Ninguna de las dos contiene datos del usuario.
**Criterio de aceptación:** un panic dentro de una dependencia registra `crate-versión/ruta:línea` y ninguna ruta absoluta; test en `crash_report.rs`.

#### [UX-01] Detalles de Preparar y Entregar que contradicen lo que muestran
**Severidad:** Bajo · **Confianza:** reproducido · **Esfuerzo:** bajo · **Áreas:** UI/UX
- **Propuesta que no cambia nada:** tras aplicar el recorte, la propuesta vuelve a listar «Recortar espacios» marcado, aunque su antes/después ya no tiene filas (`capturas/` sesión F4). Cada cambio propuesto debería llevar su contador y desaparecer con 0.
- **Resultado sin lo aplicado:** «Listo: cambios aplicados» solo muestra filas, vacíos, duplicados y columnas; tras aplicar solo el recorte, las cuatro métricas quedan igual y no se ve qué cambió (celdas recortadas).
- **Historial ilegible:** cada aplicación se registra como «Aplicar correcciones recomendadas»; con dos entradas no se sabe cuál quitó duplicados y cuál recortó (`get_history_state`).
- **Tamaño obsoleto:** Entregar muestra «536,641 filas · 8 columnas · 41.9 MiB», el tamaño del archivo original (`capturas/13-entregar.png`).
- **Recomendado no preseleccionado:** en Entregar, «Validar calidad · Recomendado» aparece sin marcar y «Exportar sin validar» marcado (`capturas/13-entregar.png`).
- **Formato de números inconsistente:** «541,909 filas» en Revisar y «541.909 filas · … 10.9 MiB» en Proyectos, en la misma línea con punto de miles y punto decimal; contradice T10-26 del CHANGELOG.
- **Aviso de esquema irrelevante:** al elegir otro archivo, «El esquema no coincide con el perfil guardado… Columnas faltantes: InvoiceNo…» compara un archivo nuevo sin relación con el perfil del anterior (`capturas/18-control-chars.png`).
- **Estado de fase incorrecto:** tras pasar por Revisar, el lateral sigue diciendo «Revisar · Después» mientras Cargar, Preparar y Entregar dicen «Hecho» (`capturas/22-odbc-form.png`).
- **Concordancia:** «1 cambios del historial activo» en el resumen de entrega.
- **Scroll horizontal en ventana estrecha:** a 900×600, con «Ver antes y después» abierto, el documento mide 911 px y la tarjeta de la propuesta se corta (`capturas/24-preparar-900x600.png`). Causa: `.prepare-proposal` (`src/workflow-styles.css:1293`) es un grid sin `grid-template-columns: minmax(0, 1fr)`, y la tabla impone su ancho mínimo; la tabla debería desplazarse dentro de su `.table-region`.
- **Guardar escondido:** guardar el trabajo exige volver a Cargar y abrir dos desplegables («Continuar un proyecto» → «Guardar y administrar proyectos»).

#### [QA-02] 1.041 tests en verde y ninguno detecta los fallos funcionales de esta auditoría
**Severidad:** Alto · **Confianza:** reproducido · **Esfuerzo:** medio · **Áreas:** QA y testing
**Ubicación:** `e2e/*.spec.ts` (shell web con IPC simulado), `src-tauri/src/dataset/tests.rs` (fixtures sintéticas pequeñas), `fixtures/manifest.json` (`synthetic-only-no-pii`).
**Evidencia:** perfil Full aprobado (493 + 521 + 27 tests) sobre el mismo commit en el que se reproducen FUN-01 a FUN-07 con datos reales. Los tests existentes fijan el comportamiento: `imputes_repeated_text_and_lower_median_numeric_nuls_only` (`tests.rs:11071`) exige la moda; `csv_export_neutralizes_spreadsheet_formulas_and_parquet_preserves_values` (`:8856`) no incluye un negativo; `exports_a_real_xlsx_with_safe_inline_strings` (`:8992`) no valida el XML con un parser estricto ni límites de Excel.
**Problema:** los tests verifican que cada pieza hace lo que el código dice, pero no los **invariantes de producto**: «lo anunciado es lo aplicado», «la copia exportada conserva valores y tipos», «el archivo entregado abre en su programa de destino», «no se inventan identificadores».
**Impacto:** la suite da una falsa sensación de seguridad justo en el camino recomendado de un clic; es la misma lección que ya dejó el pánico de Polars (datasets pequeños que no llegan a la ruta real).
**Solución propuesta:** una batería de **invariantes de ida y vuelta** con fixtures sintéticas realistas (versionadas, sin PII) que reproduzcan los rasgos de esta auditoría: CR-only, Windows-1252, identificador con 25 % de vacíos, negativos, caracteres de control, más de 1.048.576 filas (generada al vuelo). Para cada una: cargar → propuesta por defecto → exportar a CSV/XLSX/SQL → releer y comprobar tipos, recuentos anunciados = diff y validez del archivo. Correrla en el perfil Full.
**Criterio de aceptación:** la batería falla en el commit auditado por cada uno de FUN-01 a FUN-07 y pasa tras sus correcciones.

#### [QA-01] Los comandos documentados para correr tests Rust a mano fallan sin una variable no documentada
**Severidad:** Bajo · **Confianza:** reproducido · **Esfuerzo:** bajo · **Áreas:** QA y testing, Documentación
**Ubicación:** `src-tauri/src/remote_databases.rs:1923-1924`, `docs/reference/feature-parity.md:296`, `src-tauri/build.rs:10-18`
**Evidencia:** `cargo test --lib -- --ignored unreachable_server` termina con `exit code: 0xc0000139, STATUS_ENTRYPOINT_NOT_FOUND`. Con `COLUMNIA_TEST_HARNESS_MANIFEST=1` los mismos tests pasan. La variable solo aparece en `docs/archive/2026-09/historial-verificacion.md`.
**Problema:** el binario de tests de Tauri necesita el manifiesto de Common Controls v6, que `build.rs` solo incrusta si existe esa variable; `tools/check.ps1:248-256` la pone, pero los comandos manuales documentados no.
**Impacto:** quien intente correr a mano los tests ignorados (precisamente los ODBC reales) choca con un error críptico de Windows y lo deja; es una de las razones por las que la entrega ODBC no se ejerce.
**Cómo reproducirlo:** en `src-tauri`, `cargo test --lib -- --ignored unreachable_server`.
**Solución propuesta:** documentar la variable junto a cada comando manual (o un script `npm run test:rust:ignored` que la ponga) y retirar el comando suelto de `feature-parity.md`.
**Criterio de aceptación:** el comando documentado funciona copiado tal cual en una terminal nueva.

## Descartados en la refutación

- **quick-xml vulnerable en el lector de Excel** (RUSTSEC-2026-0194/0195): descartado. `calamine` usa `quick-xml` 0.41.0 (parcheado); solo `object_store` arrastra la 0.39.4, sin ruta alcanzable. La excepción de `src-tauri/deny.toml:12-13` es correcta.
