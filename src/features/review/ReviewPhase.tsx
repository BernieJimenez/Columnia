import { OperationProgressView } from "../../components/OperationProgressView";
import { ReviewTabList, type ReviewTab } from "../../components/ReviewTabList";
import type { DatasetPreview, DatasetProfile } from "../../bridge";
import { DatasetMetrics } from "../delivery/DatasetMetrics";
import type { ReadyDatasetStatus } from "../load/loadModel";
import {
  nextPageOffset,
  pageRange,
  previousPageOffset,
  type ProfileStatus,
} from "./reviewModel";
import type { ComparisonStatus } from "./compareModel";

interface ReviewPhaseProps {
  datasetStatus: ReadyDatasetStatus;
  profileStatus: ProfileStatus;
  reviewTab: ReviewTab;
  onTabChange: (tab: ReviewTab) => void;
  onPageChange: (offset: number) => void;
  onAnalyzeQuality: () => void;
  onCancelProfile: () => void;
  comparisonStatus: ComparisonStatus;
  onCompare: () => void;
  onClearComparison: () => void;
  onConsolidate: () => void;
}

export function ReviewPhase({
  datasetStatus,
  profileStatus,
  reviewTab,
  onTabChange,
  onPageChange,
  onAnalyzeQuality,
  onCancelProfile,
  comparisonStatus,
  onCompare,
  onClearComparison,
  onConsolidate,
}: ReviewPhaseProps) {
  return (
    <>
      <header className="phase-header phase-header--compact">
        <div>
          <p className="eyebrow">Revisar · Dataset activo</p>
          <h2>{datasetStatus.dataset.fileName}</h2>
          <p>Comprueba la estructura, la calidad y una muestra de los datos antes de modificarlos.</p>
        </div>
      </header>
      <ReviewTabList activeTab={reviewTab} onTabChange={onTabChange} />
      <DatasetComparisonSection
        status={comparisonStatus}
        onCompare={onCompare}
        onClear={onClearComparison}
        onConsolidate={onConsolidate}
      />

      {reviewTab === "diagnosis" ? (
        <div id="review-diagnosis-panel" role="tabpanel" aria-labelledby="review-diagnosis-tab">
          <QualitySection
            dataset={datasetStatus.dataset}
            status={profileStatus}
            onAnalyze={onAnalyzeQuality}
            onCancel={onCancelProfile}
          />
        </div>
      ) : (
        <div id="review-preview-panel" role="tabpanel" aria-labelledby="review-preview-tab">
          <DataPreview
            dataset={datasetStatus.dataset}
            pageOffset={datasetStatus.pageOffset}
            pageLoading={datasetStatus.pageLoading}
            pageError={datasetStatus.pageError}
            onPageChange={onPageChange}
          />
        </div>
      )}
    </>
  );
}

function DatasetComparisonSection({
  status,
  onCompare,
  onClear,
  onConsolidate,
}: {
  status: ComparisonStatus;
  onCompare: () => void;
  onClear: () => void;
  onConsolidate: () => void;
}) {
  return (
    <section className="phase-section comparison-section" aria-labelledby="comparison-title">
      <div className="section-heading">
        <div>
          <p className="step">Paridad de fuentes</p>
          <h3 id="comparison-title">Comparar datasets</h3>
        </div>
        <button type="button" onClick={onCompare} disabled={status.kind === "loading"}>
          {status.kind === "loading" ? "Comparando…" : "Elegir dataset para comparar"}
        </button>
      </div>
      <p className="profile-note">
        Contrasta filas como conjunto multivaluado y conserva el dataset activo hasta que decidas consolidar.
      </p>
      {status.kind === "loading" && (
        <p className="notice" role="status">Leyendo la segunda fuente local…</p>
      )}
      {status.kind === "error" && (
        <p className="notice notice--error" role="alert">
          No se pudo comparar la fuente: {status.message}
        </p>
      )}
      {status.kind === "ready" && (
        <>
          <p className="comparison-source" aria-live="polite">
            <strong>{status.comparison.currentFileName}</strong>
            <span aria-hidden="true"> ↔ </span>
            <strong>{status.comparison.comparedFileName}</strong>
          </p>
          <dl className="quality-summary" aria-label="Resumen de comparación">
            <div><dt>Filas compartidas</dt><dd>{status.comparison.commonRowCount.toLocaleString()}</dd></div>
            <div><dt>Solo en el activo</dt><dd>{status.comparison.currentOnlyRowCount.toLocaleString()}</dd></div>
            <div><dt>Solo en el comparado</dt><dd>{status.comparison.comparedOnlyRowCount.toLocaleString()}</dd></div>
          </dl>
          <div className="comparison-columns" aria-label="Resultado de columnas">
            <div>
              <h4>Columnas compartidas ({status.comparison.sharedColumns.length})</h4>
              <p>{status.comparison.sharedColumns.join(", ") || "Ninguna"}</p>
            </div>
            <div>
              <h4>Solo en el activo ({status.comparison.currentOnlyColumns.length})</h4>
              <p>{status.comparison.currentOnlyColumns.join(", ") || "Ninguna"}</p>
            </div>
            <div>
              <h4>Solo en el comparado ({status.comparison.comparedOnlyColumns.length})</h4>
              <p>{status.comparison.comparedOnlyColumns.join(", ") || "Ninguna"}</p>
            </div>
          </div>
          <div className="comparison-actions">
            <button type="button" className="secondary-action" onClick={onClear}>Descartar comparación</button>
            <button type="button" className="primary-action" onClick={onConsolidate} disabled={!status.comparison.canConsolidate}>
              Consolidar filas
            </button>
          </div>
          {!status.comparison.canConsolidate && (
            <p className="notice" role="note">
              La consolidación requiere las mismas columnas en el mismo orden y con los mismos tipos.
            </p>
          )}
        </>
      )}
    </section>
  );
}

function QualitySection({
  dataset,
  status,
  onAnalyze,
  onCancel,
}: {
  dataset: DatasetPreview;
  status: ProfileStatus;
  onAnalyze: () => void;
  onCancel: () => void;
}) {
  return (
    <section className="phase-section" aria-labelledby="quality-title">
      <div className="section-heading">
        <div>
          <p className="step">Calidad inicial</p>
          <h3 id="quality-title">Perfil por columna</h3>
        </div>
        {status.kind !== "loading" && (
          <button type="button" onClick={onAnalyze}>
            {status.kind === "ready" ? "Analizar de nuevo" : "Analizar calidad"}
          </button>
        )}
      </div>
      <DatasetMetrics dataset={dataset} />
      {status.kind === "loading" && (
        <OperationProgressView
          progress={status.progress}
          cancellation={status.cancelRequested
            ? { kind: "requested" }
            : { kind: "available", onCancel }}
        />
      )}
      {status.kind === "error" && (
        <p className="notice notice--error" role="alert">
          No se pudo analizar la calidad: {status.message}
        </p>
      )}
      {status.kind === "ready" && <QualityProfile profile={status.profile} />}
    </section>
  );
}

interface DataPreviewProps {
  dataset: DatasetPreview;
  pageOffset: number;
  pageLoading: boolean;
  pageError?: string;
  onPageChange: (offset: number) => void;
}

export function DataPreview({
  dataset,
  pageOffset,
  pageLoading,
  pageError,
  onPageChange,
}: DataPreviewProps) {
  const { end: pageEnd, hasPrevious, hasNext } = pageRange(dataset, pageOffset);

  return (
    <>
      <div className="table-region" tabIndex={0} aria-label="Vista previa del dataset">
        <table>
          <thead>
            <tr>
              {dataset.columns.map((column) => (
                <th key={column.name} scope="col">
                  <span>{column.name}</span>
                  <small>{column.dataType}</small>
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {dataset.rows.map((row, rowIndex) => (
              <tr key={pageOffset + rowIndex}>
                {row.map((value, columnIndex) => (
                  <td key={columnIndex}>{value ?? <span className="null-value">null</span>}</td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <div className="pagination" aria-label="Paginación de la vista previa">
        <p className="preview-note" aria-live="polite">
          {dataset.rowCount === 0
            ? "El dataset no contiene filas."
            : `Filas ${pageOffset + 1}–${pageEnd} de ${dataset.rowCount.toLocaleString()}`}
        </p>
        <div>
          <button
            type="button"
            onClick={() => onPageChange(previousPageOffset(pageOffset))}
            disabled={!hasPrevious || pageLoading}
          >
            Anterior
          </button>
          <button
            type="button"
            onClick={() => onPageChange(nextPageOffset(pageOffset))}
            disabled={!hasNext || pageLoading}
          >
            {pageLoading ? "Cargando…" : "Siguiente"}
          </button>
        </div>
      </div>
      {pageError && (
        <p className="notice notice--error" role="alert">
          No se pudo cambiar de página: {pageError}
        </p>
      )}
    </>
  );
}

function QualityProfile({ profile }: { profile: DatasetProfile }) {
  const textColumns = profile.columns.filter((column) => column.emptyCount !== null);
  const numericColumns = profile.columns.filter((column) => column.outlierCount !== null);

  return (
    <>
      <dl className="quality-summary" aria-label="Resumen de calidad del dataset">
        <div>
          <dt>Filas duplicadas adicionales</dt>
          <dd>
            {profile.duplicateRowCount.toLocaleString()} ({profile.duplicatePercentage.toFixed(1)}%)
          </dd>
        </div>
        <div>
          <dt>Filas analizadas</dt>
          <dd>{profile.rowCount.toLocaleString()}</dd>
        </div>
      </dl>
      <QualityVisuals profile={profile} />
      <div
        className="profile-region"
        role="region"
        tabIndex={0}
        aria-label="Perfil de calidad por columna"
      >
        <table>
          <thead>
            <tr>
              <th scope="col">Columna</th>
              <th scope="col">Completitud</th>
              <th scope="col">Nulos</th>
              <th scope="col">Únicos</th>
              <th scope="col">Mínimo</th>
              <th scope="col">Máximo</th>
              <th scope="col">Promedio</th>
            </tr>
          </thead>
          <tbody>
            {profile.columns.map((column) => (
              <tr key={column.name}>
                <th scope="row">
                  <span>{column.name}</span>
                  <small>{column.dataType}</small>
                </th>
                <td>{column.completenessPercentage.toFixed(1)}%</td>
                <td>{column.nullCount.toLocaleString()}</td>
                <td>{column.uniqueCount.toLocaleString()}</td>
                <td>{column.minimum ?? "—"}</td>
                <td>{column.maximum ?? "—"}</td>
                <td>
                  {column.mean === null
                    ? "—"
                    : column.mean.toLocaleString(undefined, { maximumFractionDigits: 3 })}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <p className="profile-note">El conteo de valores únicos excluye los nulos.</p>
      {numericColumns.length > 0 && (
        <>
          <h4 className="text-profile-title">Detalle de columnas numéricas</h4>
          <div
            className="profile-region profile-region--detail"
            role="region"
            tabIndex={0}
            aria-label="Perfil de columnas numéricas"
          >
            <table>
              <thead>
                <tr>
                  <th scope="col">Columna</th>
                  <th scope="col">Desv. estándar</th>
                  <th scope="col">Q1</th>
                  <th scope="col">Mediana</th>
                  <th scope="col">Q3</th>
                  <th scope="col">Posibles outliers</th>
                </tr>
              </thead>
              <tbody>
                {numericColumns.map((column) => (
                  <tr key={column.name}>
                    <th scope="row">{column.name}</th>
                    <td>{formatStatistic(column.standardDeviation)}</td>
                    <td>{formatStatistic(column.firstQuartile)}</td>
                    <td>{formatStatistic(column.median)}</td>
                    <td>{formatStatistic(column.thirdQuartile)}</td>
                    <td>{column.outlierCount?.toLocaleString() ?? "—"}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <p className="profile-note">
            Posibles outliers usa la regla IQR de 1.5× y requiere al menos cuatro valores. La
            desviación estándar es muestral.
          </p>
        </>
      )}
      {textColumns.length > 0 && (
        <>
          <h4 className="text-profile-title">Detalle de columnas de texto</h4>
          <div
            className="profile-region profile-region--detail"
            role="region"
            tabIndex={0}
            aria-label="Perfil de columnas de texto"
          >
            <table>
              <thead>
                <tr>
                  <th scope="col">Columna</th>
                  <th scope="col">Vacíos</th>
                  <th scope="col">Longitud mínima</th>
                  <th scope="col">Longitud máxima</th>
                  <th scope="col">Longitud promedio</th>
                  <th scope="col">Tipo sugerido</th>
                  <th scope="col">Coincidencia</th>
                  <th scope="col">No coinciden</th>
                </tr>
              </thead>
              <tbody>
                {textColumns.map((column) => (
                  <tr key={column.name}>
                    <th scope="row">{column.name}</th>
                    <td>{column.emptyCount?.toLocaleString()}</td>
                    <td>{column.minimumLength?.toLocaleString() ?? "—"}</td>
                    <td>{column.maximumLength?.toLocaleString() ?? "—"}</td>
                    <td>
                      {column.averageLength?.toLocaleString(undefined, {
                        maximumFractionDigits: 1,
                      }) ?? "—"}
                    </td>
                    <td>{suggestedTypeLabel(column.suggestedType)}</td>
                    <td>
                      {column.typeMatchPercentage === null
                        ? "—"
                        : `${column.typeMatchPercentage.toFixed(1)}%`}
                    </td>
                    <td>{column.invalidTypeCount?.toLocaleString() ?? "—"}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <p className="profile-note">
            “Vacíos” incluye cadenas sin caracteres o compuestas solamente por espacios. Las
            sugerencias requieren al menos tres valores y una coincidencia del 90%.
          </p>
        </>
      )}
    </>
  );
}

function QualityVisuals({ profile }: { profile: DatasetProfile }) {
  const numericColumns = profile.columns.filter((column) => column.outlierCount !== null);
  const maxOutlierCount = Math.max(
    1,
    ...numericColumns.map((column) => Math.max(0, column.outlierCount ?? 0)),
  );

  if (profile.columns.length === 0) return null;

  return (
    <section className="quality-visuals" aria-labelledby="quality-visuals-title">
      <div className="quality-visuals__heading">
        <div>
          <p className="step">Lectura rápida</p>
          <h4 id="quality-visuals-title">Señales del perfil</h4>
        </div>
        <p>
          Las barras ayudan a detectar patrones; las tablas de abajo conservan los valores exactos
          y el equivalente para lector de pantalla.
        </p>
      </div>
      <div className="quality-chart-grid">
        <div className="quality-chart" role="group" aria-labelledby="quality-completeness-title">
          <h5 id="quality-completeness-title">Completitud por columna</h5>
          <p className="quality-chart__note">Porcentaje de filas con un valor no nulo.</p>
          <div className="quality-chart__bars" role="list" aria-label="Completitud por columna">
            {profile.columns.map((column) => {
              const percentage = clampPercentage(column.completenessPercentage);

              return (
                <div className="quality-chart__item" role="listitem" key={column.name}>
                  <div className="quality-chart__label">
                    <span title={column.name}>{column.name}</span>
                    <strong>{percentage.toFixed(1)}%</strong>
                  </div>
                  <div className="quality-chart__track" aria-hidden="true">
                    <span style={{ width: `${percentage}%` }} />
                  </div>
                </div>
              );
            })}
          </div>
        </div>
        {numericColumns.length > 0 && (
          <div className="quality-chart" role="group" aria-labelledby="quality-outliers-title">
            <h5 id="quality-outliers-title">Posibles outliers</h5>
            <p className="quality-chart__note">Filas fuera del rango IQR de 1.5×.</p>
            <div className="quality-chart__bars" role="list" aria-label="Posibles outliers por columna">
              {numericColumns.map((column) => {
                const count = Math.max(0, column.outlierCount ?? 0);
                const percentage = (count / maxOutlierCount) * 100;

                return (
                  <div className="quality-chart__item" role="listitem" key={column.name}>
                    <div className="quality-chart__label">
                      <span title={column.name}>{column.name}</span>
                      <strong>{count.toLocaleString()}</strong>
                    </div>
                    <div className="quality-chart__track" aria-hidden="true">
                      <span style={{ width: `${percentage}%` }} />
                    </div>
                  </div>
                );
              })}
            </div>
          </div>
        )}
      </div>
    </section>
  );
}

function clampPercentage(value: number): number {
  if (!Number.isFinite(value)) return 0;
  return Math.min(100, Math.max(0, value));
}

function suggestedTypeLabel(type: string | null): string {
  switch (type) {
    case "boolean":
      return "Booleano";
    case "integer":
      return "Entero";
    case "decimal":
      return "Decimal";
    case "date":
      return "Fecha";
    default:
      return "—";
  }
}

function formatStatistic(value: number | null): string {
  return value?.toLocaleString(undefined, { maximumFractionDigits: 3 }) ?? "—";
}
