import assert from "node:assert/strict";
import test from "node:test";

import { satisfiesRange } from "./check-toolchains.mjs";

test("un rango engines acepta parches y rechaza fuera del rango (OPS-08)", () => {
  assert.equal(satisfiesRange("v24.14.1", ">=24.14.0 <25"), true);
  assert.equal(satisfiesRange("v24.15.0", ">=24.14.0 <25"), true);
  assert.equal(satisfiesRange("v25.0.0", ">=24.14.0 <25"), false);
  assert.equal(satisfiesRange("v24.13.9", ">=24.14.0 <25"), false);
});

test("una versión sin operador es exacta", () => {
  assert.equal(satisfiesRange("11.6.2", "11.6.2"), true);
  assert.equal(satisfiesRange("11.6.3", "11.6.2"), false);
});
