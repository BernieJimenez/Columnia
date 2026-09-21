import { expect, test, type Locator, type Page } from "@playwright/test";

async function installSyntheticTauriMock(page: Page) {
  await page.addInitScript(() => {
    const dataset = {
      fileName: "ventas-e2e.csv",
      fileSizeBytes: 128,
      rowCount: 2,
      columnCount: 2,
      columns: [
        { name: "cliente", dataType: "String" },
        { name: "total", dataType: "Float64" },
      ],
      rows: [["Ana", "10"], ["Luis", "20"]],
    };
    let callbackId = 0;
    const invokeCalls: string[] = [];
    const invoke = async (command: string, args: Record<string, unknown> = {}) => {
      invokeCalls.push(command);
      switch (command) {
        case "get_app_info":
          return { name: "Columnia", version: "0.168.0", platform: "windows" };
        case "list_sample_datasets":
        case "list_reusable_tasks":
          return [];
        case "list_projects":
          return { projects: [], recoveryCandidate: null };
        case "pick_dataset_source":
          return {
            selectionId: "selection-a11y-flow",
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
          return {
            rowCount: dataset.rowCount,
            duplicateRowCount: 0,
            nearDuplicateRowCount: 0,
            duplicatePercentage: 0,
            columns: [],
          };
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
        case "clear_dataset_comparison":
          return null;
        case "export_dataset":
          return {
            fileName: "ventas-e2e-export.csv",
            fileSizeBytes: 128,
            format: "CSV",
            protectedColumnCount: 0,
            protectedColumns: [],
          };
        default:
          throw new Error(`Comando Tauri no simulado: ${command}`);
      }
    };

    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      configurable: true,
      value: {
        invoke,
        transformCallback: () => ++callbackId,
        unregisterCallback: () => undefined,
      },
    });
    Object.defineProperty(window, "__COLUMNIA_ACCESSIBILITY_E2E__", {
      configurable: true,
      value: { invokeCalls },
    });
  });
}

async function tabTo(page: Page, target: Locator, description: string) {
  for (let index = 0; index < 160; index += 1) {
    await page.keyboard.press("Tab");
    if (await target.evaluate((element) => element === document.activeElement).catch(() => false)) return;
  }
  throw new Error(`No se pudo alcanzar con Tab: ${description}`);
}

async function activateWithKeyboard(
  page: Page,
  target: Locator,
  description: string,
  key: "Enter" | "Space" = "Enter",
) {
  await tabTo(page, target, description);
  await expect(target, `${description} debe recibir el foco de teclado`).toBeFocused();
  const focusState = await target.evaluate((element) => {
    const style = getComputedStyle(element);
    const bounds = element.getBoundingClientRect();
    return {
      outline: style.outlineStyle,
      outlineWidth: style.outlineWidth,
      boxShadow: style.boxShadow,
      fullyVisibleHorizontally: bounds.left >= -1 && bounds.right <= window.innerWidth + 1,
    };
  });
  expect(
    focusState.outline !== "none" && focusState.outlineWidth !== "0px" || focusState.boxShadow !== "none",
    `${description} debe mostrar el indicador de foco`,
  ).toBe(true);
  expect(
    focusState.fullyVisibleHorizontally,
    `${description} debe quedar visible al recibir foco, incluso con zoom o ventana estrecha`,
  ).toBe(true);
  await page.keyboard.press(key);
}

async function loadSyntheticDataset(page: Page, stopAt: "review" | "delivery" = "delivery") {
  await page.goto("/", { waitUntil: "commit" });
  await expect(page.locator("#app-title")).toBeVisible();

  await page.keyboard.press("Tab");
  const skipLink = page.getByRole("link", { name: "Saltar al contenido principal" });
  await expect(skipLink).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(page.locator("main#main-content")).toBeFocused();

  const selectDataset = page.getByRole("button", { name: "Seleccionar dataset" });
  await activateWithKeyboard(page, selectDataset, "Seleccionar dataset");
  const headerReview = page.getByRole("dialog", { name: "Revisar encabezados de ventas-e2e.csv" });
  await expect(headerReview).toBeVisible();
  await expect.poll(() => headerReview.evaluate((dialog) => dialog.contains(document.activeElement))).toBe(true);

  const loadButton = headerReview.getByRole("button", { name: "Cargar archivo" });
  await expect(loadButton).toBeEnabled();
  await activateWithKeyboard(page, loadButton, "Cargar archivo");

  const workflow = page.getByRole("navigation", { name: "Flujo de preparación de datos" });
  const reviewStep = workflow.getByRole("button", { name: "Revisar", exact: true });
  await expect(reviewStep).toHaveAttribute("aria-current", "step");
  await expect(page.getByRole("heading", { name: "Revisa antes de modificar" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Diagnóstico del dataset" })).toBeVisible();
  if (stopAt === "review") return;

  const continueToPrepare = page.getByRole("button", { name: "Continuar a Preparar" });
  await expect(continueToPrepare).toBeVisible();
  await activateWithKeyboard(page, continueToPrepare, "Continuar a Preparar");
  const prepareStep = workflow.getByRole("button", { name: "Preparar", exact: true });
  await expect(prepareStep).toHaveAttribute("aria-current", "step");
  await expect(page.getByRole("heading", { name: "Prepara datos consistentes" })).toBeVisible();

  const continueToDelivery = page.getByRole("button", { name: "Revisar opciones de entrega" });
  await expect(continueToDelivery).toBeVisible();
  await activateWithKeyboard(page, continueToDelivery, "Revisar opciones de entrega");
  const deliverStep = workflow.getByRole("button", { name: "Entregar", exact: true });
  await expect(deliverStep).toHaveAttribute("aria-current", "step");
  await expect(page.getByRole("heading", { name: "Exportar dataset activo" })).toBeVisible();
}

test.describe("recorrido cargado de accesibilidad", () => {
  test("mantiene el foco en la etapa nueva después de avanzar desde Revisar con teclado", async ({ page }) => {
    await installSyntheticTauriMock(page);
    await loadSyntheticDataset(page, "review");

    const continueToPrepare = page.getByRole("button", { name: "Continuar a Preparar" });
    await activateWithKeyboard(page, continueToPrepare, "Continuar a Preparar");
    await expect(page.getByRole("heading", { name: "Prepara datos consistentes" })).toBeVisible();
    const focusState = await page.evaluate(() => {
      const active = document.activeElement;
      const stage = document.querySelector(".workspace__stage");
      return {
        focusedElement: active?.tagName ?? "none",
        focusedText: active?.textContent?.trim().slice(0, 60) ?? "",
        focusStayedInNewStage: Boolean(active && stage?.contains(active)),
      };
    });
    // Proposed fix: when the keyed stage changes, focus its h2 (tabIndex=-1) or
    // the destination stage container so removing the source CTA cannot drop focus to body.
    expect(focusState.focusStayedInNewStage, JSON.stringify(focusState)).toBe(true);
  });

  test("completa Cargar → Revisar → Preparar → Entregar con teclado y asocia el error ODBC al campo", async ({ page }) => {
    await installSyntheticTauriMock(page);
    await loadSyntheticDataset(page);

    const format = page.getByRole("combobox", { name: "Formato de exportación" });
    await format.selectOption("postgresql");
    const connectionString = page.getByRole("textbox", { name: "Cadena de conexión ODBC" });
    const connectionError = page.getByRole("alert").filter({ hasText: "Indica la cadena de conexión ODBC." });
    await expect(connectionError).toBeVisible();
    await expect(connectionString).toHaveAttribute("aria-invalid", "true");
    const errorId = await connectionError.getAttribute("id");
    expect(errorId).toBeTruthy();
    await expect(connectionString).toHaveAttribute("aria-describedby", errorId!);
    await expect.poll(() => connectionString.evaluate((field) => {
      const descriptionId = field.getAttribute("aria-describedby");
      return descriptionId ? document.getElementById(descriptionId)?.textContent ?? "" : "";
    })).toContain("Indica la cadena de conexión ODBC.");

    await format.selectOption("csv");
    const unvalidatedConfirmation = page.getByRole("checkbox", { name: "Confirmo que quiero exportar sin validar la calidad" });
    await activateWithKeyboard(page, unvalidatedConfirmation, "Confirmar exportación sin validación", "Space");
    const exportButton = page.getByRole("button", { name: "Exportar CSV", exact: true });
    await expect(exportButton).toBeEnabled();
    await activateWithKeyboard(page, exportButton, "Exportar CSV");
    await expect(page.getByRole("heading", { name: "Copia lista" })).toBeVisible();
    await expect(page.getByRole("region", { name: "Etapa Entregar" })).toContainText("ventas-e2e-export.csv");
    const invokedExport = await page.evaluate(() =>
      (window as Window & { __COLUMNIA_ACCESSIBILITY_E2E__?: { invokeCalls: string[] } })
        .__COLUMNIA_ACCESSIBILITY_E2E__?.invokeCalls.includes("export_dataset") ?? false,
    );
    expect(invokedExport).toBe(true);
  });

  test("conserva el layout de Entregar al 200 % y a 320 píxeles CSS", async ({ page }) => {
    await installSyntheticTauriMock(page);
    await loadSyntheticDataset(page);

    const inspectHorizontalLayout = async (label: string) => {
      const metrics = await page.evaluate(() => {
        const root = document.documentElement;
        const body = document.body;
        const overflows = [...document.querySelectorAll<HTMLElement>("body *")]
          .filter((element) => {
            const style = getComputedStyle(element);
            if (style.display === "none" || style.visibility === "hidden") return false;
            const bounds = element.getBoundingClientRect();
            return bounds.width > 0 && (bounds.left < -1 || bounds.right > window.innerWidth + 1);
          })
          .slice(0, 8)
          .map((element) => ({
            tag: element.tagName,
            className: typeof element.className === "string" ? element.className : "",
            text: element.textContent?.trim().slice(0, 48) ?? "",
            left: Math.round(element.getBoundingClientRect().left),
            right: Math.round(element.getBoundingClientRect().right),
          }));
        return {
          viewportWidth: window.innerWidth,
          documentWidth: Math.max(root.scrollWidth, body.scrollWidth),
          overflow: Math.max(root.scrollWidth, body.scrollWidth) > window.innerWidth + 1,
          overflows,
        };
      });
      expect(metrics.overflow, `${label}: ${JSON.stringify(metrics)}`).toBe(false);
    };

    await page.evaluate(() => {
      document.documentElement.style.zoom = "2";
      document.documentElement.dataset.columniaZoom = "2";
    });
    await inspectHorizontalLayout("zoom 200 % · Entregar");
    await activateWithKeyboard(page, page.getByRole("button", { name: "Volver a Preparar", exact: true }), "Volver a Preparar");
    await expect(page.getByRole("heading", { name: "Prepara datos consistentes" })).toBeVisible();
    await inspectHorizontalLayout("zoom 200 % · Preparar");
    const prepareAction = page.getByRole("button", { name: "Aplicar plan seleccionado" });
    const nextAction = page.getByRole("button", { name: "Revisar opciones de entrega" });
    await expect(prepareAction).toBeEnabled();
    await expect(nextAction).toBeEnabled();
    const [prepareBackground, nextBackground] = await Promise.all([
      prepareAction.evaluate((element) => getComputedStyle(element).backgroundColor),
      nextAction.evaluate((element) => getComputedStyle(element).backgroundColor),
    ]);
    expect(nextBackground).not.toBe(prepareBackground);
    await page.emulateMedia({ forcedColors: "active" });
    await page.waitForTimeout(200);
    const forcedColorsStyle = await nextAction.evaluate((element) => {
      const systemButton = document.createElement("button");
      systemButton.style.backgroundColor = "ButtonFace";
      systemButton.style.color = "ButtonText";
      document.body.append(systemButton);
      const expected = getComputedStyle(systemButton);
      const actual = getComputedStyle(element);
      const result = {
        active: matchMedia("(forced-colors: active)").matches,
        background: actual.backgroundColor,
        foreground: actual.color,
        buttonFace: expected.backgroundColor,
        buttonText: expected.color,
      };
      systemButton.remove();
      return result;
    });
    expect(forcedColorsStyle.active).toBe(true);
    expect(forcedColorsStyle.background).toBe(forcedColorsStyle.buttonFace);
    expect(forcedColorsStyle.foreground).toBe(forcedColorsStyle.buttonText);
    await page.emulateMedia({ forcedColors: "none" });
    await activateWithKeyboard(page, page.getByRole("button", { name: "Volver a Revisar", exact: true }), "Volver a Revisar");
    await expect(page.getByRole("heading", { name: "Revisa antes de modificar" })).toBeVisible();
    await inspectHorizontalLayout("zoom 200 % · Revisar");

    await page.evaluate(() => {
      document.documentElement.style.zoom = "1";
      delete document.documentElement.dataset.columniaZoom;
    });
    await page.setViewportSize({ width: 320, height: 900 });
    await inspectHorizontalLayout("viewport de 320 píxeles CSS · Revisar");
    await activateWithKeyboard(page, page.getByRole("button", { name: "Continuar a Preparar" }), "Continuar a Preparar · 320 px");
    await expect(page.getByRole("heading", { name: "Prepara datos consistentes" })).toBeVisible();
    await inspectHorizontalLayout("viewport de 320 píxeles CSS · Preparar");
    await activateWithKeyboard(page, page.getByRole("button", { name: "Revisar opciones de entrega" }), "Revisar opciones de entrega · 320 px");
    await expect(page.getByRole("heading", { name: "Exportar dataset activo" })).toBeVisible();
    await inspectHorizontalLayout("viewport de 320 píxeles CSS · Entregar");
  });
});
