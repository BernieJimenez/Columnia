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
      id: "project-a11y-e2e",
      name: "Ventas accesible",
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
          return { name: "Columnia", version: "0.38.0", platform: "windows" };
        case "list_projects":
          return projects;
        case "get_recovery_candidate":
          return null;
        case "pick_dataset_source":
          return {
            selectionId: "selection-a11y-e2e",
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
        transformCallback: () => {
          callbackId += 1;
          return callbackId;
        },
        unregisterCallback: () => undefined,
      },
    });
  });
}

test.describe("contratos de accesibilidad del shell", () => {
  test("mantiene landmarks, nombres y estados ARIA coherentes", async ({ page }) => {
    await page.goto("/", { waitUntil: "commit" });

    await expect(page.getByRole("complementary", { name: "Navegación principal" })).toHaveCount(1);
    await expect(page.getByRole("navigation", { name: "Flujo de preparación de datos" })).toHaveCount(1);
    await expect(page.locator("main#main-content")).toHaveAttribute("tabindex", "-1");

    const workspace = page.getByRole("region", { name: "Etapa Cargar" });
    await expect(workspace).toHaveAttribute("aria-busy", "false");
    const runtime = page.locator(".runtime");
    await expect(runtime).toHaveAttribute("role", "status");
    await expect(runtime).toHaveAttribute("aria-live", "polite");
    await expect(runtime).toHaveAttribute("aria-atomic", "true");

    const controls = page.getByRole("button");
    const names = await controls.evaluateAll((elements) => elements.map((element) => ({
      ariaLabel: element.getAttribute("aria-label"),
      text: element.textContent?.replace(/\s+/g, " ").trim(),
    })));
    expect(names).not.toContainEqual(expect.objectContaining({ ariaLabel: null, text: "" }));
  });

  test("expone foco visible y targets táctiles mínimos en el recorrido inicial", async ({ page }) => {
    await page.goto("/", { waitUntil: "commit" });
    await expect(page.locator("#app-title")).toBeVisible();

    await page.keyboard.press("Tab");
    const skipLink = page.getByRole("link", { name: "Saltar al contenido principal" });
    await expect(skipLink).toBeFocused();
    const skipLinkBox = await skipLink.boundingBox();
    expect(skipLinkBox).not.toBeNull();
    expect(skipLinkBox?.y).toBeGreaterThanOrEqual(0);
    await expect.poll(() => skipLink.evaluate((element) => getComputedStyle(element).transform)).not.toBe("none");

    await page.keyboard.press("Enter");
    await expect(page.locator("main#main-content")).toBeFocused();

    const sizes = await page.locator("button:not(:disabled), a[href]").evaluateAll((elements) =>
      elements.map((element) => {
        const bounds = element.getBoundingClientRect();
        return { tag: element.tagName, width: bounds.width, height: bounds.height };
      }),
    );
    expect(sizes.length).toBeGreaterThan(0);
    for (const size of sizes) {
      expect(size.width, `${size.tag} width`).toBeGreaterThanOrEqual(24);
      expect(size.height, `${size.tag} height`).toBeGreaterThanOrEqual(24);
    }
  });

  test("mantiene el foco dentro del alertdialog y lo restaura al cerrarlo", async ({ page }) => {
    await installTauriProjectMock(page);
    await page.goto("/", { waitUntil: "commit" });

    await page.getByRole("button", { name: "Seleccionar dataset" }).click();
    await page.getByRole("button", { name: "Cargar" }).click();
    await page.getByLabel("Nombre del proyecto").fill("Ventas accesible");
    await page.getByRole("button", { name: "Guardar proyecto nuevo" }).click();
    await expect(page.getByRole("list", { name: "Proyectos guardados" })).toContainText("Ventas accesible");

    await page.getByRole("button", { name: "Eliminar" }).click();
    const dialog = page.getByRole("alertdialog", { name: "Eliminar “Ventas accesible”" });
    await expect(dialog).toBeVisible();
    await expect(dialog).toHaveAttribute("aria-modal", "true");
    await expect(dialog).toHaveAttribute("aria-labelledby", "delete-project-title");
    await expect(dialog).toHaveAttribute("aria-describedby", "delete-project-description");

    const cancel = dialog.getByRole("button", { name: "Cancelar" });
    const confirm = dialog.getByRole("button", { name: "Eliminar proyecto" });
    await expect(cancel).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(confirm).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(cancel).toBeFocused();
    await page.keyboard.press("Shift+Tab");
    await expect(confirm).toBeFocused();

    await page.keyboard.press("Escape");
    await expect(dialog).toBeHidden();
    await expect(page.getByRole("button", { name: "Eliminar" })).toBeFocused();
  });
});
