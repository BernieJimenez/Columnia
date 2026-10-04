import { readFile } from "node:fs/promises";
import { execFileSync } from "node:child_process";
import { resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const projectRoot = resolve(fileURLToPath(new URL("..", import.meta.url)));

function output(command, args, options = {}) {
  return execFileSync(command, args, {
    cwd: projectRoot,
    encoding: "utf8",
    ...options,
  }).trim();
}

function fail(message) {
  throw new Error(message);
}

function parseVersion(text) {
  const match = /^v?(\d+)(?:\.(\d+))?(?:\.(\d+))?/.exec(text.trim());
  return match ? [Number(match[1]), Number(match[2] ?? 0), Number(match[3] ?? 0)] : null;
}

function compareVersions(left, right) {
  for (let index = 0; index < 3; index += 1) {
    if (left[index] !== right[index]) return left[index] - right[index];
  }
  return 0;
}

/**
 * OPS-08: whether `version` satisfies an `engines` range made of
 * space-separated comparators (`>=24.14.0 <25`); a bare version is exact.
 */
export function satisfiesRange(version, range) {
  const actual = parseVersion(version);
  if (!actual) return false;
  return range.trim().split(/\s+/).every((comparator) => {
    const [, operator = "=", bound] = /^(>=|<=|>|<|=)?\s*(.+)$/.exec(comparator) ?? [];
    const expected = bound ? parseVersion(bound) : null;
    if (!expected) return false;
    const order = compareVersions(actual, expected);
    return { ">=": order >= 0, "<=": order <= 0, ">": order > 0, "<": order < 0, "=": order === 0 }[operator];
  });
}

async function main() {
  const packageManifest = JSON.parse(await readFile(resolve(projectRoot, "package.json"), "utf8"));
  const toolchain = await readFile(resolve(projectRoot, "rust-toolchain.toml"), "utf8");
  const expectedRust = toolchain.match(/channel\s*=\s*"([^"]+)"/)?.[1];
  const expectedNode = packageManifest.engines?.node;
  const expectedNpm = packageManifest.engines?.npm;
  if (!expectedRust || !expectedNode || !expectedNpm) {
    fail("Faltan las versiones fijadas de Node, npm o Rust.");
  }

  const nodeVersion = output("node", ["--version"]);
  const npmVersion = process.platform === "win32"
    ? output("cmd.exe", ["/d", "/c", "npm --version"])
    : output("npm", ["--version"]);
  const rustVersion = output("rustc", ["--version"]);
  const cargoVersion = output("cargo", ["--version"]);
  if (!satisfiesRange(nodeVersion, expectedNode)) {
    fail(`Node incompatible: ${nodeVersion}; se requiere ${expectedNode}.`);
  }
  if (!satisfiesRange(npmVersion, expectedNpm)) {
    fail(`npm incompatible: ${npmVersion}; se requiere ${expectedNpm}.`);
  }
  if (!rustVersion.startsWith(`rustc ${expectedRust} `) || !cargoVersion.startsWith(`cargo ${expectedRust} `)) {
    fail(`Rust incompatible: ${rustVersion}; ${cargoVersion}; se requiere ${expectedRust}.`);
  }
  console.log(`Toolchains aprobados: Node ${nodeVersion}, npm ${npmVersion}, Rust ${expectedRust}.`);
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  try {
    await main();
  } catch (error) {
    console.error(`Gate de toolchains falló: ${error instanceof Error ? error.message : String(error)}`);
    process.exitCode = 1;
  }
}
