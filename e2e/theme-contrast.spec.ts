import { expect, test, type Page } from "@playwright/test";

// Synthetic dataset with missing values, typed columns, duplicates and
// personal-data signals, so every contrast-sensitive element is rendered.
async function installContrastMock(page: Page) {
  await page.addInitScript(() => {
    const columns = [
      { name: "id", dataType: "Int64" },
      { name: "cliente", dataType: "String" },
      { name: "correo", dataType: "String" },
      { name: "total", dataType: "Float64" },
    ];
    const rows = [
      ["1", "Ana", "ana@example.com", "10.5"],
      ["2", "  Luis ", null, null],
      ["3", "Ana", "ana@example.com", "10.5"],
    ];
    const dataset = { fileName: "contraste.csv", fileSizeBytes: 256, rowCount: rows.length, columnCount: columns.length, columns, rows };
    const column = (overrides: Record<string, unknown>) => ({
      nullCount: 0, completenessPercentage: 100, uniqueCount: 3, minimum: null, maximum: null, mean: null,
      emptyCount: 0, minimumLength: null, maximumLength: null, averageLength: null, suggestedType: null,
      typeMatchPercentage: null, invalidTypeCount: null, sentinelCount: null, encodingIssueCount: null,
      privacySignal: null, standardDeviation: null, firstQuartile: null, median: null, thirdQuartile: null,
      outlierCount: null, histogram: null, ...overrides,
    });
    const profile = {
      rowCount: rows.length, duplicateRowCount: 1, nearDuplicateRowCount: 1, duplicatePercentage: 33.3,
      columns: [
        column({ name: "id", dataType: "Int64" }),
        column({ name: "cliente", dataType: "String", privacySignal: "name" }),
        column({ name: "correo", dataType: "String", nullCount: 1, completenessPercentage: 66.7, privacySignal: "email" }),
        column({ name: "total", dataType: "Float64", nullCount: 1, completenessPercentage: 66.7 }),
      ],
    };
    let callbackId = 0;
    const invoke = async (command: string, args: Record<string, unknown> = {}) => {
      switch (command) {
        case "plugin:event|listen": return ++callbackId;
        case "plugin:event|unlisten": case "discard_dataset_selection": case "clear_dataset_comparison": case "cancel_operation": return null;
        case "get_app_info": return { name: "Columnia", version: "1.26.0", platform: "windows", updaterConfigured: false };
        case "list_sample_datasets": case "list_reusable_tasks": case "list_delivery_presets": return [];
        case "list_projects": return { projects: [], recoveryCandidate: null };
        case "pick_dataset_source": return { selectionId: "contrast", fileName: dataset.fileName, fileSizeBytes: 256, format: "csv", sheets: [], defaultSheetId: null, isCompressedContainer: false, resourceEstimate: { processingPath: "inMemory", estimatedMaterializationRamBytes: 268435968, estimatedTemporaryDiskBytes: null } };
        case "preview_delimited_header_review": return { delimiter: ",", firstRow: { headerMode: "firstRow", columns, rows, includesFirstRow: false, sampleTruncated: false }, generated: { headerMode: "generated", columns, rows, includesFirstRow: true, sampleTruncated: false } };
        case "preview_dataset_selection": return { rowCount: rows.length, columns, schemaMismatch: null };
        case "load_dataset_selection": return dataset;
        case "get_dataset_profile": return profile;
        case "get_history_state": return { canUndo: false, canRedo: false, currentIndex: 0, entryCount: 0, entries: [], snapshotsEnabled: true, degradedReason: null, maxEntries: 50, diskBytes: 0, diskBudgetBytes: 536870912 };
        case "get_dataset_page": return { offset: args.offset ?? 0, rows };
        default: throw new Error(`Comando Tauri no simulado: ${command}`);
      }
    };
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      configurable: true,
      value: { invoke, transformCallback: () => ++callbackId, unregisterCallback: () => undefined },
    });
  });
}

/** Returns visible text elements below the WCAG AA contrast minimum. */
async function contrastFailures(page: Page) {
  return page.evaluate(() => {
    const parse = (value: string) => {
      const parts = value.match(/rgba?\(([^)]+)\)/)?.[1].split(/[ ,/]+/).filter(Boolean).map(Number) ?? [0, 0, 0, 0];
      return { r: parts[0], g: parts[1], b: parts[2], a: parts[3] ?? 1 };
    };
    const channel = (value: number) => {
      const normalized = value / 255;
      return normalized <= 0.03928 ? normalized / 12.92 : ((normalized + 0.055) / 1.055) ** 2.4;
    };
    const luminance = ({ r, g, b }: { r: number; g: number; b: number }) => 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b);
    const background = (element: Element | null) => {
      for (let current = element; current; current = current.parentElement) {
        const color = parse(getComputedStyle(current).backgroundColor);
        if (color.a > 0.99) return color;
      }
      return { r: 255, g: 255, b: 255, a: 1 };
    };
    const failures: string[] = [];
    for (const element of document.querySelectorAll("main *, aside *, dialog *")) {
      const hasText = [...element.childNodes].some((node) => node.nodeType === Node.TEXT_NODE && node.textContent?.trim());
      if (!hasText || !element.checkVisibility({ visibilityProperty: true, contentVisibilityAuto: true })) continue;
      if (element.closest(".visually-hidden, button:disabled, [aria-disabled='true']")) continue;
      const style = getComputedStyle(element);
      const [lighter, darker] = [luminance(parse(style.color)), luminance(background(element))].sort((left, right) => right - left);
      const ratio = (lighter + 0.05) / (darker + 0.05);
      const size = Number.parseFloat(style.fontSize);
      const large = size >= 24 || (size >= 18.66 && Number(style.fontWeight) >= 700);
      if (ratio < (large ? 3 : 4.5)) failures.push(`${element.textContent?.trim().slice(0, 40)} → ${ratio.toFixed(2)}:1`);
    }
    return failures;
  });
}

const themes = [
  { name: "claro", colorScheme: "light", dataTheme: "light" },
  { name: "oscuro", colorScheme: "dark", dataTheme: "dark" },
  { name: "sistema con SO oscuro", colorScheme: "dark", dataTheme: "system" },
] as const;

for (const theme of themes) {
  test(`las fases cargadas cumplen contraste AA en tema ${theme.name}`, async ({ page }) => {
    await installContrastMock(page);
    await page.emulateMedia({ colorScheme: theme.colorScheme, reducedMotion: "reduce" });
    await page.addInitScript((value) => localStorage.setItem("columnia.theme", value), theme.dataTheme);
    await page.goto("/");
    await page.getByRole("button", { name: "Seleccionar dataset" }).click();
    const review = page.getByRole("dialog", { name: "Revisar encabezados de contraste.csv" });
    await review.getByRole("button", { name: "Revisar esquema" }).click();
    await review.getByRole("button", { name: "Cargar archivo" }).click();
    await expect(page.getByRole("heading", { name: "Revisa antes de modificar" })).toBeVisible();
    await page.getByRole("tab", { name: "Vista previa" }).click();
    await expect(page.locator("main .null-value").first()).toBeVisible();
    const workflow = page.getByRole("navigation", { name: "Flujo de preparación de datos" });
    const failures: string[] = [];
    failures.push(...(await contrastFailures(page)).map((failure) => `Revisar: ${failure}`));
    await workflow.getByRole("button", { name: "Preparar", exact: true }).click();
    await expect(page.getByRole("button", { name: /^Aplicar \d+ cambios?$/ })).toBeVisible();
    failures.push(...(await contrastFailures(page)).map((failure) => `Preparar: ${failure}`));
    await workflow.getByRole("button", { name: "Entregar", exact: true }).click();
    await expect(page.getByText(/Datos personales detectados/)).toBeVisible();
    failures.push(...(await contrastFailures(page)).map((failure) => `Entregar: ${failure}`));
    expect(failures, failures.join("\n")).toEqual([]);
  });
}
