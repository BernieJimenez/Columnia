import assert from "node:assert/strict";
import test from "node:test";

import { exactTestName, nativeTestDeclaration, ranExactlyOneTest } from "./check-incremental-matrix.mjs";

test("solo cuenta como cobertura una prueba activa con #[test] (QA-52)", () => {
  const source = [
    "#[test]",
    "fn active_case() {}",
    "",
    "#[test]",
    "#[ignore = \"lento\"]",
    "fn ignored_case() {}",
    "",
    "fn helper_case() {}",
    "",
    "#[tokio::test]",
    "// explicación",
    "async fn async_case() {}",
  ].join("\n");
  assert.equal(nativeTestDeclaration(source, "active_case"), "test");
  assert.equal(nativeTestDeclaration(source, "ignored_case"), "ignored");
  assert.equal(nativeTestDeclaration(source, "helper_case"), "not-a-test");
  assert.equal(nativeTestDeclaration(source, "async_case"), "test");
  assert.equal(nativeTestDeclaration(source, "renamed_case"), "missing");
});

test("el nombre exacto exige una sola coincidencia completa (QA-52)", () => {
  const listing = [
    "dataset::tests::filters_on_disk: test",
    "dataset::tests::filters_on_disk_twice: test",
    "projects::tests::filters_on_disk: benchmark",
    "",
    "3 tests, 0 benchmarks",
  ].join("\n");
  assert.equal(exactTestName(listing, "filters_on_disk"), "dataset::tests::filters_on_disk");
  assert.equal(exactTestName(listing, "on_disk"), null);
  assert.equal(exactTestName(`${listing}\nother::filters_on_disk: test`, "filters_on_disk"), null);
});

test("cargo test con 0 pruebas no cuenta como aprobado (QA-52)", () => {
  assert.equal(ranExactlyOneTest("running 0 tests\n\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 900 filtered out"), false);
  assert.equal(ranExactlyOneTest("running 1 test\ntest a ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 899 filtered out"), true);
});
