import assert from "node:assert/strict";
import test from "node:test";

import { summarizeRustCoverage } from "./summarize-rust-coverage.mjs";

const file = (filename, count, covered) => ({ filename, summary: { lines: { count, covered } } });

test("agrupa la cobertura por módulo y omite archivos fuera del crate", () => {
  const baseline = summarizeRustCoverage({
    data: [
      {
        files: [
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
      ["dataset", 2, 200, 140, 70],
      ["projects", 1, 50, 25, 50],
    ],
  );
});

test("rechaza un export sin archivos", () => {
  assert.throws(() => summarizeRustCoverage({ data: [] }), /data\[0\]\.files/);
});
