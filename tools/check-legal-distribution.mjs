import { existsSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

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
const placeholderPattern = /^(?:pending|pending[-_ ](?:legal|review)|todo|tbd|n\/a|example)$/i;

function fail(message) {
  console.error(`Gate legal/distribución falló: ${message}`);
  process.exit(1);
}

for (const relativePath of requiredFiles) {
  if (!existsSync(join(projectRoot, relativePath.replaceAll("/", "\\")))) {
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
if (!["pending-legal-review", "approved"].includes(decision.status)) {
  fail("el estado de la ficha debe ser pending-legal-review o approved.");
}
if (!decision.lastReviewed || !/^\d{4}-\d{2}-\d{2}$/.test(decision.lastReviewed)) {
  fail("lastReviewed debe usar una fecha ISO YYYY-MM-DD.");
}
if (!decision.decision || typeof decision.decision !== "object" || Array.isArray(decision.decision)) {
  fail("la ficha debe contener el objeto decision.");
}
for (const field of requiredFields) {
  if (!(field in decision.decision)) fail(`falta el campo decision.${field}.`);
}

const unresolved = requiredFields.filter((field) => {
  const value = decision.decision[field];
  if (Array.isArray(value)) return value.length === 0;
  return typeof value !== "string" || value.trim().length === 0 || placeholderPattern.test(value.trim());
});

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

if (decision.status === "approved" && unresolved.length > 0) {
  fail(`status=approved pero faltan decisiones: ${unresolved.join(", ")}.`);
}
if (unresolved.length > 0) {
  console.log(`Gate técnico legal/distribución aprobado; aprobación jurídica pendiente (${unresolved.length} campos).`);
} else {
  console.log("Gate técnico legal/distribución aprobado: ficha completa, pendiente de marcar approved tras revisión.");
}
