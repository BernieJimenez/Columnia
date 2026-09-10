# Sesión beta de Columnia

> Copiar a `.local/beta/<fecha>/session.md`. No incluir datos personales, rutas,
> filas, capturas sensibles, consultas, credenciales ni nombres reales.

## Identificación segura

| Campo | Valor |
| --- | --- |
| Fecha | AAAA-MM-DD |
| Alias de sesión | beta-___ |
| Commit probado | ___ |
| Versión de Columnia | ___ |
| Ronda de medición | Gate 1 · baseline V1 / Gate 2 · shell de espacios |
| Release candidate | rc-___ |
| Windows / arquitectura | ___ |
| Experiencia con datos | inicial / intermedia / avanzada |
| Ayuda permitida | solo desbloqueo / ninguna |

## Datasets

| Caso | Formato | Tamaño aproximado | Filas aproximadas | Propósito |
| --- | --- | --- | --- | --- |
| Baseline sintética | CSV | <1 MiB | <100 | Comprobar el recorrido |
| Tabla real | ___ | <10 / 10–100 / >100 MiB | <10k / 10k–1M / >1M | ___ |
| Caso estructural | ___ | <10 / 10–100 / >100 MiB | <10k / 10k–1M / >1M | ___ |

## Resultado y fricción por tarea

Usar `sin ayuda`, `con ayuda` o `no completada`.

- **Acciones de navegación:** cambios de etapa, pestaña, panel o retorno necesarios
  para terminar la tarea; no contar escritura ni selección dentro de un formulario.
- **Datos reintroducidos:** cantidad de valores que la persona tuvo que escribir o
  seleccionar otra vez porque Columnia no conservó el contexto esperado.
- **Retrocesos:** veces que abandonó el camino elegido para buscar otra ruta.
- **Eventos de ayuda:** intervenciones de la persona facilitadora para desbloquearla.
- **Duda o causa:** frase sanitizada sobre el punto que produjo vacilación; usar
  `ninguna` cuando no ocurrió.

| Tarea | Resultado | Tiempo | Navegación | Datos reintroducidos | Retrocesos | Ayuda | Duda o causa | Observación sanitizada |
| --- | --- | --- | ---: | ---: | ---: | ---: | --- | --- |
| Cargar el dataset correcto | ___ | ___ | ___ | ___ | ___ | ___ | ___ | ___ |
| Comprender filas, columnas y señales | ___ | ___ | ___ | ___ | ___ | ___ | ___ | ___ |
| Revisar una señal útil | ___ | ___ | ___ | ___ | ___ | ___ | ___ | ___ | ___ |
| Distinguir muestra de cobertura completa | ___ | ___ | ___ | ___ | ___ | ___ | ___ | ___ |
| Aplicar, deshacer y rehacer un cambio | ___ | ___ | ___ | ___ | ___ | ___ | ___ | ___ |
| Guardar, cerrar y reabrir el proyecto | ___ | ___ | ___ | ___ | ___ | ___ | ___ | ___ |
| Crear y ejecutar una regla de calidad | ___ | ___ | ___ | ___ | ___ | ___ | ___ | ___ |
| Exportar y verificar la entrega | ___ | ___ | ___ | ___ | ___ | ___ | ___ | ___ |
| Confirmar que el original no cambió | ___ | ___ | ___ | ___ | ___ | ___ | ___ | ___ |
| Recuperarse de un problema | ___ | ___ | ___ | ___ | ___ | ___ | ___ | ___ | ___ |

## Hallazgos

Duplicar este bloque por hallazgo.

### BETA-___ — Título sin datos sensibles

- Severidad: P0 / P1 / P2 / P3
- Tarea:
- Expectativa:
- Resultado observado:
- Reproducción sintética o pasos sanitizados:
- Recuperación disponible:
- Decisión: corregir / aceptar / investigar
- Responsable:

## Respuestas finales

- Sorpresa principal:
- Momento de mayor duda:
- Evidencia que faltó para confiar en la entrega:
- Proceso o herramienta que Columnia reemplazaría:
- Única mejora prioritaria:

## Veredicto

- Tareas sin ayuda: ___ / 10
- Tareas con ayuda: ___ / 10
- Tareas no completadas: ___ / 10
- Acciones de navegación: ___
- Datos reintroducidos: ___
- Retrocesos: ___
- Eventos de ayuda: ___
- Momentos de duda: ___
- Guardado y reapertura: aprobado / fallido
- Entrega verificada fuera de Columnia: aprobada / fallida
- Hallazgos: P0 ___ · P1 ___ · P2 ___ · P3 ___
- Validez de la sesión: válida / no cuenta por preparación o entorno / invalidada por P0-P1
- Resultado de la sesión: aprobada / requiere correcciones / detenida
- Próxima acción:
