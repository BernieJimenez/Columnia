# Trabajo vigente

Revisado: 2026-09-15. Esta es la única cola operativa derivada de la
[`auditoría consolidada`](../../AUDITORIA.md). El historial y las tareas cerradas
permanecen en [`ROADMAP.md`](../../ROADMAP.md#tier-9--valor-operativo-consolidado-abierto-2026-09-14).

Una tarea entra aquí solo si reduce fricción del flujo principal, permite repetir
trabajo, protege resultados o aporta evidencia necesaria para declarar soporte.
Las propuestas condicionadas a demanda se enumeran al final y no son trabajo
comprometido.

## Ahora — completar el flujo automático

| ID | Resultado y criterio de cierre | Responsable | Dependencia | Estado / evidencia |
| --- | --- | --- | --- | --- |
| RV01 | **Flujo contextual y estado central.** Una sola acción primaria por estado; visitar una fase no la completa; carga, perfil, plan, aplicación, validación y entrega rechazan resultados obsoletos o dobles ejecuciones. | Producto + frontend | Ninguna | **Parcial — coordinación local reforzada.** `usePrepareController` y App rechazan clics repetidos y respuestas obsoletas: selección/inspección, comparación, paginación de conflictos, consolidación, unión y exportación quedan ligados a secuencia y revisión del dataset; el foco conserva la etapa activa. Falta validar cancelación uniforme en una sesión nativa completa. |
| RV02 | **Importación unificada y explicable.** Hoja, encabezados, esquema, ambigüedades y recursos se aceptan una vez; CSV permite revisar decisiones de encabezado antes de activar el dataset. | Producto + motor de importación | RV01 | **Parcial.** CSV conserva la revisión explícita de encabezados y el E2E cargado recorre Cargar→Revisar; source-backed sin encabezados conserva nombres generados. Falta cubrir en una sesión integrada todas las fuentes y decisiones. |
| RV04 | **Excepciones, cancelación y recuperación.** Fechas, tipos, conflictos y esquema cambiado ofrecen conservar, resolver o excluir; cancelar nunca presenta una versión parcial como terminada y los fallos de disco conservan la última revisión válida. | Motor + frontend | RV01–RV03 | **Parcial.** Las operaciones principales rechazan resultados obsoletos, las exportaciones son atómicas y el exportador CSV source-backed limita sus hilos para conservar el contrato de memoria. Preparar aún necesita cancelación uniforme y decisiones reutilizables para todas las excepciones. |
| RV05 | **Tarea reutilizable.** Guardar importación, receta, reglas y política de salida sin credenciales ni permiso implícito de sobrescritura; otro archivo compatible recorre el flujo sin reconstruir formularios y un esquema distinto exige revisión. | Proyectos + automatización | RV02–RV04 | **Parcial — integración local cubierta.** La tarea puede prepararse antes de elegir archivo, su perfil gobierna la importación compatible y el esquema cambiado exige confirmación; dos pruebas de App cubren ambos recorridos sin ejecutar la receta automáticamente. Falta aceptación con archivos y sesiones reales. |
| RV06 | **Interfaz compacta y accesible.** Retirar jerga y bloques repetidos; mantener historial/deshacer cerca del resultado; foco estable, teclado, errores asociados y zoom 200 % verificados en el flujo automático. | Diseño + accesibilidad | RV01–RV05 | **Parcial — E2E sintético cubierto.** El foco pasa al contenedor etiquetado de la nueva etapa, los errores ODBC asocian campo y descripción, y Cargar→Entregar conserva layout a 200 % y 320 CSS px; 3/3 E2E pasan. La aceptación nativa con lector de pantalla sigue pendiente. |

## Después — demostrar valor y soporte

| ID | Resultado y criterio de cierre | Responsable | Dependencia | Estado |
| --- | --- | --- | --- | --- |
| RV07 | **Beta con tareas reales.** Tres participantes distintos completan dos casos reales por sesión sobre el mismo candidato, con al menos tres datasets en total; consiguen 24/30 tareas sin ayuda, completan el flujo, guardan/reabren y verifican la entrega en cada sesión, sin P0/P1 abierto; se publica un resumen sanitizado y los reportes detallados quedan bajo `.local/beta/`. | Producto | Candidato con RV01–RV06 y gate Full | **Abierto — requiere participantes y datos de trabajo.** |
| RV08 | **Regresiones derivadas de beta.** Cada fallo dependiente de datos se reduce a una fixture sintética, se reproduce antes de corregirse y obtiene una regresión pertinente. | Mantenimiento | RV07 | **Abierto — depende de hallazgos de RV07.** |
| RV09 | **Aceptación nativa de accesibilidad.** Recorrido Cargar→Entregar en Windows con teclado y lector de pantalla real, incluidos modales, tablas, progreso, zoom y alto contraste. | QA accesibilidad | Mismo candidato de RV07 | **Abierto — requiere verificación nativa.** |
| RV10 | **Aceptación SQL Server.** Exportar y releer `true`/`false`/`null` desde frame y fuente incremental conserva tipos y valores; registrar driver y configuración sin credenciales. | QA ODBC | Instancia SQL Server accesible | **Abierto — falta instancia y driver accesibles.** |
| RV11 | **Candidato instalable y distribución.** Resolver notices y decisión por canal; probar en VM limpia instalación, reapertura, fallos/firmas del updater y recuperación sobre artefactos ligados al commit; volver a descargar y verificar hashes y firmas publicados. | Release + responsable de distribución | RV07, RV09 y decisiones externas | **Parcial — fuente únicamente aprobada.** GitHub permite publicar código fuente y la revisión técnica de notices está aprobada. Eso no autoriza instaladores/updater ni comercialización; faltan candidato/canal binario autorizado, VM limpia y verificación de los assets descargados. ONAPI sigue siendo requisito previo a comercializar. |

## Siguiente valor — priorizar con evidencia de beta

| ID | Resultado y criterio de cierre | Responsable | Dependencia | Estado / evidencia |
| --- | --- | --- | --- | --- |
| RV12 | **Recursos y escala medibles.** Una matriz varía ancho, cardinalidad, texto y tamaño; mide RAM, disco, tiempo, cancelación y limpieza. Los avisos solo aparecen cuando cambian una decisión y no confunden estimación con reserva. | Rendimiento | Evidencia de RV07 | **Completada para la matriz sintética v1.** `perf:matrix:summary` valida los ocho cruces (1 y 100 MiB × standard, wide, low-cardinality y long-text), cada uno con 3 transformaciones, 2 actualizaciones de proyecto, RAM/disco, cancelación source-backed, salida previa intacta y limpieza confirmada. El exportador CSV usa un hilo para mantener el orden con el límite de memoria; las mediciones cubren el motor nativo y excluyen UI/IPC. Las formas observadas en beta pueden añadirse como nuevas corridas. |
| RV14 | **Preflight y presets de entrega.** Tipos, nulabilidad, longitud y política remota se explican antes de escribir; los presets locales se releen y se verifican en la herramienta BI elegida sin introducir conectores nuevos. | Entrega | Recorridos confirmados en RV07 | **Implementación local lista; aceptación externa pendiente.** El preflight se repite en backend y bloquea antes de DDL enteros fuera de `i64` y decimales no representables como `f64` finitos; el binding devuelve error en vez de fabricar `NULL`. Los nulos reales conservan binding tipado, los presets no guardan credenciales y `replace` se rebaja a `create_only` al reabrir. Falta elegir herramienta BI de beta y comprobar allí formatos y presets. |
| RV15 | **Lotes gráficos.** Solo si RV07 confirma repetición frecuente: reutilizar el contrato batch con preflight conjunto, progreso por trabajo, resultados parciales honestos y ninguna sustitución implícita. | Automatización | RV05 y demanda observada | **Condicional — no iniciar hasta observar demanda frecuente en beta.** |
| RV16 | **Modularización gradual del motor.** Extraer una responsabilidad de `dataset.rs` por cambio con contratos estables, paridad conductual y sin reescritura general. | Mantenimiento | Al tocar el área por RV02–RV05 o RV12 | **Oportunidad gradual — no justifica una reescritura aparte.** |

## Fuera de la cola vigente

Diccionario de negocio, catálogos de equivalencias, reanudación avanzada de
lotes, vigilancia de carpetas, macOS/Linux y nuevos conectores solo se evaluarán
si la beta aporta casos repetidos, responsables y criterios de aceptación. No se
añaden por anticipación.
