import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type {
  ReusableTask,
  ReusableTaskSchemaCompatibility,
  ReusableTaskSummary,
} from "../../bridge";
import { ReusableTaskPanel } from "./ReusableTaskPanel";

const bridge = vi.hoisted(() => ({
  checkReusableTaskSchema: vi.fn(),
  deleteReusableTask: vi.fn(),
  listReusableTasks: vi.fn(),
  openReusableTask: vi.fn(),
  saveReusableTask: vi.fn(),
}));

vi.mock("../../bridge", () => bridge);

afterEach(cleanup);

const schema = [{ name: "id", dataType: "Int64" }];

const savedTask: ReusableTask = {
  version: 1,
  name: "Cierre mensual",
  importProfile: {
    version: 1,
    format: "csv",
    schema: [{ name: "id", dataType: "Int64" }],
  },
  recipe: null,
  qualityRules: [{ column: "id", kind: "not_null", maxInvalid: 0 }],
  outputFormat: "csv",
  privacyMode: "mask",
};

const taskWithConversionPolicy: ReusableTask = {
  ...savedTask,
  recipe: {
    version: 1,
    name: "Normalizar importes",
    savedAt: "2026-09-01T10:00:00Z",
    recipe: {
      renames: [],
      casts: [{ column: "id", target: "integer" }],
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
    },
  },
  exceptionPolicy: {
    version: 1,
    baseline: "lexical",
    schema,
    conversions: [{ kind: "cast", column: "id", target: "integer", onInvalid: "review" }],
  },
};

const taskSummary: ReusableTaskSummary = {
  id: "task-1",
  name: savedTask.name,
  createdAt: "2026-09-01T10:00:00Z",
  updatedAt: "2026-09-01T10:00:00Z",
  inputColumnCount: 1,
  hasRecipe: false,
  qualityRuleCount: 1,
  outputFormat: "csv",
};

const readyCompatibility: ReusableTaskSchemaCompatibility = {
  status: "ready",
  missingColumns: [],
  addedColumns: [],
  changedTypes: [],
  orderChanged: false,
};

const draft: Omit<ReusableTask, "name"> = {
  version: 1,
  importProfile: savedTask.importProfile,
  recipe: null,
  qualityRules: savedTask.qualityRules,
  outputFormat: "csv",
  privacyMode: "mask",
};

beforeEach(() => {
  vi.clearAllMocks();
  bridge.listReusableTasks.mockResolvedValue([taskSummary]);
  bridge.openReusableTask.mockResolvedValue(savedTask);
  bridge.checkReusableTaskSchema.mockResolvedValue(readyCompatibility);
  bridge.saveReusableTask.mockResolvedValue({ ...taskSummary, id: "task-2", name: "Cierre semanal" });
});

describe("ReusableTaskPanel", () => {
  it("muestra las conversiones guardadas como borrador y conserva la confirmación existente", async () => {
    bridge.openReusableTask.mockResolvedValue(taskWithConversionPolicy);
    const onApply = vi.fn();
    render(
      <ReusableTaskPanel
        connected
        blocked={false}
        schema={schema}
        draft={draft}
        onApply={onApply}
      />,
    );
    await waitFor(() => expect(bridge.listReusableTasks).toHaveBeenCalledOnce());
    fireEvent.click(screen.getByText("Reutilizar una tarea"));
    fireEvent.change(screen.getByLabelText("Tarea guardada"), { target: { value: taskSummary.id } });

    expect(await screen.findByText(/1 decisión · base léxica/)).toBeInTheDocument();
    expect(screen.getByText(/quedan preseleccionadas como borrador/)).toBeInTheDocument();
    const apply = screen.getByRole("button", { name: "Aplicar al dataset actual" });
    await waitFor(() => expect(apply).toBeEnabled());
    fireEvent.click(apply);
    expect(onApply).toHaveBeenCalledWith(taskWithConversionPolicy);
  });

  it("no ejecuta silenciosamente una política con acciones incompatibles disponibles", async () => {
    bridge.openReusableTask.mockResolvedValue({
      ...taskWithConversionPolicy,
      exceptionPolicy: {
        ...taskWithConversionPolicy.exceptionPolicy!,
        conversions: [{ kind: "cast", column: "id", target: "integer", onInvalid: "excludeRow" }],
      },
    });
    render(
      <ReusableTaskPanel
        connected
        blocked={false}
        schema={schema}
        draft={draft}
        onApply={vi.fn()}
      />,
    );
    await waitFor(() => expect(bridge.listReusableTasks).toHaveBeenCalledOnce());
    fireEvent.click(screen.getByText("Reutilizar una tarea"));
    fireEvent.change(screen.getByLabelText("Tarea guardada"), { target: { value: taskSummary.id } });

    expect(await screen.findByRole("alert")).toHaveTextContent("todavía no está disponible");
    expect(screen.getByRole("button", { name: "Aplicar al dataset actual" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Preparar próxima importación" })).toBeDisabled();
  });

  it("permanece plegado y revisa una tarea antes de permitir su uso en el dataset actual", async () => {
    const onApply = vi.fn();
    render(
      <ReusableTaskPanel
        connected
        blocked={false}
        schema={schema}
        draft={draft}
        onApply={onApply}
      />,
    );

    const disclosure = screen.getByText("Reutilizar una tarea").closest("details");
    expect(disclosure?.open).toBe(false);
    await waitFor(() => expect(bridge.listReusableTasks).toHaveBeenCalledOnce());
    fireEvent.click(screen.getByText("Reutilizar una tarea"));

    fireEvent.change(screen.getByLabelText("Tarea guardada"), { target: { value: taskSummary.id } });
    const apply = screen.getByRole("button", { name: "Aplicar al dataset actual" });
    expect(apply).toBeDisabled();

    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("El esquema es compatible"));
    expect(bridge.openReusableTask).toHaveBeenCalledWith(taskSummary.id);
    expect(bridge.checkReusableTaskSchema).toHaveBeenCalledWith(taskSummary.id, schema);
    expect(apply).toBeEnabled();

    fireEvent.click(apply);
    expect(onApply).toHaveBeenCalledWith(savedTask);
  });

  it("muestra diferencias y bloquea la aplicación si el esquema requiere revisión", async () => {
    bridge.checkReusableTaskSchema.mockResolvedValue({
      status: "review_required",
      missingColumns: ["id"],
      addedColumns: ["customer"],
      changedTypes: [{ column: "amount", expected: "Float64", actual: "String" }],
      orderChanged: true,
    } satisfies ReusableTaskSchemaCompatibility);
    const onApply = vi.fn();
    render(
      <ReusableTaskPanel
        connected
        blocked={false}
        schema={schema}
        draft={draft}
        onApply={onApply}
      />,
    );
    await waitFor(() => expect(bridge.listReusableTasks).toHaveBeenCalledOnce());
    fireEvent.click(screen.getByText("Reutilizar una tarea"));
    fireEvent.change(screen.getByLabelText("Tarea guardada"), { target: { value: taskSummary.id } });

    const mismatch = await screen.findByRole("alert");
    expect(mismatch).toHaveTextContent("El esquema cambió");
    expect(mismatch).toHaveTextContent("Falta la columna “id”.");
    expect(mismatch).toHaveTextContent("“amount” cambió de Float64 a String.");
    expect(mismatch).toHaveTextContent("Cambió el orden de las columnas.");
    expect(screen.getByRole("button", { name: "Aplicar al dataset actual" })).toBeDisabled();
    expect(onApply).not.toHaveBeenCalled();
  });

  it("permite preparar una tarea sin esquema para que su perfil gobierne la próxima importación", async () => {
    const onPrepareImport = vi.fn();
    render(
      <ReusableTaskPanel
        connected
        blocked={false}
        schema={null}
        draft={draft}
        onApply={vi.fn()}
        onPrepareImport={onPrepareImport}
      />,
    );
    await waitFor(() => expect(bridge.listReusableTasks).toHaveBeenCalledOnce());
    fireEvent.click(screen.getByText("Reutilizar una tarea"));
    fireEvent.change(screen.getByLabelText("Tarea guardada"), { target: { value: taskSummary.id } });

    expect(await screen.findByRole("status")).toHaveTextContent("Abre un archivo para comprobar su esquema");
    expect(bridge.openReusableTask).toHaveBeenCalledWith(taskSummary.id);
    expect(bridge.checkReusableTaskSchema).not.toHaveBeenCalled();
    expect(screen.getByText("Configuración que se reutilizará")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Preparar próxima importación" })).toBeEnabled();

    fireEvent.click(screen.getByRole("button", { name: "Preparar próxima importación" }));
    expect(onPrepareImport).toHaveBeenCalledWith(taskSummary.id, savedTask);
  });

  it("abre y prepara inmediatamente la configuración que acaba de guardar", async () => {
    const onPrepareImport = vi.fn();
    render(
      <ReusableTaskPanel
        connected
        blocked={false}
        schema={schema}
        draft={draft}
        onApply={vi.fn()}
        onPrepareImport={onPrepareImport}
      />,
    );
    await waitFor(() => expect(bridge.listReusableTasks).toHaveBeenCalledOnce());
    fireEvent.click(screen.getByText("Reutilizar una tarea"));
    fireEvent.change(screen.getByLabelText("Guardar configuración actual"), {
      target: { value: "  Cierre semanal  " },
    });
    fireEvent.click(screen.getByRole("button", { name: "Guardar" }));

    expect(bridge.saveReusableTask).toHaveBeenCalledWith(null, { ...draft, name: "Cierre semanal" });
    expect(await screen.findByText("Tarea “Cierre semanal” guardada en este equipo.")).toHaveAttribute(
      "role",
      "status",
    );
    expect(screen.getByText("Configuración que se reutilizará")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Preparar próxima importación" })).toBeEnabled();
    expect(screen.queryByRole("button", { name: /eliminar/i })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Preparar próxima importación" }));

    expect(onPrepareImport).toHaveBeenCalledWith("task-2", { ...draft, name: "Cierre semanal" });
  });

  it("bloquea los controles de tarea mientras el flujo externo está ocupado", async () => {
    render(
      <ReusableTaskPanel
        connected
        blocked
        schema={schema}
        draft={draft}
        onApply={vi.fn()}
      />,
    );
    await waitFor(() => expect(bridge.listReusableTasks).toHaveBeenCalledOnce());
    fireEvent.click(screen.getByText("Reutilizar una tarea"));
    expect(screen.getByLabelText("Tarea guardada")).toBeDisabled();
    expect(screen.getByRole("button", { name: "Guardar" })).toBeDisabled();
    expect(bridge.openReusableTask).not.toHaveBeenCalled();
    expect(bridge.saveReusableTask).not.toHaveBeenCalled();
  });
});
