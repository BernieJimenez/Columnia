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

La revisión del 2026-10-01 quedó aplicada el 2026-10-08: 92 de sus 97 objetivos
están cerrados. Quedan seis hallazgos que necesitan medir con archivos muy
grandes o un servidor que este equipo no tiene. Ninguno bloquea el uso diario.

### Rendimiento

- [ ] **REN-02** — La memoria del proceso crece al recargar el mismo dataset
  (RV49).
  - Hecho: mimalloc con purga inmediata. 20 recargas de un CSV de 144 MB pasan
    de 380→922 MiB a 476→563 MiB a la misma velocidad.
  - Falta: tras 20 recargas, no superar en más del 10 % la memoria del ciclo 2
    (hoy +12,5 %, unos 3 MiB por recarga) y medirlo en la app, no solo en la
    sonda `perf_probe_reload_memory`.
- [ ] **REN-01** — El perfil de calidad de archivos grandes es lento y se repite
  tras cada cambio (RV48).
  - Hecho: vmCloud de 11,9 s a 4,8 s y etapas visibles.
  - Falta: llegar a ≤ 3 s, medir 2019-Oct (5,3 GiB) y reutilizar el perfil tras
    cada cambio en lugar de recalcularlo.
- [ ] **REN-08** — Coste de la comparación por clave con millones de filas
  (RV50).
  - Falta: medirlo en la app con archivos de millones de filas y guardar el
    índice de conflictos para paginar sin recalcular.
- [ ] **REN-13** — Disco temporal del perfil numérico de archivos grandes (RV65).
  - Falta: medir con un CSV de unos 100 M filas antes de cambiar una sola pasada
    por una lectura por columna o por lotes (menos disco, más tiempo).

### Entrega a bases de datos

- [ ] **DAT-12** — En MySQL, «Crear» no es atómico (RV44).
  - Hecho: si la entrega falla, se borra la tabla que creó.
  - Falta: la prueba con un servidor y un controlador MySQL reales.

## Aparcado: solo si Columnia se comparte

Estos objetivos dejaron la cola el 2026-10-01 porque la aplicación es de uso
personal. Se retoman únicamente si se decide dársela a otras personas.

| ID | Criterio de cierre | Se retoma si |
| --- | --- | --- |
| RV07 — Beta | Tres participantes distintos sobre el mismo release candidate, dos casos reales por sesión y al menos tres datasets; 24 de 30 tareas sin ayuda; guardar, reabrir y entrega verificados; sin P0/P1; resumen sanitizado validado con `npm run beta:check-summary`. Cada fallo dependiente de datos se reduce a una fixture sintética con su regresión antes de corregirlo. Guía: [sesión beta](docs/how-to/run-beta-validation.md) e [invitación](docs/templates/beta-invitation.md). | Otras personas van a usarla |
| RV09 — Accesibilidad nativa | Cargar → Entregar en Windows solo con teclado y NVDA, incluidos diálogos, tablas, progreso, zoom y alto contraste. Guía: [probar con NVDA](docs/how-to/run-nvda-check.md). | La usa alguien con lector de pantalla |
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
