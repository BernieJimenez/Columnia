# Roadmap de Columnia

**Actualizado:** 2026-09-23

**Versión del repositorio:** 1.26.0

**Candidato vigente:** `v1.26.0-rc.1` (`fb58b9a`).

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
  datos reales; los límites de APIs nativas síncronas están documentados en
  [`roadmap-current.md`](docs/reference/roadmap-current.md).
- [ ] **RV05 — Tarea reutilizable.** Falta aceptación con tareas y archivos de
  trabajo reales.
- [ ] **RV06 — Interfaz compacta y accesible.** Falta aceptación nativa con
  lector de pantalla. *Enriquecida el 2026-09-23:* el foco no vuelve al
  disparador al cerrar el diálogo de importación ([T10-10](#tier-10--reauditoría-2026-09-23-integridad-de-gates-frontera-ipc-y-accesibilidad-real-abierto-2026-09-23))
  y hay contrastes por debajo de WCAG AA en los temas oscuros y claro (T10-02).
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
  *Enriquecida el 2026-09-23:* T10-01 ya corrigió el subsistema de consola del
  ejecutable release.
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

- [ ] **Tier 10 — Reauditoría del 2026-09-23.** 20 de 33 tareas cerradas; 2 de
  severidad alta (T10-01, T10-02). Detalle en
  [Tier 10](#tier-10--reauditoría-2026-09-23-integridad-de-gates-frontera-ipc-y-accesibilidad-real-abierto-2026-09-23).

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
- Modularización gradual del motor: 38 módulos concentran responsabilidades
  antes alojadas en `dataset.rs`, incluyendo historial, importación, calidad,
  recetas, consultas, limpieza de columnas y coordinadores de cancelación; se
  mantienen nombres de comandos, contratos JSON y rutas Tauri.

## Estado verificable

El candidato `v1.26.0-rc.1` aprobó el 2026-09-23:

| Gate | Resultado registrado |
| --- | --- |
| Full (`check.ps1 -Profile Full`) | Aprobado sobre `fb58b9a`, incluido Clippy con `-D warnings` |
| Frontend | 457/457 pruebas en 51 archivos |
| Cobertura | 86,17 % de sentencias y 81,34 % de ramas; capas críticas aprobadas |
| E2E | 22/22 recorridos |
| Rust | 494 aprobadas y 5 ignoradas por benchmarks opt-in o integraciones ODBC externas |
| Nativos | Smokes CLI, desktop, reinicio WebView2, selectores Win32 y CDP debug/release aprobados |
| Rendimiento | Benchmark sostenido, WebView2 de 100 MiB y memoria release dentro de presupuesto |
| Release | dry-run técnico aprobado hasta el sign-off jurídico obligatorio |

El gate de rendimiento solo conserva abierto `frontend-bundle`, que exige un
reporte del perfil Package y por tanto el sign-off jurídico de RV11; el
presupuesto de bundle se verifica en el gate Full. No sustituye los gates externos: aceptación con
datos reales, lector de pantalla, servidores ODBC/BI, beta humana, firma y
revisión legal continúan abiertos donde corresponde.

## Cola vigente

### Ahora — cerrar el flujo automático

| ID | Estado | Criterio pendiente |
| --- | --- | --- |
| RV01 | Parcial | Aceptar Cargar→Entregar con tareas y datos de trabajo reales. La coordinación, exclusión mutua y protección contra respuestas obsoletas ya están implementadas. |
| RV02 | Parcial | Validar con datasets reales la revisión única de hoja, encabezados, esquema, ambigüedades y recursos. El preflight unificado ya está implementado. |
| RV04 | Parcial | Ejercitar cancelación y recuperación con datos reales; los tramos síncronos inevitables de Calamine, SQLite, ODBC y sistema de archivos están documentados en [`roadmap-current.md`](docs/reference/roadmap-current.md). La publicación atómica y la cancelación de las rutas principales ya existen. |
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

1. ~~Repetir la suite completa sobre 1.26.0 y fijar un release candidate.~~
   Hecho: `v1.26.0-rc.1`.
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

## Tier 10 — Reauditoría 2026-09-23: integridad de gates, frontera IPC y accesibilidad real (abierto 2026-09-23)

Tanda temática abierta por la reauditoría del 2026-09-23 (áreas de código,
seguridad, accesibilidad, UI/UX, arquitectura, QA, documentación y DevOps). El
informe consolidado está en [`AUDITORIA.md`](AUDITORIA.md) y el contexto de la
sesión en [`CONTEXTO.md`](CONTEXTO.md). Cada tarea declara su severidad real:
un Tier alto no rebaja la prioridad. T10-01 y T10-02 son las únicas de
severidad alta; T10-01 bloquea cualquier binario de RV11.

### Índice

| Severidad | Tareas | Esfuerzo bajo | Esfuerzo medio | Esfuerzo alto |
| --- | ---: | ---: | ---: | ---: |
| Alta | 2 | 2 | 0 | 0 |
| Media | 22 | 10 | 12 | 0 |
| Baja | 9 | 9 | 0 | 0 |
| **Total** | **33** | **21** | **12** | **0** |

Relación con tareas existentes: T10-02 y T10-10 enriquecen RV06; T10-01
bloquea RV11; T10-13 y T10-14 preparan RV10/RV14; T10-07 corrige un defecto
latente del gate de red que T7-03 (cerrada) no cubrió. No se detectaron
regresiones de tareas cerradas; sí afirmaciones documentadas que no se cumplen
(ver T10-07, T10-08, T10-09 y T10-10).

### Tareas

- [x] **[T10-01] Compilar el ejecutable release con subsistema gráfico**
  - **Área:** DevOps y configuración · **Severidad:** Alta
  - **Ubicación:** `src-tauri/src/main.rs:1-3`; `src-tauri/target/release/columnia.exe` (cabecera PE `Subsystem=3`, consola)
  - **Qué hacer:** añadir `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]` y una comprobación del campo `Subsystem` del PE en el contrato del instalador o en el smoke release.
  - **Criterio de aceptación:** el ejecutable release tiene `Subsystem=2`; abrirlo desde el Explorador no muestra consola; el gate falla si vuelve a 3.
  - **Esfuerzo:** bajo
  - **Depende de:** ninguna
  - **Cerrada:** 2026-09-23 — `columnia.exe` release recompilado con `Subsystem=2` (WINDOWS_GUI); `tools/check-pe-subsystem.mjs` en el perfil Release; 3/3 pruebas.
- [x] **[T10-02] Corregir los contrastes por debajo de WCAG AA**
  - **Área:** Accesibilidad · **Severidad:** Alta
  - **Ubicación:** `src/styles.css:270`, `:608`, `:757`, `:1347`, `:1372`; `src/workflow-styles.css:268`, `:338-339`
  - **Qué hacer:** introducir un token de texto sobre `--accent` por tema (botón «Aplicar plan seleccionado», día de nivel 4 del calendario temporal); aplicar el color oscuro de `.null-value` también al tema Sistema; subir `th small` a 4,5:1 en tema claro.
  - **Criterio de aceptación:** ≥ 4,5:1 medido en claro, oscuro y sistema oscuro para «Aplicar plan seleccionado» (hoy 1,68:1), valores ausentes (hoy 2,07:1) y etiquetas de tipo (hoy 3,74:1).
  - **Esfuerzo:** bajo
  - **Depende de:** ninguna
  - **Cerrada:** 2026-09-23 — medido en Chrome (Sistema con SO oscuro / Oscuro / Claro): «Aplicar plan seleccionado» 9,64/9,64/6,18; valores ausentes 6,67/8,47/6,19; etiquetas de tipo 7,35/7,35/5,64; captura `.local/auditoria-2026-09-23/capturas/14-T10-02-aplicar-plan-corregido.png`; vitest 458/458. El día de nivel 4 del calendario usa el mismo token y no se midió en pantalla.
- [x] **[T10-03] Enviar a DuckDB solo SQL reconstruido desde el plan validado**
  - **Área:** Seguridad · **Severidad:** Media
  - **Ubicación:** `src-tauri/src/dataset/local_query.rs:170-181`, `:842-891`
  - **Qué hacer:** generar `WHERE` y `GROUP BY` desde `LocalQueryPlan`, igual que la proyección, con identificadores y literales re-escapados; exigir que un literal sea un único token entre comillas simples; añadir pruebas de rechazo.
  - **Criterio de aceptación:** la sentencia que llega a DuckDB no contiene texto del usuario salvo identificadores y literales re-escapados; las pruebas nuevas rechazan literales que no son un único token; suite Rust verde.
  - **Esfuerzo:** medio
  - **Depende de:** ninguna
  - **Cerrada:** 2026-09-23 — `canonicalize_duckdb_query` genera `WHERE`/`GROUP BY` desde `LocalQueryPlan` y `parse_local_literal` exige un único token; 4 pruebas nuevas; suite Rust 498 aprobadas y 5 ignoradas; Clippy `-D warnings` y fmt verdes.
- [x] **[T10-04] Restringir el acceso externo de las conexiones DuckDB que ejecutan SQL del usuario**
  - **Área:** Seguridad · **Severidad:** Media
  - **Ubicación:** `src-tauri/src/duckdb_query.rs:1661-1662`, `:1706-1765`, `:1803-1817`
  - **Qué hacer:** verificar los valores por defecto del build `bundled`; tras registrar las vistas, limitar rutas permitidas a los archivos del dataset y al temporal, desactivar el acceso externo y la autocarga/autoinstalación de extensiones, y bloquear la configuración.
  - **Criterio de aceptación:** una prueba confirma que una consulta válida funciona y que leer un archivo fuera del conjunto permitido falla; la configuración queda bloqueada durante la consulta.
  - **Esfuerzo:** medio
  - **Depende de:** ninguna
  - **Cerrada:** 2026-09-23 — `restrict_external_access` (rutas permitidas, sin acceso externo ni extensiones, configuración bloqueada); la prueba cubre Parquet y CSV con rutas canónicas `\?\`, confirma `enable_external_access=false` y rechaza un archivo ajeno; falla sin la restricción. Suite Rust 499 aprobadas y 5 ignoradas; Clippy verde.
- [ ] **[T10-05] Exigir confirmación nativa antes de cualquier conexión ODBC**
  - **Área:** Seguridad · **Severidad:** Media
  - **Ubicación:** `src-tauri/src/dataset.rs:8633`, `:8786-8795`; `src-tauri/src/remote_databases.rs:121-167`
  - **Qué hacer:** mostrar desde Rust un diálogo nativo con controlador, servidor, base, tabla y política antes del preflight y de la entrega; retirar `test_database_connection` del handler si la interfaz no lo usa.
  - **Criterio de aceptación:** sin confirmación nativa no se abre ninguna conexión; inventario IPC actualizado; prueba de rechazo sin confirmación.
  - **Esfuerzo:** medio
  - **Depende de:** ninguna
- [x] **[T10-06] Advertir o bloquear entregas ODBC sin cifrado en tránsito**
  - **Área:** Seguridad · **Severidad:** Media
  - **Ubicación:** `src-tauri/src/remote_databases.rs:744-768`
  - **Qué hacer:** detectar `Encrypt`, `TrustServerCertificate` y `sslmode` en la cadena; mostrar en el preflight un aviso que exija confirmación si no se pide cifrado. El texto del aviso requiere revisión legal (Ley 172-13).
  - **Criterio de aceptación:** un preflight sin cifrado exigido muestra el aviso y no escribe sin confirmación; pruebas por dialecto.
  - **Esfuerzo:** bajo
  - **Depende de:** ninguna
  - **Cerrada:** 2026-09-23 — `transport_encryption_issue` en el preflight (bloqueo sin cifrado, advertencia con desactivación expresa o certificado no validado, información en servidor local); prueba por dialecto; suite Rust 502 aprobadas. El texto del aviso sigue pendiente de revisión legal.
- [x] **[T10-07] Hacer efectiva la comprobación de CSP del gate de red**
  - **Área:** DevOps y configuración · **Severidad:** Media
  - **Ubicación:** `tools/check-network-policy.mjs:55`; `tools/check-network-policy.test.mjs`
  - **Qué hacer:** leer `app.security.csp` y `devCsp` como objetos y recorrer sus directivas; añadir una prueba de mutación con un origen externo; declarar como salidas permitidas la entrega ODBC por acción explícita y el updater. Defecto latente desde `47aaa50` (2026-08-23), no cubierto por T7-03.
  - **Criterio de aceptación:** la prueba de mutación falla con un origen externo; `npm run network:check` sigue verde con la configuración actual.
  - **Esfuerzo:** bajo
  - **Depende de:** ninguna
  - **Cerrada:** 2026-09-23 — `findCspViolations` recorre `app.security.csp`/`devCsp`; pruebas de mutación (origen externo, comodín, CSP ausente) y de salidas declaradas: 8/8; `npm run network:check` verde.
- [ ] **[T10-08] Crear la sección de versión del CHANGELOG y exigir cabecera en el gate**
  - **Área:** Documentación · **Severidad:** Media
  - **Ubicación:** `CHANGELOG.md:6`; `tools/check-documentation.mjs:180`
  - **Qué hacer:** trasladar lo incluido en `v1.26.0-rc.1` a una sección `## [1.26.0]` (o de candidato) y cambiar la comprobación a una cabecera, no a una mención en prosa.
  - **Criterio de aceptación:** el gate falla si la versión solo aparece en prosa; el CHANGELOG tiene la sección.
  - **Esfuerzo:** bajo
  - **Depende de:** ninguna
  - **Parcial 2026-09-23:** gate implementado (cabecera obligatoria, modo `--require-release-section` en Release/Package, 6/6 pruebas). Queda abierta hasta crear `## [1.26.0]` al cortar la versión; hoy el modo estricto falla, como debe.
- [x] **[T10-09] Regenerar la ficha de dependencias de AUDITORIA desde los manifiestos**
  - **Área:** Documentación · **Severidad:** Media
  - **Ubicación:** `AUDITORIA.md:118-186`; `tools/check-documentation.mjs:181`
  - **Qué hacer:** generar o comparar la tabla contra `package.json` y `src-tauri/Cargo.toml`; registrar la subida de versiones mayores de `cb86ea5` (2026-09-20) con fecha, motivo y resultado; incluir las dependencias Cargo que faltan.
  - **Criterio de aceptación:** el gate falla si una versión declarada difiere; la ficha coincide con los manifiestos.
  - **Esfuerzo:** bajo
  - **Depende de:** ninguna
  - **Cerrada:** 2026-09-23 — `validateDependencySnapshot` compara la ficha con ambos manifiestos (detectó 28 diferencias antes de regenerarla); `docs:check` verde, 7/7 pruebas.
- [x] **[T10-10] Devolver el foco al disparador al cerrar el diálogo de importación**
  - **Área:** Accesibilidad · **Severidad:** Media
  - **Ubicación:** `src/features/load/LoadPhase.tsx:127`; `src/components/ModalDialog.tsx:66-68`, `:90`
  - **Qué hacer:** pasar al diálogo el elemento al que debe volver el foco o capturarlo en el evento del usuario; usar `aria-disabled` durante la inspección; recurrir al contenedor de la etapa si el disparador ya no existe.
  - **Criterio de aceptación:** E2E con teclado: Escape y «Cancelar» devuelven el foco a «Seleccionar dataset» (hoy queda en `BODY`).
  - **Esfuerzo:** bajo
  - **Depende de:** ninguna
  - **Cerrada:** 2026-09-23 — prueba unitaria y E2E de teclado (Escape y Cancelar) que fallan sin el arreglo y pasan con él; suite frontend 458/458 y E2E 23/23.
- [ ] **[T10-11] Unificar la paleta oscura de los temas Oscuro y Sistema**
  - **Área:** UI/UX · **Severidad:** Media
  - **Ubicación:** `src/workflow-styles.css:338-345` y bloques `:root[data-theme="dark"]`; `src/styles.css:1347-1380`
  - **Qué hacer:** definir una sola vez los tokens oscuros y aplicarlos desde ambos selectores; retirar los overrides por componente duplicados.
  - **Criterio de aceptación:** una prueba compara estilos computados entre Oscuro y Sistema con SO oscuro y encuentra 0 diferencias (hoy 12 de 153 textos en Cargar).
  - **Esfuerzo:** medio
  - **Depende de:** T10-02
- [x] **[T10-12] Mostrar en Entregar las columnas con señales de datos personales**
  - **Área:** UI/UX · **Severidad:** Media
  - **Ubicación:** `src/features/delivery/DeliveryPhase.tsx:1673`; `src/features/prepare/PreparePhase.tsx:141-144`
  - **Qué hacer:** junto a «Protección de datos personales», indicar cuántas columnas y cuáles se detectaron y ofrecer enmascarar o aplicar hash; pedir confirmación si se exporta sin protección habiendo señales. El texto requiere revisión legal (Ley 172-13).
  - **Criterio de aceptación:** con un perfil con señales `email` o `name`, Entregar muestra el aviso; prueba de componente.
  - **Esfuerzo:** bajo
  - **Depende de:** ninguna
  - **Cerrada:** 2026-09-23 — Entregar lista las columnas con señales de correo, teléfono, dirección o nombre y exige confirmar si se exporta sin protección; 2 pruebas de componente; vitest 460/460 y E2E 23/23. El texto del aviso sigue pendiente de revisión legal (Ley 172-13).
- [x] **[T10-13] Añadir timeouts explícitos a las llamadas ODBC**
  - **Área:** Arquitectura · **Severidad:** Media
  - **Ubicación:** `src-tauri/src/remote_databases.rs:154`, `:190`, `:786`, `:910`, `:1054`, `:853-862`, `:975-984`
  - **Qué hacer:** fijar timeout de conexión y de sentencia en DDL e inserciones y traducirlo a un mensaje claro.
  - **Criterio de aceptación:** una prueba contra un servidor inaccesible termina dentro del límite documentado con un mensaje comprensible.
  - **Esfuerzo:** bajo
  - **Depende de:** ninguna
  - **Cerrada:** 2026-09-23 — login 15 s y sentencias 300 s (DDL e inserciones preparadas); mensaje para HYT00/HYT01 sin secretos; prueba manual contra dirección inalcanzable con ODBC Driver 18: falla en 15,17 s. Suite Rust 501 aprobadas y 6 ignoradas; Clippy verde.
- [ ] **[T10-14] Entregar por lotes y evitar duplicados al reintentar `append`**
  - **Área:** Arquitectura · **Severidad:** Media
  - **Ubicación:** `src-tauri/src/remote_databases.rs:841-868`, `:958-999`
  - **Qué hacer:** insertar por lotes dentro de la transacción; detectar una reentrega al mismo destino (marcador de lote o tabla de control) y pedir confirmación; medir antes y después.
  - **Criterio de aceptación:** medición registrada antes y después; un reintento tras un commit dudoso no duplica filas sin confirmación.
  - **Esfuerzo:** medio
  - **Depende de:** T10-13
- [ ] **[T10-15] Registrar pánicos y recuperar el estado envenenado**
  - **Área:** Código · **Severidad:** Media
  - **Ubicación:** `src-tauri/src/lib.rs:116-259`; `src-tauri/src/dataset.rs:1705-1737`
  - **Qué hacer:** instalar un hook de pánico que escriba un informe local sin datos ni rutas, coherente con el contrato de diagnóstico; en las operaciones de datos, capturar el pánico y recuperar el mutex invalidando el estado afectado.
  - **Criterio de aceptación:** una prueba provoca un pánico en una operación y comprueba que la siguiente responde y que existe el informe.
  - **Esfuerzo:** medio
  - **Depende de:** ninguna
- [ ] **[T10-16] Verificar si el mutex del dataset bloquea el hilo principal** *(pendiente de verificación)*
  - **Área:** Arquitectura · **Severidad:** Media
  - **Ubicación:** `src-tauri/src/dataset.rs:7791-7963`; `src-tauri/src/dataset/history.rs:684-695`
  - **Qué hacer:** medir en WebView2 si `get_history_state`, que es síncrono y toma `current`, congela la ventana mientras una consulta larga mantiene el lock; si se confirma, convertir los comandos síncronos que toman locks en asíncronos o usar `try_lock` con estado ocupado.
  - **Criterio de aceptación:** con una operación de al menos 5 s en curso, `get_app_info` responde en menos de 100 ms.
  - **Esfuerzo:** bajo
  - **Depende de:** ninguna
- [x] **[T10-17] Aislar los smokes nativos de los datos reales de la app**
  - **Área:** DevOps y configuración · **Severidad:** Media
  - **Ubicación:** `tools/probe-webview2-cdp.ps1`; `tools/probe-webview2-native-selectors.mjs:419`, `:472-474`
  - **Qué hacer:** respaldar y restaurar `%APPDATA%\app.columnia.desktop` en cada corrida o usar un directorio de datos alternativo en debug; eliminar, con aprobación, las 3 tareas sintéticas «Native reusable task …» del 2026-09-23 que quedaron en el catálogo real.
  - **Criterio de aceptación:** una corrida abortada no deja entradas en el catálogo real; la restauración queda en la evidencia.
  - **Esfuerzo:** medio
  - **Depende de:** ninguna
  - **Cerrada:** 2026-09-23 — `tools/app-data-guard.ps1` respalda `%APPDATA%\app.columnia.desktop` en el temporal del sistema y lo restaura tras detener la app (`probe-webview2-cdp.ps1` en modo normal, `probe-webview2-restart.ps1` envolviendo ambas fases); `smoke:cdp` y `smoke:restart` aprobados con datos byte a byte idénticos, `appDataRestored: true` y ninguna copia residual. Se borraron las 3 tareas sintéticas del 2026-09-23 tras respaldar el catálogo.
- [ ] **[T10-18] Medir la cobertura del motor Rust**
  - **Área:** QA y testing · **Severidad:** Media
  - **Ubicación:** `tools/check.ps1:241-253`; `tools/check-coverage.mjs:4-8`
  - **Qué hacer:** añadir cobertura Rust al perfil Full como línea base por módulo; añadir umbrales para `deliveryModel.ts` y `ReviewPhase.tsx`.
  - **Criterio de aceptación:** informe de cobertura Rust en `.local/validation`; umbrales nuevos aplicados.
  - **Esfuerzo:** medio
  - **Depende de:** ninguna
- [x] **[T10-19] Comprobar contraste y reglas axe por tema en los E2E**
  - **Área:** QA y testing · **Severidad:** Media
  - **Ubicación:** `e2e/workflow-accessibility.spec.ts`; `e2e/design-preferences.spec.ts`
  - **Qué hacer:** ejecutar comprobaciones automáticas de accesibilidad y contraste en las cuatro fases para claro, oscuro y sistema oscuro.
  - **Criterio de aceptación:** la suite falla con los contrastes actuales y pasa tras T10-02.
  - **Esfuerzo:** bajo
  - **Depende de:** T10-02
  - **Cerrada:** 2026-09-23 — `e2e/theme-contrast.spec.ts` mide contraste AA de los textos visibles de Revisar, Preparar y Entregar en claro, oscuro y sistema oscuro, sin dependencias nuevas; con el CSS anterior a T10-02 falla en oscuro y sistema oscuro (1,69:1 y 2,28:1) y pasa con el actual; E2E 26/26. Límite: con ese CSS no detectó las etiquetas de tipo en claro.
- [x] **[T10-20] Actualizar THREAT_MODEL.md**
  - **Área:** Documentación · **Severidad:** Media
  - **Ubicación:** `THREAT_MODEL.md:3`, `:72`, `:77`
  - **Qué hacer:** añadir las superficies de entrega ODBC, SQL local con DuckDB, catálogos de tareas y presets y updater; actualizar las cifras (85 comandos de producción, esquema v15, 70 estructuras) y los riesgos de T10-03 a T10-06.
  - **Criterio de aceptación:** fecha actualizada; ninguna cifra contradice el inventario IPC; la checklist cubre las salidas de red.
  - **Esfuerzo:** medio
  - **Depende de:** ninguna
  - **Cerrada:** 2026-09-23 — fecha 2026-09-23; 85 comandos, 70 estructuras y esquema v15 coinciden con el inventario IPC y `projects.rs`; superficies de SQL local, entrega ODBC y catálogos con controles y riesgos residuales (T10-05, T10-14, T10-17 abiertas); la checklist pregunta por salidas de red.
- [x] **[T10-21] Corregir contradicciones documentales puntuales**
  - **Área:** Documentación · **Severidad:** Media
  - **Ubicación:** `CONTEXTO.md:69`, `:138-166`, `:422-425`, `:608`, `:1819`, `:2178-2181`; `README.md:7`
  - **Qué hacer:** unificar el recuento de módulos (38) y las cifras de pruebas; retirar la nota obsoleta de `STATUS_ENTRYPOINT_NOT_FOUND`; corregir la descripción de `App.tsx`; devolver las filas huérfanas del registro a su tabla; actualizar la insignia de versión del README.
  - **Criterio de aceptación:** ninguna contradicción de las listadas; `npm run docs:check` verde.
  - **Esfuerzo:** bajo
  - **Depende de:** ninguna
  - **Cerrada:** 2026-09-23 — 38 módulos, cifras de pruebas y cobertura del gate Full, nota de `STATUS_ENTRYPOINT_NOT_FOUND` marcada como superada, descripción de `App.tsx` corregida, 4 filas huérfanas devueltas a la tabla del registro e insignia 1.26.0 en el README; `docs:check` verde. La fila «Pruebas observadas» de la ficha se actualiza al cierre de la tanda.
- [ ] **[T10-22] Compactar CONTEXTO.md y la cola vigente**
  - **Área:** Documentación · **Severidad:** Media
  - **Ubicación:** `CONTEXTO.md:10-438`; `docs/reference/roadmap-current.md:210`
  - **Qué hacer:** reducir el estado vigente a una pantalla con cifras tomadas del último informe JSON del gate; llevar el historial por corte al CHANGELOG o a la evidencia; limitar las celdas de la cola y enlazar la evidencia.
  - **Criterio de aceptación:** estado vigente ≤ 600 palabras (hoy 4.191); ninguna celda > 80 palabras (hoy 1.302 en RV04); tabla de traslado sin afirmaciones perdidas.
  - **Esfuerzo:** medio
  - **Depende de:** T10-21
- [ ] **[T10-23] Extraer el controlador de Revisar de App.tsx**
  - **Área:** Arquitectura · **Severidad:** Media
  - **Ubicación:** `src/App.tsx` (39 `useState`, 28 `useRef`); `src/features/review/ReviewPhase.tsx` (unas 106 props)
  - **Qué hacer:** crear un controlador de Revisar análogo a `usePrepareController` y reducir las props de la fase.
  - **Criterio de aceptación:** `App.tsx` deja de contener el estado de Revisar; `ReviewPhase` recibe menos de 30 props; pruebas y E2E verdes.
  - **Esfuerzo:** medio
  - **Depende de:** ninguna
- [ ] **[T10-24] Extraer el controlador de Entregar de App.tsx**
  - **Área:** Arquitectura · **Severidad:** Media
  - **Ubicación:** `src/App.tsx`; `src/features/delivery/DeliveryPhase.tsx` (25 `useState`)
  - **Qué hacer:** crear un controlador de Entregar y trasladar a él el estado de exportación, preflight y presets.
  - **Criterio de aceptación:** `App.tsx` deja de contener el estado de Entregar; pruebas y E2E verdes.
  - **Esfuerzo:** medio
  - **Depende de:** T10-23
- [ ] **[T10-25] Añadir un linter de frontend con reglas de hooks**
  - **Área:** Código · **Severidad:** Baja
  - **Ubicación:** `package.json`; `tsconfig.json:17`
  - **Qué hacer:** incorporar un linter con reglas de hooks de React al perfil Fast y separar los tipos de pruebas (`node`, `vitest/globals`) del tsconfig de producción.
  - **Criterio de aceptación:** el linter corre en Fast con 0 errores; el código de producción no compila si usa tipos de Node.
  - **Esfuerzo:** bajo
  - **Depende de:** ninguna
- [ ] **[T10-26] Formatear números, tamaños y tipos desde un único módulo**
  - **Área:** UI/UX · **Severidad:** Baja
  - **Ubicación:** `src/components/UpdatePanel.tsx:36`; `src/features/delivery/DatasetMetrics.tsx:4`; `src/features/load/LoadPhase.tsx:37`; `src/features/prepare/HistoryBar.tsx:79`; `src/features/projects/ProjectsPanel.tsx:436`
  - **Qué hacer:** un formateador con locale explícito y una sola convención de unidades; nombres de tipo en español en lugar de `Int64`, `String` o `Float64`.
  - **Criterio de aceptación:** un mismo archivo muestra el mismo tamaño en el diálogo, Revisar y Entregar (hoy «1 KiB» y «1.0 KB»); sin `toFixed` en textos visibles.
  - **Esfuerzo:** bajo
  - **Depende de:** ninguna
- [x] **[T10-27] Resolver los avisos menores de accesibilidad**
  - **Área:** Accesibilidad · **Severidad:** Baja
  - **Ubicación:** `src/features/load/LoadPhase.tsx:739`; `src/features/review/ReviewPhase.tsx:1253`; `src/App.tsx:1641`
  - **Qué hacer:** anunciar los valores ausentes como tales y no como la palabra «null»; dejar solo `dt`/`dd` en el `<dl>` de calidad; hacer que el nombre accesible de cada paso contenga su texto visible.
  - **Criterio de aceptación:** Lighthouse 100 en accesibilidad en las cuatro fases (hoy 97 en Revisar y 96 en Preparar).
  - **Esfuerzo:** bajo
  - **Depende de:** T10-02
  - **Cerrada:** 2026-09-23 — `MissingValue` anuncia «valor ausente»; `<dl>` de calidad solo con `dt`/`dd`; pasos laterales sin `aria-label` redundante. Lighthouse accesibilidad 100 en Revisar, Preparar y Entregar (`.local/auditoria-2026-09-23/lighthouse-T10-27-*`); vitest 460/460, E2E 23/23.
- [x] **[T10-28] Hacer visibles los espacios iniciales y finales en las vistas previas**
  - **Área:** UI/UX · **Severidad:** Baja
  - **Ubicación:** `src/styles.css:604`; `src/features/load/LoadPhase.tsx:739`
  - **Qué hacer:** mostrar un marcador para los espacios que el HTML colapsa.
  - **Criterio de aceptación:** una celda con espacios alrededor los muestra marcados en la muestra y en la vista previa.
  - **Esfuerzo:** bajo
  - **Depende de:** ninguna
  - **Cerrada:** 2026-09-23 — `CellText`/`renderCellValue` marcan con «·» los espacios exteriores en la muestra de importación y en las vistas previas de Revisar, con aviso para lectores de pantalla; 3 pruebas; vitest 463/463 y E2E 23/23.
- [x] **[T10-29] Guardar evidencia JSON completa en los smokes**
  - **Área:** DevOps y configuración · **Severidad:** Baja
  - **Ubicación:** `tools/probe-webview2-cdp.ps1:929`
  - **Qué hacer:** aumentar la profundidad de serialización y validar que la evidencia no contenga `System.Object[]` ni `@{`.
  - **Criterio de aceptación:** `summary.json` sin representaciones de objetos de PowerShell.
  - **Esfuerzo:** bajo
  - **Depende de:** ninguna
  - **Cerrada:** 2026-09-23 — `ConvertTo-Json -Depth 32` y aviso si reaparecen objetos sin serializar; `npm run smoke:cdp` en WebView2 aprobado con evidencia `.local/validation/webview2-cdp/20260924T013549Z` sin `System.Object[]` ni `@{`.
- [ ] **[T10-30] Alinear el perfil Full con lo que se declara de él**
  - **Área:** DevOps y configuración · **Severidad:** Baja
  - **Ubicación:** `tools/check.ps1:206-266`
  - **Qué hacer:** incluir en Full los E2E y los escaneos de secretos y red, o declarar explícitamente que no los cubre; ejecutar las pruebas de todos los objetivos Rust.
  - **Criterio de aceptación:** el informe JSON del perfil Full lista E2E, secretos y red.
  - **Esfuerzo:** bajo
  - **Depende de:** T10-07
- [x] **[T10-31] Dar un significado real a `-DryRun` en release.ps1**
  - **Área:** DevOps y configuración · **Severidad:** Baja
  - **Ubicación:** `tools/release.ps1:3`, `:237-242`
  - **Qué hacer:** que `-DryRun` muestre el plan sin empaquetar, o retirar el parámetro.
  - **Criterio de aceptación:** con y sin `-DryRun` el comportamiento difiere de forma documentada.
  - **Esfuerzo:** bajo
  - **Depende de:** ninguna
  - **Cerrada:** 2026-09-23 — `-DryRun` imprime el plan y marca el informe; se conserva a propósito el mismo recorrido de gates y build porque `release:updater:dry-run` debe verificar artefactos firmados reales (docs/reference/legal-distribution-review.md). Documentado en la cabecera del script; parseo PowerShell sin errores.
- [x] **[T10-32] Ofrecer un hook local de pre-push con el perfil Fast**
  - **Área:** DevOps y configuración · **Severidad:** Baja
  - **Ubicación:** `CONTRIBUTING.md`
  - **Qué hacer:** versionar un hook opcional que ejecute el perfil Fast y Clippy antes de publicar. No es CI y respeta la decisión vigente.
  - **Criterio de aceptación:** documentado en CONTRIBUTING; un push con Clippy en rojo se detiene localmente.
  - **Esfuerzo:** bajo
  - **Depende de:** ninguna
  - **Cerrada:** 2026-09-23 — `.githooks/pre-push` (perfil Fast + Clippy `-D warnings`, `set -e`) documentado en CONTRIBUTING; ejecutado de principio a fin con código 0. Activación opcional por clon con `git config core.hooksPath .githooks` (no activado aquí).
- [x] **[T10-33] Redactar de forma robusta los secretos en los errores ODBC**
  - **Área:** Seguridad · **Severidad:** Baja
  - **Ubicación:** `src-tauri/src/remote_databases.rs:1256-1271`
  - **Qué hacer:** analizar la cadena de conexión respetando los valores entre llaves y ampliar la lista de claves sensibles.
  - **Criterio de aceptación:** pruebas con valores entre llaves que contienen `;` quedan redactadas por completo.
  - **Esfuerzo:** bajo
  - **Depende de:** ninguna
  - **Cerrada:** 2026-09-23 — `odbc_attributes` respeta las llaves y `is_secret_attribute` amplía las claves; prueba con `PWD={p;a=ss}` y `AccessToken` redactados por completo; pruebas ODBC 15/15 y Clippy verde.

### Decisiones cerradas de esta reauditoría

- **CI:** no se propone. La decisión vigente está en `CONTEXTO.md` (sin CI
  por política); T10-32 es un hook local opcional.
- **FlaUI:** no se propone. La interfaz es web dentro de WebView2 y ya se
  conduce por CDP y Playwright.
- **RV03, RV12 y RV13:** no se reabren. No se re-verificaron a fondo en esta
  pasada y no hay evidencia de regresión.
- **Excepciones RUSTSEC-2026-0194/0195 (`quick-xml` 0.39.4):** se mantienen.
  Se verificó que solo llegan por `object_store`; `calamine` usa `quick-xml`
  0.41 y 0.42, ya corregidas.
- **SEO:** no aplica a una aplicación de escritorio.
- **Criterio de salida de V1:** no se modifica en esta reauditoría; incluir
  T10-01 y T10-02 queda como decisión del responsable.

### Progreso del Tier 10

| Fecha | Cerradas | Nota |
| --- | ---: | --- |
| 2026-09-23 | 0 de 33 | Tier abierto por la reauditoría. |
| 2026-09-23 | 20 de 33 | Fase 2 en curso: cerradas hasta ahora T10-17 y anteriores de esta tanda. |

## Criterio de salida de V1

V1 puede declararse soportada cuando RV01, RV02, RV04–RV11 y RV14 estén
cerradas sobre un candidato identificable; no queden defectos P0/P1; la suite
completa y los gates de privacidad, seguridad y rendimiento pasen; y el canal
de distribución elegido tenga autorización y evidencia reproducible. RV15 no
forma parte del criterio salvo que la beta lo active.
