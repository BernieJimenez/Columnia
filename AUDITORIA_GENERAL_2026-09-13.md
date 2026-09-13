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
| `npm test -- --reporter=dot` | Aprobado: 43 archivos, 328 pruebas | Suite Vitest; no implica cobertura total ni validación nativa. |
| Pruebas dirigidas E02 | Aprobadas: 25 pruebas en `PreparePhase` y mapeo de esquema | No cubre el esquema de origen ausente en recetas v1. |
| `npm run docs:check` | Aprobado; ahora también ejecuta cuatro pruebas de contrato README/package.json | No valida instrucciones ejecutadas en otros sistemas. |
| `cargo test --manifest-path src-tauri/Cargo.toml --lib` | 416 aprobadas, 4 ignoradas; 0 fallidas | Cuatro ignoradas por selección/intencionalidad de sus pruebas. |
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
- [ ] **A03 · P2 · Propuesta · S — Elegir dos recorridos de negocio principales.** Valor: enfocar diseño y documentación. Candidatos para validar: limpiar una entrega periódica para BI y comparar dos versiones de un archivo. **Cierre:** cada recorrido tiene entrada sintética, resultado esperado, métrica de éxito y evidencia de necesidad en beta. **Base:** E01–E04.
- [ ] **A04 · P2 · Propuesta · M — Medir fricción después de cada mejora importante.** Valor: comprobar que una nueva interfaz ayuda. Reutilizar Gate 2 y las métricas existentes, sin telemetría. **Cierre:** comparación local con Gate 1 de ayuda requerida, retrocesos y finalización, sin cambiar las definiciones para favorecer el resultado. **Base:** E03–E04. **Depende de:** A01.

## B. Documentación y entrada al proyecto

- [x] **B01 · P1 · Confirmado · S — Corregir requisitos de Node/npm del README.** El README dice Node 22+, mientras `engines` exige Node `>=24.14.0 <25` y npm `>=11.10.1 <12`. Valor: evitar instalaciones incompatibles. **Cierre:** README ahora coincide con `package.json`; checker documental comprueba ambas versiones. **Base:** E01.
- [x] **B02 · P1 · Confirmado · S — Corregir `npm run check`.** El README lo recomendaba, pero no existe ese script. **Cierre:** la guía ahora usa `npm run build`, `npm test`, `npm run docs:check` y `npm run legal:check`, todos declarados en el manifiesto. **Base:** E01.
- [x] **B03 · P2 · Confirmado · S — Ampliar el checker documental para comandos y requisitos.** **Cierre:** el checker rechaza comandos npm inexistentes y requisitos Node/npm desalineados; `docs:check` ejecuta también cuatro regresiones del contrato README/package.json. **Base:** E01 y resultado de esta revisión. **Depende de:** B01–B02.
- [x] **B04 · P2 · Propuesta · S — Separar roadmap operativo del historial.** Valor: que una casilla histórica no se interprete como función ausente. **Cierre aplicado:** [`roadmap-current.md`](docs/reference/roadmap-current.md) lista ID, responsable funcional, dependencia y detalle histórico; los responsables siguen pendientes de asignación nominal. T6-05 ahora describe el round-trip SQL Server que falta y aclara que PostgreSQL/MariaDB ya tienen aceptación documentada. **Base:** E03.

## C. Cargar y comprender archivos

- [ ] **C01 · P2 · Propuesta · M — Preflight explicable de importación.** Valor: detectar hoja/encabezado/tipos incorrectos antes de trabajar. Integrar lo existente en un resumen de decisiones y advertencias. **Avance aplicado:** el diálogo Excel resume formato/tamaño, hojas, hoja elegida y modo de encabezados; aclara que el esquema y los tipos se revisan tras cargar. **Cierre pendiente:** probar ceros iniciales y encabezados ambiguos antes/después de importar para confirmar que se pueden revisar sin tocar el original. **Base:** E02; ampliar, no rehacer importadores.
- [x] **C02 · P2 · Verificar · M — Matriz de archivos difíciles.** Valor: evitar resultados plausibles pero incorrectos. **Cierre comprobado:** regresiones del lector cubren CSV con BOM, campos citados, delimitadores/saltos de línea y valores léxicos; UTF-8 inválido; hoja XLSX vacía con rechazo claro; fórmulas con valores cacheados; encabezados repetidos y fechas ambiguas. **Base:** E02, límite conocido de libros. Los fixtures son sintéticos y no sustituyen archivos reales diversos.
- [ ] **C03 · P2 · Propuesta · M — Perfil de interpretación reutilizable.** Valor: ahorrar ajustes en archivos periódicos. Reutilizar hoja, encabezado y convenciones de fechas/números con validación del nuevo esquema. **Cierre:** una variación de esquema exige revisar incompatibilidades; no aplica casts silenciosamente. **Base:** E02/E07. **Depende de:** C01; verificar primero qué opciones ya persisten.
- [ ] **C04 · P2 · Propuesta · M — Estimación previa de recursos.** Valor: anticipar operaciones costosas. Mostrar aproximación de RAM/disco y si el camino necesita materialización, aprovechando las guardias existentes. **Avance aplicado:** el preflight de Excel ya muestra tamaño en disco y la advertencia existente para libros comprimidos. **Cierre pendiente:** estimar RAM/disco y el camino de procesamiento para los formatos compatibles; cualquier estimación debe estar etiquetada y la admisión real seguir siendo autoritativa. **Base:** E02/E08.

## D. Revisar calidad y explicar resultados

- [ ] **D01 · P2 · Propuesta · M — Resumen accionable de problemas.** Valor: ayudar a decidir qué corregir primero. Conectar señales existentes con acciones, impacto y consecuencias; evitar un puntaje opaco de calidad. **Cierre:** desde un problema se llega a su explicación y acción adecuada; beta demuestra menor ayuda requerida. **Base:** E02/E06.
- [x] **D02 · P2 · Verificar · S — Etiquetas consistentes de cobertura.** Valor: distinguir un resultado completo de uno calculado sobre muestra. **Cierre verificado:** perfil, correlaciones, tendencias, SQL truncado y conteos exponen alcance/límites; el cambio de revisión cancela y descarta una agregación temporal pendiente. La regresión encontró y corrigió el texto concatenado `108de 120`. **Base:** E02/E07.
- [ ] **D03 · P2 · Propuesta · M — Comparación agregada antes/después.** Valor: mostrar si la limpieza mejoró los datos. Reutilizar revisiones e historial para comparar filas, nulos, tipos y reglas. **Cierre:** deltas ligados a dos revisiones explícitas; no comparar un perfil antiguo como si fuera actual ni persistir muestras. **Base:** E06–E07.
- [ ] **D04 · P2 · Propuesta · M — Flujo de excepciones de calidad.** Valor: trabajar con filas que incumplen una regla sin perder el conjunto válido. Empezar por conteos y filtros temporales; cualquier exportación de rechazados será explícita. **Cierre:** coincidencia exacta con la regla validada, límites de paginación y protección de datos equivalentes a la entrega. **Base:** E02/E06/E10; revisar contrato de privacidad antes de ampliar valores expuestos.
- [ ] **D05 · P3 · Propuesta · M — Diccionario de negocio editable.** Valor: explicar significado, unidad y uso esperado de una columna. Extender el diccionario de bundle existente. **Cierre:** descripción/unidad opcionales versionadas, comportamiento definido ante renombre/eliminación y exportación sanitizada según elección. **Base:** E02/E07; requiere ampliar el contrato durable.

## E. Preparar y reutilizar transformaciones

- [x] **E01 · P2 · Propuesta · M — Simulación de impacto de una receta.** Valor: revisar cambios antes de aplicarlos. **Cierre comprobado para cambios de alto impacto:** la confirmación captura la revisión del dataset, se descarta si esta cambia y exige volver a revisar los efectos agregados/advertencias; el handler también rechaza una confirmación obsoleta. Regresión UI cubre cambio de revisión y re-confirmación. Reutiliza undo existente. **Base:** E06/E08. La estimación sigue identificada como estimación, no como resultado ejecutado.
- [ ] **E02 · P2 · Propuesta · M — Reaplicar receta con mapeo de esquema.** Valor: reutilizar trabajo cuando cambian encabezados. **Avance aplicado:** el preflight señala columnas faltantes y requisitos semánticos, permite mapeo manual uno-a-uno compatible y nunca sustituye automáticamente. Distingue comparaciones numéricas/temporales y operaciones que convierten a texto; bloquea aplicar/guardar mientras haya referencias inválidas y mantiene un aviso si se cierra la revisión para corregir manualmente. Cancelar una carga nueva conserva el borrador actual. **Cierre pendiente:** recetas v1 no guardan el esquema de origen, por lo que no se detectan en general cambios de tipo en referencias cuyo uso admite varios tipos; definir y migrar metadatos de esquema para cubrir ese caso. **Base:** E02/E07.
- [ ] **E03 · P2 · Propuesta · M — Biblioteca local de recetas y contratos.** Valor: descubrir, probar y reutilizar lo ya guardable. **Cierre:** nombre, descripción, versión y ejemplo sintético; importación rechaza versiones incompatibles y no incluye datos privados de origen. **Base:** E02/E06/E07.
- [ ] **E04 · P3 · Propuesta · L — Catálogos de equivalencias revisables.** Valor: normalizar categorías repetidas, por ejemplo variantes de nombres de productos. **Cierre:** reutilizar reemplazos/joins existentes, preflight de claves duplicadas y no encontradas, historial reversible y cero coincidencias difusas aceptadas sin revisión. **Base:** E02/E06. **Condición:** necesidad repetida en beta.

## F. Entregar y comprobar el resultado

- [ ] **F01 · P1 para declarar soporte del destino · Pendiente vigente · M — Completar round-trip SQL Server.** El serializador booleano ya está implementado y hay evidencia histórica PostgreSQL/MariaDB. **Cierre:** `true/false/null` preservados al exportar y releer en SQL Server desde frame y fuente incremental; registrar driver y configuración sin credenciales. **Base:** E03, T6-05. **Dependencia externa:** instancia accesible; no bloquea uso puramente local.
- [ ] **F02 · P2 · Propuesta · M — Preflight del esquema remoto.** Valor: anticipar incompatibilidades antes de escribir. **Cierre:** diferencias de tipos, nulabilidad, longitud y política de tabla explicadas; parámetros siguen siendo seguros y MySQL `replace` permanece deshabilitado hasta demostrar sustitución atómica. **Base:** E06/E02.
- [ ] **F03 · P2 · Propuesta · M — Presets de entrega para flujos BI.** Valor: reducir errores de delimitador, fechas y decimales. **Cierre:** presets explícitos, sin prometer conectores nuevos, con relectura de archivos sintéticos y verificación manual en la herramienta destino elegida. **Base:** E01/E02. **Depende de:** A03.
- [x] **F04 · P2 · Propuesta · M — Resumen legible de entrega.** Valor: comunicar qué cambió y qué se validó a quien recibe el archivo. **Cierre aplicado:** el bundle incluye `delivery-summary.md` con dimensiones, conteos agregados de operaciones, cobertura/estado del contrato, versiones y SHA-256 de sus miembros; el manifiesto registra bytes y hash del resumen. Pruebas de ZIP materializado y source-backed verifican la integridad y omisión de muestras, valores sensibles y rutas. **Base:** E02/E07/E10.

## G. Proyectos y repetición de trabajo

- [ ] **G01 · P2 · Propuesta · L — Respaldo y restauración de proyecto.** Valor: trasladar o recuperar trabajo durable. Distinguirlo del bundle de entrega de datos. **Cierre:** paquete versionado con integridad, preflight de espacio, revisión del contenido, restauración transaccional y prueba en almacén vacío; no sobreescribir el original. **Base:** E07/E10.
- [ ] **G02 · P2 · Propuesta · M — Gestión del espacio de snapshots.** Valor: evitar crecimiento inesperado del disco. **Cierre:** tamaño agregado por proyecto, política de retención explícita y limpieza que nunca elimina revisiones referenciadas; recuperación verificada. **Base:** E07/E08; inventariar primero la limpieza existente.
- [ ] **G03 · P2 · Propuesta · L — Interfaz de lotes sobre la CLI existente.** Valor: repetir recetas sin escribir manifiestos manualmente. **Cierre:** preflight completo, límites actuales respetados, progreso por trabajo y explicación de éxitos parciales; usar el mismo motor. **Base:** E07. **Depende de:** E02/E03 y demanda beta.
- [ ] **G04 · P3 · Propuesta · L — Reanudación segura de lotes.** Valor: no repetir trabajos ya completados tras un fallo tardío. El batch actual se detiene y conserva salidas anteriores. **Cierre:** manifiesto de ejecución con hashes de receta/entrada/salida, invalidación por cambios y política explícita de colisión; nunca sobrescribir por deducción. **Base:** E07. **Depende de:** G03 o demanda CLI comprobada.

## H. Robustez y mantenimiento del motor

- [ ] **H01 · P2 · Confirmado como concentración de código · L — Extraer responsabilidades de `dataset.rs` gradualmente.** Se observaron más de 32.000 líneas no vacías en este archivo; el tamaño no demuestra un bug, pero encarece entender cambios. **Cierre:** separar una responsabilidad por cambio —lectores, perfilado, calidad o exportación— con contratos estables, paridad conductual y sin reescritura general. **Base:** E08.
- [ ] **H02 · P2 · Propuesta · M — Matriz ejecutable de caminos incrementales.** Valor: saber qué combinaciones evitan cargar todo en RAM. **Cierre:** formato × operación × privacidad × calidad con resultado esperado incremental/fallback/rechazo; pruebas de paridad para combinaciones críticas y matriz documental derivada. **Base:** E02/E08.
- [ ] **H03 · P2 · Verificar · L — Fallos de disco y cancelación durante publicación.** Valor: proteger trabajo ante interrupciones. **Avance aplicado:** pruebas existentes cubren fallo de E/S del snapshot y fallo antes de publicar metadatos; una regresión vuelve a abrir el proyecto anterior tras fallar una actualización y verifica datos y configuración. **Cierre pendiente:** inyectar disco lleno, permisos denegados y cancelación en puntos de commit; el guardado de proyecto aún no expone cancelación. Reutilizar pruebas existentes antes de agregar. **Base:** E07/E08.
- [ ] **H04 · P2 · Verificar · M — Escala más allá del escenario de referencia.** Valor: hacer promesas medibles. **Avance revisado:** el gate `perf:check` aprueba su baseline; ya existen escenarios fijos de 100 MiB (CSV de 4 columnas), JOIN de 512 MiB y un test separado de perfilado de 2 M de filas × 12 columnas de alta cardinalidad. **Cierre pendiente:** matriz comparable que varíe ancho, cardinalidad, texto y escala y mida tiempo/RAM/disco/cancelación y limpieza por camino. No aumentar presupuestos solo para aprobar. **Base:** E02/E08. Los 100 MiB/512 MiB existentes no prueban toda carga de varios GB.

## I. Accesibilidad, privacidad y distribución

- [ ] **I01 · P1 · Pendiente vigente · M — Aceptación manual nativa de accesibilidad.** Valor: completar tareas con teclado y lector real. **Cierre:** recorrido Cargar→Entregar en Windows con foco, modales, tablas, progreso, escalado y alto contraste; evidencia sobre la revisión vigente y correcciones de bloqueos. **Base:** E03/E09.
- [ ] **I02 · P2 · Propuesta · M — Paquete de diagnóstico local revisable.** Valor: facilitar soporte sin compartir datasets. **Cierre:** persona revisa y exporta voluntariamente versión, códigos de error, fase y métricas agregadas; exclusión comprobada de rutas, consultas, credenciales y valores. **Base:** E10; reutilizar sanitización existente.
- [ ] **I03 · P1 únicamente para binarios · Pendiente vigente · M — Resolver la decisión de distribución correspondiente.** Valor: mantener coherencia entre publicación y estado aprobado. **Cierre:** responsable registra decisión específica para instaladores/updater y pasa el gate correspondiente; no tratar la aprobación de código fuente como autorización binaria. **Base:** E05. Esta auditoría no modifica esa decisión ni recomienda requisitos jurídicos nuevos.
- [ ] **I04 · P1 únicamente para release · Pendiente vigente · L — Validar el artefacto final de instalación/actualización.** Valor: que el producto instalado funcione fuera del entorno de desarrollo. **Cierre:** candidato de commit limpio, instalación/primera apertura/segunda instancia, actualización fallida, firma inválida y recuperación verificados según checklist; hashes y evidencias ligados al artefacto exacto. **Base:** E03/E05/E09. **Depende de:** I03 para publicar; no reconstruir el updater existente.
- [ ] **I05 · P3 · Pendiente de plataforma · L — Validar macOS/Linux solo si hay demanda.** Valor: ampliar usuarios con soporte real. **Cierre:** build, selectores, archivos, proyectos, accesibilidad y distribución verificados localmente en cada sistema antes de anunciarlo. **Base:** E01–E03. No es requisito para la beta Windows.
- [ ] **I06 · P2 · Propuesta · S — Alinear perfil público con hitos demostrados.** Valor: comunicar claramente qué puede probar alguien hoy. **Avance aplicado:** el README del perfil ahora aclara que la beta con participantes sigue pendiente, que solo se verificó automatización en Windows y que no hay aceptación manual, soporte macOS/Linux, instaladores/updater o resultados de beta publicados. **Cierre pendiente:** verificar enlaces en el sitio publicado y mantener alineados ambos README tras futuras validaciones. **Base:** README del perfil revisado y E01. No se comprobaron enlaces remotos en esta auditoría.

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

El resultado de esta revisión es un backlog de 39 acciones, además del inventario de capacidades existentes. B01–B04, C02, D02, E01 y F04 están cerradas con las evidencias indicadas. C01, C04, E02, H03 e I06 avanzaron parcialmente, pero siguen abiertas por los criterios que se detallan en sus casillas; H04 conserva la matriz de escala pendiente. El informe no da por implementadas las demás acciones abiertas ni sustituye el roadmap vigente.
