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
});
