import { spawnSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { dirname, join, resolve } from "node:path";

const projectRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const matrixPath = join(projectRoot, "fixtures", "incremental", "paths-v1.json");
const docPath = join(projectRoot, "docs", "reference", "incremental-paths.md");
const matrix = JSON.parse(await readFile(matrixPath, "utf8"));
const sourcePath = join(projectRoot, matrix.sourceTestFile);
const source = await readFile(sourcePath, "utf8");
const allowedPaths = new Set([
  "incremental",
  "snapshot-privado",
  "rechazo-cancelable",
  "fallback-en-memoria",
  "rechazo-antes-de-materializar",
  "incremental-con-salida-materializada",
]);

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
    const declaration = new RegExp(`^\\s*fn\\s+${test}\\s*\\(`, "m");
    if (!declaration.test(source)) throw new Error(`${item.id}: no existe la prueba nativa ${test}.`);
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
  process.exit(0);
}

for (const test of tests) {
  console.log(`Ejecutando ${test}`);
  const result = spawnSync("cargo", ["test", "--manifest-path", "src-tauri/Cargo.toml", "--lib", test], {
    cwd: projectRoot,
    env: { ...process.env, COLUMNIA_TEST_HARNESS_MANIFEST: "1" },
    stdio: "inherit",
  });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
}

console.log(`Matriz incremental aprobada: ${tests.size} pruebas nativas ejecutadas.`);
