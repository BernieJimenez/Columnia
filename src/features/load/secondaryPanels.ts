import { lazy } from "react";

// The panels of Cargar that only draw when their card is chosen. They load on
// demand, but the chunk is requested when the card is hovered or focused and
// again once Cargar has painted, so choosing one finds the module already here.
const loadDatabaseImportPanel = () => import("./DatabaseImportPanel");
const loadProjectsPanel = () => import("../projects/ProjectsPanel");

export const LazyDatabaseImportPanel = lazy(async () => {
  const module = await loadDatabaseImportPanel();
  return { default: module.DatabaseImportPanel };
});

export const LazyProjectsPanel = lazy(async () => {
  const module = await loadProjectsPanel();
  return { default: module.ProjectsPanel };
});

export function preloadSourcePanel(source: "file" | "database" | "sample" | "project"): void {
  if (source === "database") void loadDatabaseImportPanel();
  if (source === "project") void loadProjectsPanel();
}

/** Asks for both chunks when the browser is idle; returns what cancels the request. */
export function preloadSourcePanelsWhenIdle(): () => void {
  const request = () => {
    preloadSourcePanel("database");
    preloadSourcePanel("project");
  };
  if (typeof window.requestIdleCallback === "function") {
    const handle = window.requestIdleCallback(request, { timeout: 2_000 });
    return () => window.cancelIdleCallback(handle);
  }
  const handle = window.setTimeout(request, 300);
  return () => window.clearTimeout(handle);
}
