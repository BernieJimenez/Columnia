# Matriz de capacidades vigente

Este documento describe las capacidades que Columnia mantiene actualmente. La
matriz es una referencia de producto y no depende de herramientas externas ni
de rutas fuera del repositorio.

## Matriz

| Área | Capacidad actual | Estado | Próximo límite conocido |
| --- | --- | --- | --- |
| Entradas tabulares | CSV, TSV, JSON/JSONL, Parquet, XLSX, XLS, XLSB y ODS | Implementada | Ampliar casos difíciles de libros |
| Vista previa | Paginación de 50 filas, muestreo acotado y fallback seguro | Implementada | Ampliar evidencia con datasets grandes |
| Perfilado | Esquema, nulos, duplicados exactos y parecidos, estadísticas, calidad, outliers, grupos categóricos y cobertura temporal | Implementada | Ampliar análisis exploratorio |
| Calidad | Documento `columnia-quality-rules` v1, reglas base y avanzadas, tolerancias, severidad y validación previa a entrega | Implementada | Añadir políticas para destinos remotos |
| Transformaciones | Recetas lazy/eager, historial, renombres, casts, filtros, texto, fechas, split/merge, reemplazo literal o regex segura, outliers, imputación y agregación | Implementada | Seguir ampliando ejecución incremental |
| Comparación | Dataset secundario local, comparación por clave, consolidación segura y joins `INNER`/`LEFT`/`FULL` | Implementada | Ampliar análisis comparativo |
| Visualizaciones | Completitud, outliers, patrones de nulos, formatos, grupos, calendario, tendencia temporal y correlaciones, con tablas equivalentes | Implementada | Ampliar interacciones |
| Salidas | CSV, JSON, Parquet, SQL, Excel, SQLite y bundle ZIP auditable | Implementada | Añadir destinos de base de datos |
| Proyectos | Catálogo SQLite, snapshots Parquet, historial, reglas, recetas, CLI, archivos recientes y reapertura segura | Implementada | Preferencias de workspace más amplias |
| Privacidad | Sin telemetría, sanitización de contratos e informes, detección agregada de datos personales y máscara/hash local | Implementada | Extender contratos equivalentes |
| Escala | Lazy para recetas compatibles, recetas source-backed de proyección/filtros, lectura por bloques, snapshots administrados, DuckDB opcional, cancelación y presupuestos explícitos | Parcial | Ejecución integral fuera de RAM |

## Contrato de calidad

El formato durable es:

```json
{
  "format": "columnia-quality-rules",
  "version": 1,
  "rules": []
}
```

Columnia valida el formato antes de guardarlo o aplicarlo. También puede leer el
documento histórico sin campo `format` cuando contiene `version: 1` y `rules`.
Las versiones futuras, campos ambiguos y reglas no representables fallan de
forma cerrada.

Las reglas admitidas incluyen valores permitidos, expresiones regulares, tipos,
unicidad compuesta, comparación entre columnas, integridad referencial,
monotonía, agregados, reconciliación, drift de distribución, rangos de fecha,
condiciones, contratos de esquema y conteo de filas. La evaluación pública solo
expone estados, conteos, porcentajes y acciones de revisión.

## Recetas y ejecución

Las recetas se validan antes de modificar el dataset. El historial conserva
revisiones reversibles y la publicación de un resultado es atómica. Las rutas,
filas y valores permanecen en Rust; React recibe únicamente metadatos acotados,
identificadores opacos y resultados agregados.

Las operaciones compatibles con el plan lazy se ejecutan sobre fuentes y
snapshots sin clonar innecesariamente el dataset completo. Las recetas
source-backed que combinan selección/renombrado con hasta tres filtros también
se ejecutan directamente sobre la fuente mediante DuckDB y publican un snapshot
Parquet privado, conservando conteo, orden y nulos. Cuando una combinación no es
segura para lazy o source-backed, Columnia usa un fallback eager atómico y
mantiene la recuperación del dataset anterior ante errores o cancelación.

## Proyectos y almacenamiento

Cada proyecto conserva un snapshot Parquet administrado y un catálogo SQLite.
Abrir un proyecto valida el perfil, las reglas, el esquema, el cursor y las
revisiones antes de activar la sesión. El historial está limitado a doce
revisiones y el registro SQL conserva solo estado, duración y filas afectadas.

La interfaz no recibe rutas internas. Los selectores nativos, la canonicalización
de archivos y la política de escritura atómica viven en la capa Rust.

## Verificación

Los contratos se cubren por capas:

- `npm run test` valida la UI y los modelos TypeScript.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib` valida el motor nativo.
- `npm run build` valida tipos y bundle de producción.
- `npm run docs:check` valida enlaces, versiones y ownership documental.
- `npm run ipc:check` compara el inventario IPC con `lib.rs` y el bridge.
- `npm run verify:tier` orquesta los gates reproducibles del tier completo.
