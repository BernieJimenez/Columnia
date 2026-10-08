import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const guardPath = fileURLToPath(new URL("./app-data-guard.ps1", import.meta.url));

// Runs the guard with APPDATA and TEMP inside a scratch folder: the real
// %APPDATA%\app.columnia.desktop is never touched.
function runGuard(root, script) {
  const result = spawnSync("powershell.exe", [
    "-NoProfile", "-ExecutionPolicy", "Bypass", "-Command",
    `[Console]::OutputEncoding = [Text.Encoding]::UTF8; $ErrorActionPreference = 'Stop'; . '${guardPath}'; ${script}`,
  ], {
    encoding: "utf8",
    env: { ...process.env, APPDATA: join(root, "appdata"), TEMP: join(root, "temp"), TMP: join(root, "temp") },
  });
  return { ok: result.status === 0, output: `${result.stdout}${result.stderr}` };
}

function scratch() {
  const root = mkdtempSync(join(tmpdir(), "columnia-guard-test-"));
  mkdirSync(join(root, "temp"));
  mkdirSync(join(root, "appdata", "app.columnia.desktop"), { recursive: true });
  writeFileSync(join(root, "appdata", "app.columnia.desktop", "catalog.json"), "original");
  return root;
}

const store = (root) => join(root, "appdata", "app.columnia.desktop", "catalog.json");

test("restaura el almacén original y retira copia y puntero", () => {
  const root = scratch();
  try {
    const run = runGuard(root, `$g = Backup-ColumniaAppData; Set-Content -LiteralPath '${store(root)}' 'sintetico'; [void](Restore-ColumniaAppData -Guard $g); $g.Backup`);
    assert.ok(run.ok, run.output);
    assert.equal(readFileSync(store(root), "utf8"), "original");
    assert.equal(existsSync(run.output.trim()), false);
    assert.equal(existsSync(join(root, "temp", "columnia-appdata-guard.pending.json")), false);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("un fallo de copia deja el almacén intacto y nombra la copia (OPS-06)", () => {
  const root = scratch();
  try {
    // The backup disappears before restoring: the copy fails.
    const run = runGuard(root, `$g = Backup-ColumniaAppData; Remove-Item -LiteralPath $g.Backup -Recurse -Force; Restore-ColumniaAppData -Guard $g`);
    assert.equal(run.ok, false);
    assert.match(run.output.replace(/\s+/g, " "), /La copia original sigue en/);
    assert.equal(readFileSync(store(root), "utf8"), "original");
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("una ejecución sin restaurar bloquea la siguiente copia (OPS-06)", () => {
  const root = scratch();
  try {
    assert.ok(runGuard(root, "[void](Backup-ColumniaAppData)").ok);
    const second = runGuard(root, "[void](Backup-ColumniaAppData)");
    assert.equal(second.ok, false);
    assert.match(second.output.replace(/\s+/g, " "), /no restauró los datos/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("compara la máquina al terminar: un cambio en APPDATA aunque mida lo mismo (OPS-25)", () => {
  const root = scratch();
  try {
    const run = runGuard(root, `
      $before = Get-ColumniaMachineState
      $same = @(Compare-ColumniaMachineState -Before $before -After (Get-ColumniaMachineState))
      Start-Sleep -Milliseconds 50
      Set-Content -LiteralPath '${store(root)}' 'cambiado' -NoNewline
      $changed = @(Compare-ColumniaMachineState -Before $before -After (Get-ColumniaMachineState))
      [ordered]@{ same = $same.Count; changed = ($changed -join ' | ') } | ConvertTo-Json -Compress
    `);
    assert.ok(run.ok, run.output);
    const result = JSON.parse(run.output.trim().split(/\r?\n/).at(-1));
    assert.equal(result.same, 0);
    assert.match(result.changed, /catalog\.json/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
