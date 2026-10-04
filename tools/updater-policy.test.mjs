import assert from "node:assert/strict";
import test from "node:test";

import { MAX_ARTIFACT_BYTES, readLimitedBody, updaterPolicyProblems } from "./updater-policy.mjs";

const valid = {
  manifestVersion: "1.26.0",
  expectedVersion: "1.26.0",
  artifactUrl: "https://updates.example.invalid/columnia/Columnia_1.26.0_x64-setup.exe",
  allowedHosts: ["updates.example.invalid"],
  sizeBytes: 1024,
};

test("aprueba el manifiesto de la versión que se publica, en un host permitido", () => {
  assert.deepEqual(updaterPolicyProblems(valid), []);
  assert.deepEqual(updaterPolicyProblems({ ...valid, manifestVersion: "v1.26.0" }), []);
});

test("rechaza una versión inferior (downgrade) o distinta (SEG-04)", () => {
  assert.match(updaterPolicyProblems({ ...valid, manifestVersion: "1.24.0" })[0], /declara 1\.24\.0/);
});

test("rechaza un host ajeno, HTTP y la falta de hosts permitidos", () => {
  assert.match(
    updaterPolicyProblems({ ...valid, artifactUrl: "https://cdn.attacker.invalid/x.exe" }).join(" "),
    /no es un host permitido/,
  );
  assert.match(updaterPolicyProblems({ ...valid, artifactUrl: "http://updates.example.invalid/x.exe" }).join(" "), /HTTPS/);
  assert.match(updaterPolicyProblems({ ...valid, allowedHosts: [] }).join(" "), /No hay hosts permitidos/);
});

test("rechaza tamaños inválidos o mayores que el límite", () => {
  assert.equal(updaterPolicyProblems({ ...valid, sizeBytes: MAX_ARTIFACT_BYTES + 1 }).length, 1);
  assert.equal(updaterPolicyProblems({ ...valid, sizeBytes: 0 }).length, 1);
});

test("deja de leer una descarga que supera el límite", async () => {
  const chunks = [Buffer.alloc(600), Buffer.alloc(600)];
  const response = {
    headers: new Map(),
    body: (async function* () { yield* chunks; })(),
  };
  await assert.rejects(readLimitedBody(response, 1000, "el instalador"), /supera el límite de 1000 bytes/);
  const declared = { headers: new Map([["content-length", "5000"]]), body: [] };
  await assert.rejects(readLimitedBody(declared, 1000, "el manifiesto"), /declara 5000 bytes/);
});
