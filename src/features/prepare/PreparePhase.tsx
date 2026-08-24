import { useEffect, useState } from "react";

import { OperationProgressView } from "../../components/OperationProgressView";
import type { DatasetPreview, DatasetProfile, HistoryState, SavedRecipe, TransformRecipe } from "../../bridge";
import type { ProfileStatus } from "../review/reviewModel";
import { ChangeFeedback, HistoryBar } from "./HistoryBar";
import { TransformRecipeEditor } from "./TransformRecipeEditor";
import type { ChangeStatus } from "./prepareModel";

interface PreparePhaseProps {
  dataset: DatasetPreview;
  profileStatus: ProfileStatus;
  changeStatus: ChangeStatus;
  historyStatus: HistoryState;
  recipeDraft: SavedRecipe | null;
  recipeSession: number;
  onAnalyzeQuality: () => void;
  onCancelProfile: () => void;
  onRemoveDuplicates: () => void;
  onRemoveEmptyRows: () => void;
  onNormalizeColumns: () => void;
  onApplyRecommended: () => void;
  onTrimText: () => void;
  onNormalizeText: (columns: string[], removeAccents: boolean) => void;
  onApplyTransforms: (recipe: TransformRecipe) => void;
  onRecipeDraftChange: (draft: SavedRecipe) => void;
  onUndo: () => void;
  onRedo: () => void;
}

export function PreparePhase({
  dataset,
  profileStatus,
  changeStatus,
  historyStatus,
  recipeDraft,
  recipeSession,
  onAnalyzeQuality,
  onCancelProfile,
  onRemoveDuplicates,
  onRemoveEmptyRows,
  onNormalizeColumns,
  onApplyRecommended,
  onTrimText,
  onNormalizeText,
  onApplyTransforms,
  onRecipeDraftChange,
  onUndo,
  onRedo,
}: PreparePhaseProps) {
  const duplicateCount = profileStatus.kind === "ready" ? profileStatus.profile.duplicateRowCount : null;
  const changing = changeStatus.kind === "working";
  const textColumns = dataset.columns.filter((column) => column.dataType === "String" && column.name !== "_cambios");
  const [selectedTextColumns, setSelectedTextColumns] = useState<string[]>([]);
  const [removeAccents, setRemoveAccents] = useState(true);
  const [activeTab, setActiveTab] = useState<"corrections" | "transformations">("corrections");

  useEffect(() => {
    const available = new Set(textColumns.map((column) => column.name));
    setSelectedTextColumns((current) => current.filter((name) => available.has(name)));
  }, [dataset.columns]);

  return (
    <>
      <header className="phase-header phase-header--compact">
        <div>
          <p className="eyebrow">Preparar · {activeTab === "corrections" ? "Correcciones" : "Transformaciones"}</p>
          <h2>{dataset.fileName}</h2>
          <p>Aplica cambios controlados al dataset activo. Cada corrección indica su impacto.</p>
        </div>
      </header>
      <HistoryBar
        status={historyStatus}
        busy={changing}
        onUndo={onUndo}
        onRedo={onRedo}
      />
      <ChangeFeedback status={changeStatus} />
      <div className="stage-tabs" role="tablist" aria-label="Herramientas de preparación">
        <button
          id="prepare-corrections-tab"
          type="button"
          role="tab"
          aria-selected={activeTab === "corrections"}
          aria-controls="prepare-corrections-panel"
          tabIndex={activeTab === "corrections" ? 0 : -1}
          className={activeTab === "corrections" ? "stage-tab--active" : undefined}
          onClick={() => setActiveTab("corrections")}
          onKeyDown={(event) => {
            if (event.key === "ArrowRight" || event.key === "ArrowLeft") {
              event.preventDefault();
              setActiveTab("transformations");
              document.getElementById("prepare-transformations-tab")?.focus();
            }
          }}
        >
          Correcciones
        </button>
        <button
          id="prepare-transformations-tab"
          type="button"
          role="tab"
          aria-selected={activeTab === "transformations"}
          aria-controls="prepare-transformations-panel"
          tabIndex={activeTab === "transformations" ? 0 : -1}
          className={activeTab === "transformations" ? "stage-tab--active" : undefined}
          onClick={() => setActiveTab("transformations")}
          onKeyDown={(event) => {
            if (event.key === "ArrowRight" || event.key === "ArrowLeft") {
              event.preventDefault();
              setActiveTab("corrections");
              document.getElementById("prepare-corrections-tab")?.focus();
            }
          }}
        >
          Transformaciones
        </button>
      </div>
      {activeTab === "transformations" ? (
        <div
          id="prepare-transformations-panel"
          role="tabpanel"
          aria-labelledby="prepare-transformations-tab"
        >
          <TransformRecipeEditor
            key={recipeSession}
            dataset={dataset}
            busy={changing}
            initialDraft={recipeDraft}
            onApply={onApplyTransforms}
            onDraftChange={onRecipeDraftChange}
          />
        </div>
      ) : (
      <div
        id="prepare-corrections-panel"
        role="tabpanel"
        aria-labelledby="prepare-corrections-tab"
      >
      <section className="recommended-batch" aria-labelledby="recommended-batch-title">
        <div>
          <p className="step">Aplicación agrupada</p>
          <h3 id="recommended-batch-title">Correcciones recomendadas</h3>
          <p>
            Recorta espacios exteriores y normaliza los encabezados en una sola operación
            atómica y reversible.
          </p>
        </div>
        <button type="button" onClick={onApplyRecommended} disabled={changing}>
          Aplicar recomendadas
        </button>
      </section>
      {profileStatus.kind === "ready" && <CleaningSignals profile={profileStatus.profile} />}
      <section className="prepare-card" aria-labelledby="normalize-columns-title">
        <div>
          <p className="step">Recomendada y segura</p>
          <h3 id="normalize-columns-title">Normalizar nombres de columnas</h3>
          <p>
            Convierte los encabezados a nombres consistentes en minúsculas, sin acentos y con
            guiones bajos. Las colisiones se numeran de forma determinista.
          </p>
        </div>
        <button type="button" onClick={onNormalizeColumns} disabled={changing}>
          Normalizar columnas
        </button>
      </section>
      <section className="prepare-card" aria-labelledby="trim-text-title">
        <div>
          <p className="step">Recomendada y segura</p>
          <h3 id="trim-text-title">Eliminar espacios exteriores</h3>
          <p>
            Recorta espacios al inicio y al final de todas las columnas de texto sin cambiar
            mayúsculas, acentos ni espacios internos.
          </p>
        </div>
        <button type="button" onClick={onTrimText} disabled={changing || textColumns.length === 0}>
          Recortar espacios
        </button>
      </section>
      <section className="prepare-card prepare-card--stacked" aria-labelledby="normalize-text-title">
        <div>
          <p className="step">Requiere selección</p>
          <h3 id="normalize-text-title">Normalizar texto</h3>
          <p>
            Convierte a minúsculas, compacta espacios y, opcionalmente, elimina acentos. Puede
            unir categorías que antes eran distintas; elige las columnas conscientemente.
          </p>
        </div>
        {textColumns.length > 0 ? (
          <div className="text-cleaning-options">
            <fieldset>
              <legend>Columnas de texto</legend>
              {textColumns.map((column) => (
                <label key={column.name}>
                  <input
                    type="checkbox"
                    checked={selectedTextColumns.includes(column.name)}
                    onChange={(event) =>
                      setSelectedTextColumns((current) =>
                        event.target.checked
                          ? [...current, column.name]
                          : current.filter((name) => name !== column.name),
                      )
                    }
                  />
                  {column.name}
                </label>
              ))}
            </fieldset>
            <label className="option-toggle">
              <input
                type="checkbox"
                checked={removeAccents}
                onChange={(event) => setRemoveAccents(event.target.checked)}
              />
              Eliminar acentos
            </label>
            <button
              type="button"
              onClick={() => onNormalizeText(selectedTextColumns, removeAccents)}
              disabled={changing || selectedTextColumns.length === 0}
            >
              Normalizar texto seleccionado
            </button>
          </div>
        ) : (
          <p className="profile-note">Este dataset no contiene columnas de texto.</p>
        )}
      </section>
      <section className="prepare-card" aria-labelledby="empty-rows-title">
        <div>
          <p className="step">Corrección segura</p>
          <h3 id="empty-rows-title">Filas completamente vacías</h3>
          <p>Elimina filas cuyos valores son todos nulos o texto en blanco. La operación conserva el orden y es reversible.</p>
        </div>
        <button type="button" onClick={onRemoveEmptyRows} disabled={changing}>
          Eliminar filas vacías
        </button>
      </section>
      {profileStatus.kind === "loading" ? (
        <OperationProgressView
          progress={profileStatus.progress}
          cancellation={profileStatus.cancelRequested
            ? { kind: "requested" }
            : { kind: "available", onCancel: onCancelProfile }}
        />
      ) : (
        <section className="prepare-card" aria-labelledby="duplicates-title">
          <div>
            <p className="step">Corrección disponible</p>
            <h3 id="duplicates-title">Filas duplicadas</h3>
            <p>
              {duplicateCount === null
                ? "Analiza la calidad para identificar duplicados antes de modificar los datos."
                : duplicateCount === 0
                  ? "No se detectaron filas duplicadas adicionales."
                  : `Se detectaron ${duplicateCount.toLocaleString()} filas duplicadas adicionales.`}
            </p>
          </div>
          {duplicateCount === null ? (
            <button type="button" onClick={onAnalyzeQuality}>
              Analizar antes de preparar
            </button>
          ) : (
            <button
              type="button"
              onClick={onRemoveDuplicates}
              disabled={duplicateCount === 0 || changing}
            >
              Eliminar duplicados
            </button>
          )}
        </section>
      )}
      {profileStatus.kind === "error" && (
        <p className="notice notice--error" role="alert">
          No se pudo analizar la calidad: {profileStatus.message}
        </p>
      )}
      </div>
      )}
    </>
  );
}

function CleaningSignals({ profile }: { profile: DatasetProfile }) {
  const incomplete = profile.columns.filter((column) => column.completenessPercentage < 100);
  const constant = profile.columns.filter(
    (column) => profile.rowCount > 1 && column.uniqueCount <= 1 && column.nullCount < profile.rowCount,
  );
  const typeDrift = profile.columns.filter(
    (column) => (column.invalidTypeCount ?? 0) > 0,
  );
  const personal = profile.columns.filter((column) =>
    /(email|correo|mail|phone|tel[eé]fono|address|direcci[oó]n|dni|cedula|c[eé]dula|ssn)/i.test(column.name),
  );
  const hasSignals = profile.duplicateRowCount > 0 || incomplete.length > 0 || constant.length > 0 || typeDrift.length > 0 || personal.length > 0;

  return (
    <section className="prepare-card prepare-card--stacked cleaning-signals" aria-labelledby="cleaning-signals-title">
      <div>
        <p className="step">Catálogo de limpieza</p>
        <h3 id="cleaning-signals-title">Señales para revisar</h3>
        <p>Las señales usan solo esquema y métricas agregadas; no muestran celdas ni valores personales.</p>
      </div>
      {hasSignals ? (
        <ul className="cleaning-signals__list">
          {profile.duplicateRowCount > 0 && (
            <li><strong>Duplicados exactos:</strong> {profile.duplicateRowCount.toLocaleString()} filas adicionales; puedes eliminarlas de forma reversible.</li>
          )}
          {incomplete.length > 0 && (
            <li><strong>Completitud:</strong> {incomplete.length} {incomplete.length === 1 ? "columna tiene" : "columnas tienen"} al menos un nulo: {incomplete.map((column) => column.name).join(", ")}.</li>
          )}
          {constant.length > 0 && (
            <li><strong>Constantes:</strong> {constant.map((column) => column.name).join(", ")} {constant.length === 1 ? "no cambia" : "no cambian"} entre filas.</li>
          )}
          {typeDrift.length > 0 && (
            <li><strong>Tipos sugeridos:</strong> {typeDrift.map((column) => column.name).join(", ")} contiene valores que no coinciden con la sugerencia detectada.</li>
          )}
          {personal.length > 0 && (
            <li className="cleaning-signals__privacy"><strong>Posible dato personal:</strong> revisa el tratamiento de {personal.map((column) => column.name).join(", ")} antes de exportar o compartir.</li>
          )}
        </ul>
      ) : (
        <p className="notice notice--success" role="status">No se detectaron señales de limpieza en el perfil actual.</p>
      )}
    </section>
  );
}
