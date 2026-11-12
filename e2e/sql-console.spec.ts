import { expect, test } from "@playwright/test";

import type { DatasetPreview } from "../src/bridge";
import { installTauriMock } from "./support/tauri-mock";

const dataset = {
  fileName: "consulta-e2e.csv",
  fileSizeBytes: 128,
  rowCount: 2,
  columnCount: 2,
  columns: [{ name: "cliente", dataType: "String" }, { name: "total", dataType: "Float64" }],
  rows: [["Ana", "10"], ["Luis", "20"]],
} satisfies DatasetPreview;

test.describe("consulta SQL desde la barra superior", () => {
  test("se abre en cualquier etapa, muestra la consulta abierta y devuelve el foco al cerrar", async ({ page }) => {
    await installTauriMock(page, { dataset });
    await page.goto("/", { waitUntil: "commit" });
    // Without a dataset there is nothing to query.
    await expect(page.getByRole("button", { name: "Consulta SQL" })).toHaveCount(0);

    await page.getByRole("button", { name: "Seleccionar dataset" }).click();
    await page.getByRole("dialog", { name: /Revisar encabezados/ }).getByRole("button", { name: "Cargar archivo" }).click();
    await expect(page.getByRole("heading", { name: "Revisa antes de modificar" })).toBeVisible();
    await page.getByRole("navigation", { name: "Flujo de preparación de datos" })
      .getByRole("button", { name: "Preparar", exact: true }).click();

    const trigger = page.getByRole("button", { name: "Consulta SQL" });
    await trigger.click();
    const drawer = page.getByRole("dialog", { name: "Consulta SQL" });
    await expect(drawer).toBeVisible();
    await expect(drawer).toContainText("consulta-e2e.csv");
    // The query is open at once: no second disclosure to expand.
    await expect(drawer.locator("details.review-tool")).toHaveCount(0);
    await expect(drawer.locator(".local-query")).toBeVisible();

    await page.keyboard.press("Escape");
    await expect(drawer).toHaveCount(0);
    await expect(trigger).toBeFocused();
  });

  // A result as wide as netflix_titles.csv pushed «Cerrar» out of the window.
  test("un resultado ancho se desplaza dentro de su tabla y «Cerrar» sigue a la vista", async ({ page }) => {
    const columns = ["show_id", "type", "title", "director", "cast", "country", "date_added", "release_year", "rating", "duration", "listed_in", "description"]
      .map((name) => ({ name, dataType: "String" }));
    const wide = {
      fileName: "netflix_titles.csv",
      fileSizeBytes: 3_399_671,
      rowCount: 8807,
      columnCount: columns.length,
      columns,
      rows: Array.from({ length: 6 }, (_, row) => columns.map(({ name }) => `${name} de la fila ${row + 1} con un texto largo`)),
    } satisfies DatasetPreview;
    await page.setViewportSize({ width: 1280, height: 860 });
    await installTauriMock(page, { dataset: wide });
    await page.goto("/", { waitUntil: "commit" });
    await page.getByRole("button", { name: "Seleccionar dataset" }).click();
    await page.getByRole("dialog", { name: /Revisar encabezados/ }).getByRole("button", { name: "Cargar archivo" }).click();
    await expect(page.getByRole("heading", { name: "Revisa antes de modificar" })).toBeVisible();

    await page.getByRole("button", { name: "Consulta SQL" }).click();
    const drawer = page.getByRole("dialog", { name: "Consulta SQL" });
    await drawer.getByRole("button", { name: "Ejecutar consulta" }).click();
    await expect(drawer.getByRole("region", { name: "Resultado de consulta SQL" })).toBeVisible();

    const close = drawer.getByRole("button", { name: "Cerrar" });
    await expect(close).toBeInViewport({ ratio: 1 });
    const layout = await drawer.evaluate((dialog) => {
      const panel = dialog.querySelector(".sheet-dialog__panel") as HTMLElement;
      const result = dialog.querySelector(".local-query__result .profile-region") as HTMLElement;
      return {
        panelOverflow: panel.scrollWidth - panel.clientWidth,
        dialogOverflow: dialog.scrollWidth - dialog.clientWidth,
        resultScrolls: result.scrollWidth > result.clientWidth,
      };
    });
    expect(layout.panelOverflow, JSON.stringify(layout)).toBeLessThanOrEqual(1);
    expect(layout.dialogOverflow, JSON.stringify(layout)).toBeLessThanOrEqual(1);
    expect(layout.resultScrolls, "la tabla ancha se desplaza dentro de su recuadro").toBe(true);

    await close.click();
    await expect(drawer).toHaveCount(0);

    // The same query inside Revisar › Más análisis widened the whole page.
    await page.getByText("Más análisis y herramientas").click();
    await page.locator("summary").getByText("Consulta SQL", { exact: true }).click();
    await page.getByRole("button", { name: "Ejecutar consulta" }).click();
    await expect(page.getByRole("region", { name: "Resultado de consulta SQL" })).toBeVisible();
    const pageOverflow = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
    expect(pageOverflow).toBeLessThanOrEqual(1);
  });
});
