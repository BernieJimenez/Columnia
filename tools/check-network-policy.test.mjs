import assert from "node:assert/strict";
import test from "node:test";
import { readFileSync } from "node:fs";
import { findCargoNetworkDependencies, findCspViolations, findPolicyViolations } from "./check-network-policy.mjs";

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

test("rechaza cada API de red del frontend añadida (QA-24)", () => {
  for (const code of [
    "navigator.sendBeacon('/x', data)",
    "new WebTransport(url)",
    "new RTCPeerConnection()",
    "new Image().src = url",
    "await import('https://cdn.example.com/x.js')",
    'globalThis["fe" + "tch"](url)',
    "import posthog from 'posthog-js'",
  ]) {
    assert.ok(findPolicyViolations(code, "src/x.ts").length > 0, code);
  }
  assert.ok(findPolicyViolations('<script src="https://cdn.example.com/a.js"></script>', "index.html").length > 0);
  assert.deepEqual(findPolicyViolations("// la telemetría está prohibida; analytics no se usa", "src/x.ts"), []);
});

test("rechaza cada API de red de Rust añadida (QA-24)", () => {
  for (const code of [
    "use std::net::TcpStream;",
    "let s = TcpStream::connect(addr);",
    "use tokio::net::TcpListener;",
    "use reqwest;",
    "isahc::get(url)",
    "attohttpc::get(url)",
    "tungstenite::connect(url)",
    "native_tls::TlsConnector::new()",
  ]) {
    assert.ok(findPolicyViolations(code, "src-tauri/src/x.rs").length > 0, code);
  }
});

test("acota la excepción del updater a su módulo y a la línea de registro (QA-24)", () => {
  const registration = "builder = builder.plugin(tauri_plugin_updater::Builder::new().build());";
  assert.deepEqual(findPolicyViolations(registration, "src-tauri/src/lib.rs"), []);
  assert.ok(findPolicyViolations("tauri_plugin_updater::UpdaterExt::updater(&app)", "src-tauri/src/lib.rs").length > 0);
  assert.deepEqual(findPolicyViolations("use tauri_plugin_updater::UpdaterExt;", "src-tauri/src/updater.rs"), []);
});

test("rechaza dependencias de red directas en Cargo.toml (QA-24)", () => {
  const cargo = "[dependencies]\nserde = \"1\"\nreqwest = { version = \"0.12\" }\n[dev-dependencies]\nhyper = \"1\"\n";
  assert.deepEqual(findCargoNetworkDependencies(cargo).map((violation) => violation.pattern), ["dependencia de red reqwest"]);
});

test("solo admite 'unsafe-inline' en style-src (QA-24)", () => {
  const config = realConfig();
  config.app.security.csp = "default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'";
  const patterns = findCspViolations(config).map((violation) => violation.pattern);
  assert.ok(patterns.some((pattern) => pattern.includes("script-src 'unsafe-inline'")));
  assert.ok(!patterns.some((pattern) => pattern.includes("style-src")));
});
