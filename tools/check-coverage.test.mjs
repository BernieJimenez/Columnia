import assert from "node:assert/strict";
import test from "node:test";

import { coverageProblems } from "./check-coverage.mjs";

const metrics = (pct) => ({ statements: { pct }, lines: { pct }, branches: { pct }, functions: { pct } });
const summary = {
  total: metrics(99),
  ...Object.fromEntries([
    "App.tsx", "features/delivery/DeliveryPhase.tsx", "features/prepare/PreparePhase.tsx",
    "features/prepare/usePrepareController.ts", "features/review/useReviewController.ts",
    "features/delivery/useDeliveryController.ts", "features/projects/useProjectsController.ts",
    "features/delivery/deliveryModel.ts", "features/review/ReviewPhase.tsx", "features/load/LoadPhase.tsx",
    "features/explore/ExplorePhase.tsx", "features/review/DatasetComparisonSection.tsx",
  ].map((file) => [`C:\\repo\\src\\${file.replaceAll("/", "\\")}`, metrics(99)])),
};

test("aprueba un resumen actual por encima de los umbrales", () => {
  assert.deepEqual(coverageProblems(summary, 2_000, 1_000), []);
});

test("falla si el resumen es anterior a los fuentes (QA-26)", () => {
  assert.match(coverageProblems(summary, 1_000, 2_000)[0], /anterior a los fuentes/);
});

test("falla por debajo del umbral global o de un archivo", () => {
  assert.ok(coverageProblems({ ...summary, total: metrics(10) }, 2_000, 1_000).some((failure) => failure.startsWith("total")));
  const low = { ...summary, "C:\\repo\\src\\features\\load\\LoadPhase.tsx": metrics(50) };
  assert.ok(coverageProblems(low, 2_000, 1_000).some((failure) => failure.includes("LoadPhase")));
});
