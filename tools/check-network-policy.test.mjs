import assert from "node:assert/strict";
import test from "node:test";
import { findPolicyViolations } from "./check-network-policy.mjs";

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
