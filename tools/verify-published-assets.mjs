import { createHash } from "node:crypto";
import {
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { basename, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { verifyMinisign } from "./updater-crypto.mjs";
import {
  MAX_ARTIFACT_BYTES,
  MAX_MANIFEST_BYTES,
  configuredUpdaterHosts,
  localVersion,
  readLimitedBody,
  updaterPolicyProblems,
} from "./updater-policy.mjs";

const projectRoot = resolve(fileURLToPath(new URL("..", import.meta.url)));

function fail(message) {
  throw new Error(message);
}

function parseArguments(argv) {
  if (argv.includes("--help")) {
    console.log("Uso: verify-published-assets.mjs --manifest-url <https-url> --output-dir <dir> [--target <platform>] [--expected-version <semver>].");
    process.exit(0);
  }
  const options = {};
  for (let index = 0; index < argv.length; index += 2) {
    const key = argv[index];
    const value = argv[index + 1];
    if (!key?.startsWith("--") || value === undefined) {
      fail("Uso: verify-published-assets.mjs --manifest-url <https-url> --output-dir <dir> [--target <platform>] [--expected-version <semver>].");
    }
    options[key.slice(2)] = value;
  }
  return options;
}

function required(options, name) {
  const value = options[name]?.trim();
  if (!value) fail(`Falta --${name}.`);
  return value;
}

function httpsUrl(value, label) {
  let parsed;
  try {
    parsed = new URL(value);
  } catch {
    fail(`${label} no es una URL absoluta válida.`);
  }
  if (parsed.protocol !== "https:") fail(`${label} debe usar HTTPS.`);
  return parsed;
}

function readJsonFile(path) {
  try {
    return JSON.parse(readFileSync(path, "utf8"));
  } catch (error) {
    fail(`No se pudo leer JSON en ${path}: ${error.message}`);
  }
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function versionIsValid(value) {
  return /^v?\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/.test(value);
}

function artifactName(url) {
  const name = decodeURIComponent(basename(url.pathname));
  if (!/^[A-Za-z0-9._-]+$/.test(name) || !/\.(?:exe|msi)$/i.test(name)) {
    fail("La URL publicada no contiene un nombre de instalador seguro.");
  }
  return name;
}

async function fetchBytes(url, label, maximumBytes) {
  let response;
  try {
    response = await fetch(url, {
      redirect: "error",
      signal: AbortSignal.timeout(120_000),
    });
  } catch (error) {
    fail(`No se pudo descargar ${label}: ${error.message}`);
  }
  if (!response.ok) fail(`${label} respondió HTTP ${response.status}.`);
  // SEG-04: never more than the gate is willing to hold in memory.
  try {
    return await readLimitedBody(response, maximumBytes, label);
  } catch (error) {
    fail(error.message);
  }
}

const options = parseArguments(process.argv.slice(2));
const manifestUrl = httpsUrl(required(options, "manifest-url"), "--manifest-url").toString();
const outputDirectory = resolve(projectRoot, required(options, "output-dir"));
const target = options.target?.trim() || "windows-x86_64";
// SEG-04: the published version must be the one this checkout releases.
const expectedVersion = options["expected-version"]?.trim() || localVersion();
const allowedHosts = options["allowed-host"]
  ? [options["allowed-host"].trim()]
  : [new URL(manifestUrl).host, ...configuredUpdaterHosts()];
const config = readJsonFile(resolve(projectRoot, "src-tauri/tauri.conf.json"));
const publicKey = config.plugins?.updater?.pubkey;
if (!publicKey) fail("La compilación no declara una clave pública updater.");

mkdirSync(outputDirectory, { recursive: true });
const summaryPath = resolve(outputDirectory, "summary.json");
let downloadedPath = null;
let wroteArtifact = false;
let wroteSignature = false;
let status = "failed";
let errorMessage = null;
let summary = {
  schemaVersion: 1,
  status,
  generatedAt: new Date().toISOString(),
  manifestUrl,
  target,
  expectedVersion,
  artifact: null,
  signature: null,
  error: null,
};

try {
  const manifestBytes = await fetchBytes(manifestUrl, "el manifiesto updater", MAX_MANIFEST_BYTES);
  const manifest = JSON.parse(manifestBytes.toString("utf8"));
  const platform = manifest.platforms?.[target];
  if (!versionIsValid(manifest.version ?? "")) fail("El manifiesto publicado no tiene una versión SemVer válida.");
  if (!platform || typeof platform !== "object") fail(`El manifiesto publicado no contiene la plataforma ${target}.`);
  const policyProblems = updaterPolicyProblems({
    manifestVersion: manifest.version,
    expectedVersion,
    artifactUrl: platform.url,
    allowedHosts,
    sizeBytes: platform.sizeBytes,
  });
  if (policyProblems.length > 0) fail(policyProblems.join(" "));
  if (!Number.isInteger(platform.sizeBytes) || platform.sizeBytes <= 0) fail("El manifiesto publicado no tiene tamaño válido.");
  if (!/^[a-f0-9]{64}$/.test(platform.sha256 ?? "")) fail("El manifiesto publicado no tiene SHA-256 válido.");
  if (typeof platform.signature !== "string") fail("El manifiesto publicado no contiene firma.");
  const artifactUrl = httpsUrl(platform.url, "La URL del artefacto publicado");
  const name = artifactName(artifactUrl);
  const artifact = await fetchBytes(
    artifactUrl.toString(),
    "el instalador publicado",
    Math.min(platform.sizeBytes, MAX_ARTIFACT_BYTES),
  );
  downloadedPath = resolve(outputDirectory, name);
  if (artifact.length !== platform.sizeBytes) fail("El tamaño descargado no coincide con el manifiesto publicado.");
  const artifactHash = sha256(artifact);
  if (artifactHash !== platform.sha256) fail("El SHA-256 descargado no coincide con el manifiesto publicado.");
  const signatureEvidence = verifyMinisign(artifact, publicKey, platform.signature);
  writeFileSync(downloadedPath, artifact);
  wroteArtifact = true;
  writeFileSync(`${downloadedPath}.sig`, `${platform.signature}\n`, "utf8");
  wroteSignature = true;
  status = "passed";
  summary = {
    schemaVersion: 1,
    status,
    generatedAt: new Date().toISOString(),
    manifestUrl,
    target,
    expectedVersion,
    version: manifest.version,
    artifact: {
      name,
      url: artifactUrl.toString(),
      sizeBytes: artifact.length,
      sha256: artifactHash,
      path: name,
    },
    signature: signatureEvidence,
    error: null,
  };
} catch (error) {
  errorMessage = error.message;
  summary = { ...summary, error: errorMessage };
} finally {
  if (status !== "passed" && downloadedPath && wroteArtifact) {
    rmSync(downloadedPath, { force: true });
  }
  if (status !== "passed" && downloadedPath && wroteSignature) {
    rmSync(`${downloadedPath}.sig`, { force: true });
  }
  writeFileSync(summaryPath, `${JSON.stringify(summary, null, 2)}\n`, "utf8");
}

if (status !== "passed") {
  fail(`Verificación de assets publicados falló: ${errorMessage}. Evidencia: ${summaryPath}`);
}
console.log(`Assets publicados aprobados: ${summary.version}, ${target}, tamaño/SHA-256/firma verificados.`);
console.log(`Evidencia: ${summaryPath}`);
