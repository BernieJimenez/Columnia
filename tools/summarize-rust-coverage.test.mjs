import assert from "node:assert/strict";
import test from "node:test";

import { rustCoverageProblems, summarizeRustCoverage, thresholdsFromBaseline } from "./summarize-rust-coverage.mjs";

const file = (filename, count, covered) => ({ filename, summary: { lines: { count, covered } } });

test("agrupa la cobertura por módulo y omite archivos fuera del crate", () => {
  const baseline = summarizeRustCoverage({
    data: [
      {
        files: [
          file("C:\\repo\\src-tauri\\src\\dataset.rs", 0, 0),
          file("C:\\repo\\src-tauri\\src\\dataset\\page_reader.rs", 100, 80),
          file("C:\\repo\\src-tauri\\src\\dataset\\history.rs", 100, 60),
          file("C:\\repo\\src-tauri\\src\\projects.rs", 50, 25),
          file("C:\\cargo\\registry\\polars\\src\\lib.rs", 10, 10),
        ],
        totals: { lines: { percent: 66.6666 } },
      },
    ],
  });
  assert.equal(baseline.totalLinePercent, 66.67);
  assert.deepEqual(
    baseline.modules.map(({ module, files, lines, coveredLines, linePercent }) => [module, files, lines, coveredLines, linePercent]),
    [
      ["dataset", 3, 200, 140, 70],
      ["projects", 1, 50, 25, 50],
    ],
  );
});

test("rechaza un export sin archivos", () => {
  assert.throws(() => summarizeRustCoverage({ data: [] }), /data\[0\]\.files/);
});

test("solo agrupa directorios que son módulos y distingue un módulo sin líneas (COD-19)", () => {
  const baseline = summarizeRustCoverage({
    data: [
      {
        files: [
          file("/repo/src-tauri/src/dataset.rs", 10, 5),
          file("/repo/src-tauri/src/dataset/history.rs", 10, 5),
          file("/repo/src-tauri/src/bin/columnia_cli.rs", 20, 0),
          file("/repo/src-tauri/src/empty.rs", 0, 0),
        ],
      },
    ],
  });
  assert.deepEqual(
    baseline.modules.map(({ module, files, linePercent }) => [module, files, linePercent]),
    [
      ["bin/columnia_cli", 1, 0],
      ["dataset", 2, 50],
      ["empty", 1, null],
    ],
  );
});

test("un módulo por debajo de su umbral o sin umbral hace fallar la cobertura (QA-60)", () => {
  const baseline = {
    modules: [
      { module: "dataset", linePercent: 72.19 },
      { module: "privacy", linePercent: 96.62 },
      { module: "nuevo", linePercent: 40 },
      { module: "vacio", linePercent: null },
    ],
  };
  const thresholds = { modules: { dataset: 71, privacy: 97, vacio: 0 } };
  const problems = rustCoverageProblems(baseline, thresholds);
  assert.equal(problems.length, 2);
  assert.match(problems[0], /privacy.*96\.62.*97/);
  assert.match(problems[1], /nuevo.*no tiene umbral/);
  assert.deepEqual(rustCoverageProblems({ modules: [{ module: "dataset", linePercent: 72.19 }] }, thresholds), []);
});

test("los umbrales iniciales dejan un punto de margen bajo la medida (QA-60)", () => {
  const thresholds = thresholdsFromBaseline({ modules: [{ module: "dataset", linePercent: 72.19 }, { module: "lib", linePercent: 18.09 }, { module: "vacio", linePercent: null }] });
  assert.deepEqual(thresholds.modules, { dataset: 71, lib: 17, vacio: 0 });
});
