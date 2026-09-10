# Resumen sanitizado de beta de Columnia

> Copiar este archivo a `docs/reference/beta-v1-summary.md` al cerrar el Gate 1
> y actualizarlo con la comparación del Gate 2. No incluir nombres, rutas,
> columnas, filas, consultas, capturas, credenciales ni datos identificables.

## Release candidate

| Campo | Valor |
| --- | --- |
| Gate | Gate 1 · baseline V1 / Gate 2 · shell de espacios |
| Release candidate | rc-___ |
| Commit probado | ___ |
| Versión de Columnia | ___ |
| Fechas de ejecución | AAAA-MM-DD — AAAA-MM-DD |

## Muestra agregada

| Métrica | Resultado |
| --- | ---: |
| Sesiones válidas | ___ / 3 |
| Participantes distintos | ___ / 3 |
| Usos de fixture sintética | ___ / 3 |
| Casos reales ejecutados | ___ / 6 |
| Datasets reales distintos | ___ / mínimo 3 |
| Tareas observadas | ___ / 30 |

Confirma que todas las sesiones válidas usaron el mismo release candidate. Una
sesión interrumpida por preparación o entorno no se incluye. Si un P0/P1 exigió
otro commit, registra aquí la ronda anterior como invalidada y agrega únicamente
las tres sesiones válidas del nuevo release candidate.

## Resultado de tareas

| Resultado | Cantidad | Porcentaje |
| --- | ---: | ---: |
| Sin ayuda | ___ | ___ % |
| Con ayuda | ___ | ___ % |
| No completadas | ___ | ___ % |
| Total | ___ / 30 | 100 % |

Todas las personas completaron Cargar → Revisar → Preparar → Entregar: sí / no.

## Fricción agregada

| Métrica | Gate 1 baseline | Gate 2 shell | Cambio | Reducción |
| --- | ---: | ---: | ---: | ---: |
| Acciones de navegación | ___ | ___ | ___ | ___ % |
| Datos reintroducidos | ___ | ___ | ___ | ___ % |
| Retrocesos | ___ | ___ | ___ | ___ % |
| Eventos de ayuda | ___ | ___ | ___ | ___ % |
| Momentos de duda | ___ | ___ | ___ | ___ % |

Calcula la reducción como `(baseline - Gate 2) / baseline × 100`. Si el baseline
es cero, informa `N/A`: la métrica ya estaba en su mínimo y no puede usarse para
afirmar una reducción. La consolidación de Analizar exige al menos 20 % menos
acciones de navegación o datos reintroducidos, sin empeorar los gates de V1.

## Persistencia y entrega

| Comprobación | Resultado |
| --- | --- |
| Guardado, cierre y reapertura | ___ / 3 aprobadas |
| Entrega verificada fuera de Columnia | ___ / 3 aprobadas |
| Original sin cambios | ___ / 3 confirmado |

## Hallazgos y decisiones

| Severidad | Encontrados | Abiertos | Decisión o referencia sanitizada |
| --- | ---: | ---: | --- |
| P0 | ___ | ___ | ___ |
| P1 | ___ | ___ | ___ |
| P2 | ___ | ___ | ___ |
| P3 | ___ | ___ | ___ |

Los P2/P3 aceptados tienen responsable: sí / no.

## Veredicto

- Gate 1 V1: aprobado / pendiente.
- Gate 2 shell: aprobado / pendiente / todavía no ejecutado.
- 80 % sin ayuda: aprobado / fallido.
- Cero P0/P1 abiertos: aprobado / fallido.
- Mejora de navegación o reentrada ≥20 %: aprobado / fallido / todavía no aplica.
- Decisión final y próxima acción:
