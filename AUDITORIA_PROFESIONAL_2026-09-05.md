# Reauditoría profesional de Columnia — escritorio

**Fecha de cierre:** 2026-09-05. **Inicio:** 2026-09-04.  
**Base:** `7d837819c66d22d8c2c51f12cac84a8b8014206d` (`master`), versión `0.167.0`.  
**Dictamen:** requiere correcciones de integridad y entrega antes de considerarlo listo para uso fiable general o distribución pública. No se confirmó un hallazgo crítico; hay cuatro fallos de prioridad alta en producto y un bloqueo legal conocido de distribución.

## 0. Alcance acordado

Revisión exhaustiva de las doce áreas aplicables de la guía del usuario, en español, centrada en la aplicación de escritorio Windows x64: React/TypeScript, Tauri/Rust, persistencia SQLite/Parquet, motores Polars/DuckDB y entrega ODBC. El usuario confirmó: «nada de cli, workflow, github action, puedes empezar».

- **Excluidos:** CLI del producto, automatización batch, workflows locales o remotos, GitHub Actions y SEO. No se proponen nuevas tareas en esas superficies. Las herramientas de terminal usadas para compilar y comprobar la aplicación son instrumentos de auditoría, no revisión de la CLI de Columnia.
- Dependencias vendorizadas, generados y compilados no se revisan como código propio. Lockfiles, avisos y evidencias sí se consultan. macOS/Linux quedan sin certificación de soporte.
- Se revisaron los antecedentes `ROADMAP.md`, `CHANGELOG.md`, `CONTEXTO.md` y la auditoría de agosto; no se reabren decisiones de stack, ausencia de CI, pagos obligatorios ni compatibilidad externa retirada.
- Usuario principal, escala de producción, canal y jurisdicción no están definidos. No se infieren obligaciones normativas. Lo dependiente de estos datos queda pendiente de revisión legal.
- **Límite de cambios:** informe y fusión documental preparada para revisión, sin cambios de código de producto, pruebas permanentes, dependencias ni baselines. Los sondeos adicionales se guardan bajo `.local/` y usan datos sintéticos.
- Las ubicaciones de hallazgos se refieren al código y documentos del commit base; las inserciones documentales posteriores pueden desplazar líneas. Se consultó CodeGraph primero; sus rangos desfasados se contrastaron con la fuente real.

## 1. Resumen ejecutivo

Columnia dispone de una base técnica importante: tipos estrictos, fronteras locales, snapshots, exportaciones protegidas y numerosas pruebas.
Las 274 pruebas frontend y las 354 pruebas Rust seleccionadas aprobaron, así como nueve E2E del shell.
Sin embargo, los sondeos añadidos para esta revisión reprodujeron cinco fallos: uno de integridad de datos y cuatro aserciones de interfaz.
El caso más grave deja cero filas activas al activar trazabilidad después de un JOIN, y guardar ese JOIN también falla.
La entrega ODBC contiene un desacuerdo entre el motor seleccionado y el enviado, además de defectos de serialización y sustitución de tablas.
La cobertura crítica de proyectos y la memoria privada de la experiencia básica incumplen sus presupuestos aunque los recorridos funcionales aprueben.
La revisión visual detectó un enlace de salto ilegible en colores forzados; las guías y textos de privacidad no reflejan completamente ODBC y el procesamiento incremental.
Las decisiones legales siguen pendientes y se conservan bajo sus IDs existentes.

### Cinco prioridades

| Orden | Hallazgo | Acción inmediata |
| --- | --- | --- |
| 1 | A-01: JOIN → estado vacío | Corregir la invariante de almacenamiento antes de continuar mutaciones/guardado. |
| 2 | A-03: valores MySQL concatenados | Parametrizar los datos y verificar fidelidad/aislamiento. |
| 3 | A-04: Replace MySQL | Impedir pérdida de la tabla anterior ante errores o cancelación. |
| 4 | A-02: dialecto equivocado | Sincronizar el destino y verificar los tres motores junto con las correcciones anteriores. |
| 5 | A-07/A-06: estados obsoletos | Invalidar resultados SQL y respuestas de conexiones que ya no corresponden al estado activo. |

## 2. Puntuación y cobertura por área

Notas cualitativas de revisión, no porcentajes medidos ni certificación. No se calcula una media que oculte los fallos de integridad.

| Área incluida | Nota / 10 | Evidencia y límites |
| --- | --- | --- |
| 1. Código | 5,5 | A-01, A-02, A-05, A-06; contratos tipados y defensas existentes no evitan fallos entre operaciones. |
| 2. Seguridad | 5,5 | A-03/A-04; CSP/capability estrechas y sin secretos detectados, pero ODBC requiere correcciones. |
| 3. Rendimiento | 6 | A-08; bundle dentro de límites y exceso medido de memoria privada en la experiencia básica. |
| 5. Accesibilidad y semántica | 7 | A-09; landmarks, foco, targets y reflow pasan; falta legibilidad en una captura de contraste y prueba manual real. |
| 6. Diseño responsivo y UI/UX | 6 | A-02/A-06/A-07; shell responsivo correcto, estados de destino y datos necesitan sincronización. |
| 7. Arquitectura | 5,5 | A-01/A-07; fuente efectiva y revisión deben ser identidades explícitas en backend y UI. |
| 8. QA y testing | 6 | A-10 y cinco sondeos de regresión fallidos; suites amplias, insuficiente continuidad entre operaciones. |
| 9. Refactorización y limpieza | 6 | A-01/A-03 muestran dónde reducir estado redundante y serializadores manuales; no se prescribe una reescritura total. |
| 10. Ortografía y redacción | 6 | A-11/A-12; el problema principal es exactitud de términos y promesas, no faltas ortográficas generalizadas. |
| 11. Documentación | 5,5 | A-11; buena trazabilidad histórica, estado actual contradictorio en las guías de entrada. |
| 12. Configuración de escritorio | 8 | Sin hallazgo nuevo en manifiestos, versiones, toolchains, permisos y targets Windows revisados; workflows excluidos. |
| 13. Legal y cumplimiento | 5,5 | A-12/A-13; MIT y ficha explícita presentes, aceptación del canal sin resolver. |

SEO no aplica. El anexo FlaUI no se prescribe literalmente: la interfaz es React/WebView2, con pruebas nativas existentes; no es WinUI/WPF/WinForms. Se aprovecha su intención de probar el escritorio real y conservar evidencia, sin crear workflows.

## 3. Evidencias ejecutadas

| Comprobación | Resultado | Evidencia |
| --- | --- | --- |
| TypeScript + Vite | Aprobado, 61 módulos | `.local/audit-2026-09-04/e2e.log` y `accessibility.log` |
| Vitest existente | 274/274, 34 archivos | `.local/audit-2026-09-04/frontend-coverage.log` |
| Cobertura crítica | Falló: proyectos 72,72 % ramas <75 % | Mismo log; global 93,43 % líneas, 83,43 % ramas, 87,32 % funciones |
| Rust seleccionado | 305 dataset +30 proyectos +19 soporte =354 aprobadas; 1 benchmark opt-in omitido | `rust-projects.log`, `rust-support.log` y salida de la ejecución dataset observada durante la sesión |
| E2E del shell | 9/9, 15,4 s; incluye IPC simulado | `.local/audit-2026-09-04/e2e.log` |
| Sondeos frontend adicionales | Cuatro fallos esperados al exigir comportamiento correcto; tres causas | `.local/audit-2026-09-04/frontend-probes.log` y `frontend-probes.probe.tsx` |
| Sondeo Rust adicional | Un fallo: 0 filas frente a 3 esperadas; guardado rechazado | `.local/audit-2026-09-04/backend-probes.log` y copia `backend-harness/` |
| Capturas y contrato visual | Cinco casos aprobados estructuralmente; defecto visual A-09 encontrado al inspeccionar PNG | `.local/validation/accessibility-visual/20260905T162642Z/summary.json` |
| Bundle | 580.879 B raw y 146.035 B gzip; dentro de los límites existentes | `.local/audit-2026-09-04/bundle.json` |
| Toolchains | Node 24.14.0, npm 11.10.1, Rust 1.97.1 aprobados | Manifiestos y verificador ejecutado |
| WebView2/IPC de proyectos | Funcional aprobado: receta, exportación, guardar, reabrir, borrar, tres ciclos; cleanup=true. Falló memoria privada | `.local/validation/webview2-cdp/20260905T213021Z/summary.json` |
| Benchmark nativo 100 MiB | Falló antes de cargar: open_native_dialog_driver_timeout. Fixture 105.446.485 B, 819.137 filas; cleanup=true. No hay medida válida de carga/transformación/exportación ni causa de producto confirmada | `.local/validation/performance-webview2/20260905T213150Z/summary.json` y `.local/validation/webview2-cdp/20260905T213153Z/summary.json` |
| npm audit | 0 vulnerabilidades reportadas, sin actualizar dependencias | `.local/audit-2026-09-04/npm-audit.json` |
| cargo audit | 0 vulnerabilidades no exceptuadas; 2 IDs ignorados, 17 avisos unmaintained, 1 unsound, 1 paquete retirado | `.local/audit-2026-09-04/cargo-audit.json` |
| Secretos | 457 archivos escaneados, 0 coincidencias | `.local/audit-2026-09-04/secrets.json` |
| Legal técnico | Ficha/artefactos presentes; 10 decisiones jurídicas pendientes | `docs/reference/legal-distribution-decision.json` |

El resultado SCA no significa ausencia total de riesgo: conserva las excepciones declaradas de quick-xml y avisos transitivos, algunos de la plataforma Linux no verificada. No se demuestra aquí una ruta explotable Windows por esos avisos. No se repitieron los perfiles compuestos ni benchmarks de CLI excluidos. Las pruebas Rust usaron filtros de módulos del motor compartido; dos helpers source-backed también sirven a automatización, pero no se ejecutó el binario CLI ni sus recorridos.

### Capturas revisadas

- [Desktop](.local/validation/accessibility-visual/20260905T162642Z/desktop.png), [ventana estrecha](.local/validation/accessibility-visual/20260905T162642Z/mobile.png), [zoom 125 %](.local/validation/accessibility-visual/20260905T162642Z/zoom-125.png), [zoom 200 %](.local/validation/accessibility-visual/20260905T162642Z/zoom-200.png), [colores forzados](.local/validation/accessibility-visual/20260905T162642Z/forced-colors.png).
- No se observó overflow horizontal ni truncado en el shell vacío de estas capturas. No equivalen a inspeccionar visualmente todas las vistas con datos ni a Windows High Contrast con lector real.

## 4. Hallazgos detallados

Agrupados por responsabilidad; dentro de cada grupo aparecen primero los de mayor severidad. La tabla de prioridades anterior gobierna el orden de implementación.

### Integridad, arquitectura y código

#### A-01 — El JOIN deja un estado que puede publicar cero filas y bloquea el guardado

**Alta · Nuevo · Esfuerzo medio · Tarea T6-01**

**Ubicación:** `src-tauri/src/dataset.rs:8332, :8336, :12799, :24476, :32714; prueba existente :37553`.

```text
dataset.frame = output_schema;
dataset.source_backed = !context.snapshot_only;
// materialize_loaded_dataset:
if !dataset.source_backed { return Ok(()); }
```

**Problema:** Un JOIN que reutiliza el snapshot de un dataset materializado publica tres filas en Parquet, pero deja el frame vacío y source_backed=false. Las operaciones posteriores interpretan ese estado como ya materializado. La prueba existente comprueba precisamente frame vacío y bandera falsa, sin continuar a una mutación o guardado.

**Evidencia:** Reproducción ejecutada en una copia temporal del mismo código: JOIN de dos CSV sintéticos → preview de 3 filas → guardar devuelve «El cursor del historial activo no coincide con el dataset.» → materializar conserva 0 filas → activar trazabilidad publica 0 filas. El sondeo falla esperando conservar 3. Log: .local/audit-2026-09-04/backend-probes.log; harness: .local/audit-2026-09-04/backend-harness/src-tauri/src/dataset.rs:46871. Se reutilizó el cuerpo de la operación de producción; no se condujo ese escenario por la ventana.

**Impacto:** Pérdida de las filas del dataset activo al continuar una operación aparentemente inocua y bloqueo de guardar el resultado. No se observó corrupción de un proyecto ya guardado ni modificación del CSV original. El historial conserva la posibilidad de recuperar estados; no se afirma pérdida irreversible.

**Solución propuesta:** Representar explícitamente frame materializado frente a cursor Parquet, y resolver el cursor vigente antes de cualquier lector eager. Corregir el publicador y la materialización, sin retirar las optimizaciones. Añadir una prueba de continuidad JOIN → guardar y JOIN → trazabilidad; extender el contrato a consolidación y resolución que usan el mismo publicador.

**Aceptación:** JOIN de 3 filas conserva las 3 tras trazabilidad, guardado/reapertura y undo/redo; consolidación y resolución cumplen la misma invariante, incluidos fallbacks.

#### A-02 — MySQL y SQL Server se envían al backend como PostgreSQL

**Alta · Nuevo · Esfuerzo bajo · Tarea T6-02**

**Ubicación:** `src/features/delivery/deliveryModel.ts:14; src/features/delivery/DeliveryPhase.tsx:302, :314; src/App.tsx:679`.

```text
kind: "postgresql", // INITIAL_DATABASE_TARGET
function changeExportFormat(format: ExportFormat) {
  setLocalExportFormat(format);
  onExportFormatChange?.(format);
}
```

**Problema:** Cambiar el selector solo actualiza el formato visible. databaseTarget.kind sigue en postgresql y App lo entrega sin reconciliarlo con request.format.

**Evidencia:** Dos sondeos de DeliveryPhase ejecutados: formato=mysql y formato=sqlserver producen testKind=postgresql y exportKind=postgresql. Se pulsaron los controles y se observaron sus callbacks, sin abrir conexiones reales. Log: .local/audit-2026-09-04/frontend-probes.log.

**Impacto:** El rótulo del destino y el dialecto de tipos/identificadores divergen; exportaciones a los motores anunciados pueden fallar o usar SQL inadecuado.

**Solución propuesta:** Derivar kind del formato remoto seleccionado, ajustar el esquema inicial por motor sin sobrescribir entradas explícitas, invalidar la comprobación de conexión y rechazar incoherencias en la frontera de exportación. Coordinar la habilitación con A-03/A-04.

**Aceptación:** Los tres formatos envían el kind correspondiente tanto al probar como al exportar; cambiar de motor invalida el resultado previo y se verifica cada dialecto.

#### A-05 — El serializador PostgreSQL genera enteros para columnas BOOLEAN

**Media · Nuevo · Esfuerzo bajo · Tarea T6-05**

**Ubicación:** `src-tauri/src/remote_databases.rs:443, :495`.

```text
// database_type: bool → BOOLEAN (salvo SqlServer)
"true" => "1".to_owned(),
"false" => "0".to_owned(),
```

**Problema:** La tabla se crea con BOOLEAN, pero los INSERT envían 1/0 como literales enteros sin conversión. No son las cadenas booleanas entre comillas aceptadas por PostgreSQL.

**Evidencia:** Código y contrato del motor: PostgreSQL recomienda TRUE/FALSE y su catálogo permite int4→bool solo con conversión explícita. Fuentes: [Booleanos PostgreSQL](https://www.postgresql.org/docs/current/datatype-boolean.html) y [Catálogo de conversiones PostgreSQL 18](https://raw.githubusercontent.com/postgres/postgres/REL_18_STABLE/src/include/catalog/pg_cast.dat) (castcontext=e). Pendiente prueba de integración con ODBC real.

**Impacto:** Una exportación con booleanos puede fallar aunque el test SELECT 1 haya aprobado. Es alcanzable con el destino PostgreSQL predeterminado.

**Solución propuesta:** Transmitir parámetros booleanos tipados o generar TRUE/FALSE para PostgreSQL, conservando BIT y el contrato adecuado de otros motores.

**Aceptación:** Exportar y releer true/false/null en los tres motores conserva tipos y valores desde frame y fuente incremental.

### Seguridad de entrega

#### A-03 — Los literales de texto no preservan la frontera de datos en MySQL

**Alta · Nuevo · Esfuerzo medio · Tarea T6-03**

**Ubicación:** `src-tauri/src/remote_databases.rs:155, :264, :358, :517`.

```text
let escaped = text.replace('\'', "''");
// PostgreSQL y MySQL reciben el mismo literal entre comillas.
connection.execute(statement, (), None)
```

**Problema:** Los valores se concatenan en INSERT y solo se duplican comillas simples. No se parametrizan ni se escapan las barras inversas conforme al modo de la conexión MySQL.

**Evidencia:** Verificado en las rutas frame y source-backed. El manual MySQL establece que, sin NO_BACKSLASH_ESCAPES, secuencias como \n y \r se interpretan; una barra antes de una comilla puede alterar el cierre del literal. Es una inferencia de código y semántica documentada, no una explotación ejecutada. Fuente: [Literales MySQL 8.4](https://dev.mysql.com/doc/refman/8.4/en/string-literals.html) .

**Impacto:** Una celda como una ruta Windows puede cambiar de contenido; valores manipulados pueden salir del literal SQL. La explotación depende de modo SQL, controlador y permisos. A-02 puede hacer fallar antes el CREATE con MySQL habitual; el riesgo permanece en el backend con kind correcto y en configuraciones compatibles con identificadores ANSI. No se atribuye ejecución remota incondicional.

**Solución propuesta:** Usar parámetros ODBC tipados para los valores, también en lotes incrementales; mantener la validación y el escape de identificadores por separado. Verificar round-trip de barras, comillas, Unicode, saltos de línea y nulos.

**Aceptación:** Datos adversos se recuperan byte a byte como datos en MySQL con y sin NO_BACKSLASH_ESCAPES; ningún valor altera la estructura de la sentencia.

#### A-04 — Reemplazar una tabla MySQL puede perder la anterior ante fallo o cancelación

**Alta · Nuevo · Esfuerzo medio · Tarea T6-04**

**Ubicación:** `src-tauri/src/remote_databases.rs:120, :134, :243, :417`.

```text
connection.set_autocommit(false)
// Replace: DROP TABLE → CREATE TABLE → INSERT por lotes → commit
```

**Problema:** Ambas rutas de exportación eliminan la tabla anterior antes de crear y cargar la nueva, confiando en una transacción común. MySQL confirma implícitamente esos DDL.

**Evidencia:** Orden comprobado en código. El manual oficial especifica commit implícito para DROP TABLE y CREATE TABLE: [Commit implícito MySQL 8.4](https://dev.mysql.com/doc/refman/8.4/en/implicit-commit.html) . No se ejecutó una sustitución en una base de datos real.

**Impacto:** Un error de conversión, restricción, conexión o cancelación después del DROP puede dejar perdida la tabla original y una tabla nueva vacía o parcial. Requiere exportación explícita Replace a MySQL y superar las condiciones de dialecto de A-02; no aplica automáticamente a la semántica transaccional de PostgreSQL o SQL Server.

**Solución propuesta:** Deshabilitar Replace para MySQL hasta disponer de una estrategia validada de tabla de preparación y sustitución que preserve datos, permisos y dependencias. Documentar el límite y probar la recuperación con fallos inducidos antes de habilitarlo.

**Aceptación:** Fallo o cancelación antes de completar la sustitución conserva íntegra la tabla anterior; restricciones y permisos quedan verificados con una base sintética.

### UI y vigencia de resultados

#### A-06 — Una comprobación antigua habilita una conexión editada

**Media · Nuevo · Esfuerzo bajo · Tarea T6-06**

**Ubicación:** `src/features/delivery/DeliveryPhase.tsx:319, :324, :1137`.

```text
const result = await onTestDatabaseConnection(databaseTarget);
setDatabaseConnectionState({ kind: "ready", result });
```

**Problema:** Mientras la prueba está pendiente, los campos siguen editables. Cambiarlos pone idle, pero la respuesta de la petición anterior vuelve a poner ready sin comprobar la identidad del destino.

**Evidencia:** Sondeo controlado: iniciar prueba para servidor sintético A, editar a B y resolver la promesa de A. Exportar PostgreSQL queda habilitado para B. Log: .local/audit-2026-09-04/frontend-probes.log.

**Impacto:** La UI presenta como comprobado un destino distinto y puede permitir exportar a una configuración que no se probó. La conexión real aún puede fallar; no se afirma que el backend omita conectarse.

**Solución propuesta:** Vincular cada respuesta a una generación de petición y una huella de la configuración; descartar respuestas obsoletas e invalidar al editar o cambiar de motor.

**Aceptación:** Resolver A después de editar B no habilita B; solo una comprobación vigente del destino exacto permite exportar.

#### A-07 — El panel SQL conserva resultados de otro dataset

**Media · Nuevo · Esfuerzo medio · Tarea T6-07**

**Ubicación:** `src/features/review/ReviewPhase.tsx:568, :595, :611; src/App.tsx:919`.

```text
<LocalQueryPanel comparisonAvailable={comparisonAvailable} ... />
const [state, setState] = useState<...>({ kind: "idle" });
```

**Problema:** El panel no recibe identidad o revisión del dataset. Actualiza el historial agregado, pero no invalida el resultado local cuando Revisar permanece montado y el dataset cambia, como después de consolidar o unir.

**Evidencia:** Sondeo: ejecutar consulta con resultado RESULTADO_ANTERIOR, reemplazar datasetStatus por despues.csv y mantener la misma instancia de ReviewPhase. El encabezado cambia, pero el resultado anterior sigue visible. Log: .local/audit-2026-09-04/frontend-probes.log.

**Impacto:** La persona puede interpretar filas antiguas como resultado del dataset activo; la identidad visual y los datos mostrados no coinciden.

**Solución propuesta:** Propagar una identidad de revisión y reiniciar resultado/peticiones al cambiarla; descartar respuestas de consultas iniciadas sobre revisiones anteriores. No usar solo el nombre del archivo como identidad.

**Aceptación:** Consolidar/JOIN/restaurar revisión invalida resultados y respuestas pendientes; una consulta nueva muestra únicamente datos de la revisión vigente.

### Rendimiento

#### A-08 — La memoria privada vuelve a superar el presupuesto del recorrido nativo

**Media · Regresión de T5-02 · Esfuerzo medio · Tarea T6-08**

**Ubicación:** `fixtures/performance/performance-baseline-v1.json:7; tools/probe-webview2-cdp.ps1:18`. La causa del exceso de memoria no está localizada en el código de producto.

```text
"privateMemoryBytes": 268435456
```

**Problema:** El recorrido nativo pequeño con tres ciclos de transformación/exportación y proyectos excede el límite contractual de 256 MiB privados.

**Evidencia:** WebView2 20260905T213021Z: 272.375.808 B privados (259,76 MiB), 506.159.104 B working set (482,71 MiB), 4 muestras, 7 procesos propios y cleanup=true. Playwright y proyectos/IPC aprueban; falla solo la memoria privada. Fuente: .local/validation/webview2-cdp/20260905T213021Z/summary.json.

**Impacto:** El presupuesto de la experiencia básica no se cumple en esta estación. El exceso es 3,76 MiB y una corrida no demuestra fuga ni crecimiento sostenido. La causa no está localizada; el anclaje a App identifica el recorrido, no atribuye responsabilidad al componente.

**Solución propuesta:** Perfilar retención por fase/proceso y repetir de forma aislada el escenario tras corregir la causa que se identifique. Conservar los presupuestos y no confundirlo con el escenario de 100 MiB, que tiene otros límites.

**Aceptación:** Tres recorridos aislados dentro de 256 MiB privados y 512 MiB working set con cleanup, explicando la variación y conservando presupuestos.

### Accesibilidad

#### A-09 — El enlace de salto pierde el texto en colores forzados

**Media · Nuevo; fecha de introducción no demostrada · Esfuerzo bajo · Tarea T6-09**

**Ubicación:** `src/styles.css:2103; src/App.tsx:840`.

```text
.skip-link { border-color: Highlight; background: Highlight; color: HighlightText; }
```

**Problema:** La captura de Edge con forced-colors:active muestra el enlace enfocado como un rectángulo morado con una franja blanca sin texto legible. El mismo enlace sí muestra su nombre en desktop y zoom.

**Evidencia:** Inspección visual de .local/validation/accessibility-visual/20260905T162642Z/forced-colors.png, comparada con desktop.png. El checker estructural aprueba porque mide foco, geometría y landmarks, no legibilidad. La causa exacta de composición/colores requiere comprobar estilos de pintura; no se declara una ratio numérica ni una prueba de Windows High Contrast real.

**Impacto:** Quien usa contraste forzado puede perder el nombre visible del control al navegar con teclado. La semántica accesible y la acción del enlace siguen presentes.

**Solución propuesta:** Revisar la interacción de colores de sistema y ajustes forzados del enlace, y comprobar visualmente texto y foco en temas claros/oscuros de contraste. Mantener la prueba manual con tecnología de asistencia.

**Aceptación:** Nombre y foco legibles en colores forzados, sin romper el salto al contenido; evidencia visual en las variantes de contraste. Referencia: [WCAG 2.2: contraste mínimo](https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html) .

### QA

#### A-10 — El controlador de proyectos queda bajo la cobertura de ramas acordada

**Media · Regresión de T5-04 · Esfuerzo bajo · Tarea T6-10**

**Ubicación:** `src/features/projects/useProjectsController.ts:41; tools/check-coverage.mjs:8; CONTEXTO.md:24`.

```text
"/src/features/projects/useProjectsController.ts": { statements: 80, lines: 80, branches: 75, functions: 75 }
```

**Problema:** Las 274 pruebas pasan y la cobertura global es alta, pero useProjectsController solo alcanza 72,72 % de ramas frente a 75 %. El control correctamente devuelve error; no es un falso aprobado del runner.

**Evidencia:** Log de la misma HEAD: .local/audit-2026-09-04/frontend-coverage.log. Global: líneas 93,43 %, ramas 83,43 %, funciones 87,32 %. CONTEXTO.md todavía afirma que las cinco capas aprueban.

**Impacto:** La cobertura pactada para operaciones de proyectos no se cumple y no puede presentarse la validación completa como verde. La cifra no identifica por sí sola un defecto funcional.

**Solución propuesta:** Añadir casos conductuales de errores, bloqueo y transiciones realmente faltantes según el mapa de ramas, sin pruebas que solo repliquen la implementación; conservar el umbral.

**Aceptación:** Las cinco capas críticas alcanzan sus umbrales y test:coverage devuelve éxito; la documentación refleja el resultado observado.

### Documentación y redacción

#### A-11 — Las guías de entrada describen capacidades y versiones superadas

**Media · Regresión documental de T5-15/T5-19 · Esfuerzo medio · Tarea T6-11**

**Ubicación:** `README.md:22, :90, :133; docs/explanation/local-first-architecture.md:47; docs/reference/network-privacy.md:3, :18; ROADMAP.md:171; src-tauri/src/projects.rs:23`.

```text
// Código actual
const SCHEMA_VERSION: i64 = 12;
// README describe todavía «El esquema SQLite v4».
```

**Problema:** README y explicación presentan materialización en RAM como regla general; README conserva SQLite v4 y cuatro casos visuales, mientras el código usa v12 y el capturador cinco casos con zoom CSS. Red/privacidad conserva versión 0.95.0 y updater futuro. El índice del roadmap conserva 20 abiertas en Tier 5 aunque solo dos casillas siguen abiertas.

**Evidencia:** Contraste directo entre documentos actuales, proyectos.rs:23, dataset.rs:46 y rutas source-backed, CHANGELOG de 0.146–0.167 y los cinco casos visuales ejecutados. Se respetan las entradas históricas fechadas: el defecto está en información presentada como estado actual.

**Impacto:** Desarrolladores y usuarios toman decisiones de capacidad, privacidad y mantenimiento con un contexto contradictorio; se pierde trazabilidad entre lo pendiente y lo implementado.

**Solución propuesta:** Actualizar las guías de escritorio para distinguir rutas materializadas/incrementales, SQLite v12, ODBC opt-in y cinco casos visuales; consolidar estado actual y conservar la bitácora histórica. No ampliar documentación de CLI ni workflows.

**Aceptación:** Las guías de escritorio concuerdan con la versión auditada y explican límites actuales; el índice se deriva de tareas abiertas y no se borran decisiones históricas.

#### A-12 — El texto de privacidad promete que ningún dataset sale del dispositivo

**Media · Ya conocido, enriquecido por ODBC · Esfuerzo bajo · Tarea T5-18**

**Ubicación:** `src/App.tsx:827, :832; src/features/delivery/DeliveryPhase.tsx:1120; src/App.tsx:681`.

```text
Columnia funciona localmente y no envía datasets a servicios externos.
```

**Problema:** La promesa visible es absoluta, pero Entregar incluye destinos ODBC remotos iniciados por el usuario. «Sin telemetría» no equivale a «sin ninguna transferencia».

**Evidencia:** Texto literal del panel legal comparado con la selección de destino remoto y exportDatasetToDatabase. No se observó envío automático ni una filtración real.

**Impacto:** La información que recibe el usuario sobre transferencias es inexacta. Las implicaciones normativas requieren revisión legal según el contexto de distribución.

**Solución propuesta:** Enriquecer T5-18 con una explicación explícita de procesamiento local y excepción de exportación remota voluntaria, datos enviados, destino y alcance de las credenciales.

**Aceptación:** El texto visible distingue procesamiento local, ausencia de telemetría y exportación remota explícita, y coincide con el comportamiento aprobado.

### Legal y cumplimiento

#### A-13 — La aceptación legal de distribución continúa pendiente

**Alta para distribución pública · Ya conocido · Esfuerzo alto · Tarea T5-20**

**Ubicación:** `docs/reference/legal-distribution-decision.json:3, :6; THIRD_PARTY_NOTICES.md:4; ROADMAP.md:2003, :2153`.

```text
"status": "pending-legal-review",
"responsibleEntity": null,
"jurisdiction": null,
"contact": null,
```

**Problema:** La ficha mantiene diez campos sin resolver, incluidos mercados, canal, privacidad, retención y revisión de atribuciones. La presencia de MIT y un inventario SPDX no equivale a aceptación legal del canal.

**Evidencia:** Lectura de la ficha y verificador técnico: aprobado con diez decisiones jurídicas pendientes. Las tareas T5-18 y T5-20 ya están abiertas; no se duplican ni se marca su aprobación.

**Impacto:** No hay base documental para declarar el producto listo para distribución pública en una jurisdicción concreta. No impide por sí mismo el uso del prototipo local.

**Solución propuesta:** Conservar T5-18/T5-20 y completar decisiones, revisión de avisos/textos y evidencia del canal elegido. Requiere revisión legal; no se inventan obligaciones de cookies, cuentas, GDPR o HIPAA sin contexto.

**Aceptación:** Ficha aprobada por el responsable y revisión jurídica documentada conforme a mercados/canal, con avisos y privacidad coherentes.

## 5. Clasificación y trazabilidad

| Hallazgo | Clasificación | Tarea |
| --- | --- | --- |
| A-01 | Nuevo | T6-01 |
| A-02 | Nuevo | T6-02 |
| A-03 | Nuevo | T6-03 |
| A-04 | Nuevo | T6-04 |
| A-05 | Nuevo | T6-05 |
| A-06 | Nuevo | T6-06 |
| A-07 | Nuevo | T6-07 |
| A-08 | Regresión de T5-02 | T6-08 |
| A-09 | Nuevo; fecha de introducción no demostrada | T6-09 |
| A-10 | Regresión de T5-04 | T6-10 |
| A-11 | Regresión documental de T5-15/T5-19 | T6-11 |
| A-12 | Ya conocido, enriquecido por ODBC | T5-18 |
| A-13 | Ya conocido | T5-20 |

Se abre **Tier 6 — Integridad de escritorio y entrega ODBC (2026-09-05)** con once tareas nuevas. T5-18/T5-20 se enriquecen y conservan abiertas. Las regresiones de T5-02/T5-04/T5-15/T5-19 tienen IDs nuevos; no se borran sus cierres históricos. A-09 es un hallazgo nuevo: sin evidencia anterior comparable no se atribuye una regresión de T5-07.

Comprobaciones de cierres anteriores: cobertura por capa y el rechazo cuando falla siguen implementados; reflow a 125/200 % se ejerce; versiones/toolchains coinciden; CSP/capability siguen acotadas; el recorrido nativo de proyectos funciona y limpia sus artefactos. La regresión está en cumplir el presupuesto/resultado, no en negar que existan los controles anteriores. No se certifican aquí los cierres de workflows excluidos.

## 6. Plan de acción priorizado

- **Quick wins, menos de un día por tarea:** T6-02, T6-05, T6-06, T6-09 y T6-10; ajustar T5-18 para explicar ODBC. No habilitar un destino corregido de forma aislada si persisten T6-03/T6-04.
- **Corto plazo, 1–2 semanas de planificación orientativa:** T6-01, T6-03, T6-04 y T6-07; pruebas de continuidad y round-trip con bases sintéticas. Después T6-08/T6-11 para recursos y documentación verificable. Cada tarea tiene aceptación propia; los plazos no son una medición del equipo.
- **Medio plazo, 1–3 meses según disponibilidad:** completar la revisión manual de accesibilidad I3, validación de instalación I5 y aceptación T5-18/T5-20 con canal/jurisdicción. Son pendientes conocidos, no duplicados. Los workflows de publicación permanecen fuera de esta auditoría.

## 7. Puntos fuertes que conservar

- Capability de la ventana limitada a `core:default` (`src-tauri/capabilities/main.json:6`) y CSP de producción acotada (`src-tauri/tauri.conf.json:29`).
- Proyectos comprueban rutas/snapshots y rechazan el estado inconsistente de A-01 antes de guardar: es preferible el error explícito a corromper un proyecto existente (`src-tauri/src/dataset.rs:32714`).
- El transporte UI/backend mantiene IDs y operaciones concretas; los destinos ODBC usan campos explícitos y credenciales de sesión, sin incorporarlas al workspace (`src/bridge.ts:826`, `src/features/delivery/DeliveryPhase.tsx:1120`).
- Amplia suite y controles que fallan de verdad cuando no se cumplen umbrales; se conservaron todos los resultados rojos y los presupuestos.
- Las figuras de calidad tienen tablas equivalentes (`src/features/review/ReviewPhase.tsx:1341`), y foco/reflow del shell tienen evidencia reproducible.
- Updater exige una versión posterior y verifica tamaño declarado antes de preparar la instalación; sin endpoint no queda habilitado (`src-tauri/src/updater.rs:46`, `:89`, `:94`, `:199`). No se ejerció un canal real.

## 8. Zonas no cubiertas y límites

- CLI, workflows, GitHub Actions y SEO: exclusión expresa; no recomendaciones ni veredicto sobre esas áreas.
- Bases PostgreSQL/MySQL/SQL Server reales: no hay credenciales ni entornos sintéticos configurados. Los hallazgos SQL se apoyan en fuente y documentación oficial; falta validar contra controladores reales. No se ejecutaron escrituras remotas.
- Lector NVDA/Narrador y High Contrast real, ventana con todas las vistas y todos los estados: pendiente. El navegador integrado no tenía una instancia disponible; se conservaron las capturas locales y los recorridos WebView2 existentes como evidencia limitada.
- Instalación en VM limpia, macOS/Linux y updater contra canal real: pendientes conocidos, sin afirmación de soporte o aceptación.
- No se certifica WCAG completo ni cumplimiento jurídico. La jurisdicción, usuarios y escala siguen abiertos.
- El benchmark de 100 MiB falló antes de cargar el dataset y no produjo medidas válidas de rendimiento. Tampoco se verificaron todos los formatos, operaciones ni ejecución integral fuera de RAM; el umbral source-backed habitual es 512 MiB. No se midieron concurrencia de múltiples sesiones ni un crecimiento sostenido de memoria.
- La revisión cubre las áreas acordadas mediante lectura, trazas, pruebas y muestreo de recorridos relevantes. No equivale a demostrar la ausencia de todo defecto en cada línea del repositorio.

## 9. Fusión documental preparada

| Archivo | Se conserva | Se actualiza o añade |
| --- | --- | --- |
| ROADMAP.md | Fases, IDs, cierres y decisiones anteriores | Índice actual de Tier 5, once tareas de Tier 6 y evidencia adicional en T5-18/T5-20. |
| CHANGELOG.md | Todas las versiones existentes | Entrada Interno en Unreleased: únicamente auditoría/documentación, sin anunciar arreglos de producto. |
| CONTEXTO.md | Convención española y registro histórico | Estado de auditoría, límites, resultados rojos, decisiones y sesión del 2026-09-05. |

La fusión es aditiva y queda sin commit para revisión; no se reemplaza el historial ni se crea un CONTEXT.md duplicado. Una copia de los tres documentos previos se conserva en `.local/audit-2026-09-04/documentos-originales/`. El detalle del diagnóstico vive aquí; el roadmap contiene ejecución y aceptación, el changelog registra el cambio documental y el contexto conserva el motivo y estado de continuidad. Este párrafo refleja el cierre diagnóstico anterior a la autorización posterior; el estado aplicado se registra a continuación.

## 10. Estado posterior a la aplicación

Después del cierre diagnóstico, el usuario autorizó aplicar el plan. Se
implementaron y marcaron como cerradas T6-01, T6-02, T6-04, T6-06, T6-07,
T6-09, T6-10 y T6-11. El JOIN publicado desde un cursor histórico conserva
ahora una fuente materializable; el motor ODBC se deriva del formato visible,
las respuestas de conexión y SQL se vinculan a la configuración/revisión
vigente, los valores ODBC usan parámetros tipados, MySQL rechaza `replace`, el
enlace de salto usa colores legibles en `forced-colors`, la cobertura crítica
de proyectos supera el umbral y las guías reflejan la capacidad actual.

La verificación posterior obtuvo 284 pruebas frontend aprobadas, cobertura
crítica aprobada (proyectos 81,08 % de ramas), 78 pruebas focalizadas de UI y
una prueba Rust de la regresión JOIN; las cuatro pruebas de conexión/revisión
añadidas cubren respuestas tardías y cambios de motor. T6-03 y T6-05 quedan
abiertas para round-trip contra controladores reales de PostgreSQL, MySQL y SQL
Server, que no están disponibles en esta estación. T6-08 también queda abierta:
el sondeo nativo se difiere hasta abrir el panel de recursos y dos de tres
recorridos mutados quedaron dentro del presupuesto (248,78 y 253,61 MiB), pero
dos recorridos posteriores alcanzaron 256,24 y 256,88 MiB (evidencias
`.local/validation/webview2-cdp/20260905T223115Z` y
`.local/validation/webview2-cdp/20260905T224811Z`). La variación del proceso
WebView2 aún no tiene una causa de producto localizada, por lo que la aceptación
exige repetir tres recorridos conformes. La aplicación no se declara lista para
distribución pública hasta completar esas verificaciones y T5-18/T5-20.

La corrección técnica de T5-18 también se aplicó: el panel legal distingue el
procesamiento local, la ausencia de conexiones automáticas y la exportación ODBC
iniciada explícitamente por el usuario, e indica que la cadena y contraseña solo
viven durante esa sesión. La aceptación jurídica y del canal permanece abierta.
