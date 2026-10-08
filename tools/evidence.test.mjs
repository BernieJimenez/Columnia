import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const projectRoot = fileURLToPath(new URL("..", import.meta.url));
const modulePath = join(projectRoot, "tools", "evidence.psm1");

function powershell(script) {
  const result = spawnSync("powershell.exe", ["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", `[Console]::OutputEncoding = [Text.Encoding]::UTF8\n${script}`], { encoding: "utf8" });
  assert.equal(result.status, 0, result.stderr);
  return result.stdout;
}

// OPS-24: every summary.json carries the same header with commit, tree,
// version and times, and a gate refuses evidence of another commit.
test("la cabecera común dice commit, árbol, versión y horas (OPS-24)", () => {
  const output = powershell(`
    Import-Module '${modulePath}' -Force
    [ordered]@{ status = 'passed' } | Add-EvidenceHeader -Root '${projectRoot}' -StartedAt '2026-10-08T10:00:00.000Z' | ConvertTo-Json -Depth 4 -Compress
  `);
  const summary = JSON.parse(output);
  const head = execFileSync("git", ["rev-parse", "HEAD"], { cwd: projectRoot }).toString("utf8").trim();
  const version = JSON.parse(readFileSync(join(projectRoot, "package.json"), "utf8")).version;
  assert.equal(summary.status, "passed");
  assert.equal(summary.evidence.contract, "columnia-evidence-header");
  assert.equal(summary.evidence.commit, head);
  assert.equal(typeof summary.evidence.dirty, "boolean");
  assert.equal(summary.evidence.version, version);
  assert.equal(summary.evidence.startedAt, "2026-10-08T10:00:00.000Z");
  assert.ok(Date.parse(summary.evidence.finishedAt) > 0);
});

test("la evidencia de otro commit, con cambios o sin cabecera no cuenta (OPS-24)", () => {
  const output = powershell(`
    Import-Module '${modulePath}' -Force
    $Head = 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'
    $Same = [pscustomobject]@{ evidence = [pscustomobject]@{ commit = $Head; dirty = $false } }
    $Other = [pscustomobject]@{ evidence = [pscustomobject]@{ commit = 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb'; dirty = $false } }
    $Dirty = [pscustomobject]@{ evidence = [pscustomobject]@{ commit = $Head; dirty = $true } }
    $Missing = [pscustomobject]@{ status = 'passed' }
    @(
      [string](Get-EvidenceCommitProblem -Document $Same -HeadCommit $Head),
      (Get-EvidenceCommitProblem -Document $Other -HeadCommit $Head),
      (Get-EvidenceCommitProblem -Document $Dirty -HeadCommit $Head),
      (Get-EvidenceCommitProblem -Document $Missing -HeadCommit $Head)
    ) | ConvertTo-Json -Compress
  `);
  const [same, other, dirty, missing] = JSON.parse(output);
  assert.equal(same, "");
  assert.match(other, /otro commit \(bbbbbbb\)/);
  assert.match(dirty, /cambios sin commit/);
  assert.match(missing, /cabecera común/);
});

test("cada script que escribe un summary.json le pone la cabecera común (OPS-24)", () => {
  const scripts = execFileSync("git", ["ls-files", "tools/*.ps1"], { cwd: projectRoot }).toString("utf8").split(/\r?\n/).filter(Boolean);
  const writers = scripts.filter((script) => /Join-Path \$\w+ "summary\.json"/.test(readFileSync(join(projectRoot, script), "utf8")));
  assert.ok(writers.length >= 10, writers.join(", "));
  const without = writers.filter((script) => {
    const contents = readFileSync(join(projectRoot, script), "utf8");
    return !contents.includes('Import-Module (Join-Path $PSScriptRoot "evidence.psm1")')
      || !/Add-EvidenceHeader -Root \$ProjectRoot -StartedAt \$EvidenceStartedAt \| ConvertTo-Json/.test(contents);
  });
  assert.deepEqual(without, []);
});
