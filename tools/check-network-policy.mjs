import { mkdir, readdir, readFile, writeFile } from "node:fs/promises";
import { join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const projectRoot = resolve(fileURLToPath(new URL("..", import.meta.url)));
const stamp = new Date().toISOString().replace(/[-:]/g, "").replace(/\.\d{3}Z$/, "Z");
const evidenceDirectory = join(projectRoot, ".local", "validation", "network-policy", stamp);
const evidencePath = join(evidenceDirectory, "summary.json");
const sourceRoots = ["src", "src-tauri/src"];
const sourceExtensions = new Set([".ts", ".tsx", ".rs"]);
const forbiddenPatterns = [
  /\bfetch\s*\(/,
  /\bXMLHttpRequest\b/,
  /\bWebSocket\b/,
  /\bEventSource\b/,
  /\b(?:reqwest|ureq|surf|hyper)::/,
  /\b(?:sentry|posthog|plausible|amplitude|analytics|telemetry)\b/i,
];

async function collectSources(root, output = []) {
  for (const entry of await readdir(root, { withFileTypes: true })) {
    const path = join(root, entry.name);
    if (entry.isDirectory()) await collectSources(path, output);
    else if (sourceExtensions.has(path.slice(path.lastIndexOf("."))) && !path.endsWith(".test.ts") && !path.endsWith(".test.tsx")) output.push(path);
  }
  return output;
}

const violations = [];
for (const root of sourceRoots) {
  for (const path of await collectSources(join(projectRoot, root))) {
    const contents = await readFile(path, "utf8");
    for (const pattern of forbiddenPatterns) {
      if (pattern.test(contents)) {
        violations.push({ file: relative(projectRoot, path).replaceAll("\\", "/"), pattern: pattern.source });
      }
    }
  }
}

const tauriConfig = JSON.parse(await readFile(join(projectRoot, "src-tauri", "tauri.conf.json"), "utf8"));
const productionCsp = String(tauriConfig.security?.csp ?? "");
const externalOrigins = productionCsp.match(/https?:\/\/[^\s;']+/g) ?? [];
const allowedInternalOrigins = new Set(["http://ipc.localhost"]);
const unexpectedOrigins = externalOrigins.filter((origin) => !allowedInternalOrigins.has(origin));
for (const origin of unexpectedOrigins) violations.push({ file: "src-tauri/tauri.conf.json", pattern: origin });

const status = violations.length === 0 ? "passed" : "failed";
await mkdir(evidenceDirectory, { recursive: true });
await writeFile(
  evidencePath,
  `${JSON.stringify({
    schemaVersion: 1,
    status,
    generatedAt: new Date().toISOString(),
    policy: "sin red de aplicación ni telemetría; solo IPC interno de Tauri en CSP de producción",
    scannedRoots: sourceRoots,
    externalOrigins,
    violations,
    evidenceDirectory: `.local/validation/network-policy/${stamp}`,
  }, null, 2)}\n`,
  "utf8",
);

if (status !== "passed") {
  console.error(`Política de red falló: ${JSON.stringify(violations)}. Evidencia: ${relative(projectRoot, evidencePath)}`);
  process.exitCode = 1;
} else {
  console.log(`Política de red aprobada. Evidencia: ${relative(projectRoot, evidencePath)}`);
}
