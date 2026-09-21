import { readFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { resolvePhysicalValidationReport, validateGate1Evidence } from "./check-beta-gate-evidence.mjs";

const projectRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const sessionAliases = ["beta-01", "beta-02", "beta-03"];

function normalized(value) {
  return String(value ?? "")
    .normalize("NFD")
    .replace(/\p{Diacritic}/gu, "")
    .trim()
    .toLocaleLowerCase("es");
}

function tableRows(markdown) {
  return markdown.split(/\r?\n/)
    .filter((line) => /^\s*\|/.test(line))
    .map((line) => line.trim().replace(/^\|/, "").replace(/\|$/, "").split("|").map((cell) => cell.trim()))
    .filter((cells) => cells.length > 1 && !cells.every((cell) => /^:?-{3,}:?$/.test(cell)));
}

function section(markdown, heading) {
  const lines = markdown.split(/\r?\n/);
  const expected = normalized(heading);
  const start = lines.findIndex((line) => /^##\s+/.test(line) && normalized(line.replace(/^##\s+/, "")) === expected);
  if (start < 0) return "";
  let end = lines.findIndex((line, index) => index > start && /^#{1,2}\s+/.test(line));
  if (end < 0) end = lines.length;
  return lines.slice(start + 1, end).join("\n");
}

function tableValue(markdown, heading, label) {
  const wanted = normalized(label);
  const row = tableRows(section(markdown, heading)).find((cells) => normalized(cells[0]) === wanted);
  return row?.[1] ?? null;
}

async function readEvidence(path, label) {
  try {
    return await readFile(path, "utf8");
  } catch (error) {
    if (error?.code === "ENOENT") throw new Error(`falta ${label} (${path.replace(`${projectRoot}\\`, "").replaceAll("\\", "/")})`);
    throw error;
  }
}

async function run() {
  const summaryPath = join(projectRoot, "docs", "reference", "beta-v1-summary.md");
  const summaryMarkdown = await readEvidence(summaryPath, "el resumen sanitizado Gate 1");
  const candidateId = tableValue(summaryMarkdown, "Release candidate", "Release candidate");
  if (!/^rc-[a-z0-9][a-z0-9.-]{2,79}$/i.test(candidateId ?? "")) {
    throw new Error("El resumen no identifica un release candidate Gate 1 real.");
  }

  const candidateRoot = join(projectRoot, ".local", "beta", candidateId);
  const manifest = JSON.parse(await readEvidence(join(candidateRoot, "manifest.json"), "el manifiesto local de Gate 1"));
  const sessions = await Promise.all(sessionAliases.map(async (alias) => ({
    alias,
    markdown: await readEvidence(join(candidateRoot, alias, "session.md"), `el formulario local ${alias}`),
  })));
  const reportRelativePath = manifest.technicalGate?.report;
  const reportPath = await resolvePhysicalValidationReport(reportRelativePath);
  const fullReport = JSON.parse(await readEvidence(reportPath, "el reporte técnico Full original"));
  const result = validateGate1Evidence({
    summaryMarkdown,
    manifest,
    sessions,
    fullReport,
    currentCommit: manifest.commit,
    gate1CommitIsAncestor: true,
    requireGate2Commit: false,
  });

  if (!result.valid) {
    console.error("Resumen Gate 1 rechazado; corrige la evidencia antes de versionarlo:");
    for (const error of result.errors) console.error(`- ${error}`);
    process.exitCode = 1;
    return;
  }
  console.log(`Resumen Gate 1 validado; RC ${candidateId} (${manifest.commit.slice(0, 7)}) es consistente con sus tres sesiones y el gate Full.`);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  run().catch((error) => {
    console.error(`Resumen Gate 1 bloqueado: ${error instanceof Error ? error.message : String(error)}`);
    process.exitCode = 1;
  });
}

