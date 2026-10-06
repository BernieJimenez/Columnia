import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const projectRoot = resolve(fileURLToPath(new URL("..", import.meta.url)));

/** Runs the project's lint, as `npm run lint` does, on one file. */
function lint(source) {
  const directory = mkdtempSync(join(tmpdir(), "columnia-lint-"));
  try {
    const file = join(directory, "Example.tsx");
    writeFileSync(file, source);
    return spawnSync(process.execPath, [
      join(projectRoot, "node_modules", "oxlint", "bin", "oxlint"),
      "-c", join(projectRoot, ".oxlintrc.json"), "--deny-warnings", file,
    ], { encoding: "utf8" });
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
}

test("una dependencia ausente en un efecto rompe el lint (COD-06)", () => {
  const missing = lint(`import { useEffect, useState } from "react";
export function Example({ id }: { id: string }) {
  const [value, setValue] = useState("");
  useEffect(() => { setValue(id); }, []);
  return <p>{value}</p>;
}
`);
  assert.notEqual(missing.status, 0, missing.stdout);
  const complete = lint(`import { useEffect, useState } from "react";
export function Example({ id }: { id: string }) {
  const [value, setValue] = useState("");
  useEffect(() => { setValue(id); }, [id]);
  return <p>{value}</p>;
}
`);
  assert.equal(complete.status, 0, complete.stdout);
});
