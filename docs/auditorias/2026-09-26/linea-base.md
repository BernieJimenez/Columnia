# Línea base — 2026-09-26

Commit `5a35244` (`master`, árbol limpio) · Columnia 1.26.0 · Windows 11 x64 · Node 24.14.0 · Rust 1.98.1.
Medida con las herramientas del propio proyecto. Reporte completo del perfil: `full-report.json`.

| Medida | Resultado | Cómo |
|---|---|---|
| Perfil `Full` (fmt, check, lint, Vitest, cobertura, build, presupuesto, Clippy `-D warnings`, tests Rust, secretos, red, cobertura Rust, E2E) | **Aprobado** en 491 s | `tools/check.ps1 -Profile Full` |
| Tests frontend (Vitest) | **493 aprobados** / 0 fallidos / 0 omitidos (57 archivos) | idem |
| Tests Rust | **521 aprobados** / 0 fallidos / **7 ignorados** | idem |
| Tests E2E (Playwright, shell web con IPC simulado) | **27 aprobados** | idem |
| Cobertura frontend | 86,85 % sentencias · 82,11 % ramas · 88,12 % funciones · 90,98 % líneas | `test:coverage` |
| Cobertura Rust (líneas) | **70,51 %** total · `dataset` 68,9 % · `projects` 84,7 % · `remote_databases` **52,7 %** · `updater` **25,1 %** · `resource` 34,0 % | `cargo llvm-cov` |
| Lint (oxlint) | 0 avisos | `npm run lint` |
| Clippy | 0 avisos (`-D warnings`) | perfil Full |
| `npm audit --omit=optional` | 0 vulnerabilidades | |
| `cargo audit` | 2 vulnerabilidades (`quick-xml` 0.39.4, RUSTSEC-2026-0194/0195) con excepción documentada y **verificada no alcanzable**; 5 avisos de crates sin mantenimiento (grafo GTK/Tauri, `bincode`) | `cargo audit` |
| Secretos en el árbol | 0 (641 archivos) | `npm run secrets:check` |
| Secretos en el historial de git (652 commits) | 0 coincidencias de claves privadas ni asignaciones de credenciales | `git log -p -S/-G` |
| Bundle frontend | 803.409 B raw (**98,1 %** del límite de 800 KiB) · 196.897 B gzip (80,1 % de 240 KiB) | presupuesto del perfil Full |
| Runtimes | Node 24 (LTS), Rust 1.98.1, TS 7, Vite 8, React 19.3, Tauri 2.11: ninguno fuera de soporte | `package.json`, `rust-toolchain.toml` |

## Tests ignorados (7)

| Test | Motivo declarado |
|---|---|
| `duckdb_source_backed_join_handles_large_file` | Benchmark opt-in (`perf:duckdb:join`) |
| `benchmark_source_backed_export_cooperative_cancellation` | Benchmark opt-in (`benchmark-datasets.ps1`) |
| `unreachable_server_fails_within_the_login_timeout` | Requiere driver ODBC de SQL Server |
| `external_odbc_round_trip_postgresql` | Requiere servidor PostgreSQL real |
| `external_odbc_round_trip_mysql_with_and_without_no_backslash_escapes` | Requiere servidor MySQL real |
| `external_odbc_append_batching_benchmark_sql_server` | Requiere SQL Server real |
| `external_odbc_round_trip_sql_server` | Requiere SQL Server real |

Cinco de los siete son la entrega ODBC contra bases reales: la suite estándar no la ejerce de punta a punta.
