import { chromium } from "@playwright/test";

const portArgumentIndex = process.argv.indexOf("--port");
const port = portArgumentIndex >= 0 ? Number(process.argv[portArgumentIndex + 1]) : 9222;
const probeTimeoutMs = 15_000;
const pollIntervalMs = 250;

if (!Number.isInteger(port) || port < 1024 || port > 65535) {
  console.error(JSON.stringify({ status: "failed", error: "Invalid --port." }));
  process.exit(1);
}

const endpoint = `http://127.0.0.1:${port}`;
let browser;

function sleep(milliseconds) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

function isProvisionalUrl(url) {
  return /^about:blank(?:#.*)?$/i.test(url);
}

async function inspectNativeProjectIpc(page) {
  try {
    return await page.evaluate(async () => {
      const internals = window.__TAURI_INTERNALS__;
      if (!internals || typeof internals.invoke !== "function") {
        return {
          status: "failed",
          phase: "tauri_ipc_unavailable",
          error: "invoke_unavailable",
        };
      }

      try {
        const [projects, recoveryCandidate] = await Promise.all([
          internals.invoke("list_projects"),
          internals.invoke("get_recovery_candidate"),
        ]);
        const forbiddenFields = (value) => {
          if (!value || typeof value !== "object" || Array.isArray(value)) return [];
          return Object.keys(value).filter((key) => /path|filepath|sourcepath/i.test(key));
        };
        const isSummary = (value) => {
          if (!value || typeof value !== "object" || Array.isArray(value)) return false;
          const required = [
            "id",
            "name",
            "datasetFileName",
            "rowCount",
            "columnCount",
            "createdAt",
            "updatedAt",
          ];
          return required.every((key) => Object.prototype.hasOwnProperty.call(value, key))
            && typeof value.id === "string"
            && typeof value.name === "string"
            && typeof value.datasetFileName === "string"
            && Number.isInteger(value.rowCount) && value.rowCount >= 0
            && Number.isInteger(value.columnCount) && value.columnCount >= 0
            && typeof value.createdAt === "string"
            && typeof value.updatedAt === "string"
            && forbiddenFields(value).length === 0;
        };
        const projectsArray = Array.isArray(projects);
        const recoveryValid = recoveryCandidate === null || isSummary(recoveryCandidate);
        const forbiddenPathFields = projectsArray
          && (projects.some((project) => forbiddenFields(project).length > 0)
            || forbiddenFields(recoveryCandidate).length > 0);
        return {
          status: projectsArray && recoveryValid && projects.every(isSummary) ? "passed" : "failed",
          phase: "native_project_ipc_read_only",
          commands: ["list_projects", "get_recovery_candidate"],
          projectsCount: projectsArray ? projects.length : null,
          recoveryPresent: recoveryCandidate !== null,
          projectSummariesValid: projectsArray && projects.every(isSummary),
          recoverySummaryValid: recoveryValid,
          forbiddenPathFields,
          interactions: [],
        };
      } catch {
        return {
          status: "failed",
          phase: "native_project_ipc_error",
          error: "invoke_failed",
          interactions: [],
        };
      }
    });
  } catch {
    return {
      status: "failed",
      phase: "native_project_ipc_evaluation_error",
      error: "evaluation_failed",
      interactions: [],
    };
  }
}

async function inspectPage(page) {
  const url = page.url();
  const provisional = isProvisionalUrl(url);

  try {
    const shell = await page.evaluate(() => ({
      appMounted: Boolean(document.querySelector("#app-title")),
      panelMounted: Boolean(document.querySelector("section.projects")),
      title: document.title,
      readyState: document.readyState,
    }));

    if (!shell.panelMounted) {
      return { url, provisional, ...shell, status: "not_ready", phase: "projects_panel_not_mounted" };
    }

    const panel = page.locator("section.projects").first();
    const panelVisible = await panel.isVisible();
    if (!panelVisible) {
      return {
        url,
        provisional,
        ...shell,
        status: "not_ready",
        phase: "projects_panel_not_visible",
      };
    }

    const region = page.getByRole("region", { name: "Proyectos" }).first();
    const heading = panel.getByRole("heading", { name: "Proyectos" }).first();
    const input = panel.getByRole("textbox", { name: "Nombre del proyecto" }).first();
    const saveButton = panel.getByRole("button", { name: "Guardar proyecto nuevo" }).first();
    const accessible = await panel.evaluate((element) => {
      const visible = (candidate) => {
        if (!(candidate instanceof HTMLElement)) return false;
        const style = getComputedStyle(candidate);
        const rect = candidate.getBoundingClientRect();
        return style.display !== "none" && style.visibility !== "hidden" && rect.width > 0 && rect.height > 0;
      };
      const accessibleName = (candidate) => {
        const ariaLabel = candidate.getAttribute("aria-label")?.trim();
        if (ariaLabel) return ariaLabel;
        const labelledBy = candidate.getAttribute("aria-labelledby");
        if (labelledBy) {
          const labelledText = labelledBy
            .split(/\s+/)
            .map((id) => document.getElementById(id)?.textContent ?? "")
            .join(" ")
            .replace(/\s+/g, " ")
            .trim();
          if (labelledText) return labelledText;
        }
        return (candidate.textContent ?? "").replace(/\s+/g, " ").trim();
      };
      const buttons = [...element.querySelectorAll("button")];
      const visibleRouteLinks = [...document.querySelectorAll("a[href]")]
        .filter(visible)
        .map((anchor) => ({
          href: anchor.getAttribute("href") ?? "",
          name: accessibleName(anchor),
        }))
        .filter(({ href }) => /^(?:https?:|\/|\.\.?\/)/i.test(href));
      const panelLinks = [...element.querySelectorAll("a[href]")]
        .filter(visible)
        .map((anchor) => ({ href: anchor.getAttribute("href") ?? "", name: accessibleName(anchor) }));

      return {
        headingId: element.querySelector("h3")?.id ?? null,
        labelledBy: element.getAttribute("aria-labelledby"),
        inputId: element.querySelector("input")?.id ?? null,
        inputLabel: element.querySelector("label[for='project-name']")?.textContent?.trim() ?? null,
        inputDisabled: element.querySelector("#project-name")?.hasAttribute("disabled") ?? false,
        datasetFileName: document.querySelector(".sidebar__dataset strong")?.textContent?.trim() ?? null,
        saveDisabled: element.querySelector("button[type='submit']")?.hasAttribute("disabled") ?? false,
        actionNames: buttons.map(accessibleName),
        unnamedActions: buttons.filter((button) => !accessibleName(button)).length,
        visibleRouteLinks,
        panelLinks,
      };
    });

    const checks = {
      headingRegion: {
        regionCount: await region.count(),
        regionVisible: (await region.count()) > 0 && await region.isVisible(),
        headingCount: await heading.count(),
        headingVisible: (await heading.count()) > 0 && await heading.isVisible(),
        labelledByMatchesHeading: accessible.labelledBy === accessible.headingId && accessible.headingId === "projects-title",
        valid: (await region.count()) > 0 && await region.isVisible()
          && (await heading.count()) > 0 && await heading.isVisible()
          && accessible.labelledBy === accessible.headingId && accessible.headingId === "projects-title",
      },
      inputLabel: {
        inputCount: await input.count(),
        inputVisible: (await input.count()) > 0 && await input.isVisible(),
        inputLabel: accessible.inputLabel,
        inputId: accessible.inputId,
        valid: accessible.inputId === "project-name" && accessible.inputLabel === "Nombre del proyecto",
      },
      saveDisabledWithoutDataset: {
        datasetFileName: accessible.datasetFileName,
        saveButtonCount: await saveButton.count(),
        saveDisabled: accessible.saveDisabled,
        valid: !accessible.datasetFileName || accessible.datasetFileName === "Sin dataset"
          ? accessible.saveDisabled
          : null,
      },
      noVisibleRoutes: {
        visibleRouteLinks: accessible.visibleRouteLinks,
        panelLinks: accessible.panelLinks,
        valid: accessible.visibleRouteLinks.length === 0 && accessible.panelLinks.length === 0,
      },
      actionNames: {
        names: accessible.actionNames,
        unnamedCount: accessible.unnamedActions,
        valid: accessible.actionNames.length > 0 && accessible.unnamedActions === 0,
      },
    };

    const nativeIpc = await inspectNativeProjectIpc(page);
    const valid = Object.values(checks).every((check) => check.valid === true)
      && nativeIpc.status === "passed";
    return {
      url,
      provisional,
      ...shell,
      status: valid ? "passed" : "failed",
      phase: valid ? "projects_panel_contract" : "projects_panel_contract_failed",
      checks,
      nativeIpc,
      interactions: [],
    };
  } catch (error) {
    return {
      url,
      provisional,
      status: "failed",
      phase: "inspection_error",
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

function snapshotResult(status, pages, extra = {}) {
  return {
    status,
    endpoint,
    contextCount: browser?.contexts().length ?? 0,
    pageCount: pages.length,
    pages,
    checks: {
      headingRegion: "section.projects[aria-labelledby=projects-title]",
      inputLabel: "label[for=project-name]",
      saveDisabledWithoutDataset: "button[type=submit]:disabled",
      noVisibleRoutes: "visible route anchors",
      actionNames: "all visible ProjectsPanel buttons",
      nativeIpc: ["list_projects", "get_recovery_candidate"],
    },
    interactions: [],
    ...extra,
  };
}

try {
  browser = await chromium.connectOverCDP(endpoint, { timeout: 5_000 });
  const deadline = Date.now() + probeTimeoutMs;
  let lastPages = [];
  let sawMountedApp = false;

  while (Date.now() < deadline) {
    const pages = browser.contexts().flatMap((context) => context.pages());
    const pageEvidence = [];
    for (const page of pages) {
      const evidence = await inspectPage(page);
      pageEvidence.push(evidence);
      sawMountedApp ||= evidence.appMounted === true;
      if (evidence.status === "passed") {
        console.log(JSON.stringify(snapshotResult("passed", pageEvidence)));
        process.exitCode = 0;
        throw new Error("__probe_complete__");
      }
      if (evidence.status === "failed") {
        console.log(JSON.stringify(snapshotResult("failed", pageEvidence, {
          error: "ProjectsPanel o su IPC nativo de solo lectura no cumple el contrato esperado.",
        })));
        process.exitCode = 1;
        throw new Error("__probe_complete__");
      }
    }
    lastPages = pageEvidence;
    await sleep(pollIntervalMs);
  }

  console.log(JSON.stringify(snapshotResult("not_ready", lastPages, {
    phase: sawMountedApp ? "projects_panel_not_mounted" : "app_not_mounted",
    error: "ProjectsPanel no montó dentro del timeout; no se realizaron interacciones.",
  })));
  process.exitCode = 0;
} catch (error) {
  if (error instanceof Error && error.message === "__probe_complete__") {
    // The structured result was already emitted above.
  } else {
    console.error(JSON.stringify({
      status: "failed",
      endpoint,
      interactions: [],
      error: error instanceof Error ? error.message : String(error),
    }));
    process.exitCode = 1;
  }
} finally {
  if (browser) {
    await browser.close();
  }
}
