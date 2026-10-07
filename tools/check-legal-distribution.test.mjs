import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import { unresolvedDecisionFields, validReviewDate } from "./check-legal-distribution.mjs";

const decision = () => JSON.parse(readFileSync(new URL("../docs/reference/legal-distribution-decision.json", import.meta.url), "utf8")).decision;

test("«ninguno» y [\"pending\"] cuentan como decisiones sin resolver (LEG-02)", () => {
  assert.deepEqual(unresolvedDecisionFields(decision()), []);
  assert.deepEqual(unresolvedDecisionFields({ ...decision(), contact: "ninguno" }), ["contact"]);
  assert.deepEqual(unresolvedDecisionFields({ ...decision(), markets: ["pending"] }), ["markets"]);
  assert.deepEqual(unresolvedDecisionFields({ ...decision(), channel: "-", jurisdiction: "por definir" }), ["jurisdiction", "channel"]);
});

test("lastReviewed debe ser una fecha real y no futura (LEG-02)", () => {
  const today = new Date("2026-10-07T12:00:00Z");
  assert.equal(validReviewDate("2026-09-09", today), true);
  assert.equal(validReviewDate("2026-99-99", today), false);
  assert.equal(validReviewDate("2026-02-30", today), false);
  assert.equal(validReviewDate("2027-01-01", today), false);
});
