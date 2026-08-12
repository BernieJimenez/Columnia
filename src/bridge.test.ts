import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  getAppInfo,
  getDatasetPage,
  getDatasetProfile,
  pickAndLoadCsv,
  removeDuplicates,
  undoLastChange,
} from "./bridge";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

describe("desktop bridge", () => {
  beforeEach(() => vi.mocked(invoke).mockReset());

  it("invoca el comando Rust con un contrato estrecho", async () => {
    vi.mocked(invoke).mockResolvedValue({
      name: "Columnia",
      version: "0.1.0",
      platform: "windows",
    });

    await expect(getAppInfo()).resolves.toEqual({
      name: "Columnia",
      version: "0.1.0",
      platform: "windows",
    });
    expect(invoke).toHaveBeenCalledWith("get_app_info");
  });

  it("solicita la selección nativa sin entregar una ruta desde React", async () => {
    vi.mocked(invoke).mockResolvedValue(null);

    await expect(pickAndLoadCsv()).resolves.toBeNull();

    expect(invoke).toHaveBeenCalledWith("pick_and_load_csv");
  });

  it("solicita una página por posición sin volver a entregar la ruta", async () => {
    vi.mocked(invoke).mockResolvedValue({ offset: 50, rows: [["Santiago"]] });

    await expect(getDatasetPage(50, 50)).resolves.toEqual({
      offset: 50,
      rows: [["Santiago"]],
    });

    expect(invoke).toHaveBeenCalledWith("get_dataset_page", { offset: 50, limit: 50 });
  });

  it("solicita el perfil del dataset activo sin argumentos", async () => {
    vi.mocked(invoke).mockResolvedValue({
      rowCount: 0,
      duplicateRowCount: 0,
      duplicatePercentage: 0,
      columns: [],
    });

    await expect(getDatasetProfile()).resolves.toEqual({
      rowCount: 0,
      duplicateRowCount: 0,
      duplicatePercentage: 0,
      columns: [],
    });

    expect(invoke).toHaveBeenCalledWith("get_dataset_profile");
  });

  it("aplica y deshace transformaciones mediante comandos sin argumentos", async () => {
    vi.mocked(invoke).mockResolvedValue({});

    await removeDuplicates();
    await undoLastChange();

    expect(invoke).toHaveBeenNthCalledWith(1, "remove_duplicates");
    expect(invoke).toHaveBeenNthCalledWith(2, "undo_last_change");
  });
});
