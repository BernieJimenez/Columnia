import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";

function parseFixture(contents) {
  const [headerLine, ...dataLines] = contents.trimEnd().split(/\r?\n/);
  const headers = headerLine.split(",");
  return dataLines.map((line) => Object.fromEntries(
    line.split(",").map((value, index) => [headers[index], value]),
  ));
}

async function readFixture(name) {
  const contents = await readFile(new URL(`../fixtures/beta/${name}`, import.meta.url), "utf8");
  return parseFixture(contents);
}

test("el caso sintético de ventas conserva códigos y produce el agregado esperado", async () => {
  const rows = await readFixture("monthly-sales.csv");
  const confirmed = rows.filter((row) => row.status === "confirmed");
  const revenueCents = confirmed.reduce(
    (total, row) => total + Number(row.units) * Math.round(Number(row.unit_price) * 100),
    0,
  );

  assert.equal(confirmed.length, 3);
  assert.deepEqual(confirmed.map((row) => row.customer_code), ["00124", "00007", "00042"]);
  assert.equal(revenueCents, 1560);
});

test("el caso sintético de inventario distingue iguales, cambiados, nuevos y retirados", async () => {
  const before = await readFixture("inventory-before.csv");
  const after = await readFixture("inventory-after.csv");
  const beforeBySku = new Map(before.map((row) => [row.sku, row]));
  const afterBySku = new Map(after.map((row) => [row.sku, row]));
  const same = [];
  const changed = [];
  const added = [];

  for (const current of after) {
    const previous = beforeBySku.get(current.sku);
    if (!previous) added.push(current.sku);
    else if (previous.product !== current.product || previous.stock !== current.stock) changed.push(current.sku);
    else same.push(current.sku);
  }
  const removed = before.filter((row) => !afterBySku.has(row.sku)).map((row) => row.sku);

  assert.deepEqual(same, ["B-02"]);
  assert.deepEqual(changed, ["A-01"]);
  assert.deepEqual(added, ["D-04"]);
  assert.deepEqual(removed, ["C-03"]);
});
