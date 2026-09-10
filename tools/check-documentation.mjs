import { readdir, readFile, stat } from "node:fs/promises";
import { join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const projectRoot = resolve(fileURLToPath(new URL("..", import.meta.url)));
const requiredFiles = [
  "README.md",
  "CHANGELOG.md",
  "docs/README.md",
  "docs/adr/README.md",
  "docs/adr/0001-contratos-del-repositorio.md",
  "docs/tutorials/first-dataset.md",
  "docs/how-to/run-beta-validation.md",
  "docs/how-to/validate-release-evidence.md",
  "docs/templates/beta-session.md",
  "docs/reference/cli.md",
  "docs/reference/v1-scope.md",
  "docs/reference/release-evidence.md",
  "ROADMAP.md",
  "CONTEXTO.md",
  "AUDITORIA_PROFESIONAL_2026-08-28.md",
  "THREAT_MODEL.md",
  "docs/reference/ipc-inventory.json",
  "docs/reference/legal-distribution-decision.json",
  "docs/explanation/local-first-architecture.md",
];
const markdownRoots = ["README.md", "CONTRIBUTING.md", "CHANGELOG.md", "ROADMAP.md", "CONTEXTO.md", "AUDITORIA_PROFESIONAL_2026-08-28.md", "THREAT_MODEL.md", "docs"];
const imageExtensions = new Set([".png", ".jpg", ".jpeg", ".gif", ".webp"]);
const decoder = new TextDecoder("utf-8", { fatal: true });

function fail(message) {
  throw new Error(message);
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

try {
  for (const relativePath of requiredFiles) {
    await readUtf8(relativePath);
  }
  const packageManifest = JSON.parse(await readUtf8("package.json"));
  const packageLock = JSON.parse(await readUtf8("package-lock.json"));
  const dependencyAudit = await readUtf8("docs/reference/dependency-audit.md");
  const ipcInventory = JSON.parse(await readUtf8("docs/reference/ipc-inventory.json"));
  const tauriConfig = JSON.parse(await readUtf8("src-tauri/tauri.conf.json"));
  const legalDecision = JSON.parse(await readUtf8("docs/reference/legal-distribution-decision.json"));
  const cargoManifest = await readUtf8("src-tauri/Cargo.toml");
  const cargoLock = await readUtf8("src-tauri/Cargo.lock");
  const changelog = await readUtf8("CHANGELOG.md");
  const docsIndex = await readUtf8("docs/README.md");
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
  if (!changelog.includes(`[${version}]`)) fail(`CHANGELOG.md no contiene la versión ${version}.`);
  if (!dependencyAudit.includes(`sobre \`${version}\``)) fail("La ficha de dependencias no está actualizada a la versión del proyecto.");
  const npmAuditCount = dependencyAudit.match(/`npm audit --json --omit=optional`[^|]*\|[^|]*; (\d+) dependencias del lockfile/);
  if (!npmAuditCount || Number(npmAuditCount[1]) !== packageCount) {
    fail(`La ficha de dependencias no coincide con package-lock.json: declara ${npmAuditCount?.[1] ?? "sin conteo"}, actual ${packageCount}.`);
  }
  const ipcAuditCount = dependencyAudit.match(/`npm run ipc:check`[^|]*\| Aprobado; (\d+) comandos de producción, (\d+) debug y (\d+) estructuras compartidas/);
  const expectedIpcCount = [ipcInventory.productionCommands?.length, ipcInventory.debugCommands?.length, ipcInventory.sharedStructures?.length];
  if (!ipcAuditCount || expectedIpcCount.some((count, index) => Number(ipcAuditCount[index + 1]) !== count)) {
    fail(`La ficha de dependencias no coincide con el inventario IPC: declara ${ipcAuditCount?.[1] ?? "sin conteo"}/${ipcAuditCount?.[2] ?? "sin conteo"}/${ipcAuditCount?.[3] ?? "sin conteo"}, actual ${expectedIpcCount.join("/")}.`);
  }
  if (!changelog.includes("Tier 5")) fail("CHANGELOG.md no documenta el estado de Tier 5.");
  if (!await readUtf8("ROADMAP.md").then((roadmap) => roadmap.includes("Tier 5"))) fail("ROADMAP.md no contiene el roadmap Tier 5.");
  if (!await readUtf8("CONTEXTO.md").then((context) => context.includes("Tier 5"))) fail("CONTEXTO.md no contiene el contexto Tier 5.");
  if (!await readUtf8("AUDITORIA_PROFESIONAL_2026-08-28.md").then((audit) => audit.includes("T5-"))) fail("El informe de auditoría no contiene la trazabilidad Tier 5.");
  if (!docsIndex.includes("tutorials/first-dataset.md") || !docsIndex.includes("how-to/run-beta-validation.md") || !docsIndex.includes("templates/beta-session.md") || !docsIndex.includes("how-to/validate-release-evidence.md") || !docsIndex.includes("reference/cli.md") || !docsIndex.includes("explanation/local-first-architecture.md")) {
    fail("docs/README.md no expone los cuatro cuadrantes Diátaxis.");
  }

  const files = [];
  for (const root of markdownRoots) files.push(...await markdownFiles(root));
  const brokenLinks = [];
  for (const relativePath of files) {
    const contents = await readUtf8(relativePath);
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
