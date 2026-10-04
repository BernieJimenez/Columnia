# Roadmap de Columnia

**Actualizado:** 2026-10-03 · **Versión:** 1.26.0

**Estado:** aplicación local funcional para Windows x64, de **uso personal**. No
se comparte ni se distribuye; las mejoras se eligen por lo que su dueño nota al
usarla con sus propios archivos.

Este documento solo contiene lo pendiente. Lo entregado está en
[`CHANGELOG.md`](CHANGELOG.md). El roadmap anterior, con los Tiers 5 a 10 y los
objetivos RV01–RV16, está archivado sin cambios en
[`docs/archive/2026-09/`](docs/archive/2026-09/ROADMAP.md).

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

No hay pendientes comprometidos. Cada mejora se decide al usar la aplicación y
se describe en `CHANGELOG.md` al cerrarla.

## Pendientes de la revisión del 2026-10-01

Salen de la revisión completa del 2026-10-01. El informe y las fichas de cada hallazgo se guardan
solo en local (`docs/auditorias/`, fuera de git porque el repositorio es público). **Veredicto: no lista según la
rúbrica de la revisión**, porque quedan seis defectos Altos en flujos que no pueden fallar (Cargar,
Preparar y Entregar); utilizable para el uso personal actual con las precauciones del informe.

No son compromisos: su dueño decide cuáles pasan a la cola vigente. Cada pendiente tiene una casilla
y una por cada problema que resuelve; se marcan con `[x]` al quedar resueltos y verificados, y el
pendiente se marca cuando lo están todos sus problemas. El ID (`FUN-01`, `DAT-02`…) remite a la ficha
local del hallazgo, con pruebas y criterio de aceptación. Esfuerzo: bajo = 2 h,
medio = 1 día; agregado por suma.

| Tier | Pendientes | Hallazgos | Esfuerzo agregado |
| --- | ---: | ---: | ---: |
| Tier 1 — Alta prioridad | 6 | 6 | 5,2 días |
| Tier 2 — Mejoras sustanciales | 36 | 136 | 80 días |
| Tier 3 — Pulido y mantenimiento | 41 | 183 | 61,5 días |
| Tier 4 — Futuro u opcional | 14 | 37 | 12,2 días |
| **Total** | **97** | **362** | **159 días** |

### Tier 1 — Alta prioridad

- [x] **RV23** — Impedir que una exportación (ventana, CLI y `project-export`) escriba sobre el archivo de origen o, en la CLI, sobre un archivo existente sin `--force` · *Esfuerzo: 1 día* — hecho el 2026-10-03
  - Criterio de cierre: Exportar eligiendo el propio origen falla con un mensaje claro en la ventana y en la CLI, también con rutas alias (`..`, mayúsculas, enlaces); `transform` sobre un archivo existente exige `--force`; prueba Rust y el original queda sin cambios.
  - [x] `DAT-01` (Alto) Exportar puede sobrescribir el archivo original: la CLI sin ningún aviso y la ventana con solo el aviso de Windows — hecho el 2026-10-03
- [x] **RV24** — Que «Convertir números detectados» e «Interpretar fechas» cuenten, anuncien y pidan confirmar las celdas que pasarían a nulo · *Esfuerzo: 1 día* — hecho el 2026-10-03
  - Criterio de cierre: La propuesta y el mensaje final dicen cuántas celdas quedan nulas por columna; con más de 0 se pide confirmación como en «Apartar valores incompatibles»; prueba con una columna al 95 % numérica.
  - [x] `FUN-01` (Alto) «Convertir números detectados» e «Interpretar fechas» anulan celdas sin pedir confirmación ni decirlo — hecho el 2026-10-03
- [x] **RV25** — Rechazar o resolver los años de 1-3 dígitos en la convención de fechas de la importación y en las recetas (un solo analizador de fechas) · *Esfuerzo: 1 día* — hecho el 2026-10-03
  - Criterio de cierre: `01/02/25` con «día-mes-año» se importa como 2025 o se rechaza con aviso, igual que en Preparar; prueba con años de 2 dígitos y fechas imposibles.
  - [x] `FUN-02` (Alto) Con la convención «día-mes-año», las fechas con año de 2 dígitos se importan como años 0025 o 0099 — hecho el 2026-10-03
- [x] **RV26** — Exportar los datasets grandes (≥ 512 MiB) con los mismos valores y tipos que los pequeños · *Esfuerzo: 1 día* — hecho el 2026-10-03
  - Criterio de cierre: La misma receta exportada a CSV y SQL desde un CSV de 600 MiB y desde su versión de 1 MiB da salidas idénticas valor a valor (incluidos `1.50`, `1e5`, fechas y el sufijo ` UTC`); `changed` refleja lo que cambia.
  - [x] `DAT-02` (Alto) Con archivos de 512 MiB o más, la exportación a CSV y SQL reescribe números, fechas y zonas horarias y lo da por «sin cambios» — hecho el 2026-10-03
- [x] **RV27** — Reconocer los nombres de tipo de Polars 0.55 (`str`, `f64`, …) en la compatibilidad ODBC y probarlo con los nombres reales · *Esfuerzo: 2 h* — hecho el 2026-10-03
  - Criterio de cierre: «Añadir a tabla existente» funciona sobre una tabla creada por la propia Columnia con columnas de texto y decimales; las pruebas usan los nombres que produce Polars y cubren `interval`/`image`.
  - [x] `FUN-03` (Alto) «Añadir a tabla existente» está bloqueado para toda columna de texto o decimal, incluso al reañadir a una tabla que creó Columnia (confirma FUN-R4-13) — hecho el 2026-10-03
- [x] **RV28** — Acotar la memoria al abrir libros con un rango declarado enorme (no reservar filas × ancho declarados) · *Esfuerzo: 1 día* — hecho el 2026-10-03
  - Criterio de cierre: un libro de 5 KB con `A1:XFD1048576` se inspecciona con menos de 500 MB o se rechaza con un mensaje; prueba Rust con una dimensión inflada.
  - [x] `ARQ-01` (Alto) Un libro XLSX de 5 KB con `<dimension ref="A1:XFD1048576">` agota más de 8 GB de memoria en `inspect` — hecho el 2026-10-03

### Tier 2 — Mejoras sustanciales

- [x] **RV29** — **Entregar** — Los textareas de listas del editor de reglas pierden el Intro y los espacios al teclear (y 1 problemas más) · *Esfuerzo: 1,2 días* — hecho el 2026-10-03
  - Criterio de cierre: Se cumplen los criterios de aceptación de FUN-04, QA-11.
  - [x] `FUN-04` (Medio) Los textareas de listas del editor de reglas pierden el Intro y los espacios al teclear — hecho el 2026-10-03
  - [x] `QA-11` (Medio) Pruebas de Entregar que no pueden fallar por lo importante y huecos de la compuerta — hecho el 2026-10-03
- [x] **RV30** — **Consulta SQL y DuckDB** — Los dos motores SQL dan resultados distintos para la misma consulta y la interfaz no dice… (y 4 problemas más) · *Esfuerzo: 3,5 días* — hecho el 2026-10-03
  - Criterio de cierre: Se cumplen los criterios de aceptación de FUN-05, FUN-13, DAT-08, DAT-11, REN-05.
  - [x] `FUN-05` (Medio) Los dos motores SQL dan resultados distintos para la misma consulta y la interfaz no dice cuál respondió — hecho el 2026-10-03
  - [x] `FUN-13` (Medio) Consola SQL: `join_date` y columnas con coma hacen fallar consultas válidas — hecho el 2026-10-03
  - [x] `DAT-08` (Medio) Las celdas vacías salen como cadena vacía en Parquet, SQL, SQLite, JSON y XLSX en archivos pequeños y como NULL en los grandes — hecho el 2026-10-03
  - [x] `DAT-11` (Medio) El script SQL exportado borra la tabla `dataset` del destino y puede declarar tipos que no caben — hecho el 2026-10-03
  - [x] `REN-05` (Medio) Cada consulta DuckDB sobre un DataFrame escribe un Parquet completo del dataset (candidato a medir) — hecho el 2026-10-03
- [x] **RV31** — **Explorar** — Las barras del histograma de Explorar cuentan `[a,b)` y el filtro filtra `[a,b]`: la cifr… (y 3 problemas más) · *Esfuerzo: 2,5 días* — hecho el 2026-10-03
  - Criterio de cierre: Se cumplen los criterios de aceptación de FUN-06, ARQ-04, UX-01, ACC-01.
  - [x] `FUN-06` (Medio) Las barras del histograma de Explorar cuentan `[a,b)` y el filtro filtra `[a,b]`: la cifra de la barra no coincide con «Filas» — hecho el 2026-10-03
  - [x] `ARQ-04` (Medio) Varias operaciones largas retienen el mutex del dataset durante todo el cálculo (Explorar, consultas, agregación temporal, deshacer, comparación) — hecho el 2026-10-03
  - [x] `UX-01` (Medio) Si el panel de Explorar falla tras un filtro, desaparece todo (chips y botón «Personalizar» incluidos) y no hay forma de quitar el filtro — hecho el 2026-10-03
  - [x] `ACC-01` (Medio) Barras de la tendencia de Explorar con 60 periodos miden 10,9 × 160 px (9,7 a 900 px de ancho) — hecho el 2026-10-03
- [x] **RV32** — **Exportación y libros** — La protección contra fórmulas altera datos legítimos (`-5`, `+34…`, `@ana`) y la interfaz… (y 4 problemas más) · *Esfuerzo: 2 días* — hecho el 2026-10-03
  - Criterio de cierre: Se cumplen los criterios de aceptación de FUN-07, FUN-15, FUN-29, FUN-31, SEG-01.
  - [x] `FUN-07` (Medio) La protección contra fórmulas altera datos legítimos (`-5`, `+34…`, `@ana`) y la interfaz no la anuncia ni deja elegirla — hecho el 2026-10-03
  - [x] `FUN-15` (Medio) Encabezados de hoja `a, a, a_2` repiten nombres y el libro no se puede abrir — hecho el 2026-10-03
  - [x] `FUN-29` (Medio) XLSX exportado pierde precisión silenciosamente en enteros de más de 15 dígitos — hecho el 2026-10-03
  - [x] `FUN-31` (Medio) Columnas de libro con fechas mezcladas con texto o errores se convierten en texto/serial sin aviso — hecho el 2026-10-03
  - [x] `SEG-01` (Medio) Las cabeceras que empiezan por `=` (y `+`, `-`, `@`) no se neutralizan al exportar a CSV — hecho el 2026-10-03
- [x] **RV33** — **Exportación y libros** — El modo «hash» de privacidad es SHA-256 sin sal: reversible por diccionario en datos de b… (y 4 problemas más) · *Esfuerzo: 2 días* — hecho el 2026-10-03
  - Criterio de cierre: Se cumplen los criterios de aceptación de SEG-02, ARQ-05, LIM-02, QA-14, DOC-02.
  - [x] `SEG-02` (Medio) El modo «hash» de privacidad es SHA-256 sin sal: reversible por diccionario en datos de baja entropía — hecho el 2026-10-03
  - [x] `ARQ-05` (Medio) Los ZIP (bundle, xlsx) no activan `large_file`: entradas de más de 4 GiB fallarán — hecho el 2026-10-03
  - [x] `LIM-02` (Medio) Esqueleto de exportación duplicado en siete variantes y bloques idénticos — hecho el 2026-10-03
  - [x] `QA-14` (Medio) Las pruebas de exportación source-backed no cubren ceros a la izquierda, destino igual a fuente ni fórmulas — hecho el 2026-10-03
  - [x] `DOC-02` (Medio) THREAT_MODEL.md: fecha de verificación vencida, omisiones y contradicciones con el código — hecho el 2026-10-03
- [x] **RV34** — **Motor (dataset.rs)** — «Eliminar duplicados parecidos» necesita dos pasadas y la primera deja una pareja parecida (y 4 problemas más) · *Esfuerzo: 3,8 días* — hecho el 2026-10-03
  - Criterio de cierre: Se cumplen los criterios de aceptación de FUN-08, FUN-16, REN-03, REN-04, LIM-01.
  - [x] `FUN-08` (Medio) «Eliminar duplicados parecidos» necesita dos pasadas y la primera deja una pareja parecida — hecho el 2026-10-03
  - [x] `FUN-16` (Medio) `apply_outlier_mode` (Cap) cambia una columna Int64 a Float64 y `impute_missing_values_in_columns` pasa enteros grandes por f64 — hecho el 2026-10-03
  - [x] `REN-03` (Medio) `validate_conflict_decisions_with_cancel` es O(conflictos × decisiones) — hecho el 2026-10-03
  - [x] `REN-04` (Medio) `open_last_export_in_power_bi` lee el CSV completo en un comando síncrono — hecho el 2026-10-03
  - [x] `LIM-01` (Medio) `src-tauri/src/dataset.rs` mezcla modelo de datos, historial de mutaciones, rutas DuckDB y comandos; deuda estructural y código duplicado entre rutas — hecho el 2026-10-03
- [x] **RV35** — **Cargar** — No existe la noción de «cambios sin guardar»: cargar otro archivo, abrir otro proyecto o… (y 3 problemas más) · *Esfuerzo: 2,5 días* — hecho el 2026-10-03
  - Criterio de cierre: Se cumplen los criterios de aceptación de FUN-09, ACC-10, PROD-02, QA-12.
  - [x] `FUN-09` (Medio) No existe la noción de «cambios sin guardar»: cargar otro archivo, abrir otro proyecto o cerrar la ventana descartan el trabajo sin avisar — hecho el 2026-10-03
  - [x] `ACC-10` (Medio) El diálogo de importación no recibe el foco al abrirse ni lo devuelve al cerrarse con Esc — hecho el 2026-10-03
  - [x] `PROD-02` (Medio) Una detección equivocada del separador o la codificación no se puede corregir desde el diálogo de importación — hecho el 2026-10-03
  - [x] `QA-12` (Medio) LoadPhase al 72,2 %: los caminos sin prueba son los de error, cancelación, codificación y perfil — hecho el 2026-10-03
- [x] **RV36** — **App y puente** — Paginar la vista previa de Revisar invalida la validación de Entregar sin cambio en el da… (y 2 problemas más) · *Esfuerzo: 1,5 días* — hecho el 2026-10-03
  - Criterio de cierre: Se cumplen los criterios de aceptación de FUN-10, ARQ-03, QA-05.
  - [x] `FUN-10` (Medio) Paginar la vista previa de Revisar invalida la validación de Entregar sin cambio en el dataset — hecho el 2026-10-03
  - [x] `ARQ-03` (Medio) No hay error boundary ni manejador global: una excepción de render deja la ventana en blanco — hecho el 2026-10-03
  - [x] `QA-05` (Medio) `src/App.test.tsx`: archivo único de 3.495 líneas, dependiente de temporizadores, de la configuración regional y con flujos sin test — hecho el 2026-10-03
- [x] **RV37** — **Preparar** — La pantalla «Listo: cambios aplicados» queda obsoleta tras Deshacer desde el Historial (y 4 problemas más) · *Esfuerzo: 1,2 días* — hecho el 2026-10-03
  - Criterio de cierre: Se cumplen los criterios de aceptación de FUN-11, FUN-12, FUN-22, FUN-23, FUN-24.
  - [x] `FUN-11` (Medio) La pantalla «Listo: cambios aplicados» queda obsoleta tras Deshacer desde el Historial — hecho el 2026-10-03
  - [x] `FUN-12` (Medio) La propuesta de Preparar anuncia menos duplicados de los que aplica — hecho el 2026-10-03
  - [x] `FUN-22` (Medio) `looksLikeIdentifier` solo mira la última palabra: `id_cliente`, `codigo_postal`, `num_factura` no se detectan como identificadores — hecho el 2026-10-03
  - [x] `FUN-23` (Medio) La estimación de filas del filtro cuenta 0 coincidencias para filtros por fecha o con coma decimal — hecho el 2026-10-03
  - [x] `FUN-24` (Medio) «Convertir N columnas a fecha» cuenta las columnas ambiguas, pero se aplican solo las resueltas; el modo «paso a paso» nunca pregunta el orden — hecho el 2026-10-03
- [x] **RV38** — **Preparar** — Conversiones de tipo, interpretación de fechas y «buscar y reemplazar» no piden confirmac… (y 4 problemas más) · *Esfuerzo: 2 días* — hecho el 2026-10-03
  - Criterio de cierre: Se cumplen los criterios de aceptación de FUN-25, FUN-26, TXT-01, COD-01, QA-08.
  - [x] `FUN-25` (Medio) Conversiones de tipo, interpretación de fechas y «buscar y reemplazar» no piden confirmación y el asesor las califica de «bajo riesgo» aunque pueden anular o reescribir datos — hecho el 2026-10-03
  - [x] `FUN-26` (Medio) El editor guarda estado inicial de las columnas (`keptColumns`, borradores) y solo se reinicia con `recipeSession`: si el dataset cambia sin remontar, una columna nueva se trata como «descartada» — hecho el 2026-10-03
  - [x] `TXT-01` (Medio) Afirmaciones absolutas que el producto no cumple: «nunca inventa valores», «todas las acciones son reversibles», «reversible desde el historial» — hecho el 2026-10-03
  - [x] `COD-01` (Medio) La detección de cancelación compara texto en español, copiado en 6 sitios — hecho el 2026-10-03
  - [x] `QA-08` (Medio) `src/features/prepare/usePrepareController.test.tsx`: fija los plurales defectuosos y no cubre los caminos que importan (historial desactivado, anulados, cancelación tardía) — hecho el 2026-10-03
- [x] **RV39** — **Preparar** — `PreparePhase.test.tsx`: un test sin aserciones, plurales fijados y huecos en los caminos de estado de la propuesta y e… · *Esfuerzo: 1 día* — hecho el 2026-10-03
  - Criterio de cierre: Se cumplen los criterios de aceptación de QA-09.
  - [x] `QA-09` (Medio) `src/features/prepare/PreparePhase.test.tsx`: un test sin aserciones, plurales fijados y huecos en los caminos de estado de la propuesta y el editor — hecho el 2026-10-03
- [x] **RV40** — **Motor de recetas** — Renombrar `a→total` y añadir una columna calculada `total`: en pequeños sobrescribe en si… (y 4 problemas más) · *Esfuerzo: 3,5 días* — hecho el 2026-10-03
  - Criterio de cierre: Se cumplen los criterios de aceptación de FUN-14, FUN-17, FUN-32, FUN-33, FUN-34.
  - [x] `FUN-14` (Medio) Renombrar `a→total` y añadir una columna calculada `total`: en pequeños sobrescribe en silencio, en grandes falla sin explicar — hecho el 2026-10-03
  - [x] `FUN-17` (Medio) Camino lazy: «filas filtradas» incluye las filas atípicas eliminadas, que además se informan aparte — hecho el 2026-10-03
  - [x] `FUN-32` (Medio) Los dos caminos de receta no son equivalentes: la validación usa el parser eager y la ejecución lazy usa Polars (ISO con `strict:false`, números con espacios) — hecho el 2026-10-03
  - [x] `FUN-33` (Medio) Camino «source-backed» (DuckDB/RE2): las expresiones regulares y los tokens no significan lo mismo que en Rust, con datos en español — hecho el 2026-10-03
  - [x] `FUN-34` (Medio) Camino «source-backed»: los casts SQL aceptan o redondean valores que eager rechaza — hecho el 2026-10-03
- [x] **RV41** — **Motor de recetas** — Materializaciones completas de columnas como `Vec<Option<String>>` y accesos celda a celd… (y 2 problemas más) · *Esfuerzo: 4 días* — hecho el 2026-10-03
  - Criterio de cierre: Se cumplen los criterios de aceptación de REN-06, COD-03, QA-13.
  - [x] `REN-06` (Medio) Materializaciones completas de columnas como `Vec<Option<String>>` y accesos celda a celda en validación y caminos eager (candidato a medir) — hecho el 2026-10-03
  - [x] `COD-03` (Medio) Tres implementaciones paralelas de cada paso de receta con validaciones copiadas (origen de las divergencias R6-06/07/12/13) — hecho el 2026-10-03
  - [x] `QA-13` (Medio) Los tests «source-backed igual que eager» comparan en realidad con el motor lazy, no con el eager — hecho el 2026-10-03
- [x] **RV42** — **Reglas de calidad** — Mínimo/máximo temporales de columnas de texto ignoran el orden de fecha inferido (siempre… (y 4 problemas más) · *Esfuerzo: 2,8 días* — hecho el 2026-10-03
  - Criterio de cierre: Se cumplen los criterios de aceptación de FUN-18, FUN-35, FUN-36, REN-07, PROD-03.
  - [x] `FUN-18` (Medio) Mínimo/máximo temporales de columnas de texto ignoran el orden de fecha inferido (siempre día/mes primero) — hecho el 2026-10-03
  - [x] `FUN-35` (Medio) `conditional`: si el valor de `when` no se puede interpretar para el tipo de la columna, la regla pasa en vacío — hecho el 2026-10-03
  - [x] `FUN-36` (Medio) Reglas agregadas: textos no numéricos se omiten sin contarlos, y la tolerancia por defecto es 0 sobre sumas f64 — hecho el 2026-10-03
  - [x] `REN-07` (Medio) `conditional` con `then: regex` compila la expresión regular en cada fila — hecho el 2026-10-03
  - [x] `PROD-03` (Medio) `distribution_drift` solo compara la media y trata la columna vacía como media 0 — hecho el 2026-10-03
- [x] **RV43** — **Varios** — «Marcadores sin dato» convierte en nulo un título legítimo («Unknown») y rompe el contrat… (y 1 problemas más) · *Esfuerzo: 1,2 días* — hecho el 2026-10-03
  - Criterio de cierre: Se cumplen los criterios de aceptación de FUN-19, UX-03.
  - [x] `FUN-19` (Medio) «Marcadores sin dato» convierte en nulo un título legítimo («Unknown») y rompe el contrato propuesto — hecho el 2026-10-03
  - [x] `UX-03` (Medio) El CSV exportado es UTF-8 sin BOM con coma: Excel (separador de listas `;`) lo abre en una columna y con mojibake — hecho el 2026-10-03
- [ ] **RV44** — **Entrega ODBC** — El análisis de compatibilidad declara «Preflight completo» con una columna de más de 128… (y 4 problemas más) · *Esfuerzo: 3,5 días*
  - Criterio de cierre: Se cumplen los criterios de aceptación de FUN-20, FUN-21, DAT-10, DAT-12, UX-04.
  - [x] `FUN-20` (Medio) El análisis de compatibilidad declara «Preflight completo» con una columna de más de 128 caracteres y el CREATE TABLE falla después — hecho el 2026-10-03
  - [x] `FUN-21` (Medio) Dos columnas que solo difieren en mayúsculas («Dup»/«dup») hacen que el análisis ODBC de un dataset respaldado en archivo falle con «El esquema source-backed cambió durante el preflight remoto» — hecho el 2026-10-03
  - [x] `DAT-10` (Medio) Si la conexión se pierde justo al confirmar, el servidor guarda las filas pero la ventana dice «No se pudo crear la copia» — hecho el 2026-10-03
  - [ ] `DAT-12` (Medio) En MySQL la política «Crear» no es atómica: el DDL confirma implícitamente y un fallo posterior deja una tabla vacía que bloquea el reintento — implementado el 2026-10-03 (se borra la tabla creada si la entrega falla); falta la prueba externa con MySQL: no hay servidor ni controlador MySQL en este equipo
  - [x] `UX-04` (Medio) Los errores de ODBC llegan al usuario con el volcado `Debug` de Rust («Diagnostics { record: State: 08S01, Native error: 10054, … function: "SQLExecute" }») — hecho el 2026-10-03
- [x] **RV45** — **Entrega ODBC** — Los tests de la comprobación previa usan nombres de tipo que el código real no produce, y las pruebas con servidor solo… · *Esfuerzo: 2 h* — hecho el 2026-10-03
  - Criterio de cierre: Se cumplen los criterios de aceptación de QA-20.
  - [x] `QA-20` (Medio) Los tests de la comprobación previa usan nombres de tipo que el código real no produce, y las pruebas con servidor solo cubren «Crear» — hecho el 2026-10-03
- [x] **RV46** — **Revisar** — El SQL escrito y su resultado se pierden al cambiar de pestaña o al re-perfilar (y 3 problemas más) · *Esfuerzo: 1 día* — hecho el 2026-10-03
  - Criterio de cierre: Se cumplen los criterios de aceptación de FUN-27, FUN-28, ACC-06, ACC-09.
  - [x] `FUN-27` (Medio) El SQL escrito y su resultado se pierden al cambiar de pestaña o al re-perfilar — hecho el 2026-10-03
  - [x] `FUN-28` (Medio) `formatStatistic` redondea a 3 decimales y muestra 0 para magnitudes pequeñas — hecho el 2026-10-03
  - [x] `ACC-06` (Medio) La celda más intensa del calendario de Revisar tiene texto ilegible en todos los temas (1,0:1 claro, 1,08:1 oscuro) — hecho el 2026-10-03
  - [x] `ACC-09` (Medio) Tablas «Ver datos» de Revisar con scroll pero sin foco por teclado (axe `scrollable-region-focusable`, serious) — hecho el 2026-10-03
- [x] **RV47** — **Proyectos e historial (motor)** — `project-save --id` sobre un proyecto existente construye un espacio de trabajo vacío (po… (y 4 problemas más) · *Esfuerzo: 4,2 días* — hecho el 2026-10-04
  - Criterio de cierre: Se cumplen los criterios de aceptación de FUN-30, DAT-03, DAT-04, DAT-05, DAT-07.
  - [x] `FUN-30` (Medio) `project-save --id` sobre un proyecto existente construye un espacio de trabajo vacío (posible pérdida de reglas, receta e historial SQL) — hecho el 2026-10-03
  - [x] `DAT-03` (Medio) Un cierre abrupto durante guardar proyecto o exportar deja temporales de cientos de MB que nada limpia — hecho el 2026-10-04
  - [x] `DAT-04` (Medio) Los snapshots del historial de Preparar viven en `%TEMP%` y nadie los purga (ni al cerrar con normalidad) — hecho el 2026-10-04
  - [x] `DAT-05` (Medio) Un proyecto cuya carpeta de generación falta no se puede abrir ni eliminar desde la app — hecho el 2026-10-04
  - [x] `DAT-07` (Medio) Un solo snapshot por encima de 1 GiB destruye todo el historial reversible ya acumulado y no se recupera — hecho el 2026-10-04
- [ ] **RV48** — **Perfil** — Perfil source-backed: mínimo/máximo de enteros se pasan por f64 y no coinciden con la rut… (y 4 problemas más) · *Esfuerzo: 3,8 días*
  - Criterio de cierre: Se cumplen los criterios de aceptación de FUN-37, FUN-38, FUN-39, REN-01, PROD-04.
  - [x] `FUN-37` (Medio) Perfil source-backed: mínimo/máximo de enteros se pasan por f64 y no coinciden con la ruta en memoria — hecho el 2026-10-04
  - [x] `FUN-38` (Medio) Perfil en memoria: `expect` de Q1/Q3 entra en pánico si algún cuartil es infinito o NaN — hecho el 2026-10-04
  - [x] `FUN-39` (Medio) Texto numérico con espacios: el tipo sugerido lo cuenta, pero las estadísticas lo descartan (hipótesis) — hecho el 2026-10-04
  - [ ] `REN-01` (Medio) El perfil de calidad tarda 4,5 min en un CSV de 5,3 GiB y 12,8 s en uno de 338 MB (30× más que duckdb), y se repite tras cada cambio — avance el 2026-10-04: vmCloud 11,9 s → 4,8 s (categorías en una pasada, fechas que se dejan de intentar, tendencia en paralelo, duplicados a la vez que las columnas) y etapas visibles tras el recorrido de archivos grandes; sigue abierto: no llega a ≤ 3 s, falta medir 2019-Oct y reutilizar el perfil tras cada cambio
  - [x] `PROD-04` (Medio) Los números con coma decimal o separador de miles no se reconocen como numéricos — hecho el 2026-10-04
- [ ] **RV49** — **Importación** — Convención decimal convierte códigos con ceros a la izquierda y enteros largos en float s… (y 4 problemas más) · *Esfuerzo: 4,2 días*
  - Criterio de cierre: Se cumplen los criterios de aceptación de FUN-40, DAT-06, DAT-09, REN-02, PROD-01.
  - [x] `FUN-40` (Medio) Convención decimal convierte códigos con ceros a la izquierda y enteros largos en float sin aviso — hecho el 2026-10-04
  - [x] `DAT-06` (Medio) «El archivo de origen cambió» se detecta solo por tamaño en seis caminos (exportar, validar, recetas, paginar) — hecho el 2026-10-04
  - [x] `DAT-09` (Medio) `transform` con receta identidad sobre un CSV de 5,4 GB falla con un mensaje genérico, mientras una receta equivalente sí funciona — hecho el 2026-10-04
  - [ ] `REN-02` (Medio) El proceso Rust retiene cada vez más memoria al recargar el mismo dataset (+450 MB en 10 recargas de un CSV de 144 MB)
  - [x] `PROD-01` (Medio) La CLI rechaza Windows-1252, MacRoman, UTF-16, JSON con BOM y finales de línea mezclados con un mensaje que no dice por qué — hecho el 2026-10-04
- [ ] **RV50** — **Comparación** — La comparación lee siempre el archivo comparado con reglas fijas (primera hoja, primera f… (y 4 problemas más) · *Esfuerzo: 3,5 días*
  - Criterio de cierre: Se cumplen los criterios de aceptación de FUN-41, FUN-42, REN-08, UX-05, UX-06.
  - [x] `FUN-41` (Medio) La comparación lee siempre el archivo comparado con reglas fijas (primera hoja, primera fila como encabezado, sin convenciones) aunque el activo se importó con otras — hecho el 2026-10-04
  - [x] `FUN-42` (Medio) La clave de comparación exige dtype idéntico en ambos lados y los lados se tipan con motores distintos — hecho el 2026-10-04
  - [ ] `REN-08` (Medio) Candidatos de coste en la comparación por clave (hipótesis, medir) — sigue abierto (2026-10-04): no medido; necesita comparar archivos de millones de filas en la app y guardar el índice de conflictos para paginar sin recalcular
  - [x] `UX-05` (Medio) Cambiar la clave con una comparación visible deja resultados y decisiones obsoletos — hecho el 2026-10-04
  - [x] `UX-06` (Medio) Error de comparación con tipos de clave distintos se muestra crudo y sin guía — hecho el 2026-10-04
- [x] **RV51** — **Comparación** — Los tests de comparación usan copias `#[cfg(test)]` de las funciones de persistencia, no… (y 1 problemas más) · *Esfuerzo: 1,2 días* — hecho el 2026-10-04
  - Criterio de cierre: Se cumplen los criterios de aceptación de QA-01, QA-10.
  - [x] `QA-01` (Medio) Los tests de comparación usan copias `#[cfg(test)]` de las funciones de persistencia, no las `_with_cancel` de producción — hecho el 2026-10-04
  - [x] `QA-10` (Medio) Caminos de `src/features/review/DatasetComparisonSection.tsx` sin test (cobertura 78,8 % de líneas) — hecho el 2026-10-04
- [x] **RV52** — **Gates y scripts** — check-secrets: patrones con falsos negativos relevantes para este proyecto (y 4 problemas más) · *Esfuerzo: 2 días* — hecho el 2026-10-04
  - Criterio de cierre: Se cumplen los criterios de aceptación de SEG-03, SEG-04, QA-21, QA-22, QA-23.
  - [x] `SEG-03` (Medio) check-secrets: patrones con falsos negativos relevantes para este proyecto — hecho el 2026-10-04
  - [x] `SEG-04` (Medio) `verify-published-assets` y `check-updater-manifest` no protegen contra downgrade ni acotan el host — hecho el 2026-10-04
  - [x] `QA-21` (Medio) check-performance-baseline acepta evidencia de `.local/` de cualquier commit y antigüedad (salvo dos comprobaciones y solo con `-RequireEvidenceAfter`) — hecho el 2026-10-04
  - [x] `QA-22` (Medio) smoke-tauri: «aprobado» sin ventana visible, «contrato» por subcadenas y sin guardia del almacén real — hecho el 2026-10-04
  - [x] `QA-23` (Medio) automate-native-file-dialog: el modo `open` da «passed» al cerrarse el diálogo (también si se canceló) y hay pulsaciones globales de respaldo — hecho el 2026-10-04
- [ ] **RV53** — **Gates y scripts** — `check-network-policy` solo detecta cuatro APIs de red y no mira dependencias ni `index.h… (y 4 problemas más) · *Esfuerzo: 2 días*
  - Criterio de cierre: Se cumplen los criterios de aceptación de QA-24, QA-25, QA-26, QA-27, QA-28.
  - [ ] `QA-24` (Medio) `check-network-policy` solo detecta cuatro APIs de red y no mira dependencias ni `index.html`
  - [ ] `QA-25` (Medio) `check-ipc-inventory` es circular y no cruza con TypeScript
  - [ ] `QA-26` (Medio) `check-coverage` puede aprobar con un resumen viejo y cubre pocos archivos
  - [ ] `QA-27` (Medio) El «baseline» de accesibilidad acepta evidencia de cualquier commit y cubre solo el shell inicial
  - [ ] `QA-28` (Medio) Dos suites de `node:test` en `tools/` no las ejecuta nadie
- [ ] **RV54** — **Gates y scripts** — check-supply-chain: las vulnerabilidades de `npm audit` nunca hacen fallar el gate (y 4 problemas más) · *Esfuerzo: 1,2 días*
  - Criterio de cierre: Se cumplen los criterios de aceptación de OPS-01, OPS-02, OPS-03, OPS-04, OPS-05.
  - [ ] `OPS-01` (Medio) check-supply-chain: las vulnerabilidades de `npm audit` nunca hacen fallar el gate
  - [ ] `OPS-02` (Medio) El desinstalador silencioso deja `HKCU\Software\columnia\Columnia` apuntando a la carpeta ya borrada, y una instalación de prueba comparte claves y accesos con la real
  - [ ] `OPS-03` (Medio) `notices:check` y `supply-chain:check` fallan en un clon con fin de línea CRLF
  - [ ] `OPS-04` (Medio) El baseline de evidencia release fija la versión del proyecto: cada subida de versión rompe el gate y `--update-baseline` no puede repararlo
  - [ ] `OPS-05` (Medio) check-secrets da «aprobado» si `git` falla o devuelve rutas entrecomilladas
- [ ] **RV55** — **Gates y scripts** — app-data-guard: restauración destructiva sin salvaguarda ante fallo parcial (y 4 problemas más) · *Esfuerzo: 2 días*
  - Criterio de cierre: Se cumplen los criterios de aceptación de OPS-06, OPS-07, OPS-08, OPS-09, OPS-10.
  - [ ] `OPS-06` (Medio) app-data-guard: restauración destructiva sin salvaguarda ante fallo parcial
  - [ ] `OPS-07` (Medio) release.ps1: no comprueba que las variables de tests destructivos estén desactivadas ni que las versiones coincidan
  - [ ] `OPS-08` (Medio) `check-toolchains` convierte un rango `engines` en versión exacta
  - [ ] `OPS-09` (Medio) `docs:check` valida enlaces con regex sobre texto crudo y recorre `docs/auditorias/**`
  - [ ] `OPS-10` (Medio) `check-release-evidence --update-baseline` re-aprueba cualquier captura; hashes de PNG a nivel de byte
- [ ] **RV56** — **Arranque y servicios** — Un catálogo auxiliar corrupto o de versión futura (tareas reutilizables / presets de entr… (y 1 problemas más) · *Esfuerzo: 1,2 días*
  - Criterio de cierre: Se cumplen los criterios de aceptación de ARQ-02, DOC-05.
  - [ ] `ARQ-02` (Medio) Un catálogo auxiliar corrupto o de versión futura (tareas reutilizables / presets de entrega) impide arrancar la app, sin mensaje
  - [ ] `DOC-05` (Medio) network-privacy.md: versión vencida y afirmación «sin crash reporting» frente a `crash-reports/`; ficha legal con fecha y ejemplo viejos
- [ ] **RV57** — **Estilos y componentes** — Con la ventana mínima (900 × 640) o con zoom 150-200 % la barra lateral ocupa del 63 al 7… (y 4 problemas más) · *Esfuerzo: 4,5 días*
  - Criterio de cierre: Se cumplen los criterios de aceptación de UX-02, ACC-02, ACC-03, ACC-04, ACC-07.
  - [ ] `UX-02` (Medio) Con la ventana mínima (900 × 640) o con zoom 150-200 % la barra lateral ocupa del 63 al 78 % de la altura útil
  - [ ] `ACC-02` (Medio) En tema oscuro, el texto de los gráficos y la barra de progreso usan el token equivocado y quedan entre 1,08:1 y 1,53:1
  - [ ] `ACC-03` (Medio) Bordes de controles de formulario a 1,2-2,0:1 en todos los temas (exigible ≥ 3:1)
  - [ ] `ACC-04` (Medio) Objetivos táctiles y tamaños de texto por debajo de lo que promete `DESIGN.md` (candidato D)
  - [ ] `ACC-07` (Medio) El enlace «Saltar al contenido principal» es ilegible con el foco en el tema oscuro (1,19:1)
- [ ] **RV58** — **Estilos y componentes** — Botones «Eliminar» de proyectos y acciones peligrosas con color fijo: 2,05:1 en oscuro · *Esfuerzo: 2 h*
  - Criterio de cierre: Se cumplen los criterios de aceptación de ACC-08.
  - [ ] `ACC-08` (Medio) Botones «Eliminar» de proyectos y acciones peligrosas con color fijo: 2,05:1 en oscuro
- [ ] **RV59** — **Proyectos** — Tras actualizar o eliminar un proyecto el foco cae en `<body>` y la lista se vacía un ins… (y 1 problemas más) · *Esfuerzo: 4 h*
  - Criterio de cierre: Se cumplen los criterios de aceptación de ACC-05, TXT-02.
  - [ ] `ACC-05` (Medio) Tras actualizar o eliminar un proyecto el foco cae en `<body>` y la lista se vacía un instante
  - [ ] `TXT-02` (Medio) El saneado de errores recorta el motivo real tras una ruta (p. ej. eliminar con carpeta ausente)
- [ ] **RV60** — **E2E** — CSS: cascada por acumulación de parches (bloques duplicados, tokens definidos dos veces,… (y 2 problemas más) · *Esfuerzo: 3 días*
  - Criterio de cierre: Se cumplen los criterios de aceptación de COD-02, QA-06, QA-07.
  - [ ] `COD-02` (Medio) CSS: cascada por acumulación de parches (bloques duplicados, tokens definidos dos veces, `!important`, token inexistente, arnés de pruebas en producción)
  - [ ] `QA-06` (Medio) Los E2E duplican el mock IPC en 5 archivos, reimplementan lógica de Rust en el mock y por tanto no detectan fallos de contrato ni de motor
  - [ ] `QA-07` (Medio) El E2E de contraste cubre 3 de los 5 temas y no mide SVG, `progress`, bordes ni Cargar/Explorar
- [ ] **RV61** — **Pruebas del motor** — Tres tests de publicación source-backed ejercen una copia `#[cfg(test)]`, no la función d… (y 4 problemas más) · *Esfuerzo: 2,8 días*
  - Criterio de cierre: Se cumplen los criterios de aceptación de QA-02, QA-03, QA-04, QA-15, QA-16.
  - [ ] `QA-02` (Medio) Tres tests de publicación source-backed ejercen una copia `#[cfg(test)]`, no la función de producción
  - [ ] `QA-03` (Medio) Dos tests fijan como correcto el umbral del 90 % de coincidencia y la anulación silenciosa de lo no coincidente
  - [ ] `QA-04` (Medio) La protección contra enlaces y puntos de reanálisis en los destinos no tiene ninguna prueba efectiva
  - [ ] `QA-15` (Medio) Recetas: ningún test de equivalencia lazy↔eager ni con datos en varios chunks; los tests lazy usan 2-6 filas
  - [ ] `QA-16` (Medio) La suite fija como esperado que un entero de Excel vuelva como `1.0` (ida y vuelta xlsx cambia i64→f64)
- [ ] **RV62** — **Pruebas del motor** — El test de ida y vuelta solo cubre el camino eager de exportación; el source-backed (`all… (y 2 problemas más) · *Esfuerzo: 1,5 días*
  - Criterio de cierre: Se cumplen los criterios de aceptación de QA-17, QA-18, QA-19.
  - [ ] `QA-17` (Medio) El test de ida y vuelta solo cubre el camino eager de exportación; el source-backed (`all_varchar`) no tiene equivalente
  - [x] `QA-18` (Medio) Explorar: ningún test cruza el filtro de rango con los intervalos del histograma (rango cerrado frente a bins semiabiertos) — hecho el 2026-10-03
  - [ ] `QA-19` (Medio) No hay ningún test de perfil numérico con NaN o infinito (el `expect` de Q1/Q3 en `src-tauri/src/dataset/numeric_profile.rs:549-550` queda sin cubrir)
- [ ] **RV63** — **Documentación** — CONTEXTO.md: cifras e inventario de IPC desactualizados y contradictorios entre sí (y 2 problemas más) · *Esfuerzo: 1,5 días*
  - Criterio de cierre: Se cumplen los criterios de aceptación de DOC-01, DOC-03, DOC-04.
  - [ ] `DOC-01` (Medio) CONTEXTO.md: cifras e inventario de IPC desactualizados y contradictorios entre sí
  - [ ] `DOC-03` (Medio) AUDITORIA.md: resultados de gates con fecha vieja o ya falsos
  - [ ] `DOC-04` (Medio) CHANGELOG: la versión vigente 1.26.0 no tiene sección y el archivo funciona como diario de trabajo
- [ ] **RV64** — **Configuración de Rust y Tauri** — El único ADR está obsoleto y contradice la realidad; decisiones mayores sin ADR · *Esfuerzo: 1 día*
  - Criterio de cierre: Se cumplen los criterios de aceptación de DOC-06.
  - [ ] `DOC-06` (Medio) El único ADR está obsoleto y contradice la realidad; decisiones mayores sin ADR

### Tier 3 — Pulido y mantenimiento

- [ ] **RV65** — **Perfil** — Perfil con `inf`/NaN: se degrada sin avisar (no cae) (y 5 problemas más) · *Esfuerzo: 3 días*
  - Criterio de cierre: Se cumplen los criterios de aceptación de FUN-43, DAT-20, REN-12, REN-13, COD-17, LIM-10.
  - [ ] `FUN-43` (Bajo) Perfil con `inf`/NaN: se degrada sin avisar (no cae)
  - [ ] `DAT-20` (Bajo) Los ejemplos se crean una sola vez y nunca se verifican: un archivo truncado o editado se reutiliza para siempre
  - [ ] `REN-12` (Bajo) Duplicados parecidos: con muchas filas casi idénticas un solo cubo concentra todas las huellas y se carga entero en memoria
  - [ ] `REN-13` (Bajo) Perfil de origen grande: una corrida ordenada por cada 262.144 valores y por columna numérica, todas vivas hasta el final
  - [ ] `COD-17` (Bajo) Perfilado en paralelo: `expect` sobre perfiles que pueden faltar si el candado de la cola se envenena
  - [ ] `LIM-10` (Bajo) `privacy_signal` clasifica por subcadena y produce falsos positivos
- [ ] **RV66** — **Perfil** — Suites pequeñas: perfil de importación con coma decimal solo se serializa (nunca se aplica) y la migración solo se prue… · *Esfuerzo: 1 día*
  - Criterio de cierre: Se cumplen los criterios de aceptación de QA-44.
  - [ ] `QA-44` (Bajo) Suites pequeñas: perfil de importación con coma decimal solo se serializa (nunca se aplica) y la migración solo se prueba desde v12
- [ ] **RV67** — **Estilos y componentes** — El cronómetro de la operación se pone a 00:00 al pulsar «Cancelar» (y 5 problemas más) · *Esfuerzo: 1,5 días*
  - Criterio de cierre: Se cumplen los criterios de aceptación de FUN-44, REN-09, ACC-13, ACC-14, ACC-15, ACC-16.
  - [ ] `FUN-44` (Bajo) El cronómetro de la operación se pone a 00:00 al pulsar «Cancelar»
  - [ ] `REN-09` (Bajo) Sondeo de ResourceMonitor: sin protección de solapamiento ni pausa por ventana oculta (hipótesis)
  - [ ] `ACC-13` (Bajo) Celdas de correlación a 4,19-4,49:1 (texto en negrita de 15 px)
  - [ ] `ACC-14` (Bajo) La lista de etapas del informe de diagnóstico omite Explorar
  - [ ] `ACC-15` (Bajo) Mensaje de error del actualizador con color fijo `#a33e35`: 1,78:1 sobre el panel de la barra lateral
  - [ ] `ACC-16` (Bajo) Texto de acento sobre superficie sutil por debajo de 4,5:1 en el tema Ocean
- [ ] **RV68** — **Estilos y componentes** — En tema oscuro los avisos ámbar (`.notice`) se pintan de verde «éxito» (y 2 problemas más) · *Esfuerzo: 6 h*
  - Criterio de cierre: Se cumplen los criterios de aceptación de UI-01, TXT-10, QA-32.
  - [ ] `UI-01` (Bajo) En tema oscuro los avisos ámbar (`.notice`) se pintan de verde «éxito»
  - [ ] `TXT-10` (Bajo) Textos y detección por texto en CellText y UpdatePanel
  - [ ] `QA-32` (Bajo) Varios tests de estilos son regex sobre el texto del CSS y uno fija una declaración muerta
- [ ] **RV69** — **Preparar** — La vista previa de columnas ignora el resumen agrupado, que reescribe todo el esquema (y 5 problemas más) · *Esfuerzo: 2,2 días*
  - Criterio de cierre: Se cumplen los criterios de aceptación de FUN-45, FUN-46, FUN-47, UX-10, UX-12, UX-13.
  - [ ] `FUN-45` (Bajo) La vista previa de columnas ignora el resumen agrupado, que reescribe todo el esquema
  - [ ] `FUN-46` (Bajo) `pendingPlanComparison` no se limpia al cancelar: la siguiente operación ajena aparece como «Listo: cambios aplicados» de la propuesta
  - [ ] `FUN-47` (Bajo) «Completar categorías desconocidas» y la imputación avanzada se ofrecen para cualquier columna de texto con nulos, sin las guardas de la propuesta (identificadores, datos personales, fechas)
  - [ ] `UX-10` (Bajo) «Listo: cambios aplicados» solo aparece cuando termina el reperfilado, 10–13 s después de que los cambios ya estaban aplicados
  - [ ] `UX-12` (Bajo) `RevisionComparison`: una comparación cancelada se muestra como error; cancelar sin éxito deja el botón bloqueado; textos con jerga interna
  - [ ] `UX-13` (Bajo) Un borrador inválido no se conserva al salir de la pestaña
- [ ] **RV70** — **Preparar** — «Imputar outliers con mediana» se aplica sin confirmación mientras «Limitar» y «Eliminar»… (y 5 problemas más) · *Esfuerzo: 1,5 días*
  - Criterio de cierre: Se cumplen los criterios de aceptación de UX-14, ACC-17, TXT-11, COD-08, COD-09, LIM-05.
  - [ ] `UX-14` (Bajo) «Imputar outliers con mediana» se aplica sin confirmación mientras «Limitar» y «Eliminar» sí la piden
  - [ ] `ACC-17` (Bajo) El foco se pierde al cambiar entre propuesta, pasos y resultado
  - [ ] `TXT-11` (Bajo) Marcas de código literales y anglicismos en textos de interfaz
  - [ ] `COD-08` (Bajo) Umbral «80 %» y otros textos del motor duplicados como literales en el controlador
  - [ ] `COD-09` (Bajo) `recipeSchema`: `dateParses` se auto-satisface y ofrece cualquier columna como compatible; tipos enteros sin signo se tratan como cambio de tipo
  - [ ] `LIM-05` (Bajo) JSDoc huérfano y textos auxiliares mal ubicados en `src/features/prepare/proposalModel.ts`
- [ ] **RV71** — **Preparar** — Props declaradas y no usadas; manejadores opcionales con no-op silencioso (y 1 problemas más) · *Esfuerzo: 4 h*
  - Criterio de cierre: Se cumplen los criterios de aceptación de LIM-06, QA-33.
  - [ ] `LIM-06` (Bajo) Props declaradas y no usadas; manejadores opcionales con no-op silencioso
  - [ ] `QA-33` (Bajo) `src/features/prepare/proposalModel.test.ts` / `src/features/prepare/PrepareProposal.test.tsx`: los casos de identificadores son solo en inglés y el botón puede decir «Aplicar 0 cambios»
- [ ] **RV72** — **Revisar** — La preferencia global de motor/muestra se sobrescribe al abrir un proyecto (y 5 problemas más) · *Esfuerzo: 1,5 días*
  - Criterio de cierre: Se cumplen los criterios de aceptación de FUN-48, FUN-49, FUN-50, FUN-51, FUN-52, UX-15.
  - [ ] `FUN-48` (Bajo) La preferencia global de motor/muestra se sobrescribe al abrir un proyecto
  - [ ] `FUN-49` (Bajo) Formato numérico mezclado: `toLocaleString()` frente a `formatDecimal`, y «_cambios» codificado
  - [ ] `FUN-50` (Bajo) Página vacía muestra «Filas 101–100»
  - [ ] `FUN-51` (Bajo) Porcentajes redondeados a «100,0 %» con nulos reales
  - [ ] `FUN-52` (Bajo) Calendario diario supone días contiguos y resumidos
  - [ ] `UX-15` (Bajo) El ajuste «filas de muestra» dice aplicarse «al próximo análisis» pero no hay forma de lanzarlo
- [ ] **RV73** — **Revisar** — Tests de cifras dependen de la configuración regional del sistema (y 1 problemas más) · *Esfuerzo: 1,2 días*
  - Criterio de cierre: Se cumplen los criterios de aceptación de QA-34, QA-36.
  - [ ] `QA-34` (Bajo) Tests de cifras dependen de la configuración regional del sistema
  - [ ] `QA-36` (Bajo) Cobertura del controlador de Revisar: sin pruebas de carreras ni de exclusión mutua
- [ ] **RV74** — **Proyectos** — `runExclusive` y `save` abandonan en silencio si hay una operación activa o `blocked` (y 2 problemas más) · *Esfuerzo: 6 h*
  - Criterio de cierre: Se cumplen los criterios de aceptación de FUN-53, UX-17, QA-35.
  - [ ] `FUN-53` (Bajo) `runExclusive` y `save` abandonan en silencio si hay una operación activa o `blocked`
  - [ ] `UX-17` (Bajo) El botón «Guardar proyecto nuevo/Actualizar» no explica por qué está desactivado y no permite «Guardar como»
  - [ ] `QA-35` (Bajo) Tests de proyectos: errores solo con mensajes que son una ruta y paneles sin cubrir
- [ ] **RV75** — **Reglas de calidad** — La validación de regex del editor usa el motor de JavaScript, no el de Rust (y 4 problemas más) · *Esfuerzo: 2 días*
  - Criterio de cierre: Se cumplen los criterios de aceptación de FUN-54, FUN-76, SEG-05, REN-11, COD-16.
  - [ ] `FUN-54` (Bajo) La validación de regex del editor usa el motor de JavaScript, no el de Rust
  - [ ] `FUN-76` (Bajo) Semántica inconsistente de nulos/ceros con signo en reglas entre filas
  - [ ] `SEG-05` (Bajo) La compuerta de calidad la decide quien llama: Rust valida las reglas que recibe por IPC, sin atarlas a un contrato guardado y aprobado
  - [ ] `REN-11` (Bajo) `allowed_values`, `referential_integrity` y claves `unique_together`: coste O(filas × referencias) y asignaciones por celda
  - [ ] `COD-16` (Bajo) El presupuesto de texto del contrato omite campos (`baseline`, `dtype`, `minDate`, `maxDate`, `then.referenceValues`)
- [ ] **RV76** — **Entregar** — Fechas de `date_range` con formato ambiguo se interpretan con `Date.parse` del motor JS (y 5 problemas más) · *Esfuerzo: 2,2 días*
  - Criterio de cierre: Se cumplen los criterios de aceptación de FUN-55, FUN-56, UX-18, UX-19, ACC-12, PROD-05.
  - [ ] `FUN-55` (Bajo) Fechas de `date_range` con formato ambiguo se interpretan con `Date.parse` del motor JS
  - [ ] `FUN-56` (Bajo) Una exportación en curso que queda obsoleta bloquea en silencio la siguiente y su resultado se descarta
  - [ ] `UX-18` (Bajo) «Guardar preset» sobrescribe el preset seleccionado sin avisar
  - [ ] `UX-19` (Bajo) Campos numéricos de tolerancia vuelven a 0 al vaciarlos y la tolerancia por defecto de «Comprobación agregada» es 0
  - [ ] `ACC-12` (Bajo) Nombres accesibles repetidos: varios «Eliminar» y dos «Deshacer» sin contexto
  - [x] `PROD-05` (Bajo) «Exportar y abrir en Power BI» con Parquet exporta y no abre nada, sin decirlo — hecho el 2026-10-03
- [ ] **RV77** — **Entregar** — Los presets prometen guardar «columnas», pero siempre guardan todas y al aplicar se ignoran · *Esfuerzo: 2 h*
  - Criterio de cierre: Se cumplen los criterios de aceptación de PROD-06.
  - [ ] `PROD-06` (Bajo) Los presets prometen guardar «columnas», pero siempre guardan todas y al aplicar se ignoran
- [ ] **RV78** — **Motor (dataset.rs)** — `cast_fully_numeric_columns` promete «sin pérdida» pero convierte enteros > i64 y decimal… (y 5 problemas más) · *Esfuerzo: 2,2 días*
  - Criterio de cierre: Se cumplen los criterios de aceptación de FUN-57, FUN-58, FUN-59, FUN-60, FUN-61, FUN-62.
  - [ ] `FUN-57` (Bajo) `cast_fully_numeric_columns` promete «sin pérdida» pero convierte enteros > i64 y decimales largos a `f64` sin comprobar la precisión
  - [ ] `FUN-58` (Bajo) «Normalizar texto sin acentos» elimina marcas combinantes de todas las escrituras y no recompone
  - [ ] `FUN-59` (Bajo) Un archivo UTF-16 u otro binario se ofrece como «Windows-1252»
  - [ ] `FUN-60` (Bajo) Delimitador por defecto con encabezado de una sola línea
  - [ ] `FUN-61` (Bajo) `query_dataset` (motor Polars) traga errores de DuckDB y cae a otro motor sin avisar
  - [ ] `FUN-62` (Bajo) `apply_safe_corrections` anuncia `affected_row_count` como el máximo de dos conjuntos de filas, no su unión
- [ ] **RV79** — **Motor (dataset.rs)** — `.pbids` escrito sin atomicidad ni aviso de sobrescritura (y 1 problemas más) · *Esfuerzo: 4 h*
  - Criterio de cierre: Se cumplen los criterios de aceptación de DAT-14, LIM-07.
  - [ ] `DAT-14` (Bajo) `.pbids` escrito sin atomicidad ni aviso de sobrescritura
  - [ ] `LIM-07` (Bajo) Variable muerta `eager_frame` en `join_dataset`
- [ ] **RV80** — **Consulta SQL y DuckDB** — El rechazo de `;`, `--`, `/*` se aplica al texto completo, también dentro de literales y… (y 5 problemas más) · *Esfuerzo: 2,2 días*
  - Criterio de cierre: Se cumplen los criterios de aceptación de FUN-63, FUN-64, FUN-65, COD-13, COD-14, LIM-08.
  - [ ] `FUN-63` (Bajo) El rechazo de `;`, `--`, `/*` se aplica al texto completo, también dentro de literales y nombres de columna
  - [ ] `FUN-64` (Bajo) Las claves de JOIN con punto en el nombre no se pueden usar
  - [ ] `FUN-65` (Bajo) Consultas y transmisión sobre fuentes JSON con columnas anidadas devuelven error de tipo no compatible
  - [ ] `COD-13` (Bajo) Regex compiladas en cada llamada y `expect` sobre patrones
  - [ ] `COD-14` (Bajo) Dos detectores de «JOIN» distintos para enrutar una misma consulta
  - [ ] `LIM-08` (Bajo) Duplicación de bloques de materialización a Parquet y año fijo 1900-2100
- [ ] **RV81** — **Entrega ODBC** — Mapeo de tipos a DDL: `time` → TIMESTAMP/DATETIME2 y DECIMAL → DOUBLE (y 3 problemas más) · *Esfuerzo: 1 día*
  - Criterio de cierre: Se cumplen los criterios de aceptación de FUN-66, DAT-18, SEG-09, DOC-10.
  - [ ] `FUN-66` (Bajo) Mapeo de tipos a DDL: `time` → TIMESTAMP/DATETIME2 y DECIMAL → DOUBLE
  - [ ] `DAT-18` (Bajo) El libro de entregas remotas se reinicia en silencio si el archivo está dañado o cambia de versión, y no tiene bloqueo entre instancias
  - [ ] `SEG-09` (Bajo) El aviso de cifrado trata como «local» cualquier servidor cuyo nombre contenga «localhost» o «127.0.0.1»
  - [ ] `DOC-10` (Bajo) Variables de entorno de construcción/ejecución sin documentar
- [ ] **RV82** — **Arranque y servicios** — El evento final de progreso de la descarga de actualización siempre informa 0 bytes (y 3 problemas más) · *Esfuerzo: 1 día*
  - Criterio de cierre: Se cumplen los criterios de aceptación de FUN-67, DAT-19, SEG-08, COD-11.
  - [ ] `FUN-67` (Bajo) El evento final de progreso de la descarga de actualización siempre informa 0 bytes
  - [ ] `DAT-19` (Bajo) `reusable-tasks.sqlite3` se reescribe en cada arranque (migración no idempotente)
  - [ ] `SEG-08` (Bajo) CSP con `style-src 'unsafe-inline'` y updater con `endpoints: []` pero plugin cargado
  - [ ] `COD-11` (Bajo) `get_performance_settings` usa `lock()` y falla con veneno; el resto del módulo usa `lock_recovering`
- [ ] **RV83** — **Exportación y libros** — Un destino XLSX con más de 16.384 columnas produce un libro que Excel no abre (y 3 problemas más) · *Esfuerzo: 1,8 días*
  - Criterio de cierre: Se cumplen los criterios de aceptación de FUN-68, TXT-07, COD-15, QA-55.
  - [ ] `FUN-68` (Bajo) Un destino XLSX con más de 16.384 columnas produce un libro que Excel no abre
  - [ ] `TXT-07` (Bajo) El paquete ZIP promete receta y el resumen dice «no incluida» aunque se aplicaron cambios; el mensaje de la UI lista un contenido distinto del real
  - [ ] `COD-15` (Bajo) Candidatos de rendimiento sin medir en exportación y updater
  - [ ] `QA-55` (Bajo) Mutantes no detectados en la detección de números con signo y en el analizador de fechas
- [ ] **RV84** — **CLI** — Parser CLI: valores que empiezan por `--` rechazados y sin separador `--` (y 5 problemas más) · *Esfuerzo: 1,5 días*
  - Criterio de cierre: Se cumplen los criterios de aceptación de FUN-69, DAT-13, SEG-10, UX-07, DOC-14, DOC-18.
  - [ ] `FUN-69` (Bajo) Parser CLI: valores que empiezan por `--` rechazados y sin separador `--`
  - [ ] `DAT-13` (Bajo) `project-list` crea la carpeta del almacén y lista proyectos cuyo snapshot ya no existe
  - [ ] `SEG-10` (Bajo) `sanitize_error`/stderr: la CLI imprime errores sin sanear y la heurística de rutas tiene huecos
  - [ ] `UX-07` (Bajo) Mensajes de error de la CLI que no explican la causa
  - [ ] `DOC-14` (Bajo) cli.md, v1-scope.md y feature-parity.md: omisiones, subcomando mal ubicado y celda ilegible
  - [ ] `DOC-18` (Bajo) `inspect` devuelve `str` para todas las columnas de un CSV y tipos reales en libros
- [ ] **RV85** — **Motor de recetas** — Huellas normalizadas: el camino ASCII y el Unicode no coinciden en el espacio vertical (U… (y 5 problemas más) · *Esfuerzo: 1,5 días*
  - Criterio de cierre: Se cumplen los criterios de aceptación de FUN-70, FUN-71, FUN-72, FUN-73, FUN-74, FUN-75.
  - [ ] `FUN-70` (Bajo) Huellas normalizadas: el camino ASCII y el Unicode no coinciden en el espacio vertical (U+000B)
  - [ ] `FUN-71` (Bajo) «Quitar columnas constantes» elimina columnas que mezclan un valor único con nulos
  - [ ] `FUN-72` (Bajo) Validación de receta v1 no rechaza `export_options` y la validación de presupuesto de texto omite campos
  - [ ] `FUN-73` (Bajo) Mínimo/máximo de texto en el camino eager compara `AnyValue::to_string()` (posiblemente con comillas) y no el texto
  - [ ] `FUN-74` (Bajo) El recuento de «celdas reemplazadas» incluye columnas que `keepColumns` descarta después
  - [ ] `FUN-75` (Bajo) Orden de salida de grupos y fechas en blanco en el camino «source-backed» dependen de DuckDB
- [ ] **RV86** — **Motor de recetas** — Mensajes de error con jerga interna y mezcla de inglés (y 1 problemas más) · *Esfuerzo: 4 h*
  - Criterio de cierre: Se cumplen los criterios de aceptación de TXT-14, LIM-09.
  - [ ] `TXT-14` (Bajo) Mensajes de error con jerga interna y mezcla de inglés
  - [ ] `LIM-09` (Bajo) `load_recipe_file`: segunda rama inalcanzable y nombre sugerido sin proteger nombres reservados de Windows
- [ ] **RV87** — **Explorar** — Explorar: filtros con lista vacía se ignoran, y los valores no tienen tope (y 4 problemas más) · *Esfuerzo: 1,2 días*
  - Criterio de cierre: Se cumplen los criterios de aceptación de FUN-77, FUN-78, UX-09, UX-11, TXT-06.
  - [ ] `FUN-77` (Bajo) Explorar: filtros con lista vacía se ignoran, y los valores no tienen tope
  - [ ] `FUN-78` (Bajo) Explorar: histograma y KPIs descartan en silencio texto no numérico, nulos y no finitos
  - [ ] `UX-09` (Bajo) Observaciones menores
  - [ ] `UX-11` (Bajo) Primera carga de Explorar sin indicador: pantalla vacía hasta que llega el panel
  - [ ] `TXT-06` (Bajo) Años e identificadores con separador de millares («release_year de 1,925 a 1,929.8», «Mín. 1,001»)
- [ ] **RV88** — **Importación** — Muestra de encabezados CSV con fin de línea CR solo falla en archivos grandes (y 4 problemas más) · *Esfuerzo: 2,8 días*
  - Criterio de cierre: Se cumplen los criterios de aceptación de FUN-79, FUN-80, FUN-81, ARQ-07, COD-18.
  - [ ] `FUN-79` (Bajo) Muestra de encabezados CSV con fin de línea CR solo falla en archivos grandes
  - [ ] `FUN-80` (Bajo) Una columna con marcadores de nulo (`NA`, `-`, `N/A`) queda entera como texto bajo convención numérica/fecha, sin informar
  - [ ] `FUN-81` (Bajo) Un JSON cuyo raíz es un objeto envolvente (`{"data":[...]}`) se carga como una sola fila
  - [ ] `ARQ-07` (Bajo) La paginación directa traga cualquier error de la fuente y cae a materializar el dataset entero, con el mutex del dataset tomado durante toda la E/S
  - [ ] `COD-18` (Bajo) `unwrap_or(HistoryManager::deferred()?)` crea un directorio temporal aunque no se use
- [ ] **RV89** — **Comparación** — Claves duplicadas y nulas: filas excluidas de conflictos y sin recuento de filas afectadas (y 5 problemas más) · *Esfuerzo: 1,5 días*
  - Criterio de cierre: Se cumplen los criterios de aceptación de FUN-82, ARQ-08, UX-16, TXT-12, LIM-11, QA-45.
  - [ ] `FUN-82` (Bajo) Claves duplicadas y nulas: filas excluidas de conflictos y sin recuento de filas afectadas
  - [ ] `ARQ-08` (Bajo) Fallo de la comparación por Parquet se reintenta en silencio materializando el activo en RAM
  - [ ] `UX-16` (Bajo) Unir/consolidar con la comparación aún en curso es un clic sin efecto
  - [ ] `TXT-12` (Bajo) Textos de conflictos: «Esta página está completa» siempre, y `null` indistinguible del texto «null»
  - [ ] `LIM-11` (Bajo) Los archivos `.jsonl`/`.ndjson` comparados van por la ruta en memoria, distinta de la de `.json`
  - [ ] `QA-45` (Bajo) El test de comparación de revisiones manipula `history.cursor` directamente y las aserciones de privacidad usan `contains` de cadenas
- [ ] **RV90** — **Proyectos e historial (motor)** — Un único respaldo de versión ilegible hace fallar el listado completo de versiones (y 5 problemas más) · *Esfuerzo: 3 días*
  - Criterio de cierre: Se cumplen los criterios de aceptación de DAT-15, DAT-16, DAT-17, REN-10, COD-12, QA-46.
  - [ ] `DAT-15` (Bajo) Un único respaldo de versión ilegible hace fallar el listado completo de versiones
  - [ ] `DAT-16` (Bajo) El guardado valida estrictamente campos de vista (p. ej. `previewOffset`) y puede rechazar guardar el dataset por un dato cosmético
  - [ ] `DAT-17` (Bajo) `sync_directory` es un no-op fuera de Unix: en Windows la durabilidad del renombrado de la generación no se garantiza antes de confirmar el catálogo
  - [ ] `REN-10` (Bajo) Cada autoguardado reescribe una generación completa (dataset + hasta 12 snapshots de historial) con el mutex de operaciones de proyecto tomado (candidato a medir)
  - [ ] `COD-12` (Bajo) `queue_dropped_path` y `remember_last_export` ignoran en silencio un mutex envenenado
  - [ ] `QA-46` (Bajo) Huecos de prueba en la persistencia: migraciones v3/v4/v5/v12/v14, borrado con generación ausente, staging huérfano; tests de reparse point que pasan en silencio sin privilegios
- [ ] **RV91** — **App y puente** — `envPrefix: ["VITE_", "TAURI_"]` permitiría incrustar en el bundle las variables de firma… (y 5 problemas más) · *Esfuerzo: 2,2 días*
  - Criterio de cierre: Se cumplen los criterios de aceptación de SEG-06, ACC-11, TXT-03, TXT-08, TXT-09, COD-04.
  - [ ] `SEG-06` (Bajo) `envPrefix: ["VITE_", "TAURI_"]` permitiría incrustar en el bundle las variables de firma del updater
  - [ ] `ACC-11` (Bajo) Texto oculto «Carga un dataset…» fuera de cualquier landmark (axe `region`)
  - [ ] `TXT-03` (Bajo) El texto legal visible aún lleva un recordatorio interno: «deben definirse… antes de distribuir»
  - [ ] `TXT-08` (Bajo) `formatDataType` deja en inglés `Categorical`/`Binary`/`Enum`; `isTextType` no reconoce `categorical`
  - [ ] `TXT-09` (Bajo) `formatBytes` produce «NaN B»/«Infinity B» y «1024 KiB»
  - [ ] `COD-04` (Bajo) El puente no valida en ejecución ninguna respuesta de Rust
- [ ] **RV92** — **App y puente** — Estado `loading` del motor inalcanzable y «Motor local listo» antes de confirmar (y 5 problemas más) · *Esfuerzo: 3 días*
  - Criterio de cierre: Se cumplen los criterios de aceptación de COD-05, COD-06, COD-07, LIM-04, QA-29, QA-30.
  - [ ] `COD-05` (Bajo) Estado `loading` del motor inalcanzable y «Motor local listo» antes de confirmar
  - [ ] `COD-06` (Bajo) oxlint solo con la categoría `correctness`, reglas de React clave desactivadas y avisos que no rompen `npm run lint`
  - [ ] `COD-07` (Bajo) `tsconfig` sin `noUncheckedIndexedAccess` ni `noUnused*`
  - [ ] `LIM-04` (Bajo) Configuración de E2E incoherente (Edge frente a Chromium instalado; reintentos solo en CI inexistente)
  - [ ] `QA-29` (Bajo) El test de paridad IPC compara nombres de campos y formas, no variantes de enums ni la constante de cancelación
  - [ ] `QA-30` (Bajo) `src/bridge.test.ts` prueba sobre todo que el mock devuelve lo que se le dijo; 14 funciones del puente sin test
- [ ] **RV93** — **App y puente** — `tsc --noEmit` y el tipado no cubren `e2e/` ni `tools/` · *Esfuerzo: 2 h*
  - Criterio de cierre: Se cumplen los criterios de aceptación de QA-31.
  - [ ] `QA-31` (Bajo) `tsc --noEmit` y el tipado no cubren `e2e/` ni `tools/`
- [ ] **RV94** — **Cargar** — El saneado de rutas en errores de tareas no cubre rutas UNC y borra el motivo del error (y 5 problemas más) · *Esfuerzo: 1,5 días*
  - Criterio de cierre: Se cumplen los criterios de aceptación de SEG-07, TXT-04, TXT-13, PROD-07, COD-10, LIM-03.
  - [ ] `SEG-07` (Bajo) El saneado de rutas en errores de tareas no cubre rutas UNC y borra el motivo del error
  - [ ] `TXT-04` (Bajo) «privacidad none»: valor interno visible en el resumen de la tarea reutilizable
  - [ ] `TXT-13` (Bajo) El texto «sin límite fijo de tamaño» contradice el límite de carga en memoria que el propio diálogo menciona
  - [ ] `PROD-07` (Bajo) Las tareas reutilizables no se pueden borrar ni renombrar desde la interfaz
  - [ ] `COD-10` (Bajo) Claves de React duplicadas en la vista previa si el archivo trae encabezados repetidos
  - [ ] `LIM-03` (Bajo) Exportaciones sin uso en el frontend
- [ ] **RV95** — **Cargar** — Tres pruebas de LoadPhase quedaron fuera del `describe` por una llave mal colocada (y 1 problemas más) · *Esfuerzo: 4 h*
  - Criterio de cierre: Se cumplen los criterios de aceptación de QA-37, QA-38.
  - [ ] `QA-37` (Bajo) Tres pruebas de LoadPhase quedaron fuera del `describe` por una llave mal colocada
  - [ ] `QA-38` (Bajo) Huecos de prueba en el modelo de Cargar y en el de reglas importadas
- [ ] **RV96** — **Gates y scripts** — Verificaciones criptográficas parciales en el updater (comentario de confianza y política… (y 5 problemas más) · *Esfuerzo: 1,5 días*
  - Criterio de cierre: Se cumplen los criterios de aceptación de SEG-11, SEG-12, COD-19, COD-20, LIM-12, LIM-13.
  - [ ] `SEG-11` (Bajo) Verificaciones criptográficas parciales en el updater (comentario de confianza y política de claves)
  - [ ] `SEG-12` (Bajo) `probe-webview2-native-selectors --prepare-flow` deja copias del dataset del usuario en `%TEMP%` con nombres fijos
  - [ ] `COD-19` (Bajo) Robustez menor de los gates (varios, causa común: validación de entrada débil)
  - [ ] `COD-20` (Bajo) `check-retired-brand` divide por un regex con barras escapadas de más: siempre informa la línea 1
  - [ ] `LIM-12` (Bajo) Código muerto y mensaje desfasado en los gates pequeños
  - [ ] `LIM-13` (Bajo) Plantillas de GitHub desfasadas respecto al proyecto
- [ ] **RV97** — **Gates y scripts** — SBOM: mezcla dependencias de desarrollo/otras plataformas y no distingue alcance (y 5 problemas más) · *Esfuerzo: 1,5 días*
  - Criterio de cierre: Se cumplen los criterios de aceptación de LIM-14, LIM-15, QA-47, QA-48, QA-49, QA-50.
  - [ ] `LIM-14` (Bajo) SBOM: mezcla dependencias de desarrollo/otras plataformas y no distingue alcance
  - [ ] `LIM-15` (Bajo) `skills-lock.json` solo fija 11 de las 18 skills versionadas en `.agents/` y no está documentado
  - [ ] `QA-47` (Bajo) probe-roundtrip: el veredicto de cada fixture depende solo de `$?`, no del código de salida
  - [ ] `QA-48` (Bajo) verify-tier / prepare-beta-gate: pequeñeces de robustez
  - [ ] `QA-49` (Bajo) El gate de documentación acopla CI local a prosa de Beta y a nombres de archivo
  - [ ] `QA-50` (Bajo) `tools/test-updater-manifest.mjs` omite los casos de seguridad clave
- [ ] **RV98** — **Gates y scripts** — Filtro de privacidad del resumen Beta: regex de ruta mal escrita y falsos positivos con U… (y 5 problemas más) · *Esfuerzo: 1,5 días*
  - Criterio de cierre: Se cumplen los criterios de aceptación de QA-51, QA-52, QA-53, QA-54, OPS-11, OPS-12.
  - [ ] `QA-51` (Bajo) Filtro de privacidad del resumen Beta: regex de ruta mal escrita y falsos positivos con URL
  - [ ] `QA-52` (Bajo) `check-incremental-matrix --run-native` puede pasar sin ejecutar ninguna prueba
  - [ ] `QA-53` (Bajo) `tools/check-beta-workflows.test.mjs` prueba las fixtures contra sí mismas, no flujos
  - [ ] `QA-54` (Bajo) Rigidez y exit code de las sondas CDP
  - [ ] `OPS-11` (Bajo) Los mensajes con tilde de los scripts salen ilegibles en Windows PowerShell 5.1
  - [ ] `OPS-12` (Bajo) release.ps1 genera el manifiesto del updater con las notas de `## [Unreleased]` salvo que se pase `-UpdaterNotesPath`
- [ ] **RV99** — **Gates y scripts** — El MSI es por máquina y solo en inglés, mientras el contrato del instalador solo vigila e… (y 5 problemas más) · *Esfuerzo: 1,5 días*
  - Criterio de cierre: Se cumplen los criterios de aceptación de OPS-14, OPS-17, OPS-18, OPS-19, OPS-20, OPS-21.
  - [ ] `OPS-14` (Bajo) El MSI es por máquina y solo en inglés, mientras el contrato del instalador solo vigila el NSIS por usuario
  - [ ] `OPS-17` (Bajo) check-supply-chain: avisos ignorados fijos en el script y herramientas «no disponibles» no bloquean fuera de release
  - [ ] `OPS-18` (Bajo) release.ps1: `-DryRun` no se diferencia de una ejecución real y los fallos no imprimen la ruta del reporte
  - [ ] `OPS-19` (Bajo) release.ps1: `$LASTEXITCODE` sin reiniciar y `node` del manifiesto sin comprobar
  - [ ] `OPS-20` (Bajo) check.ps1: el reporte «passed» fija commit y árbol al inicio, y el perfil Package ejecuta el smoke del instalador sobre la máquina real
  - [ ] `OPS-21` (Bajo) Benchmarks y resumen de rendimiento: cuatro fragilidades menores en 5.1 y en la evidencia acumulada
- [ ] **RV100** — **Gates y scripts** — La sonda de proyectos en modo reinicio deja un proyecto y una tarea reutilizable en el al… (y 3 problemas más) · *Esfuerzo: 1 día*
  - Criterio de cierre: Se cumplen los criterios de aceptación de OPS-22, DOC-11, DOC-13, LEG-02.
  - [ ] `OPS-22` (Bajo) La sonda de proyectos en modo reinicio deja un proyecto y una tarea reutilizable en el almacén real si no se completa la fase de verificación
  - [ ] `DOC-11` (Bajo) CONTRIBUTING y SECURITY: invitan a contribuir a un proyecto personal, sin URL del repositorio, y SECURITY depende de un correo personal
  - [ ] `DOC-13` (Bajo) CHANGELOG (tramo 2160-2917): sección 0.58.0 gigante, «sistema anterior» como marcador sin nombre y afirmaciones de release sin respaldo
  - [ ] `LEG-02` (Bajo) `check-legal-distribution` acepta decisiones vacías en la práctica y usa rutas solo-Windows
- [ ] **RV101** — **Varios** — Dos incidencias de arranque y ventana vistas una sola vez al conducir la app por CDP (no… (y 3 problemas más) · *Esfuerzo: 2,5 días*
  - Criterio de cierre: Se cumplen los criterios de aceptación de ARQ-06, UX-08, TXT-05, QA-43.
  - [ ] `ARQ-06` (Bajo) Dos incidencias de arranque y ventana vistas una sola vez al conducir la app por CDP (no reproducidas)
  - [ ] `UX-08` (Bajo) Textos y detalles menores de Cargar/Revisar/Preparar
  - [ ] `TXT-05` (Bajo) Plurales fijos en los mensajes («1 filas», «1 columnas», «1 outliers»)
  - [ ] `QA-43` (Bajo) Datos de prueba de la mitad 2: casi todo sintético y de unas pocas filas; solo 4 tests con volumen o varios chunks
- [ ] **RV102** — **Pruebas del motor** — Fixtures temporales sin guardia RAII: se filtran carpetas y archivos si el test falla o c… (y 3 problemas más) · *Esfuerzo: 1 día*
  - Criterio de cierre: Se cumplen los criterios de aceptación de QA-39, QA-40, QA-41, QA-42.
  - [ ] `QA-39` (Bajo) Fixtures temporales sin guardia RAII: se filtran carpetas y archivos si el test falla o con `.keep()`
  - [ ] `QA-40` (Bajo) Tests que construyen estructuras del tamaño de un límite (coste de memoria/tiempo no medido)
  - [ ] `QA-41` (Bajo) Fixtures `temporary_csv` sueltos en `%TEMP%` y borrados solo en la última línea (se filtran si el test falla)
  - [ ] `QA-42` (Bajo) Aserciones débiles: solo el tipo en formatos de fecha, `||` entre mensajes y tuplas posicionales de 21 campos
- [ ] **RV103** — **Configuración de Rust y Tauri** — Ejecutables e instaladores sin firma Authenticode (y 2 problemas más) · *Esfuerzo: 1,5 días*
  - Criterio de cierre: Se cumplen los criterios de aceptación de OPS-13, OPS-15, OPS-16.
  - [ ] `OPS-13` (Bajo) Ejecutables e instaladores sin firma Authenticode
  - [ ] `OPS-15` (Bajo) `src-tauri/deny.toml` ignora diez avisos que ya no corresponden a ningún crate
  - [ ] `OPS-16` (Bajo) Sin `[profile.release]`: binarios de 129,5 MB / 112,9 MB sin LTO, strip ni codegen-units
- [ ] **RV104** — **Documentación** — README: «Rust estable» frente a la versión fijada en `rust-toolchain.toml` (y 5 problemas más) · *Esfuerzo: 1,5 días*
  - Criterio de cierre: Se cumplen los criterios de aceptación de DOC-07, DOC-08, DOC-09, DOC-12, DOC-15, DOC-16.
  - [ ] `DOC-07` (Bajo) README: «Rust estable» frente a la versión fijada en `rust-toolchain.toml`
  - [ ] `DOC-08` (Bajo) README: «cuatro etapas» frente a las cinco de ROADMAP; enlaza la beta que está aparcada
  - [ ] `DOC-09` (Bajo) CONTEXTO.md cita scripts npm y herramientas que no existen o duplicadas
  - [ ] `DOC-12` (Bajo) CHANGELOG cita un archivo que no existe en la ruta indicada
  - [ ] `DOC-15` (Bajo) Tutorial y guía de release con requisitos vagos y ejemplos de versiones muertas
  - [ ] `DOC-16` (Bajo) Guías de beta, NVDA y release: referencias a niveles retirados, flujo sin «Explorar» y 0.95.0 otra vez
- [ ] **RV105** — **Documentación** — Galería del README fechada antes de los rediseños y de Explorar; plantillas beta con «Gat… (y 1 problemas más) · *Esfuerzo: 1,2 días*
  - Criterio de cierre: Se cumplen los criterios de aceptación de DOC-17, LEG-01.
  - [ ] `DOC-17` (Bajo) Galería del README fechada antes de los rediseños y de Explorar; plantillas beta con «Gate 2 · shell de espacios»
  - [ ] `LEG-01` (Bajo) THIRD_PARTY_NOTICES no incluye textos de licencia ni titulares de copyright; mezcla dependencias de otras plataformas y de desarrollo

### Tier 4 — Futuro u opcional

- [ ] **RV106** — **Datos y persistencia** — Migración del catálogo sin copia de seguridad previa del `projects.sqlite3` (y 1 problemas más) · *Esfuerzo: 4 h*
  - Criterio de cierre: Hecho lo que proponen DAT-21, DAT-22.
  - [ ] `DAT-21` (Propuesta) Migración del catálogo sin copia de seguridad previa del `projects.sqlite3`
  - [ ] `DAT-22` (Propuesta) Unificar nombres únicos entre catálogos
- [ ] **RV107** — **Seguridad** — Confirmar que un preset con `table_policy = replace` no se aplica sin confirmación · *Esfuerzo: 2 h*
  - Criterio de cierre: Hecho lo que proponen SEG-13.
  - [ ] `SEG-13` (Propuesta) Confirmar que un preset con `table_policy = replace` no se aplica sin confirmación
- [ ] **RV108** — **Arquitectura y resiliencia** — Registrar el motivo cuando una ruta source-backed cae a la ruta eager · *Esfuerzo: 2 h*
  - Criterio de cierre: Hecho lo que proponen ARQ-09.
  - [ ] `ARQ-09` (Propuesta) Registrar el motivo cuando una ruta source-backed cae a la ruta eager
- [ ] **RV109** — **Experiencia de uso** — Resolver conflictos exige decidir celda a celda en todas las páginas, sin acción masiva (y 1 problemas más) · *Esfuerzo: 1,2 días*
  - Criterio de cierre: Hecho lo que proponen UX-20, UX-21.
  - [ ] `UX-20` (Propuesta) Resolver conflictos exige decidir celda a celda en todas las páginas, sin acción masiva
  - [ ] `UX-21` (Propuesta) Reglas propuestas también cuando ya hay contrato incompleto
- [ ] **RV110** — **Accesibilidad** — Alternativa textual para los gráficos de Explorar (y 1 problemas más) · *Esfuerzo: 4 h*
  - Criterio de cierre: Hecho lo que proponen ACC-18, ACC-19.
  - [ ] `ACC-18` (Propuesta) Alternativa textual para los gráficos de Explorar
  - [ ] `ACC-19` (Propuesta) Ejecutar y registrar la checklist manual
- [ ] **RV111** — **Interfaz visual** — Token `--control-border` y tabla de contraste generada · *Esfuerzo: 2 h*
  - Criterio de cierre: Hecho lo que proponen UI-02.
  - [ ] `UI-02` (Propuesta) Token `--control-border` y tabla de contraste generada
- [ ] **RV112** — **Producto** — Error estructurado en el puente en vez de cadenas (y 5 problemas más) · *Esfuerzo: 3 días*
  - Criterio de cierre: Hecho lo que proponen PROD-08, PROD-09, PROD-10, PROD-11, PROD-12, PROD-13.
  - [ ] `PROD-08` (Propuesta) Error estructurado en el puente en vez de cadenas
  - [ ] `PROD-09` (Propuesta) «Archivos recientes» no reabre el archivo
  - [ ] `PROD-10` (Propuesta) Un único resumen «qué cambió» con celdas anuladas, filas perdidas y reversibilidad real
  - [ ] `PROD-11` (Propuesta) Receta: decimales con coma y fechas dd/mm en conversiones de tipo
  - [ ] `PROD-12` (Propuesta) Propuesta: lista de columnas e inserción con cita correcta en la consola SQL
  - [ ] `PROD-13` (Propuesta) Mostrar en la entrega el destino y un resumen de lo transformado
- [ ] **RV113** — **Producto** — Los tres informes de pánico reales son anteriores a CODE-01 y no permiten identificar el… (y 5 problemas más) · *Esfuerzo: 1,5 días*
  - Criterio de cierre: Hecho lo que proponen PROD-14, PROD-15, PROD-16, PROD-17, PROD-18, PROD-19.
  - [ ] `PROD-14` (Propuesta) Los tres informes de pánico reales son anteriores a CODE-01 y no permiten identificar el origen (3 pánicos seguidos en `tokio-rt-worker`)
  - [ ] `PROD-15` (Propuesta) Mostrar el controlador y el usuario en la confirmación nativa de la conexión remota
  - [ ] `PROD-16` (Propuesta) Cancelación de entrega ODBC solo entre filas/lotes
  - [ ] `PROD-17` (Propuesta) Exportar CSV compatible con Excel en configuración regional española
  - [ ] `PROD-18` (Propuesta) Exportar fechas a XLSX como fechas
  - [ ] `PROD-19` (Propuesta) Informar de hojas ocultas, celdas combinadas y filas de título al cargar libros
- [ ] **RV114** — **Producto** — Informar de qué muestra se usa en correlaciones y permitir su semilla/estrategia (y 1 problemas más) · *Esfuerzo: 4 h*
  - Criterio de cierre: Hecho lo que proponen PROD-20, PROD-21.
  - [ ] `PROD-20` (Propuesta) Informar de qué muestra se usa en correlaciones y permitir su semilla/estrategia
  - [ ] `PROD-21` (Propuesta) Tolerancia numérica y normalización de texto opcionales en la comparación
- [ ] **RV115** — **Refactorización y limpieza** — `SnapshotComparisonResult` anunciada como «solo agregados» pero el listado por columna muestra nombres de columna sin c… · *Esfuerzo: 2 h*
  - Criterio de cierre: Hecho lo que proponen LIM-16.
  - [ ] `LIM-16` (Propuesta) `SnapshotComparisonResult` anunciada como «solo agregados» pero el listado por columna muestra nombres de columna sin comprobar privacidad
- [ ] **RV116** — **QA y testing** — axe-core en los E2E (y 5 problemas más) · *Esfuerzo: 2,2 días*
  - Criterio de cierre: Hecho lo que proponen QA-56, QA-57, QA-58, QA-59, QA-60, QA-61.
  - [ ] `QA-56` (Propuesta) axe-core en los E2E
  - [ ] `QA-57` (Propuesta) Pruebas de mutación en los módulos críticos
  - [ ] `QA-58` (Propuesta) Probar claves de JOIN con nulos, ceros a la izquierda y espacios también en la ruta DuckDB
  - [ ] `QA-59` (Propuesta) Casos de borde de los límites numéricos en la ruta source-backed (coma decimal, día/mes ambiguo)
  - [ ] `QA-60` (Propuesta) Puerta de cobertura para Rust
  - [ ] `QA-61` (Propuesta) Un único `npm run tools:test` que ejecute todos los `node --test` de `tools/`
- [ ] **RV117** — **QA y testing** — Quitar los números fijos del inventario IPC y comprobarlo contra TypeScript · *Esfuerzo: 2 h*
  - Criterio de cierre: Hecho lo que proponen QA-62.
  - [ ] `QA-62` (Propuesta) Quitar los números fijos del inventario IPC y comprobarlo contra TypeScript
- [ ] **RV118** — **DevOps y gates** — `.gitignore` sin secretos típicos ni `.gitattributes` (y 2 problemas más) · *Esfuerzo: 6 h*
  - Criterio de cierre: Hecho lo que proponen OPS-23, OPS-24, OPS-25.
  - [ ] `OPS-23` (Propuesta) `.gitignore` sin secretos típicos ni `.gitattributes`
  - [ ] `OPS-24` (Propuesta) Un único contenedor de «evidencia» con commit, versión y árbol
  - [ ] `OPS-25` (Propuesta) Salvaguarda de la instalación real antes de los smokes que tocan HKCU/APPDATA
- [ ] **RV119** — **Documentación** — Guía de uso, solución de problemas, respaldo y restauración (y 2 problemas más) · *Esfuerzo: 6 h*
  - Criterio de cierre: Hecho lo que proponen DOC-19, DOC-20, DOC-21.
  - [ ] `DOC-19` (Propuesta) Guía de uso, solución de problemas, respaldo y restauración
  - [ ] `DOC-20` (Propuesta) Compatibilidad declarada y reconstrucción en máquina nueva
  - [ ] `DOC-21` (Propuesta) Glosario y política de nombres

## Aparcado: solo si Columnia se comparte

Estos tres objetivos dejaron la cola el 2026-10-01 porque la aplicación es de
uso personal. Se retoman únicamente si se decide dársela a otras personas.

| ID | Criterio de cierre | Se retoma si |
| --- | --- | --- |
| RV07 — Beta | Tres participantes distintos sobre el mismo release candidate, dos casos reales por sesión y al menos tres datasets; 24 de 30 tareas sin ayuda; guardar, reabrir y entrega verificados; sin P0/P1; resumen sanitizado validado con `npm run beta:check-summary`. Cada fallo dependiente de datos se reduce a una fixture sintética con su regresión antes de corregirlo. Guía: [sesión beta](docs/how-to/run-beta-validation.md) e [invitación](docs/templates/beta-invitation.md). | Otras personas van a usarla |
| RV09 — Accesibilidad nativa | Cargar → Entregar en Windows solo con teclado y NVDA, incluidos diálogos, tablas, progreso, zoom y alto contraste. Guía: [probar con NVDA](docs/how-to/run-nvda-check.md). | La usa alguien con lector de pantalla |
| RV11 — Distribución | Aprobación jurídica, prueba en VM limpia (instalación, reapertura, updater, fallos y recuperación), hashes y firmas de los artefactos descargados, y la sección `## [versión]` del CHANGELOG al cortar la versión. | Se reparten binarios |

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

- Una fila por pendiente, con su criterio de cierre. Nada de registros de
  sesiones ni cifras de pruebas. Excepción: en «Pendientes de la revisión»
  lo resuelto se marca `[x]` en vez de quitarse.
- Al cerrar algo, se quita de aquí y se describe en `CHANGELOG.md`.
- Una decisión duradera de arquitectura o de trabajo va en `CONTEXTO.md`.
