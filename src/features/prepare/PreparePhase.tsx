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
  onRemoveConstantColumns: () => void;
  onRemoveEmptyColumns: () => void;
  onRemoveHighNullColumns: () => void;
  onNormalizeSentinels: () => void;
  onNormalizeBooleans: () => void;
  onImputeMissingValues: () => void;
  onEnableRowAudit: () => void;
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
  onRemoveConstantColumns,
  onRemoveEmptyColumns,
  onRemoveHighNullColumns,
  onNormalizeSentinels,
  onNormalizeBooleans,
  onImputeMissingValues,
  onEnableRowAudit,
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
      {profileStatus.kind === "ready" && (
        <CleaningSignals
          profile={profileStatus.profile}
          busy={changing}
          onRemoveConstantColumns={onRemoveConstantColumns}
              onRemoveEmptyColumns={onRemoveEmptyColumns}
              onRemoveHighNullColumns={onRemoveHighNullColumns}
              onNormalizeSentinels={onNormalizeSentinels}
              onNormalizeBooleans={onNormalizeBooleans}
              onImputeMissingValues={onImputeMissingValues}
        />
      )}
      <section className="prepare-card" aria-labelledby="row-audit-title">
        <div>
          <p className="step">Trazabilidad local</p>
          <h3 id="row-audit-title">Cambios por fila</h3>
          <p>
            {dataset.columns.some((column) => column.name === "_cambios")
              ? "La columna _cambios está activa; cada corrección posterior añadirá su operación a la fila afectada."
              : "Activa una columna reservada _cambios para conservar una etiqueta breve de las correcciones posteriores."}
          </p>
        </div>
        <button
          type="button"
          onClick={onEnableRowAudit}
          disabled={changing || dataset.columns.some((column) => column.name === "_cambios")}
        >
          {dataset.columns.some((column) => column.name === "_cambios") ? "Trazabilidad activa" : "Activar trazabilidad"}
        </button>
      </section>
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
      <details className="advanced-corrections">
        <summary>
          <span>Correcciones avanzadas</span>
          <small>Selección manual, eliminación de filas y duplicados</small>
        </summary>
        <div className="advanced-corrections__content">
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
      </details>
      </div>
      )}
    </>
  );
}

function CleaningSignals({
  profile,
  busy,
  onRemoveConstantColumns,
  onRemoveEmptyColumns,
  onRemoveHighNullColumns,
  onNormalizeSentinels,
  onNormalizeBooleans,
  onImputeMissingValues,
}: {
  profile: DatasetProfile;
  busy: boolean;
  onRemoveConstantColumns: () => void;
  onRemoveEmptyColumns: () => void;
  onRemoveHighNullColumns: () => void;
  onNormalizeSentinels: () => void;
  onNormalizeBooleans: () => void;
  onImputeMissingValues: () => void;
}) {
  const incomplete = profile.columns.filter((column) => column.completenessPercentage < 100);
  const imputable = incomplete.filter(
    (column) => column.nullCount > 0 && column.nullCount < profile.rowCount && column.name !== "_cambios",
  );
  const constant = profile.columns.filter(
    (column) => profile.rowCount > 1 && column.uniqueCount <= 1 && column.nullCount < profile.rowCount,
  );
  const empty = profile.columns.filter(
    (column) => profile.rowCount > 0 && column.nullCount === profile.rowCount,
  );
  const highNull = profile.columns.filter(
    (column) => profile.rowCount > 0 && column.nullCount > 0 && column.nullCount < profile.rowCount &&
      column.nullCount * 100 >= profile.rowCount * 80,
  );
  const sentinels = profile.columns.filter((column) => (column.sentinelCount ?? 0) > 0);
  const nearDuplicates = profile.nearDuplicateRowCount > 0;
  const booleans = profile.columns.filter(
    (column) => column.suggestedType === "boolean" && (column.typeMatchPercentage ?? 0) >= 90,
  );
  const typeDrift = profile.columns.filter(
    (column) => (column.invalidTypeCount ?? 0) > 0,
  );
  const personal = profile.columns.filter((column) => column.privacySignal !== null);
  const hasSignals = profile.duplicateRowCount > 0 || nearDuplicates || incomplete.length > 0 || constant.length > 0 || empty.length > 0 || highNull.length > 0 || sentinels.length > 0 || booleans.length > 0 || typeDrift.length > 0 || personal.length > 0;

  return (
    <section className="prepare-card prepare-card--stacked cleaning-signals" aria-labelledby="cleaning-signals-title">
      <div>
        <p className="step">Catálogo de limpieza</p>
        <h3 id="cleaning-signals-title">Señales para revisar</h3>
        <p>Las señales usan solo esquema y métricas agregadas; no muestran celdas ni valores personales.</p>
      </div>
      {hasSignals ? (
        <>
          <ul className="cleaning-signals__list">
          {profile.duplicateRowCount > 0 && (
            <li><strong>Duplicados exactos:</strong> {profile.duplicateRowCount.toLocaleString()} filas adicionales; puedes eliminarlas de forma reversible.</li>
          )}
          {nearDuplicates && (
            <li><strong>Duplicados parecidos:</strong> {profile.nearDuplicateRowCount.toLocaleString()} filas adicionales coinciden al normalizar mayúsculas, espacios y acentos; requieren revisión manual.</li>
          )}
          {incomplete.length > 0 && (
            <li><strong>Completitud:</strong> {incomplete.length} {incomplete.length === 1 ? "columna tiene" : "columnas tienen"} al menos un nulo: {incomplete.map((column) => column.name).join(", ")}.</li>
          )}
          {constant.length > 0 && (
            <li><strong>Constantes:</strong> {constant.map((column) => column.name).join(", ")} {constant.length === 1 ? "no cambia" : "no cambian"} entre filas.</li>
          )}
          {empty.length > 0 && (
            <li><strong>Vacías:</strong> {empty.map((column) => column.name).join(", ")} no contiene valores en ninguna fila.</li>
          )}
          {highNull.length > 0 && (
            <li><strong>Alta nulidad:</strong> {highNull.map((column) => column.name).join(", ")} tiene al menos 80% de valores nulos.</li>
          )}
          {sentinels.length > 0 && (
            <li><strong>Valores centinela:</strong> {sentinels.map((column) => `${column.name} (${(column.sentinelCount ?? 0).toLocaleString()})`).join(", ")} usa tokens textuales que pueden representar datos ausentes.</li>
          )}
          {booleans.length > 0 && (
            <li><strong>Booleanos:</strong> {booleans.map((column) => column.name).join(", ")} admite alias textuales que pueden canonicalizarse como `true`/`false`.</li>
          )}
          {typeDrift.length > 0 && (
            <li><strong>Tipos sugeridos:</strong> {typeDrift.map((column) => column.name).join(", ")} contiene valores que no coinciden con la sugerencia detectada.</li>
          )}
          {personal.length > 0 && (
            <li className="cleaning-signals__privacy"><strong>Posible dato personal:</strong> revisa el tratamiento de {personal.map((column) => column.name).join(", ")} antes de exportar o compartir.</li>
          )}
          </ul>
          {constant.length > 0 && (
            <div className="cleaning-signals__action">
              <p>
                Puedes retirar estas columnas de baja información; se conservará al menos una
                columna para que el dataset siga siendo utilizable.
              </p>
              <button type="button" onClick={onRemoveConstantColumns} disabled={busy}>
                Eliminar columnas constantes
              </button>
            </div>
          )}
          {empty.length > 0 && (
            <div className="cleaning-signals__action">
              <p>
                Puedes retirar estas columnas sin información; se conservará al menos una
                columna para que el dataset siga siendo utilizable.
              </p>
              <button type="button" onClick={onRemoveEmptyColumns} disabled={busy}>
                Eliminar columnas vacías
              </button>
            </div>
          )}
          {highNull.length > 0 && (
            <div className="cleaning-signals__action">
              <p>
                Estas columnas tienen poca información disponible; la acción usa un umbral explícito
                de 80% y no elimina columnas completamente vacías ni todas las columnas del dataset.
              </p>
              <button type="button" onClick={onRemoveHighNullColumns} disabled={busy}>
                Eliminar columnas con alta nulidad
              </button>
            </div>
          )}
          {sentinels.length > 0 && (
            <div className="cleaning-signals__action">
              <p>
                Puedes convertir los tokens ausentes conocidos a valores nulos; la operación es
                reversible y no modifica números ni la columna de trazabilidad.
              </p>
              <button type="button" onClick={onNormalizeSentinels} disabled={busy}>
                Convertir centinelas a nulos
              </button>
            </div>
          )}
          {booleans.length > 0 && (
            <div className="cleaning-signals__action">
              <p>
                La normalización solo convierte `yes`/`no`, `sí`/`no` y `true`/`false` a
                `true`/`false`; deja intactos los valores que no reconoce.
              </p>
              <button type="button" onClick={onNormalizeBooleans} disabled={busy}>
                Normalizar booleanos
              </button>
            </div>
          )}
          {imputable.length > 0 && (
            <div className="cleaning-signals__action">
              <p>
                Intenta completar solo nulos: usa el valor textual más repetido cuando aparece al
                menos dos veces y la mediana observada para columnas numéricas. No modifica blancos,
                centinelas ni _cambios.
              </p>
              <button type="button" onClick={onImputeMissingValues} disabled={busy}>
                Intentar imputación conservadora
              </button>
            </div>
          )}
        </>
      ) : (
        <p className="notice notice--success" role="status">No se detectaron señales de limpieza en el perfil actual.</p>
      )}
    </section>
  );
}
