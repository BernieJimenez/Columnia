import { expect, test, type Page } from "@playwright/test";

import type { ColumnProfile, DatasetPreview, DatasetProfile, ExplorePanel } from "../src/bridge";
import { installTauriMock } from "./support/tauri-mock";

// Synthetic dataset with missing values, typed columns, duplicates, dates and
// personal-data signals, so every contrast-sensitive element is rendered.
const columns = [
  { name: "id", dataType: "Int64" },
  { name: "cliente", dataType: "String" },
  { name: "correo", dataType: "String" },
  { name: "total", dataType: "Float64" },
  { name: "fecha", dataType: "Date" },
];
const rows = [
  ["1", "Ana", "ana@example.com", "10.5", "2026-01-05"],
  ["2", "  Luis ", null, null, "2026-02-11"],
  ["3", "Ana", "ana@example.com", "10.5", "2026-03-20"],
];
const dataset = {
  fileName: "contraste.csv", fileSizeBytes: 256, rowCount: rows.length, columnCount: columns.length, columns, rows,
} satisfies DatasetPreview;

function column(overrides: Pick<ColumnProfile, "name" | "dataType"> & Partial<ColumnProfile>): ColumnProfile {
  return {
    nullCount: 0, completenessPercentage: 100, uniqueCount: 3, minimum: null, maximum: null, mean: null,
    emptyCount: 0, minimumLength: null, maximumLength: null, averageLength: null, suggestedType: null,
    typeMatchPercentage: null, invalidTypeCount: null, sentinelCount: null, encodingIssueCount: null,
    privacySignal: null, standardDeviation: null, firstQuartile: null, median: null, thirdQuartile: null,
    outlierCount: null, histogram: null, dateOrder: null, dateHasTime: null, untrimmedCount: null, ...overrides,
  };
}

const profile = {
  rowCount: rows.length, duplicateRowCount: 1, nearDuplicateRowCount: 1, duplicatePercentage: 33.3,
  columns: [
    column({ name: "id", dataType: "Int64" }),
    column({ name: "cliente", dataType: "String", privacySignal: "name", untrimmedCount: 1 }),
    column({ name: "correo", dataType: "String", nullCount: 1, completenessPercentage: 66.7, privacySignal: "email" }),
    column({
      name: "total", dataType: "Float64", nullCount: 1, completenessPercentage: 66.7, minimum: "10.5", maximum: "10.5", mean: 10.5,
      histogram: [{ lower: 10, upper: 11, count: 2 }],
    }),
    column({ name: "fecha", dataType: "Date", dateOrder: "iso", dateHasTime: false }),
  ],
  temporalSeries: [{
    column: "fecha", granularity: "month", parsedRowCount: 3, unparsedRowCount: 0, truncated: false,
    periods: [
      { period: "2026-01", rowCount: 1, percentage: 33.3 },
      { period: "2026-02", rowCount: 1, percentage: 33.3 },
      { period: "2026-03", rowCount: 1, percentage: 33.3 },
    ],
  }],
} satisfies DatasetProfile;

const explore = {
  rowCount: rows.length,
  totalRowCount: rows.length,
  kpis: [{ kind: "count", column: null, value: rows.length }, { kind: "mean", column: "total", value: 10.5 }],
  categories: [{ column: "cliente", bars: [{ value: "Ana", count: 2 }, { value: null, count: 1 }], otherCount: 0, distinctCount: 2 }],
  histogram: { column: "total", bins: [{ lower: 10, upper: 11, count: 2 }] },
  trend: null,
  options: { categories: ["cliente"], measures: ["total"], dates: ["fecha"] },
} satisfies ExplorePanel;

async function installContrastMock(page: Page) {
  await installTauriMock(page, { dataset, profile, explore });
}

/**
 * Elements below the WCAG AA minimum: HTML text (4.5:1, 3:1 when large), SVG
 * text by its fill, and the edges of form fields (3:1, WCAG 1.4.11). Partly
 * transparent backgrounds are composed over their ancestors (QA-07).
 */
async function contrastFailures(page: Page) {
  return page.evaluate(() => {
    type Rgba = { r: number; g: number; b: number; a: number };
    const parse = (value: string): Rgba => {
      const parts = value.match(/rgba?\(([^)]+)\)/)?.[1].split(/[ ,/]+/).filter(Boolean).map(Number) ?? [0, 0, 0, 0];
      return { r: parts[0], g: parts[1], b: parts[2], a: parts[3] ?? 1 };
    };
    const over = (top: Rgba, bottom: Rgba): Rgba => ({
      r: top.r * top.a + bottom.r * (1 - top.a),
      g: top.g * top.a + bottom.g * (1 - top.a),
      b: top.b * top.a + bottom.b * (1 - top.a),
      a: 1,
    });
    const channel = (value: number) => {
      const normalized = value / 255;
      return normalized <= 0.03928 ? normalized / 12.92 : ((normalized + 0.055) / 1.055) ** 2.4;
    };
    const luminance = ({ r, g, b }: Rgba) => 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b);
    const ratio = (left: Rgba, right: Rgba) => {
      const [lighter, darker] = [luminance(left), luminance(right)].sort((a, b) => b - a);
      return (lighter + 0.05) / (darker + 0.05);
    };
    const background = (element: Element | null): Rgba => {
      const layers: Rgba[] = [];
      for (let current = element; current; current = current.parentElement) {
        const color = parse(getComputedStyle(current).backgroundColor);
        if (color.a > 0) layers.push(color);
        if (color.a > 0.99) break;
      }
      let result = parse(getComputedStyle(document.body).backgroundColor);
      if (result.a < 1) result = { r: 255, g: 255, b: 255, a: 1 };
      for (const layer of layers.reverse()) result = over(layer, result);
      return result;
    };
    const visible = (element: Element) => element.checkVisibility({ visibilityProperty: true, contentVisibilityAuto: true });
    const skipped = (element: Element) => Boolean(element.closest(".visually-hidden, button:disabled, [aria-disabled='true']"));
    const failures: string[] = [];
    let svgTexts = 0;
    for (const element of document.querySelectorAll("main *, aside *, dialog *")) {
      if (!visible(element) || skipped(element)) continue;
      const style = getComputedStyle(element);
      if (element instanceof SVGTextElement) {
        svgTexts += 1;
        const fill = parse(style.fill);
        const value = ratio(fill, background(element.closest("svg")));
        if (value < 4.5) failures.push(`svg «${element.textContent?.trim().slice(0, 30)}» → ${value.toFixed(2)}:1`);
        continue;
      }
      if (element.matches("input:not([type='checkbox']):not([type='radio']):not([type='range']), select, textarea")) {
        const value = ratio(parse(style.borderTopColor), background(element.parentElement));
        if (Number.parseFloat(style.borderTopWidth) > 0 && value < 3) {
          failures.push(`borde de ${element.tagName.toLowerCase()} «${element.getAttribute("aria-label") ?? element.id}» → ${value.toFixed(2)}:1`);
        }
      }
      const hasText = [...element.childNodes].some((node) => node.nodeType === Node.TEXT_NODE && node.textContent?.trim());
      if (!hasText || element.closest("svg")) continue;
      const value = ratio(parse(style.color), background(element));
      const size = Number.parseFloat(style.fontSize);
      const large = size >= 24 || (size >= 18.66 && Number(style.fontWeight) >= 700);
      if (value < (large ? 3 : 4.5)) {
        const back = background(element);
        failures.push(`${element.textContent?.trim().slice(0, 40)} → ${value.toFixed(2)}:1 (${style.color} sobre rgb(${Math.round(back.r)}, ${Math.round(back.g)}, ${Math.round(back.b)}))`);
      }
    }
    return { failures, svgTexts };
  });
}

// QA-07: the five themes, plus "system" following a dark OS.
const themes = [
  { name: "claro", colorScheme: "light", dataTheme: "light" },
  { name: "oscuro", colorScheme: "dark", dataTheme: "dark" },
  { name: "sistema con SO oscuro", colorScheme: "dark", dataTheme: "system" },
  { name: "papel", colorScheme: "light", dataTheme: "paper" },
  { name: "océano", colorScheme: "light", dataTheme: "ocean" },
  { name: "pizarra", colorScheme: "light", dataTheme: "slate" },
] as const;

for (const theme of themes) {
  test(`las cinco fases cumplen contraste AA en tema ${theme.name}`, async ({ page }) => {
    await installContrastMock(page);
    await page.emulateMedia({ colorScheme: theme.colorScheme, reducedMotion: "reduce" });
    await page.addInitScript((value) => localStorage.setItem("columnia.theme", value), theme.dataTheme);
    await page.goto("/");
    const failures: string[] = [];
    const measure = async (phase: string) => {
      // A control that has just been enabled can still be painted with the
      // disabled colours for a frame; measure the settled page.
      await page.waitForTimeout(250);
      const result = await contrastFailures(page);
      failures.push(...result.failures.map((failure) => `${phase}: ${failure}`));
      return result;
    };
    await expect(page.getByRole("button", { name: "Seleccionar dataset" })).toBeVisible();
    await measure("Cargar");
    await page.getByRole("button", { name: "Seleccionar dataset" }).click();
    const review = page.getByRole("dialog", { name: "Revisar encabezados de contraste.csv" });
    await expect(review.getByRole("button", { name: "Cargar archivo" })).toBeEnabled();
    await measure("Importar");
    await review.getByRole("button", { name: "Cargar archivo" }).click();
    await expect(page.getByRole("heading", { name: "Revisa antes de modificar" })).toBeVisible();
    await page.getByText("Más análisis y herramientas", { exact: true }).click();
    await page.getByText("Explorar análisis detallado", { exact: true }).click();
    await expect(page.locator("main svg text").first()).toBeAttached();
    const diagnosis = await measure("Revisar");
    expect(diagnosis.svgTexts, "la tendencia temporal se mide por su relleno SVG").toBeGreaterThan(0);
    await page.getByRole("tab", { name: "Vista previa" }).click();
    await expect(page.locator("main .null-value").first()).toBeVisible();
    await measure("Revisar · vista previa");
    const workflow = page.getByRole("navigation", { name: "Flujo de preparación de datos" });
    await workflow.getByRole("button", { name: "Preparar", exact: true }).click();
    await expect(page.getByRole("button", { name: /^Aplicar \d+ cambios?$/ })).toBeVisible();
    await measure("Preparar");
    await workflow.getByRole("button", { name: "Explorar", exact: true }).click();
    await expect(page.getByRole("heading", { name: "Explora los datos limpios" })).toBeVisible();
    await expect(page.getByRole("button", { name: /^Sin dato/ })).toBeVisible();
    await measure("Explorar");
    await workflow.getByRole("button", { name: "Entregar", exact: true }).click();
    await expect(page.getByText(/Datos personales detectados/)).toBeVisible();
    await measure("Entregar");
    expect(failures, failures.join("\n")).toEqual([]);
  });
}

/** Colors of every visible element in document order, for theme parity checks. */
async function colorSnapshot(page: Page) {
  return page.evaluate(() => {
    const rows: string[] = [];
    for (const element of document.querySelectorAll("body *")) {
      if (!element.checkVisibility({ visibilityProperty: true, contentVisibilityAuto: true })) continue;
      const style = getComputedStyle(element);
      const label = `${element.tagName.toLowerCase()}.${[...element.classList].join(".")}`;
      rows.push(`${label}|${style.color}|${style.backgroundColor}|${style.borderTopColor}|${style.borderLeftColor}|${style.boxShadow}`);
    }
    return rows;
  });
}

async function phaseSnapshots(page: Page, colorScheme: "dark", dataTheme: "dark" | "system") {
  await installContrastMock(page);
  await page.emulateMedia({ colorScheme, reducedMotion: "reduce" });
  await page.addInitScript((value) => localStorage.setItem("columnia.theme", value), dataTheme);
  await page.goto("/");
  const snapshots: Record<string, string[]> = {};
  await page.getByRole("button", { name: "Seleccionar dataset" }).waitFor();
  await page.waitForTimeout(300);
  snapshots.Cargar = await colorSnapshot(page);
  await page.getByRole("button", { name: "Seleccionar dataset" }).click();
  const review = page.getByRole("dialog", { name: "Revisar encabezados de contraste.csv" });
  await review.getByRole("button", { name: "Cargar archivo" }).click();
  await expect(page.getByRole("heading", { name: "Revisa antes de modificar" })).toBeVisible();
  await page.waitForTimeout(300);
  snapshots.Revisar = await colorSnapshot(page);
  const workflow = page.getByRole("navigation", { name: "Flujo de preparación de datos" });
  await workflow.getByRole("button", { name: "Preparar", exact: true }).click();
  await expect(page.getByRole("button", { name: /^Aplicar \d+ cambios?$/ })).toBeVisible();
  await page.waitForTimeout(300);
  snapshots.Preparar = await colorSnapshot(page);
  await workflow.getByRole("button", { name: "Entregar", exact: true }).click();
  await expect(page.getByText(/Datos personales detectados/)).toBeVisible();
  await page.waitForTimeout(300);
  snapshots.Entregar = await colorSnapshot(page);
  return snapshots;
}

test("los temas Oscuro y Sistema con SO oscuro pintan lo mismo (T10-11)", async ({ browser }) => {
  const darkPage = await browser.newPage();
  const systemPage = await browser.newPage();
  const dark = await phaseSnapshots(darkPage, "dark", "dark");
  const system = await phaseSnapshots(systemPage, "dark", "system");
  const differences: string[] = [];
  for (const phase of Object.keys(dark)) {
    const length = Math.max(dark[phase].length, system[phase].length);
    for (let index = 0; index < length; index += 1) {
      if (dark[phase][index] !== system[phase][index]) {
        differences.push(`${phase}: ${dark[phase][index]} ≠ ${system[phase][index]}`);
      }
    }
  }
  await darkPage.close();
  await systemPage.close();
  expect(differences, `${differences.length} diferencias\n${differences.slice(0, 25).join("\n")}`).toEqual([]);
});
