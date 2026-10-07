import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import test from "node:test";
import { fileURLToPath } from "node:url";

const modulePath = fileURLToPath(new URL("./process-tree.psm1", import.meta.url));

test("tras un timeout no queda ningún proceso hijo vivo (OPS-21)", () => {
  // Windows PowerShell 5.1, where Process.Kill($true) does not exist.
  const script = `
$ErrorActionPreference = 'Stop'
Import-Module '${modulePath}' -Force
$parent = Start-Process powershell -ArgumentList '-NoProfile','-Command','ping -n 60 127.0.0.1 | Out-Null' -PassThru -WindowStyle Hidden
$child = $null
for ($i = 0; $i -lt 50 -and $null -eq $child; $i++) {
    Start-Sleep -Milliseconds 200
    $child = Get-CimInstance Win32_Process -Filter "ParentProcessId=$($parent.Id)" | Where-Object { $_.Name -eq 'PING.EXE' }
}
if ($null -eq $child) { throw 'el hijo no arrancó' }
Stop-ProcessTree $parent
Start-Sleep -Milliseconds 300
Stop-ProcessTree $parent
$alive = $null -ne (Get-Process -Id $child.ProcessId -ErrorAction SilentlyContinue)
"parentExited=$($parent.HasExited) childAlive=$alive"
`;
  const run = spawnSync("powershell", ["-NoProfile", "-Command", script], { encoding: "utf8", timeout: 60_000 });
  assert.equal(run.status, 0, run.stderr);
  assert.match(run.stdout, /parentExited=True childAlive=False/);
});
