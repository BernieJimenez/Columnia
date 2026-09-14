# Auditoría consolidada de Columnia

Revisada: 2026-09-14. Este documento sustituye las auditorías fechadas de
producto, diseño, ingeniería, usabilidad y automatización. Conserva las
conclusiones que explican decisiones ya tomadas; el trabajo pendiente vive en
[`docs/reference/roadmap-current.md`](docs/reference/roadmap-current.md) y su
detalle histórico en [`ROADMAP.md`](ROADMAP.md).

## Dictamen vigente

Columnia ya dispone de un motor local amplio para importar, perfilar, preparar,
validar y entregar datos. El mayor valor pendiente no proviene de añadir más
herramientas aisladas. Proviene de reducir decisiones repetidas, coordinar el
flujo completo y comprobarlo con archivos y personas reales.

La dirección aprobada es:

1. una acción principal contextual y estados ligados a resultados reales;
2. una importación explicable seguida de un plan de preparación ejecutable;
3. validación, excepciones, cancelación y recuperación coordinadas;
4. tareas reutilizables para repetir un trabajo compatible;
5. accesibilidad y beta nativas antes de ampliar el alcance del producto.

No se identificó un defecto crítico abierto en las revisiones consolidadas. La
ausencia de un hallazgo crítico no certifica seguridad, rendimiento, usabilidad
o compatibilidad con todos los entornos.

## Capacidades comprobadas que se conservan

- Importación local de delimitados, JSON, Parquet y libros, con perfilado y
  preflight de recursos.
- Preparación reversible, recetas, historial, deshacer/rehacer, SQL local,
  comparación, consolidación, conflictos y JOIN.
- Contratos de calidad, privacidad visible, exportaciones locales, bundle
  auditable y entrega ODBC con límites explícitos.
- Proyectos durables, recuperación, preferencias, CLI y ejecución batch.
- Procesamiento incremental, guardias de RAM, cancelación cooperativa y gates
  locales de pruebas, documentación, red, secretos, IPC y distribución.
- Un sistema visual común, temas, navegación por fases y controles accesibles
  verificados automáticamente en navegador.

Estas capacidades no deben reconstruirse como funciones nuevas. Las mejoras del
roadmap deben integrarlas y simplificar su uso.

## Registro histórico consolidado

| Fecha | Revisión | Resultado que se conserva |
| --- | --- | --- |
| 2026-08-28 | Auditoría profesional integral | Abrió Tier 5 para robustecer gates, evidencia de release, arquitectura y distribución. Las tareas técnicas se implementaron; notices, descubribilidad y aprobación de distribución siguen dentro del gate de release. |
| 2026-09-05 | Reauditoría de escritorio | Abrió Tier 6. Se corrigieron integridad de JOIN/publicación, dialectos y seguridad ODBC, resultados obsoletos, memoria, accesibilidad y cobertura. Queda la aceptación real de booleanos en SQL Server. |
| 2026-09-06 | Revisión visual | Aplicó navegación petróleo, superficies neutras, jerarquía común, temas y corrección de Preferencias. La aceptación nativa con lector de pantalla continúa pendiente. |
| 2026-09-07 | Reauditoría incremental | Cerró Tier 7: presupuesto CSS, inventario IPC, falso positivo del gate de red y evidencia de dependencias. |
| 2026-09-13 | Auditoría general | Confirmó que beta, repetición de tareas, explicabilidad y robustez aportan más valor que ampliar funciones. Corrigió la entrada documental y verificó suites frontend/nativas. |
| 2026-09-13–14 | Usabilidad y automatización | Simplificó inicio, carga automática, herramientas contextuales, plan acotado, validación integrada, reglas resumidas y resultado de exportación accionable. El resto se fusionó en Tier 9. |

La evidencia detallada permanece en los commits, `CHANGELOG.md`, `CONTEXTO.md`,
las carpetas locales de validación y las tareas históricas de `ROADMAP.md`.

## Mejoras aplicadas desde la última revisión

- El diagnóstico comienza tras cargar y descarta resultados de revisiones
  anteriores.
- El inicio prioriza abrir un archivo o continuar un proyecto; la administración
  queda en contexto.
- Preparar muestra primero señales presentes y un plan acotado ligado a la
  revisión para recorte, encabezados y marcadores de ausencia.
- Entregar valida el contrato y exporta desde una sola acción; las reglas se
  resumen en lenguaje de datos y el editor técnico queda a demanda.
- Una publicación exitosa muestra destino, formato, tamaño, calidad, cambios y
  privacidad. Error y cancelación conservan el dataset preparado.

## Decisiones de alcance

Se mantienen fuera de la cola vigente hasta que la beta demuestre demanda:

- diccionario de negocio durable;
- catálogos de equivalencias y coincidencia difusa;
- vigilancia o programación de carpetas;
- reanudación sofisticada de lotes;
- soporte anunciado para macOS o Linux;
- nuevos conectores, nube, cuentas, colaboración o telemetría.

También se descarta añadir un chat, más paneles o más botones como solución a la
automatización. Las reglas deterministas existentes deben coordinarse mediante
estados y contratos explícitos.

## Trazabilidad hacia el roadmap

| Hallazgos anteriores | Tarea consolidada |
| --- | --- |
| UX01, UX02, UX06, CO02, CO03, CO05 | RV01 — Flujo contextual y estado central |
| UX05, C01 | RV02 — Importación unificada y explicable |
| AU01, AU02, AU03, AU06, D01 | RV03 — Plan completo y resultado antes/después |
| AU04, AU05, D04, H03 | RV04 — Excepciones, cancelación y recuperación |
| OUT03, REP01, REP02, E03 | RV05 — Tarea reutilizable de preparación y entrega |
| CO04, CO06 | RV06 — Accesibilidad y controles compactos |
| A01, A03, A04, CO07 | RV07 — Beta y medición con tareas reales |
| A02 | RV08 — Regresiones sintéticas derivadas de beta |
| I01 | RV09 — Aceptación nativa de accesibilidad |
| F01, T6-05 | RV10 — Aceptación SQL Server |
| I03, I04, T5-18, T5-20, I5–I7 | RV11 — Candidato instalable y distribución |
| AU07, H04 | RV12 — Recursos y escala medibles |
| G01, REP04 | RV13 — Respaldo y autoguardado recuperables |
| F02, F03 | RV14 — Preflight y presets de entrega |
| G03, REP03 | RV15 — Lotes gráficos bajo demanda validada |
| H01 | RV16 — Modularización gradual del motor |

Los identificadores antiguos sirven solo para rastrear decisiones y commits. No
son una segunda lista de trabajo.
