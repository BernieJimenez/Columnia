import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const source = readFileSync(new URL("./capture-accessibility-evidence.mjs", import.meta.url), "utf8");

test("el servidor de vista previa no cambia de puerto en silencio (COD-19)", () => {
  assert.match(source, /"--port",\s*String\(port\),[\s\S]*?"--strictPort",/);
});
