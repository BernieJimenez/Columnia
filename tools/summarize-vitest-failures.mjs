// AUD-082: resumes the JSON report of Vitest (--reporter=json) into the failed
// tests, so the validation report names which test failed instead of only
// "Frontend tests falló con código 1". Usage:
//   node tools/summarize-vitest-failures.mjs <vitest-report.json>
// Prints a JSON array: [{ file, test, message }]. Empty array when nothing failed.
import { readFileSync } from "node:fs";
import { relative, isAbsolute } from "node:path";
import { fileURLToPath } from "node:url";

const MAX_FAILURES = 20;
const MAX_MESSAGE_CHARS = 400;
const ANSI_PATTERN = new RegExp(`${String.fromCharCode(27)}\\[[0-9;]*m`, "g");

function firstLines(text, maxChars) {
  const clean = String(text ?? "")
    .replace(ANSI_PATTERN, "")
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean)
    .slice(0, 2)
    .join(" | ");
  return clean.length > maxChars ? `${clean.slice(0, maxChars - 1)}…` : clean;
}

function shortFile(name, root) {
  if (!name) return null;
  const path = isAbsolute(name) && root ? relative(root, name) : name;
  return path.replaceAll("\\", "/");
}

export function summarizeVitestFailures(report, { root = null } = {}) {
  const failures = [];
  for (const suite of report?.testResults ?? []) {
    const file = shortFile(suite.name, root);
    const failedAssertions = (suite.assertionResults ?? []).filter((item) => item.status === "failed");
    for (const item of failedAssertions) {
      failures.push({
        file,
        test: item.fullName ?? ([...(item.ancestorTitles ?? []), item.title].filter(Boolean).join(" > ") || null),
        message: firstLines((item.failureMessages ?? [])[0], MAX_MESSAGE_CHARS),
      });
    }
    // A suite that fails to load or crashes has no failed assertion.
    if (suite.status === "failed" && failedAssertions.length === 0) {
      failures.push({ file, test: null, message: firstLines(suite.message, MAX_MESSAGE_CHARS) });
    }
  }
  return failures.slice(0, MAX_FAILURES);
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  const reportPath = process.argv[2];
  let failures = [];
  try {
    failures = summarizeVitestFailures(JSON.parse(readFileSync(reportPath, "utf8")), { root: process.cwd() });
  } catch {
    process.exitCode = 2;
  }
  process.stdout.write(JSON.stringify(failures));
}
