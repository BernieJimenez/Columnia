import { existsSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const projectRoot = resolve(fileURLToPath(new URL("..", import.meta.url)));
const requireSignoff = process.argv.includes("--require-signoff");
const decisionPath = join(projectRoot, "docs", "reference", "legal-distribution-decision.json");
const requiredFiles = [
  "LICENSE",
  "THIRD_PARTY_NOTICES.md",
  "docs/reference/legal-distribution-review.md",
  "docs/reference/legal-distribution-decision.json",
];
const requiredFields = [
  "responsibleEntity",
  "jurisdiction",
  "contact",
  "markets",
  "channel",
  "privacyPolicy",
  "retentionPolicy",
  "trademarkReview",
  "thirdPartyNoticesReview",
  "updaterDistributionReview",
];
// LEG-02: also the Spanish and symbolic ways of leaving a field unresolved.
const placeholderPattern = /^(?:pending|pending[-_ ](?:legal|review)|pendiente|todo|tbd|n\/a|n\/d|none|ninguno|ninguna|por definir|xxx+|example|ejemplo|-+|\?+|\.+)$/i;

/** LEG-02: a decision text is resolved only when it says something. */
function resolvedText(value) {
  return typeof value === "string" && value.trim().length >= 3 && !placeholderPattern.test(value.trim());
}

/** The required decision fields that are still empty or placeholders; every item of a list counts. */
export function unresolvedDecisionFields(decision) {
  return requiredFields.filter((field) => {
    const value = decision?.[field];
    if (Array.isArray(value)) return value.length === 0 || !value.every(resolvedText);
    return !resolvedText(value);
  });
}

/** LEG-02: a real calendar date, not in the future. */
export function validReviewDate(value, today = new Date()) {
  if (typeof value !== "string" || !/^\d{4}-\d{2}-\d{2}$/.test(value)) return false;
  const date = new Date(`${value}T00:00:00Z`);
  return !Number.isNaN(date.getTime()) && date.toISOString().slice(0, 10) === value && date.getTime() <= today.getTime();
}

function fail(message) {
  console.error(`Gate legal/distribución falló: ${message}`);
  process.exit(1);
}

function main() {
  for (const relativePath of requiredFiles) {
    // LEG-02: the platform separator, not a Windows-only one.
    if (!existsSync(join(projectRoot, ...relativePath.split("/")))) {
      fail(`falta el archivo requerido ${relativePath}`);
    }
  }

  let decision;
  try {
    decision = JSON.parse(readFileSync(decisionPath, "utf8"));
  } catch (error) {
    fail(`la ficha de decisiones no es JSON válido: ${error instanceof Error ? error.message : String(error)}`);
  }

  if (decision.schemaVersion !== 1) fail("la ficha de decisiones debe usar schemaVersion 1.");
  if (!["pending-legal-review", "source-publication-approved", "approved"].includes(decision.status)) {
    fail("el estado de la ficha debe ser pending-legal-review, source-publication-approved o approved.");
  }
  if (!validReviewDate(decision.lastReviewed)) {
    fail("lastReviewed debe ser una fecha ISO YYYY-MM-DD real y no futura.");
  }
  if (!decision.decision || typeof decision.decision !== "object" || Array.isArray(decision.decision)) {
    fail("la ficha debe contener el objeto decision.");
  }
  for (const field of requiredFields) {
    if (!(field in decision.decision)) fail(`falta el campo decision.${field}.`);
  }

  const unresolved = unresolvedDecisionFields(decision.decision);

  if (requireSignoff) {
    if (decision.status !== "approved") {
      fail("el perfil de distribución exige status=approved y la aprobación jurídica documentada.");
    }
    if (unresolved.length > 0) {
      fail(`la aprobación no está completa; faltan: ${unresolved.join(", ")}.`);
    }
    console.log("Gate legal/distribución aprobado: ficha jurídica completa y artefactos técnicos presentes.");
    process.exit(0);
  }

  if (["source-publication-approved", "approved"].includes(decision.status) && unresolved.length > 0) {
    fail(`status=${decision.status} pero faltan decisiones: ${unresolved.join(", ")}.`);
  }
  if (unresolved.length > 0) {
    console.log(`Gate técnico legal/distribución aprobado; aprobación jurídica pendiente (${unresolved.length} campos).`);
  } else if (decision.status === "source-publication-approved") {
    console.log("Gate legal aprobado para publicar el código fuente; instaladores y updater permanecen bloqueados.");
  } else {
    console.log("Gate técnico legal/distribución aprobado: ficha completa, pendiente de marcar approved tras revisión.");
  }
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  main();
}
