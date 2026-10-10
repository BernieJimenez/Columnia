# Roadmap de Columnia

**Actualizado:** 2026-10-08 · **Versión:** 1.26.0

**Estado:** aplicación local funcional para Windows x64, de **uso personal**. No
se comparte ni se distribuye; las mejoras se eligen por lo que su dueño nota al
usarla con sus propios archivos. Los identificadores (RVnn y códigos de
hallazgo) se explican en el [glosario](docs/reference/glosario.md).

Este documento solo contiene lo pendiente. Lo entregado está en
[`CHANGELOG.md`](CHANGELOG.md). Los roadmaps anteriores se conservan sin cambios
en [`docs/archive/`](docs/archive/README.md): el de la revisión del 2026-10-01,
con sus 97 objetivos y casillas, en
[`docs/archive/2026-10/`](docs/archive/2026-10/ROADMAP.md).

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
cerraron REN-08 y REN-13 y DAT-12 pasó a aparcado; solo queda REN-01, que no
bloquea el uso diario.

### Preparar archivos grandes

- [ ] **REN-14** — Aplicar la propuesta a un archivo muy grande (2019-Oct,
  5,6 GB) falla.
  - Hecho: convertir a número ya se hace en disco, sin cargar el archivo, y la
    simulación de la propuesta no intenta cargarlo en memoria. Sonda
    `perf_probe_apply_proposal`.
  - Falta: «Quitar 30.220 filas duplicadas» agota los 8 GB de disco temporal
    que DuckDB tiene permitidos (consulta de ventana sobre 42 M filas y 9
    columnas; agrupar por todas las columnas tampoco cupo). Opciones: quitar
    duplicados con las huellas de fila del perfil o subir el límite de disco
    temporal según el espacio libre.

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
  - Falta: bajar 2019-Oct a ≤ 60 s (64 s; al final se esperan unos 9 s al
    conteo de valores distintos de DuckDB) y, para reutilizar el perfil tras
    un cambio, actualizar duplicados y filas parecidas sin releer todo. Con
    100 M filas (CSV sintético de 4,8 GB) queda una espera de 48 s al cerrar
    el conteo de valores distintos de una columna con 100 M valores únicos,
    fase en la que DuckDB no informa de progreso.

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
