# Auditoría general y oportunidades de valor — Columnia

Fecha: 13 de septiembre de 2026. Versión revisada: `0.167.0`.
Commit de referencia: `33042eceec61c08897cc64859ce829a66f671c3b`.

## Dictamen

Columnia ya tiene un núcleo amplio para preparar datos localmente. Su siguiente avance de mayor valor es demostrar que una persona completa el flujo con sus datos reales, recupera su trabajo y obtiene una entrega correcta. Añadir más funciones antes de cerrar esa evidencia aumentaría la complejidad sin demostrar utilidad.

Las prioridades son: corregir la entrada al proyecto desde la documentación, completar la beta, verificar accesibilidad y destinos reales, y hacer más comprensibles el impacto de los cambios y los límites de procesamiento. Después conviene mejorar reutilización de recetas, tratamiento de excepciones y repetición de trabajos.

No se identificó un defecto crítico reproducido en esta revisión. Esto no constituye una certificación de seguridad ni de integridad del motor: la revisión de código fue selectiva. Como seguimiento de implementación se ejecutó la suite nativa de biblioteca, además de la suite Vitest.

## Alcance y método

Se revisaron arquitectura, producto, carga, revisión, preparación, entrega, proyectos, automatización, privacidad, rendimiento, pruebas, documentación y distribución. Se usó CodeGraph antes de localizar código en el repositorio indexado; se contrastaron sus resultados con contratos y documentación vigentes. Sus avisos de “sin pruebas” no se tomaron como demostración de ausencia de cobertura.

El proyecto principal del workspace es `Columnia/`. `BernieJimenez-profile/` contiene un README de perfil, no otro producto ejecutable; se revisó su presentación básica. `tmp/`, dependencias instaladas y artefactos generados no se consideran funcionalidades del producto. No se hizo una revisión exhaustiva de cada línea, una investigación de mercado, una prueba con personas ni una inspección visual nativa nueva.

Fuentes de referencia:

| Ref. | Evidencia local | Qué permite concluir |
| --- | --- | --- |
| E01 | [README](README.md), [package.json](package.json), [check.ps1](tools/check.ps1) | Promesa del producto, versiones requeridas y comandos disponibles. |
| E02 | [Alcance V1](docs/reference/v1-scope.md), [matriz vigente](docs/reference/feature-parity.md) | Capacidades declaradas, límites y exclusiones deliberadas. |
| E03 | [ROADMAP](ROADMAP.md), especialmente Tier 8 y T6-05 | Beta pendiente y aceptación externa ODBC incompleta. El historial contiene pendientes antiguos ya superados. |
| E04 | [Protocolo beta](docs/how-to/run-beta-validation.md), [plantillas](docs/templates/beta-session.md) | Criterios reproducibles de validación con personas. |
| E05 | [Decisión de distribución](docs/reference/legal-distribution-decision.json), [configuración Tauri](src-tauri/tauri.conf.json) | Código fuente aprobado; distribución binaria separada; endpoints del updater vacíos. Es un estado documental, no una opinión jurídica. |
| E06 | [Preparación](src/features/prepare/PreparePhase.tsx), [entrega](src/features/delivery/DeliveryPhase.tsx), [modelo de entrega](src/features/delivery/deliveryModel.ts) | Correcciones reversibles, confirmaciones, calidad y conexión ODBC ya presentes. |
| E07 | [Proyectos](src-tauri/src/projects.rs), [automatización](src-tauri/src/automation.rs), [CLI](docs/reference/cli.md) | Persistencia durable y batch existentes; batch secuencial con resultados parciales ante fallo tardío. |
| E08 | [Motor de datos](src-tauri/src/dataset.rs), [DuckDB](src-tauri/src/duckdb_query.rs), [presupuestos](fixtures/performance/performance-baseline-v1.json) | Complejidad del motor y presupuestos explícitos; no prueba por sí sola el rendimiento actual. |
| E09 | [Checklist de accesibilidad](ACCESSIBILITY_MANUAL_CHECKLIST.md), [evidencia de release](docs/reference/release-evidence.md) | Existe infraestructura de verificación; aceptación manual no equivale a tests web. |
| E10 | [Privacidad de red](docs/reference/network-privacy.md), [modelo de amenazas](THREAT_MODEL.md) | Contratos de privacidad que las ampliaciones deben conservar. |

## Verificación ejecutada

| Comprobación | Resultado de esta revisión | Límite |
| --- | --- | --- |
| Estado inicial de Git | Árbol sin cambios reportados | Referencia previa a crear este informe. |
| `npm run build` | Aprobado: TypeScript y compilación Vite | No compila ni valida el ejecutable Rust. |
| `npm test -- --reporter=dot` | Aprobado: 48 archivos, 361 pruebas | Suite Vitest; no implica cobertura total ni validación nativa. |
| Pruebas dirigidas E02 | Aprobadas: 25 pruebas en `PreparePhase` y mapeo de esquema | No cubre el esquema de origen ausente en recetas v1. |
| `npm run docs:check` | Aprobado; ahora también ejecuta cuatro pruebas de contrato README/package.json | No valida instrucciones ejecutadas en otros sistemas. |
| `npm run beta:workflows:check` | Aprobado: los dos casos sintéticos reproducen sus resultados y métricas esperados | Valida el contrato de las fixtures; no prueba por sí solo la interacción de Columnia ni evidencia demanda real. |
| `cargo test --manifest-path src-tauri/Cargo.toml --lib` | 429 aprobadas, 4 ignoradas; 0 fallidas | Cuatro ignoradas por selección/intencionalidad de sus pruebas. |
| `npm run incremental:check` | Aprobado: 14 casos declarativos enlazan y ejecutan 16 regresiones nativas | Verifica rutas críticas; no sustituye la matriz de escala de H04. |
| `npm run network:check`, `npm run secrets:check`, `npm run ipc:check` | Aprobados: política de red, 551 archivos sin secretos y 70 comandos de producción / 64 estructuras IPC | Son gates locales; no constituyen auditoría externa. |
| Regresión de rollback/reapertura de proyecto | Caso dirigido aprobado: fallo al publicar una actualización, reapertura en un almacén nuevo y datos/configuración anteriores intactos | Aún no cubre espacio agotado, permisos ni cancelación en puntos de commit. |
| `npm run perf:check` | Aprobado con el baseline existente | No equivale a la matriz ampliada de H04. |
| Matriz nativa de archivos difíciles | CSV con BOM, campos citados, delimitadores/saltos de línea y valores léxicos; libro vacío con error claro; fórmulas cacheadas; encabezados repetidos y fechas ambiguas | Casos sintéticos del lector; no sustituyen archivos reales diversos. |
| `npm run legal:check` | Aprobado para código fuente | El propio gate mantiene instaladores/updater bloqueados. |

No se ejecutaron Clippy, Playwright, benchmarks, instalación, updater, servidores ODBC, lector de pantalla, auditoría actualizada de dependencias ni gates Full/Release. Las evidencias históricas se citan como antecedentes, no como resultados nuevos.

## Capacidades que ya existen: no recrearlas

- [x] Importación local de delimitados, JSON, Parquet y libros, según la matriz vigente.
- [x] Perfilado de calidad, visualizaciones y tendencias temporales; su existencia no elimina la necesidad de aceptación con usuarios.
- [x] Recetas, transformaciones, deshacer/rehacer y confirmaciones para cambios sensibles.
- [x] SQL local, comparación por clave, resolución de conflictos, consolidación y JOIN.
- [x] Contratos de calidad y entrega explícita sin validación cuando la persona lo confirma.
- [x] Exportaciones locales, bundle auditable y entrega ODBC con limitaciones declaradas.
- [x] Proyectos durables, recuperación, preferencias y CLI con ejecución batch.
- [x] Controles de privacidad, recursos, ejecución incremental y guardias de RAM.
- [x] Pruebas, herramientas de validación local y documentación de distribución.

Estas casillas indican existencia comprobada en contratos/código o documentación vigente; no certifican todas sus variantes en ejecución.

## Cómo usar las casillas pendientes

**P0:** bloqueo demostrado de integridad o seguridad; ninguno reproducido aquí. **P1:** cerrar antes de declarar lista la beta/release correspondiente. **P2:** siguiente mejora de utilidad. **P3:** expansión condicionada a demanda.

**Confirmado:** discrepancia observada. **Pendiente vigente:** registrado por el proyecto, falta aceptación. **Propuesta:** ampliación cuya ausencia absoluta no se afirma; revisar el flujo existente antes de diseñar. **Verificar:** riesgo a comprobar, no bug demostrado.

Esfuerzo relativo: **S** acotado; **M** varios componentes; **L** motor, persistencia o validación extensa. No son compromisos de fechas. Cada casilla se cierra con evidencia y responsable asignado; las propuestas no son obligaciones de V1.

## A. Producto y aceptación

- [ ] **A01 · P1 · Pendiente vigente · M — Completar Gate 1 de beta.** Valor: saber si Columnia resuelve trabajo real. Ejecutar tres sesiones con participantes distintos, mismo RC/commit, dos casos reales por sesión y al menos tres datasets reales distintos. **Cierre:** 24/30 tareas sin ayuda, guardado/reapertura y entrega externa verificados, ningún P0/P1 abierto y resumen sanitizado. **Base:** E03–E04, Tier 8. **Dependencia:** validación Full del candidato; requiere personas reales.
- [ ] **A02 · P1 · Pendiente vigente · M — Convertir fallos beta en casos reproducibles.** Valor: corregir causas sin conservar datos privados. Crear fixtures sintéticas mínimas por defecto observado, priorizar por pérdida de trabajo y bloqueo del flujo. **Cierre:** reproducción antes de corregir, regresión pertinente después y decisión documentada de P2/P3. **Base:** E04. **Depende de:** A01.
- [ ] **A03 · P2 · Propuesta · S — Elegir dos recorridos de negocio principales.** Valor: enfocar diseño y documentación. **Avance aplicado:** [`beta-workflows.md`](docs/reference/beta-workflows.md) define entradas, resultados esperados y métricas para preparar un reporte periódico y comparar dos inventarios; las fixtures son sintéticas y el protocolo pide comprobar primero si cada flujo refleja trabajo real. **Cierre pendiente:** evidenciar su necesidad con participantes beta y sustituirlos como recorridos principales si la observación contradice estas hipótesis. **Base:** E01–E04.
- [ ] **A04 · P2 · Propuesta · M — Medir fricción después de cada mejora importante.** Valor: comprobar que una nueva interfaz ayuda. Reutilizar Gate 2 y las métricas existentes, sin telemetría. **Cierre:** comparación local con Gate 1 de ayuda requerida, retrocesos y finalización, sin cambiar las definiciones para favorecer el resultado. **Base:** E03–E04. **Depende de:** A01.

## B. Documentación y entrada al proyecto

- [x] **B01 · P1 · Confirmado · S — Corregir requisitos de Node/npm del README.** El README dice Node 22+, mientras `engines` exige Node `>=24.14.0 <25` y npm `>=11.10.1 <12`. Valor: evitar instalaciones incompatibles. **Cierre:** README ahora coincide con `package.json`; checker documental comprueba ambas versiones. **Base:** E01.
- [x] **B02 · P1 · Confirmado · S — Corregir `npm run check`.** El README lo recomendaba, pero no existe ese script. **Cierre:** la guía ahora usa `npm run build`, `npm test`, `npm run docs:check` y `npm run legal:check`, todos declarados en el manifiesto. **Base:** E01.
- [x] **B03 · P2 · Confirmado · S — Ampliar el checker documental para comandos y requisitos.** **Cierre:** el checker rechaza comandos npm inexistentes y requisitos Node/npm desalineados; `docs:check` ejecuta también cuatro regresiones del contrato README/package.json. **Base:** E01 y resultado de esta revisión. **Depende de:** B01–B02.
- [x] **B04 · P2 · Propuesta · S — Separar roadmap operativo del historial.** Valor: que una casilla histórica no se interprete como función ausente. **Cierre aplicado:** [`roadmap-current.md`](docs/reference/roadmap-current.md) lista ID, responsable funcional, dependencia y detalle histórico; los responsables siguen pendientes de asignación nominal. T6-05 ahora describe el round-trip SQL Server que falta y aclara que PostgreSQL/MariaDB ya tienen aceptación documentada. **Base:** E03.

## C. Cargar y comprender archivos

- [ ] **C01 · P2 · Propuesta · M — Preflight explicable de importación.** Valor: detectar hoja/encabezado/tipos incorrectos antes de trabajar. Integrar lo existente en un resumen de decisiones y advertencias. **Avance aplicado:** el diálogo Excel resume formato/tamaño, hojas, hoja elegida y modo de encabezados; aclara que el esquema y los tipos se revisan tras cargar. Una regresión nativa comprueba que un CSV real con identificador `00123` y dos columnas `total` conserva los tres valores, mantiene los encabezados distinguibles y deja el archivo original idéntico; una regresión UI confirma que los valores y encabezados se pueden revisar en la vista previa. **Cierre pendiente:** aún no se permite revisar o cambiar decisiones de encabezado CSV antes de cargar; validar con archivos reales diversos y comprobar con usuarios si ese control resuelve la necesidad. **Base:** E02; ampliar, no rehacer importadores.
- [x] **C02 · P2 · Verificar · M — Matriz de archivos difíciles.** Valor: evitar resultados plausibles pero incorrectos. **Cierre comprobado:** regresiones del lector cubren CSV con BOM, campos citados, delimitadores/saltos de línea y valores léxicos; UTF-8 inválido; hoja XLSX vacía con rechazo claro; fórmulas con valores cacheados; encabezados repetidos y fechas ambiguas. **Base:** E02, límite conocido de libros. Los fixtures son sintéticos y no sustituyen archivos reales diversos.
- [x] **C03 · P2 · Propuesta · M — Perfil de interpretación reutilizable.** Valor: ahorrar ajustes en archivos periódicos. **Cierre aplicado:** perfil v1 persiste formato, hoja/encabezado, convenciones declaradas y esquema; la migración 12→13 conserva proyectos existentes. Antes de activar un dataset se comparan columnas ausentes/agregadas y tipos; se puede cancelar conservando el dataset anterior o importar el esquema nuevo tras confirmarlo. Alias equivalentes como `str`/`String` se canonizan; las convenciones de fecha/número guían la revisión y no convierten valores silenciosamente. **Evidencia:** 361 pruebas UI, 429 pruebas Rust, incluidos perfiles, migración, reapertura, incompatibilidades y alias. **Base:** E02/E07. **Depende de:** C01.
- [x] **C04 · P2 · Propuesta · M — Estimación previa de recursos.** Valor: anticipar operaciones costosas. **Cierre aplicado:** el preflight nativo declara ruta en memoria/source-backed, tamaño fuente, estimación de RAM de materialización (misma fórmula de la guardia: 4× tamaño + 256 MiB) y necesidad aproximada de snapshot/disco temporal. Se muestra antes de cargar archivos grandes, rutas diferidas y libros comprimidos; la elección final sigue en manos de la admisión nativa. **Evidencia:** pruebas de UI del resumen Excel y confirmación previa para CSV source-backed; Rust verifica ruta y estimación para CSV, JSON y ODS; compilación y `ipc:check` aprobados (70 comandos, 64 estructuras). **Límite:** RAM/disco son aproximaciones por tamaño, no reserva ni predicción exacta; compresión, cardinalidad, compresión Parquet y ediciones pueden cambiar el costo. No se bloquea una operación por espacio previsto. **Base:** E02/E08.

## D. Revisar calidad y explicar resultados

- [ ] **D01 · P2 · Propuesta · M — Resumen accionable de problemas.** Valor: ayudar a decidir qué corregir primero. **Avance aplicado:** cada señal medida de nulos, duplicados exactos y tipos incompatibles explica alcance, posible impacto y cautelas; su acción abre Preparar con el control relacionado enfocado. No se añade un puntaje opaco. **Cierre pendiente:** validar en beta que la ruta reduce ayuda requerida y comprobar las explicaciones con usuarios. **Evidencia:** pruebas dirigidas del plan, callback de navegación y foco en Preparar; build aprobado. **Base:** E02/E06.
- [x] **D02 · P2 · Verificar · S — Etiquetas consistentes de cobertura.** Valor: distinguir un resultado completo de uno calculado sobre muestra. **Cierre verificado:** perfil, correlaciones, tendencias, SQL truncado y conteos exponen alcance/límites; el cambio de revisión cancela y descarta una agregación temporal pendiente. La regresión encontró y corrigió el texto concatenado `108de 120`. **Base:** E02/E07.
- [x] **D03 · P2 · Propuesta · M — Comparación agregada antes/después.** Valor: mostrar si la limpieza mejoró los datos. **Cierre aplicado:** comparación bajo demanda entre dos IDs estables de revisiones; perfiles recalculados para cada selección y deltas agregados de filas, columnas, nulos, tipos y reglas. Reglas/columnas no comparables se etiquetan; no se guardan muestras ni perfiles calculados. Cambio de selección descarta resultados obsoletos y la interfaz explica historial no disponible/degradado. **Evidencia:** pruebas dirigidas de Rust, reapertura de proyecto y UI dentro de las suites integradas. **Límite:** ambos snapshots deben cargarse para recalcular; datasets grandes pueden requerir RAM sustancial y cancelar puede esperar a que termine una evaluación larga de regla. **Base:** E06–E07.
- [ ] **D04 · P2 · Propuesta · M — Flujo de excepciones de calidad.** Valor: trabajar con filas que incumplen una regla sin perder el conjunto válido. Empezar por conteos y filtros temporales; cualquier exportación de rechazados será explícita. **Cierre:** coincidencia exacta con la regla validada, límites de paginación y protección de datos equivalentes a la entrega. **Base:** E02/E06/E10; revisar contrato de privacidad antes de ampliar valores expuestos.
- [ ] **D05 · P3 · Propuesta · M — Diccionario de negocio editable.** Valor: explicar significado, unidad y uso esperado de una columna. Extender el diccionario de bundle existente. **Cierre:** descripción/unidad opcionales versionadas, comportamiento definido ante renombre/eliminación y exportación sanitizada según elección. **Base:** E02/E07; requiere ampliar el contrato durable.

## E. Preparar y reutilizar transformaciones

- [x] **E01 · P2 · Propuesta · M — Simulación de impacto de una receta.** Valor: revisar cambios antes de aplicarlos. **Cierre comprobado para cambios de alto impacto:** la confirmación captura la revisión del dataset, se descarta si esta cambia y exige volver a revisar los efectos agregados/advertencias; el handler también rechaza una confirmación obsoleta. Regresión UI cubre cambio de revisión y re-confirmación. Reutiliza undo existente. **Base:** E06/E08. La estimación sigue identificada como estimación, no como resultado ejecutado.
- [x] **E02 · P2 · Propuesta · M — Reaplicar receta con mapeo de esquema.** Valor: reutilizar trabajo cuando cambian encabezados. **Cierre aplicado:** el preflight señala columnas faltantes, incompatibilidades semánticas y cambios de tipo contra el esquema guardado; permite mapeo manual uno-a-uno compatible y nunca sustituye automáticamente. Referencias inválidas bloquean aplicar/guardar; cerrar la revisión conserva el aviso de corrección manual. Recetas v2 guardan tipos de las columnas referenciadas y se rebasan al esquema actual solo tras confirmación; recetas v1 siguen cargando con compatibilidad y el límite esperable de no tener esquema histórico se conserva explícito. Cancelar una carga nueva conserva el borrador. **Evidencia:** regresiones UI/contrato y prueba Rust de persistencia v2, carga de v1 y rechazo de esquema duplicado; build aprobado. **Base:** E02/E07.
- [ ] **E03 · P2 · Propuesta · M — Biblioteca local de recetas y contratos.** Valor: descubrir, probar y reutilizar lo ya guardable. **Cierre:** nombre, descripción, versión y ejemplo sintético; importación rechaza versiones incompatibles y no incluye datos privados de origen. **Base:** E02/E06/E07.
- [ ] **E04 · P3 · Propuesta · L — Catálogos de equivalencias revisables.** Valor: normalizar categorías repetidas, por ejemplo variantes de nombres de productos. **Cierre:** reutilizar reemplazos/joins existentes, preflight de claves duplicadas y no encontradas, historial reversible y cero coincidencias difusas aceptadas sin revisión. **Base:** E02/E06. **Condición:** necesidad repetida en beta.

## F. Entregar y comprobar el resultado

- [ ] **F01 · P1 para declarar soporte del destino · Pendiente vigente · M — Completar round-trip SQL Server.** El serializador booleano ya está implementado y hay evidencia histórica PostgreSQL/MariaDB. **Cierre:** `true/false/null` preservados al exportar y releer en SQL Server desde frame y fuente incremental; registrar driver y configuración sin credenciales. **Base:** E03, T6-05. **Dependencia externa:** instancia accesible; no bloquea uso puramente local.
- [ ] **F02 · P2 · Propuesta · M — Preflight del esquema remoto.** Valor: anticipar incompatibilidades antes de escribir. **Cierre:** diferencias de tipos, nulabilidad, longitud y política de tabla explicadas; parámetros siguen siendo seguros y MySQL `replace` permanece deshabilitado hasta demostrar sustitución atómica. **Base:** E06/E02.
- [ ] **F03 · P2 · Propuesta · M — Presets de entrega para flujos BI.** Valor: reducir errores de delimitador, fechas y decimales. **Cierre:** presets explícitos, sin prometer conectores nuevos, con relectura de archivos sintéticos y verificación manual en la herramienta destino elegida. **Base:** E01/E02. **Depende de:** A03.
- [x] **F04 · P2 · Propuesta · M — Resumen legible de entrega.** Valor: comunicar qué cambió y qué se validó a quien recibe el archivo. **Cierre aplicado:** el bundle incluye `delivery-summary.md` con dimensiones, conteos agregados de operaciones, cobertura/estado del contrato, versiones y SHA-256 de sus miembros; el manifiesto registra bytes y hash del resumen. Pruebas de ZIP materializado y source-backed verifican la integridad y omisión de muestras, valores sensibles y rutas. **Base:** E02/E07/E10.

## G. Proyectos y repetición de trabajo

- [ ] **G01 · P2 · Propuesta · L — Respaldo y restauración de proyecto.** Valor: trasladar o recuperar trabajo durable. Distinguirlo del bundle de entrega de datos. **Cierre:** paquete versionado con integridad, preflight de espacio, revisión del contenido, restauración transaccional y prueba en almacén vacío; no sobreescribir el original. **Base:** E07/E10.
- [x] **G02 · P2 · Propuesta · M — Gestión del espacio de snapshots.** Valor: evitar crecimiento inesperado del disco. **Cierre aplicado:** el historial activo muestra estados retenidos y bytes frente al presupuesto (máximo actual: 12 estados o 1 GiB); explica el retiro de estados antiguos, la conservación del actual y la degradación si un único snapshot rebasa el presupuesto. La lista de proyectos informa el tamaño durable del snapshot actual más las revisiones enumeradas por su manifiesto; si el almacén/manifiesto no puede verificarse, el tamaño se muestra como no disponible y no se intenta limpiar. Las actualizaciones publican el nuevo manifiesto antes de retirar la generación anterior. **Evidencia:** Rust comprueba el tamaño real de los archivos listados, reapertura y conteo exacto; resave conserva IDs de revisiones, restaura undo/redo, elimina la generación anterior después de publicar y la recuperación local sigue disponible; UI prueba las cifras de política y proyecto. **Límite:** informa el almacenamiento de generaciones administradas, no archivos temporales/orfandad fuera de un manifiesto válido; no hay limpieza automática de huérfanos. **Base:** E07/E08.
- [ ] **G03 · P2 · Propuesta · L — Interfaz de lotes sobre la CLI existente.** Valor: repetir recetas sin escribir manifiestos manualmente. **Cierre:** preflight completo, límites actuales respetados, progreso por trabajo y explicación de éxitos parciales; usar el mismo motor. **Base:** E07. **Depende de:** E02/E03 y demanda beta.
- [ ] **G04 · P3 · Propuesta · L — Reanudación segura de lotes.** Valor: no repetir trabajos ya completados tras un fallo tardío. El batch actual se detiene y conserva salidas anteriores. **Cierre:** manifiesto de ejecución con hashes de receta/entrada/salida, invalidación por cambios y política explícita de colisión; nunca sobrescribir por deducción. **Base:** E07. **Depende de:** G03 o demanda CLI comprobada.

## H. Robustez y mantenimiento del motor

- [ ] **H01 · P2 · Confirmado como concentración de código · L — Extraer responsabilidades de `dataset.rs` gradualmente.** Se observaron más de 32.000 líneas no vacías en este archivo; el tamaño no demuestra un bug, pero encarece entender cambios. **Cierre:** separar una responsabilidad por cambio —lectores, perfilado, calidad o exportación— con contratos estables, paridad conductual y sin reescritura general. **Base:** E08.
- [x] **H02 · P2 · Propuesta · M — Matriz ejecutable de caminos incrementales.** Valor: saber qué combinaciones evitan cargar todo en RAM. **Cierre aplicado:** `fixtures/incremental/paths-v1.json` define 14 combinaciones críticas entre CSV, JSON, XLSX y Parquet; [`incremental-paths.md`](docs/reference/incremental-paths.md) se genera desde esa fuente y `npm run incremental:check` enlaza y ejecuta 16 regresiones nativas. Incluye incremental, snapshot privado, fallback, rechazo cancelable y admisión por memoria. **Evidencia:** 16/16 aprobadas; `docs:check` valida documentos. **Límite:** la matriz no reemplaza H04 (escala/recursos comparables). **Base:** E02/E08.
- [ ] **H03 · P2 · Verificar · L — Fallos de disco y cancelación durante publicación.** Valor: proteger trabajo ante interrupciones. **Avance aplicado:** pruebas existentes cubren fallo de E/S del snapshot y fallo antes de publicar metadatos; una regresión vuelve a abrir el proyecto anterior tras fallar una actualización y verifica datos y configuración. **Cierre pendiente:** inyectar disco lleno, permisos denegados y cancelación en puntos de commit; el guardado de proyecto aún no expone cancelación. Reutilizar pruebas existentes antes de agregar. **Base:** E07/E08.
- [ ] **H04 · P2 · Verificar · M — Escala más allá del escenario de referencia.** Valor: hacer promesas medibles. **Avance revisado:** el gate `perf:check` aprueba su baseline; ya existen escenarios fijos de 100 MiB (CSV de 4 columnas), JOIN de 512 MiB y un test separado de perfilado de 2 M de filas × 12 columnas de alta cardinalidad. **Cierre pendiente:** matriz comparable que varíe ancho, cardinalidad, texto y escala y mida tiempo/RAM/disco/cancelación y limpieza por camino. No aumentar presupuestos solo para aprobar. **Base:** E02/E08. Los 100 MiB/512 MiB existentes no prueban toda carga de varios GB.

## I. Accesibilidad, privacidad y distribución

- [ ] **I01 · P1 · Pendiente vigente · M — Aceptación manual nativa de accesibilidad.** Valor: completar tareas con teclado y lector real. **Cierre:** recorrido Cargar→Entregar en Windows con foco, modales, tablas, progreso, escalado y alto contraste; evidencia sobre la revisión vigente y correcciones de bloqueos. **Base:** E03/E09.
- [x] **I02 · P2 · Propuesta · M — Paquete de diagnóstico local revisable.** Valor: facilitar soporte sin compartir datasets. **Cierre aplicado:** la persona elige manualmente una fase, estado y códigos permitidos; las métricas son opcionales y acotadas. La vista previa precede al selector nativo de guardado; no hay envío de red, captura automática ni campos libres. El contrato Rust rechaza campos desconocidos y límites/inconsistencias, guarda atómicamente y omite rutas, nombres, consultas, valores, credenciales, trazas y datos de máquina. **Evidencia:** pruebas UI/contrato/IPC/Rust, `build`, `ipc:check`, `network:check` y `secrets:check` aprobados; se prueba la cancelación por la misma ruta de producción. **Límite:** no se hizo una prueba interactiva nueva del selector nativo. **Base:** E10; reutiliza sanitización existente.
- [ ] **I03 · P1 únicamente para binarios · Pendiente vigente · M — Resolver la decisión de distribución correspondiente.** Valor: mantener coherencia entre publicación y estado aprobado. **Cierre:** responsable registra decisión específica para instaladores/updater y pasa el gate correspondiente; no tratar la aprobación de código fuente como autorización binaria. **Base:** E05. Esta auditoría no modifica esa decisión ni recomienda requisitos jurídicos nuevos.
- [ ] **I04 · P1 únicamente para release · Pendiente vigente · L — Validar el artefacto final de instalación/actualización.** Valor: que el producto instalado funcione fuera del entorno de desarrollo. **Cierre:** candidato de commit limpio, instalación/primera apertura/segunda instancia, actualización fallida, firma inválida y recuperación verificados según checklist; hashes y evidencias ligados al artefacto exacto. **Base:** E03/E05/E09. **Depende de:** I03 para publicar; no reconstruir el updater existente.
- [ ] **I05 · P3 · Pendiente de plataforma · L — Validar macOS/Linux solo si hay demanda.** Valor: ampliar usuarios con soporte real. **Cierre:** build, selectores, archivos, proyectos, accesibilidad y distribución verificados localmente en cada sistema antes de anunciarlo. **Base:** E01–E03. No es requisito para la beta Windows.
- [ ] **I06 · P2 · Propuesta · S — Alinear perfil público con hitos demostrados.** Valor: comunicar claramente qué puede probar alguien hoy. **Avance aplicado:** el README local del perfil aclara que la beta con participantes sigue pendiente, que solo se verificó automatización en Windows y que no hay aceptación manual, soporte macOS/Linux, instaladores/updater o resultados de beta publicados. **Verificación remota 2026-09-13:** el repositorio del perfil, el repositorio Columnia y el enlace del proyecto BCRD responden; la versión pública aún no refleja los commits locales y el README público de Columnia conserva Node 22+ y `npm run check`, ya corregidos en el árbol local. GitHub devuelve el GIF como tipo de contenido no compatible con el lector web; otros enlaces no se pudieron verificar de forma fiable. **Cierre pendiente:** publicar los commits mediante el flujo aprobado y volver a comprobar todos los enlaces y el texto renderizado. **Base:** README del perfil revisado, E01 y comprobación remota.

## Orden recomendado de ejecución

| Tanda | Casillas | Resultado esperado | Condición para avanzar |
| --- | --- | --- | --- |
| 1. Entrada fiable | B01–B03 | Instrucciones reproducibles | Requisitos y comandos coherentes. |
| 2. Evidencia de utilidad | A01–A02, I01; F01 si se ofrece SQL Server | Beta y límites demostrados | Gate beta aprobado; sin bloqueos graves. |
| 3. Menos esfuerzo por dataset | Elegir entre C01, D01, D03, E01–E03 y F04 | Mejoras nacidas de fricción observada | Cada mejora tiene caso real y medición A04. |
| 4. Repetición y robustez | G01–G03, H01–H04 | Trabajo repetible y límites previsibles | Contratos durables y regresiones verificadas. |
| 5. Distribución o expansión | I03–I05 y propuestas P3 seleccionadas | Llegar a más personas con soporte real | Decisión y evidencia de cada canal/plataforma. |

No implementar toda la lista simultáneamente. Para elegir entre propuestas P2, valorar frecuencia del problema, minutos ahorrados, riesgo de errores evitado y esfuerzo. La utilidad es una hipótesis hasta observarla en el flujo real; no se inventan porcentajes de ahorro.

## Qué no conviene agregar ahora

No añadir cuentas, roles, organizaciones, nube obligatoria, telemetría, colaboración remota ni persistencia automática de SQL/credenciales: contradicen el alcance V1. Tampoco priorizar un chat de IA, un marketplace de plugins, conectores masivos o un rediseño visual general sin un caso repetido que lo justifique. No introducir CI/GitHub Actions ni servicios de pago obligatorios: el roadmap establece validación local y ausencia de costos obligatorios.

No tratar las tablas accesibles, tendencias temporales, undo/redo, privacidad, bundle, CLI o procesamiento incremental como funciones ausentes: ya forman parte del producto. Las extensiones descritas deben aprovecharlas.

## Plantilla para cerrar una casilla

```text
ID:
Responsable:
Estado: pendiente / en curso / en validación / cerrada / descartada
Caso real y valor esperado:
Alcance mínimo decidido:
Dependencias:
Commit o artefacto:
Evidencia del criterio de cierre:
Resultado observado y limitaciones:
Fecha y decisión:
```

El resultado de esta revisión es un backlog de 39 acciones, además del inventario de capacidades existentes. B01–B04, C02–C04, D02–D03, E01–E02, F04, G02, H02 e I02 están cerradas con las evidencias indicadas. A03, C01, D01, H03 e I06 avanzaron parcialmente, pero siguen abiertas por los criterios que se detallan en sus casillas; H04 conserva la matriz de escala pendiente. F01 sigue condicionado a una instancia SQL Server accesible: en esta sesión no está configurada y el servicio local detenido no pudo iniciarse por falta de permisos. El informe no da por implementadas las demás acciones abiertas ni sustituye el roadmap vigente.
