import assert from "node:assert/strict";
import test from "node:test";
import { readFileSync } from "node:fs";
import { findCspViolations, findPolicyViolations } from "./check-network-policy.mjs";

const realConfig = () => JSON.parse(readFileSync(new URL("../src-tauri/tauri.conf.json", import.meta.url), "utf8"));

test("aprueba la CSP real de producción y desarrollo", () => {
  assert.deepEqual(findCspViolations(realConfig()), []);
});

test("rechaza un origen externo añadido a la CSP de producción", () => {
  const config = realConfig();
  config.app.security.csp["connect-src"] += " https://example.com";
  const violations = findCspViolations(config);
  assert.equal(violations.length, 1);
  assert.match(violations[0].pattern, /connect-src https:\/\/example\.com/);
});

test("rechaza comodines y esquemas amplios aunque la CSP sea texto", () => {
  const config = realConfig();
  config.app.security.csp = "default-src 'self'; img-src * https:";
  const patterns = findCspViolations(config).map((violation) => violation.pattern);
  assert.ok(patterns.some((pattern) => pattern.endsWith("img-src *")));
  assert.ok(patterns.some((pattern) => pattern.endsWith("img-src https:")));
});

test("exige que exista la CSP", () => {
  const config = realConfig();
  delete config.app.security.csp;
  assert.match(findCspViolations(config)[0].pattern, /ausente/);
});

test("limita las salidas ODBC y updater a sus módulos declarados", () => {
  assert.deepEqual(findPolicyViolations("odbc_api::Environment::new()", "src-tauri/src/remote_databases.rs"), []);
  const violations = findPolicyViolations("odbc_api::Environment::new()", "src-tauri/src/dataset.rs");
  assert.equal(violations.length, 1);
  assert.match(violations[0].pattern, /odbc_api fuera de/);
});

test("permite fetch del cursor ODBC en pruebas Rust", () => {
  assert.deepEqual(
    findPolicyViolations("while let Some(batch) = cursor.fetch()? {}", "src-tauri/src/remote_databases.rs"),
    [],
  );
});

test("rechaza fetch web en TypeScript de producción", () => {
  const violations = findPolicyViolations("const response = await fetch('/api');", "src/api.ts");
  assert.equal(violations.length, 1);
  assert.match(violations[0].pattern, /fetch/);
});

test("rechaza clientes HTTP Rust de producción", () => {
  const violations = findPolicyViolations("let client = reqwest::Client::new();", "src-tauri/src/client.rs");
  assert.equal(violations.length, 1);
  assert.match(violations[0].pattern, /reqwest/);
});
