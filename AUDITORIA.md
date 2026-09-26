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

Ficha de dependencias regenerada el **2026-09-23** sobre `1.26.0` desde los manifiestos. El commit `cb86ea5` (2026-09-20) subió versiones mayores de TypeScript (7), Vite (8), Vitest y su cobertura (5), jsdom (29), `@vitejs/plugin-react` (6) y `@testing-library/jest-dom` (7), además de `rusqlite` 0.40, `tokio` 1.53 y `tauri-plugin-updater` 2.12; el gate Full del 2026-09-23 pasa con ellas. El inventario IPC quedó verificado y sincronizado el **2026-09-22**. Es una referencia local
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

`calamine 0.36.1`, `chrono 0.4.45`, `duckdb 1.10505.0`, `hex 0.4.3`, `polars 0.55.2`, `odbc-api 29.1.0`, `rayon 1.12`, `regex 1`, `rusqlite 0.40.2`, `serde 1`, `serde_json 1`, `semver 1.0.28`, `sha2 0.11.0`, `sysinfo 0.39.6`, `tauri 2`, `tauri-plugin-dialog 2.7.3`, `tempfile 3`, `tokio 1.53.1`, `tokio-util 0.7.19`, `unicode-normalization 0.1`, `xxhash-rust 0.8.18`, `zip 8.6.0`;
`tauri-plugin-single-instance 2`, `tauri-plugin-updater 2.12.0` para escritorio, y `tauri-build 2` como dependencia de build.

### Auditorías ejecutadas

| Comando | Resultado del snapshot | Interpretación |
| --- | --- | --- |
| `npm audit --json --omit=optional` | 0 vulnerabilidades reportadas; 200 dependencias del lockfile | Reauditado el 2026-09-24 tras añadir `oxlint`; repetir antes de release |
| `cargo audit --json` | `cargo-audit 0.22.2`; 0 vulnerabilidades después de las excepciones documentadas; los avisos informativos no son bloqueantes | `quick-xml 0.39.4` llega transitivamente por `object_store 0.13.2`; Columnia no habilita los features cloud ni expone un flujo remoto. La razón vigente está en `src-tauri/deny.toml` |
| `cargo deny --format json check` | `cargo-deny 0.20.2`; advisories/licencias/fuentes sin errores; 48 duplicados en warning | Política explícita en `src-tauri/deny.toml`; excepciones upstream tienen razón y se revisan al actualizar Tauri/Polars |
| `cargo outdated --version` | Herramienta no instalada | No se inventa un estado de actualización Cargo |
| `npm run secrets:check` | 0 hallazgos; 477 archivos inspeccionados | Escaneo local de claves privadas, tokens y credenciales asignadas |
| `npm run network:check` | Aprobado | Sin APIs de red/telemetría en producción; CSP solo deja IPC interno |
| `npm run notices:check` | Aprobado; 999 identidades de dependencia sin `UNKNOWN`, sin filas duplicadas | `THIRD_PARTY_NOTICES.md` se deriva offline de ambos lockfiles y rechaza licencias desconocidas, contradictorias o incompletas |
| `npm run toolchains:check` | Aprobado; Node 24.14.0, npm 11.10.1 y Rust/Cargo 1.98.1 | Las versiones exactas están fijadas en `package.json` y `rust-toolchain.toml` |
| `npm run ipc:check` | Aprobado; 85 comandos de producción, 4 debug y 70 estructuras compartidas | El inventario se genera desde `generate_handler!` y se publica en [`ipc-inventory.json`](docs/reference/ipc-inventory.json); incluye tareas reutilizables, catálogo de proyectos/candidato de recuperación combinado, preflight/presets de entrega, inspección de libros en dos pasos y updater autenticado |

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
