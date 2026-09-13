# Trabajo vigente

Revisado: 2026-09-13. Este índice enumera aceptación y evidencia aún pendientes;
no implica que las capacidades locales correspondientes no estén implementadas.
El desarrollo por etapas y el historial están en [`ROADMAP.md`](../../ROADMAP.md).

| ID | Qué falta para cerrar | Responsable | Dependencia | Detalle histórico |
| --- | --- | --- | --- | --- |
| A01 | Tres sesiones de beta sobre el mismo candidato; datos y tareas conforme al Gate 1. | Equipo de producto (asignación nominal pendiente). | Release candidate con gates Full y participantes con datos reales. | [Tier 8](../../ROADMAP.md#tier-8--beta-local-con-datos-de-trabajo-abierto-2026-09-09) |
| A02 | Convertir los fallos observados en fixtures sintéticas y regresiones. | Mantenimiento (asignación nominal pendiente). | A01. | [Tier 8](../../ROADMAP.md#tier-8--beta-local-con-datos-de-trabajo-abierto-2026-09-09) |
| F01 / T6-05 | Completar round-trip `true`/`false`/`null` en SQL Server por frame y fuente incremental. | Responsable de QA ODBC (asignación nominal pendiente). | Instancia y controlador ODBC SQL Server accesibles. | [T6-05](../../ROADMAP.md#tier-6--integridad-de-escritorio-y-entrega-odbc) |
| I01 | Aceptación manual Windows con teclado y lector de pantalla real. | QA de accesibilidad (asignación nominal pendiente). | Release candidate vigente y tecnología asistiva disponible. | [Checklist de accesibilidad](../../ACCESSIBILITY_MANUAL_CHECKLIST.md) |
| I03 | Registrar una decisión específica para distribución binaria y updater. | Responsable de distribución (asignación nominal pendiente). | Revisión de distribución y gate correspondiente. | [Decisión de distribución](legal-distribution-decision.json) |
| I04 | Instalar, abrir, actualizar y recuperar el artefacto candidato, con evidencia ligada a sus hashes. | Release engineering (asignación nominal pendiente). | I03 y artefacto exacto aprobado. | [Evidencia de release](release-evidence.md) |

La aceptación PostgreSQL y MariaDB de T6-05 ya está documentada; no hay una
corrección de PostgreSQL pendiente. El bloqueo actual es el round-trip real de
SQL Server, que requiere el servicio y controlador externos.
