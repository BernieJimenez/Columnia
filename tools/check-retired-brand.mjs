import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { pathToFileURL } from "node:url";

// COD-20: the retired names as whole words, so «metadata prepared» is not one.
const retiredPatterns = [
  ["data", "prep"],
  ["data", "prev"],
].map((parts) => new RegExp(String.raw`\b${parts.join(String.raw`\s*`)}\b`, "i"));

/** `file:line` for each line of `contents` that names a retired brand. */
export function retiredBrandFindings(relativeFile, contents) {
  if (contents.includes("\x00")) return [];
  // COD-20: real line breaks; `/\\r?\\n/` split on the text «\r\n» and
  // reported every finding as line 1.
  return contents.split(/\r?\n/).flatMap((line, index) =>
    retiredPatterns.some((pattern) => pattern.test(line)) ? [`${relativeFile}:${index + 1}`] : []);
}

function trackedAndUnignoredFiles(projectRoot) {
  return execFileSync("git", ["ls-files", "--cached", "--others", "--exclude-standard", "-z"], { cwd: projectRoot })
    .toString("utf8")
    .split("\x00")
    .filter(Boolean);
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  const projectRoot = process.cwd();
  const files = trackedAndUnignoredFiles(projectRoot);
  const findings = files.flatMap((relativeFile) => {
    try {
      return retiredBrandFindings(relativeFile, fs.readFileSync(path.join(projectRoot, relativeFile), "utf8"));
    } catch {
      return [];
    }
  });
  if (findings.length > 0) {
    console.error("El árbol activo contiene referencias de marca retiradas:");
    findings.forEach((finding) => console.error(`- ${finding}`));
    process.exitCode = 1;
  } else {
    console.log(`Verificación de marca retirada aprobada: ${files.length} archivos activos.`);
  }
}
