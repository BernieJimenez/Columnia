// QA-34: runs the frontend suite as on a machine with another regional
// setting (decimal comma), so no assertion depends on the separators of the
// machine that runs it. Usage: node tools/run-vitest-locale.mjs [es-ES]
import { spawnSync } from "node:child_process";

const locale = process.argv[2] ?? "es-ES";
const result = spawnSync("npx", ["vitest", "run", "--maxWorkers=1"], {
  stdio: "inherit",
  shell: process.platform === "win32",
  env: { ...process.env, COLUMNIA_TEST_LOCALE: locale },
});
process.exitCode = result.status ?? 1;
