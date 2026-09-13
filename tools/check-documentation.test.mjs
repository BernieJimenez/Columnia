import assert from "node:assert/strict";
import test from "node:test";
import { validateReadmeSetupContract } from "./check-documentation.mjs";

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
