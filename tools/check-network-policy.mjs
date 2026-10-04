import { mkdir, readdir, readFile, writeFile } from "node:fs/promises";
import { join, relative, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

let projectRoot;
try {
  projectRoot = resolve(fileURLToPath(new URL("..", import.meta.url)));
} catch {
  projectRoot = resolve(process.cwd());
}
const sourceRoots = ["src", "src-tauri/src", "public"];
// QA-24: files outside the source roots that can open the network too.
const extraSourceFiles = ["index.html", "src-tauri/build.rs"];
const sourceExtensions = new Set([".ts", ".tsx", ".rs", ".js", ".mjs", ".html"]);
const frontendForbiddenPatterns = [
  /\bfetch\s*\(/,
  /\bXMLHttpRequest\b/,
  /\bWebSocket\b/,
  /\bEventSource\b/,
  /\bsendBeacon\b/,
  /\bWebTransport\b/,
  /\bRTCPeerConnection\b/,
  /\bnew\s+Image\s*\(/,
  /\bimport\s*\(\s*["'`]https?:/,
  /<script[^>]+\bsrc\s*=\s*["']https?:/i,
  // Computed access hides a name: globalThis["fe" + "tch"], window["…"].
  /\b(?:globalThis|window|self)\s*\[\s*["'`]/,
  // SDK packages, not every word that mentions analytics.
  /(?:from\s+|require\(\s*|import\(\s*)["'](?:@sentry\/|posthog|plausible|@amplitude\/|amplitude-js|@vercel\/analytics|@segment\/)/,
];
const rustForbiddenPatterns = [
  /\b(?:reqwest|ureq|surf|hyper|isahc|attohttpc|tungstenite|native_tls|curl)\b(?:::|\s*;)/,
  /\b(?:std|tokio|async_std)::net\b/,
  /\b(?:TcpStream|UdpSocket|TcpListener)\b/,
  /\b(?:sentry|opentelemetry|posthog)::/,
];
// Crates that open the network; a direct dependency must be declared below.
const networkCrates = ["reqwest", "ureq", "surf", "hyper", "isahc", "attohttpc", "tungstenite", "tokio-tungstenite", "native-tls", "curl", "sentry", "opentelemetry", "posthog-rs"];

// Declared, user-initiated network exits. Any other file using these crates is
// a new outbound channel and must be reviewed before it is added here.
export const declaredEgress = [
  { crate: "odbc_api", files: ["src-tauri/src/remote_databases.rs"], reason: "entrega ODBC por acción explícita" },
  // QA-24: in lib.rs only the line that registers the plugin.
  { crate: "tauri_plugin_updater", files: ["src-tauri/src/updater.rs"], lines: { "src-tauri/src/lib.rs": /\.plugin\(\s*tauri_plugin_updater::Builder::new\(\)\.build\(\)\s*\)/ }, reason: "updater firmado, solo con endpoint de distribución" },
];

/** Direct dependencies of Cargo.toml that open the network (QA-24). */
export function findCargoNetworkDependencies(cargoToml) {
  const violations = [];
  let inDependencies = false;
  for (const line of cargoToml.split(/\r?\n/)) {
    const section = line.match(/^\s*\[(.+)\]\s*$/);
    if (section) {
      inDependencies = /(^|\.)(dependencies|build-dependencies)$/.test(section[1].trim());
      continue;
    }
    const name = inDependencies ? line.match(/^\s*([A-Za-z0-9_-]+)\s*=/)?.[1] : undefined;
    if (name && networkCrates.includes(name)) {
      violations.push({ file: "src-tauri/Cargo.toml", pattern: `dependencia de red ${name}` });
    }
  }
  return violations;
}

export function findPolicyViolations(contents, file) {
  const extension = file.slice(file.lastIndexOf(".")).toLowerCase();
  const patterns = extension === ".rs" ? rustForbiddenPatterns : frontendForbiddenPatterns;
  const violations = patterns
    .filter((pattern) => pattern.test(contents))
    .map((pattern) => ({ file, pattern: pattern.source }));
  if (extension === ".rs") {
    for (const egress of declaredEgress) {
      const usage = new RegExp(`\\b${egress.crate}::`);
      if (!usage.test(contents) || egress.files.includes(file)) continue;
      const allowedLine = egress.lines?.[file];
      const stray = allowedLine
        ? contents.split(/\r?\n/).some((line) => usage.test(line) && !allowedLine.test(line))
        : true;
      if (stray) violations.push({ file, pattern: `${egress.crate} fuera de ${egress.files.join(", ")}` });
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
        // QA-24: inline code only for styles, never for scripts.
        if (source === "'unsafe-inline'" && directive !== "style-src") {
          violations.push({ file: "src-tauri/tauri.conf.json", pattern: `app.security.${key} ${directive} 'unsafe-inline'` });
          continue;
        }
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
    let paths = [];
    try {
      paths = await collectSources(join(projectRoot, root));
    } catch {
      continue;
    }
    for (const path of paths) {
      const contents = await readFile(path, "utf8");
      violations.push(...findPolicyViolations(contents, relative(projectRoot, path).replaceAll("\\", "/")));
    }
  }
  for (const file of extraSourceFiles) {
    try {
      violations.push(...findPolicyViolations(await readFile(join(projectRoot, file), "utf8"), file));
    } catch {
      // A missing optional file has nothing to scan.
    }
  }
  violations.push(...findCargoNetworkDependencies(await readFile(join(projectRoot, "src-tauri", "Cargo.toml"), "utf8")));

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
