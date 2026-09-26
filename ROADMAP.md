# Roadmap de Columnia

**Actualizado:** 2026-09-26 · **Versión:** 1.26.0

**Estado:** prototipo local funcional para Windows x64. Faltan la beta con
personas y datos de trabajo, la prueba con lector de pantalla y, si se decide,
la distribución de binarios.

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
4. **Entregar:** validar un contrato y exportar una copia segura.

Columnia propone y la persona aprueba: nunca cambia datos sin un clic. Tauri 2 y
Rust para el shell y el motor; React, TypeScript y Vite para la interfaz; Polars
y DuckDB para procesar en local. Sin cuentas, telemetría ni sincronización
remota. El archivo original no se modifica.

## Cola vigente

| Orden | ID | Criterio de cierre | Depende de |
| --- | --- | --- | --- |
| 1 | RV21 — Invariantes de ida y vuelta | Batería en el perfil Full con fixtures sintéticas realistas (CR-only, Windows-1252, identificador con 25 % de vacíos, negativos, caracteres de control y más de 1.048.576 filas generadas al vuelo): cargar → propuesta por defecto → exportar a CSV, XLSX y SQL → releer y comprobar tipos, que lo anunciado sea lo aplicado y que el archivo abra con un parser estricto. Falla hoy por cada uno de RV17–RV20. Hallazgo QA-02 de la [re-auditoría 2026-09-26](docs/auditorias/2026-09-26/informe.md). | Ninguna |
| 2 | RV17 — Propuesta de Preparar fiel | La propuesta por defecto no imputa identificadores, texto libre ni fechas (o lo hace con «Desconocido» y columna de auditoría, desmarcado); el antes/después muestra valor y recuento por columna; los recuentos anunciados salen de simular la cadena completa y coinciden con las celdas cambiadas. Con `OnlineRetail`, `CustomerID` y `Description` no cambian. Hallazgos FUN-03 y FUN-06. | RV21 |
| 3 | RV18 — Números que siguen siendo números | CSV exporta `-1`, `-11062.06` y `+3` sin apóstrofo y sigue neutralizando `=`, `@`, `+1+1` y `-2+3`; la propuesta ofrece tipar columnas numéricas y de fecha casi completas, y la entrega a SQL Server crea columnas numéricas. Hallazgos FUN-04 y FUN-07. | RV21 |
| 4 | RV19 — Excel que abre | El XLSX elimina los caracteres no válidos en XML 1.0 e informa de cuántas celdas tocó; la comprobación previa de Entregar avisa o propone alternativa con más de 1.048.575 filas o celdas de más de 32.767 caracteres; nunca se presenta como correcto un XLSX inválido. Hallazgo FUN-05. | RV21 |
| 5 | RV20 — Archivos reales que cargan | Un CSV CR-only carga con las mismas filas que su equivalente LF; un CSV en Windows-1252 muestra una propuesta de lectura con vista previa antes/después y se abre tras un clic, sin conversión automática; ningún error de carga muestra texto del motor en inglés. Hallazgos FUN-01 y FUN-02. | RV21 |
| 6 | RV07 — Beta | Tres participantes distintos sobre el mismo release candidate, dos casos reales por sesión y al menos tres datasets; 24 de 30 tareas sin ayuda; guardar, reabrir y entrega verificados; sin P0/P1; resumen sanitizado validado con `npm run beta:check-summary`. Cada fallo dependiente de datos se reduce a una fixture sintética con su regresión antes de corregirlo. Guía: [sesión beta](docs/how-to/run-beta-validation.md) e [invitación](docs/templates/beta-invitation.md). | Participantes y datos de trabajo; RV17–RV20 |
| 7 | RV22 — Menores de la re-auditoría | Aviso de cierre inesperado con la hora del último guardado (DAT-01); informe de fallo con ruta desde el crate (CODE-01); los diez detalles de UX-01; comando manual de tests Rust documentado con `COLUMNIA_TEST_HARNESS_MANIFEST` (QA-01). | Ninguna |
| 8 | RV09 — Accesibilidad nativa | Cargar → Entregar en Windows solo con teclado y NVDA, incluidos diálogos, tablas, progreso, zoom y alto contraste. Guía: [probar con NVDA](docs/how-to/run-nvda-check.md). | El mismo candidato que RV07 |
| 9 | RV11 — Distribución | Solo si se reparten binarios: aprobación jurídica, prueba en VM limpia (instalación, reapertura, updater, fallos y recuperación), hashes y firmas de los artefactos descargados, y la sección `## [versión]` del CHANGELOG al cortar la versión. | Decisión de distribución |

La publicación vigente autoriza **código fuente únicamente**. No autoriza
instaladores, updater público ni comercialización. ONAPI sigue siendo requisito
previo a comercializar.

## Fuera de la cola

Solo entran si la beta aporta casos repetidos, una persona responsable y un
criterio de aceptación: lotes gráficos, validar en una herramienta BI concreta,
diccionario de negocio, catálogos de equivalencias, reanudación avanzada de
lotes, vigilancia de carpetas, macOS/Linux y conectores nuevos.

Modularizar `dataset.rs` no es un objetivo: se extrae una responsabilidad cuando
se toca esa área, sin cambiar contratos.

## Criterio de salida de V1

V1 puede declararse soportada cuando RV07 y RV09 estén cerradas sobre un
candidato identificable; no queden defectos P0/P1; pasen la suite completa y los
gates de privacidad, seguridad y rendimiento; y, si se distribuyen binarios,
RV11 tenga autorización y evidencia reproducible.

## Cómo mantener este documento

- Una fila por pendiente, con su criterio de cierre. Nada de registros de
  sesiones, cifras de pruebas ni tareas cerradas.
- Al cerrar algo, se quita de aquí y se describe en `CHANGELOG.md`.
- Una decisión duradera de arquitectura o de trabajo va en `CONTEXTO.md`.
