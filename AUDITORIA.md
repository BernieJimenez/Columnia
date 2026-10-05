# Auditoría de Columnia

Este documento conserva solo lo que sigue vigente de las auditorías: la ficha de
dependencias y los controles que los gates verifican. Las auditorías y
reauditorías anteriores (hasta el Tier 10, cerrado el 2026-09-25) están
archivadas sin cambios en
[`docs/archive/2026-09/AUDITORIA.md`](docs/archive/2026-09/AUDITORIA.md); una
revisión nueva debe partir del código, no de ese archivo.

## Inventario de dependencias y controles

Este snapshot técnico se conserva aquí para evitar una segunda auditoría
independiente. No es otra cola de trabajo: los cambios necesarios siguen el orden
de [`ROADMAP.md`](ROADMAP.md).

Ficha de dependencias comprobada el **2026-10-04** sobre `1.26.0` desde los manifiestos (`docs:check` la compara con ellos). El commit `cb86ea5` (2026-09-20) subió versiones mayores de TypeScript (7), Vite (8), Vitest y su cobertura (5), jsdom (29), `@vitejs/plugin-react` (6) y `@testing-library/jest-dom` (7), además de `rusqlite` 0.40, `tokio` 1.53 y `tauri-plugin-updater` 2.12; el gate Full del 2026-09-23 pasa con ellas. Es una referencia local
reproducible, no una aprobación permanente de actualizar a la última versión.
Antes de cambiar una dependencia, ejecuta los comandos de la tabla y registra el
resultado en el mismo cambio.

### Dependencias directas declaradas

`npm run docs:check` compara esta ficha con `package.json` y
`src-tauri/Cargo.toml` y falla ante cualquier diferencia.

#### npm

| Grupo | Dependencia | Declaración |
| --- | --- | --- |
| runtime | `@tauri-apps/api` | `^2.11.1` |
| runtime | `react` | `^19.3.0` |
| runtime | `react-dom` | `^19.3.0` |
| desarrollo | `@playwright/test` | `^1.63.0` |
| desarrollo | `@tauri-apps/cli` | `^2.11.5` |
| desarrollo | `@testing-library/jest-dom` | `^7.0.1` |
| desarrollo | `@testing-library/react` | `^16.3.3` |
| desarrollo | `@types/node` | `^24.13.6` |
| desarrollo | `@types/react` | `^19.3.0` |
| desarrollo | `@types/react-dom` | `^19.3.0` |
| desarrollo | `@vitejs/plugin-react` | `^6.1.1` |
| desarrollo | `@vitest/coverage-v8` | `^5.0.1` |
| desarrollo | `jsdom` | `^29.1.1` |
| desarrollo | `oxlint` | `1.85.0` |
| desarrollo | `typescript` | `~7.0.2` |
| desarrollo | `vite` | `^8.3.0` |
| desarrollo | `vitest` | `^5.0.1` |

#### Cargo

Las dependencias se resuelven desde `crates.io` mediante `Cargo.lock` y el gate
de supply chain verifica checksums y fuentes. Las versiones declaradas son:

`calamine 0.36.1`, `chrono 0.4.45`, `duckdb 1.10505.0`, `encoding_rs 0.8.41`, `hex 0.4.3`, `polars 0.55.2`, `odbc-api 29.1.0`, `rayon 1.12`, `getrandom 0.3`, `regex 1`, `rusqlite 0.40.2`, `same-file 1.0.6`, `serde 1`, `serde_json 1`, `semver 1.0.28`, `sha2 0.11.0`, `sysinfo 0.39.6`, `tauri 2`, `tauri-plugin-dialog 2.7.3`, `tempfile 3`, `tokio 1.53.1`, `tokio-util 0.7.19`, `unicode-normalization 0.1`, `xxhash-rust 0.8.18`, `zip 8.6.0`;
`tauri-plugin-single-instance 2`, `tauri-plugin-updater 2.12.0` para escritorio, y `tauri-build 2` como dependencia de build.

### Auditorías ejecutadas

Resultados del **2026-10-04** sobre el commit `11dc27d` (DOC-03). Son una
fotografía fechada, no un estado permanente: la evidencia completa queda en
`.local/validation/` y antes de un release se vuelven a ejecutar. Los conteos de
dependencias del lockfile y del inventario IPC los comprueba `docs:check` contra
`package-lock.json` e [`ipc-inventory.json`](docs/reference/ipc-inventory.json).

| Comando | Resultado (2026-10-04, `11dc27d`) | Interpretación |
| --- | --- | --- |
| `npm audit --json --omit=optional` | 0 vulnerabilidades; 200 dependencias del lockfile | Desde OPS-01, una vulnerabilidad hace fallar `supply-chain:check` |
| `cargo audit --json` | 0 vulnerabilidades; avisos informativos (7 crates sin mantenimiento, 1 unsound) con 2 excepciones documentadas | `quick-xml` llega transitivamente por `object_store`; Columnia no habilita los features cloud. La razón vigente está en `src-tauri/deny.toml` |
| `cargo deny --format json check` | Aprobado | Política explícita en `src-tauri/deny.toml`; las excepciones upstream tienen razón y se revisan al actualizar Tauri/Polars |
| `cargo outdated --version` | Herramienta no instalada | No se inventa un estado de actualización Cargo |
| `npm run secrets:check` | Aprobado | Escaneo de los archivos versionados en busca de claves privadas, tokens y credenciales |
| `npm run network:check` | Aprobado | Sin APIs de red/telemetría en producción; CSP solo deja IPC interno |
| `npm run notices:check` | Aprobado | `THIRD_PARTY_NOTICES.md` se deriva offline de ambos lockfiles; aprueba igual en un clon con CRLF (OPS-03) |
| `npm run toolchains:check` | Aprobado; Node 24.14.0, npm 11.10.1 y Rust/Cargo 1.98.1 | Node y npm se comparan con el rango de `engines`; Rust con `rust-toolchain.toml` |
| `npm run ipc:check` | Aprobado; 92 comandos de producción, 4 debug y 84 estructuras compartidas | El inventario se genera desde `generate_handler!` y exige un `invoke` de TypeScript por cada comando de producción |

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
