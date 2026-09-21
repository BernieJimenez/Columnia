import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { saveDiagnosticReport } from "../../bridge/diagnostics";
import { DiagnosticsDialog } from "./DiagnosticsDialog";

vi.mock("../../bridge/diagnostics", () => ({
  saveDiagnosticReport: vi.fn(),
}));

const datasetMetrics = { rowCount: 42_500, columnCount: 16, fileSizeBytes: 200 * 1024 ** 2 };

function openIssuePreview() {
  fireEvent.change(screen.getByLabelText("Estado que quieres informar"), {
    target: { value: "issue_reported" },
  });
  fireEvent.click(screen.getByLabelText("Aplicación de transformación"));
  fireEvent.click(screen.getByLabelText(/Incluir rangos agregados/));
  fireEvent.click(screen.getByRole("button", { name: "Crear vista previa" }));
}

describe("informe de diagnóstico local", () => {
  const onDismiss = vi.fn();

  beforeEach(() => {
    vi.mocked(saveDiagnosticReport).mockReset();
    onDismiss.mockReset();
  });

  afterEach(() => cleanup());

  it("requiere una acción explícita para generar y revisar el contrato de allowlist", () => {
    render(
      <DiagnosticsDialog
        appVersion="1.25.0"
        activePhase="prepare"
        datasetMetrics={datasetMetrics}
        onDismiss={onDismiss}
      />,
    );

    expect(screen.queryByLabelText("Contenido exacto del diagnóstico")).not.toBeInTheDocument();
    expect(saveDiagnosticReport).not.toHaveBeenCalled();
    openIssuePreview();

    const preview = screen.getByLabelText("Contenido exacto del diagnóstico");
    const payload = JSON.parse(preview.textContent ?? "{}") as Record<string, unknown>;
    expect(payload).toMatchObject({
      contract: "columnia-diagnostic-report",
      schemaVersion: 1,
      phase: "prepare",
      errorCodes: ["TRANSFORM_APPLY_FAILED"],
      metrics: { rows: "10k_to_99k", columns: "10_to_49", sourceSize: "100_to_999_mib" },
    });
    expect(preview.textContent).not.toMatch(/errorMessage|path|query|fileName|customer|secret|email|stackTrace|machineId/i);
    expect(saveDiagnosticReport).not.toHaveBeenCalled();
  });

  it("permite cancelar antes del preview sin abrir el guardado local", () => {
    render(
      <DiagnosticsDialog
        appVersion="1.25.0"
        activePhase="review"
        datasetMetrics={null}
        onDismiss={onDismiss}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Cancelar" }));
    expect(onDismiss).toHaveBeenCalledOnce();
    expect(saveDiagnosticReport).not.toHaveBeenCalled();
  });

  it("trata la cancelación del selector nativo como una cancelación sin escritura", async () => {
    vi.mocked(saveDiagnosticReport).mockResolvedValue(null);
    render(
      <DiagnosticsDialog
        appVersion="1.25.0"
        activePhase="prepare"
        datasetMetrics={datasetMetrics}
        onDismiss={onDismiss}
      />,
    );
    openIssuePreview();

    fireEvent.click(screen.getByRole("button", { name: "Guardar archivo local" }));
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("No se creó ningún archivo"));
    expect(saveDiagnosticReport).toHaveBeenCalledOnce();
    expect(onDismiss).not.toHaveBeenCalled();
  });

  it("exporta solo el reporte revisado mediante el bridge local", async () => {
    vi.mocked(saveDiagnosticReport).mockResolvedValue(undefined);
    render(
      <DiagnosticsDialog
        appVersion="1.25.0"
        activePhase="deliver"
        datasetMetrics={null}
        onDismiss={onDismiss}
      />,
    );
    fireEvent.change(screen.getByLabelText("Estado que quieres informar"), {
      target: { value: "issue_reported" },
    });
    fireEvent.click(screen.getByLabelText("Exportación de archivo local"));
    fireEvent.click(screen.getByRole("button", { name: "Crear vista previa" }));
    const preview = screen.getByLabelText("Contenido exacto del diagnóstico");
    const reviewedJson = preview.textContent;

    fireEvent.click(screen.getByRole("button", { name: "Guardar archivo local" }));
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("guardado localmente"));
    expect(saveDiagnosticReport).toHaveBeenCalledWith(JSON.parse(reviewedJson ?? "{}"));
  });

  it("no cierra por Escape mientras espera el selector o escribe el informe", async () => {
    let finishSave: (() => void) | undefined;
    vi.mocked(saveDiagnosticReport).mockImplementation(() => new Promise<void>((resolve) => {
      finishSave = resolve;
    }));
    render(
      <DiagnosticsDialog
        appVersion="1.25.0"
        activePhase="prepare"
        datasetMetrics={datasetMetrics}
        onDismiss={onDismiss}
      />,
    );
    openIssuePreview();

    fireEvent.click(screen.getByRole("button", { name: "Guardar archivo local" }));
    expect(await screen.findByRole("button", { name: "Guardando…" })).toBeDisabled();
    fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
    expect(onDismiss).not.toHaveBeenCalled();

    finishSave?.();
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("guardado localmente"));
  });
});
