# Changelog

Todos los cambios visibles de Columnia se registran aquí. Las versiones siguen
SemVer y el estado real del prototipo se contrasta con el código, los tests y
los artefactos de validación locales.

## [Unreleased]

### Añadido

- Preparar detecta outliers numéricos mediante IQR y permite reemplazarlos de
  forma reversible por la mediana observada, conservando tipos `Int64`/`Float64`,
  `_cambios` y un impacto agregado. La misma regla está disponible en recetas
  como `impute`; el inventario IPC pasa a 60 comandos de producción.
- Preparar ofrece además una imputación categórica explícita y reversible que
  completa nulos textuales como `Desconocido`, sin tocar números ni `_cambios`,
  con impacto agregado; el inventario IPC actual pasa a 61 comandos de producción.
- Preparar ofrece acciones IQR confirmables para limitar valores atípicos o
  eliminar las filas que excedan los límites, con historial reversible, impacto
  agregado y sin mostrar celdas; el inventario IPC pasa a 63 comandos de producción.
- Preparar ofrece una acción confirmable para proteger valores no nulos de
  correo, teléfono, dirección y nombre detectados, sustituyéndolos por
  `[REDACTED]` sin tocar identificadores, números, nulos ni `_cambios`; conserva
  las columnas, reporta solo conteos agregados y permite deshacer desde el
  historial. El inventario IPC pasa a 64 comandos de producción y 58 estructuras.
- Preparar ofrece una acción reversible para interpretar columnas de texto con
  fechas detectadas: solo usa un formato dominante cerrado, omite columnas
  ambiguas y publica únicamente el impacto agregado. El inventario IPC pasa a
  65 comandos de producción.
- Preparar ofrece una acción reversible para convertir texto numérico con más
  de 90% de coincidencia: rechaza pérdida de precisión y conserva
  identificadores y códigos con ceros iniciales. El inventario IPC pasa a 66
  comandos de producción.
- La cobertura M1 añade un round-trip de sesión contra un libro `.xlsx` real:
  genera el libro con el exportador nativo, valida la hoja registrada, importa
  la receta, reabre el proyecto y comprueba esquema, conteos y etapa activa.
- La importación M1 conserva hasta cinco entradas de historial de ejecución solo
  cuando contienen estado, duración y filas agregadas; asigna IDs locales y no
  copia consultas, rutas, valores ni entradas inválidas.
- La importación de sesiones DataPrep reproduce `drop_duplicates`,
   `drop_fuzzy_duplicates`,
   `drop_high_null_cols`, `drop_id_cols`, `drop_empty_cols`, `drop_constant_cols`,
   `drop_empty_rows`, `normalize_sentinels`, `impute_numeric`, `impute_categorical`, `parse_dates`, `trim_text`, `fix_encoding`, `cast_numeric`, `cap_outliers`, `impute_outliers` y `drop_outliers`,
   `normalize_text`, `normalize_booleans`, `mask_pii`, `normalize_columns` y `add_cambios_col` durante el fallback a la fuente
   cuando no hay snapshot compatible, respetando el orden fijo de limpieza; un snapshot disponible
   conserva prioridad para evitar reaplicar operaciones sobre un estado
   materializado.
- El replay M1 rechaza antes de publicar una sesión que combine las estrategias
  IQR incompatibles `cap_outliers`, `impute_outliers` y `drop_outliers`, igualando
  la exclusividad del catálogo DataPrep.
- El catálogo DataPrep `selected_cleaning_operations` se migra por aliases canónicos:
  las limpiezas deterministas, incluido `mask_pii` en su modo `mask` predeterminado,
  se reproducen desde la fuente y se conservan en el informe de sesión. Los modos
  hash/clave explícita no se inventan y las operaciones sin equivalente quedan como
  advertencias accionables, sin descartarse silenciosamente.
- La importación de sesiones DataPrep convierte etiquetas de etapa conocidas
  (`Cargar`, `Revisar`, `Preparar`, `Entregar` y equivalentes de análisis,
  transformación o exportación) en la etapa activa del workspace; etiquetas
  desconocidas conservan el fallback seguro a Revisar.
- El preflight de sesiones DataPrep clasifica bloques reconocibles de resultados,
  historial y cachés como artefactos no portables, conservando solo sus categorías
  sanitizadas y una acción manual; no copia contenido ni rutas de esos artefactos.
- El probe WebView2 atribuye memoria por proceso y fase, y limita el perfil y el
  cleanup a procesos pertenecientes al `Job Object`; así evita contar o terminar
  descendientes externos con un `ParentProcessId` coincidente.
- Benchmark reproducible de datasets grandes dentro de WebView2: `perf:webview2`
  genera un CSV temporal de 100 MiB y verifica selección nativa, carga,
  paginación, transformación y exportación con evidencia agregada de duración,
  memoria y cleanup. `perf:check` y `verify:tier` exigen este recorrido además
  del benchmark CLI.
- La migración de sesiones DataPrep conserva en el artefacto de receta los
  identificadores estructurales acotados de operaciones aplicadas y comprobaciones
  de análisis, además de sus conteos; no guarda resultados, cachés ni rutas.
- Cargar ofrece dos datasets de ejemplo locales para explorar señales de calidad
  y series temporales sin descargar datos ni exponer rutas; la selección usa un
  identificador opaco y el inventario IPC queda en 68 comandos de producción y
  59 estructuras.
- La migración de sesiones DataPrep conserva metadatos agregados de muestras de
  análisis —estado muestreado y conteos de filas— cuando están disponibles, sin
  copiar filas, valores ni resultados originales; el informe los muestra como
  contexto de compatibilidad.
- La ruta lazy de recetas compatibles y los lectores CSV/TSV/TXT delimitados y
  Parquet comparten una colección Polars con el motor `streaming`; así las
  operaciones de carga y transformación compatibles no vuelven a una
  colección eager silenciosa, mientras las recetas no compatibles conservan su
  fallback eager explícito.
- La apertura y validación de snapshots Parquet de proyectos y del historial
  temporal reutiliza la misma frontera streaming de baja memoria, sin cambiar
  el formato durable ni exponer rutas; el `DataFrame` activo continúa siendo
  materializado para preservar el contrato actual de sesión.
- La restauración del historial ya valida y copia cada snapshot de forma
  incremental: conserva en memoria solo el frame del cursor mientras procesa
  las entradas restantes una por una, reduciendo el pico de RAM al reabrir
  proyectos con varias revisiones.
- La comparación por filas y claves fusiona firmas en bloques acotados y evita
  conjuntos auxiliares duplicados durante el resumen de claves; conserva los
  conteos exactos, el orden estable de los conflictos y los límites actuales
  de JOIN/comparación.
- La importación de sesiones DataPrep prioriza un snapshot local compatible para
  conservar el estado materializado exacto; solo reaplica la receta sobre el
  origen cuando no existe snapshot, dejando explícito el límite de paridad de
  operaciones cuyos parámetros no están en el manifiesto.
- La fixture de round-trip de sesiones cubre fuente, hoja/etapa, operaciones
  deterministas, reglas y análisis/historial/caché no portables; la prueba importa
  y reabre el proyecto sin copiar valores privados ni artefactos operativos.
- Preparar detecta secuencias comunes de doble codificación UTF-8 (`Ã©`, `â€™`)
  por columna y ofrece una reparación reversible, limitada a valores de texto
  que pueden decodificarse inequívocamente sin tocar números, `_cambios` ni
  valores ambiguos.
- Preparar permite confirmar y apartar como nulos los valores de texto que no
  coincidan con una sugerencia semántica con al menos 90% de confianza; no muestra
  celdas, conserva tipos no textuales y ofrece reversión desde el historial.
- La importación de sesiones DataPrep recalcula y persiste el perfil agregado
  del dataset importado antes de publicar el proyecto, para que Revisar abra
  con una caché de calidad válida sin conservar filas, celdas, rutas ni muestras.
- Los proyectos con perfil cacheado guardan la huella SHA-256 del
   `current.parquet` durable y descartan automáticamente solo ese perfil si el
   snapshot cambia al reabrir; los catálogos anteriores siguen siendo legibles y
   se actualizan al próximo guardado mediante la migración SQLite v6.
- El workspace durable conserva la última vista de Revisar (`diagnosis` o
  `preview`) y la restaura al abrir el proyecto; los catálogos anteriores usan
  Diagnóstico por defecto y las vistas inválidas se rechazan sin reemplazar la
  sesión activa. También conserva el desplazamiento de la página visible de la
  muestra, normaliza offsets inválidos y vuelve a la primera página si la página
  guardada ya no está disponible. La migración SQLite pasa a v7.
- La cobertura crítica por capa vuelve a pasar con `npm run test:coverage`:
  `App`, `DeliveryPhase`, `PreparePhase`, `usePrepareController` y
  `useProjectsController` mantienen sus umbrales propios, incluyendo las ramas
  de confirmación y desmarcado de correcciones de Preparar.
- El workspace durable conserva también la etapa activa del flujo (`load`,
  `review`, `prepare` o `deliver`) y la restaura al abrir proyectos; los
  catálogos anteriores vuelven a Revisar y las etapas desconocidas se rechazan
  sin sustituir la sesión activa. La migración SQLite pasa a v8.
- Updater autenticado de Tauri 2 con consulta explícita, metadatos visibles,
  descarga con progreso/cancelación, instalación nativa y clave pública
  embebida; la frontera Rust rechaza versiones semver iguales, anteriores o
  inválidas; el flujo firmado genera `.sig`, manifiesto estático e inventario
  SHA-256 sin guardar la clave privada en el repositorio.
- Contrato reproducible `updater:contract:test` para el gate Release/Package:
  genera una clave Ed25519 efímera, valida criptográficamente el par válido y
  confirma fallo cerrado ante artefacto truncado, firma alterada, manifiesto
  incompleto/corrupto y URL HTTP. Este fixture no reemplaza el ejercicio contra
  un canal real.
- `release:updater:dry-run` amplía el orquestador de release con configuración
  temporal de endpoint, firma local, SBOM, gates completos y verificación
  fail-closed de instalador/firma/manifiesto.
- El orquestador de release reutiliza el binario `Release/Package` para capturar
  la evidencia visual, evitando reconstruir un ejecutable distinto después del
  gate de empaquetado.
- Los gates de release, SBOM e instalador calculan sus fingerprints SHA-256 con
  la API criptográfica del runtime, manteniendo el flujo compatible con los
  hosts PowerShell usados por npm y por la ejecución directa.
- El perfil `Release` incorpora y aprueba el contrato updater junto con sus
  gates de documentación, IPC, toolchains, cobertura, supply chain, instalador,
  SBOM y binario Tauri sin bundle; la corrida local no implica un árbol limpio
  ni autoriza publicación.
- Cargar admite arrastrar un dataset a la ventana sin entregar su ruta a React
  y mantiene hasta cinco referencias recientes sanitizadas que vuelven a abrir
  el selector nativo.
- Revisar incorpora actividad SQL agregada para las últimas cinco ejecuciones y
  la conserva en el workspace del proyecto al guardarlo; se restauran estado,
  duración y filas, nunca la consulta, rutas ni valores. También mantiene la
  tendencia temporal diaria para rangos cortos, con días vacíos y tabla accesible
  equivalente.
- Entregar ofrece abrir la carpeta del último output local después de una
  exportación exitosa; Rust conserva y revalida temporalmente el destino, usa el
  explorador nativo y no expone la ruta a React.
- Preparar permite retirar columnas identificadoras de forma explícita,
  confirmada y reversible, sin publicar sus valores.
- La CLI sanitiza reportes, recetas y manifiestos en una frontera común, y las
  consultas/joins locales incorporan cancelación y preflight de cardinalidad.
- El smoke CLI valida la redacción de nombres de archivo en stdout sin dejar de
  comprobar que las salidas locales se publiquen completas y de forma atómica.
- Los manifiestos de sesión DataPrep importados conservan un resumen estructural
  sanitizado en el informe de migración: hoja, etapa, conteos de operaciones,
  reglas y análisis, además de señales booleanas para referencias de origen y
  snapshot. No se restauran sesiones ni se escriben proyectos automáticamente.
- Contrato de calidad versionado `columnia-quality-rules` v1, con guardado
  atómico, importación de Columnia/DataPrep v1–v3 y compatibilidad con el
  documento legado v1; versiones futuras y formatos ambiguos fallan cerrados.
- Selector nativo Win32 estabilizado para Abrir/Guardar como, con soporte de
  editores `1148`/`1001`, fallback de UI Automation/Win32/Unicode y entrada al
  gate `verify:tier` mediante `npm run smoke:native-selectors`; el driver filtra
  ventanas por PID/owner, espera el cierre del modal y corta procesos bloqueados.
- Entregar permite importar y guardar contratos mediante diálogos nativos,
  muestra el origen/versión y mantiene las rutas fuera de React; la CLI acepta
  el formato canónico y el legado.
- Fase I1 completa: recetas compatibles con renombres, casts, filtros y
  columnas calculadas ejecutadas mediante planes Polars lazy, con fallback eager
  atómico para operaciones que requieren validaciones específicas.
- Monitor compacto de consumo en el lateral, con CPU/RAM del proceso y del
  equipo, actualización nativa periódica y estado accesible para el shell web.
- Benchmark reproducible de 100 MiB contra `dataprepv1.1`, con comparación de
  duración, working set, conteos, fixture sintética y cleanup sin conservar datos.
- Gate de cobertura V8 global para `src` (80% statements/lines, 75% branches y
  functions), más umbrales por capa crítica para App, Entrega, Preparar y su
  controller; la suite frontend actual tiene 267 tests.
- Supply chain local con `npm audit`, `cargo-audit`, `cargo-deny`, secret scan,
  inventario reproducible de `THIRD_PARTY_NOTICES` y verificación de red sin
  telemetría.
- Contrato de instalador NSIS `currentUser`, recursos MIT/third-party notices y
  política WebView2 `downloadBootstrapper`; Polars actualizado a `0.55.2`.
- Primera entrega de paridad funcional: exportación JSON atómica en UI, Rust,
  CLI, batch y proyectos, con matriz comparativa frente a `dataprepv1.1`.
- Segunda entrega de paridad funcional: comparación local de dos datasets,
  diferencias multivaluadas de filas/columnas y consolidación opt-in con
  historial cuando el esquema es compatible.
- Tercera entrega de paridad funcional: exportación SQL como script portable y
  atómico en Entregar, CLI, batch y proyectos, con escape de identificadores,
  valores y transacción explícita.
- Cuarta entrega de paridad funcional: visualizaciones compactas y accesibles de
  completitud y posibles outliers en Diagnóstico, con valores exactos y tablas
  equivalentes para lector de pantalla.
- Quinta entrega de paridad funcional: comparación opcional por claves explícitas,
  detección de duplicados/conflictos y consolidación segura de claves nuevas,
  manteniendo el dataset activo hasta confirmar la operación.
- Sexta entrega de paridad funcional: joins locales `Inner`, `Left` y `Full` por
  claves explícitas, con validación de tipos, columnas compartidas sufijadas,
  historial e invalidación de perfil/entrega.
- Benchmark CLI de 256 MiB con 2,220,032 filas, tres transformaciones sostenidas
  y dos actualizaciones durables; el flujo pasa, pero su working set máximo es
  aproximadamente 1.12 GiB y queda fuera del presupuesto de 512 MiB.
- Tier 5 endurece la automatización batch: las salidas quedan confinadas al
  `outputRoot` del manifiesto por defecto y `--force` es obligatorio para
  destinos externos o existentes.
- Tier 5 añade inventario IPC generado desde `generate_handler!`, contrato de
  58 comandos de producción, 4 debug y 56 estructuras compartidas, toolchains
  exactas Node/npm/Rust, notices offline sin `UNKNOWN` y una revisión legal de
  distribución pendiente de completar por canal/jurisdicción.
- Tier 5 hace durable la recuperación del catálogo: los fallos de inicialización
  se pueden reintentar sin reiniciar y las generaciones huérfanas antiguas se
  reconcilian sin tocar las activas. El perfilado de duplicados normalizados
  incorpora un fast-path ASCII; la evidencia corta de 100 MiB registra
  `project-save` en 59.75 s y su actualización en 58.84 s.
- Tier 5 abre el primer límite modular del motor Rust en
  `dataset_fingerprints.rs`, con API interna acotada y cobertura equivalente
  para duplicados exactos y parecidos; quality/recipe/export/persistence
  conservan extracciones posteriores como trabajo incremental.
- Tier 5 incorpora `tools/release.ps1` y `npm run release:dry-run` para
  orquestar los gates locales con rama y árbol limpios; el flujo no etiqueta,
  publica ni contacta servicios remotos.
- Tier 5 liga la evidencia visual de release a un commit limpio: el baseline
  versionado conserva commit, rama, hashes de `package-lock.json` y `Cargo.lock`,
  además de los cinco escenarios desktop, móvil, zoom 125%, zoom 200% y
  forced-colors. El checker solo tolera el commit posterior que modifica
  exclusivamente el propio baseline.
- Smoke reproducible del instalador NSIS con usuario sin privilegios, ruta
  Unicode/con espacios, primera apertura, segunda invocación con instancia única,
  desinstalación y retención controlada de datos de usuario; `Package` lo ejecuta
  después de construir e inventariar el bundle.
- El smoke del instalador acepta un NSIS anterior explícito para probar un
  upgrade real en la misma ruta, con comprobación de versión y supervivencia de
  datos antes de la desinstalación.
- Política ejecutable de rotación/recuperación del updater con release puente
  firmada por la clave anterior, preservación de la versión instalada ante fallo
  y gate de fingerprint; `updater:verify-published` vuelve a descargar el
  manifiesto/instalador HTTPS y verifica tamaño, SHA-256 y firma minisign. Ante
  compromiso de la clave, el contrato congela el canal y prohíbe firmar otra
  release puente con la clave comprometida.

### Validación histórica de la reauditoría

- En la reauditoría del 2026-08-28 aprobaron 234 pruebas Rust, 248 frontend y 9
  E2E, además de build Vite, contratos IPC, cobertura y los perfiles Full,
  Release y Package; Package produjo MSI y NSIS para `137520b`.
- Las capturas visuales web y release frescas cumplieron sus contratos. Los
  recorridos CDP/selectores fueron funcionales, pero excedieron memoria; el gate
  de rendimiento y la comparación del baseline visual release quedaron fallidos
  y trazados en Tier 5.

### Validación de la implementación Tier 5

- `tools/check.ps1 -Profile Full` y `tools/check.ps1 -Profile Release` pasan:
  build, cobertura, clippy, supply chain, SBOM, instalador, 267 tests frontend
  y 244 tests Rust.
- El benchmark formal final de tres actualizaciones durables pasa en 100 MiB:
  `project-save` en 52.09 s y sus tres actualizaciones en 56.37–57.31 s, con
  reapertura, exportación y cleanup confirmados. Evidencia:
  `.local/validation/performance-benchmark/20260828T184531Z/summary.json`.
- La medición nativa de memoria privada WebView2 ya pasa en el smoke aislado:
  521.79 MiB de working set, 265.98 MiB privados y cleanup confirmado, con los
  cuatro diálogos Win32 aprobados. La captura release desde un commit limpio y
  la revisión visual del baseline ya están aprobadas; la revisión legal final
  y la validación del canal siguen siendo requisitos de publicación.
- El probe CDP funcional de ProjectsPanel midió 470.25 MiB de working set y
  253.48 MiB privados, dentro de presupuesto, ejecutó 3 ciclos sostenidos y
  confirmó cleanup. El smoke nativo aislado aprobó abrir dataset, guardar/cargar
  receta y exportar sin exponer rutas; Playwright y selectores se ejecutan como
  gates separados para no mezclar sus perfiles de memoria.
- La nueva corrida estricta de `smoke:cdp` aprobó Playwright, ProjectsPanel,
  mutaciones IPC, persistencia, reapertura y cleanup, con 501,563,392 bytes de
  working set dentro de 512 MiB, pero 272,379,904 bytes de memoria privada sobre
  el límite de 256 MiB. Las corridas diagnósticas previas quedaron en 256.06–258.06
  MiB sin crecimiento monotónico; el gate privado estable sigue pendiente y no se
  presenta esta señal como una fuga confirmada. Evidencia:
  `.local/validation/webview2-cdp/20260829T023923Z/summary.json`.
- Las pruebas Rust del updater cubren versiones estables, downgrade, igualdad,
  prerelease e inputs inválidos; el ejercicio contra un canal real sigue siendo
  una validación de I6 pendiente.
- `release:dry-run` valida el preflight de distribución y exige rama/árbol
  limpios; la captura release aprobada queda ligada al commit de evidencia y el
  baseline solo se guarda en un commit posterior exclusivo de ese archivo.
- El smoke NSIS local pasó desde un usuario no administrador: instalación en
  5.3 s, primera ventana en 711 ms, segunda invocación sin proceso duplicado,
  desinstalación en 1.3 s y sentinel de datos de usuario conservado durante la
  desinstalación y limpiado después. La VM limpia, el canal real y la
  aceptación legal siguen abiertos.
- La rotación de claves ya tiene contrato versionado y gate local; la descarga
  posterior a publicación tiene verificador criptográfico, pero no se ejecutó
  contra un canal real porque todavía no existe un canal público configurado.
- Accesibilidad visual (desktop/móvil, zoom 125%/200% y forced-colors) y
  `perf:check` pasan con la evidencia renovada.

### Interno

- Reauditoría profesional exhaustiva sobre `137520b`: se añadió
  `AUDITORIA_PROFESIONAL_2026-08-28.md` y se abrió Tier 5 con 20 tareas
  trazables (`T5-01`–`T5-20`). No se modificó comportamiento del producto.
- `Full`, `Release` y `Package` aprobaron; Package produjo MSI y NSIS ligados al
  commit auditado. Los gates de rendimiento y baseline visual release fallaron
  y se conservaron como fallos, sin elevar presupuestos ni aprobar hashes.
- El estado operativo posterior se documenta en la sección de validación de la
  implementación Tier 5; el texto de la reauditoría anterior se conserva como
  histórico del commit auditado.

## [0.57.0] - 2026-08-24

### Añadido

- Inventario y fixtures sintéticas para pipelines, sesiones, reglas de calidad
  y recetas legacy DataPrep, declaradas en `fixtures/manifest.json`.
- La importación de manifiestos de sesión reconoce origen, snapshot, hoja,
  etapa, operaciones aplicadas, calidad y análisis, y los publica como warnings
  sanitizados sin afirmar una restauración automática.

### Validación

- 183 pruebas Rust, 211 pruebas frontend, build Vite, contrato IPC, formato
  Rust y Clippy estricto aprobados.
- Documentación, gobernanza y baseline de rendimiento aprobados; evidencia
  release desktop/móvil/zoom 125%/`forced-colors` en
  `.local/validation/release-evidence/20260825T023814Z` y puerta visual
  confirmada en `.local/validation/release-evidence-check/20260825T023954Z`.

## [0.56.0] - 2026-08-24

### Añadido

- La migración de pipelines DataPrep v1–v3 conserva las opciones de entrega
  compatibles: formatos locales, columnas seleccionadas y privacidad.
- `xlsx` se normaliza a `excel`; reportes, CSV/ZIP y parámetros SQL sin
  equivalente generan warnings estructurados en el informe de migración.
- Preparar muestra el informe con conversiones, omisiones, acciones manuales y
  SHA-256 del artefacto, y conserva sus metadatos al guardar la receta.

### Validación

- 181 pruebas Rust, 211 pruebas frontend, build Vite, contrato IPC, formato
  Rust y Clippy estricto aprobados.
- Documentación, gobernanza y baseline de rendimiento aprobados; evidencia
  release desktop/móvil/zoom 125%/`forced-colors` en
  `.local/validation/release-evidence/20260825T022756Z` y puerta visual
  confirmada en `.local/validation/release-evidence-check/20260825T023005Z`.

## [0.55.0] - 2026-08-24

### Añadido

- La importación de contratos de calidad DataPrep ahora entrega un informe
  estructurado con total de entradas, reglas convertidas, omisiones,
  advertencias, acciones manuales y SHA-256 del artefacto original.
- Entregar muestra el resumen del informe y las acciones recomendadas sin
  publicar rutas, filas ni valores del dataset.

### Validación

- 181 pruebas Rust, 209 frontend, build Vite, formato Rust, Clippy estricto,
  documentación y gobernanza aprobados para la vertical de migración.

## [0.54.0] - 2026-08-24

### Mejorado

- El arranque frontend carga inicialmente solo el shell, Cargar y Proyectos;
  Review, Preparar y Entregar se separan en chunks y se precargan al enfocar o
  pasar el cursor por su etapa.
- Tauri ya puede mostrar la ventana y el shell local sin esperar la consulta
  secundaria de información de versión/plataforma.
- La preparación del catálogo SQLite de proyectos se difiere hasta la primera
  operación de proyecto; se conservan la migración segura y la inicialización
  eager para CLI y automatizaciones.
- El monitor de consumo se inicia después del primer paint para no competir con
  la apertura de la interfaz.

### Validación

- 181 pruebas Rust, 209 frontend, build Vite, Clippy estricto, formato Rust,
  smoke desktop, smoke CDP, resumen y baseline de rendimiento aprobados.
- El bundle inicial bajó de 359.93 a 253.10 KB raw y de 100.26 a 77.31 KB gzip.
  El smoke debug continúa condicionado por la compilación nativa de desarrollo.

## [0.53.0] - 2026-08-24

### Añadido

- Importación segura de recetas JSON DataPrep v1–v3 desde el selector nativo de
  Preparar: renombres, casts, fechas, filtros, reemplazos literales, columnas
  conservadas, cálculos, split/merge, outliers, grupos, contactos y extracciones
  se normalizan a una receta Columnia v1.
- Rechazo explícito de expresiones regulares, booleanos personalizados y
  operaciones sin equivalente para evitar perder semántica durante la
  migración.
- Inventario de migración documentado y anuncio accesible de la etapa activa.

### Validación

- 180 pruebas Rust, 209 frontend, build Vite, formato Rust, documentación y
  gobernanza aprobados.

## [0.52.0] - 2026-08-24

### Añadido

- Selector visible de apariencia en la barra lateral con los modos `Sistema`,
  `Claro` y `Oscuro`, botones con estado accesible y persistencia local segura.
- Aplicación temprana del tema antes de montar React para evitar saltos visuales,
  con overrides explícitos para que `Claro` y `Oscuro` funcionen aunque el modo
  del sistema sea el contrario.
- Refinamiento visual del shell: panel de apariencia, jerarquía de navegación,
  fondos con profundidad, tarjetas redondeadas y estados de interacción más
  distinguibles.

### Validación

- 209 pruebas frontend, build Vite y prueba de sincronización entre npm, Cargo,
  Cargo.lock, package-lock y Tauri.

## [0.51.0] - 2026-08-24

### Añadido

- Paginación de conflictos por clave en bloques de 50, con índices globales,
  navegación accesible y bloqueo de la resolución hasta completar todas las
  páginas.
- Resolución completa de conflictos fuera del primer preview mediante una
  validación backend del conjunto total, manteniendo historial reversible y
  rechazo de decisiones repetidas o incompletas.
- Versionado sincronizado `0.51.0` en npm, Cargo, Cargo.lock, package-lock y
  configuración Tauri.

### Validación

- 177 pruebas Rust y 204 Vitest, build Vite, Clippy estricto, formato,
  documentación, gobernanza, diff limpio, smoke CLI y smoke WebView2 con
  selectores nativos Win32 aprobados. Evidencia: `.local/validation/cli-smoke/20260825T002214Z`
  y `.local/validation/webview2-cdp/20260825T002400Z`.

## [0.50.0] - 2026-08-24

### Añadido

- Resolución independiente por columna/valor para conflictos visibles por clave,
  con cobertura obligatoria de cada celda, historial reversible y compatibilidad
  con decisiones legacy por fila.
- Privacidad visible en los seis destinos locales actuales: máscara/hash para
  señales de correo, teléfono, dirección, nombre e identificadores, incluidos
  identificadores numéricos, con conteo/nombres protegidos en el resultado sin
  exponer valores.
- Versionado sincronizado `0.50.0` en npm, Cargo, Cargo.lock, package-lock y
  configuración Tauri.

### Validación

- 177 pruebas Rust y 202 pruebas Vitest pasan, junto con build Vite, formato,
  Clippy estricto, documentación, gobernanza, diff limpio, smoke CLI y smoke
  WebView2 con selectores nativos Win32. Evidencia: `.local/validation/cli-smoke/20260825T000531Z`
  y `.local/validation/webview2-cdp/20260825T000531Z`.

## [0.49.0] - 2026-08-23

### Añadido

- Catálogo durable de proyectos SQLite v3 con snapshots Parquet, recuperación
  explícita, reglas de calidad, receta y cursor de historial.
- CLI local con `inspect`, `transform`, `validate`, `batch` y operaciones de
  proyectos con contratos JSON v1.
- Evidencia local de accesibilidad, rendimiento, SBOM y empaquetado Windows.
- Documentación Diátaxis, ADRs, política de contribución y política de fixtures
  sintéticas.

### Validación

- 132 pruebas frontend Vitest.
- 127 pruebas Rust.
- Gates locales `Fast`, `Full`, `Release` y `Package` disponibles; los smokes
  que necesitan Windows interactivo conservan evidencia bajo `.local/`.

### Corregido

- Los controles de la interfaz conservan texto legible y foco visible bajo
  `forced-colors: active`, incluidos los controles deshabilitados.

### Limitaciones conocidas

- El dataset se materializa en memoria y la ejecución de recetas todavía no es
  lazy/incremental.
- La auditoría manual con lector de pantalla y High Contrast sigue separada de
  los gates automáticos.
- El release actual es un prototipo local; updater, publicación y firma del
  artefacto son fases posteriores.
