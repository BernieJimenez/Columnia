import assert from "node:assert/strict";
import { mkdir, mkdtemp, rm, symlink, writeFile } from "node:fs/promises";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";
import { dirname, join } from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";

import { resolvePhysicalValidationReport, resolveValidationReportPath, validateGate1Evidence } from "./check-beta-gate-evidence.mjs";

const projectRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const sessionTemplate = readFileSync(join(projectRoot, "docs", "templates", "beta-session.md"), "utf8");
const summaryTemplate = readFileSync(join(projectRoot, "docs", "templates", "beta-summary.md"), "utf8");
const commit = "a".repeat(40);
const version = "0.167.0";
const candidateId = "rc-0.167.0-aaaaaaaa-gate1";

function replaceOnce(source, before, after) {
  assert.ok(source.includes(before), `fixture source is missing: ${before}`);
  return source.replace(before, after);
}

function makeSession(index, datasetAliases = [`dataset-0${(index % 3) + 1}`, `dataset-0${((index + 1) % 3) + 1}`]) {
  let markdown = sessionTemplate;
  for (const [before, after] of [
    ["| Fecha | AAAA-MM-DD |", `| Fecha | 2026-09-12 |`],
    ["| Alias de sesión | beta-___ |", `| Alias de sesión | beta-0${index + 1} |`],
    ["| Alias anónimo de participante | participante-___ |", `| Alias anónimo de participante | participante-0${index + 1} |`],
    ["| Commit probado | ___ |", `| Commit probado | ${commit} |`],
    ["| Versión de Columnia | ___ |", `| Versión de Columnia | ${version} |`],
    ["| Ronda de medición | Gate 1 · baseline V1 / Gate 2 · shell de espacios |", "| Ronda de medición | Gate 1 · baseline V1 |"],
    ["| Release candidate | rc-___ |", `| Release candidate | ${candidateId} |`],
    ["| Windows / arquitectura | ___ |", "| Windows / arquitectura | Windows 11 / x64 |"],
    ["| Experiencia con datos | inicial / intermedia / avanzada |", "| Experiencia con datos | intermedia |"],
    ["| Ayuda permitida | solo desbloqueo / ninguna |", "| Ayuda permitida | ninguna |"],
    ["| Tabla real | dataset-___ |", `| Tabla real | ${datasetAliases[0]} |`],
    ["| Caso estructural | dataset-___ |", `| Caso estructural | ${datasetAliases[1]} |`],
    ["- Tareas sin ayuda: ___ / 10", "- Tareas sin ayuda: 8 / 10"],
    ["- Tareas con ayuda: ___ / 10", "- Tareas con ayuda: 2 / 10"],
    ["- Tareas no completadas: ___ / 10", "- Tareas no completadas: 0 / 10"],
    ["- Flujo principal Cargar → Revisar → Preparar → Entregar: sí / no", "- Flujo principal Cargar → Revisar → Preparar → Entregar: sí"],
    ["- Original sin cambios: sí / no", "- Original sin cambios: sí"],
    ["- Guardado y reapertura: aprobado / fallido", "- Guardado y reapertura: aprobado"],
    ["- Entrega verificada fuera de Columnia: aprobada / fallida", "- Entrega verificada fuera de Columnia: aprobada"],
    ["- Hallazgos: P0 ___ · P1 ___ · P2 ___ · P3 ___", "- Hallazgos: P0 0 · P1 0 · P2 0 · P3 0"],
    ["- Validez de la sesión: válida / no cuenta por preparación o entorno / invalidada por P0-P1", "- Validez de la sesión: válida"],
    ["- Resultado de la sesión: aprobada / requiere correcciones / detenida", "- Resultado de la sesión: aprobada"],
  ]) markdown = replaceOnce(markdown, before, after);

  const tasks = [
    "Cargar el dataset correcto",
    "Comprender filas, columnas y señales",
    "Revisar una señal útil",
    "Distinguir muestra de cobertura completa",
    "Aplicar, deshacer y rehacer un cambio",
    "Guardar, cerrar y reabrir el proyecto",
    "Crear y ejecutar una regla de calidad",
    "Exportar y verificar la entrega",
    "Confirmar que el original no cambió",
    "Recuperarse de un problema",
  ];
  const lines = markdown.split(/\r?\n/);
  let outcomeIndex = 0;
  for (let lineIndex = 0; lineIndex < lines.length; lineIndex += 1) {
    const task = tasks.find((label) => lines[lineIndex].startsWith(`| ${label} |`));
    if (!task) continue;
    const outcome = outcomeIndex < 8 ? "sin ayuda" : "con ayuda";
    lines[lineIndex] = `| ${task} | ${outcome} | 2 min | 1 | 0 | 0 | 0 | ninguna | sin incidentes |`;
    outcomeIndex += 1;
  }
  assert.equal(outcomeIndex, 10);
  return lines.join("\n");
}

function addP2Finding(markdown) {
  let session = replaceOnce(markdown, "### BETA-___ — Título sin datos sensibles", "### BETA-001 — Riesgo acotado de clasificación");
  for (const [before, after] of [
    ["- Severidad: P0 / P1 / P2 / P3", "- Severidad: P2"],
    ["- Tarea:", "- Tarea: Revisar una señal útil"],
    ["- Expectativa:", "- Expectativa: presentar el indicador correcto"],
    ["- Resultado observado:", "- Resultado observado: el indicador requirió aclaración"],
    ["- Reproducción sintética o pasos sanitizados:", "- Reproducción sintética o pasos sanitizados: dataset sintético con una categoría ambigua"],
    ["- Recuperación disponible:", "- Recuperación disponible: continuar con la señal alternativa"],
    ["- Decisión: corregir / aceptar / investigar", "- Decisión: aceptar con seguimiento"],
    ["- Responsable:", "- Responsable: producto"],
    ["- Hallazgos: P0 0 · P1 0 · P2 0 · P3 0", "- Hallazgos: P0 0 · P1 0 · P2 1 · P3 0"],
  ]) session = replaceOnce(session, before, after);
  return session;
}

function makeSummary() {
  let markdown = summaryTemplate;
  for (const [before, after] of [
    ["| Gate | Gate 1 · baseline V1 / Gate 2 · shell de espacios |", "| Gate | Gate 1 · baseline V1 |"],
    ["| Release candidate | rc-___ |", `| Release candidate | ${candidateId} |`],
    ["| Commit probado | ___ |", `| Commit probado | ${commit} |`],
    ["| Versión de Columnia | ___ |", `| Versión de Columnia | ${version} |`],
    ["| Fechas de ejecución | AAAA-MM-DD — AAAA-MM-DD |", "| Fechas de ejecución | 2026-09-12 — 2026-09-12 |"],
    ["| Sesiones válidas | ___ / 3 |", "| Sesiones válidas | 3 / 3 |"],
    ["| Participantes distintos | ___ / 3 |", "| Participantes distintos | 3 / 3 |"],
    ["| Usos de fixture sintética | ___ / 3 |", "| Usos de fixture sintética | 3 / 3 |"],
    ["| Casos reales ejecutados | ___ / 6 |", "| Casos reales ejecutados | 6 / 6 |"],
    ["| Datasets reales distintos | ___ / mínimo 3 |", "| Datasets reales distintos | 3 / mínimo 3 |"],
    ["| Tareas observadas | ___ / 30 |", "| Tareas observadas | 30 / 30 |"],
    ["| Sin ayuda | ___ | ___ % |", "| Sin ayuda | 24 | 80 % |"],
    ["| Con ayuda | ___ | ___ % |", "| Con ayuda | 6 | 20 % |"],
    ["| No completadas | ___ | ___ % |", "| No completadas | 0 | 0 % |"],
    ["| Total | ___ / 30 | 100 % |", "| Total | 30 / 30 | 100 % |"],
    ["Todas las personas completaron Cargar → Revisar → Preparar → Entregar: sí / no.", "Todas las personas completaron Cargar → Revisar → Preparar → Entregar: sí."],
    ["| Guardado, cierre y reapertura | ___ / 3 aprobadas |", "| Guardado, cierre y reapertura | 3 / 3 aprobadas |"],
    ["| Entrega verificada fuera de Columnia | ___ / 3 aprobadas |", "| Entrega verificada fuera de Columnia | 3 / 3 aprobadas |"],
    ["| Original sin cambios | ___ / 3 confirmado |", "| Original sin cambios | 3 / 3 confirmado |"],
    ["| P0 | ___ | ___ | ___ |", "| P0 | 0 | 0 | Ninguno |"],
    ["| P1 | ___ | ___ | ___ |", "| P1 | 0 | 0 | Ninguno |"],
    ["| P2 | ___ | ___ | ___ |", "| P2 | 0 | 0 | Ninguno |"],
    ["| P3 | ___ | ___ | ___ |", "| P3 | 0 | 0 | Ninguno |"],
    ["Los P2/P3 aceptados tienen responsable: sí / no.", "Los P2/P3 aceptados tienen responsable: sí."],
    ["- Gate 1 V1: aprobado / pendiente.", "- Gate 1 V1: aprobado."],
    ["- 80 % sin ayuda: aprobado / fallido.", "- 80 % sin ayuda: aprobado."],
    ["- Cero P0/P1 abiertos: aprobado / fallido.", "- Cero P0/P1 abiertos: aprobado."],
    ["- Revisión de privacidad del resumen: aprobado / pendiente.", "- Revisión de privacidad del resumen: aprobado."],
  ]) markdown = replaceOnce(markdown, before, after);
  return markdown;
}

function makeEvidence(overrides = {}) {
  const summaryMarkdown = overrides.summaryMarkdown ?? makeSummary();
  const sessions = overrides.sessions ?? [
    { alias: "beta-01", markdown: makeSession(0) },
    { alias: "beta-02", markdown: makeSession(1) },
    { alias: "beta-03", markdown: makeSession(2) },
  ];
  return {
    summaryMarkdown,
    sessions,
    currentCommit: overrides.currentCommit ?? "b".repeat(40),
    gate1CommitIsAncestor: overrides.gate1CommitIsAncestor ?? true,
    manifest: overrides.manifest ?? {
      schemaVersion: 1,
      candidateId,
      gate: "Gate1",
      commit,
      version,
      sessions: ["beta-01/session.md", "beta-02/session.md", "beta-03/session.md"],
      technicalGate: { profile: "Full", status: "passed", report: ".local/validation/test-full.json" },
    },
    fullReport: overrides.fullReport ?? {
      schemaVersion: 1,
      profile: "Full",
      status: "passed",
      git: { commit, dirty: false },
    },
  };
}

describe("prerrequisitos de evidencia para Gate 2", () => {
  it("acepta tres sesiones observadas y coherentes con el resumen Gate 1", () => {
    assert.deepEqual(validateGate1Evidence(makeEvidence()), { valid: true, errors: [] });
  });

  it("exige que Gate 2 esté en un commit posterior que descienda del baseline", () => {
    const sameCommit = validateGate1Evidence(makeEvidence({ currentCommit: commit }));
    assert.equal(sameCommit.valid, false);
    assert.ok(sameCommit.errors.some((error) => error.includes("distinto del baseline Gate 1")));

    const divergent = validateGate1Evidence(makeEvidence({ gate1CommitIsAncestor: false }));
    assert.equal(divergent.valid, false);
    assert.ok(divergent.errors.some((error) => error.includes("debe ser ancestro")));
  });

  it("acepta tareas observadas no completadas si el flujo y el agregado todavía cumplen", () => {
    const evidence = makeEvidence();
    evidence.sessions[0].markdown = replaceOnce(
      evidence.sessions[0].markdown,
      "| Distinguir muestra de cobertura completa | sin ayuda |",
      "| Distinguir muestra de cobertura completa | no completada |",
    );
    evidence.sessions[0].markdown = replaceOnce(
      evidence.sessions[0].markdown,
      "| Confirmar que el original no cambió | con ayuda |",
      "| Confirmar que el original no cambió | sin ayuda |",
    );
    evidence.sessions[0].markdown = replaceOnce(
      evidence.sessions[0].markdown,
      "| Recuperarse de un problema | con ayuda |",
      "| Recuperarse de un problema | sin ayuda |",
    );
    evidence.sessions[0].markdown = evidence.sessions[0].markdown
      .replace("- Tareas sin ayuda: 8 / 10", "- Tareas sin ayuda: 9 / 10")
      .replace("- Tareas con ayuda: 2 / 10", "- Tareas con ayuda: 0 / 10")
      .replace("- Tareas no completadas: 0 / 10", "- Tareas no completadas: 1 / 10");
    evidence.summaryMarkdown = evidence.summaryMarkdown
      .replace("| Sin ayuda | 24 | 80 % |", "| Sin ayuda | 25 | 83.3 % |")
      .replace("| Con ayuda | 6 | 20 % |", "| Con ayuda | 4 | 13.3 % |")
      .replace("| No completadas | 0 | 0 % |", "| No completadas | 1 | 3.3 % |");

    assert.deepEqual(validateGate1Evidence(evidence), { valid: true, errors: [] });
  });

  it("encierra la ruta del reporte Full dentro de validation, también frente a traversal y symlinks", async () => {
    const root = await mkdtemp(join(tmpdir(), "columnia-gate2-path-"));
    try {
      assert.equal(resolveValidationReportPath(".local/validation/full.json", root).reportPath, join(root, ".local", "validation", "full.json"));
      assert.throws(() => resolveValidationReportPath(".local/validation/../beta/full.json", root), /sin rutas relativas de escape/);
      assert.throws(() => resolveValidationReportPath(".local/validation/../../outside.json", root), /sin rutas relativas de escape/);

      const validationRoot = join(root, ".local", "validation");
      const outsideRoot = join(root, ".local", "beta");
      await mkdir(validationRoot, { recursive: true });
      await mkdir(outsideRoot, { recursive: true });
      await writeFile(join(outsideRoot, "full.json"), "{}", "utf8");
      await symlink(outsideRoot, join(validationRoot, "redirect"), process.platform === "win32" ? "junction" : "dir");
      await assert.rejects(
        resolvePhysicalValidationReport(".local/validation/redirect/full.json", root),
        /debe permanecer dentro de/,
      );
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  });

  it("bloquea una mezcla de commits entre formularios", () => {
    const evidence = makeEvidence();
    evidence.sessions[1].markdown = evidence.sessions[1].markdown.replace(commit, "b".repeat(40));

    const result = validateGate1Evidence(evidence);
    assert.equal(result.valid, false);
    assert.ok(result.errors.some((error) => error.includes("beta-02") && error.includes("mismo baseline")));
  });

  it("bloquea alias de participantes duplicados y pérdida de tareas", () => {
    const evidence = makeEvidence();
    evidence.sessions[2].markdown = evidence.sessions[2].markdown.replace("participante-03", "participante-02");
    evidence.sessions[0].markdown = evidence.sessions[0].markdown.replace("| Recuperarse de un problema | con ayuda |", "| Recuperarse de un problema | ___ |");

    const result = validateGate1Evidence(evidence);
    assert.equal(result.valid, false);
    assert.ok(result.errors.some((error) => error.includes("tres participantes distintos")));
    assert.ok(result.errors.some((error) => error.includes("resultado válido")));
  });

  it("bloquea un resumen que dice aprobado pero no alcanza 80 % sin ayuda", () => {
    const summaryMarkdown = makeSummary().replace("| Sin ayuda | 24 | 80 % |", "| Sin ayuda | 23 | 76 % |");

    const result = validateGate1Evidence(makeEvidence({ summaryMarkdown }));
    assert.equal(result.valid, false);
    assert.ok(result.errors.some((error) => error.includes("al menos 24 de 30")));
    assert.ok(result.errors.some((error) => error.includes("no coinciden con los resultados")));
  });

  it("bloquea porcentajes del resumen que contradicen las cantidades reportadas", () => {
    const summaryMarkdown = makeSummary().replace("| Sin ayuda | 24 | 80 % |", "| Sin ayuda | 24 | 75 % |");

    const result = validateGate1Evidence(makeEvidence({ summaryMarkdown }));
    assert.equal(result.valid, false);
    assert.ok(result.errors.some((error) => error.includes("porcentaje de “Sin ayuda”")));
  });

  it("no interpreta los valores alternativos de la plantilla como aprobación", () => {
    const summaryMarkdown = makeSummary().replace(
      "- Revisión de privacidad del resumen: aprobado.",
      "- Revisión de privacidad del resumen: aprobado / pendiente.",
    );

    const result = validateGate1Evidence(makeEvidence({ summaryMarkdown }));
    assert.equal(result.valid, false);
    assert.ok(result.errors.some((error) => error.includes("Revisión de privacidad")));
  });

  it("acepta hallazgos P2 documentados y bloquea hallazgos rellenados bajo el ID de ejemplo", () => {
    const accepted = makeEvidence();
    accepted.sessions[0].markdown = addP2Finding(accepted.sessions[0].markdown);
    accepted.summaryMarkdown = accepted.summaryMarkdown.replace(
      "| P2 | 0 | 0 | Ninguno |",
      "| P2 | 1 | 0 | responsable producto |",
    );
    assert.equal(validateGate1Evidence(accepted).valid, true);

    const hiddenFinding = makeEvidence();
    hiddenFinding.sessions[0].markdown = replaceOnce(
      hiddenFinding.sessions[0].markdown,
      "- Severidad: P0 / P1 / P2 / P3",
      "- Severidad: P2",
    );
    const result = validateGate1Evidence(hiddenFinding);
    assert.equal(result.valid, false);
    assert.ok(result.errors.some((error) => error.includes("ID BETA")));
  });

  it("bloquea formularios inválidos y un reporte Full que no corresponda", () => {
    const evidence = makeEvidence({
      manifest: { ...makeEvidence().manifest, gate: "Gate2" },
      fullReport: { schemaVersion: 1, profile: "Full", status: "passed", git: { commit: "b".repeat(40), dirty: false } },
    });

    const result = validateGate1Evidence(evidence);
    assert.equal(result.valid, false);
    assert.ok(result.errors.some((error) => error.includes("no coincide con Gate 1")));
    assert.ok(result.errors.some((error) => error.includes("evidencia Full original")));
  });
});
