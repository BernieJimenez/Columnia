import { createHash } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { basename, dirname, extname, relative, resolve, sep } from "node:path";
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
      fail("Uso: generate-updater-manifest.mjs --output <json> --bundle-root <dir> --base-url <https-url> [opciones].");
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

function normalizedRelative(root, file) {
  return relative(root, file).split(sep).join("/");
}

function listFiles(root) {
  if (!existsSync(root)) return [];
  const files = [];
  const visit = (directory) => {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      const fullPath = resolve(directory, entry.name);
      if (entry.isDirectory()) visit(fullPath);
      if (entry.isFile()) files.push(fullPath);
    }
  };
  visit(root);
  return files.sort((left, right) => normalizedRelative(root, left).localeCompare(normalizedRelative(root, right), "en"));
}

function sha256(file) {
  return createHash("sha256").update(readFileSync(file)).digest("hex");
}

function safeVersion(value) {
  if (!/^v?\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/.test(value)) {
    fail(`Versión inválida para el manifiesto updater: ${value}.`);
  }
  return value;
}

function artifactCandidates(bundleRoot, kind) {
  const extension = kind === "msi" ? ".msi" : kind === "nsis" ? ".exe" : null;
  if (!extension) fail(`Tipo de artefacto updater no soportado: ${kind}.`);
  const directory = resolve(bundleRoot, kind);
  return listFiles(directory).filter((file) => extname(file).toLowerCase() === extension);
}

function resolveArtifact(options, bundleRoot) {
  if (options.artifact) {
    const artifact = resolve(projectRoot, options.artifact);
    if (!artifact.startsWith(`${bundleRoot}${sep}`) || !existsSync(artifact) || !statSync(artifact).isFile()) {
      fail(`El artefacto updater debe ser un archivo existente dentro de ${normalizedRelative(projectRoot, bundleRoot)}.`);
    }
    return artifact;
  }

  const candidates = artifactCandidates(bundleRoot, options["artifact-kind"] ?? "nsis");
  if (candidates.length === 0) fail("No se encontró un instalador updater compatible.");
  if (candidates.length > 1) {
    fail(`Hay varios instaladores candidatos; selecciona uno con --artifact: ${candidates.map((file) => normalizedRelative(projectRoot, file)).join(", ")}.`);
  }
  return candidates[0];
}

function artifactUrl(baseUrl, artifact) {
  const normalizedBase = baseUrl.endsWith("/") ? baseUrl : `${baseUrl}/`;
  const url = new URL(encodeURIComponent(basename(artifact)), normalizedBase);
  if (url.protocol !== "https:") fail("La URL de los artefactos updater debe usar HTTPS.");
  return url.toString();
}

function releaseNotes(options, version) {
  const source = options["notes-file"]
    ? readFileSync(resolve(projectRoot, options["notes-file"]), "utf8")
    : readFileSync(resolve(projectRoot, "CHANGELOG.md"), "utf8");
  const section = options["notes-file"]
    ? source
    : source.match(/^##\s+\[?[^\]\r\n]+\]?\s*$([\s\S]*?)(?=^##\s|(?![\s\S]))/m)?.[1] ?? "";
  const notes = section.replace(/^###\s+/gm, "").trim();
  return notes || `Actualización de Columnia ${version}.`;
}

const options = parseArguments(process.argv.slice(2));
const bundleRoot = resolve(projectRoot, required(options, "bundle-root"));
const outputPath = resolve(projectRoot, required(options, "output"));
const inventoryPath = options["inventory-output"]
  ? resolve(projectRoot, options["inventory-output"])
  : null;
const baseUrl = required(options, "base-url");
const parsedBaseUrl = new URL(baseUrl);
if (parsedBaseUrl.protocol !== "https:") fail("--base-url debe usar HTTPS.");
const target = required(options, "target");
const version = safeVersion(required(options, "version"));
const artifact = resolveArtifact(options, bundleRoot);
const signaturePath = `${artifact}.sig`;
if (!existsSync(signaturePath) || !statSync(signaturePath).isFile()) {
  fail(`Falta la firma updater para ${normalizedRelative(projectRoot, artifact)}.`);
}
const signature = readFileSync(signaturePath, "utf8").trim();
const decodedSignature = signature ? Buffer.from(signature, "base64").toString("utf8") : "";
if (!signature || !/^[A-Za-z0-9+/]+={0,2}$/.test(signature) || !decodedSignature.includes("untrusted comment:")) {
  fail(`La firma updater de ${normalizedRelative(projectRoot, artifact)} está vacía o no parece minisign.`);
}

const artifactStats = statSync(artifact);
const manifest = {
  version,
  notes: releaseNotes(options, version),
  pub_date: new Date().toISOString(),
  platforms: {
    [target]: {
      url: artifactUrl(baseUrl, artifact),
      signature,
      sizeBytes: artifactStats.size,
      sha256: sha256(artifact),
    },
  },
};
if (!manifest.notes) fail("Las notas del release no pueden estar vacías.");

const writeJson = (path, value) => {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`, "utf8");
};
writeJson(outputPath, manifest);

if (inventoryPath) {
  writeJson(inventoryPath, {
    schemaVersion: 1,
    status: "passed",
    generatedAt: new Date().toISOString(),
    version,
    target,
    artifact: {
      path: normalizedRelative(projectRoot, artifact),
      url: manifest.platforms[target].url,
      sizeBytes: artifactStats.size,
      sha256: manifest.platforms[target].sha256,
      signaturePath: normalizedRelative(projectRoot, signaturePath),
      signatureSha256: sha256(signaturePath),
      signatureBytes: Buffer.byteLength(signature, "utf8"),
    },
    manifest: {
      path: normalizedRelative(projectRoot, outputPath),
      sha256: sha256(outputPath),
    },
  });
}

console.log(`Manifiesto updater generado: ${normalizedRelative(projectRoot, outputPath)}.`);
