import { expect, test } from "@playwright/test";

test("marca el primer render del shell dentro del presupuesto", async ({ page }) => {
  await page.goto("/", { waitUntil: "commit" });
  await expect(page.locator("#app-title")).toBeVisible();

  const firstRender = await page.evaluate(() => {
    const mark = performance.getEntriesByName("columnia:app-render", "mark")[0];
    return mark?.startTime ?? null;
  });

  expect(firstRender).not.toBeNull();
  expect(firstRender).toBeLessThan(3_000);
  test.info().annotations.push({
    type: "performance",
    description: `first-render=${Math.round(firstRender ?? 0)}ms`,
  });
});
