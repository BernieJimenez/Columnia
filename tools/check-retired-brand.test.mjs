import assert from "node:assert/strict";
import test from "node:test";

import { retiredBrandFindings } from "./check-retired-brand.mjs";

// Built at run time so this file does not name the retired brand itself.
const retired = ["Data", "Prep"].join(" ");

test("informa la línea real de una marca retirada (COD-20)", () => {
  const contents = ["primera", "segunda", `tercera con ${retired}`].join("\r\n");
  assert.deepEqual(retiredBrandFindings("docs/a.md", contents), ["docs/a.md:3"]);
});

test("no confunde palabras que solo contienen las letras (COD-20)", () => {
  assert.deepEqual(retiredBrandFindings("a.md", "metadata prepared\ndata preparation"), []);
  assert.deepEqual(retiredBrandFindings("bin.dat", `\x00${retired}`), []);
});
