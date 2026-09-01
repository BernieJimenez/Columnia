# Gobierno del repositorio

Este documento es la referencia operativa de la Fase I0. Describe las reglas
que deben cumplirse antes de integrar cambios en Columnia.

## Contratos del proyecto

| Contrato | Estado | Evidencia |
| --- | --- | --- |
| Licencia MIT | Aprobado | [`LICENSE`](../../LICENSE), `package.json`, `src-tauri/Cargo.toml` |
| Objetivo inicial Windows x64 | Aprobado | [`ADR-0001`](../adr/0001-contratos-del-repositorio.md) |
| SemVer sincronizado | Implementado | `src/version-sync.test.ts` |
| Frontera React/Rust | Aprobado | [`CONTEXTO.md`](../../CONTEXTO.md), `src/bridge.ts`, `src-tauri/src/lib.rs` |
| Validación solo local | Aprobado | [`tools/check.ps1`](../../tools/check.ps1) |
| Fixtures sintéticas sin PII | Aprobado y comprobado | [`fixtures/manifest.json`](../../fixtures/manifest.json), `src/governance.test.ts` |

## Flujo de ramas y commits

`master` es la rama de integración. El trabajo normal usa ramas cortas
`feat/<slug>`, `fix/<slug>`, `docs/<slug>` o `chore/<slug>`. Cada cambio debe
ser acotado, revisable y acompañado por una explicación de riesgo cuando cruce
IPC, filesystem, persistencia, seguridad o empaquetado.

Los commits siguen Conventional Commits y no incluyen datos reales, PII,
secretos, rutas privadas, lockfiles reescritos incidentalmente ni artefactos de
`.local/`. La revisión compara el diff completo y confirma que el árbol está
limpio antes de integrar.

## Gates

```powershell
npm run governance:check
npm run brand:check
.\tools\check.ps1 -Profile Fast
.\tools\check.ps1 -Profile Full
.\tools\check.ps1 -Profile Release
npm run test:coverage
npm run supply-chain:check -- -RequireAuditTools
npm run installer:check
```

`governance:check` valida los documentos, la licencia, el manifiesto de
fixtures y la sincronía de los contratos de gobierno. `Fast` cubre formato,
compilación, tests, TypeScript, build y presupuesto frontend. `Full` añade
Clippy y tests Rust. `Release` añade SBOM, supply chain con herramientas
instaladas, contrato de instalador y build Tauri sin bundle. `Package` se
reserva para cambios de distribución Windows. La cobertura V8 y el inventario
de avisos también se pueden ejecutar de forma independiente.

No hay CI ni workflows automáticos por decisión del proyecto. Una evidencia
local debe conservar el commit, la rama, el estado del árbol, las versiones de
herramientas y los hashes de lockfiles; los reportes de `tools/check.ps1` ya
cumplen ese contrato.

## Cambios que requieren ADR

Registra un ADR antes de integrar cambios que:

- cambien licencia, distribución, plataforma soportada o modelo de costos;
- introduzcan una dependencia arquitectónica, un motor de datos o un servidor;
- amplíen capabilities Tauri, red, telemetría o la frontera de confianza;
- cambien persistencia, contratos IPC, límites de memoria o política de fixtures;
- cambien el proceso de release, firma, actualización o publicación.

## Relación con el contexto vivo

Después de un cambio duradero actualiza [`CONTEXTO.md`](../../CONTEXTO.md) y
[`ROADMAP.md`](../../ROADMAP.md) en el mismo cambio. El código y sus pruebas
siguen teniendo prioridad sobre estos resúmenes si aparece una contradicción.
