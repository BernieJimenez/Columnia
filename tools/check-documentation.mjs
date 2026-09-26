import { readdir, readFile, stat } from "node:fs/promises";
import { join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const projectRoot = resolve(fileURLToPath(new URL("..", import.meta.url)));
const requiredFiles = [
  "README.md",
  "DESIGN.md",
  "CHANGELOG.md",
  "docs/README.md",
  "docs/adr/README.md",
  "docs/adr/0001-contratos-del-repositorio.md",
  "docs/tutorials/first-dataset.md",
  "docs/how-to/run-beta-validation.md",
  "docs/how-to/validate-release-evidence.md",
  "docs/templates/beta-session.md",
  "docs/templates/beta-summary.md",
  "tools/prepare-beta-gate.ps1",
  "tools/check-beta-gate-evidence.mjs",
  "tools/check-beta-summary.mjs",
  "docs/reference/cli.md",
  "docs/reference/v1-scope.md",
  "docs/reference/release-evidence.md",
  "ROADMAP.md",
  "CONTEXTO.md",
  "AUDITORIA.md",
  "THREAT_MODEL.md",
  "docs/reference/ipc-inventory.json",
  "docs/reference/legal-distribution-decision.json",
  "docs/explanation/local-first-architecture.md",
];
const markdownRoots = ["README.md", "CONTRIBUTING.md", "CHANGELOG.md", "ROADMAP.md", "CONTEXTO.md", "AUDITORIA.md", "THREAT_MODEL.md", "docs"];
const imageExtensions = new Set([".png", ".jpg", ".jpeg", ".gif", ".webp"]);
const decoder = new TextDecoder("utf-8", { fatal: true });

function fail(message) {
  throw new Error(message);
}

function requireFragments(relativePath, contents, fragments) {
  const missing = fragments.filter((fragment) => !contents.includes(fragment));
  if (missing.length > 0) {
    fail(`${relativePath} no conserva el contrato requerido: ${missing.join(", ")}.`);
  }
}

/**
 * A version counts only as a Keep a Changelog heading, never as a mention in
 * prose. Release profiles require the version's own section; development
 * accepts pending changes under [Unreleased].
 */
export function validateChangelogVersion(changelog, version, { requireReleaseSection = false } = {}) {
  const escaped = version.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const hasVersionSection = new RegExp(`^## \\[${escaped}\\](?:\\s|$)`, "m").test(changelog);
  const hasUnreleasedSection = /^## \[Unreleased\]\s*$/m.test(changelog);
  if (requireReleaseSection && !hasVersionSection) {
    throw new Error(`CHANGELOG.md no tiene la sección "## [${version}]" que exige la publicación.`);
  }
  if (!hasVersionSection && !hasUnreleasedSection) {
    throw new Error(`CHANGELOG.md no tiene la sección "## [${version}]" ni "## [Unreleased]".`);
  }
}

/** Declared Cargo dependencies (normal, target-specific and build) as name → version. */
export function parseCargoDependencies(cargoToml) {
  const dependencies = new Map();
  let inDependencies = false;
  for (const rawLine of cargoToml.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (line.startsWith("[")) {
      inDependencies = /dependencies\]$/.test(line) && !line.includes("dev-dependencies");
      continue;
    }
    if (!inDependencies || !line || line.startsWith("#")) continue;
    const match = line.match(/^([A-Za-z0-9_-]+)\s*=\s*(?:"([^"]+)"|\{[^}]*version\s*=\s*"([^"]+)")/);
    if (match) dependencies.set(match[1], match[2] ?? match[3]);
  }
  return dependencies;
}

/** The dependency sheet in AUDITORIA.md must match both manifests exactly. */
export function validateDependencySnapshot(auditDocument, packageManifest, cargoToml) {
  const problems = [];
  const declaredNpm = new Map(Object.entries({ ...packageManifest.dependencies, ...packageManifest.devDependencies }));
  const documentedNpm = new Map(
    [...auditDocument.matchAll(/^\| (?:runtime|desarrollo) \| `([^`]+)` \| `([^`]+)` \|$/gm)].map((match) => [match[1], match[2]]),
  );
  const cargoSection = auditDocument.split(/^#### Cargo\s*$/m)[1]?.split(/^###/m)[0] ?? "";
  const documentedCargo = new Map(
    [...cargoSection.matchAll(/`([A-Za-z0-9_-]+) ([0-9][^`]*)`/g)].map((match) => [match[1], match[2]]),
  );
  const compare = (label, declared, documented) => {
    for (const [name, version] of declared) {
      if (documented.get(name) !== version) problems.push(`${label} ${name}: manifiesto ${version}, ficha ${documented.get(name) ?? "ausente"}`);
    }
    for (const name of documented.keys()) {
      if (!declared.has(name)) problems.push(`${label} ${name}: en la ficha pero no en el manifiesto`);
    }
  };
  compare("npm", declaredNpm, documentedNpm);
  compare("Cargo", parseCargoDependencies(cargoToml), documentedCargo);
  return problems;
}

export function validateReadmeSetupContract(readme, packageManifest) {
  const heading = "## Ejecutar desde el código fuente";
  const sectionStart = readme.indexOf(heading);
  if (sectionStart < 0) fail(`README.md no contiene la sección ${heading}.`);
  const nextHeading = readme.indexOf("\n## ", sectionStart + heading.length);
  const section = readme.slice(sectionStart, nextHeading < 0 ? undefined : nextHeading);

  const requirements = section.match(/^Requisitos:\s*(.+)$/m)?.[1];
  const engines = packageManifest.engines ?? {};
  for (const [label, runtime, range] of [
    ["Node.js", "node", engines.node],
    ["npm", "npm", engines.npm],
  ]) {
    if (typeof range !== "string" || range.length === 0) {
      fail(`package.json debe declarar engines.${runtime} para validar README.md.`);
    }
    if (!requirements?.includes(`${label} \`${range}\``)) {
      fail(`README.md debe declarar ${label} \`${range}\` según package.json engines.${runtime}.`);
    }
  }

  const scripts = packageManifest.scripts ?? {};
  const documentedScripts = [];
  for (const match of section.matchAll(/\bnpm[ \t]+(run(?:[ \t]+([^\s`]+))?|test)\b/g)) {
    const scriptName = match[1] === "test" ? "test" : match[2]?.replace(/[.,;)]+$/g, "");
    if (!scriptName) fail("README.md contiene `npm run` sin nombre de script.");
    if (!Object.hasOwn(scripts, scriptName)) {
      const command = scriptName === "test" ? "npm test" : `npm run ${scriptName}`;
      fail(`README.md documenta ${command}, pero package.json no define ese script.`);
    }
    documentedScripts.push(scriptName);
  }
  if (documentedScripts.length === 0) fail("README.md debe documentar comandos npm vinculados a package.json scripts.");
}

function markdownTableCellCount(line) {
  const trimmed = line.trim();
  if (!trimmed.startsWith("|") || !trimmed.endsWith("|")) return null;
  return trimmed.slice(1, -1).split("|").length;
}

function requireConsistentMarkdownTable(relativePath, contents, header) {
  const lines = contents.split(/\r?\n/);
  const headerIndex = lines.findIndex((line) => line.trim() === header);
  if (headerIndex < 0) fail(`${relativePath} no contiene la tabla esperada.`);

  const expectedCells = markdownTableCellCount(lines[headerIndex]);
  for (let index = headerIndex + 1; index < lines.length; index += 1) {
    const line = lines[index].trim();
    if (!line.startsWith("|")) break;
    const actualCells = markdownTableCellCount(line);
    if (actualCells !== expectedCells) {
      fail(`${relativePath}:${index + 1} tiene ${actualCells ?? "un formato inválido de"} celdas; se esperaban ${expectedCells}.`);
    }
  }
}

function absolute(relativePath) {
  return join(projectRoot, relativePath.replaceAll("/", "\\"));
}

async function readUtf8(relativePath) {
  const bytes = await readFile(absolute(relativePath));
  if (bytes.length >= 3 && bytes[0] === 0xef && bytes[1] === 0xbb && bytes[2] === 0xbf) {
    fail(`${relativePath} contiene BOM; usa UTF-8 sin BOM.`);
  }
  return decoder.decode(bytes);
}

async function markdownFiles(root) {
  const path = absolute(root);
  const details = await stat(path);
  if (details.isFile()) return [root];
  const entries = await readdir(path, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const child = `${root}/${entry.name}`;
    if (entry.isDirectory()) files.push(...await markdownFiles(child));
    else if (entry.name.endsWith(".md")) files.push(child);
  }
  return files;
}

async function imagesUnder(root) {
  const path = absolute(root);
  const entries = await readdir(path, { withFileTypes: true });
  const images = [];
  for (const entry of entries) {
    const child = `${root}/${entry.name}`;
    if (entry.isDirectory()) images.push(...await imagesUnder(child));
    else if (imageExtensions.has(entry.name.slice(entry.name.lastIndexOf(".")).toLowerCase())) images.push(child);
  }
  return images;
}

async function main() {
try {
  for (const relativePath of requiredFiles) {
    await readUtf8(relativePath);
  }
  const obsoleteAuditFiles = (await readdir(projectRoot))
    .filter((name) => /^AUDITORIA_.+\.md$/u.test(name));
  obsoleteAuditFiles.push(...(await markdownFiles("docs"))
    .filter((relativePath) => relativePath === "docs/reference/dependency-audit.md"));
  if (obsoleteAuditFiles.length > 0) {
    fail(`Las auditorías independientes deben estar fusionadas en AUDITORIA.md: ${obsoleteAuditFiles.join(", ")}.`);
  }
  const packageManifest = JSON.parse(await readUtf8("package.json"));
  const readme = await readUtf8("README.md");
  validateReadmeSetupContract(readme, packageManifest);
  const packageLock = JSON.parse(await readUtf8("package-lock.json"));
  const auditDocument = await readUtf8("AUDITORIA.md");
  const ipcInventory = JSON.parse(await readUtf8("docs/reference/ipc-inventory.json"));
  const tauriConfig = JSON.parse(await readUtf8("src-tauri/tauri.conf.json"));
  const legalDecision = JSON.parse(await readUtf8("docs/reference/legal-distribution-decision.json"));
  const cargoManifest = await readUtf8("src-tauri/Cargo.toml");
  const cargoLock = await readUtf8("src-tauri/Cargo.lock");
  const changelog = await readUtf8("CHANGELOG.md");
  const docsIndex = await readUtf8("docs/README.md");
  const betaGuide = await readUtf8("docs/how-to/run-beta-validation.md");
  const betaSession = await readUtf8("docs/templates/beta-session.md");
  const betaSummary = await readUtf8("docs/templates/beta-summary.md");
  const version = packageManifest.version;
  const packageCount = Math.max(0, Object.keys(packageLock.packages ?? {}).length - 1);
  const cargoVersion = cargoManifest.match(/^version = "([^"]+)"$/m)?.[1];
  const cargoPackageBlock = cargoLock.split(/^\[\[package\]\]\s*$/m).find((block) => /^name = "columnia"$/m.test(block));
  const cargoLockVersion = cargoPackageBlock?.match(/^version = "([^"]+)"$/m)?.[1];
  if (!version || packageLock.version !== version || packageLock.packages?.[""].version !== version || tauriConfig.version !== version || cargoVersion !== version || cargoLockVersion !== version) {
    fail("La versión de npm, lockfile, Cargo, Cargo.lock y Tauri no está sincronizada.");
  }
  if (legalDecision.schemaVersion !== 1 || !["pending-legal-review", "source-publication-approved", "approved"].includes(legalDecision.status)) {
    fail("La ficha legal/distribución debe usar schemaVersion 1 y un estado conocido.");
  }
  try {
    validateChangelogVersion(changelog, version, { requireReleaseSection: process.argv.includes("--require-release-section") });
  } catch (error) {
    fail(error.message);
  }
  if (!auditDocument.includes(`sobre \`${version}\``)) fail("La ficha de dependencias no está actualizada a la versión del proyecto.");
  const dependencyProblems = validateDependencySnapshot(auditDocument, packageManifest, cargoManifest);
  if (dependencyProblems.length > 0) fail(`La ficha de dependencias de AUDITORIA.md no coincide con los manifiestos: ${dependencyProblems.join("; ")}.`);
  const npmAuditCount = auditDocument.match(/`npm audit --json --omit=optional`[^|]*\|[^|]*; (\d+) dependencias del lockfile/);
  if (!npmAuditCount || Number(npmAuditCount[1]) !== packageCount) {
    fail(`La ficha de dependencias no coincide con package-lock.json: declara ${npmAuditCount?.[1] ?? "sin conteo"}, actual ${packageCount}.`);
  }
  const ipcAuditCount = auditDocument.match(/`npm run ipc:check`[^|]*\| Aprobado; (\d+) comandos de producción, (\d+) debug y (\d+) estructuras compartidas/);
  const expectedIpcCount = [ipcInventory.productionCommands?.length, ipcInventory.debugCommands?.length, ipcInventory.sharedStructures?.length];
  if (!ipcAuditCount || expectedIpcCount.some((count, index) => Number(ipcAuditCount[index + 1]) !== count)) {
    fail(`La ficha de dependencias no coincide con el inventario IPC: declara ${ipcAuditCount?.[1] ?? "sin conteo"}/${ipcAuditCount?.[2] ?? "sin conteo"}/${ipcAuditCount?.[3] ?? "sin conteo"}, actual ${expectedIpcCount.join("/")}.`);
  }
  if (!changelog.includes("Tier 5")) fail("CHANGELOG.md no documenta el estado de Tier 5.");
  // The living documents stay short; their history is archived, not deleted.
  for (const [document, contents] of [
    ["ROADMAP.md", await readUtf8("ROADMAP.md")],
    ["CONTEXTO.md", await readUtf8("CONTEXTO.md")],
    ["AUDITORIA.md", auditDocument],
  ]) {
    if (!contents.includes("docs/archive/2026-09/")) fail(`${document} debe enlazar su historial archivado en docs/archive/2026-09/.`);
  }
  for (const archived of ["ROADMAP.md", "CONTEXTO.md", "AUDITORIA.md", "historial-verificacion.md", "roadmap-current.md"]) {
    await readUtf8(`docs/archive/2026-09/${archived}`);
  }
  if (!docsIndex.includes("../DESIGN.md") || !docsIndex.includes("tutorials/first-dataset.md") || !docsIndex.includes("how-to/run-beta-validation.md") || !docsIndex.includes("templates/beta-session.md") || !docsIndex.includes("templates/beta-summary.md") || !docsIndex.includes("how-to/validate-release-evidence.md") || !docsIndex.includes("reference/cli.md") || !docsIndex.includes("explanation/local-first-architecture.md")) {
    fail("docs/README.md no expone los cuatro cuadrantes Diátaxis.");
  }
  requireFragments("docs/how-to/run-beta-validation.md", betaGuide, [
    "## Cuándo una sesión cuenta",
    "## Gate 2: validar el shell de espacios",
    "24 de las 30 tareas agregadas",
    "una tarea no completada no invalida por sí",
    "docs/reference/beta-v1-summary.md",
    "npm run beta:prepare",
    "npm run beta:check-summary",
    "npm run beta:check-gate1",
    "valida el resumen, el manifiesto,",
    "acciones de navegación o los datos reintroducidos bajan al menos 20 %",
    "alias anónimo estable",
    "dataset-01",
  ]);
  requireFragments("docs/templates/beta-session.md", betaSession, [
    "| Ronda de medición |",
    "| Release candidate |",
    "| Alias anónimo de participante | participante-___ |",
    "| Caso | Alias local de dataset | Formato | Tamaño aproximado | Filas aproximadas | Propósito |",
    "| Tarea | Resultado | Tiempo | Navegación | Datos reintroducidos | Retrocesos | Ayuda | Duda o causa | Observación sanitizada |",
    "- Acciones de navegación: ___",
    "- Datos reintroducidos: ___",
    "- Eventos de ayuda: ___",
    "- Validez de la sesión:",
    "`no completada` es un resultado",
    "- Flujo principal Cargar → Revisar → Preparar → Entregar: sí / no",
    "- Original sin cambios: sí / no",
  ]);
  requireConsistentMarkdownTable(
    "docs/templates/beta-session.md",
    betaSession,
    "| Caso | Alias local de dataset | Formato | Tamaño aproximado | Filas aproximadas | Propósito |",
  );
  requireConsistentMarkdownTable(
    "docs/templates/beta-session.md",
    betaSession,
    "| Tarea | Resultado | Tiempo | Navegación | Datos reintroducidos | Retrocesos | Ayuda | Duda o causa | Observación sanitizada |",
  );
  requireFragments("docs/templates/beta-summary.md", betaSummary, [
    "## Release candidate",
    "## Muestra agregada",
    "## Fricción agregada",
    "## Persistencia y entrega",
    "## Hallazgos y decisiones",
    "## Veredicto",
    "(baseline - Gate 2) / baseline × 100",
    "Los alias anónimos de participantes y datasets permanecen en los formularios",
    "Revisión de privacidad del resumen",
    "redondea a un decimal",
  ]);

  const files = [];
  for (const root of markdownRoots) files.push(...await markdownFiles(root));
  const brokenLinks = [];
  for (const relativePath of files) {
    const contents = await readUtf8(relativePath);
    // Archived documents are frozen snapshots: their relative links are not maintained.
    if (relativePath.startsWith("docs/archive/")) continue;
    for (const match of contents.matchAll(/\]\(([^)#]+)(?:#[^)]+)?\)/g)) {
      const target = match[1];
      if (/^(https?|mailto):/i.test(target)) continue;
      const targetPath = resolve(absolute(relativePath).replace(/[\\/][^\\/]+$/, ""), target);
      try { await stat(targetPath); } catch { brokenLinks.push(`${relativePath} -> ${target}`); }
    }
  }
  if (brokenLinks.length > 0) fail(`Enlaces locales rotos: ${brokenLinks.join(", ")}`);

  for (const imageRoot of ["docs", "fixtures"]) {
    for (const image of await imagesUnder(imageRoot)) {
      const metadataPath = `${image}.meta.json`;
      try {
        const metadata = JSON.parse(await readUtf8(metadataPath));
        if (!metadata.owner || !metadata.date || !metadata.purpose) fail(`${image}.meta.json debe tener owner, date y purpose.`);
      } catch (error) {
        if (error instanceof SyntaxError || error?.code === "ENOENT") fail(`${image} requiere metadata ${metadataPath}.`);
        throw error;
      }
    }
  }

  console.log(`Documentación aprobada: ${files.length} Markdown, UTF-8, enlaces, versiones y ownership de imágenes.`);
} catch (error) {
  console.error(`Gate de documentación falló: ${error instanceof Error ? error.message : String(error)}`);
  process.exitCode = 1;
}
}

if (process.argv[1] && pathToFileURL(resolve(process.argv[1])).href === import.meta.url) {
  await main();
}
