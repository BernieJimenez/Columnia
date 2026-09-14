# Trabajo vigente

Revisado: 2026-09-14. Esta es la única cola operativa derivada de la
[`auditoría consolidada`](../../AUDITORIA.md). El historial y las tareas cerradas
permanecen en [`ROADMAP.md`](../../ROADMAP.md#tier-9--valor-operativo-consolidado-abierto-2026-09-14).

Una tarea entra aquí solo si reduce fricción del flujo principal, permite repetir
trabajo, protege resultados o aporta evidencia necesaria para declarar soporte.
Las propuestas condicionadas a demanda se enumeran al final y no son trabajo
comprometido.

## Ahora — completar el flujo automático

| ID | Resultado y criterio de cierre | Responsable | Dependencia |
| --- | --- | --- | --- |
| RV01 | **Flujo contextual y estado central.** Una sola acción primaria por estado; visitar una fase no la completa; carga, perfil, plan, aplicación, validación y entrega rechazan resultados obsoletos o dobles ejecuciones. | Producto + frontend | Ninguna |
| RV02 | **Importación unificada y explicable.** Hoja, encabezados, esquema, ambigüedades y recursos se aceptan una vez; CSV permite revisar decisiones de encabezado antes de activar el dataset. | Producto + motor de importación | RV01 |
| RV03 | **Plan completo y resultado antes/después.** El plan ligado a la revisión reúne operaciones compatibles, motivos e impacto; se publica como una unidad reversible cuando el motor lo permite y recalcula calidad una sola vez. | Motor de preparación + frontend | RV01–RV02 |
| RV04 | **Excepciones, cancelación y recuperación.** Fechas, tipos, conflictos y esquema cambiado ofrecen conservar, resolver o excluir; cancelar nunca presenta una versión parcial como terminada y los fallos de disco conservan la última revisión válida. | Motor + frontend | RV01–RV03 |
| RV05 | **Tarea reutilizable.** Guardar importación, receta, reglas y política de salida sin credenciales ni permiso implícito de sobrescritura; otro archivo compatible recorre el flujo sin reconstruir formularios y un esquema distinto exige revisión. | Proyectos + automatización | RV02–RV04 |
| RV06 | **Interfaz compacta y accesible.** Retirar jerga y bloques repetidos; mantener historial/deshacer cerca del resultado; foco estable, teclado, errores asociados y zoom 200 % verificados en el flujo automático. | Diseño + accesibilidad | RV01–RV05 |

## Después — demostrar valor y soporte

| ID | Resultado y criterio de cierre | Responsable | Dependencia |
| --- | --- | --- | --- |
| RV07 | **Beta con tareas reales.** Tres participantes y un mismo candidato completan dos casos reales por sesión con al menos tres datasets; 24/30 tareas sin ayuda, guardado/reapertura y entrega externa; se publica un resumen sanitizado. | Producto | Candidato con RV01–RV06 y gate Full |
| RV08 | **Regresiones derivadas de beta.** Cada fallo dependiente de datos se reduce a una fixture sintética, se reproduce antes de corregirse y obtiene una regresión pertinente. | Mantenimiento | RV07 |
| RV09 | **Aceptación nativa de accesibilidad.** Recorrido Cargar→Entregar en Windows con teclado y lector de pantalla real, incluidos modales, tablas, progreso, zoom y alto contraste. | QA accesibilidad | Mismo candidato de RV07 |
| RV10 | **Aceptación SQL Server.** Exportar y releer `true`/`false`/`null` desde frame y fuente incremental conserva tipos y valores; registrar driver y configuración sin credenciales. | QA ODBC | Instancia SQL Server accesible |
| RV11 | **Candidato instalable y distribución.** Resolver notices y decisión de distribución; verificar instalación, reapertura, actualización fallida, firma inválida y recuperación sobre artefactos ligados al commit y sus hashes. | Release + responsable de distribución | RV07, RV09 y decisiones externas |

## Siguiente valor — priorizar con evidencia de beta

| ID | Resultado y criterio de cierre | Responsable | Dependencia |
| --- | --- | --- | --- |
| RV12 | **Recursos y escala medibles.** Una matriz varía ancho, cardinalidad, texto y tamaño; mide RAM, disco, tiempo, cancelación y limpieza. Los avisos solo aparecen cuando cambian una decisión y no confunden estimación con reserva. | Rendimiento | Evidencia de RV07 |
| RV13 | **Respaldo y autoguardado recuperables.** Paquete versionado, restauración transaccional y autoguardado opt-in con cuota y estados guardando/guardado/error; disco lleno conserva la última versión válida. | Proyectos | RV04–RV05 |
| RV14 | **Preflight y presets de entrega.** Tipos, nulabilidad, longitud y política remota se explican antes de escribir; los presets locales se releen y se verifican en la herramienta BI elegida sin introducir conectores nuevos. | Entrega | Recorridos confirmados en RV07 |
| RV15 | **Lotes gráficos.** Solo si RV07 confirma repetición frecuente: reutilizar el contrato batch con preflight conjunto, progreso por trabajo, resultados parciales honestos y ninguna sustitución implícita. | Automatización | RV05 y demanda observada |
| RV16 | **Modularización gradual del motor.** Extraer una responsabilidad de `dataset.rs` por cambio con contratos estables, paridad conductual y sin reescritura general. | Mantenimiento | Al tocar el área por RV02–RV05 o RV12 |

## Fuera de la cola vigente

Diccionario de negocio, catálogos de equivalencias, reanudación avanzada de
lotes, vigilancia de carpetas, macOS/Linux y nuevos conectores solo se evaluarán
si la beta aporta casos repetidos, responsables y criterios de aceptación. No se
añaden por anticipación.
