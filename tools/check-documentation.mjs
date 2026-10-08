import { readdir, readFile, stat } from "node:fs/promises";
import { basename, dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const projectRoot = resolve(fileURLToPath(new URL("..", import.meta.url)));
const requiredFiles = [
  "README.md",
  "DESIGN.md",
  "CHANGELOG.md",
  "docs/README.md",
  "docs/adr/README.md",
  "docs/adr/0001-contratos-del-repositorio.md",
  "docs/adr/0002-motores-entrega-y-distribucion.md",
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
  "docs/reference/environment-variables.md",
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

/**
 * QA-49: the Beta documents keep their structure (headings, commands, the
 * fields of the forms), not their wording, so rephrasing them passes.
 */
export function betaDocumentProblems(guide, session, summary) {
  const contracts = [
    ["docs/how-to/run-beta-validation.md", guide, [
      "## Cuándo una sesión cuenta",
      "## Gate 2: validar el shell de espacios",
      "docs/reference/beta-v1-summary.md",
      "npm run beta:prepare",
      "npm run beta:check-summary",
      "npm run beta:check-gate1",
    ]],
    ["docs/templates/beta-session.md", session, [
      "| Ronda de medición |",
      "| Release candidate |",
      "| Alias anónimo de participante | participante-___ |",
      "| Caso | Alias local de dataset | Formato | Tamaño aproximado | Filas aproximadas | Propósito |",
      "| Tarea | Resultado | Tiempo | Navegación | Datos reintroducidos | Retrocesos | Ayuda | Duda o causa | Observación sanitizada |",
      "- Acciones de navegación: ___",
      "- Datos reintroducidos: ___",
      "- Eventos de ayuda: ___",
      "- Validez de la sesión:",
      "- Flujo principal Cargar → Revisar → Preparar → Entregar: sí / no",
      "- Original sin cambios: sí / no",
    ]],
    ["docs/templates/beta-summary.md", summary, [
      "## Release candidate",
      "## Muestra agregada",
      "## Fricción agregada",
      "## Persistencia y entrega",
      "## Hallazgos y decisiones",
      "## Veredicto",
    ]],
  ];
  return contracts.flatMap(([relativePath, contents, fragments]) => {
    const missing = fragments.filter((fragment) => !contents.includes(fragment));
    return missing.length > 0 ? [`${relativePath}: ${missing.join(", ")}`] : [];
  });
}

/** DOC-11: README, CONTRIBUTING and SECURITY state the same contribution policy. */
export const CONTRIBUTION_POLICY = "Columnia es un proyecto personal de su autor: se aceptan reportes de errores en los formularios del [repositorio](https://github.com/BernieJimenez/Columnia), no se aceptan pull requests externos";

export function contributionPolicyProblems(documents) {
  const flatten = (text) => text.replace(/\s+/g, " ");
  return Object.entries(documents)
    .filter(([, contents]) => !flatten(contents).includes(CONTRIBUTION_POLICY))
    .map(([name]) => name);
}

/** DOC-07: README names the Rust release that rust-toolchain.toml pins. */
export function readmeRustProblems(readme, rustToolchain) {
  const channel = rustToolchain.match(/^channel\s*=\s*"([^"]+)"/m)?.[1];
  if (!channel) return ["rust-toolchain.toml no fija channel"];
  return readme.includes(`Rust \`${channel}\``) ? [] : [`README.md debe pedir Rust \`${channel}\` como rust-toolchain.toml`];
}

/** Living documents: not the frozen archive, the local audit or the changelog's history. */
export function isLivingDocument(relativePath) {
  return !relativePath.startsWith("docs/archive/") && !relativePath.startsWith("docs/auditorias/") && relativePath !== "CHANGELOG.md";
}

/** DOC-09: every `npm run X` a living document cites exists in package.json. */
export function missingNpmScripts(contents, scripts) {
  return [...new Set([...contents.matchAll(/\bnpm[ \t]+run[ \t]+([A-Za-z0-9:_-]+)/g)].map((match) => match[1]))]
    .filter((name) => !Object.hasOwn(scripts, name));
}

/** Files the documents tell the reader to create; they do not exist yet. */
const FILES_CREATED_LATER = new Set(["docs/reference/beta-v1-summary.md"]);

/** DOC-12: repository paths written in backticks (`docs/…`, `tools/…`, `fixtures/…`). */
export function backtickRepositoryPaths(contents) {
  return [...new Set([...contents.matchAll(/`((?:docs|tools|fixtures)\/[A-Za-z0-9._\/-]+\.[A-Za-z0-9]+)`/g)].map((match) => match[1]))]
    .filter((path) => !path.includes("<") && !path.includes("*") && !FILES_CREATED_LATER.has(path));
}

/**
 * DOC-15: a how-to or tutorial example names the current version or the
 * placeholder `<versión>`, never another release to copy by mistake.
 */
export function staleVersionExamples(contents, version) {
  const patterns = [/\/columnia\/(\d+\.\d+\.\d+)\//g, /--expected-version[ \t]+(\d+\.\d+\.\d+)/g, /Columnia_(\d+\.\d+\.\d+)_/g];
  return patterns.flatMap((pattern) => [...contents.matchAll(pattern)].map((match) => match[1]))
    .filter((cited) => cited !== version);
}

/** DOC-16: retired planning labels only inside a block marked as archived. */
export function retiredLabels(contents) {
  return contents.split(/^## /m)
    .filter((section) => !/archivad/i.test(section.split("\n")[0] ?? ""))
    .flatMap((section) => [...section.matchAll(/\b(Tier \d+|Gate 2)\b/g)].map((match) => match[1]));
}

/** QA-49: the Diátaxis entry points that docs/README.md must link. */
export function missingDocsIndexLinks(docsIndex) {
  return [
    "../DESIGN.md",
    "tutorials/first-dataset.md",
    "how-to/run-beta-validation.md",
    "templates/beta-session.md",
    "templates/beta-summary.md",
    "how-to/validate-release-evidence.md",
    "reference/cli.md",
    "explanation/local-first-architecture.md",
  ].filter((target) => !docsIndex.includes(target));
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
  // COD-19: `[dependencies]`, `[build-dependencies]` and target tables count;
  // `[dev-dependencies]` and `[workspace.dependencies]` do not. A
  // `[dependencies.foo]` table declares `foo` through its own keys, and a
  // dependency without a version is recorded by its source.
  let inDependencies = false;
  let tableDependency = null;
  const sourceOf = (body) => body.match(/version\s*=\s*"([^"]+)"/)?.[1]
    ?? (/workspace\s*=\s*true/.test(body) ? "workspace" : null)
    ?? (/\bgit\s*=/.test(body) ? "git" : null)
    ?? (/\bpath\s*=/.test(body) ? "path" : null);
  for (const rawLine of cargoToml.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (line.startsWith("[")) {
      const header = line.replace(/\s+/g, "");
      const table = header.match(/^\[(?:target\..+\.)?(?:build-)?dependencies\.([A-Za-z0-9_-]+)\]$/);
      tableDependency = table?.[1] ?? null;
      inDependencies = !table && /^\[(?:target\..+\.)?(?:build-)?dependencies\]$/.test(header);
      continue;
    }
    if (!line || line.startsWith("#")) continue;
    if (tableDependency) {
      const source = sourceOf(line);
      if (source && (source !== "path" || !dependencies.has(tableDependency))) dependencies.set(tableDependency, source);
      continue;
    }
    if (!inDependencies) continue;
    const match = line.match(/^([A-Za-z0-9_-]+)\s*=\s*(?:"([^"]+)"|(\{.*))/);
    if (!match) continue;
    const source = match[2] ?? sourceOf(match[3] ?? "");
    if (source) dependencies.set(match[1], source);
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

/**
 * OPS-09: local link targets of a Markdown text, ignoring fenced blocks and
 * inline code, titles (`](path "title")`) and URL escapes such as `%20`.
 */
export function localLinkTargets(contents) {
  const prose = contents
    .replace(/^(```|~~~)[^\n]*\n[\s\S]*?^\1[^\n]*$/gm, "")
    .replace(/(`+)[^`\n]*?\1/g, "");
  const targets = [];
  for (const match of prose.matchAll(/\]\(\s*<?([^)\s#>]+)>?(?:\s+"[^"]*")?\s*(?:#[^)]*)?\)/g)) {
    const target = match[1];
    if (/^(https?|mailto):/i.test(target)) continue;
    try {
      targets.push(decodeURIComponent(target));
    } catch {
      targets.push(target);
    }
  }
  return targets;
}

/**
 * DOC-01: figures that only the generated inventory or a test run can know.
 * Written by hand in a living document they go stale, so they are rejected.
 */
export function handWrittenInventoryFigures(contents) {
  const pattern = /\b\d+ (?:comandos de producción|estructuras compartidas|estructuras del inventario|pruebas (?:frontend|Rust)|tests (?:frontend|Rust))\b/g;
  return [...contents.matchAll(pattern)].map((match) => match[0]);
}

/**
 * DOC-10: `COLUMNIA_*` variables the code names but the single table in
 * docs/reference/environment-variables.md does not list.
 */
export function undocumentedEnvironmentVariables(sources, table) {
  const names = new Set();
  for (const source of sources) {
    for (const match of source.matchAll(/(?<![A-Za-z0-9_])COLUMNIA_[A-Z0-9_]*[A-Z0-9](?![A-Za-z0-9_])/g)) names.add(match[0]);
  }
  return [...names].filter((name) => !table.includes(`\`${name}\``)).sort();
}

/**
 * DOC-14: the subcommands documented in docs/reference/cli.md (one `###`
 * heading each) against those of the CLI help in src-tauri/src/automation.rs.
 */
export function cliSubcommandProblems(reference, automationSource) {
  const documented = new Set([...reference.matchAll(/^### `([a-z-]+)`$/gm)].map((match) => match[1]));
  const help = automationSource.match(/const GENERAL_HELP: &str = "([^"]*(?:\\"[^"]*)*)"/)?.[1] ?? "";
  const implemented = new Set([...help.matchAll(/columnia-cli ([a-z-]+)/g)].map((match) => match[1]));
  return [
    ...[...implemented].filter((name) => !documented.has(name)).map((name) => `${name} falta en cli.md`),
    ...[...documented].filter((name) => !implemented.has(name)).map((name) => `${name} no existe en la CLI`),
  ];
}

/** Documents whose links are not maintained: frozen snapshots and audit drafts. */
export function skipsLinkCheck(relativePath) {
  return relativePath.startsWith("docs/archive/") || /^docs\/auditorias\/[^/]+\/parciales\//.test(relativePath);
}

/** Whether a path exists with exactly this spelling (Windows ignores case). */
async function existsWithExactCase(path) {
  try {
    await stat(path);
  } catch (error) {
    if (error?.code === "ENOENT" || error?.code === "ENOTDIR") return false;
    throw error;
  }
  const parent = dirname(path);
  if (parent === path) return true;
  return (await readdir(parent)).includes(basename(path));
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

async function codeFiles(root) {
  const path = absolute(root);
  const details = await stat(path);
  if (details.isFile()) return [root];
  const files = [];
  for (const entry of await readdir(path, { withFileTypes: true })) {
    const child = `${root}/${entry.name}`;
    if (entry.isDirectory()) files.push(...await codeFiles(child));
    else if (/\.(?:rs|ts|tsx|mjs|ps1)$/.test(entry.name)) files.push(child);
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
  // The living documents stay short; their history is archived, not deleted.
  for (const [document, contents] of [
    ["ROADMAP.md", await readUtf8("ROADMAP.md")],
    ["CONTEXTO.md", await readUtf8("CONTEXTO.md")],
    ["AUDITORIA.md", auditDocument],
  ]) {
    // QA-49: any archive folder; the link check below proves it exists.
    if (!/\]\([^)]*docs\/archive\//.test(contents)) fail(`${document} debe enlazar su historial archivado en docs/archive/.`);
  }
  const policyProblems = contributionPolicyProblems({
    "README.md": readme,
    "CONTRIBUTING.md": await readUtf8("CONTRIBUTING.md"),
    "SECURITY.md": await readUtf8("SECURITY.md"),
  });
  if (policyProblems.length > 0) fail(`La política de contribución no coincide en: ${policyProblems.join(", ")}.`);
  const missingIndexLinks = missingDocsIndexLinks(docsIndex);
  if (missingIndexLinks.length > 0) {
    fail(`docs/README.md no enlaza los puntos de entrada Diátaxis: ${missingIndexLinks.join(", ")}.`);
  }
  const betaProblems = betaDocumentProblems(betaGuide, betaSession, betaSummary);
  if (betaProblems.length > 0) fail(`Los documentos Beta no conservan su estructura: ${betaProblems.join("; ")}.`);
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

  const files = [];
  for (const root of markdownRoots) files.push(...await markdownFiles(root));
  const brokenLinks = [];
  for (const relativePath of files) {
    const contents = await readUtf8(relativePath);
    if (skipsLinkCheck(relativePath)) continue;
    for (const target of localLinkTargets(contents)) {
      const targetPath = resolve(dirname(absolute(relativePath)), target);
      if (!await existsWithExactCase(targetPath)) brokenLinks.push(`${relativePath} -> ${target}`);
    }
  }
  if (brokenLinks.length > 0) fail(`Enlaces locales rotos: ${brokenLinks.join(", ")}`);
  const livingProblems = [...readmeRustProblems(readme, await readUtf8("rust-toolchain.toml"))];
  for (const relativePath of files) {
    const contents = await readUtf8(relativePath);
    if (relativePath === "CHANGELOG.md" || isLivingDocument(relativePath)) {
      for (const path of backtickRepositoryPaths(contents)) {
        if (!await existsWithExactCase(absolute(path))) livingProblems.push(`${relativePath} cita \`${path}\`, que no existe`);
      }
    }
    if (!isLivingDocument(relativePath)) continue;
    for (const name of missingNpmScripts(contents, packageManifest.scripts ?? {})) {
      livingProblems.push(`${relativePath} cita npm run ${name}, que package.json no define`);
    }
    if (relativePath.startsWith("docs/how-to/") || relativePath.startsWith("docs/tutorials/")) {
      for (const cited of staleVersionExamples(contents, version)) {
        livingProblems.push(`${relativePath} usa la versión ${cited} como ejemplo (escribe <versión>)`);
      }
      for (const label of retiredLabels(contents)) {
        livingProblems.push(`${relativePath} menciona «${label}» fuera de un bloque archivado`);
      }
    }
  }
  if (livingProblems.length > 0) fail(`Documentación desfasada: ${livingProblems.join("; ")}.`);
  const environmentSources = [];
  for (const root of ["src", "src-tauri/src", "src-tauri/build.rs", "tools", "e2e"]) {
    for (const file of await codeFiles(root)) environmentSources.push((await readFile(absolute(file), "utf8")).replace(/^\uFEFF/, ""));
  }
  const undocumented = undocumentedEnvironmentVariables(environmentSources, await readUtf8("docs/reference/environment-variables.md"));
  if (undocumented.length > 0) fail(`Variables de entorno sin documentar en docs/reference/environment-variables.md: ${undocumented.join(", ")}`);
  const cliProblems = cliSubcommandProblems(await readUtf8("docs/reference/cli.md"), await readUtf8("src-tauri/src/automation.rs"));
  if (cliProblems.length > 0) fail(`docs/reference/cli.md no coincide con la CLI: ${cliProblems.join(", ")}`);
  // AUDITORIA.md keeps its counts because the checks above verify them.
  for (const living of ["CONTEXTO.md", "THREAT_MODEL.md", "README.md"]) {
    const figures = handWrittenInventoryFigures(await readUtf8(living));
    if (figures.length > 0) fail(`${living} repite cifras que caducan (${figures.join(", ")}); remite a docs/reference/ipc-inventory.json o a la evidencia fechada.`);
  }

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
