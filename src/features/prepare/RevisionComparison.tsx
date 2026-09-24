import { useEffect, useMemo, useRef, useState } from "react";

import {
  cancelOperation,
  compareHistorySnapshots,
  type HistoryState,
  type OperationProgress,
  type QualityRule,
  type SnapshotRevisionComparison as SnapshotComparisonResult,
} from "../../bridge";
import { OperationProgressView } from "../../components/OperationProgressView";
import { formatPercent } from "../../format";

type ComparisonState =
  | { kind: "idle" }
  | { kind: "loading"; progress: OperationProgress; cancelRequested: boolean }
  | { kind: "ready"; result: SnapshotComparisonResult }
  | { kind: "error"; message: string };

interface RevisionComparisonProps {
  historyStatus: HistoryState;
  qualityRules: QualityRule[];
  datasetRevision: number;
  busy: boolean;
}

function signedDelta(value: number | null): string {
  if (value === null) return "No disponible";
  if (value > 0) return `+${value.toLocaleString()}`;
  return value.toLocaleString();
}

function metricLabel(value: boolean | null): string {
  if (value === null) return "No comparable";
  return value ? "Sí" : "No";
}

export function RevisionComparison({
  historyStatus,
  qualityRules,
  datasetRevision,
  busy,
}: RevisionComparisonProps) {
  const availableEntries = historyStatus.snapshotsEnabled
    ? historyStatus.entries.filter((entry) => entry.id !== null)
    : [];
  const revisionIdsKey = availableEntries.map((entry) => entry.id).join("|");
  const rulesKey = useMemo(() => JSON.stringify(qualityRules), [qualityRules]);
  const [beforeId, setBeforeId] = useState(availableEntries[0]?.id ?? "");
  const [afterId, setAfterId] = useState(availableEntries.at(-1)?.id ?? "");
  const [comparison, setComparison] = useState<ComparisonState>({ kind: "idle" });
  const requestGeneration = useRef(0);
  const currentInputs = useRef({ datasetRevision, revisionIdsKey, rulesKey, historyStatus });
  currentInputs.current = { datasetRevision, revisionIdsKey, rulesKey, historyStatus };

  useEffect(() => {
    requestGeneration.current += 1;
    setBeforeId(availableEntries[0]?.id ?? "");
    setAfterId(availableEntries.at(-1)?.id ?? "");
    setComparison({ kind: "idle" });
  }, [datasetRevision, revisionIdsKey, rulesKey, historyStatus.snapshotsEnabled]);

  async function runComparison() {
    const requestedBeforeId = beforeId;
    const requestedAfterId = afterId;
    if (!requestedBeforeId || !requestedAfterId || requestedBeforeId === requestedAfterId) return;
    const requestId = ++requestGeneration.current;
    const requestedRevision = datasetRevision;
    const requestedIdsKey = revisionIdsKey;
    const requestedRulesKey = rulesKey;
    setComparison({
      kind: "loading",
      progress: { operation: "profile", stage: "Preparando revisiones explícitas", percent: 0 },
      cancelRequested: false,
    });
    try {
      const result = await compareHistorySnapshots(
        requestedBeforeId,
        requestedAfterId,
        qualityRules,
        (progress) => {
          if (requestGeneration.current !== requestId) return;
          setComparison((current) => current.kind === "loading"
            ? { ...current, progress }
            : current);
        },
      );
      const inputs = currentInputs.current;
      const currentIds = new Set(
        inputs.historyStatus.snapshotsEnabled
          ? inputs.historyStatus.entries.map((entry) => entry.id).filter((id): id is string => id !== null)
          : [],
      );
      if (
        requestGeneration.current !== requestId
        || inputs.datasetRevision !== requestedRevision
        || inputs.revisionIdsKey !== requestedIdsKey
        || inputs.rulesKey !== requestedRulesKey
        || !currentIds.has(requestedBeforeId)
        || !currentIds.has(requestedAfterId)
      ) {
        return;
      }
      setComparison({ kind: "ready", result });
    } catch (error: unknown) {
      if (requestGeneration.current !== requestId) return;
      const message = error instanceof Error ? error.message : String(error);
      setComparison({ kind: "error", message });
    }
  }

  async function requestCancellation() {
    setComparison((current) => current.kind === "loading"
      ? { ...current, cancelRequested: true }
      : current);
    await cancelOperation("snapshotComparison").catch(() => undefined);
  }

  function changeSelection(side: "before" | "after", id: string) {
    requestGeneration.current += 1;
    setComparison({ kind: "idle" });
    if (side === "before") setBeforeId(id);
    else setAfterId(id);
  }

  return (
    <section className="prepare-card revision-comparison" aria-labelledby="revision-comparison-title">
      <div>
        <p className="step">Análisis comparativo</p>
        <h3 id="revision-comparison-title">Comparar revisiones</h3>
        <p>
          Elige dos cambios del historial para medir el antes y el después. Cada comparación vuelve a
          perfilar ambos snapshots y solo muestra agregados; no guarda perfiles ni devuelve valores de muestra.
        </p>
        <p className="profile-note">
          Los IDs pertenecen a cada snapshot. Los proyectos guardados los conservan al reabrirse; el historial de una sesión sin guardar es temporal.
        </p>
      </div>

      {!historyStatus.snapshotsEnabled ? (
        <p className="notice" role="note">
          {historyStatus.degradedReason
            ? `Comparación desactivada. Motivo del historial: ${historyStatus.degradedReason}`
            : "La comparación está desactivada porque este dataset no conserva snapshots de historial."}
        </p>
      ) : availableEntries.length < 2 ? (
        <p className="profile-note">Se necesitan al menos dos revisiones disponibles para comparar.</p>
      ) : comparison.kind === "loading" ? (
        <OperationProgressView
          progress={comparison.progress}
          cancellation={comparison.cancelRequested
            ? { kind: "requested" }
            : { kind: "available", onCancel: () => void requestCancellation() }}
        />
      ) : (
        <>
          <div className="revision-comparison__selectors">
            <label>
              Antes
              <select value={beforeId} onChange={(event) => changeSelection("before", event.target.value)} disabled={busy}>
                {availableEntries.map((entry) => (
                  <option key={entry.id} value={entry.id ?? ""}>{entry.label}</option>
                ))}
              </select>
            </label>
            <label>
              Después
              <select value={afterId} onChange={(event) => changeSelection("after", event.target.value)} disabled={busy}>
                {availableEntries.map((entry) => (
                  <option key={entry.id} value={entry.id ?? ""}>{entry.label}</option>
                ))}
              </select>
            </label>
            <button
              type="button"
              onClick={() => void runComparison()}
              disabled={busy || beforeId === afterId || !beforeId || !afterId}
            >
              Comparar agregados
            </button>
          </div>
          {qualityRules.length === 0 && (
            <p className="profile-note">
              Este proyecto no tiene reglas de calidad configuradas; se compararán filas, columnas, nulos y tipos.
            </p>
          )}
          {comparison.kind === "error" && (
            <p className="notice notice--error" role="alert">No se pudo comparar: {comparison.message}</p>
          )}
        </>
      )}

      {comparison.kind === "ready" && (
        <SnapshotComparisonResultView result={comparison.result} />
      )}
    </section>
  );
}

function SnapshotComparisonResultView({ result }: { result: SnapshotComparisonResult }) {
  return (
    <div className="revision-comparison__result" aria-label="Resultado agregado de revisiones">
      <p role="status">
        <strong>{result.beforeLabel}</strong> → <strong>{result.afterLabel}</strong>
      </p>
      <dl className="revision-comparison__metrics">
        <div><dt>Filas</dt><dd>{result.before.rowCount.toLocaleString()} → {result.after.rowCount.toLocaleString()} ({signedDelta(result.deltas.rowCount)})</dd></div>
        <div><dt>Columnas</dt><dd>{result.before.columnCount.toLocaleString()} → {result.after.columnCount.toLocaleString()} ({signedDelta(result.deltas.columnCount)})</dd></div>
        <div><dt>Celdas nulas</dt><dd>{result.before.nullCount.toLocaleString()} → {result.after.nullCount.toLocaleString()} ({signedDelta(result.deltas.nullCount)})</dd></div>
        <div><dt>Valores incompatibles</dt><dd>{result.before.invalidTypeCount.toLocaleString()} → {result.after.invalidTypeCount.toLocaleString()} ({signedDelta(result.deltas.invalidTypeCount)})</dd></div>
        <div><dt>Filas duplicadas</dt><dd>{result.before.duplicateRowCount.toLocaleString()} → {result.after.duplicateRowCount.toLocaleString()} ({signedDelta(result.deltas.duplicateRowCount)})</dd></div>
      </dl>

      <details>
        <summary>Tipos y nulos por columna ({result.columns.length})</summary>
        <ul>
          {result.columns.map((column) => (
            <li key={column.name}>
              <strong>{column.name}</strong>{": "}
              {column.comparable && column.before && column.after
                ? `${column.before.dataType} → ${column.after.dataType}; nulos ${column.before.nullCount.toLocaleString()} → ${column.after.nullCount.toLocaleString()}; incompatibles ${column.before.invalidTypeCount.toLocaleString()} → ${column.after.invalidTypeCount.toLocaleString()}`
                : column.reason ?? "No comparable"}
            </li>
          ))}
        </ul>
      </details>

      <details>
        <summary>
          Reglas de calidad ({result.quality.comparableRuleCount} comparables, {result.quality.nonComparableRuleCount} no comparables)
        </summary>
        {result.quality.rules.length === 0 ? (
          <p>No hay reglas configuradas en este proyecto.</p>
        ) : (
          <ul>
            {result.quality.rules.map((rule) => (
              <li key={rule.ruleIndex}>
                <strong>Regla {rule.ruleIndex} · {rule.kind}</strong>{rule.column ? ` · ${rule.column}` : ""}{": "}
                {rule.comparable
                  ? `${rule.beforeInvalidCount?.toLocaleString()} → ${rule.afterInvalidCount?.toLocaleString()} valores inválidos; tasa ${rule.beforeInvalidPercentage == null ? "—" : formatPercent(rule.beforeInvalidPercentage, 1)} → ${rule.afterInvalidPercentage == null ? "—" : formatPercent(rule.afterInvalidPercentage, 1)}; pasó ${metricLabel(rule.beforePassed)} → ${metricLabel(rule.afterPassed)}`
                  : rule.reason ?? "No comparable"}
              </li>
            ))}
          </ul>
        )}
      </details>
      <p className="profile-note">
        Mejoras de tasa de incumplimiento: {result.quality.improvedRuleCount}; retrocesos: {result.quality.degradedRuleCount}.
        Las reglas se evaluaron sin cambios en ambos snapshots.
      </p>
    </div>
  );
}
