# Referencia de la CLI de Columnia

`columnia-cli` reutiliza el motor Rust sin abrir la ventana Tauri. Todos los
comandos exigen rutas explícitas, validan y canonicalizan sus entradas y emiten
JSON v1 sin rutas, filas ni muestras del dataset.

## Ejecutable

Desde la raíz del repositorio:

```powershell
cargo run --release --manifest-path src-tauri/Cargo.toml --bin columnia-cli -- <comando> <opciones>
```

## Comandos de datasets

### `inspect`

```text
inspect --input FILE [--sheet NAME --header first-row|generated]
```

Acepta CSV, TSV, JSON, Parquet, XLSX, XLS, XLSB y ODS. Los libros requieren una
hoja exacta y un modo de encabezado. Devuelve `schemaVersion`, `command`, nombre
visible, dimensiones y columnas con nombre y tipo.

### `transform`

```text
transform --input FILE [--sheet NAME --header first-row|generated] --recipe RECIPE --output FILE --format csv|parquet
```

Aplica una receta JSON v1 de forma atómica y publica CSV o Parquet. Los destinos
no pueden ser el input ni la receta; los fallos no dejan outputs parciales.

### `validate`

```text
validate --input FILE [--sheet NAME --header first-row|generated] --rules RULES
```

Evalúa un contrato de calidad JSON v1 y devuelve únicamente conteos. Código 0
indica contrato aprobado, 2 contrato reprobado y 1 error de uso o carga.

### `batch`

```text
batch --manifest MANIFEST
```

El manifiesto JSON v1 contiene entre 1 y 64 trabajos. Columnia ejecuta un
preflight completo antes de escribir, rechaza colisiones y publica cada salida
individualmente. Un fallo tardío conserva los trabajos anteriores y termina con
código 2; un manifiesto inválido termina con código 1 sin outputs.

## Comandos de proyectos

Todos requieren `--store DIR`. Ese almacén es independiente del directorio
privado de la aplicación de escritorio.

### `project-save`

```text
project-save --store DIR --name NAME --input FILE [--id ID] [--sheet NAME --header first-row|generated] [--recipe FILE] [--rules FILE] [--profile]
```

Crea o actualiza un snapshot durable SQLite/Parquet. `--profile`, receta y
reglas son opcionales.

### `project-list`

```text
project-list --store DIR
```

Lista resúmenes ordenados sin activar datasets ni devolver rutas internas.

### `project-inspect`

```text
project-inspect --store DIR --id ID
```

Devuelve metadatos, presencia de perfil/receta, reglas y estado agregado del
historial sin abrir el proyecto en una sesión.

### `project-export`

```text
project-export --store DIR --id ID --output FILE --format csv|parquet [--allow-unvalidated]
```

Valida reglas guardadas y exporta atómicamente. `--allow-unvalidated` solo
autoriza proyectos sin reglas; nunca omite una regla reprobada.

### `project-delete`

```text
project-delete --store DIR --id ID --confirm ID
```

Borra únicamente cuando `--confirm` coincide exactamente con `--id`.

## Contratos y seguridad

- JSON de salida: `schemaVersion: 1` y `command`.
- Código 0: éxito; código 1: uso, carga o almacenamiento; código 2: calidad
  reprobada o fallo parcial de batch.
- Las rutas se mantienen en Rust y no aparecen en stdout ni en errores de
  contrato.
- La exportación es atómica y un fallo conserva un destino anterior.

## Relacionado

- [Tutorial del primer dataset](../tutorials/first-dataset.md)
- [How to validar evidencia release](../how-to/validate-release-evidence.md)
- [Explicación local-first](../explanation/local-first-architecture.md)
