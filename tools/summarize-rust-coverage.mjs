// Summarizes a `cargo llvm-cov --json --summary-only` export into a per-module
// line-coverage baseline. It records the baseline; it does not enforce limits
// until the Rust coverage has a history to compare against.
import { readFile, writeFile } from "node:fs/promises";
import { pathToFileURL } from "node:url";

/** Groups file summaries by module: `dataset/*.rs` under `dataset`, others by file stem. */
export function summarizeRustCoverage(exportDocument) {
  const files = exportDocument?.data?.[0]?.files;
  if (!Array.isArray(files)) throw new Error("el export de llvm-cov no contiene data[0].files");
  const modules = new Map();
  for (const file of files) {
    const normalized = String(file.filename).replaceAll("\\", "/");
    const marker = normalized.lastIndexOf("/src-tauri/src/");
    if (marker < 0) continue;
    const relative = normalized.slice(marker + "/src-tauri/src/".length);
    const [first, ...rest] = relative.split("/");
    const module = rest.length ? first : first.replace(/\.rs$/, "");
    const lines = file.summary?.lines ?? { count: 0, covered: 0 };
    const entry = modules.get(module) ?? { module, files: 0, lines: 0, coveredLines: 0 };
    entry.files += 1;
    entry.lines += lines.count;
    entry.coveredLines += lines.covered;
    modules.set(module, entry);
  }
  const rows = [...modules.values()]
    .map((entry) => ({
      ...entry,
      linePercent: entry.lines ? Math.round((entry.coveredLines / entry.lines) * 10000) / 100 : 0,
    }))
    .sort((left, right) => left.module.localeCompare(right.module));
  const totals = exportDocument.data[0].totals?.lines ?? {};
  return {
    contract: "columnia-rust-coverage-baseline",
    schemaVersion: 1,
    totalLinePercent: typeof totals.percent === "number" ? Math.round(totals.percent * 100) / 100 : null,
    modules: rows,
  };
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  const [input, output] = process.argv.slice(2);
  if (!input || !output) {
    console.error("Uso: node tools/summarize-rust-coverage.mjs <llvm-cov.json> <baseline.json>");
    process.exit(2);
  }
  try {
    const baseline = summarizeRustCoverage(JSON.parse(await readFile(input, "utf8")));
    await writeFile(output, `${JSON.stringify(baseline, null, 2)}\n`);
    console.log(`Cobertura Rust: ${baseline.totalLinePercent}% de líneas en ${baseline.modules.length} módulos.`);
    for (const row of baseline.modules) {
      console.log(`  ${row.module.padEnd(28)} ${String(row.linePercent).padStart(6)}%  (${row.coveredLines}/${row.lines})`);
    }
  } catch (error) {
    console.error(`No se pudo resumir la cobertura Rust: ${error instanceof Error ? error.message : String(error)}`);
    process.exitCode = 1;
  }
}
