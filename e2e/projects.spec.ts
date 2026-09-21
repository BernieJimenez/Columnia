import { expect, test, type Page } from "@playwright/test";

async function installTauriProjectMock(page: Page, seedRecoveryCandidate = false) {
  await page.addInitScript((hasRecoveryCandidate) => {
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
    let projects: typeof project[] = hasRecoveryCandidate ? [project] : [];
    let latestProject = project;
    let recoveryCandidate: typeof project | null = hasRecoveryCandidate ? project : null;
    let callbackId = 0;

    const invoke = async (command: string, args: Record<string, unknown> = {}) => {
      switch (command) {
        case "get_app_info":
          return { name: "Columnia", version: "0.49.0", platform: "windows" };
        case "list_sample_datasets":
        case "list_reusable_tasks":
          return [];
        case "list_projects":
          return { projects, recoveryCandidate };
        case "pick_dataset_source":
          return {
            selectionId: "selection-e2e",
            fileName: dataset.fileName,
            fileSizeBytes: dataset.fileSizeBytes,
            format: "csv",
            sheets: [],
            defaultSheetId: null,
            isCompressedContainer: false,
            resourceEstimate: {
              processingPath: "inMemory",
              estimatedMaterializationRamBytes: 268435968,
              estimatedTemporaryDiskBytes: null,
            },
          };
        case "preview_delimited_header_review":
          return {
            delimiter: ",",
            firstRow: {
              headerMode: "firstRow",
              columns: dataset.columns,
              rows: dataset.rows,
              includesFirstRow: false,
              sampleTruncated: false,
            },
            generated: {
              headerMode: "generated",
              columns: dataset.columns.map((column, index) => ({ ...column, name: `column_${index + 1}` })),
              rows: dataset.rows,
              includesFirstRow: true,
              sampleTruncated: false,
            },
          };
        case "load_dataset_selection":
          return dataset;
        case "get_dataset_profile":
          return { rowCount: dataset.rowCount, duplicateRowCount: 0, nearDuplicateRowCount: 0, duplicatePercentage: 0, columns: [] };
        case "get_history_state":
          return {
            canUndo: false,
            canRedo: false,
            currentIndex: 0,
            entryCount: 0,
            entries: [],
            snapshotsEnabled: true,
            degradedReason: null,
            maxEntries: 50,
            diskBytes: 0,
            diskBudgetBytes: 536870912,
          };
        case "get_dataset_page":
          return { offset: args.offset ?? 0, rows: dataset.rows };
        case "save_project": {
          if (hasRecoveryCandidate && args.projectId !== latestProject.id) {
            throw new Error("La actualización debe conservar el ID del proyecto recuperado.");
          }
          latestProject = {
            ...latestProject,
            id: typeof args.projectId === "string" ? args.projectId : latestProject.id,
            name: typeof args.name === "string" ? args.name : latestProject.name,
            updatedAt: "2026-01-02T00:00:00.000Z",
          };
          projects = [latestProject];
          recoveryCandidate = null;
          return latestProject;
        }
        case "open_project": {
          const openedProject = projects.find((item) => item.id === args.projectId) ?? latestProject;
          recoveryCandidate = null;
          return {
            project: openedProject,
            dataset,
            workspace: {
              qualityRules: [],
              recipeDraft: null,
              ...(hasRecoveryCandidate ? { activePhase: "prepare" } : {}),
            },
            profile: null,
          };
        }
        case "delete_project":
          projects = [];
          recoveryCandidate = null;
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
  }, seedRecoveryCandidate);
}

async function selectAndConfirmDataset(page: Page) {
  await page.getByRole("button", { name: "Seleccionar dataset" }).click();
  const headerReview = page.getByRole("dialog", { name: "Revisar encabezados de ventas.csv" });
  const loadButton = headerReview.getByRole("button", { name: "Cargar archivo" });
  await expect(loadButton).toBeEnabled();
  await loadButton.click();
}

test("recorre guardar, abrir y eliminar un proyecto desde el shell Tauri simulado", async ({ page }) => {
  await installTauriProjectMock(page);
  await page.goto("/", { waitUntil: "commit" });

  await expect(page.getByRole("button", { name: "Seleccionar dataset" })).toBeVisible();
  await selectAndConfirmDataset(page);
  const workflow = page.getByRole("navigation", { name: "Flujo de preparación de datos" });
  await expect(workflow.getByRole("button", { name: "Revisar", exact: true })).toHaveAttribute("aria-current", "step");

  await page
    .getByRole("navigation", { name: "Flujo de preparación de datos" })
    .getByRole("button", { name: "Cargar", exact: true })
    .click();
  await page.locator(".load-secondary").filter({ hasText: "Continuar un proyecto" }).locator(":scope > summary").click();
  await expect(page.getByRole("heading", { name: "Proyectos" })).toBeVisible();
  await page.getByText("Guardar y administrar proyectos", { exact: true }).click();
  await page.getByLabel("Nombre del proyecto").fill("Ventas E2E");
  await page.getByRole("button", { name: "Guardar proyecto nuevo" }).click();
  await expect(page.locator("p.notice--success")).toContainText("Proyecto “Ventas E2E” guardado.");
  await expect(page.getByRole("list", { name: "Proyectos guardados" })).toContainText("Ventas E2E");

  await page.getByRole("button", { name: "Abrir" }).click();
  await expect(workflow.getByRole("button", { name: "Revisar", exact: true })).toHaveAttribute("aria-current", "step");
  await expect(page.getByRole("heading", { name: "Revisa antes de modificar" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "ventas.csv" })).toBeVisible();

  await workflow.getByRole("button", { name: "Cargar", exact: true }).click();
  await page.locator(".load-secondary").filter({ hasText: "Continuar un proyecto" }).locator(":scope > summary").click();
  await page.getByText("Guardar y administrar proyectos", { exact: true }).click();
  await expect(page.getByRole("list", { name: "Proyectos guardados" })).toContainText("Ventas E2E · activo");
  await page.getByRole("button", { name: "Eliminar" }).click();
  await expect(page.getByRole("alertdialog", { name: "Eliminar “Ventas E2E”" })).toBeVisible();
  await page.getByRole("button", { name: "Eliminar proyecto" }).click();
  await expect(page.locator("p.notice--success")).toContainText("Proyecto “Ventas E2E” eliminado.");
  await expect(page.getByText("Todavía no hay proyectos guardados.")).toBeVisible();
});

test("recupera la última sesión, restaura su etapa y actualiza el mismo proyecto", async ({ page }) => {
  await installTauriProjectMock(page, true);
  await page.goto("/", { waitUntil: "commit" });

  const workflow = page.getByRole("navigation", { name: "Flujo de preparación de datos" });
  const projectDetails = page.locator(".load-secondary")
    .filter({ hasText: "Continuar un proyecto" });
  await projectDetails.locator(":scope > summary").click();
  await expect(page.getByRole("button", { name: "Recuperar proyecto" })).toBeVisible();
  await page.getByRole("button", { name: "Recuperar proyecto" }).click();

  await expect(workflow.getByRole("button", { name: "Preparar", exact: true }))
    .toHaveAttribute("aria-current", "step");
  await expect(page.getByRole("heading", { name: "Prepara datos consistentes" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "ventas.csv" })).toBeVisible();

  await workflow.getByRole("button", { name: "Cargar", exact: true }).click();
  await projectDetails.locator(":scope > summary").click();
  await page.getByText("Guardar y administrar proyectos", { exact: true }).click();
  const projects = page.getByRole("list", { name: "Proyectos guardados" });
  await expect(projects).toContainText("Ventas E2E · activo");
  const projectName = page.getByLabel("Nombre del proyecto");
  await expect(projectName).toHaveValue("Ventas E2E");
  await projectName.fill("Ventas recuperadas");
  await page.getByRole("button", { name: "Actualizar proyecto" }).click();

  await expect(page.locator("p.notice--success"))
    .toContainText("Proyecto “Ventas recuperadas” actualizado.");
  await expect(projects).toContainText("Ventas recuperadas · activo");
});
