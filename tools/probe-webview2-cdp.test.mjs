import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const script = fileURLToPath(new URL("./probe-webview2-cdp.ps1", import.meta.url));
const pendingGuard = join(tmpdir(), "columnia-appdata-guard.pending.json");

test("una fase de reinicio suelta no arranca sin el respaldo del appdata (OPS-22)", { skip: existsSync(pendingGuard) && "hay un respaldo pendiente" }, () => {
  const run = spawnSync("powershell", [
    "-NoProfile",
    "-ExecutionPolicy",
    "Bypass",
    "-File",
    script,
    "-RunProjects",
    "-ProjectProbeMode",
    "restart-prepare",
    "-ProjectProbeTaskName",
    `__columnia_native_probe__restart_${"0".repeat(32)}`,
  ], { encoding: "utf8", timeout: 60_000 });
  assert.notEqual(run.status, 0);
  assert.match(run.stderr + run.stdout, /probe-webview2-restart\.ps1/);
  assert.equal(existsSync(pendingGuard), false);
});
