import { spawnSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import { fileURLToPath, pathToFileURL } from "node:url";
import { dirname, join, resolve } from "node:path";

const projectRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const matrixPath = join(projectRoot, "fixtures", "incremental", "paths-v1.json");
const docPath = join(projectRoot, "docs", "reference", "incremental-paths.md");
const allowedPaths = new Set([
  "incremental",
  "snapshot-privado",
  "rechazo-cancelable",
  "fallback-en-memoria",
  "rechazo-antes-de-materializar",
  "incremental-con-salida-materializada",
]);

/**
 * QA-52: how `name` is declared in `source`: "test" only for a `#[test]`
 * function that is not `#[ignore]`d; a plain `fn` or an ignored test does
 * not count as native coverage.
 */
export function nativeTestDeclaration(source, name) {
  const lines = source.split(/\r?\n/);
  const index = lines.findIndex((line) => new RegExp(`^\\s*(?:pub(?:\\([^)]*\\))?\\s+)?(?:async\\s+)?fn\\s+${name}\\s*\\(`).test(line));
  if (index < 0) return "missing";
  const attributes = [];
  for (let previous = index - 1; previous >= 0; previous--) {
    const line = lines[previous].trim();
    if (line.startsWith("#[")) attributes.push(line);
    else if (line !== "" && !line.startsWith("//")) break;
  }
  if (attributes.some((attribute) => attribute.startsWith("#[ignore"))) return "ignored";
  return attributes.some((attribute) => /^#\[(?:\w+::)?test\b/.test(attribute)) ? "test" : "not-a-test";
}

/** QA-52: the full libtest name of `name` in `cargo test -- --list` output, if exactly one matches. */
export function exactTestName(listOutput, name) {
  const matches = listOutput
    .split(/\r?\n/)
    .map((line) => line.match(/^(\S+): test$/)?.[1])
    .filter((full) => full === name || full?.endsWith(`::${name}`));
  return matches.length === 1 ? matches[0] : null;
}

/** QA-52: `cargo test` exits 0 even when its filter selects nothing. */
export function ranExactlyOneTest(output) {
  return /test result: ok\. 1 passed; 0 failed; 0 ignored/.test(output);
}

function cargoTest(args) {
  const result = spawnSync("cargo", ["test", "--manifest-path", "src-tauri/Cargo.toml", "--lib", "--", ...args], {
    cwd: projectRoot,
    env: { ...process.env, COLUMNIA_TEST_HARNESS_MANIFEST: "1" },
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
  });
  if (result.error) throw result.error;
  return result;
}

async function main() {
  const matrix = JSON.parse(await readFile(matrixPath, "utf8"));
  const source = await readFile(join(projectRoot, matrix.sourceTestFile), "utf8");

  if (matrix.schemaVersion !== 1 || !Array.isArray(matrix.cases) || matrix.cases.length === 0) {
    throw new Error("La matriz incremental debe usar schemaVersion 1 y contener casos.");
  }

  const ids = new Set();
  const tests = new Set();
  for (const item of matrix.cases) {
    if (!item.id || ids.has(item.id)) throw new Error(`ID ausente o repetido: ${item.id ?? "(vacío)"}`);
    ids.add(item.id);
    if (!allowedPaths.has(item.expectedPath)) throw new Error(`${item.id}: ruta esperada desconocida.`);
    for (const field of ["format", "operation", "privacy", "quality", "test"]) {
      if (typeof item[field] !== "string" || item[field].trim() === "") {
        throw new Error(`${item.id}: falta el campo ${field}.`);
      }
    }
    for (const test of [item.test, ...(item.additionalTests ?? [])]) {
      const declaration = nativeTestDeclaration(source, test);
      if (declaration === "missing") throw new Error(`${item.id}: no existe la prueba nativa ${test}.`);
      if (declaration !== "test") throw new Error(`${item.id}: ${test} no es una prueba activa (${declaration}).`);
      tests.add(test);
    }
  }

  for (const path of allowedPaths) {
    if (!matrix.cases.some((item) => item.expectedPath === path)) {
      throw new Error(`La matriz no tiene cobertura para la ruta ${path}.`);
    }
  }

  const markdown = [
    "# Matriz de rutas incrementales",
    "",
    "Esta tabla se genera desde `fixtures/incremental/paths-v1.json`. Los casos son una selección de riesgos críticos, no una promesa de que toda operación/formato use el camino incremental.",
    "",
    "| Caso | Formato | Operación | Ruta esperada | Privacidad | Calidad | Prueba nativa |",
    "| --- | --- | --- | --- | --- | --- | --- |",
    ...matrix.cases.map((item) => `| ${item.id} | ${item.format} | ${item.operation} | ${item.expectedPath} | ${item.privacy} | ${item.quality} | ${(item.additionalTests ? [item.test, ...item.additionalTests] : [item.test]).map((test) => `\`${test}\``).join(", ")} |`),
    "",
    "`npm run incremental:check` valida el contrato y ejecuta cada prueba nativa enlazada. Si se amplía una ruta, agrega primero la regresión correspondiente y actualiza esta fuente; los filtros siguen sujetos a los límites de memoria y cancelación reales del motor.",
    "",
  ].join("\n");

  const existingDoc = await readFile(docPath, "utf8").catch(() => null);
  if (existingDoc !== markdown) {
    if (process.argv.includes("--write-doc")) {
      const { writeFile } = await import("node:fs/promises");
      await writeFile(docPath, markdown, "utf8");
    } else {
      throw new Error("La matriz documentada no coincide con su fuente. Ejecuta node tools/check-incremental-matrix.mjs --write-doc.");
    }
  }

  if (!process.argv.includes("--run-native")) {
    console.log(`Matriz incremental válida: ${matrix.cases.length} casos, ${tests.size} regresiones nativas enlazadas.`);
    console.log("Para ejecutar las regresiones enlazadas, usa --run-native.");
    return;
  }

  // QA-52: each linked test runs by its exact name and must report one pass.
  const listing = cargoTest(["--list"]);
  if (listing.status !== 0) throw new Error(`cargo test --list falló: ${listing.stderr.trim()}`);
  for (const test of tests) {
    const fullName = exactTestName(listing.stdout, test);
    if (!fullName) throw new Error(`${test} no aparece una sola vez entre las pruebas compiladas.`);
    console.log(`Ejecutando ${fullName}`);
    const result = cargoTest(["--exact", fullName]);
    process.stdout.write(result.stdout);
    if (result.status !== 0 || !ranExactlyOneTest(result.stdout)) {
      process.stderr.write(result.stderr);
      throw new Error(`${fullName} no se ejecutó como una sola prueba aprobada.`);
    }
  }

  console.log(`Matriz incremental aprobada: ${tests.size} pruebas nativas ejecutadas.`);
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  await main();
}
