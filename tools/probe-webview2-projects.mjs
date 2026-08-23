import { chromium } from "@playwright/test";

const portArgumentIndex = process.argv.indexOf("--port");
const port = portArgumentIndex >= 0 ? Number(process.argv[portArgumentIndex + 1]) : 9222;
const runMutations = process.argv.includes("--mutate");
const sustainedRunsArgumentIndex = process.argv.indexOf("--sustained-runs");
const sustainedRuns = sustainedRunsArgumentIndex >= 0
  ? Number(process.argv[sustainedRunsArgumentIndex + 1])
  : 3;
const restartMode = process.argv.includes("--restart-prepare")
  ? "prepare"
  : process.argv.includes("--restart-verify")
    ? "verify"
    : "normal";
const probeTimeoutMs = 15_000;
const pollIntervalMs = 250;

if (!Number.isInteger(port) || port < 1024 || port > 65535
  || !Number.isInteger(sustainedRuns) || sustainedRuns < 1 || sustainedRuns > 5) {
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
    return await page.evaluate(async ({ shouldMutate, restartMode: currentRestartMode, nativeSustainedRuns }) => {
      const internals = window.__TAURI_INTERNALS__;
      if (!internals || typeof internals.invoke !== "function") {
        return {
          status: "failed",
          phase: "tauri_ipc_unavailable",
          error: "invoke_unavailable",
        };
      }

      const nativeOperationStartedAt = performance.now();
      const operationTimingsMs = {};
      const invoke = async (command, args) => {
        const startedAt = performance.now();
        try {
          return await internals.invoke(command, args);
        } finally {
          const elapsedMs = Number((performance.now() - startedAt).toFixed(2));
          operationTimingsMs[command] = [...(operationTimingsMs[command] ?? []), elapsedMs];
        }
      };
      const timingEvidence = () => ({
        nativeOperationCount: Object.values(operationTimingsMs)
          .reduce((count, samples) => count + samples.length, 0),
        nativeOperationDurationMs: Number((performance.now() - nativeOperationStartedAt).toFixed(2)),
        nativeOperationTimingsMs: operationTimingsMs,
      });

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
      const readCatalog = async () => {
        const [projects, recoveryCandidate] = await Promise.all([
          invoke("list_projects"),
          invoke("get_recovery_candidate"),
        ]);
        const projectsArray = Array.isArray(projects);
        const recoveryValid = recoveryCandidate === null || isSummary(recoveryCandidate);
        const forbiddenPathFields = projectsArray
          && (projects.some((project) => forbiddenFields(project).length > 0)
            || forbiddenFields(recoveryCandidate).length > 0);
        return {
          valid: projectsArray && recoveryValid && projects.every(isSummary) && !forbiddenPathFields,
          projects,
          recoveryCandidate,
          projectsCount: projectsArray ? projects.length : null,
          recoveryPresent: recoveryCandidate !== null,
          projectSummariesValid: projectsArray && projects.every(isSummary),
          recoverySummaryValid: recoveryValid,
          forbiddenPathFields,
        };
      };

      try {
        const before = await readCatalog();
        if (!before.valid) {
          return {
            status: "failed",
            phase: "native_project_ipc_catalog_invalid",
            commands: ["list_projects", "get_recovery_candidate"],
            projectsCount: before.projectsCount,
            recoveryPresent: before.recoveryPresent,
            projectSummariesValid: before.projectSummariesValid,
            recoverySummaryValid: before.recoverySummaryValid,
            forbiddenPathFields: before.forbiddenPathFields,
            mutationRequested: shouldMutate,
            interactions: [],
            ...timingEvidence(),
          };
        }

        if (!shouldMutate) {
          return {
            status: "passed",
            phase: "native_project_ipc_read_only",
            commands: ["list_projects", "get_recovery_candidate"],
            projectsCount: before.projectsCount,
            recoveryPresent: before.recoveryPresent,
            projectSummariesValid: before.projectSummariesValid,
            recoverySummaryValid: before.recoverySummaryValid,
            forbiddenPathFields: before.forbiddenPathFields,
            mutationRequested: false,
            interactions: [],
            ...timingEvidence(),
          };
        }

        let projectId = null;
        let cleanupConfirmed = true;
        const projectName = `__columnia_native_probe__${crypto.randomUUID().slice(0, 8)}`;
        const recipe = {
          renames: [{ from: "value", to: "label" }],
          casts: [],
          dateParses: [],
          filters: [],
          calculatedColumn: null,
          findReplace: null,
          keepColumns: null,
          splitColumn: null,
          mergeColumns: null,
          outlierTreatments: [],
          groupSummary: null,
          contactNormalizations: [],
          textExtractions: [],
        };
        const qualityRule = { column: "label", kind: "non_empty", maxInvalid: 0 };
        const normalInteractions = [
          "probe_seed_dataset",
          "probe_save_transform_recipe",
          "apply_transform_recipe",
          "probe_export_dataset",
          "probe_reopen_project",
          "save_project",
          "list_projects",
          "open_project",
          "get_dataset_page",
          "delete_project",
        ];
        const prepareInteractions = [
          "probe_seed_dataset",
          "probe_save_transform_recipe",
          "apply_transform_recipe",
          "probe_export_dataset",
          "save_project",
          "list_projects",
        ];
        const verifyInteractions = [
          "get_recovery_candidate",
          "probe_reopen_project",
          "open_project",
          "get_dataset_page",
          "delete_project",
        ];
        const interactions = currentRestartMode === "prepare"
          ? prepareInteractions
          : currentRestartMode === "verify"
            ? verifyInteractions
            : normalInteractions;
        try {
          if (currentRestartMode === "verify") {
            const recovery = await invoke("get_recovery_candidate");
            if (!isSummary(recovery) || !recovery.name.startsWith("__columnia_native_probe__")) {
              throw new Error("recovery_invalid");
            }
            projectId = recovery.id;

            const reopened = await invoke("probe_reopen_project", { projectId });
            const reopenedValid = Boolean(reopened)
              && isSummary(reopened.project)
              && reopened.project.id === projectId
              && reopened.datasetFileName === "native-probe.csv"
              && reopened.rowCount === 2
              && reopened.columnCount === 2
              && reopened.qualityRuleCount === 1
              && reopened.recipeDraftPresent === true
              && reopened.recoveryCandidatePresent === true
              && forbiddenFields(reopened).length === 0;
            if (!reopenedValid) throw new Error("reopen_after_restart_invalid");

            const opened = await invoke("open_project", { projectId });
            const openedValid = Boolean(opened)
              && isSummary(opened.project)
              && opened.project.id === projectId
              && opened.dataset?.fileName === "native-probe.csv"
              && opened.dataset?.rowCount === 2
              && opened.dataset?.columnCount === 2
              && opened.workspace?.qualityRules?.length === 1
              && opened.workspace.qualityRules[0]?.column === "label"
              && opened.workspace.recipeDraft?.name === "Native probe recipe"
              && forbiddenFields(opened).length === 0;
            if (!openedValid) throw new Error("open_after_restart_invalid");

            const page = await invoke("get_dataset_page", { offset: 0, limit: 10 });
            if (!(page?.offset === 0 && Array.isArray(page.rows) && page.rows.length === 2)) {
              throw new Error("page_after_restart_invalid");
            }

            const deletedProjectId = projectId;
            await invoke("delete_project", { projectId });
            projectId = null;
            const after = await readCatalog();
            if (!(after.valid
              && after.projectsCount === before.projectsCount - 1
              && !after.projects.some((project) => project.id === deletedProjectId))) {
              throw new Error("restart_cleanup_invalid");
            }
            return {
              status: "passed",
              phase: "native_project_restart_verify",
              commands: interactions,
              projectsCountBefore: before.projectsCount,
              projectsCountAfter: after.projectsCount,
              recoveryPresentBefore: before.recoveryPresent,
              recoveryPresentAfter: after.recoveryPresent,
              projectSummariesValid: after.projectSummariesValid,
              recoverySummaryValid: after.recoverySummaryValid,
              forbiddenPathFields: before.forbiddenPathFields || after.forbiddenPathFields,
              restartVerified: true,
              mutationRequested: true,
              cleanupConfirmed: true,
              interactions,
              ...timingEvidence(),
            };
          }

          const seed = await invoke("probe_seed_dataset");
          const seedValid = Boolean(seed)
            && seed.fileName === "native-probe.csv"
            && seed.rowCount === 2
            && seed.columnCount === 2
            && forbiddenFields(seed).length === 0;
          if (!seedValid) throw new Error("seed_invalid");

          const savedRecipe = await invoke("probe_save_transform_recipe", {
            recipe,
            name: "Native probe recipe",
          });
          const savedRecipeValid = savedRecipe?.version === 1
            && savedRecipe.name === "Native probe recipe"
            && typeof savedRecipe.savedAt === "string"
            && savedRecipe.recipe?.renames?.[0]?.from === "value"
            && savedRecipe.recipe?.renames?.[0]?.to === "label"
            && Object.keys(savedRecipe).every((key) => !/path|filepath|sourcepath/i.test(key));
          if (!savedRecipeValid) throw new Error("recipe_save_invalid");

          const transformed = await invoke("apply_transform_recipe", {
            recipe: savedRecipe.recipe,
          });
          const transformedValid = transformed?.changed === true
            && transformed.dataset?.rowCount === 2
            && transformed.dataset?.columnCount === 2
            && transformed.dataset?.columns?.map((column) => column.name).join(",") === "id,label";
          if (!transformedValid) throw new Error("recipe_apply_invalid");

          const sustainedTimings = [];
          for (let iteration = 1; iteration <= nativeSustainedRuns; iteration += 1) {
            if (iteration > 1) {
              const undone = await invoke("undo_last_change");
              const undoneValid = Boolean(undone)
                && undone.dataset?.fileName === "native-probe.csv"
                && undone.dataset?.rowCount === 2
                && undone.dataset?.columnCount === 2
                && undone.dataset?.columns?.map((column) => column.name).join(",") === "id,value";
              if (!undoneValid) throw new Error("sustained_undo_invalid");
            }

            const sustainedTransform = iteration === 1
              ? transformed
              : await invoke("apply_transform_recipe", { recipe: savedRecipe.recipe });
            const sustainedTransformValid = sustainedTransform?.changed === true
              && sustainedTransform.dataset?.rowCount === 2
              && sustainedTransform.dataset?.columnCount === 2
              && sustainedTransform.dataset?.columns?.map((column) => column.name).join(",") === "id,label";
            if (!sustainedTransformValid) throw new Error("sustained_transform_invalid");

            const sustainedExport = await invoke("probe_export_dataset", {
              format: "csv",
              qualityRules: [qualityRule],
              allowUnvalidated: false,
            });
            const sustainedExportValid = sustainedExport?.format === "CSV"
              && typeof sustainedExport.fileName === "string"
              && sustainedExport.fileName.endsWith(".csv")
              && Number.isInteger(sustainedExport.fileSizeBytes)
              && sustainedExport.fileSizeBytes > 0;
            if (!sustainedExportValid) throw new Error("sustained_export_invalid");

            const transformSamples = operationTimingsMs.apply_transform_recipe ?? [];
            const exportSamples = operationTimingsMs.probe_export_dataset ?? [];
            sustainedTimings.push({
              iteration,
              transformDurationMs: transformSamples.length > 0 ? transformSamples[transformSamples.length - 1] : null,
              exportDurationMs: exportSamples.length > 0 ? exportSamples[exportSamples.length - 1] : null,
            });
          }

          const saved = await invoke("save_project", {
            projectId: null,
            name: projectName,
            workspace: { qualityRules: [qualityRule], recipeDraft: savedRecipe },
          });
          projectId = saved?.id ?? null;
          const savedValid = isSummary(saved)
            && saved.id === projectId
            && saved.name === projectName
            && saved.datasetFileName === "native-probe.csv"
            && saved.rowCount === 2
            && saved.columnCount === 2;
          if (!savedValid) throw new Error("save_invalid");

          const nativeSustainedTransformDurations = sustainedTimings
            .map(({ transformDurationMs }) => transformDurationMs)
            .filter((duration) => Number.isFinite(duration));
          const nativeSustainedExportDurations = sustainedTimings
            .map(({ exportDurationMs }) => exportDurationMs)
            .filter((duration) => Number.isFinite(duration));

          if (currentRestartMode === "prepare") {
            const prepared = await readCatalog();
            const restartReady = prepared.valid
              && prepared.projectsCount === before.projectsCount + 1
              && prepared.recoveryPresent
              && prepared.projects.some((project) => project.id === projectId && isSummary(project));
            if (!restartReady) throw new Error("restart_prepare_invalid");
            return {
              status: "passed",
              phase: "native_project_restart_prepare",
              commands: interactions,
              projectsCountBefore: before.projectsCount,
              projectsCountAfter: prepared.projectsCount,
              recoveryPresentBefore: before.recoveryPresent,
              recoveryPresentAfter: prepared.recoveryPresent,
              projectSummariesValid: prepared.projectSummariesValid,
              recoverySummaryValid: prepared.recoverySummaryValid,
              forbiddenPathFields: before.forbiddenPathFields || prepared.forbiddenPathFields,
              restartReady: true,
              projectPersisted: true,
              mutationRequested: true,
              nativeSustainedRuns,
              nativeSustainedTransformMaxMs: nativeSustainedTransformDurations.length > 0
                ? Math.max(...nativeSustainedTransformDurations)
                : null,
              nativeSustainedExportMaxMs: nativeSustainedExportDurations.length > 0
                ? Math.max(...nativeSustainedExportDurations)
                : null,
              cleanupConfirmed: true,
              interactions,
              ...timingEvidence(),
            };
          }

          const reopened = await invoke("probe_reopen_project", { projectId });
          const reopenedValid = Boolean(reopened)
            && isSummary(reopened.project)
            && reopened.project.id === projectId
            && reopened.datasetFileName === "native-probe.csv"
            && reopened.rowCount === 2
            && reopened.columnCount === 2
            && reopened.qualityRuleCount === 1
            && reopened.recipeDraftPresent === true
            && reopened.recoveryCandidatePresent === true
            && forbiddenFields(reopened).length === 0;
          if (!reopenedValid) throw new Error("reopen_invalid");

          const listed = await invoke("list_projects");
          const listedValid = Array.isArray(listed)
            && listed.length === before.projectsCount + 1
            && listed.some((project) => project.id === projectId && isSummary(project));
          if (!listedValid) throw new Error("list_invalid");

          const opened = await invoke("open_project", { projectId });
          const openedValid = Boolean(opened)
            && isSummary(opened.project)
            && opened.project.id === projectId
            && opened.dataset?.fileName === "native-probe.csv"
            && opened.dataset?.rowCount === 2
            && opened.dataset?.columnCount === 2
            && Array.isArray(opened.workspace?.qualityRules)
            && opened.workspace.qualityRules.length === 1
            && opened.workspace.qualityRules[0]?.column === "label"
            && opened.workspace.recipeDraft?.name === "Native probe recipe"
            && opened.workspace.recipeDraft?.version === 1
            && forbiddenFields(opened).length === 0;
          if (!openedValid) throw new Error("open_invalid");

          const page = await invoke("get_dataset_page", { offset: 0, limit: 10 });
          const pageValid = page?.offset === 0 && Array.isArray(page.rows) && page.rows.length === 2;
          if (!pageValid) throw new Error("page_invalid");

          await invoke("delete_project", { projectId });
          projectId = null;
          const after = await readCatalog();
          const catalogRestored = after.valid && after.projectsCount === before.projectsCount;
          if (!catalogRestored) throw new Error("cleanup_invalid");
          return {
            status: "passed",
            phase: "native_project_ipc_mutation",
            commands: interactions,
            projectsCountBefore: before.projectsCount,
            projectsCountAfter: after.projectsCount,
            recoveryPresentBefore: before.recoveryPresent,
            recoveryPresentAfter: after.recoveryPresent,
            projectSummariesValid: after.projectSummariesValid,
            recoverySummaryValid: after.recoverySummaryValid,
            forbiddenPathFields: before.forbiddenPathFields || after.forbiddenPathFields,
            recipeSaved: true,
            recipeApplied: true,
            exportVerified: true,
            persistenceReopenVerified: true,
            mutationRequested: true,
            nativeSustainedRuns,
            nativeSustainedTransformMaxMs: nativeSustainedTransformDurations.length > 0
              ? Math.max(...nativeSustainedTransformDurations)
              : null,
            nativeSustainedExportMaxMs: nativeSustainedExportDurations.length > 0
              ? Math.max(...nativeSustainedExportDurations)
              : null,
            cleanupConfirmed: true,
            interactions,
            ...timingEvidence(),
          };
        } catch (error) {
          if (projectId) {
            try {
              await invoke("delete_project", { projectId });
            } catch {
              cleanupConfirmed = false;
            }
          }
          return {
            status: "failed",
            phase: currentRestartMode === "verify"
              ? "native_project_restart_verify_failed"
              : currentRestartMode === "prepare"
                ? "native_project_restart_prepare_failed"
                : "native_project_ipc_mutation_failed",
            commands: interactions,
            mutationRequested: true,
            cleanupConfirmed,
            errorCode: error instanceof Error && /^(recovery|reopen|open|page|restart|seed|recipe|export|sustained|save|list|cleanup)_/.test(error.message)
              ? error.message
              : "invoke_failed",
            interactions,
            ...timingEvidence(),
          };
        }
      } catch {
        return {
          status: "failed",
          phase: "native_project_ipc_error",
          error: "invoke_failed",
          mutationRequested: shouldMutate,
          interactions: [],
          ...timingEvidence(),
        };
      }
    }, { shouldMutate: runMutations, restartMode, nativeSustainedRuns: sustainedRuns });
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
  const nativeCommands = restartMode === "prepare"
    ? [
      "probe_seed_dataset",
      "probe_save_transform_recipe",
      "apply_transform_recipe",
      "probe_export_dataset",
      "save_project",
      "list_projects",
    ]
    : restartMode === "verify"
      ? ["get_recovery_candidate", "probe_reopen_project", "open_project", "get_dataset_page", "delete_project"]
      : [
        "probe_seed_dataset",
        "probe_save_transform_recipe",
        "apply_transform_recipe",
        "probe_export_dataset",
        "probe_reopen_project",
        "save_project",
        "list_projects",
        "open_project",
        "get_dataset_page",
        "delete_project",
      ];
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
      nativeIpc: runMutations ? nativeCommands : ["list_projects", "get_recovery_candidate"],
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
