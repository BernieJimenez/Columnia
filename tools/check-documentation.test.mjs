import assert from "node:assert/strict";
import test from "node:test";
import { validateChangelogVersion, validateDependencySnapshot, validateReadmeSetupContract } from "./check-documentation.mjs";

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
