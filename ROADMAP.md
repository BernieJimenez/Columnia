# Roadmap de Columnia

**Actualizado:** 2026-09-22

**Versión del repositorio:** 1.26.0

**Estado:** prototipo local funcional para Windows x64; beta con personas y datos
de trabajo, aceptación externa y distribución binaria todavía pendientes.

Este documento resume lo construido y lo que falta para declarar soporte. El
detalle de cada cambio pertenece a [`CHANGELOG.md`](CHANGELOG.md), la evidencia
de ejecución a [`docs/reference/roadmap-current.md`](docs/reference/roadmap-current.md)
y los hallazgos consolidados a [`AUDITORIA.md`](AUDITORIA.md). No se vuelve a
copiar aquí el historial de commits, corridas ni auditorías cerradas.

## Cómo leer el avance

- `[x]` significa **implementado y cerrado** con evidencia local.
- `[ ]` significa **todavía no cerrado**. Si existe implementación parcial, el
  texto indica exclusivamente qué falta para marcarlo.
- Una tarea no se marca como terminada solo porque el código exista: las que
  requieren datos reales, personas, servicios externos o aprobación conservan
  `[ ]` hasta completar esa aceptación.

### Checklist maestro

- [ ] **RV01 — Flujo contextual y estado central.** Falta aceptar el recorrido
  completo con tareas y datos de trabajo reales.
- [ ] **RV02 — Importación unificada y explicable.** Falta aceptar el preflight
  unificado con datasets reales.
- [x] **RV03 — Plan completo y resultado antes/después.** Implementado con una
  revisión reversible y comparación de calidad.
- [ ] **RV04 — Excepciones, cancelación y recuperación.** Falta aceptación con
  datos reales y documentar los límites de APIs nativas síncronas.
- [ ] **RV05 — Tarea reutilizable.** Falta aceptación con tareas y archivos de
  trabajo reales.
- [ ] **RV06 — Interfaz compacta y accesible.** Falta aceptación nativa con
  lector de pantalla.
- [ ] **RV07 — Beta con tareas reales.** Faltan tres sesiones humanas sobre el
  mismo candidato y cumplir el gate 24/30 sin ayuda.
- [ ] **RV08 — Regresiones derivadas de beta.** Depende de los hallazgos de
  RV07.
- [ ] **RV09 — Aceptación nativa de accesibilidad.** Falta el recorrido con
  teclado y lector de pantalla real.
- [ ] **RV10 — Aceptación SQL Server.** Falta una instancia accesible para el
  round-trip de booleanos y nulos.
- [ ] **RV11 — Candidato instalable y distribución.** Faltan aprobación
  jurídica, VM limpia y verificación de instalador, updater, hashes y firmas.
- [x] **RV12 — Recursos y escala medibles.** Cerrado para la matriz sintética v1
  y ampliado con el benchmark WebView2 de 100 MiB.
- [x] **RV13 — Respaldo y autoguardado recuperables.** Implementado con
  restauración transaccional y retención.
- [ ] **RV14 — Preflight y presets de entrega.** La implementación local está
  lista; falta validarla en la herramienta BI elegida.
- [ ] **RV15 — Lotes gráficos.** Condicionado a que la beta demuestre demanda
  frecuente; no es trabajo comprometido todavía.
- [ ] **RV16 — Modularización gradual del motor.** En curso; continuar solo al
  tocar cada área y sin cambiar contratos.

**Resumen:** 3 de 16 objetivos cerrados; 6 con implementación local parcial o
en curso; 6 pendientes de evidencia externa y 1 condicionado a la beta.

## Dirección del producto

Columnia es una aplicación de escritorio local-first para convertir archivos
desordenados en datasets confiables. La experiencia principal es:

1. **Cargar:** inspeccionar el origen y confirmar cómo interpretarlo.
2. **Revisar:** entender calidad, estructura, diferencias y relaciones.
3. **Preparar:** corregir y transformar con historial reversible.
4. **Entregar:** validar un contrato y exportar una copia segura.

Las decisiones base están cerradas: Tauri 2 y Rust para el shell y el motor;
React, TypeScript y Vite para la interfaz; Polars y DuckDB para procesamiento
local. No hay cuenta, telemetría ni sincronización remota. El archivo original
no se modifica.

## Implementado

### Flujo de datos

- Importación de CSV, TSV, TXT, JSON, JSONL/NDJSON, Parquet, XLS/XLSX/XLSB y
  ODS mediante selección nativa o arrastrar y soltar.
- Inspección previa sin reemplazar el dataset activo: tamaño, filas, columnas,
  tipos, recursos estimados, hoja de Excel, encabezados delimitados, muestra y
  diferencias contra el perfil guardado.
- Confirmación explícita antes de cargar y detección de cambios del archivo
  entre inspección y lectura final.
- Representación eager o source-backed según el origen y el tamaño, con
  paginación y pushdown de lectura para Parquet.
- Perfil de calidad con nulos, duplicados, tipos incompatibles, estadísticas,
  cardinalidad, correlaciones y tendencias temporales con suma o promedio.
- Comparación entre datasets, conflictos paginados, resolución por origen o
  celda, exclusión por clave, consolidación y JOIN.
- Consulta SQL local con Polars o DuckDB.

### Preparación reversible

- Recorte de texto, normalización de marcadores nulos y nombres de columnas,
  eliminación de duplicados exactos, conversión de tipos y fechas, reemplazos,
  filtros, orden, selección y renombrado de columnas.
- Plan combinado de correcciones con motivos, impacto y comparación de calidad
  antes/después.
- Recetas eager, lazy y source-backed con validación previa y publicación
  atómica.
- Políticas versionadas para valores incompatibles: revisar, convertir en nulo
  o excluir la fila; los nulos reales se conservan.
- Historial Deshacer/Rehacer, comparación de revisiones y snapshots con límites
  visibles de retención y disco.
- Cancelación cooperativa y rechazo de resultados obsoletos en las operaciones
  principales; un fallo o una cancelación no presenta un resultado parcial como
  terminado.

### Calidad, privacidad y entrega

- Reglas de calidad persistentes, migrables y evaluadas antes de entregar.
- Exportación local a CSV, JSON, Parquet, Excel, SQLite y Bundle; staging y
  reemplazo seguro conservan la salida previa si la operación falla.
- Neutralización de fórmulas en CSV/Bundle y enmascaramiento local de datos
  personales.
- Preflight ODBC con tipos, nulabilidad y valores incompatibles antes de DDL;
  políticas `create_only`, `append` y `replace` sin guardar credenciales.
- Presets de entrega locales que nunca restauran autorización implícita para
  sobrescribir.
- Manifiesto y comprobaciones para updater autenticado; todavía no se publica
  ningún canal binario.

### Proyectos y reutilización

- Proyectos locales con guardado, reapertura, borrado, recuperación y snapshots
  asociados a la revisión activa.
- Autoguardado opt-in y respaldos versionados con restauración transaccional,
  retención de hasta cinco versiones o 512 MiB y conservación de la última
  versión válida.
- Tareas reutilizables con perfil de importación, receta, reglas, decisiones de
  excepciones y salida. Un esquema compatible restaura la configuración como
  borrador; uno distinto exige revisión. No se guardan credenciales ni permiso
  de sobrescritura.
- Catálogos locales cancelables para proyectos, versiones, tareas y presets.

### Experiencia y accesibilidad

- Navegación por etapas cuyo estado «Hecho» está ligado a la revisión vigente;
  una mutación invalida diagnósticos o entregas anteriores cuando corresponde.
- Diálogos modales nativos del navegador, control de foco, operación por
  teclado, errores asociados a sus campos, anuncios de progreso acotados,
  colores forzados y diseño probado a zoom 200 %/320 CSS px.
- Historial compacto: resultado reciente y Deshacer/Rehacer visibles, lista de
  cambios plegada bajo demanda.
- Interfaz y documentación en español, galería de recorrido y guía para la beta.

### Arquitectura, seguridad y entrega técnica

- Frontera Tauri explícita, inventario IPC y validación de rutas, extensiones,
  enlaces simbólicos y reparse points.
- Política de red local-first, modelo de amenazas, escaneo de secretos,
  gobernanza del repositorio y verificaciones de supply chain y licencias.
- Gates automatizados de build, pruebas, cobertura, E2E, documentación,
  accesibilidad, privacidad de red, instalador, updater, rendimiento y release.
- Smokes nativos de Windows con diálogos reales y round-trip CSV, XLSX y
  Parquet; recorrido CDP para proyectos, recetas, exportación y reinicio.
- Matriz sintética de rendimiento para 1 y 100 MiB en cuatro perfiles, más un
  benchmark WebView2 de 100 MiB con carga, paginación, transformación,
  exportación y limpieza.
- Modularización gradual del motor: 26 módulos concentran responsabilidades
  antes alojadas en `dataset.rs`, incluyendo historial, importación, calidad,
  recetas, consultas y coordinadores de cancelación; se mantienen nombres de
  comandos, contratos JSON y rutas Tauri.

## Estado verificable

El corte local actual de 1.26.0 aprobó:

| Gate | Resultado registrado |
| --- | --- |
| Frontend | 457/457 pruebas en 51 archivos |
| Cobertura | 86,17 % de sentencias y 81,34 % de ramas; capas críticas aprobadas |
| E2E | 22/22 recorridos |
| Rust | 494 aprobadas y 5 ignoradas por benchmarks opt-in o integraciones ODBC externas |
| Rendimiento | 8/8 cruces de la matriz sintética y smoke WebView2 de 100 MiB |
| Release | dry-run técnico aprobado hasta el sign-off jurídico obligatorio |

Este corte repitió build, Vitest con cobertura, Playwright, Rust, formato,
inventario IPC y documentación. No sustituye los gates externos: aceptación con
datos reales, lector de pantalla, servidores ODBC/BI, beta humana, firma y
revisión legal continúan abiertos donde corresponde.

## Cola vigente

### Ahora — cerrar el flujo automático

| ID | Estado | Criterio pendiente |
| --- | --- | --- |
| RV01 | Parcial | Aceptar Cargar→Entregar con tareas y datos de trabajo reales. La coordinación, exclusión mutua y protección contra respuestas obsoletas ya están implementadas. |
| RV02 | Parcial | Validar con datasets reales la revisión única de hoja, encabezados, esquema, ambigüedades y recursos. El preflight unificado ya está implementado. |
| RV04 | Parcial | Ejercitar cancelación y recuperación con datos reales y documentar los tramos síncronos inevitables de Calamine, SQLite, ODBC y sistema de archivos. La publicación atómica y la cancelación de las rutas principales ya existen. |
| RV05 | Parcial | Confirmar tareas reutilizables con archivos de trabajo reales. Guardado, reinicio, compatibilidad de esquema y aplicación como borrador ya están cubiertos localmente. |
| RV06 | Parcial | Completar aceptación nativa con lector de pantalla. Teclado, foco, zoom, alto contraste y compactación ya tienen cobertura automatizada. |
| RV16 | En curso | Continuar extrayendo responsabilidades de `dataset.rs` solo al tocar cada área, sin reescritura general ni cambios de contrato. Los coordinadores de cancelación/publicación y el estado de generaciones ya viven en módulos dedicados. |

RV03 —plan completo de preparación—, RV12 —matriz de recursos y escala— y
RV13 —respaldo/autoguardado recuperable— están cerradas. No deben reabrirse sin
una regresión o nueva evidencia de beta.

### Después — demostrar soporte

| ID | Dependencia | Criterio de cierre |
| --- | --- | --- |
| RV07 — Beta real | Participantes y datos de trabajo | Tres personas sobre el mismo candidato; dos casos por sesión y al menos tres datasets; 24/30 tareas sin ayuda; guardado, reapertura y entrega comprobados; ningún P0/P1; resumen sanitizado publicado. |
| RV08 — Regresiones de beta | RV07 | Reducir cada fallo dependiente de datos a una fixture sintética y una prueba antes de corregirlo. |
| RV09 — Accesibilidad nativa | Mismo candidato de RV07 | Completar Cargar→Entregar en Windows con teclado y lector de pantalla real. |
| RV10 — SQL Server | Instancia accesible | Verificar round-trip de `true`/`false`/`null` desde frame y fuente incremental, sin registrar credenciales. |
| RV11 — Distribución | RV07, RV09 y decisiones externas | Obtener aprobación jurídica; probar instalación/reapertura y updater en VM limpia; verificar fallos, recuperación, hashes y firmas de los artefactos descargados. |
| RV14 — Aceptación BI | Herramienta elegida en beta | Confirmar formatos y presets en el destino BI. El preflight y los presets locales ya están implementados. |

La publicación vigente autoriza **código fuente únicamente**. No autoriza
instaladores, updater público ni comercialización. ONAPI continúa como requisito
previo a comercializar.

### Condicionado a evidencia

- **RV15 — Lotes gráficos:** iniciar solo si la beta demuestra repetición
  frecuente; reutilizar el contrato batch existente con preflight, progreso y
  resultados parciales honestos.
- Diccionario de negocio, catálogos de equivalencias, reanudación avanzada de
  lotes, vigilancia de carpetas, macOS/Linux y conectores nuevos quedan fuera de
  la cola hasta existir demanda repetida, responsable y criterio de aceptación.

## Orden recomendado

1. Repetir la suite completa sobre 1.26.0 y fijar un release candidate.
2. Cerrar RV01, RV02, RV04, RV05 y RV06 con recorridos de trabajo y aceptación
   nativa, sin añadir alcance nuevo.
3. Ejecutar RV07 y RV09 sobre el mismo candidato; convertir hallazgos en RV08.
4. Ejecutar RV10 cuando exista una instancia SQL Server accesible y RV14 en la
   herramienta BI elegida.
5. Resolver RV11 únicamente después de la evidencia de producto y las
   aprobaciones externas.
6. Mantener RV16 incremental y decidir RV15 solo con evidencia de uso.

## Hitos históricos compactados

| Hito | Resultado |
| --- | --- |
| Infraestructura I0–I8 | Contratos, prototipo Tauri, seguridad, calidad reproducible, supply chain, empaquetado, updater, release y documentación implementados como infraestructura local. |
| Tier 5 — Integridad de gates y distribución | Gates técnicos y legales instrumentados; la aceptación jurídica y la publicación binaria pasaron a RV11. |
| Tier 6 — Escritorio y ODBC | Flujos nativos y contrato ODBC implementados; la aceptación SQL Server real pasó a RV10. |
| Tier 7 — Regresiones de rediseño | Cerrado; build, pruebas y contratos recuperados tras la modularización de interfaz. |
| Tier 8 — Beta local | El protocolo y sus checkers están listos; las sesiones humanas se consolidaron en RV07. |
| Tier 9 — Valor operativo | Hito vigente: cerrar aceptación real, evidencia externa y distribución sin ampliar el producto por anticipación. |

## Criterio de salida de V1

V1 puede declararse soportada cuando RV01, RV02, RV04–RV11 y RV14 estén
cerradas sobre un candidato identificable; no queden defectos P0/P1; la suite
completa y los gates de privacidad, seguridad y rendimiento pasen; y el canal
de distribución elegido tenga autorización y evidencia reproducible. RV15 no
forma parte del criterio salvo que la beta lo active.
