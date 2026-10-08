import assert from "node:assert/strict";
import test from "node:test";

import { commandParityProblems, expectedInventory, handlerEntries, inventoryDrift, invokedCommands } from "./check-ipc-inventory.mjs";

test("detecta un invoke huérfano en cada sentido (QA-25)", () => {
  const invoked = invokedCommands([
    'await invoke("load_dataset", { path });',
    "await invoke<DatasetPreview>('get_dataset_page', { offset });",
  ]);
  assert.deepEqual([...invoked].sort(), ["get_dataset_page", "load_dataset"]);
  assert.deepEqual(commandParityProblems(["load_dataset", "get_dataset_page"], invoked), []);
  assert.match(commandParityProblems(["load_dataset", "get_dataset_page", "export_dataset"], invoked)[0], /export_dataset está registrado en Rust/);
  assert.match(commandParityProblems(["load_dataset"], invoked)[0], /TypeScript invoca get_dataset_page/);
});

test("ignora comentarios dentro de generate_handler", () => {
  const entries = handlerEntries(`tauri::generate_handler![
    // dataset::retired_command, ya no existe
    dataset::load_dataset,
    #[cfg(debug_assertions)]
    probe::probe_seed_dataset,
  ]`);
  assert.deepEqual(entries.map(({ name }) => name), ["load_dataset", "probe_seed_dataset"]);
});

test("un comando nuevo sin inventariar hace fallar el gate y el inventario al día pasa (QA-62)", () => {
  const handler = `tauri::generate_handler![
    dataset::load_dataset,
    dataset::get_dataset_page,
  ]`;
  const inventory = expectedInventory(handler);
  assert.deepEqual(inventoryDrift(inventory, expectedInventory(handler)), []);
  const withNewCommand = handler.replace("dataset::get_dataset_page,", "dataset::get_dataset_page,\n    dataset::export_dataset,");
  assert.match(inventoryDrift(inventory, expectedInventory(withNewCommand))[0], /no coincide con generate_handler/);
});
