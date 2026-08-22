import { expect, test, type Page } from "@playwright/test";

const viewportCases = [
  { name: "desktop", viewport: { width: 1280, height: 900 } },
  { name: "móvil", viewport: { width: 390, height: 844 } },
] as const;

async function assertResponsiveShell(page: Page) {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.goto("/", { waitUntil: "commit" });

  await expect(page).toHaveTitle("Columnia");
  await expect(page.locator("#app-title")).toHaveText("Columnia");
  await expect(page.getByRole("complementary", { name: "Navegación principal" })).toHaveCount(1);
  await expect(page.getByRole("navigation", { name: "Flujo de preparación de datos" })).toHaveCount(1);
  await expect(page.getByRole("main")).toHaveAttribute("id", "main-content");
  await expect(page.getByRole("region", { name: "Etapa Cargar" })).toHaveCount(1);

  await expect(page.getByRole("link", { name: "Saltar al contenido principal" }))
    .toHaveAttribute("href", "#main-content");
  expect(await page.evaluate(() =>
    window.matchMedia("(prefers-reduced-motion: reduce)").matches,
  )).toBe(true);

  const viewport = await page.evaluate(() => {
    const width = window.innerWidth;
    const offenders = Array.from(document.querySelectorAll("*"))
      .map((element) => {
        const bounds = element.getBoundingClientRect();
        return {
          tag: element.tagName,
          className: typeof element.className === "string" ? element.className : "",
          right: Math.round(bounds.right),
          width: Math.round(bounds.width),
        };
      })
      .filter((element) => element.right > width + 1)
      .slice(0, 5);
    return {
      width,
      documentWidth: document.documentElement.scrollWidth,
      bodyWidth: document.body.scrollWidth,
      offenders,
    };
  });
  expect(Math.max(viewport.documentWidth, viewport.bodyWidth), JSON.stringify(viewport)).toBeLessThanOrEqual(viewport.width);

  const targets = await page.locator("button:not(:disabled), a[href]").evaluateAll((elements) =>
    elements.map((element) => {
      const bounds = element.getBoundingClientRect();
      return {
        name: element.textContent?.replace(/\s+/g, " ").trim() || element.getAttribute("aria-label"),
        width: bounds.width,
        height: bounds.height,
        left: bounds.left,
        right: bounds.right,
        viewportWidth: window.innerWidth,
      };
    }),
  );
  expect(targets.length).toBeGreaterThan(0);
  for (const target of targets) {
    expect(target.width, `${target.name ?? "target"} width`).toBeGreaterThanOrEqual(24);
    expect(target.height, `${target.name ?? "target"} height`).toBeGreaterThanOrEqual(24);
    expect(target.left, `${target.name ?? "target"} left`).toBeGreaterThanOrEqual(-1);
    expect(target.right, `${target.name ?? "target"} right`).toBeLessThanOrEqual(target.viewportWidth + 1);
  }

  const skipLink = page.getByRole("link", { name: "Saltar al contenido principal" });
  await page.keyboard.press("Tab");
  await expect(skipLink).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(page.getByRole("main")).toBeFocused();
}

for (const viewportCase of viewportCases) {
  test.describe(`preferencias del shell en ${viewportCase.name}`, () => {
    test.use({ viewport: viewportCase.viewport, reducedMotion: "reduce" });

    test("conserva landmarks, foco y targets sin overflow horizontal", async ({ page }) => {
      await assertResponsiveShell(page);
    });
  });
}
