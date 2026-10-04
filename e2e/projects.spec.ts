import { expect, test, type Page } from "@playwright/test";

import type { DatasetPreview, ProjectSummary } from "../src/bridge";
import { installTauriMock, recordedCalls } from "./support/tauri-mock";

const dataset = {
  fileName: "ventas.csv",
  fileSizeBytes: 128,
  rowCount: 2,
  columnCount: 2,
  columns: [
    { name: "cliente", dataType: "String" },
    { name: "total", dataType: "Float64" },
  ],
  rows: [["Ana", "10"], ["Luis", "20"]],
} satisfies DatasetPreview;
const project = {
  id: "project-e2e",
  name: "Ventas E2E",
  datasetFileName: dataset.fileName,
  rowCount: dataset.rowCount,
  columnCount: dataset.columnCount,
  createdAt: "2026-01-01T00:00:00.000Z",
  updatedAt: "2026-01-01T00:00:00.000Z",
} satisfies ProjectSummary;

async function installTauriProjectMock(page: Page, seedRecoveryCandidate = false) {
  await installTauriMock(page, {
    dataset,
    project: { summary: project, recover: seedRecoveryCandidate, workspace: seedRecoveryCandidate ? { activePhase: "prepare" } : {} },
  });
}

async function selectAndConfirmDataset(page: Page) {
  await page.getByRole("button", { name: "Seleccionar dataset" }).click();
  const headerReview = page.getByRole("dialog", { name: "Revisar encabezados de ventas.csv" });
  const loadButton = headerReview.getByRole("button", { name: "Cargar archivo" });
  await expect(loadButton).toBeEnabled();
  await loadButton.click();
}

test("recorre guardar, abrir y eliminar un proyecto desde el shell Tauri simulado", async ({ page }) => {
  await installTauriProjectMock(page);
  await page.goto("/", { waitUntil: "commit" });

  await expect(page.getByRole("button", { name: "Seleccionar dataset" })).toBeVisible();
  await selectAndConfirmDataset(page);
  const workflow = page.getByRole("navigation", { name: "Flujo de preparación de datos" });
  await expect(workflow.getByRole("button", { name: "Revisar", exact: true })).toHaveAttribute("aria-current", "step");

  await page
    .getByRole("navigation", { name: "Flujo de preparación de datos" })
    .getByRole("button", { name: "Cargar", exact: true })
    .click();
  await page.locator(".load-secondary").filter({ hasText: "Continuar un proyecto" }).locator(":scope > summary").click();
  await expect(page.getByRole("heading", { name: "Proyectos" })).toBeVisible();
  await page.getByText("Guardar y administrar proyectos", { exact: true }).click();
  await page.getByLabel("Nombre del proyecto").fill("Ventas E2E");
  await page.getByRole("button", { name: "Guardar proyecto nuevo" }).click();
  await expect(page.locator("p.notice--success")).toContainText("Proyecto “Ventas E2E” guardado.");
  await expect(page.getByRole("list", { name: "Proyectos guardados" })).toContainText("Ventas E2E");

  await page.getByRole("button", { name: "Abrir" }).click();
  await expect(workflow.getByRole("button", { name: "Revisar", exact: true })).toHaveAttribute("aria-current", "step");
  await expect(page.getByRole("heading", { name: "Revisa antes de modificar" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "ventas.csv" })).toBeVisible();

  await workflow.getByRole("button", { name: "Cargar", exact: true }).click();
  await page.locator(".load-secondary").filter({ hasText: "Continuar un proyecto" }).locator(":scope > summary").click();
  await page.getByText("Guardar y administrar proyectos", { exact: true }).click();
  await expect(page.getByRole("list", { name: "Proyectos guardados" })).toContainText("Ventas E2E · activo");
  await page.getByRole("button", { name: "Eliminar" }).click();
  await expect(page.getByRole("alertdialog", { name: "Eliminar “Ventas E2E”" })).toBeVisible();
  await page.getByRole("button", { name: "Eliminar proyecto" }).click();
  await expect(page.locator("p.notice--success")).toContainText("Proyecto “Ventas E2E” eliminado.");
  await expect(page.getByText("Todavía no hay proyectos guardados.")).toBeVisible();
});

test("recupera la última sesión, restaura su etapa y actualiza el mismo proyecto", async ({ page }) => {
  await installTauriProjectMock(page, true);
  await page.goto("/", { waitUntil: "commit" });

  const workflow = page.getByRole("navigation", { name: "Flujo de preparación de datos" });
  const projectDetails = page.locator(".load-secondary")
    .filter({ hasText: "Continuar un proyecto" });
  await projectDetails.locator(":scope > summary").click();
  await expect(page.getByRole("button", { name: "Recuperar proyecto" })).toBeVisible();
  await page.getByRole("button", { name: "Recuperar proyecto" }).click();

  await expect(workflow.getByRole("button", { name: "Preparar", exact: true }))
    .toHaveAttribute("aria-current", "step");
  await expect(page.getByRole("heading", { name: "Prepara datos consistentes" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "ventas.csv" })).toBeVisible();

  await workflow.getByRole("button", { name: "Cargar", exact: true }).click();
  await projectDetails.locator(":scope > summary").click();
  await page.getByText("Guardar y administrar proyectos", { exact: true }).click();
  const projects = page.getByRole("list", { name: "Proyectos guardados" });
  await expect(projects).toContainText("Ventas E2E · activo");
  const projectName = page.getByLabel("Nombre del proyecto");
  await expect(projectName).toHaveValue("Ventas E2E");
  await projectName.fill("Ventas recuperadas");
  await page.getByRole("button", { name: "Actualizar proyecto" }).click();

  await expect(page.locator("p.notice--success"))
    .toContainText("Proyecto “Ventas recuperadas” actualizado.");
  await expect(projects).toContainText("Ventas recuperadas · activo");
  const saves = (await recordedCalls(page)).filter((call) => call.command === "save_project");
  expect(saves.at(-1)?.args).toMatchObject({ projectId: project.id, name: "Ventas recuperadas" });
});
