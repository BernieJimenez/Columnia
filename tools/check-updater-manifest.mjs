import { createHash } from "node:crypto";
import { existsSync, readFileSync, statSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const projectRoot = resolve(fileURLToPath(new URL("..", import.meta.url)));

function fail(message) {
  throw new Error(message);
}

function parseArguments(argv) {
  const options = {};
  for (let index = 0; index < argv.length; index += 2) {
    const key = argv[index];
    const value = argv[index + 1];
    if (!key?.startsWith("--") || value === undefined) {
      fail("Uso: check-updater-manifest.mjs --manifest <json> --inventory <json>.");
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

function readJson(path) {
  return JSON.parse(readFileSync(path, "utf8"));
}

function sha256(path) {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

const options = parseArguments(process.argv.slice(2));
const manifestPath = resolve(projectRoot, required(options, "manifest"));
const inventoryPath = resolve(projectRoot, required(options, "inventory"));
const manifest = readJson(manifestPath);
const inventory = readJson(inventoryPath);
const target = inventory.target;
const platform = manifest.platforms?.[target];
const artifactPath = resolve(projectRoot, inventory.artifact?.path ?? "");
const signaturePath = resolve(projectRoot, inventory.artifact?.signaturePath ?? "");

if (inventory.schemaVersion !== 1 || inventory.status !== "passed") fail("El inventario updater no está aprobado.");
if (manifest.version !== inventory.version) fail("La versión del manifiesto updater no coincide con su inventario.");
if (!platform) fail(`El manifiesto updater no contiene la plataforma ${target}.`);
if (!/^https:\/\//.test(platform.url)) fail("La URL updater no usa HTTPS.");
if (!Number.isInteger(platform.sizeBytes) || platform.sizeBytes <= 0) fail("El manifiesto updater no tiene tamaño válido.");
if (!/^[a-f0-9]{64}$/.test(platform.sha256 ?? "")) fail("El manifiesto updater no tiene SHA-256 válido.");
if (!existsSync(artifactPath) || !statSync(artifactPath).isFile()) fail("Falta el artefacto updater local.");
if (!existsSync(signaturePath) || !statSync(signaturePath).isFile()) fail("Falta la firma updater local.");
const signature = readFileSync(signaturePath, "utf8").trim();
const decodedSignature = Buffer.from(signature, "base64").toString("utf8");
if (signature !== platform.signature || !decodedSignature.includes("untrusted comment:")) {
  fail("La firma local no coincide con el manifiesto o no es minisign codificado por Tauri.");
}
if (statSync(artifactPath).size !== platform.sizeBytes) fail("El tamaño local no coincide con el manifiesto updater.");
if (sha256(artifactPath) !== platform.sha256 || sha256(artifactPath) !== inventory.artifact.sha256) {
  fail("El SHA-256 local no coincide con el manifiesto y su inventario.");
}
if (sha256(signaturePath) !== inventory.artifact.signatureSha256) fail("El SHA-256 de la firma no coincide con el inventario.");
if (sha256(manifestPath) !== inventory.manifest.sha256) fail("El SHA-256 del manifiesto no coincide con el inventario.");

console.log(`Manifiesto updater aprobado: ${manifest.version}, ${target}, firma e integridad verificadas.`);
