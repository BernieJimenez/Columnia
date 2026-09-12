import { readFile } from "node:fs/promises";
import { dirname, isAbsolute, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const projectRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const requiredTasks = [
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

function normalized(value) {
  return String(value ?? "")
    .normalize("NFD")
    .replace(/\p{Diacritic}/gu, "")
    .trim()
    .toLocaleLowerCase("es");
}

function tableRows(markdown) {
  return markdown.split(/\r?\n/)
    .filter((line) => /^\s*\|/.test(line))
    .map((line) => line.trim().replace(/^\|/, "").replace(/\|$/, "").split("|").map((cell) => cell.trim()))
    .filter((cells) => cells.length > 1 && !cells.every((cell) => /^:?-{3,}:?$/.test(cell)));
}

function section(markdown, heading) {
  const lines = markdown.split(/\r?\n/);
  const expected = normalized(heading);
  const start = lines.findIndex((line) => /^##\s+/.test(line) && normalized(line.replace(/^##\s+/, "")) === expected);
  if (start < 0) return "";
  let end = lines.findIndex((line, index) => index > start && /^#{1,2}\s+/.test(line));
  if (end < 0) end = lines.length;
  return lines.slice(start + 1, end).join("\n");
}

function tableValue(markdown, heading, label, valueIndex = 1) {
  const wanted = normalized(label);
  const row = tableRows(section(markdown, heading)).find((cells) => normalized(cells[0]) === wanted);
  return row?.[valueIndex] ?? null;
}

function allTableValues(markdown, heading, label) {
  const wanted = normalized(label);
  return tableRows(section(markdown, heading))
    .filter((cells) => normalized(cells[0]) === wanted)
    .map((cells) => cells.slice(1));
}

function bulletValue(markdown, label) {
  const wanted = normalized(label);
  const match = markdown.split(/\r?\n/).map((line) => line.match(/^\s*-?\s*([^:]+):\s*(.*?)\s*$/))
    .find((candidate) => candidate && normalized(candidate[1]) === wanted);
  return match?.[2] ?? null;
}

function count(value) {
  const match = String(value ?? "").match(/^\s*(\d+)/);
  return match ? Number(match[1]) : null;
}

function fraction(value) {
  const match = String(value ?? "").match(/^\s*(\d+)\s*\/\s*(\d+)/);
  return match ? { numerator: Number(match[1]), denominator: Number(match[2]) } : null;
}

function requireFraction(errors, value, numerator, denominator, label) {
  const parsed = fraction(value);
  if (!parsed || parsed.numerator !== numerator || parsed.denominator !== denominator) {
    errors.push(`${label}: se esperaba ${numerator} / ${denominator}.`);
  }
  return parsed;
}

function requireApproved(errors, value, label) {
  const status = normalized(value ?? "");
  if (!/^aprobad[oa]s?\b/.test(status) || /\b(pendiente|fallido|fallida|todavia no ejecutado)\b/.test(status)) {
    errors.push(`${label}: debe indicar aprobado explícitamente.`);
  }
}

function requireYes(errors, value, label) {
  const status = normalized(value ?? "");
  if (!/^(si|confirmado|confirmada)\b/.test(status) || /\bno\b/.test(status)) {
    errors.push(`${label}: debe indicar sí explícitamente.`);
  }
}

function parseCandidate(summaryMarkdown, errors) {
  const gate = tableValue(summaryMarkdown, "Release candidate", "Gate");
  const candidateId = tableValue(summaryMarkdown, "Release candidate", "Release candidate");
  const commit = tableValue(summaryMarkdown, "Release candidate", "Commit probado");
  const version = tableValue(summaryMarkdown, "Release candidate", "Versión de Columnia");

  if (!normalized(gate).startsWith("gate 1") || !normalized(gate).includes("baseline v1")) {
    errors.push("El resumen debe identificar Gate 1 como baseline V1, no como shell.");
  }
  if (!/^rc-[a-z0-9][a-z0-9.-]{2,79}$/i.test(candidateId ?? "")) {
    errors.push("El resumen debe identificar un release candidate Gate 1 real.");
  }
  if (!/^[a-f0-9]{40}$/i.test(commit ?? "")) {
    errors.push("El resumen debe identificar el commit completo de Gate 1.");
  }
  if (!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(version ?? "")) {
    errors.push("El resumen debe identificar la versión de Columnia probada.");
  }
  return { candidateId, commit, version };
}

function validateSummary(markdown, candidate, errors) {
  const sessions = fraction(tableValue(markdown, "Muestra agregada", "Sesiones válidas"));
  requireFraction(errors, tableValue(markdown, "Muestra agregada", "Sesiones válidas"), 3, 3, "Sesiones válidas");
  requireFraction(errors, tableValue(markdown, "Muestra agregada", "Participantes distintos"), 3, 3, "Participantes distintos");
  requireFraction(errors, tableValue(markdown, "Muestra agregada", "Casos reales ejecutados"), 6, 6, "Casos reales ejecutados");
  const realDatasets = count(tableValue(markdown, "Muestra agregada", "Datasets reales distintos"));
  if (realDatasets === null || realDatasets < 3) errors.push("Datasets reales distintos: se requieren al menos 3.");
  requireFraction(errors, tableValue(markdown, "Muestra agregada", "Tareas observadas"), 30, 30, "Tareas observadas");

  const noHelp = count(tableValue(markdown, "Resultado de tareas", "Sin ayuda"));
  const helped = count(tableValue(markdown, "Resultado de tareas", "Con ayuda"));
  const incomplete = count(tableValue(markdown, "Resultado de tareas", "No completadas"));
  const total = fraction(tableValue(markdown, "Resultado de tareas", "Total"));
  if (noHelp === null || noHelp < 24) errors.push("Tareas sin ayuda: se requieren al menos 24 de 30.");
  if (noHelp !== null && helped !== null && incomplete !== null && noHelp + helped + incomplete !== 30) {
    errors.push("Los resultados de las tareas deben sumar exactamente 30.");
  }
  if (!total || total.numerator !== 30 || total.denominator !== 30) {
    errors.push("El total de tareas debe ser 30 / 30.");
  }

  requireYes(errors, bulletValue(markdown, "Todas las personas completaron Cargar → Revisar → Preparar → Entregar"), "Flujo principal de todas las personas");
  requireFraction(errors, tableValue(markdown, "Persistencia y entrega", "Guardado, cierre y reapertura"), 3, 3, "Guardado, cierre y reapertura");
  requireFraction(errors, tableValue(markdown, "Persistencia y entrega", "Entrega verificada fuera de Columnia"), 3, 3, "Entrega verificada fuera de Columnia");
  requireFraction(errors, tableValue(markdown, "Persistencia y entrega", "Original sin cambios"), 3, 3, "Original sin cambios");

  for (const severity of ["P0", "P1"]) {
    const values = allTableValues(markdown, "Hallazgos y decisiones", severity)[0];
    if (!values || count(values[1]) !== 0) errors.push(`${severity}: no puede haber hallazgos abiertos en Gate 1.`);
  }
  requireYes(errors, bulletValue(markdown, "Los P2/P3 aceptados tienen responsable"), "Responsables P2/P3");

  const verdict = bulletValue(markdown, "Gate 1 V1");
  requireApproved(errors, verdict, "Veredicto Gate 1 V1");
  requireApproved(errors, bulletValue(markdown, "80 % sin ayuda"), "Veredicto de tareas sin ayuda");
  requireApproved(errors, bulletValue(markdown, "Cero P0/P1 abiertos"), "Veredicto P0/P1");
  requireApproved(errors, bulletValue(markdown, "Revisión de privacidad del resumen"), "Revisión de privacidad del resumen");

  if (sessions && sessions.numerator !== 3) errors.push("El resumen debe contar exactamente tres sesiones Gate 1.");
  if (candidate.candidateId && tableValue(markdown, "Release candidate", "Release candidate") !== candidate.candidateId) {
    errors.push("El identificador RC del resumen cambió durante la validación.");
  }
}

function validateManifest(manifest, candidate, errors) {
  if (!manifest || manifest.schemaVersion !== 1) {
    errors.push("Falta el manifiesto local válido de la RC Gate 1.");
    return;
  }
  if (manifest.gate !== "Gate1" || manifest.candidateId !== candidate.candidateId || manifest.commit !== candidate.commit || manifest.version !== candidate.version) {
    errors.push("El manifiesto local no coincide con Gate 1, RC, commit y versión del resumen.");
  }
  const expectedSessions = ["beta-01/session.md", "beta-02/session.md", "beta-03/session.md"];
  if (!Array.isArray(manifest.sessions) || manifest.sessions.length !== expectedSessions.length ||
      expectedSessions.some((path, index) => manifest.sessions[index]?.replaceAll("\\", "/") !== path)) {
    errors.push("El manifiesto no enumera las tres sesiones Gate 1 esperadas.");
  }
  if (manifest.technicalGate?.profile !== "Full" || manifest.technicalGate?.status !== "passed") {
    errors.push("El manifiesto Gate 1 no conserva un gate técnico Full aprobado.");
  }
}

function validateSession(session, index, candidate, errors) {
  const alias = `beta-${String(index + 1).padStart(2, "0")}`;
  const markdown = session?.markdown ?? "";
  const sessionAlias = tableValue(markdown, "Identificación segura", "Alias de sesión");
  const participant = tableValue(markdown, "Identificación segura", "Alias anónimo de participante");
  const commit = tableValue(markdown, "Identificación segura", "Commit probado");
  const version = tableValue(markdown, "Identificación segura", "Versión de Columnia");
  const round = tableValue(markdown, "Identificación segura", "Ronda de medición");
  const candidateId = tableValue(markdown, "Identificación segura", "Release candidate");

  if (session?.alias !== alias || sessionAlias !== alias) errors.push(`${alias}: falta el formulario correcto de la sesión.`);
  if (!participant || /^participante-___$/i.test(participant)) errors.push(`${alias}: falta el alias anónimo de participante.`);
  if (commit !== candidate.commit || version !== candidate.version || candidateId !== candidate.candidateId ||
      !normalized(round).startsWith("gate 1") || !normalized(round).includes("baseline v1")) {
    errors.push(`${alias}: participante, ronda, RC, versión y commit deben corresponder al mismo baseline Gate 1.`);
  }

  const datasetRows = tableRows(section(markdown, "Datasets"));
  for (const label of ["Tabla real", "Caso estructural"]) {
    const row = datasetRows.find((cells) => normalized(cells[0]) === normalized(label));
    if (!row || !/^dataset-[a-z0-9][a-z0-9.-]*$/i.test(row[1] ?? "")) {
      errors.push(`${alias}: falta un alias estable para ${label.toLocaleLowerCase("es")} real.`);
    } else {
      session.realDatasetAliases.push(row[1].toLocaleLowerCase("es"));
    }
  }

  const taskRows = tableRows(section(markdown, "Resultado y fricción por tarea"))
    .filter((cells) => requiredTasks.some((task) => normalized(cells[0]) === normalized(task)));
  if (taskRows.length !== requiredTasks.length) {
    errors.push(`${alias}: deben observarse las diez tareas definidas.`);
  }
  const taskCounts = { "sin ayuda": 0, "con ayuda": 0, "no completada": 0 };
  for (const task of requiredTasks) {
    const row = taskRows.find((cells) => normalized(cells[0]) === normalized(task));
    const outcome = normalized(row?.[1]);
    if (!(outcome in taskCounts)) errors.push(`${alias}: la tarea “${task}” necesita un resultado válido.`);
    else taskCounts[outcome] += 1;
  }
  session.taskCounts = taskCounts;

  const verdict = section(markdown, "Veredicto");
  for (const [label, expected] of [
    ["Tareas sin ayuda", taskCounts["sin ayuda"]],
    ["Tareas con ayuda", taskCounts["con ayuda"]],
    ["Tareas no completadas", taskCounts["no completada"]],
  ]) {
    const actual = fraction(bulletValue(verdict, label));
    if (!actual || actual.numerator !== expected || actual.denominator !== 10) {
      errors.push(`${alias}: el conteo “${label}” no coincide con las diez tareas registradas.`);
    }
  }
  requireYes(errors, bulletValue(verdict, "Flujo principal Cargar → Revisar → Preparar → Entregar"), `${alias}: flujo principal`);
  requireYes(errors, bulletValue(verdict, "Original sin cambios"), `${alias}: original sin cambios`);
  requireApproved(errors, bulletValue(verdict, "Guardado y reapertura"), `${alias}: guardado y reapertura`);
  requireApproved(errors, bulletValue(verdict, "Entrega verificada fuera de Columnia"), `${alias}: entrega independiente`);
  if (normalized(bulletValue(verdict, "Validez de la sesión")) !== "valida") {
    errors.push(`${alias}: la sesión debe estar marcada como válida.`);
  }

  const findingCounts = bulletValue(verdict, "Hallazgos")?.match(/P([0-3])\s+(\d+)/gi) ?? [];
  const countsBySeverity = { P0: 0, P1: 0, P2: 0, P3: 0 };
  for (const entry of findingCounts) {
    const match = entry.match(/(P[0-3])\s+(\d+)/i);
    if (match) countsBySeverity[match[1].toUpperCase()] = Number(match[2]);
  }
  for (const severity of ["P0", "P1", "P2", "P3"]) {
    if (!findingCounts.some((entry) => new RegExp(`${severity}\\s+\\d+`, "i").test(entry))) {
      errors.push(`${alias}: falta el conteo explícito ${severity} del veredicto.`);
    }
  }
  if (countsBySeverity.P0 !== 0 || countsBySeverity.P1 !== 0) {
    errors.push(`${alias}: una sesión con hallazgos P0/P1 no puede integrar la muestra Gate 1 aceptada.`);
  }

  const findingSection = section(markdown, "Hallazgos");
  const findingHeadings = [...findingSection.matchAll(/^###\s+(BETA-[A-Z0-9]+)\s+[—-]\s+(.+)$/gim)]
    .filter((match) => !/^BETA-___$/i.test(match[1]));
  for (const match of findingHeadings) {
    const start = match.index + match[0].length;
    const remaining = findingSection.slice(start);
    const nextHeading = remaining.search(/^###\s+/m);
    const finding = nextHeading >= 0 ? remaining.slice(0, nextHeading) : remaining;
    const severity = bulletValue(finding, "Severidad");
    if (!/^P[0-3]$/i.test(severity ?? "")) errors.push(`${alias}: cada hallazgo debe tener severidad.`);
    if (["P0", "P1"].includes(String(severity).toUpperCase())) errors.push(`${alias}: el formulario incluye un hallazgo ${String(severity).toUpperCase()}.`);
    if (["P2", "P3"].includes(String(severity).toUpperCase())) {
      const decision = bulletValue(finding, "Decisión");
      const owner = bulletValue(finding, "Responsable");
      const reproduction = bulletValue(finding, "Reproducción sintética o pasos sanitizados");
      if (!/^(corregir|aceptar|investigar)\b/i.test(normalized(decision ?? ""))) errors.push(`${alias}: cada hallazgo P2/P3 necesita una decisión.`);
      if (!owner || /^(___|pendiente|n\/a)$/i.test(owner)) errors.push(`${alias}: cada hallazgo P2/P3 necesita responsable.`);
      if (!reproduction || /^(___|pendiente)$/i.test(reproduction)) errors.push(`${alias}: cada hallazgo P2/P3 necesita reproducción o razón sanitizada.`);
    }
  }

  session.findingCounts = countsBySeverity;
  session.participant = normalized(participant);
}

export function validateGate1Evidence({ summaryMarkdown, manifest, sessions, fullReport }) {
  const errors = [];
  const candidate = parseCandidate(summaryMarkdown ?? "", errors);
  validateSummary(summaryMarkdown ?? "", candidate, errors);
  validateManifest(manifest, candidate, errors);

  if (!fullReport || fullReport.schemaVersion !== 1 || fullReport.profile !== "Full" ||
      fullReport.status !== "passed" || fullReport.git?.commit !== candidate.commit || fullReport.git?.dirty !== false) {
    errors.push("La evidencia Full original debe estar aprobada, limpia y asociada al commit Gate 1.");
  }
  if (!Array.isArray(sessions) || sessions.length !== 3) {
    errors.push("Se requieren exactamente tres formularios locales Gate 1.");
  }

  const sessionEvidence = Array.isArray(sessions) ? sessions.slice(0, 3).map((session) => ({
    ...session,
    realDatasetAliases: [],
    taskCounts: null,
    findingCounts: null,
    participant: null,
  })) : [];
  sessionEvidence.forEach((session, index) => validateSession(session, index, candidate, errors));

  const participants = sessionEvidence.map((session) => session.participant).filter(Boolean);
  if (new Set(participants).size !== 3) errors.push("Las tres sesiones deben pertenecer a tres participantes distintos.");
  if (new Set(sessionEvidence.flatMap((session) => session.realDatasetAliases)).size < 3) {
    errors.push("Las sesiones deben cubrir al menos tres datasets reales distintos.");
  }

  const totals = sessionEvidence.reduce((sum, session) => {
    if (!session.taskCounts) return sum;
    for (const outcome of Object.keys(sum)) sum[outcome] += session.taskCounts[outcome];
    return sum;
  }, { "sin ayuda": 0, "con ayuda": 0, "no completada": 0 });
  const reportedNoHelp = count(tableValue(summaryMarkdown ?? "", "Resultado de tareas", "Sin ayuda"));
  const reportedHelped = count(tableValue(summaryMarkdown ?? "", "Resultado de tareas", "Con ayuda"));
  const reportedIncomplete = count(tableValue(summaryMarkdown ?? "", "Resultado de tareas", "No completadas"));
  if (reportedNoHelp !== totals["sin ayuda"] || reportedHelped !== totals["con ayuda"] || reportedIncomplete !== totals["no completada"]) {
    errors.push("Los agregados del resumen no coinciden con los resultados de los tres formularios.");
  }

  for (const severity of ["P0", "P1", "P2", "P3"]) {
    const reported = allTableValues(summaryMarkdown ?? "", "Hallazgos y decisiones", severity)[0];
    const found = count(reported?.[0]);
    const actual = sessionEvidence.reduce((sum, session) => sum + (session.findingCounts?.[severity] ?? 0), 0);
    if (found !== actual) errors.push(`El resumen y los formularios no coinciden en hallazgos ${severity}.`);
  }

  return { valid: errors.length === 0, errors };
}

function assertInsideProject(path) {
  const absolute = resolve(projectRoot, path);
  const relativePath = relative(projectRoot, absolute);
  if (!relativePath || relativePath === ".." || relativePath.startsWith(`..${sep}`) || isAbsolute(relativePath)) {
    throw new Error("La ruta de evidencia sale del repositorio.");
  }
  return absolute;
}

async function readEvidenceFile(path, label) {
  try {
    return await readFile(path, "utf8");
  } catch (error) {
    if (error?.code === "ENOENT") {
      const relativePath = relative(projectRoot, path).replaceAll("\\", "/");
      throw new Error(`falta ${label} (${relativePath}); completa y revisa Gate 1 antes de preparar Gate 2.`);
    }
    throw error;
  }
}

async function run() {
  const summaryPath = join(projectRoot, "docs", "reference", "beta-v1-summary.md");
  const summaryMarkdown = await readEvidenceFile(summaryPath, "el resumen sanitizado Gate 1");
  const candidateId = tableValue(summaryMarkdown, "Release candidate", "Release candidate");
  if (!candidateId || !/^rc-[a-z0-9][a-z0-9.-]{2,79}$/i.test(candidateId)) {
    throw new Error("El resumen no identifica una RC Gate 1 utilizable.");
  }
  const candidateRoot = join(projectRoot, ".local", "beta", candidateId);
  const manifest = JSON.parse(await readEvidenceFile(join(candidateRoot, "manifest.json"), "el manifiesto local de Gate 1"));
  const sessionAliases = ["beta-01", "beta-02", "beta-03"];
  const sessions = await Promise.all(sessionAliases.map(async (alias) => ({
    alias,
    markdown: await readEvidenceFile(join(candidateRoot, alias, "session.md"), `el formulario local ${alias}`),
  })));
  const reportRelativePath = manifest.technicalGate?.report;
  if (typeof reportRelativePath !== "string" || !reportRelativePath.replaceAll("\\", "/").startsWith(".local/validation/")) {
    throw new Error("El manifiesto no apunta a evidencia Full dentro de .local/validation.");
  }
  const fullReportPath = assertInsideProject(reportRelativePath);
  const fullReport = JSON.parse(await readEvidenceFile(fullReportPath, "el reporte técnico Full original"));
  const result = validateGate1Evidence({ summaryMarkdown, manifest, sessions, fullReport });
  if (!result.valid) {
    console.error("Gate 2 bloqueado; Gate 1 no tiene evidencia completa y consistente:");
    for (const error of result.errors) console.error(`- ${error}`);
    process.exitCode = 1;
    return;
  }
  console.log(`Gate 1 validado; Gate 2 puede preparar una nueva RC. Baseline: ${candidateId} (${manifest.commit.slice(0, 7)}).`);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  run().catch((error) => {
    console.error(`Gate 2 bloqueado: ${error instanceof Error ? error.message : String(error)}`);
    process.exitCode = 1;
  });
}
