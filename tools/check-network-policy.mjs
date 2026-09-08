import { mkdir, readdir, readFile, writeFile } from "node:fs/promises";
import { join, relative, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

let projectRoot;
try {
  projectRoot = resolve(fileURLToPath(new URL("..", import.meta.url)));
} catch {
  projectRoot = resolve(process.cwd());
}
const sourceRoots = ["src", "src-tauri/src"];
const sourceExtensions = new Set([".ts", ".tsx", ".rs"]);
const frontendForbiddenPatterns = [
  /\bfetch\s*\(/,
  /\bXMLHttpRequest\b/,
  /\bWebSocket\b/,
  /\bEventSource\b/,
  /\b(?:sentry|posthog|plausible|amplitude|analytics|telemetry)\b/i,
];
const rustForbiddenPatterns = [
  /\b(?:reqwest|ureq|surf|hyper)::/,
  /\b(?:sentry|posthog|plausible|amplitude|analytics|telemetry)\b/i,
];

export function findPolicyViolations(contents, file) {
  const extension = file.slice(file.lastIndexOf(".")).toLowerCase();
  const patterns = extension === ".rs" ? rustForbiddenPatterns : frontendForbiddenPatterns;
  return patterns
    .filter((pattern) => pattern.test(contents))
    .map((pattern) => ({ file, pattern: pattern.source }));
}

async function collectSources(root, output = []) {
  for (const entry of await readdir(root, { withFileTypes: true })) {
    const path = join(root, entry.name);
    if (entry.isDirectory()) await collectSources(path, output);
    else if (sourceExtensions.has(path.slice(path.lastIndexOf("."))) && !path.endsWith(".test.ts") && !path.endsWith(".test.tsx")) output.push(path);
  }
  return output;
}

export async function runNetworkPolicyCheck() {
  const stamp = new Date().toISOString().replace(/[-:]/g, "").replace(/\.\d{3}Z$/, "Z");
  const evidenceDirectory = join(projectRoot, ".local", "validation", "network-policy", stamp);
  const evidencePath = join(evidenceDirectory, "summary.json");
  const violations = [];
  for (const root of sourceRoots) {
    for (const path of await collectSources(join(projectRoot, root))) {
      const contents = await readFile(path, "utf8");
      violations.push(...findPolicyViolations(contents, relative(projectRoot, path).replaceAll("\\", "/")));
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

  return { status, evidencePath, violations };
}

const invokedPath = process.argv[1] ? pathToFileURL(resolve(process.argv[1])).href : "";
if (import.meta.url === invokedPath) {
  const result = await runNetworkPolicyCheck();
  if (result.status !== "passed") {
    console.error(`Política de red falló: ${JSON.stringify(result.violations)}. Evidencia: ${relative(projectRoot, result.evidencePath)}`);
    process.exitCode = 1;
  } else {
    console.log(`Política de red aprobada. Evidencia: ${relative(projectRoot, result.evidencePath)}`);
  }
}
