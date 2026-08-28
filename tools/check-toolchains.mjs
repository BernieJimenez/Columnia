import { readFile } from "node:fs/promises";
import { execFileSync } from "node:child_process";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

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

try {
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
  if (!new RegExp(`^v${expectedNode.replace(/^[^0-9]*/, "").split(/[ <>=]/)[0]}$`).test(nodeVersion)) {
    fail(`Node incompatible: ${nodeVersion}; se requiere ${expectedNode}.`);
  }
  if (!new RegExp(`^${expectedNpm.replace(/^[^0-9]*/, "").split(/[ <>=]/)[0]}$`).test(npmVersion)) {
    fail(`npm incompatible: ${npmVersion}; se requiere ${expectedNpm}.`);
  }
  if (!rustVersion.startsWith(`rustc ${expectedRust} `) || !cargoVersion.startsWith(`cargo ${expectedRust} `)) {
    fail(`Rust incompatible: ${rustVersion}; ${cargoVersion}; se requiere ${expectedRust}.`);
  }
  console.log(`Toolchains aprobados: Node ${nodeVersion}, npm ${npmVersion}, Rust ${expectedRust}.`);
} catch (error) {
  console.error(`Gate de toolchains falló: ${error instanceof Error ? error.message : String(error)}`);
  process.exitCode = 1;
}
