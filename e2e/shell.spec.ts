import { expect, test } from "@playwright/test";

test.describe("shell web de Columnia", () => {
  test("expone landmarks, estado del runtime y navegación accesible", async ({ page }) => {
    await page.goto("/", { waitUntil: "commit" });

    await expect(page).toHaveTitle("Columnia");
    await expect(page.locator("#app-title")).toHaveText("Columnia");
    await expect(page.getByRole("link", { name: "Saltar al contenido principal" }))
      .toHaveAttribute("href", "#main-content");
    await expect(page.locator("#main-content")).toBeVisible();
    await expect(page.getByRole("complementary", { name: "Navegación principal" })).toBeVisible();
    const workflow = page.getByRole("navigation", { name: "Flujo de preparación de datos" });
    await expect(workflow).toBeVisible();
    await expect(workflow.getByRole("button", { name: "Cargar", exact: true })).toHaveAttribute("aria-current", "step");
    await expect(workflow.getByRole("button", { name: "Revisar", exact: true })).toHaveAttribute("aria-disabled", "true");
    await expect(workflow.getByRole("button", { name: "Preparar", exact: true })).toHaveAttribute("aria-disabled", "true");
    await expect(workflow.getByRole("button", { name: "Entregar", exact: true })).toHaveAttribute("aria-disabled", "true");
    await expect(page.getByText("Vista web · motor no conectado")).toBeVisible();
    await expect(page.getByRole("button", { name: "Seleccionar dataset" })).toBeVisible();
  });

  test("permite saltar al contenido con el teclado", async ({ page }) => {
    await page.goto("/", { waitUntil: "commit" });
    await expect(page.locator("#app-title")).toBeVisible();

    await page.keyboard.press("Tab");
    const skipLink = page.getByRole("link", { name: "Saltar al contenido principal" });
    await expect(skipLink).toBeFocused();

    await page.keyboard.press("Enter");
    await expect(page.locator("#main-content")).toBeFocused();
  });
});
