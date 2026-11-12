import assert from "node:assert/strict";
import test from "node:test";

import { summarizeVitestFailures } from "./summarize-vitest-failures.mjs";

test("nombra la prueba fallida con su archivo y la primera línea del error (AUD-082)", () => {
  const report = {
    testResults: [
      {
        name: "C:\\repo\\src\\App.test.tsx",
        status: "failed",
        assertionResults: [
          { status: "passed", fullName: "App abre", failureMessages: [] },
          {
            status: "failed",
            fullName: "App muestra el error al rechazar la cancelación",
            failureMessages: ["\u001b[31mAssertionError: no se encontró el aviso\u001b[39m\n    at foo.tsx:1"],
          },
        ],
      },
      { name: "C:\\repo\\src\\ok.test.ts", status: "passed", assertionResults: [{ status: "passed", fullName: "ok" }] },
    ],
  };
  const failures = summarizeVitestFailures(report, { root: "C:\\repo" });
  assert.equal(failures.length, 1);
  assert.equal(failures[0].test, "App muestra el error al rechazar la cancelación");
  assert.equal(failures[0].message, "AssertionError: no se encontró el aviso | at foo.tsx:1");
  assert.match(failures[0].file, /App\.test\.tsx$/);
});

test("una suite que no carga se informa sin nombre de prueba, y sin fallos devuelve vacío", () => {
  const crashed = { testResults: [{ name: "src/x.test.ts", status: "failed", message: "Cannot find module", assertionResults: [] }] };
  assert.deepEqual(summarizeVitestFailures(crashed), [{ file: "src/x.test.ts", test: null, message: "Cannot find module" }]);
  assert.deepEqual(summarizeVitestFailures({ testResults: [] }), []);
  assert.deepEqual(summarizeVitestFailures(null), []);
});
