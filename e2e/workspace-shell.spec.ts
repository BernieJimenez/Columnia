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

test.describe("shell de trabajo", () => {
  test("nombra el espacio y el dataset sin añadir controles ni landmarks de navegación", async ({ page }) => {
    await page.goto("/", { waitUntil: "commit" });

    const sidebar = page.getByRole("complementary", { name: "Navegación principal" });
    await expect(sidebar.locator(".workspace-label")).toContainText("Espacio de trabajo: Analizar");
    await expect(page.getByRole("navigation", { name: "Espacios de trabajo" })).toHaveCount(0);
    // The top bar always says which dataset the work applies to.
    await expect(page.locator(".dataset-bar__name")).toHaveText("Sin dataset");
  });

  // UX-02: in the compact view the brand and the flow form a band above the
  // top bar, so the work keeps most of the height.
  test("pone la marca antes del flujo y ambos antes de la barra superior en la vista compacta", async ({ page }) => {
    await page.setViewportSize({ width: 800, height: 900 });
    await page.goto("/", { waitUntil: "commit" });
    await expect(page.locator(".app-brand")).toBeVisible();

    const layout = await page.evaluate(() => {
      const bounds = (selector: string) => {
        const element = document.querySelector(selector);
        if (!element) throw new Error(`Falta ${selector}`);
        const rect = element.getBoundingClientRect();
        return { top: rect.top, bottom: rect.bottom, left: rect.left };
      };
      return {
        brand: bounds(".app-brand"),
        flow: bounds(".flow-nav__list"),
        topbar: bounds(".app-topbar"),
      };
    });

    expect(layout.flow.top).toBeGreaterThanOrEqual(layout.brand.bottom);
    expect(layout.topbar.top).toBeGreaterThanOrEqual(layout.flow.bottom);
  });

  test("la barra lateral deja al menos el 60 % de la altura en la ventana mínima (UX-02)", async ({ page }) => {
    for (const [width, height] of [[900, 640], [640, 400]]) {
      await page.setViewportSize({ width, height });
      await page.goto("/", { waitUntil: "commit" });
      await expect(page.locator(".app-brand")).toBeVisible();
      const sidebarHeight = await page.locator(".app-sidebar").evaluate((element) => element.getBoundingClientRect().height);
      expect(sidebarHeight / height, `${width}x${height}`).toBeLessThanOrEqual(0.4);
    }
  });

  test("refluye a 320 CSS px y conserva los estados accesibles al 200 %", async ({ page }) => {
    await page.setViewportSize({ width: 320, height: 844 });
    await page.goto("/", { waitUntil: "commit" });
    await assertNoHorizontalOverflow(page);

    const workflow = page.getByRole("navigation", { name: "Flujo de preparación de datos" });
    await expect(workflow.getByRole("button", { name: "Cargar", exact: true })).toBeVisible();
    await expect(workflow.getByRole("button", { name: "Cargar", exact: true })).toHaveAttribute("aria-current", "step");

    await page.setViewportSize({ width: 1280, height: 900 });
    await page.emulateMedia({ forcedColors: "active", reducedMotion: "reduce" });
    await page.evaluate(() => {
      document.documentElement.style.zoom = "2";
      document.documentElement.dataset.columniaZoom = "2";
    });
    await assertNoHorizontalOverflow(page);
    await expect(workflow.getByRole("button", { name: "Entregar", exact: true })).toBeVisible();
    await expect(page.locator(".dataset-bar__name")).toBeVisible();
    expect(await page.locator(".sidebar__tools").evaluate((element) => getComputedStyle(element).gridArea))
      .toBe("tools");
    expect(await page.evaluate(() => window.matchMedia("(forced-colors: active)").matches)).toBe(true);
    expect(await page.evaluate(() => window.matchMedia("(prefers-reduced-motion: reduce)").matches)).toBe(true);
  });

  // AUD-083: measured counterpart of the text guards in WorkspaceShellStyles.test.ts
  // and AccessibilityStyles.test.ts (max-width: 900px, zoom 200 %, floating panels).
  test("bajo 900 px la barra lateral es una franja sobre el trabajo y sus paneles no flotan (UX-02)", async ({ page }) => {
    const measure = () => page.evaluate(() => {
      const sidebar = document.querySelector(".app-sidebar")!.getBoundingClientRect();
      const main = document.querySelector("main")!.getBoundingClientRect();
      return {
        position: getComputedStyle(document.querySelector(".app-sidebar")!).position,
        sidebarBottom: sidebar.bottom,
        mainTop: main.top,
        sidebarRight: sidebar.right,
        mainLeft: main.left,
        panelPosition: getComputedStyle(document.querySelector(".sidebar__utilities-content")!).position,
      };
    });

    await page.setViewportSize({ width: 900, height: 900 });
    await page.goto("/", { waitUntil: "commit" });
    const flow = page.getByRole("navigation", { name: "Flujo de preparación de datos" });
    await expect(flow.getByRole("button", { name: "Entregar", exact: true })).toBeVisible();
    const compact = await measure();
    expect(compact.position, JSON.stringify(compact)).toBe("static");
    expect(compact.sidebarBottom, JSON.stringify(compact)).toBeLessThanOrEqual(compact.mainTop + 1);
    expect(compact.panelPosition, JSON.stringify(compact)).not.toBe("fixed");
    await assertNoHorizontalOverflow(page);

    await page.setViewportSize({ width: 901, height: 900 });
    await expect.poll(async () => (await measure()).panelPosition).toBe("fixed");
    const wide = await measure();
    expect(wide.position, JSON.stringify(wide)).not.toBe("static");
    expect(wide.sidebarRight, JSON.stringify(wide)).toBeLessThanOrEqual(wide.mainLeft + 1);
    await assertNoHorizontalOverflow(page);
  });

  test("al 200 % la barra lateral no ocupa toda la altura y las etapas siguen visibles (UX-02)", async ({ page }) => {
    await page.setViewportSize({ width: 1280, height: 900 });
    await page.goto("/", { waitUntil: "commit" });
    await page.evaluate(() => {
      document.documentElement.style.zoom = "2";
      document.documentElement.dataset.columniaZoom = "2";
    });
    const flow = page.getByRole("navigation", { name: "Flujo de preparación de datos" });
    for (const stage of ["Cargar", "Revisar", "Preparar", "Explorar", "Entregar"]) {
      await expect(flow.getByRole("button", { name: stage, exact: true })).toBeVisible();
    }
    // zoom scales CSS px: the sidebar must be a band, not the full-height rail.
    const metrics = await page.evaluate(() => {
      const sidebar = document.querySelector(".app-sidebar")!.getBoundingClientRect();
      const main = document.querySelector("main")!.getBoundingClientRect();
      return { sidebarHeight: sidebar.height, viewportHeight: window.innerHeight, sidebarBottom: sidebar.bottom, mainTop: main.top };
    });
    expect(metrics.sidebarHeight, JSON.stringify(metrics)).toBeLessThan(metrics.viewportHeight * 0.6);
    expect(metrics.sidebarBottom, JSON.stringify(metrics)).toBeLessThanOrEqual(metrics.mainTop + 1);
    await assertNoHorizontalOverflow(page);
  });

  // QA-32: the size comes from the cascade the browser applies, not from a
  // pattern in the stylesheet text.
  test("los botones de añadir de la receta miden al menos 44 px de alto", async ({ page }) => {
    await page.goto("/", { waitUntil: "commit" });
    await expect(page.locator(".workspace")).toBeVisible();
    const height = await page.evaluate(() => {
      const button = document.createElement("button");
      button.type = "button";
      button.className = "recipe-add";
      button.textContent = "+ Añadir renombre";
      document.querySelector(".workspace")?.append(button);
      const measured = button.getBoundingClientRect().height;
      button.remove();
      return measured;
    });
    expect(height).toBeGreaterThanOrEqual(44);
  });

  test("en tema oscuro un aviso no se pinta con el verde de éxito (UI-01)", async ({ page }) => {
    await page.addInitScript(() => localStorage.setItem("columnia.theme", "dark"));
    await page.goto("/", { waitUntil: "commit" });
    await expect(page.locator(".workspace")).toBeVisible();
    const [warning, success] = await page.evaluate(() => ["notice", "notice notice--success"].map((className) => {
      const notice = document.createElement("p");
      notice.className = className;
      notice.textContent = "Aviso";
      document.querySelector(".workspace")?.append(notice);
      const background = getComputedStyle(notice).backgroundColor;
      notice.remove();
      return background;
    }));
    expect(warning).not.toBe(success);
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
