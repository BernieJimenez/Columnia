// LIM-03: exports in src that no other file uses (tests count as users), the
// same rule as knip's «Unused exports», without downloading knip.
import { readdirSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const projectRoot = resolve(fileURLToPath(new URL("..", import.meta.url)));

function sourceFiles(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) return sourceFiles(path);
    return /\.(ts|tsx)$/.test(entry.name) ? [path] : [];
  });
}

/** `file: name` for each export no other file mentions. */
export function unusedExports(files) {
  const unused = [];
  for (const [file, source] of files) {
    if (/\.test\.|[\\/]test[\\/]/.test(file)) continue;
    for (const match of source.matchAll(/^export (?:async )?(?:function|const|let|type|interface|class|enum) ([A-Za-z0-9_]+)/gm)) {
      const pattern = new RegExp(String.raw`\b${match[1]}\b`);
      if (![...files].some(([other, contents]) => other !== file && pattern.test(contents))) unused.push(`${file}: ${match[1]}`);
    }
  }
  return unused;
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  const files = new Map(sourceFiles(join(projectRoot, "src")).map((file) => [file.slice(projectRoot.length + 1), readFileSync(file, "utf8")]));
  const unused = unusedExports(files);
  if (unused.length > 0) {
    console.error(`Exportaciones sin uso (quita export o bórralas):\n${unused.join("\n")}`);
    process.exitCode = 1;
  } else {
    console.log("Sin exportaciones sin uso en src.");
  }
}
