import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const projectRoot = process.cwd();
const retiredPatterns = [
  ["data", "prep"],
  ["data", "prev"],
].map((parts) => new RegExp(parts.join("\\s*"), "i"));

const trackedAndUnignoredFiles = execFileSync(
  "git",
  ["ls-files", "--cached", "--others", "--exclude-standard", "-z"],
  { cwd: projectRoot },
)
  .toString("utf8")
  .split("\x00")
  .filter(Boolean);

const findings = [];
for (const relativeFile of trackedAndUnignoredFiles) {
  const absoluteFile = path.join(projectRoot, relativeFile);
  let contents;
  try {
    contents = fs.readFileSync(absoluteFile, "utf8");
  } catch {
    continue;
  }
  if (contents.includes("\x00")) continue;
  contents.split(/\\r?\\n/).forEach((line, index) => {
    if (retiredPatterns.some((pattern) => pattern.test(line))) {
      findings.push(`${relativeFile}:${index + 1}`);
    }
  });
}

if (findings.length > 0) {
  console.error("El árbol activo contiene referencias de marca retiradas:");
  findings.forEach((finding) => console.error(`- ${finding}`));
  process.exitCode = 1;
} else {
  console.log(`Verificación de marca retirada aprobada: ${trackedAndUnignoredFiles.length} archivos activos.`);
}
