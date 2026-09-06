import { expect, test } from "@playwright/test";

test("abre preferencias con un clic y conserva el tema del sistema", async ({ page }) => {
  await page.emulateMedia({ colorScheme: "dark", reducedMotion: "reduce" });
  await page.goto("/");
  const panel = page.locator(".sidebar__utilities");
  const trigger = panel.locator("summary");
  await trigger.click();
  await expect(panel).toHaveAttribute("open", "");
  await expect(page.getByRole("group", { name: "Tema de la interfaz" })).toBeVisible();

  const surface = () => page.locator(".workspace").evaluate((element) => getComputedStyle(element).backgroundColor);
  await page.getByRole("button", { name: "Oscuro", exact: true }).click();
  const dark = await surface();
  await page.getByRole("button", { name: "Claro", exact: true }).click();
  const light = await surface();
  expect(light).not.toBe(dark);
  await page.getByRole("button", { name: "Sistema", exact: true }).click();
  expect(await surface()).toBe(dark);
  await page.emulateMedia({ colorScheme: "light" });
  expect(await surface()).toBe(light);

  await trigger.focus();
  await page.keyboard.press("Enter");
  await expect(panel).not.toHaveAttribute("open");
  await page.keyboard.press("Space");
  await expect(panel).toHaveAttribute("open", "");
});
