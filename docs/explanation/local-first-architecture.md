# Por qué Columnia es local-first

Columnia prepara datos que pueden contener información sensible. Si el flujo
depende de un servidor para abrir, perfilar o exportar un archivo, la privacidad
deja de ser una propiedad del producto y pasa a depender de una red y un
servicio externo. El diseño local-first mantiene ese camino fuera del flujo
normal.

## El problema

Una interfaz web tiene acceso sencillo a APIs remotas y al filesystem del
navegador es limitado. Un diseño que mezcla ambas cosas puede terminar enviando
rutas, filas o fórmulas a una capa que no necesita conocerlas. También dificulta
probar que una operación cancelada no dejó un archivo parcial.

## El enfoque

```text
Persona
  |
  v
React / App.tsx
  |  estado, formularios, navegación y confirmaciones
  v
bridge.ts
  |  invoke() + contratos TypeScript + progreso
  v
Tauri / Rust
  |  rutas, archivos, datasets, SQLite y snapshots Parquet
  v
Polars + Calamine + filesystem local
```

React solicita casos de uso concretos. Rust valida y canonicaliza las rutas,
lee los archivos, mantiene el dataset y publica operaciones atómicas. La CLI
entra directamente al mismo motor Rust. La CSP de producción no permite
conexiones remotas y la capability de la ventana solo concede `core:default`.

## Qué gana la persona usuaria

- Los datos permanecen en el equipo durante el flujo normal.
- Cancelar o fallar una exportación conserva el archivo anterior.
- Abrir un proyecto valida snapshots y catálogo antes de reemplazar el dataset
  activo.
- La misma lógica puede verificarse desde la UI, la CLI y tests locales.

## Trade-offs

- El binario ocupa más espacio y requiere Rust/Tauri/WebView2 en desarrollo.
- El dataset se materializa en memoria en este prototipo; lazy/incremental queda
  pendiente.
- No hay sincronización ni colaboración remota integrada.
- La calidad depende de ejecutar los gates locales porque el proyecto no usa CI.

## Alternativas consideradas

- **Servidor de procesamiento:** descartado para el flujo base porque ampliaría
  la frontera de confianza y haría obligatoria la red.
- **Filesystem controlado directamente por React:** descartado porque permite
  que una UI tenga más autoridad de la necesaria sobre rutas y datos.
- **DuckDB desde el scaffold:** diferido hasta medir Polars con datasets grandes
  y justificar operaciones multidataset.

Para ver los contratos exactos de la frontera y sus decisiones, consulta
[ADR-0001](../adr/0001-contratos-del-repositorio.md) y la
[referencia de la CLI](../reference/cli.md).
