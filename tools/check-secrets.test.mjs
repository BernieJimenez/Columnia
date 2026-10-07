import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const script = fileURLToPath(new URL("./check-secrets.ps1", import.meta.url));

// The fixtures are assembled at run time, so this file holds no secret itself.
const fixtures = {
  "slack.md": `token ${"xoxb"}-${"1234567890"}-abcdefghijklmnop`,
  "anthropic.ts": `const key = "${"sk"}-ant-api03-${"A".repeat(24)}";`,
  "openai.py": `KEY = "${"sk"}-proj-${"b".repeat(24)}"`,
  "github.txt": `${"ghp"}_${"c".repeat(36)}`,
  "minisign.key": `untrusted comment: ${"rsign"} encrypted secret key\nRWRTY0Iy`,
  // The literal is split so this file is not itself a signing key to scanners.
  // oxlint-disable-next-line no-useless-concat
  "tauri.conf.txt": `TAURI_SIGNING_PRIVATE_KEY=dW50cnVzdGVkIGNvbW1lbnQ6${"IHJzaWduIGVuY3J5cHRl" + "ZCBzZWNyZXQga2V5"}Cg==`,
  ".env": `DATABASE_PASSWORD=${"hunter2hunter2"}`,
  "server.pem": `-----BEGIN ${"ENCRYPTED"} PRIVATE KEY-----\nMIIE`,
  "pgp.asc": `-----BEGIN PGP ${"PRIVATE"} KEY BLOCK-----`,
};

function scan(files) {
  const root = mkdtempSync(join(tmpdir(), "columnia-secrets-"));
  try {
    for (const [name, contents] of Object.entries(files)) writeFileSync(join(root, name), contents);
    const output = join(root, "resultado.json");
    const run = spawnSync(
      "powershell",
      ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", script, "-ScanRoot", root, "-OutputPath", output],
      { encoding: "utf8" },
    );
    const report = JSON.parse(readFileSync(output, "utf8").replace(/^﻿/, ""));
    return { status: run.status, report };
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test("detecta cada formato de secreto de las fixtures (SEG-03)", () => {
  const { status, report } = scan(fixtures);
  assert.notEqual(status, 0);
  assert.equal(report.status, "failed");
  const flagged = new Set(report.hits.map((hit) => hit.file));
  for (const name of Object.keys(fixtures)) {
    assert.ok(flagged.has(name), `${name} no se detectó: ${JSON.stringify(report.hits)}`);
  }
  const rules = new Set(report.hits.map((hit) => hit.rule));
  for (const rule of ["provider_token", "signing_secret_key", "env_secret", "private_key"]) {
    assert.ok(rules.has(rule), `falta la regla ${rule}`);
  }
});

test("aprueba texto normal y código con nombres en mayúsculas", () => {
  const { status, report } = scan({
    "notas.md": "La clave pública del updater es visible en tauri.conf.json.",
    "core.py": "TOKEN_RE = re.compile(r\"^[a-z0-9]+$\")\nquery_tokens = self.tokenize(query)",
  });
  assert.equal(status, 0);
  assert.equal(report.status, "passed");
});

function scanRepository(root) {
  // QA-39: the output folder is removed too; it used to stay in %TEMP%.
  const outputDirectory = mkdtempSync(join(tmpdir(), "columnia-secrets-out-"));
  const output = join(outputDirectory, "resultado.json");
  const run = spawnSync(
    "powershell",
    ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", script, "-RepositoryRoot", root, "-OutputPath", output],
    { encoding: "utf8" },
  );
  let report = null;
  try {
    report = JSON.parse(readFileSync(output, "utf8").replace(/^﻿/, ""));
  } catch {
    // No report: the script stopped before scanning.
  } finally {
    rmSync(outputDirectory, { recursive: true, force: true });
  }
  return { status: run.status, report };
}

test("falla si git no puede listar el repositorio (OPS-05)", () => {
  const notARepository = mkdtempSync(join(tmpdir(), "columnia-sin-git-"));
  try {
    const { status, report } = scanRepository(notARepository);
    assert.notEqual(status, 0);
    assert.notEqual(report?.status, "passed");
  } finally {
    rmSync(notARepository, { recursive: true, force: true });
  }
});

test("revisa archivos con tildes en el nombre (OPS-05)", () => {
  const repository = mkdtempSync(join(tmpdir(), "columnia-git-"));
  try {
    spawnSync("git", ["init", "-q"], { cwd: repository });
    writeFileSync(join(repository, "ñ.ts"), `const t = "${"ghp"}_${"d".repeat(36)}";`);
    const { status, report } = scanRepository(repository);
    assert.notEqual(status, 0);
    assert.ok(report.hits.some((hit) => hit.file === "ñ.ts"), JSON.stringify(report.hits));
  } finally {
    rmSync(repository, { recursive: true, force: true });
  }
});

