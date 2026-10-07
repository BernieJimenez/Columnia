import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import test from "node:test";
import { fileURLToPath } from "node:url";

const modulePath = fileURLToPath(new URL("./supply-chain.psm1", import.meta.url));
const denyPath = fileURLToPath(new URL("../src-tauri/deny.toml", import.meta.url));

function powershell(body) {
  const run = spawnSync(
    "powershell",
    ["-NoProfile", "-Command", `$ErrorActionPreference = 'Stop'; Import-Module '${modulePath}' -Force; ${body}`],
    { encoding: "utf8" },
  );
  return { ...run, stdout: run.stdout.trim() };
}

test("el resumen distingue omitido de correcto (OPS-17)", () => {
  const status = (results, failure = "$null") => powershell(
    `$r = [ordered]@{}; ${results}; (Get-SupplyChainStatus $r ${failure}).status`,
  ).stdout;
  assert.equal(status("$r.npmAudit = @{ status = 'passed' }; $r.cargoAudit = @{ status = 'passed' }"), "passed");
  assert.equal(status("$r.npmAudit = @{ status = 'passed' }; $r.cargoAudit = @{ status = 'unavailable' }"), "passed-with-skips");
  assert.equal(status("$r.npmAudit = @{ status = 'failed' }; $r.cargoAudit = @{ status = 'unavailable' }"), "failed");
  assert.equal(status("$r.npmAudit = @{ status = 'passed' }", "'npm audit falló'"), "failed");
});

test("las excepciones de cargo audit salen de deny.toml y cada una tiene motivo (OPS-17)", () => {
  const listed = powershell(`(Get-DenyIgnoredAdvisories (Get-Content -Encoding UTF8 -LiteralPath '${denyPath}' -Raw)) -join ','`);
  assert.equal(listed.status, 0, listed.stderr);
  assert.ok(listed.stdout.split(",").includes("RUSTSEC-2026-0194"));
  const withoutReason = powershell(`Get-DenyIgnoredAdvisories '{ id = "RUSTSEC-2026-0001" }'`);
  assert.notEqual(withoutReason.status, 0);
  assert.match(withoutReason.stderr, /RUSTSEC-2026-0001/);
});
