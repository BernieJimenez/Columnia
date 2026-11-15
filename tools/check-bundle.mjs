import { createHash } from "node:crypto";
import { createReadStream, existsSync, mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { basename, dirname, extname, relative, resolve, sep } from "node:path";
import process from "node:process";
import { pathToFileURL } from "node:url";
import { gzipSync } from "node:zlib";

export const FRONTEND_LIMITS = Object.freeze({
  javascript: Object.freeze({ rawBytesPerFile: 512 * 1024, gzipBytesPerFile: 160 * 1024 }),
  css: Object.freeze({ rawBytesPerFile: 128 * 1024, gzipBytesPerFile: 40 * 1024 }),
  // COD-19: fonts, images, .wasm and the page itself count too.
  other: Object.freeze({ rawBytesPerFile: 256 * 1024, gzipBytesPerFile: 256 * 1024 }),
  // 2026-10-07: raw total raised from 832 to 896 KiB for the roadmap features
  // (conflict bulk actions, suggested rules); the gzip total, closer to what the
  // WebView loads, keeps its 240 KiB limit.
  // Dos límites de JS+CSS, por qué:
  // - initialLoad (estricto): lo que el navegador descarga al abrir la app
  //   (index.html, su chunk de entrada, los estáticos de ese chunk y el CSS
  //   enlazado). Es lo que el usuario espera al arrancar; sin import() lazy.
  //   Crecer aquí castiga cada arranque, así que el límite va justo a lo medido.
  // - total (holgado): todo el JS+CSS de dist, lazy incluido. Mover código a
  //   chunks lazy no lo reduce, así que solo frena el crecimiento global: lo
  //   medido + 10 %. Se redondea a KiB hacia arriba.
  // Medido 2026-11-15 con el build de Windows (chrome105): carga inicial 497.530 raw /
  // 132.287 gzip; total 986.128 raw / 257.842 gzip. Límites: carga inicial lo medido
  // redondeado hacia arriba a KiB (486 / 130); total lo medido + 10 % a KiB (1060 / 277).
  initialLoad: Object.freeze({ rawBytes: 486 * 1024, gzipBytes: 130 * 1024 }),
  total: Object.freeze({ rawBytes: 1060 * 1024, gzipBytes: 277 * 1024 }),
  otherTotal: Object.freeze({ rawBytes: 512 * 1024 }),
});

// AUD-079: vite.config.ts compiles for safari13 (a larger bundle) when
// TAURI_ENV_PLATFORM is not "windows"; check.ps1 sets it, a manual build does not.
const BUILD_TARGET_MARK = ".local/validation/frontend-build-target.json";

export function buildTargetWarning(distPath, markPath = resolve(BUILD_TARGET_MARK)) {
  let mark;
  try {
    mark = JSON.parse(readFileSync(markPath, "utf8"));
  } catch {
    return "No hay marca del target de compilación (.local/validation/frontend-build-target.json): no se sabe si el build usó el target de Windows; el veredicto puede diferir del de check.ps1.";
  }
  if (resolve(mark.outDir ?? "") !== resolve(distPath)) {
    return "La marca del target de compilación es de otro directorio de salida: no se sabe con qué target se compiló este dist.";
  }
  if (mark.target !== "chrome105") {
    return `El dist se compiló con target ${mark.target}, mayor que el de Windows (chrome105): compila con TAURI_ENV_PLATFORM=windows (como check.ps1) o el veredicto puede ser un falso rojo.`;
  }
  return null;
}

function fileKind(file) {
  const extension = extname(file).toLowerCase();
  if (extension === ".js") return "javascript";
  if (extension === ".css") return "css";
  return "other";
}

export function parseArguments(argv) {
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
  // COD-19: name the missing option instead of a TypeError from resolve().
  const required = { snapshot: ["output"], artifacts: ["snapshot", "output"] }[command] ?? [];
  const missing = required.filter((name) => options[name] === undefined);
  if (missing.length > 0) {
    throw new Error(`${command} requiere ${missing.map((name) => `--${name}`).join(" y ")}.`);
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

// Import estático (no import() dinámico) de un chunk; cubre `import x from "./a.js"`,
// `import "./a.css"` y `export ... from "./a.js"`, con comillas simples, dobles o backtick.
const STATIC_SPECIFIER = /\b(?:import|export)\s*(?!\s*\()(?:[^'"`;]*?\bfrom\s*)?(["'`])(\.{1,2}\/[^"'`]+)\1/g;

function htmlAttribute(tag, name) {
  return tag.match(new RegExp(`\\b${name}="([^"]+)"`))?.[1];
}

// Lista los archivos que el navegador descarga al abrir dist/index.html: los
// <script src> y <link> de stylesheet/modulepreload de la página, y de ahí en
// adelante los imports estáticos de cada JS. Vite no genera manifest aquí, así
// que se analiza el propio build. Los import() dinámicos no se siguen.
export function initialLoadFiles(distPath) {
  const indexPath = resolve(distPath, "index.html");
  if (!existsSync(indexPath)) {
    throw new Error(`No hay ${indexPath}: no se puede medir la carga inicial.`);
  }
  const html = readFileSync(indexPath, "utf8");
  const references = [
    ...[...html.matchAll(/<script\b[^>]*>/g)].map((match) => htmlAttribute(match[0], "src")),
    ...[...html.matchAll(/<link\b[^>]*>/g)]
      .filter((match) => /\brel="(?:stylesheet|modulepreload)"/.test(match[0]))
      .map((match) => htmlAttribute(match[0], "href")),
  ].filter((reference) => reference !== undefined);

  const loaded = new Set();
  const queue = references.map((reference) => resolve(distPath, reference.replace(/^\/+/, "")));
  while (queue.length > 0) {
    const file = queue.shift();
    if (loaded.has(file)) continue;
    if (!existsSync(file)) {
      throw new Error(`index.html referencia ${normalizedRelative(distPath, file)}, que no existe en el build.`);
    }
    loaded.add(file);
    if (extname(file).toLowerCase() === ".js") {
      for (const match of readFileSync(file, "utf8").matchAll(STATIC_SPECIFIER)) {
        queue.push(resolve(dirname(file), match[2]));
      }
    }
  }
  return loaded;
}

export function checkFrontendBudget(distPath, outputPath, markPath, limits = FRONTEND_LIMITS) {
  const initial = initialLoadFiles(distPath);
  const files = listFiles(distPath)
    .map((file) => {
      const contents = readFileSync(file);
      const kind = fileKind(file);
      return {
        path: normalizedRelative(distPath, file),
        kind,
        initial: initial.has(file),
        rawBytes: contents.byteLength,
        gzipBytes: gzipSync(contents, { level: 9 }).byteLength,
      };
    });

  const sumJsCss = (selected) => selected.filter((file) => file.kind !== "other").reduce(
    (sum, file) => ({ rawBytes: sum.rawBytes + file.rawBytes, gzipBytes: sum.gzipBytes + file.gzipBytes }),
    { rawBytes: 0, gzipBytes: 0 },
  );
  const totals = sumJsCss(files);
  const initialLoad = sumJsCss(files.filter((file) => file.initial));
  const otherRawBytes = files.filter((file) => file.kind === "other").reduce((sum, file) => sum + file.rawBytes, 0);
  const violations = [];
  for (const file of files.filter((candidate) => candidate.path.toLowerCase().endsWith(".map"))) {
    violations.push(`${file.path}: el build no debe publicar source maps.`);
  }
  if (otherRawBytes > FRONTEND_LIMITS.otherTotal.rawBytes) {
    violations.push(`Recursos no JS/CSS ${otherRawBytes} bytes raw exceden ${FRONTEND_LIMITS.otherTotal.rawBytes}.`);
  }
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
  if (initialLoad.rawBytes > limits.initialLoad.rawBytes) {
    violations.push(`Carga inicial raw ${initialLoad.rawBytes} excede ${limits.initialLoad.rawBytes} bytes.`);
  }
  if (initialLoad.gzipBytes > limits.initialLoad.gzipBytes) {
    violations.push(`Carga inicial gzip ${initialLoad.gzipBytes} excede ${limits.initialLoad.gzipBytes} bytes.`);
  }
  if (totals.rawBytes > limits.total.rawBytes) {
    violations.push(`Total raw ${totals.rawBytes} excede ${limits.total.rawBytes} bytes.`);
  }
  if (totals.gzipBytes > limits.total.gzipBytes) {
    violations.push(`Total gzip ${totals.gzipBytes} excede ${limits.total.gzipBytes} bytes.`);
  }

  const warning = buildTargetWarning(distPath, markPath);
  if (warning) console.warn(`AVISO: ${warning}`);
  const evidence = {
    schemaVersion: 1,
    status: violations.length === 0 ? "passed" : "failed",
    warnings: warning ? [warning] : [],
    limits,
    initialLoad: { ...initialLoad, files: files.filter((file) => file.initial).map((file) => file.path) },
    totals,
    otherRawBytes,
    files,
    violations,
  };
  writeJson(outputPath, evidence);
  const summary = `Carga inicial: ${initialLoad.rawBytes} bytes raw (límite ${limits.initialLoad.rawBytes}), ${initialLoad.gzipBytes} bytes gzip (límite ${limits.initialLoad.gzipBytes}). Total JS+CSS: ${totals.rawBytes} bytes raw (límite ${limits.total.rawBytes}), ${totals.gzipBytes} bytes gzip (límite ${limits.total.gzipBytes}).`;
  if (violations.length > 0) {
    throw new Error(`Presupuesto frontend excedido. ${summary} ${violations.join(" ")} Divide el bundle o revisa explícitamente los límites.`);
  }
  console.log(`Bundle frontend: ${files.length} archivos. ${summary}`);
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

export async function snapshotArtifacts(projectRoot, bundleRoot, outputPath) {
  writeJson(outputPath, {
    schemaVersion: 1,
    artifacts: await inspectArtifacts(projectRoot, bundleRoot),
  });
  console.log("Snapshot previo de bundles capturado.");
}

export async function reportProducedArtifacts(projectRoot, bundleRoot, snapshotPath, outputPath) {
  const previousDocument = JSON.parse(readFileSync(snapshotPath, "utf8"));
  const previous = new Map(previousDocument.artifacts.map((artifact) => [artifact.path, artifact]));
  const current = await inspectArtifacts(projectRoot, bundleRoot);
  // COD-19: only the content says an artifact is new; a file restored from a
  // cache with another mtime is listed as touched, not as produced.
  const changed = (artifact) => {
    const old = previous.get(artifact.path);
    return !old || old.sizeBytes !== artifact.sizeBytes || old.sha256 !== artifact.sha256;
  };
  const withoutTime = ({ mtimeMs: _mtimeMs, ...artifact }) => artifact;
  const produced = current.filter(changed).map(withoutTime);
  const touched = current
    .filter((artifact) => !changed(artifact) && previous.get(artifact.path)?.mtimeMs !== artifact.mtimeMs)
    .map(withoutTime);
  const evidence = {
    schemaVersion: 1,
    status: produced.length > 0 ? "available" : "failed",
    artifacts: produced,
    touchedWithoutChanges: touched,
  };
  writeJson(outputPath, evidence);
  if (produced.length === 0) {
    throw new Error("El empaquetado terminó sin producir artefactos nuevos o actualizados en target/release/bundle.");
  }
  console.log(`Artefactos de distribución producidos: ${produced.length}.`);
}

async function main() {
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
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  await main();
}
