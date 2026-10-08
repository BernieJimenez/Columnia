import assert from "node:assert/strict";
import test from "node:test";
import { readFileSync } from "node:fs";
import {
  backtickRepositoryPaths,
  betaDocumentProblems,
  contributionPolicyProblems,
  isLivingDocument,
  missingDocsIndexLinks,
  missingNpmScripts,
  readmeRustProblems,
  retiredLabels,
  staleVersionExamples,
  parseCargoDependencies,
  validateChangelogVersion,
  validateDependencySnapshot,
  validateReadmeSetupContract,
} from "./check-documentation.mjs";

test("la ficha de dependencias debe coincidir con ambos manifiestos", () => {
  const manifest = { dependencies: { react: "^19.3.0" }, devDependencies: { vite: "^8.3.0" } };
  const cargo = '[dependencies]\npolars = { version = "0.55.2", features = ["csv"] }\nserde = "1"\n\n[dev-dependencies]\nproptest = "1"\n';
  const sheet = (reactVersion) => [
    "| runtime | `react` | `" + reactVersion + "` |",
    "| desarrollo | `vite` | `^8.3.0` |",
    "#### Cargo",
    "`polars 0.55.2`, `serde 1`.",
    "### Snapshot",
  ].join("\n");
  assert.deepEqual(validateDependencySnapshot(sheet("^19.3.0"), manifest, cargo), []);
  assert.match(validateDependencySnapshot(sheet("^19.1.0"), manifest, cargo)[0], /react: manifiesto \^19\.3\.0, ficha \^19\.1\.0/);
  assert.match(validateDependencySnapshot(sheet("^19.3.0"), manifest, cargo.replace('serde = "1"', 'serde = "1"\nduckdb = "1.10505.0"')).join(), /duckdb: manifiesto 1\.10505\.0, ficha ausente/);
});

test("una mención en prosa no cuenta como sección de versión", () => {
  const prose = "# Changelog\n\nLa versión vigente es [1.26.0].\n\n## [1.25.0] - 2026-09-21\n";
  assert.throws(() => validateChangelogVersion(prose, "1.26.0"), /ni "## \[Unreleased\]"/);
});

test("desarrollo acepta cambios pendientes bajo Unreleased; publicación exige la sección", () => {
  const pending = "# Changelog\n\n## [Unreleased]\n\n## [1.25.0] - 2026-09-21\n";
  assert.doesNotThrow(() => validateChangelogVersion(pending, "1.26.0"));
  assert.throws(() => validateChangelogVersion(pending, "1.26.0", { requireReleaseSection: true }), /exige la publicación/);
  const released = "# Changelog\n\n## [1.26.0] - 2026-09-30\n";
  assert.doesNotThrow(() => validateChangelogVersion(released, "1.26.0", { requireReleaseSection: true }));
});

const packageManifest = {
  engines: { node: ">=24.14.0 <25", npm: ">=11.10.1 <12" },
  scripts: {
    build: "tsc --noEmit && vite build",
    test: "vitest run",
    "docs:check": "node tools/check-documentation.mjs",
    "legal:check": "node tools/check-legal-distribution.mjs",
  },
};

const validReadme = `## Ejecutar desde el código fuente

Requisitos: Node.js \`${packageManifest.engines.node}\`, npm \`${packageManifest.engines.npm}\`, Rust estable.

~~~bash
npm install
npm run build
npm test
npm run docs:check
npm run legal:check
~~~
`;

test("acepta requisitos y comandos del README que coinciden con package.json", () => {
  assert.doesNotThrow(() => validateReadmeSetupContract(validReadme, packageManifest));
});

test("detecta requisitos de Node.js o npm que no coinciden con engines", () => {
  const outdatedReadme = validReadme.replace("Node.js `>=24.14.0 <25`", "Node.js 22+");
  assert.throws(() => validateReadmeSetupContract(outdatedReadme, packageManifest), /Node\.js .*package\.json engines\.node/);
});

test("rechaza un comando npm run que no existe en package.json", () => {
  const invalidReadme = validReadme.replace("npm run build", "npm run check");
  assert.throws(() => validateReadmeSetupContract(invalidReadme, packageManifest), /npm run check.*no define ese script/);
});

test("rechaza npm run sin nombre de script", () => {
  const invalidReadme = validReadme.replace("npm run build", "npm run");
  assert.throws(() => validateReadmeSetupContract(invalidReadme, packageManifest), /npm run.*sin nombre/);
});

test("los enlaces dentro de bloques o código en línea no se validan (OPS-09)", async () => {
  const { localLinkTargets } = await import("./check-documentation.mjs");
  const markdown = [
    "Ver [guía](docs/guia.md#inicio) y [con espacios](docs/mi%20archivo.md \"título\").",
    "Código `arr[0](x)` en línea.",
    "```js",
    "const link = \"[a](no-existe.md)\";",
    "```",
    "[externo](https://example.com)",
  ].join("\n");
  assert.deepEqual(localLinkTargets(markdown), ["docs/guia.md", "docs/mi archivo.md"]);
});

test("los parciales de auditoría y el archivo no se revisan por enlaces (OPS-09)", async () => {
  const { skipsLinkCheck } = await import("./check-documentation.mjs");
  assert.equal(skipsLinkCheck("docs/auditorias/2026-10-01/parciales/a.md"), true);
  assert.equal(skipsLinkCheck("docs/archive/v0.md"), true);
  assert.equal(skipsLinkCheck("docs/auditorias/2026-10-01/AUDITORIA.md"), false);
});

test("rechaza cifras de inventario escritas a mano (DOC-01)", async () => {
  const { handWrittenInventoryFigures } = await import("./check-documentation.mjs");
  assert.deepEqual(handWrittenInventoryFigures("Registra 92 comandos de producción y 84 estructuras compartidas."), ["92 comandos de producción", "84 estructuras compartidas"]);
  assert.deepEqual(handWrittenInventoryFigures("Las cifras vigentes están en el inventario."), []);
});

test("cada variable COLUMNIA_* del código está en la tabla única (DOC-10)", async () => {
  const { undocumentedEnvironmentVariables } = await import("./check-documentation.mjs");
  // Built at run time so this file does not name an undocumented variable.
  const unlisted = ["COLUMNIA", "NUEVA"].join("_");
  const sources = ['option_env!("COLUMNIA_UPDATER_ENDPOINT")', `process.env.${unlisted}; window.__COLUMNIA_E2E_CALLS__`];
  assert.deepEqual(undocumentedEnvironmentVariables(sources, "| `COLUMNIA_UPDATER_ENDPOINT` | ..."), [unlisted]);
});

test("los subcomandos de cli.md coinciden con la ayuda de la CLI (DOC-14)", async () => {
  const { cliSubcommandProblems } = await import("./check-documentation.mjs");
  const source = 'const GENERAL_HELP: &str = "USO:\\n  columnia-cli inspect --input <ruta>\\n  columnia-cli batch --manifest <ruta>\\n";';
  assert.deepEqual(cliSubcommandProblems("### `inspect`\n\n### `batch`\n", source), []);
  assert.deepEqual(cliSubcommandProblems("### `inspect`\n\n### `viejo`\n", source), ["batch falta en cli.md", "viejo no existe en la CLI"]);
});

test("lee tablas de dependencia, workspace y git/path, y descarta el workspace (COD-19)", () => {
  const cargo = [
    "[workspace.dependencies]",
    'shared = "9"',
    "[dependencies]",
    "local = { path = \"../local\" }",
    "inherited = { workspace = true }",
    "remote = { git = \"https://example.invalid/remote\" }",
    "[dependencies.polars]",
    'version = "0.55.2"',
    'features = ["csv"]',
    "[target.'cfg(windows)'.dependencies]",
    'windows = "0.61"',
    "[dev-dependencies]",
    'proptest = "1"',
  ].join("\n");
  assert.deepEqual(Object.fromEntries(parseCargoDependencies(cargo)), {
    local: "path",
    inherited: "workspace",
    remote: "git",
    polars: "0.55.2",
    windows: "0.61",
  });
});

const readDoc = (path) => readFileSync(new URL(`../${path}`, import.meta.url), "utf8");

test("reformular la prosa Beta no rompe el gate, quitar su estructura sí (QA-49)", () => {
  const guide = readDoc("docs/how-to/run-beta-validation.md");
  const session = readDoc("docs/templates/beta-session.md");
  const summary = readDoc("docs/templates/beta-summary.md");
  assert.deepEqual(betaDocumentProblems(guide, session, summary), []);
  const rephrased = guide.replace("24 de las 30 tareas agregadas", "al menos el 80 % de las tareas").replace("dataset-01", "conjunto-a");
  assert.deepEqual(betaDocumentProblems(rephrased, session, summary), []);
  const problems = betaDocumentProblems(guide.replace("## Cuándo una sesión cuenta", "## Validez"), session, summary);
  assert.equal(problems.length, 1);
  assert.match(problems[0], /run-beta-validation\.md: ## Cuándo una sesión cuenta/);
});

test("el índice de docs nombra el enlace Diátaxis que falta (QA-49)", () => {
  const index = readDoc("docs/README.md");
  assert.deepEqual(missingDocsIndexLinks(index), []);
  assert.deepEqual(missingDocsIndexLinks(index.replaceAll("reference/cli.md", "reference/otro.md")), ["reference/cli.md"]);
});

test("README, CONTRIBUTING y SECURITY dicen lo mismo sobre contribuciones (DOC-11)", () => {
  const documents = Object.fromEntries(["README.md", "CONTRIBUTING.md", "SECURITY.md"].map((name) => [name, readDoc(name)]));
  assert.deepEqual(contributionPolicyProblems(documents), []);
  const welcoming = { ...documents, "README.md": "Los reportes de errores y las mejoras son bienvenidos." };
  assert.deepEqual(contributionPolicyProblems(welcoming), ["README.md"]);
});

test("el README pide la versión de Rust que fija rust-toolchain.toml (DOC-07)", () => {
  const toolchain = '[toolchain]\nchannel = "1.98.1"\n';
  assert.deepEqual(readmeRustProblems("Requisitos: Rust `1.98.1` (rustup).", toolchain), []);
  assert.equal(readmeRustProblems("Requisitos: Rust estable.", toolchain).length, 1);
});

test("los documentos vivos solo citan scripts npm que existen (DOC-09)", () => {
  assert.deepEqual(missingNpmScripts("npm run build y npm run perf:i1:check", { build: "vite build" }), ["perf:i1:check"]);
  assert.equal(isLivingDocument("CONTEXTO.md"), true);
  assert.equal(isLivingDocument("CHANGELOG.md"), false);
  assert.equal(isLivingDocument("docs/archive/2026-09/ROADMAP.md"), false);
});

test("las rutas entre backticks se pueden comprobar (DOC-12)", () => {
  assert.deepEqual(
    backtickRepositoryPaths("ver `docs/reference/cli.md`, `tools/<script>.mjs` y `docs/archive/2026-09/x.md`"),
    ["docs/reference/cli.md", "docs/archive/2026-09/x.md"],
  );
  assert.deepEqual(backtickRepositoryPaths("copia a `docs/reference/beta-v1-summary.md`"), []);
});

test("las guías no usan otra versión como ejemplo (DOC-15)", () => {
  const guide = "--expected-version 0.95.0 y https://x/columnia/1.26.0/ y Columnia_<versión>_x64";
  assert.deepEqual(staleVersionExamples(guide, "1.26.0"), ["0.95.0"]);
  assert.deepEqual(staleVersionExamples("--expected-version <versión>", "1.26.0"), []);
});

test("Tier y Gate 2 solo aparecen en bloques archivados (DOC-16)", () => {
  assert.deepEqual(retiredLabels("## Uso\n\nTier 8 se cierra.\n\n## Gate 2 (archivado)\n\nGate 2 y Tier 5.\n"), ["Tier 8"]);
});
