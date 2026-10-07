import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const modulePath = fileURLToPath(new URL("./git-state.psm1", import.meta.url));

function gitState(root, startCommit) {
  const run = spawnSync("powershell", [
    "-NoProfile",
    "-Command",
    `Import-Module '${modulePath}' -Force; Get-RunGitState -Root '${root}' -StartCommit '${startCommit}' -Branch 'main' -StartDirty $false | ConvertTo-Json -Compress`,
  ], { encoding: "utf8" });
  assert.equal(run.status, 0, run.stderr);
  return JSON.parse(run.stdout);
}

test("modificar un archivo o el commit durante el gate deja dirty=true (OPS-20)", () => {
  const root = mkdtempSync(join(tmpdir(), "columnia-git-state-"));
  try {
    const git = (...args) => execFileSync("git", ["-C", root, "-c", "user.name=t", "-c", "user.email=t@example.invalid", ...args]).toString().trim();
    git("init", "-q");
    writeFileSync(join(root, "a.txt"), "1\n");
    git("add", "a.txt");
    git("commit", "-q", "-m", "inicio");
    const start = git("rev-parse", "HEAD");
    assert.deepEqual(gitState(root, start), { commit: start, branch: "main", dirty: false, changedDuringRun: false, commitAtEnd: start });

    writeFileSync(join(root, "a.txt"), "2\n");
    assert.equal(gitState(root, start).dirty, true);
    assert.equal(gitState(root, start).changedDuringRun, true);

    git("commit", "-q", "-am", "durante el gate");
    const moved = gitState(root, start);
    assert.equal(moved.dirty, true);
    assert.notEqual(moved.commitAtEnd, start);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
