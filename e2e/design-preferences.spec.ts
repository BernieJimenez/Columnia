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
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  await expect.poll(surface).toBe("rgb(27, 38, 42)");

  await page.getByRole("button", { name: "Claro", exact: true }).click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
  await expect.poll(surface).toBe("rgb(255, 255, 255)");

  await page.getByRole("button", { name: "Sistema", exact: true }).click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "system");
  await expect.poll(surface).toBe("rgb(27, 38, 42)");

  await page.emulateMedia({ colorScheme: "light" });
  await expect.poll(surface).toBe("rgb(255, 255, 255)");

  await trigger.focus();
  await page.keyboard.press("Enter");
  await expect(panel).not.toHaveAttribute("open");
  await page.keyboard.press("Space");
  await expect(panel).toHaveAttribute("open", "");
});
