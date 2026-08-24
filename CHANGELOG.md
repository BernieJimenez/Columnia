# Changelog

Todos los cambios visibles de Columnia se registran aquí. Las versiones siguen
SemVer y el estado real del prototipo se contrasta con el código, los tests y
los artefactos de validación locales.

## [Unreleased]

### Añadido

- Contrato de calidad versionado `columnia-quality-rules` v1, con guardado
  atómico, importación de Columnia/DataPrep v1–v3 y compatibilidad con el
  documento legado v1; versiones futuras y formatos ambiguos fallan cerrados.
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
- Gate de cobertura V8 por capa para `src` (80% statements/lines, 75% branches y
  functions) y 202 tests frontend.
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

### Validación

- Verificados build Vite, contratos IPC, 177 pruebas Rust y 202 pruebas frontend,
  pruebas del monitor,
  evidencia release desktop/móvil/zoom 125%/`forced-colors` y el gate
  `perf:i1:check`, cobertura frontend, auditorías de supply chain, smoke CDP
  (`.local/validation/webview2-cdp/20260824T011617Z`), Release y Package con
  MSI/NSIS.

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
