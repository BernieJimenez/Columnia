import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const projectRoot = fileURLToPath(new URL("..", import.meta.url));

// OPS-11: Windows PowerShell 5.1 reads a script without BOM as ANSI, so every
// accent of its messages came out garbled («estÃ¡»). A BOM makes it read UTF-8.
test("cada script de PowerShell con texto no ASCII lleva BOM UTF-8 (OPS-11)", () => {
  const scripts = execFileSync("git", ["ls-files", "--cached", "--others", "--exclude-standard", "*.ps1", "*.psm1"], { cwd: projectRoot })
    .toString("utf8")
    .split(/\r?\n/)
    .filter(Boolean);
  assert.ok(scripts.length > 0);
  const withoutBom = scripts.filter((script) => {
    const bytes = readFileSync(join(projectRoot, script));
    const hasBom = bytes[0] === 0xef && bytes[1] === 0xbb && bytes[2] === 0xbf;
    return !hasBom && bytes.some((byte) => byte > 0x7f);
  });
  assert.deepEqual(withoutBom, []);
  // OPS-11: with the BOM the literals decode as UTF-8, so the files they are
  // compared with must be read as UTF-8 too, not as ANSI.
  const ansiReads = scripts.flatMap((script) => readFileSync(join(projectRoot, script), "utf8")
    .split(/\r?\n/)
    .map((line, index) => ({ line, at: `${script}:${index + 1}` }))
    .filter(({ line }) => /\b(?:Get-Content|Import-Csv|Select-String)\b/.test(line) && !/-Encoding/i.test(line) && !line.trim().startsWith("#"))
    .map(({ at }) => at));
  assert.deepEqual(ansiReads, []);
});

test("ningún literal de ruta lleva un tabulador en lugar de «\\t» (OPS-11)", () => {
  const scripts = execFileSync("git", ["ls-files", "--cached", "--others", "--exclude-standard", "*.ps1", "*.psm1"], { cwd: projectRoot })
    .toString("utf8")
    .split(/\r?\n/)
    .filter(Boolean);
  const tabs = scripts.filter((script) => /"[^"\n]*\t[^"\n]*"/.test(readFileSync(join(projectRoot, script), "utf8")));
  assert.deepEqual(tabs, []);
});
