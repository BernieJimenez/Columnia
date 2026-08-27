import { describe, expect, it } from "vitest";

import {
  MAX_RECENT_DATASETS,
  RECENT_DATASETS_STORAGE_KEY,
  formatRecentDatasetFormat,
  normalizeRecentFileName,
  readRecentDatasets,
  rememberRecentDataset,
  removeRecentDataset,
  writeRecentDatasets,
  type RecentDataset,
} from "./recentFilesModel";

function memoryStorage(): Storage {
  const values = new Map<string, string>();
  return {
    getItem: (key) => values.get(key) ?? null,
    setItem: (key, value) => values.set(key, value),
    removeItem: (key) => values.delete(key),
    clear: () => values.clear(),
    key: (index) => [...values.keys()][index] ?? null,
    get length() { return values.size; },
  };
}

describe("recentFilesModel", () => {
  it("guarda solo el nombre visible y descarta cualquier ruta al persistir", () => {
    const storage = memoryStorage();
    const recent = rememberRecentDataset([], {
      fileName: "C:\\Users\\Ana\\Documents\\ventas.csv",
      format: "csv",
    }, 123);

    writeRecentDatasets(recent, storage);

    expect(normalizeRecentFileName("/home/ana/ventas.csv")).toBe("ventas.csv");
    expect(storage.getItem(RECENT_DATASETS_STORAGE_KEY)).not.toContain("Users");
    expect(readRecentDatasets(storage)).toMatchObject([
      { fileName: "ventas.csv", format: "csv", lastOpenedAt: 123 },
    ]);
  });

  it("actualiza un archivo repetido, mantiene el id opaco y limita el historial", () => {
    let recent: RecentDataset[] = [];
    for (let index = 0; index < MAX_RECENT_DATASETS + 2; index += 1) {
      recent = rememberRecentDataset(recent, { fileName: `archivo-${index}.csv`, format: "csv" }, index + 1);
    }
    const originalId = recent.find((item) => item.fileName === "archivo-6.csv")?.id;
    const updated = rememberRecentDataset(recent, { fileName: "archivo-6.csv", format: "csv" }, 99);

    expect(updated).toHaveLength(MAX_RECENT_DATASETS);
    expect(updated[0]).toMatchObject({ fileName: "archivo-6.csv", lastOpenedAt: 99, id: originalId });
    expect(updated.some((item) => item.fileName === "archivo-0.csv")).toBe(false);
  });

  it("ignora registros corruptos y permite quitar un elemento", () => {
    const storage = memoryStorage();
    storage.setItem(RECENT_DATASETS_STORAGE_KEY, JSON.stringify([
      { id: "ok-1", fileName: "datos.json", format: "json", lastOpenedAt: 20 },
      { id: "bad", fileName: "C:\\secret\\datos.csv", format: "csv", lastOpenedAt: "ayer" },
      { id: "C:\\secret\\selection", fileName: "datos.csv", format: "csv", lastOpenedAt: 10 },
    ]));

    const loaded = readRecentDatasets(storage);
    expect(loaded).toHaveLength(1);
    expect(removeRecentDataset(loaded, "ok-1")).toEqual([]);
    expect(formatRecentDatasetFormat("excel")).toBe("Excel");
  });
});
