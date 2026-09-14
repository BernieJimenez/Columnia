import { expect, test, type Page } from "@playwright/test";

async function assertNoHorizontalOverflow(page: Page) {
  const dimensions = await page.evaluate(() => ({
    viewport: document.documentElement.clientWidth,
    document: document.documentElement.scrollWidth,
    body: document.body.scrollWidth,
  }));

  expect(Math.max(dimensions.document, dimensions.body), JSON.stringify(dimensions))
    .toBeLessThanOrEqual(dimensions.viewport + 1);
}

test.describe("shell informativo de espacios de trabajo", () => {
  test("muestra estados sin añadir controles ni landmarks de navegación", async ({ page }) => {
    await page.goto("/", { waitUntil: "commit" });

    const region = page.getByRole("region", { name: "Espacio actual" });

    await expect(region.getByText("Analizar")).toBeVisible();
    await expect(region).toContainText("En uso");
    await expect(region.getByRole("button")).toHaveCount(0);
    await expect(region.getByRole("link")).toHaveCount(0);
    await expect(page.getByRole("navigation", { name: "Espacios de trabajo" })).toHaveCount(0);
  });

  test("mantiene el orden de marca, dataset, espacios y flujo en la vista compacta", async ({ page }) => {
    await page.setViewportSize({ width: 800, height: 900 });
    await page.goto("/", { waitUntil: "commit" });
    await expect(page.getByRole("region", { name: "Espacio actual" })).toBeVisible();

    const layout = await page.evaluate(() => {
      const bounds = (selector: string) => {
        const element = document.querySelector(selector);
        if (!element) throw new Error(`Falta ${selector}`);
        const rect = element.getBoundingClientRect();
        return { top: rect.top, bottom: rect.bottom, left: rect.left };
      };
      return {
        brand: bounds(".brand"),
        workspaces: bounds(".workspace-list"),
        flow: bounds(".side-nav"),
      };
    });

    expect(layout.workspaces.top).toBeGreaterThan(layout.brand.bottom);
    expect(layout.flow.top).toBeGreaterThan(layout.workspaces.bottom);
  });

  test("refluye a 320 CSS px y conserva los estados accesibles al 200 %", async ({ page }) => {
    await page.setViewportSize({ width: 320, height: 844 });
    await page.goto("/", { waitUntil: "commit" });
    await assertNoHorizontalOverflow(page);

    const region = page.getByRole("region", { name: "Espacio actual" });
    await expect(region.getByText("Analizar")).toBeVisible();
    await expect(region).toContainText("En uso");

    await page.setViewportSize({ width: 1280, height: 900 });
    await page.emulateMedia({ forcedColors: "active", reducedMotion: "reduce" });
    await page.evaluate(() => {
      document.documentElement.style.zoom = "2";
      document.documentElement.dataset.columniaZoom = "2";
    });
    await assertNoHorizontalOverflow(page);
    await expect(region.getByText("Analizar")).toBeVisible();
    await expect(region).toContainText("En uso");
    expect(await page.locator(".sidebar__tools").evaluate((element) => getComputedStyle(element).gridArea))
      .toBe("utilities");
    expect(await page.evaluate(() => window.matchMedia("(forced-colors: active)").matches)).toBe(true);
    expect(await page.evaluate(() => window.matchMedia("(prefers-reduced-motion: reduce)").matches)).toBe(true);
  });

  test("mantiene las preferencias alcanzables en una ventana de poca altura", async ({ page }) => {
    await page.setViewportSize({ width: 1280, height: 720 });
    await page.goto("/", { waitUntil: "commit" });

    const preferences = page.locator(".sidebar__utilities");
    await preferences.locator("summary").click();
    await expect(preferences).toHaveAttribute("open", "");
    await expect(page.getByRole("group", { name: "Tema de la interfaz" })).toBeVisible();
  });
});
