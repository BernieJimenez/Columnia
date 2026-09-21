# Auditoría consolidada de Columnia

Revisada: 2026-09-14. Este documento sustituye las auditorías fechadas de
producto, diseño, ingeniería, usabilidad y automatización. Conserva las
conclusiones que explican decisiones ya tomadas e integra el inventario de
dependencias y gates locales; el trabajo pendiente vive en
[`docs/reference/roadmap-current.md`](docs/reference/roadmap-current.md) y su
detalle histórico en [`ROADMAP.md`](ROADMAP.md).

## Dictamen vigente

Columnia ya dispone de un motor local amplio para importar, perfilar, preparar,
validar y entregar datos. El mayor valor pendiente no proviene de añadir más
herramientas aisladas. Proviene de reducir decisiones repetidas, coordinar el
flujo completo y comprobarlo con archivos y personas reales.

La dirección aprobada es:

1. una acción principal contextual y estados ligados a resultados reales;
2. una importación explicable seguida de un plan de preparación ejecutable;
3. validación, excepciones, cancelación y recuperación coordinadas;
4. tareas reutilizables para repetir un trabajo compatible;
5. accesibilidad y beta nativas antes de ampliar el alcance del producto.

No se identificó un defecto crítico abierto en las revisiones consolidadas. La
ausencia de un hallazgo crítico no certifica seguridad, rendimiento, usabilidad
o compatibilidad con todos los entornos.

## Capacidades comprobadas que se conservan

- Importación local de delimitados, JSON, Parquet y libros, con perfilado y
  preflight de recursos.
- Preparación reversible, recetas, historial, deshacer/rehacer, SQL local,
  comparación, consolidación, conflictos y JOIN.
- Contratos de calidad, privacidad visible, exportaciones locales, bundle
  auditable y entrega ODBC con límites explícitos.
- Proyectos durables, recuperación, preferencias, CLI y ejecución batch.
- Procesamiento incremental, guardias de RAM, cancelación cooperativa y gates
  locales de pruebas, documentación, red, secretos, IPC y distribución.
- Un sistema visual común, temas, navegación por fases y controles accesibles
  verificados automáticamente en navegador.

Estas capacidades no deben reconstruirse como funciones nuevas. Las mejoras del
roadmap deben integrarlas y simplificar su uso.

## Registro histórico consolidado

| Fecha | Revisión | Resultado que se conserva |
| --- | --- | --- |
| 2026-08-28 | Auditoría profesional integral | Abrió Tier 5 para robustecer gates, evidencia de release, arquitectura y distribución. Las tareas técnicas se implementaron; notices, descubribilidad y aprobación de distribución siguen dentro del gate de release. |
| 2026-09-05 | Reauditoría de escritorio | Abrió Tier 6. Se corrigieron integridad de JOIN/publicación, dialectos y seguridad ODBC, resultados obsoletos, memoria, accesibilidad y cobertura. Queda la aceptación real de booleanos en SQL Server. |
| 2026-09-06 | Revisión visual | Aplicó navegación petróleo, superficies neutras, jerarquía común, temas y corrección de Preferencias. La aceptación nativa con lector de pantalla continúa pendiente. |
| 2026-09-07 | Reauditoría incremental | Cerró Tier 7: presupuesto CSS, inventario IPC, falso positivo del gate de red y evidencia de dependencias. |
| 2026-09-13 | Auditoría general | Confirmó que beta, repetición de tareas, explicabilidad y robustez aportan más valor que ampliar funciones. Corrigió la entrada documental y verificó suites frontend/nativas. |
| 2026-09-13–14 | Usabilidad y automatización | Simplificó inicio, carga automática, herramientas contextuales, plan acotado, validación integrada, reglas resumidas y resultado de exportación accionable. El resto se fusionó en Tier 9. |

La evidencia detallada permanece en los commits, `CHANGELOG.md`, `CONTEXTO.md`,
las carpetas locales de validación y las tareas históricas de `ROADMAP.md`.

## Mejoras aplicadas desde la última revisión

- El diagnóstico comienza tras cargar y descarta resultados de revisiones
  anteriores.
- El inicio prioriza abrir un archivo o continuar un proyecto; la administración
  queda en contexto.
- Preparar muestra primero señales presentes y un plan acotado ligado a la
  revisión para recorte, encabezados y marcadores de ausencia.
- Entregar valida el contrato y exporta desde una sola acción; las reglas se
  resumen en lenguaje de datos y el editor técnico queda a demanda.
- Una publicación exitosa muestra destino, formato, tamaño, calidad, cambios y
  privacidad. Error y cancelación conservan el dataset preparado.

## Decisiones de alcance

Se mantienen fuera de la cola vigente hasta que la beta demuestre demanda:

- diccionario de negocio durable;
- catálogos de equivalencias y coincidencia difusa;
- vigilancia o programación de carpetas;
- reanudación sofisticada de lotes;
- soporte anunciado para macOS o Linux;
- nuevos conectores, nube, cuentas, colaboración o telemetría.

También se descarta añadir un chat, más paneles o más botones como solución a la
automatización. Las reglas deterministas existentes deben coordinarse mediante
estados y contratos explícitos.

## Trazabilidad hacia el roadmap

| Hallazgos anteriores | Tarea consolidada |
| --- | --- |
| UX01, UX02, UX06, CO02, CO03, CO05 | RV01 — Flujo contextual y estado central |
| UX05, C01 | RV02 — Importación unificada y explicable |
| AU01, AU02, AU03, AU06, D01 | RV03 — Plan completo y resultado antes/después |
| AU04, AU05, D04, H03 | RV04 — Excepciones, cancelación y recuperación |
| OUT03, REP01, REP02, E03 | RV05 — Tarea reutilizable de preparación y entrega |
| CO04, CO06 | RV06 — Accesibilidad y controles compactos |
| A01, A03, A04, CO07 | RV07 — Beta y medición con tareas reales |
| A02 | RV08 — Regresiones sintéticas derivadas de beta |
| I01 | RV09 — Aceptación nativa de accesibilidad |
| F01, T6-05 | RV10 — Aceptación SQL Server |
| I03, I04, T5-18, T5-20, I5–I7 | RV11 — Candidato instalable y distribución |
| AU07, H04 | RV12 — Recursos y escala medibles |
| G01, REP04 | RV13 — Respaldo y autoguardado recuperables |
| F02, F03 | RV14 — Preflight y presets de entrega |
| G03, REP03 | RV15 — Lotes gráficos bajo demanda validada |
| H01 | RV16 — Modularización gradual del motor |

Los identificadores antiguos sirven solo para rastrear decisiones y commits. No
son una segunda lista de trabajo.

## Inventario de dependencias y controles

Este snapshot técnico se conserva aquí para evitar una segunda auditoría
independiente. No es otra cola de trabajo: los cambios necesarios siguen el orden
de [`roadmap vigente`](docs/reference/roadmap-current.md).

Snapshot de dependencias actualizado el **2026-09-07** sobre `0.167.0`. La ficha vigente se conserva sobre `1.25.0` porque este incremento solo cambia la versión del proyecto, no el grafo de dependencias. El inventario IPC quedó sincronizado el **2026-09-14**. Es una referencia local
reproducible, no una aprobación permanente de actualizar a la última versión.
Antes de cambiar una dependencia, ejecuta los comandos de la tabla y registra el
resultado en el mismo cambio.

### Dependencias directas declaradas

#### npm

| Grupo | Dependencia | Declaración |
| --- | --- | --- |
| runtime | `@tauri-apps/api` | `^2.11.1` |
| runtime | `react` | `^19.1.0` |
| runtime | `react-dom` | `^19.1.0` |
| desarrollo | `@playwright/test` | `^1.62.1` |
| desarrollo | `@tauri-apps/cli` | `^2.11.4` |
| desarrollo | `@testing-library/jest-dom` | `^6.8.0` |
| desarrollo | `@testing-library/react` | `^16.3.0` |
| desarrollo | `@types/node` | `^24.0.0` |
| desarrollo | `@types/react` | `^19.1.8` |
| desarrollo | `@types/react-dom` | `^19.1.6` |
| desarrollo | `@vitejs/plugin-react` | `^4.6.0` |
| desarrollo | `jsdom` | `^26.1.0` |
| desarrollo | `typescript` | `~5.8.3` |
| desarrollo | `vite` | `^7.0.4` |
| desarrollo | `vitest` | `^3.2.4` |
| desarrollo | `@vitest/coverage-v8` | `^3.2.7` |

#### Cargo

Las dependencias se resuelven desde `crates.io` mediante `Cargo.lock` y el gate
de supply chain verifica checksums y fuentes. Las versiones declaradas son:

`calamine 0.36.1`, `chrono 0.4.45`, `polars 0.55.2`, `rusqlite 0.37.0`,
`serde 1`, `serde_json 1`, `tauri 2`, `tauri-plugin-dialog 2.7.2`, `tempfile 3`,
`unicode-normalization 0.1`, `tokio 1.48`, `tokio-util 0.7`,
`tauri-plugin-single-instance 2` y `tauri-plugin-updater 2.10.1` para escritorio,
y `tauri-build 2` como dependencia de build.

### Snapshot de actualización y auditorías ejecutadas

`npm outdated --json` encontró versiones mayores disponibles para estas
dependencias. El campo `wanted` coincide con la declaración actual; no se
actualizaron automáticamente porque varias versiones cambian el major:

| Dependencia | Resuelta | Wanted | Latest observado |
| --- | ---: | ---: | ---: |
| `@testing-library/jest-dom` | 6.9.1 | 6.9.1 | 7.0.1 |
| `@types/node` | 24.13.3 | 24.13.3 | 26.2.0 |
| `@vitejs/plugin-react` | 4.7.0 | 4.7.0 | 6.1.0 |
| `jsdom` | 26.1.0 | 26.1.0 | 29.1.1 |
| `typescript` | 5.8.3 | 5.8.3 | 7.0.2 |
| `vite` | 7.3.6 | 7.3.6 | 8.2.2 |
| `vitest` | 3.2.7 | 3.2.7 | 4.1.11 |

El snapshot no implica que una actualización sea necesaria. Cada major exige
revisar Tauri/Vite, tests y build antes de modificar el lockfile.

| Comando | Resultado del snapshot | Interpretación |
| --- | --- | --- |
| `npm audit --json --omit=optional` | 0 vulnerabilidades reportadas; 180 dependencias del lockfile | Reauditado el 2026-09-20; repetir antes de release |
| `cargo audit --json` | `cargo-audit 0.22.2`; 0 vulnerabilidades después de las excepciones documentadas; los avisos informativos no son bloqueantes | `quick-xml 0.39.4` llega transitivamente por `object_store 0.13.2`; Columnia no habilita los features cloud ni expone un flujo remoto. La razón vigente está en `src-tauri/deny.toml` |
| `cargo deny --format json check` | `cargo-deny 0.20.2`; advisories/licencias/fuentes sin errores; 48 duplicados en warning | Política explícita en `src-tauri/deny.toml`; excepciones upstream tienen razón y se revisan al actualizar Tauri/Polars |
| `cargo outdated --version` | Herramienta no instalada | No se inventa un estado de actualización Cargo |
| `npm run secrets:check` | 0 hallazgos; 477 archivos inspeccionados | Escaneo local de claves privadas, tokens y credenciales asignadas |
| `npm run network:check` | Aprobado | Sin APIs de red/telemetría en producción; CSP solo deja IPC interno |
| `npm run notices:check` | Aprobado; 999 identidades de dependencia sin `UNKNOWN`, sin filas duplicadas | `THIRD_PARTY_NOTICES.md` se deriva offline de ambos lockfiles y rechaza licencias desconocidas, contradictorias o incompletas |
| `npm run toolchains:check` | Aprobado; Node 24.14.0, npm 11.10.1 y Rust/Cargo 1.97.1 | Las versiones exactas están fijadas en `package.json` y `rust-toolchain.toml` |
| `npm run ipc:check` | Aprobado; 84 comandos de producción, 4 debug y 69 estructuras compartidas | El inventario se genera desde `generate_handler!` y se publica en [`ipc-inventory.json`](docs/reference/ipc-inventory.json); incluye tareas reutilizables, catálogo de proyectos/candidato de recuperación combinado, preflight/presets de entrega, inspección de libros en dos pasos y updater autenticado |

Las excepciones de `cargo audit`/`cargo deny` no ocultan una vulnerabilidad de
la aplicación: están limitadas a advisories transitivos con razón, versión y
ruta upstream registradas en [`deny.toml`](src-tauri/deny.toml). Si una
actualización de Polars/Tauri elimina una excepción, se debe quitar del archivo
en el mismo cambio. Cada ejecución deja JSON sanitizado bajo
`.local/validation/`.

### Procedimiento de actualización

1. Ejecuta `npm outdated --json` y, si aplica, `cargo outdated` en una estación
   con la herramienta instalada.
2. Lee los changelogs y revisa breaking changes de Tauri, Vite, Polars y Rust.
3. Cambia una familia relacionada por vez; conserva lockfiles reproducibles.
4. Ejecuta `npm run governance:check`, `npm test`, `npm run test:coverage`,
   `npm run build`, `npm run supply-chain:check` y el perfil `Full` si la
   dependencia cruza Rust, IPC o Tauri.
5. Actualiza este snapshot con fecha, versión observada, motivo y resultado.

### Fuentes de verdad

- [`package.json`](package.json) y [`package-lock.json`](package-lock.json)
  para npm.
- [`src-tauri/Cargo.toml`](src-tauri/Cargo.toml) y
  [`src-tauri/Cargo.lock`](src-tauri/Cargo.lock) para Cargo.
- [`src/supply-chain.test.ts`](src/supply-chain.test.ts) para integridad,
  procedencia y ausencia de identidades contradictorias.
- [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md) para el inventario
  generado de avisos de terceros.
