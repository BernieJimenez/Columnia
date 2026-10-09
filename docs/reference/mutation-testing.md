# Pruebas de mutación

`cargo mutants` cambia una a una pequeñas partes del código (un `==` por `!=`,
una función que devuelve siempre `true`…) y ejecuta los tests de la librería.
Un mutante «superviviente» es un cambio de comportamiento que ningún test nota.
Es una comprobación puntual: no forma parte de `tools/check.ps1` porque tarda
horas.

## Cómo ejecutarla

`tauri.conf.json` usa `..\LICENSE`, que queda fuera de `src-tauri`, así que la
copia que hace `cargo mutants` por defecto no compila. Se ejecuta en un worktree
aparte, mutando en el sitio y con un `target` de ruta corta (DuckDB no compila
con rutas largas en Windows):

```powershell
git worktree add --detach $env:TEMP\columnia-mutants HEAD
Copy-Item -Recurse dist $env:TEMP\columnia-mutants\dist
cd $env:TEMP\columnia-mutants\src-tauri
$env:COLUMNIA_TEST_HARNESS_MANIFEST = "1"
$env:CARGO_TARGET_DIR = "$env:TEMP\cmt"
cargo mutants --in-place -C=--lib -f src/dataset/csv_formula_safety.rs -f src/dataset/file_validation.rs -f src/dataset_fingerprints.rs -f src/dataset/date_inference.rs --timeout 300
```

`-C=--lib` deja fuera el binario `columnia-cli`, cuyo test choca con el
manifiesto del arnés. Al terminar se borran el worktree
(`git worktree remove --force`) y la carpeta `cmt`.

## Resultado del 2026-10-08 (QA-57)

121 mutantes en 2 h sobre el commit `e71247c`.

| Módulo | Detectados | Supervivientes | No compilan |
| --- | --- | --- | --- |
| `dataset/csv_formula_safety.rs` | 15 | 0 | 0 |
| `dataset/date_inference.rs` | 49 | 0 | 2 |
| `dataset_fingerprints.rs` | 13 | 3 | 1 |
| `dataset/file_validation.rs` | 30 | 6 | 2 |

Qué se hizo con cada superviviente:

- `dataset_fingerprints.rs:111` (tres mutantes en `row_index % 4096 == 0`):
  ningún test comprobaba cada cuánto se consulta la cancelación. Un test fija
  ahora que se consulta una vez cada 4096 filas.
- `file_validation.rs:33` y `:43` (`is_symbolic_link_or_reparse_point`
  devuelve siempre `true` o `false`, o `||` pasa a `&&`): el test con enlace
  simbólico necesita privilegios que Windows no da por defecto. Un test usa
  ahora una unión de directorio, que no los necesita. El cambio de `||` por `&&`
  sigue sin detectarse: Rust ya ve la unión como enlace, y un punto de
  reanálisis que no sea enlace (un archivo de OneDrive solo en la nube, por
  ejemplo) no se puede crear en un test.
- `file_validation.rs:93` (aceptar cualquier error al leer el destino, no solo
  «no existe»): un test con un nombre de archivo inválido comprueba ahora que
  ese error se informa.
- `file_validation.rs:121` y `:122` (plan B de `ensure_destination_is_not_source`
  con rutas canónicas): solo corre si `same_file` no puede abrir un archivo que
  `canonicalize` sí resuelve, y no se ha encontrado forma de provocarlo. Quedan
  como supervivientes aceptados.
