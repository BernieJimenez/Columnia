# Inventario local de dependencias y auditorías

Snapshot tomado el **2026-08-23** sobre `0.49.0`, rama `master`. Este archivo
es una lista local reproducible, no una aprobación permanente de actualizar a
la última versión. Antes de cambiar una dependencia, ejecuta los comandos de la
tabla y registra el resultado en el mismo cambio.

## Dependencias directas declaradas

### npm

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

### Cargo

Las dependencias se resuelven desde `crates.io` mediante `Cargo.lock` y el gate
de supply chain verifica checksums y fuentes. Las versiones declaradas son:

`calamine 0.36.1`, `chrono 0.4.45`, `polars 0.54.4`, `rusqlite 0.37.0`,
`serde 1`, `serde_json 1`, `tauri 2`, `tauri-plugin-dialog 2.7.2`, `tempfile 3`,
`unicode-normalization 0.1`, `tauri-plugin-single-instance 2` para escritorio,
y `tauri-build 2` como dependencia de build.

## Snapshot de actualización

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

## Auditorías ejecutadas

| Comando | Resultado del snapshot | Interpretación |
| --- | --- | --- |
| `npm audit --json --omit=optional` | 0 vulnerabilidades reportadas; 229 dependencias totales | Revisión npm limpia en esta fecha; repetir antes de release |
| `cargo audit --version` | Herramienta no instalada | La auditoría Cargo bloqueante pertenece a I4 y sigue pendiente |
| `cargo deny --version` | Herramienta no instalada | Evaluación de advisories/licencias/duplicados pendiente en I4 |
| `cargo outdated --version` | Herramienta no instalada | No se inventa un estado de actualización Cargo |

La ausencia de una herramienta no se interpreta como ausencia de vulnerabilidades.
I4 debe instalar o ejecutar equivalentes locales y conservar la evidencia antes
de un release.

## Procedimiento de actualización

1. Ejecuta `npm outdated --json` y, si aplica, `cargo outdated` en una estación
   con la herramienta instalada.
2. Lee los changelogs y revisa breaking changes de Tauri, Vite, Polars y Rust.
3. Cambia una familia relacionada por vez; conserva lockfiles reproducibles.
4. Ejecuta `npm run governance:check`, `npm test`, `npm run build` y el perfil
   `Full` si la dependencia cruza Rust, IPC o Tauri.
5. Actualiza este snapshot con fecha, versión observada, motivo y resultado.

## Fuentes de verdad

- [`package.json`](../../package.json) y [`package-lock.json`](../../package-lock.json)
  para npm.
- [`src-tauri/Cargo.toml`](../../src-tauri/Cargo.toml) y
  [`src-tauri/Cargo.lock`](../../src-tauri/Cargo.lock) para Cargo.
- [`src/supply-chain.test.ts`](../../src/supply-chain.test.ts) para integridad,
  procedencia y ausencia de identidades contradictorias.
