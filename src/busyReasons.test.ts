import { describe, expect, it } from "vitest";
import { busyReasonCodes, busyReasonMessage, busyReasonPhrase, type BusyFlags } from "./busyReasons";

const idle: BusyFlags = {
  dataset: false, page: false, profile: false, prepare: false, qualityGate: false, exportRunning: false,
  comparison: false, join: false, reviewMutation: false, selection: null, project: false, autosave: false,
};

describe("busyReasons", () => {
  it("no da motivo si no hay nada en curso", () => {
    expect(busyReasonCodes(idle)).toBe("");
    expect(busyReasonPhrase(idle)).toBeNull();
    expect(busyReasonMessage(idle)).toBe("");
  });

  it("explica el análisis de calidad y conserva los nombres para las sondas", () => {
    const flags = { ...idle, profile: true };
    expect(busyReasonCodes(flags)).toBe("profile");
    expect(busyReasonMessage(flags)).toBe("Disponible al terminar el análisis de calidad.");
  });

  it("muestra el primer motivo y lista todos en el atributo", () => {
    const flags = { ...idle, exportRunning: true, selection: "inspecting+finalizing", autosave: true };
    expect(busyReasonCodes(flags)).toBe("export,selection:inspecting+finalizing,autosave");
    expect(busyReasonPhrase(flags)).toBe("la exportación");
  });
});
