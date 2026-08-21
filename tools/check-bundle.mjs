import { createHash } from "node:crypto";
import { createReadStream, existsSync, mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { basename, dirname, extname, relative, resolve, sep } from "node:path";
import { gzipSync } from "node:zlib";

const FRONTEND_LIMITS = Object.freeze({
  javascript: Object.freeze({ rawBytesPerFile: 512 * 1024, gzipBytesPerFile: 160 * 1024 }),
  css: Object.freeze({ rawBytesPerFile: 128 * 1024, gzipBytesPerFile: 40 * 1024 }),
  total: Object.freeze({ rawBytes: 768 * 1024, gzipBytes: 240 * 1024 }),
});

function parseArguments(argv) {
  const [command, ...rest] = argv;
  const options = {};
  for (let index = 0; index < rest.length; index += 2) {
    const key = rest[index];
    const value = rest[index + 1];
    if (!key?.startsWith("--") || value === undefined) {
      throw new Error("Uso: check-bundle.mjs <budget|snapshot|artifacts> --opción valor.");
    }
    options[key.slice(2)] = value;
  }
  return { command, options };
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
      else if (entry.isFile()) files.push(fullPath);
    }
  };
  visit(root);
  return files.sort((left, right) => normalizedRelative(root, left).localeCompare(normalizedRelative(root, right), "en"));
}

function writeJson(outputPath, value) {
  mkdirSync(dirname(outputPath), { recursive: true });
  writeFileSync(outputPath, `${JSON.stringify(value, null, 2)}\n`, "utf8");
}

function checkFrontendBudget(distPath, outputPath) {
  const files = listFiles(distPath)
    .filter((file) => [".js", ".css"].includes(extname(file).toLowerCase()))
    .map((file) => {
      const contents = readFileSync(file);
      const kind = extname(file).toLowerCase() === ".js" ? "javascript" : "css";
      return {
        path: normalizedRelative(distPath, file),
        kind,
        rawBytes: contents.byteLength,
        gzipBytes: gzipSync(contents, { level: 9 }).byteLength,
      };
    });

  const totals = files.reduce(
    (sum, file) => ({ rawBytes: sum.rawBytes + file.rawBytes, gzipBytes: sum.gzipBytes + file.gzipBytes }),
    { rawBytes: 0, gzipBytes: 0 },
  );
  const violations = [];
  if (!files.some((file) => file.kind === "javascript")) {
    violations.push("El build no contiene ningún archivo JavaScript medible.");
  }
  for (const file of files) {
    const limit = FRONTEND_LIMITS[file.kind];
    if (file.rawBytes > limit.rawBytesPerFile) {
      violations.push(`${file.path}: ${file.rawBytes} bytes raw exceden ${limit.rawBytesPerFile}.`);
    }
    if (file.gzipBytes > limit.gzipBytesPerFile) {
      violations.push(`${file.path}: ${file.gzipBytes} bytes gzip exceden ${limit.gzipBytesPerFile}.`);
    }
  }
  if (totals.rawBytes > FRONTEND_LIMITS.total.rawBytes) {
    violations.push(`Total raw ${totals.rawBytes} excede ${FRONTEND_LIMITS.total.rawBytes} bytes.`);
  }
  if (totals.gzipBytes > FRONTEND_LIMITS.total.gzipBytes) {
    violations.push(`Total gzip ${totals.gzipBytes} excede ${FRONTEND_LIMITS.total.gzipBytes} bytes.`);
  }

  const evidence = {
    schemaVersion: 1,
    status: violations.length === 0 ? "passed" : "failed",
    limits: FRONTEND_LIMITS,
    totals,
    files,
    violations,
  };
  writeJson(outputPath, evidence);
  if (violations.length > 0) {
    throw new Error(`Presupuesto frontend excedido. ${violations.join(" ")} Divide el bundle o revisa explícitamente los límites.`);
  }
  console.log(`Bundle frontend: ${files.length} archivos, ${totals.rawBytes} bytes raw, ${totals.gzipBytes} bytes gzip.`);
}

function isDistributionArtifact(file) {
  const name = basename(file).toLowerCase();
  return [".msi", ".exe", ".dmg", ".deb", ".rpm", ".appimage", ".apk", ".aab", ".ipa", ".zip", ".sig"]
    .some((suffix) => name.endsWith(suffix)) || name.endsWith(".tar.gz");
}

function sha256(file) {
  return new Promise((resolveHash, rejectHash) => {
    const hash = createHash("sha256");
    const stream = createReadStream(file);
    stream.on("error", rejectHash);
    stream.on("data", (chunk) => hash.update(chunk));
    stream.on("end", () => resolveHash(hash.digest("hex")));
  });
}

async function inspectArtifacts(projectRoot, bundleRoot) {
  const artifacts = [];
  for (const file of listFiles(bundleRoot).filter(isDistributionArtifact)) {
    const stats = statSync(file);
    artifacts.push({
      path: normalizedRelative(projectRoot, file),
      sizeBytes: stats.size,
      sha256: await sha256(file),
      mtimeMs: Math.trunc(stats.mtimeMs),
    });
  }
  return artifacts;
}

async function snapshotArtifacts(projectRoot, bundleRoot, outputPath) {
  writeJson(outputPath, {
    schemaVersion: 1,
    artifacts: await inspectArtifacts(projectRoot, bundleRoot),
  });
  console.log("Snapshot previo de bundles capturado.");
}

async function reportProducedArtifacts(projectRoot, bundleRoot, snapshotPath, outputPath) {
  const previousDocument = JSON.parse(readFileSync(snapshotPath, "utf8"));
  const previous = new Map(previousDocument.artifacts.map((artifact) => [artifact.path, artifact]));
  const current = await inspectArtifacts(projectRoot, bundleRoot);
  const produced = current
    .filter((artifact) => {
      const old = previous.get(artifact.path);
      return !old || old.sizeBytes !== artifact.sizeBytes || old.sha256 !== artifact.sha256 || old.mtimeMs !== artifact.mtimeMs;
    })
    .map(({ mtimeMs: _mtimeMs, ...artifact }) => artifact);
  const evidence = {
    schemaVersion: 1,
    status: produced.length > 0 ? "available" : "failed",
    artifacts: produced,
  };
  writeJson(outputPath, evidence);
  if (produced.length === 0) {
    throw new Error("El empaquetado terminó sin producir artefactos nuevos o actualizados en target/release/bundle.");
  }
  console.log(`Artefactos de distribución producidos: ${produced.length}.`);
}

const { command, options } = parseArguments(process.argv.slice(2));
const projectRoot = resolve(options["project-root"] ?? ".");
if (command === "budget") {
  checkFrontendBudget(resolve(options.dist ?? "dist"), resolve(options.output ?? ".local/validation/frontend-bundle.json"));
} else if (command === "snapshot") {
  await snapshotArtifacts(projectRoot, resolve(options["bundle-root"] ?? "src-tauri/target/release/bundle"), resolve(options.output));
} else if (command === "artifacts") {
  await reportProducedArtifacts(
    projectRoot,
    resolve(options["bundle-root"] ?? "src-tauri/target/release/bundle"),
    resolve(options.snapshot),
    resolve(options.output),
  );
} else {
  throw new Error("Comando requerido: budget, snapshot o artifacts.");
}
