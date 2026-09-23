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

// Declared, user-initiated network exits. Any other file using these crates is
// a new outbound channel and must be reviewed before it is added here.
export const declaredEgress = [
  { crate: "odbc_api", files: ["src-tauri/src/remote_databases.rs"], reason: "entrega ODBC por acción explícita" },
  { crate: "tauri_plugin_updater", files: ["src-tauri/src/updater.rs", "src-tauri/src/lib.rs"], reason: "updater firmado, solo con endpoint de distribución" },
];

export function findPolicyViolations(contents, file) {
  const extension = file.slice(file.lastIndexOf(".")).toLowerCase();
  const patterns = extension === ".rs" ? rustForbiddenPatterns : frontendForbiddenPatterns;
  const violations = patterns
    .filter((pattern) => pattern.test(contents))
    .map((pattern) => ({ file, pattern: pattern.source }));
  if (extension === ".rs") {
    for (const egress of declaredEgress) {
      if (new RegExp(`\\b${egress.crate}::`).test(contents) && !egress.files.includes(file)) {
        violations.push({ file, pattern: `${egress.crate} fuera de ${egress.files.join(", ")}` });
      }
    }
  }
  return violations;
}

const cspAllowedSources = {
  production: new Set(["'self'", "'none'", "'unsafe-inline'", "data:", "ipc:", "http://ipc.localhost"]),
  development: new Set(["'self'", "'none'", "'unsafe-inline'", "data:", "ipc:", "http://ipc.localhost", "http://127.0.0.1:1420", "ws://127.0.0.1:1420"]),
};

function cspDirectives(csp) {
  if (typeof csp === "string") {
    return Object.fromEntries(
      csp.split(";").map((part) => part.trim()).filter(Boolean).map((part) => {
        const [name, ...sources] = part.split(/\s+/);
        return [name, sources.join(" ")];
      }),
    );
  }
  return csp;
}

/** Validates the Tauri 2 CSP (app.security.csp/devCsp), as an object or string. */
export function findCspViolations(tauriConfig) {
  const security = tauriConfig?.app?.security;
  const violations = [];
  for (const [key, kind] of [["csp", "production"], ["devCsp", "development"]]) {
    const csp = security?.[key];
    if (csp === undefined || csp === null || csp === "") {
      violations.push({ file: "src-tauri/tauri.conf.json", pattern: `app.security.${key} ausente: Tauri desactivaría la CSP` });
      continue;
    }
    for (const [directive, value] of Object.entries(cspDirectives(csp))) {
      for (const source of String(value).split(/\s+/).filter(Boolean)) {
        if (!cspAllowedSources[kind].has(source)) {
          violations.push({ file: "src-tauri/tauri.conf.json", pattern: `app.security.${key} ${directive} ${source}` });
        }
      }
    }
  }
  return violations;
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
  const cspViolations = findCspViolations(tauriConfig);
  violations.push(...cspViolations);
  const externalOrigins = cspViolations.map((violation) => violation.pattern);

  const status = violations.length === 0 ? "passed" : "failed";
  await mkdir(evidenceDirectory, { recursive: true });
  await writeFile(
    evidencePath,
    `${JSON.stringify({
      schemaVersion: 1,
      status,
      generatedAt: new Date().toISOString(),
      policy: "sin red de aplicación ni telemetría; CSP de producción y desarrollo limitada a fuentes internas; salidas declaradas solo en sus módulos",
      declaredEgress,
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
