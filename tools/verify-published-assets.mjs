import { createHash, createPublicKey, verify } from "node:crypto";
import {
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { basename, resolve } from "node:path";
import { fileURLToPath } from "node:url";

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

function strictBase64(value, label) {
  if (typeof value !== "string" || !/^[A-Za-z0-9+/]+={0,2}$/.test(value) || value.length % 4 !== 0) {
    fail(`${label} no es Base64 estricto.`);
  }
  return Buffer.from(value, "base64");
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

function verifyMinisign(artifact, encodedPublicKey, encodedSignature) {
  const publicKeyText = strictBase64(encodedPublicKey, "La clave pública updater").toString("utf8");
  const publicKeyLines = publicKeyText.split(/\r?\n/).filter(Boolean);
  const publicKeyLine = publicKeyLines.find((line) => /^[A-Za-z0-9+/]+={0,2}$/.test(line));
  if (!publicKeyLine) fail("La clave pública updater no contiene un bloque minisign.");
  const publicKey = strictBase64(publicKeyLine, "El bloque de clave pública updater");
  if (publicKey.length !== 42 || publicKey.subarray(0, 2).toString("ascii") !== "Ed") {
    fail("La clave pública updater no usa el formato Ed25519 esperado.");
  }

  const signatureText = strictBase64(encodedSignature, "La firma updater").toString("utf8");
  const signatureLines = signatureText.split(/\r?\n/).filter(Boolean);
  if (!signatureLines[0]?.startsWith("untrusted comment:") || !signatureLines[2]?.startsWith("trusted comment:")) {
    fail("La firma updater no contiene los comentarios minisign esperados.");
  }
  const signature = strictBase64(signatureLines[1], "La firma primaria updater");
  const trustedSignature = strictBase64(signatureLines[3], "La firma del comentario updater");
  if (signature.length !== 74 || trustedSignature.length !== 64 || signature.subarray(0, 2).toString("ascii") !== "ED") {
    fail("La firma updater no usa el formato Ed25519 esperado.");
  }
  if (!signature.subarray(2, 10).equals(publicKey.subarray(2, 10))) {
    fail("La firma updater pertenece a una clave distinta de la clave pública embebida.");
  }

  const publicKeyDer = Buffer.concat([
    Buffer.from("302a300506032b6570032100", "hex"),
    publicKey.subarray(10),
  ]);
  const keyObject = createPublicKey({ key: publicKeyDer, format: "der", type: "spki" });
  const digest = createHash("blake2b512").update(artifact).digest();
  if (!verify(null, digest, keyObject, signature.subarray(10))) {
    fail("La firma minisign no valida el contenido descargado.");
  }
  return {
    algorithm: "Ed25519 over BLAKE2b-512",
    fingerprint: publicKey.subarray(2, 10).toString("hex").toUpperCase(),
    trustedCommentPresent: true,
    verified: true,
  };
}

async function fetchBytes(url, label) {
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
  return Buffer.from(await response.arrayBuffer());
}

const options = parseArguments(process.argv.slice(2));
const manifestUrl = httpsUrl(required(options, "manifest-url"), "--manifest-url").toString();
const outputDirectory = resolve(projectRoot, required(options, "output-dir"));
const target = options.target?.trim() || "windows-x86_64";
const expectedVersion = options["expected-version"]?.trim() || null;
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
  const manifestBytes = await fetchBytes(manifestUrl, "el manifiesto updater");
  const manifest = JSON.parse(manifestBytes.toString("utf8"));
  const platform = manifest.platforms?.[target];
  if (!versionIsValid(manifest.version ?? "")) fail("El manifiesto publicado no tiene una versión SemVer válida.");
  if (expectedVersion && manifest.version !== expectedVersion) {
    fail(`El manifiesto publicado declara ${manifest.version}, pero se esperaba ${expectedVersion}.`);
  }
  if (!platform || typeof platform !== "object") fail(`El manifiesto publicado no contiene la plataforma ${target}.`);
  if (!Number.isInteger(platform.sizeBytes) || platform.sizeBytes <= 0) fail("El manifiesto publicado no tiene tamaño válido.");
  if (!/^[a-f0-9]{64}$/.test(platform.sha256 ?? "")) fail("El manifiesto publicado no tiene SHA-256 válido.");
  if (typeof platform.signature !== "string") fail("El manifiesto publicado no contiene firma.");
  const artifactUrl = httpsUrl(platform.url, "La URL del artefacto publicado");
  const name = artifactName(artifactUrl);
  const artifact = await fetchBytes(artifactUrl.toString(), "el instalador publicado");
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
