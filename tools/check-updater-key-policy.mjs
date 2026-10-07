import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const projectRoot = resolve(fileURLToPath(new URL("..", import.meta.url)));

function fail(message) {
  throw new Error(message);
}

function readJson(relativePath) {
  return JSON.parse(readFileSync(resolve(projectRoot, relativePath), "utf8"));
}

function decodeBase64(value, label) {
  if (typeof value !== "string" || !/^[A-Za-z0-9+/]+={0,2}$/.test(value) || value.length % 4 !== 0) {
    fail(`${label} no es Base64 estricto.`);
  }
  return Buffer.from(value, "base64");
}

function requiredText(value) {
  return typeof value === "string" && value.trim() !== "";
}

// SEG-11: `--policy` lets the contract test check an altered policy.
const policyArgument = process.argv.indexOf("--policy");
const policyPath = policyArgument === -1 ? "fixtures/updater/key-policy-v1.json" : process.argv[policyArgument + 1];
if (!policyPath) fail("--policy requiere una ruta.");
const config = readJson("src-tauri/tauri.conf.json");
const policy = readJson(policyPath);
const configuredKey = config.plugins?.updater?.pubkey;
const activeKey = policy.activeKey;

if (policy.schemaVersion !== 1) fail("La política de claves updater requiere schemaVersion=1.");
if (!activeKey?.id || !/^[0-9A-F]{16}$/.test(activeKey.fingerprint ?? "")) {
  fail("La política updater debe declarar un fingerprint hexadecimal de 16 caracteres.");
}
if (configuredKey !== activeKey.publicKey) {
  fail("La clave pública embebida no coincide con la política versionada de rotación.");
}

const publicKeyDocument = decodeBase64(configuredKey, "La clave pública updater").toString("utf8");
const fingerprint = publicKeyDocument.match(/minisign public key:\s*([0-9A-F]{16})/i)?.[1]?.toUpperCase();
if (fingerprint !== activeKey.fingerprint) {
  fail("El fingerprint de la clave pública updater no coincide con la política versionada.");
}

if (policy.rotation?.strategy !== "bridge-release-signed-by-previous-key") {
  fail("La rotación updater debe usar una release puente firmada por la clave anterior.");
}
if (policy.rotation?.dualKeySupport !== false) {
  fail("La implementación actual no soporta confianza dual; la política debe declararlo explícitamente.");
}
if (!requiredText(policy.rotation?.oldPrivateKeyRetirement)) {
  fail("La política debe impedir retirar la clave privada histórica demasiado pronto.");
}
if (!Array.isArray(policy.rotation?.requiredEvidence) || policy.rotation.requiredEvidence.length < 4) {
  fail("La política debe exigir evidencia de puente, descarga, verificación y rollback.");
}
if (policy.recovery?.preserveInstalledVersion !== true || !requiredText(policy.recovery?.manualRecovery)) {
  fail("La recuperación updater debe preservar la versión instalada y definir recuperación manual.");
}
if (!Array.isArray(policy.recovery?.failClosedOn) || policy.recovery.failClosedOn.length < 5) {
  fail("La política debe enumerar las condiciones de fallo cerrado del updater.");
}
if (policy.recovery.compromiseResponse?.automatedBridgeAllowed !== false ||
    !Array.isArray(policy.recovery.compromiseResponse.steps) ||
    policy.recovery.compromiseResponse.steps.length < 5) {
  fail("El compromiso de la clave debe congelar el canal y prohibir una release puente automática.");
}

console.log(`Política de claves updater aprobada: ${activeKey.id}, fingerprint ${activeKey.fingerprint}.`);
