import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const script = fileURLToPath(new URL("./check-installer-contract.ps1", import.meta.url));
const realConfig = () => JSON.parse(readFileSync(new URL("../src-tauri/tauri.conf.json", import.meta.url), "utf8"));

function check(config) {
  const root = mkdtempSync(join(tmpdir(), "columnia-installer-"));
  try {
    const path = join(root, "tauri.conf.json");
    writeFileSync(path, JSON.stringify(config));
    return spawnSync("powershell", ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", script, "-ConfigPath", path], { encoding: "utf8" });
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test("el contrato aprueba la configuración real (OPS-14)", () => {
  const run = check(realConfig());
  assert.equal(run.status, 0, run.stderr);
});

test("falla con un instalador por máquina o que no está en español (OPS-14)", () => {
  const withMsi = realConfig();
  withMsi.bundle.targets = ["msi", "nsis"];
  assert.notEqual(check(withMsi).status, 0);
  const english = realConfig();
  english.bundle.windows.nsis.languages = ["English"];
  assert.notEqual(check(english).status, 0);
  const perMachine = realConfig();
  perMachine.bundle.windows.nsis.installMode = "perMachine";
  assert.notEqual(check(perMachine).status, 0);
});
