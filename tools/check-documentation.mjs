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
  "docs/how-to/validate-release-evidence.md",
  "docs/reference/cli.md",
  "docs/reference/release-evidence.md",
  "docs/explanation/local-first-architecture.md",
];
const markdownRoots = ["README.md", "CONTRIBUTING.md", "CHANGELOG.md", "docs"];
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
  const tauriConfig = JSON.parse(await readUtf8("src-tauri/tauri.conf.json"));
  const cargoManifest = await readUtf8("src-tauri/Cargo.toml");
  const cargoLock = await readUtf8("src-tauri/Cargo.lock");
  const changelog = await readUtf8("CHANGELOG.md");
  const docsIndex = await readUtf8("docs/README.md");
  const version = packageManifest.version;
  const cargoVersion = cargoManifest.match(/^version = "([^"]+)"$/m)?.[1];
  const cargoPackageBlock = cargoLock.split(/^\[\[package\]\]\s*$/m).find((block) => /^name = "columnia"$/m.test(block));
  const cargoLockVersion = cargoPackageBlock?.match(/^version = "([^"]+)"$/m)?.[1];
  if (!version || packageLock.version !== version || packageLock.packages?.[""].version !== version || tauriConfig.version !== version || cargoVersion !== version || cargoLockVersion !== version) {
    fail("La versión de npm, lockfile, Cargo, Cargo.lock y Tauri no está sincronizada.");
  }
  if (!changelog.includes(`[${version}]`)) fail(`CHANGELOG.md no contiene la versión ${version}.`);
  if (!docsIndex.includes("tutorials/first-dataset.md") || !docsIndex.includes("how-to/validate-release-evidence.md") || !docsIndex.includes("reference/cli.md") || !docsIndex.includes("explanation/local-first-architecture.md")) {
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
