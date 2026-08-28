import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import * as bridge from "../bridge";
import { UpdatePanel } from "./UpdatePanel";

const update = {
  currentVersion: "0.57.0",
  version: "0.58.0",
  notes: "Correcciones de estabilidad",
  date: null,
  sizeBytes: 2048,
};

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe("UpdatePanel", () => {
  it("mantiene el updater cerrado y sin red cuando la compilación no está configurada", () => {
    const check = vi.spyOn(bridge, "checkForUpdate");

    render(<UpdatePanel enabled={false} currentVersion={null} />);

    expect(screen.getByText("El updater no está configurado para esta compilación.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Buscar actualizaciones" })).toBeDisabled();
    expect(check).not.toHaveBeenCalled();
  });

  it("comprueba solo por acción y comunica que no hay una versión nueva", async () => {
    const check = vi.spyOn(bridge, "checkForUpdate").mockResolvedValue(null);

    render(<UpdatePanel enabled currentVersion="0.57.0" />);

    fireEvent.click(screen.getByRole("button", { name: "Buscar actualizaciones" }));
    expect(check).toHaveBeenCalledOnce();
    expect(await screen.findByText("No hay actualizaciones disponibles.")).toBeInTheDocument();
  });

  it("muestra metadatos, progreso y permite instalar después de descargar", async () => {
    vi.spyOn(bridge, "checkForUpdate").mockResolvedValue(update);
    const download = vi.spyOn(bridge, "downloadUpdate").mockImplementation(async (onProgress) => {
      onProgress?.({ phase: "started", downloadedBytes: 0, contentLength: 2048 });
      onProgress?.({ phase: "progress", downloadedBytes: 2048, contentLength: 2048 });
    });
    const install = vi.spyOn(bridge, "installUpdate").mockResolvedValue(undefined);

    render(<UpdatePanel enabled currentVersion="0.57.0" />);
    fireEvent.click(screen.getByRole("button", { name: "Buscar actualizaciones" }));
    expect(await screen.findByText("Disponible: 0.58.0")).toBeInTheDocument();
    expect(screen.getByText("Correcciones de estabilidad")).toBeInTheDocument();
    expect(screen.getByText("Tamaño: 2.0 KB")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Descargar actualización" }));
    await waitFor(() => expect(download).toHaveBeenCalledOnce());
    expect(await screen.findByRole("button", { name: "Instalar y reiniciar" })).toBeInTheDocument();
    expect(screen.getByText("100% · 2.0 KB de 2.0 KB")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Instalar y reiniciar" }));
    await waitFor(() => expect(install).toHaveBeenCalledOnce());
    expect(await screen.findByText(/La instalación se inició/)).toBeInTheDocument();
  });

  it("cancela una descarga en curso", async () => {
    vi.spyOn(bridge, "checkForUpdate").mockResolvedValue(update);
    let resolveDownload: (() => void) | undefined;
    vi.spyOn(bridge, "downloadUpdate").mockImplementation(async (onProgress) => {
      onProgress?.({ phase: "progress", downloadedBytes: 512, contentLength: 2048 });
      await new Promise<void>((resolve) => { resolveDownload = resolve; });
    });
    const cancel = vi.spyOn(bridge, "cancelUpdateDownload").mockResolvedValue(undefined);

    render(<UpdatePanel enabled currentVersion="0.57.0" />);
    fireEvent.click(screen.getByRole("button", { name: "Buscar actualizaciones" }));
    fireEvent.click(await screen.findByRole("button", { name: "Descargar actualización" }));
    fireEvent.click(await screen.findByRole("button", { name: "Cancelar descarga" }));

    await waitFor(() => expect(cancel).toHaveBeenCalledOnce());
    expect(screen.getByRole("button", { name: "Cancelando…" })).toBeDisabled();
    resolveDownload?.();
  });
});
