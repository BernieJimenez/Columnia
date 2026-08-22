import { expect, test, type Page } from "@playwright/test";

async function installTauriProjectMock(page: Page) {
  await page.addInitScript(() => {
    const dataset = {
      fileName: "ventas.csv",
      fileSizeBytes: 128,
      rowCount: 2,
      columnCount: 2,
      columns: [
        { name: "cliente", dataType: "String" },
        { name: "total", dataType: "Float64" },
      ],
      rows: [["Ana", "10"], ["Luis", "20"]],
    };
    const project = {
      id: "project-e2e",
      name: "Ventas E2E",
      datasetFileName: dataset.fileName,
      rowCount: dataset.rowCount,
      columnCount: dataset.columnCount,
      createdAt: "2026-01-01T00:00:00.000Z",
      updatedAt: "2026-01-01T00:00:00.000Z",
    };
    let projects: typeof project[] = [];
    let callbackId = 0;

    const invoke = async (command: string, args: Record<string, unknown> = {}) => {
      switch (command) {
        case "get_app_info":
          return { name: "Columnia", version: "0.33.0", platform: "windows" };
        case "list_projects":
          return projects;
        case "get_recovery_candidate":
          return null;
        case "pick_dataset_source":
          return {
            selectionId: "selection-e2e",
            fileName: dataset.fileName,
            fileSizeBytes: dataset.fileSizeBytes,
            format: "csv",
            sheets: [],
            defaultSheetId: null,
            isCompressedContainer: false,
          };
        case "load_dataset_selection":
          return dataset;
        case "get_history_state":
          return { canUndo: false, canRedo: false, currentIndex: 0, entryCount: 0, entries: [], snapshotsEnabled: true };
        case "get_dataset_page":
          return { offset: args.offset ?? 0, rows: dataset.rows };
        case "save_project":
          projects = [project];
          return project;
        case "open_project":
          return { project, dataset, workspace: { qualityRules: [], recipeDraft: null }, profile: null };
        case "delete_project":
          projects = [];
          return null;
        default:
          throw new Error(`Comando Tauri no simulado: ${command}`);
      }
    };

    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      configurable: true,
      value: {
        invoke,
        transformCallback: (callback: unknown) => {
          callbackId += 1;
          return callbackId;
        },
        unregisterCallback: () => undefined,
      },
    });
  });
}

test("recorre guardar, abrir y eliminar un proyecto desde el shell Tauri simulado", async ({ page }) => {
  await installTauriProjectMock(page);
  await page.goto("/", { waitUntil: "commit" });

  await expect(page.getByRole("button", { name: "Seleccionar dataset" })).toBeVisible();
  await page.getByRole("button", { name: "Seleccionar dataset" }).click();
  await expect(page.getByRole("button", { name: "Revisar" })).toHaveAttribute("aria-current", "step");

  await page.getByRole("button", { name: "Cargar" }).click();
  await expect(page.getByRole("heading", { name: "Proyectos" })).toBeVisible();
  await page.getByLabel("Nombre del proyecto").fill("Ventas E2E");
  await page.getByRole("button", { name: "Guardar proyecto nuevo" }).click();
  await expect(page.locator("p.notice--success")).toContainText("Proyecto “Ventas E2E” guardado.");
  await expect(page.getByRole("list", { name: "Proyectos guardados" })).toContainText("Ventas E2E");

  await page.getByRole("button", { name: "Abrir" }).click();
  await expect(page.getByRole("button", { name: "Revisar" })).toHaveAttribute("aria-current", "step");
  await expect(page.getByRole("heading", { name: "ventas.csv" })).toBeVisible();

  await page.getByRole("button", { name: "Cargar" }).click();
  await expect(page.getByRole("list", { name: "Proyectos guardados" })).toContainText("Ventas E2E · activo");
  await page.getByRole("button", { name: "Eliminar" }).click();
  await expect(page.getByRole("alertdialog", { name: "Eliminar “Ventas E2E”" })).toBeVisible();
  await page.getByRole("button", { name: "Eliminar proyecto" }).click();
  await expect(page.locator("p.notice--success")).toContainText("Proyecto “Ventas E2E” eliminado.");
  await expect(page.getByText("Todavía no hay proyectos guardados.")).toBeVisible();
});
