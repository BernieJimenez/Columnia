import assert from "node:assert/strict";
import test from "node:test";

import { readPeSubsystem } from "./check-pe-subsystem.mjs";

function syntheticPe(subsystem, magic = 0x20b) {
  const bytes = Buffer.alloc(0x200);
  bytes.write("MZ", 0, "latin1");
  bytes.writeUInt32LE(0x80, 0x3c);
  bytes.writeUInt32LE(0x00004550, 0x80);
  bytes.writeUInt16LE(magic, 0x80 + 24);
  bytes.writeUInt16LE(subsystem, 0x80 + 24 + 68);
  return bytes;
}

test("lee el subsistema gráfico de PE32+ y PE32", () => {
  assert.equal(readPeSubsystem(syntheticPe(2)), 2);
  assert.equal(readPeSubsystem(syntheticPe(2, 0x10b)), 2);
});

test("distingue el subsistema de consola", () => {
  assert.equal(readPeSubsystem(syntheticPe(3)), 3);
});

test("rechaza archivos que no son PE", () => {
  assert.throws(() => readPeSubsystem(Buffer.from("no es un ejecutable")), /MZ/);
});
