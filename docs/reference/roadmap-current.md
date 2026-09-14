# Trabajo vigente

Revisado: 2026-09-14. Esta es la única cola operativa derivada de la
[`auditoría consolidada`](../../AUDITORIA.md). El historial y las tareas cerradas
permanecen en [`ROADMAP.md`](../../ROADMAP.md#tier-9--valor-operativo-consolidado-abierto-2026-09-14).

Una tarea entra aquí solo si reduce fricción del flujo principal, permite repetir
trabajo, protege resultados o aporta evidencia necesaria para declarar soporte.
Las propuestas condicionadas a demanda se enumeran al final y no son trabajo
comprometido.

## Ahora — completar el flujo automático

| ID | Resultado y criterio de cierre | Responsable | Dependencia | Estado / evidencia |
| --- | --- | --- | --- | --- |
| RV01 | **Flujo contextual y estado central.** Una sola acción primaria por estado; visitar una fase no la completa; carga, perfil, plan, aplicación, validación y entrega rechazan resultados obsoletos o dobles ejecuciones. | Producto + frontend | Ninguna | **Parcial.** La revisión se marca al avanzar desde su acción contextual y la finalización de carga bloquea cambios de etapa. Preparar y Entregar aún no reflejan operaciones exitosas en su estado visual; falta consolidar el control de operaciones. |
| RV02 | **Importación unificada y explicable.** Hoja, encabezados, esquema, ambigüedades y recursos se aceptan una vez; CSV permite revisar decisiones de encabezado antes de activar el dataset. | Producto + motor de importación | RV01 | **Parcial.** Hay revisión de encabezados CSV y la ruta source-backed sin encabezados conserva los nombres generados en consultas y exportación DuckDB. Falta probar el ciclo completo para todas las fuentes y decisiones en una sesión integrada. |
| RV04 | **Excepciones, cancelación y recuperación.** Fechas, tipos, conflictos y esquema cambiado ofrecen conservar, resolver o excluir; cancelar nunca presenta una versión parcial como terminada y los fallos de disco conservan la última revisión válida. | Motor + frontend | RV01–RV03 | **Parcial.** Las operaciones principales rechazan resultados obsoletos y la exportación publica archivos atómicamente. Preparar aún necesita cancelación uniforme y decisiones reutilizables para conservar, resolver o excluir excepciones. |
| RV05 | **Tarea reutilizable.** Guardar importación, receta, reglas y política de salida sin credenciales ni permiso implícito de sobrescritura; otro archivo compatible recorre el flujo sin reconstruir formularios y un esquema distinto exige revisión. | Proyectos + automatización | RV02–RV04 | **Parcial.** Catálogo local, bridge, panel plegable y revisión de compatibilidad de esquema ya tienen pruebas. Falta una aceptación integrada de reuso con otro archivo y esquema distinto. |
| RV06 | **Interfaz compacta y accesible.** Retirar jerga y bloques repetidos; mantener historial/deshacer cerca del resultado; foco estable, teclado, errores asociados y zoom 200 % verificados en el flujo automático. | Diseño + accesibilidad | RV01–RV05 | **Parcial.** Diagnóstico agrupado, acción primaria contextual y herramientas avanzadas plegables reducen controles repetidos. La verificación nativa de teclado, foco, zoom 200 % y lector de pantalla sigue pendiente. |

## Después — demostrar valor y soporte

| ID | Resultado y criterio de cierre | Responsable | Dependencia | Estado |
| --- | --- | --- | --- | --- |
| RV07 | **Beta con tareas reales.** Tres participantes y un mismo candidato completan dos casos reales por sesión con al menos tres datasets; 24/30 tareas sin ayuda, guardado/reapertura y entrega externa; se publica un resumen sanitizado. | Producto | Candidato con RV01–RV06 y gate Full | **Abierto — requiere participantes y datos de trabajo.** |
| RV08 | **Regresiones derivadas de beta.** Cada fallo dependiente de datos se reduce a una fixture sintética, se reproduce antes de corregirse y obtiene una regresión pertinente. | Mantenimiento | RV07 | **Abierto — depende de hallazgos de RV07.** |
| RV09 | **Aceptación nativa de accesibilidad.** Recorrido Cargar→Entregar en Windows con teclado y lector de pantalla real, incluidos modales, tablas, progreso, zoom y alto contraste. | QA accesibilidad | Mismo candidato de RV07 | **Abierto — requiere verificación nativa.** |
| RV10 | **Aceptación SQL Server.** Exportar y releer `true`/`false`/`null` desde frame y fuente incremental conserva tipos y valores; registrar driver y configuración sin credenciales. | QA ODBC | Instancia SQL Server accesible | **Abierto — falta instancia y driver accesibles.** |
| RV11 | **Candidato instalable y distribución.** Resolver notices y decisión de distribución; verificar instalación, reapertura, actualización fallida, firma inválida y recuperación sobre artefactos ligados al commit y sus hashes. | Release + responsable de distribución | RV07, RV09 y decisiones externas | **Abierto — depende de decisiones de distribución y candidato instalable.** |

## Siguiente valor — priorizar con evidencia de beta

| ID | Resultado y criterio de cierre | Responsable | Dependencia | Estado / evidencia |
| --- | --- | --- | --- | --- |
| RV12 | **Recursos y escala medibles.** Una matriz varía ancho, cardinalidad, texto y tamaño; mide RAM, disco, tiempo, cancelación y limpieza. Los avisos solo aparecen cuando cambian una decisión y no confunden estimación con reserva. | Rendimiento | Evidencia de RV07 | **Parcial.** Los cuatro perfiles sintéticos aprobaron a 1 y 100 MiB, con RAM, disco, duración y limpieza registrados; el benchmark aún no mide cancelación. |
| RV13 | **Respaldo y autoguardado recuperables.** Paquete versionado, restauración transaccional y autoguardado opt-in con cuota y estados guardando/guardado/error; disco lleno conserva la última versión válida. | Proyectos | RV04–RV05 | **Abierto — sin implementación comprometida todavía.** |
| RV14 | **Preflight y presets de entrega.** Tipos, nulabilidad, longitud y política remota se explican antes de escribir; los presets locales se releen y se verifican en la herramienta BI elegida sin introducir conectores nuevos. | Entrega | Recorridos confirmados en RV07 | **Abierto — esperar destino confirmado en beta.** |
| RV15 | **Lotes gráficos.** Solo si RV07 confirma repetición frecuente: reutilizar el contrato batch con preflight conjunto, progreso por trabajo, resultados parciales honestos y ninguna sustitución implícita. | Automatización | RV05 y demanda observada | **Condicional — no iniciar hasta observar demanda frecuente en beta.** |
| RV16 | **Modularización gradual del motor.** Extraer una responsabilidad de `dataset.rs` por cambio con contratos estables, paridad conductual y sin reescritura general. | Mantenimiento | Al tocar el área por RV02–RV05 o RV12 | **Oportunidad gradual — no justifica una reescritura aparte.** |

## Fuera de la cola vigente

Diccionario de negocio, catálogos de equivalencias, reanudación avanzada de
lotes, vigilancia de carpetas, macOS/Linux y nuevos conectores solo se evaluarán
si la beta aporta casos repetidos, responsables y criterios de aceptación. No se
añaden por anticipación.
