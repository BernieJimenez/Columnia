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

  it("anuncia un fallo al comprobar y permite volver a intentarlo", async () => {
    const check = vi.spyOn(bridge, "checkForUpdate")
      .mockRejectedValueOnce(new Error("No se pudo comprobar."))
      .mockResolvedValueOnce(null);

    render(<UpdatePanel enabled currentVersion="0.57.0" />);

    fireEvent.click(screen.getByRole("button", { name: "Buscar actualizaciones" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("No se pudo comprobar.");

    fireEvent.click(screen.getByRole("button", { name: "Buscar actualizaciones" }));
    expect(await screen.findByText("No hay actualizaciones disponibles.")).toBeInTheDocument();
    expect(check).toHaveBeenCalledTimes(2);
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
    expect(screen.getByText(/^Tamaño: 2[.,]0 KiB$/)).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Descargar actualización" }));
    await waitFor(() => expect(download).toHaveBeenCalledOnce());
    expect(await screen.findByRole("button", { name: "Instalar y reiniciar" })).toBeInTheDocument();
    expect(screen.getByText(/^100% · 2[.,]0 KiB de 2[.,]0 KiB$/)).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Instalar y reiniciar" }));
    await waitFor(() => expect(install).toHaveBeenCalledOnce());
    expect(await screen.findByText(/La instalación se inició/)).toBeInTheDocument();
  });

  it("anuncia un fallo de descarga y permite reintentar tras comprobar de nuevo", async () => {
    vi.spyOn(bridge, "checkForUpdate").mockResolvedValue(update);
    const download = vi.spyOn(bridge, "downloadUpdate")
      .mockRejectedValueOnce(new Error("No se pudo descargar."))
      .mockImplementationOnce(async (onProgress) => {
        onProgress?.({ phase: "finished", downloadedBytes: 2048, contentLength: 2048 });
      });

    render(<UpdatePanel enabled currentVersion="0.57.0" />);
    fireEvent.click(screen.getByRole("button", { name: "Buscar actualizaciones" }));
    fireEvent.click(await screen.findByRole("button", { name: "Descargar actualización" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("No se pudo descargar.");

    fireEvent.click(screen.getByRole("button", { name: "Buscar actualizaciones" }));
    fireEvent.click(await screen.findByRole("button", { name: "Descargar actualización" }));
    expect(await screen.findByRole("button", { name: "Instalar y reiniciar" })).toBeInTheDocument();
    expect(download).toHaveBeenCalledTimes(2);
  });

  it("anuncia un fallo de instalación y permite reintentar el flujo", async () => {
    vi.spyOn(bridge, "checkForUpdate").mockResolvedValue(update);
    vi.spyOn(bridge, "downloadUpdate").mockImplementation(async (onProgress) => {
      onProgress?.({ phase: "finished", downloadedBytes: 2048, contentLength: 2048 });
    });
    const install = vi.spyOn(bridge, "installUpdate")
      .mockRejectedValueOnce(new Error("No se pudo instalar."))
      .mockResolvedValue(undefined);

    render(<UpdatePanel enabled currentVersion="0.57.0" />);
    fireEvent.click(screen.getByRole("button", { name: "Buscar actualizaciones" }));
    fireEvent.click(await screen.findByRole("button", { name: "Descargar actualización" }));
    fireEvent.click(await screen.findByRole("button", { name: "Instalar y reiniciar" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("No se pudo instalar.");

    fireEvent.click(screen.getByRole("button", { name: "Buscar actualizaciones" }));
    fireEvent.click(await screen.findByRole("button", { name: "Descargar actualización" }));
    fireEvent.click(await screen.findByRole("button", { name: "Instalar y reiniciar" }));
    expect(await screen.findByText(/La instalación se inició/)).toBeInTheDocument();
    expect(install).toHaveBeenCalledTimes(2);
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

  it("anuncia si falla la solicitud de cancelación y conserva el resultado de la descarga", async () => {
    vi.spyOn(bridge, "checkForUpdate").mockResolvedValue(update);
    let resolveDownload: (() => void) | undefined;
    vi.spyOn(bridge, "downloadUpdate").mockImplementation(async (onProgress) => {
      onProgress?.({ phase: "progress", downloadedBytes: 512, contentLength: 2048 });
      await new Promise<void>((resolve) => { resolveDownload = resolve; });
    });
    vi.spyOn(bridge, "cancelUpdateDownload").mockRejectedValue(new Error("No se pudo cancelar."));

    render(<UpdatePanel enabled currentVersion="0.57.0" />);
    fireEvent.click(screen.getByRole("button", { name: "Buscar actualizaciones" }));
    fireEvent.click(await screen.findByRole("button", { name: "Descargar actualización" }));
    fireEvent.click(await screen.findByRole("button", { name: "Cancelar descarga" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("No se pudo cancelar.");
    resolveDownload?.();
    expect(await screen.findByRole("button", { name: "Instalar y reiniciar" })).toBeInTheDocument();
  });

  it("vuelve al estado inactivo cuando el backend confirma la cancelación", async () => {
    vi.spyOn(bridge, "checkForUpdate").mockResolvedValue(update);
    let rejectDownload: ((error: Error) => void) | undefined;
    vi.spyOn(bridge, "downloadUpdate").mockImplementation(async (onProgress) => {
      onProgress?.({ phase: "progress", downloadedBytes: 512, contentLength: 2048 });
      await new Promise<void>((_resolve, reject) => {
        rejectDownload = (error) => reject(error);
      });
    });
    const cancel = vi.spyOn(bridge, "cancelUpdateDownload").mockResolvedValue(undefined);

    render(<UpdatePanel enabled currentVersion="0.57.0" />);
    fireEvent.click(screen.getByRole("button", { name: "Buscar actualizaciones" }));
    fireEvent.click(await screen.findByRole("button", { name: "Descargar actualización" }));
    fireEvent.click(await screen.findByRole("button", { name: "Cancelar descarga" }));
    await waitFor(() => expect(cancel).toHaveBeenCalledOnce());

    rejectDownload?.(new Error("La descarga fue cancelada."));
    const checkButton = screen.getByRole("button", { name: "Buscar actualizaciones" });
    await waitFor(() => expect(checkButton).not.toBeDisabled());
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Descargar actualización" })).not.toBeInTheDocument();
  });

  it("muestra bytes descargados cuando el canal no informa el tamaño total", async () => {
    vi.spyOn(bridge, "checkForUpdate").mockResolvedValue({ ...update, sizeBytes: null });
    vi.spyOn(bridge, "downloadUpdate").mockImplementation(async (onProgress) => {
      onProgress?.({ phase: "finished", downloadedBytes: 512, contentLength: null });
    });

    render(<UpdatePanel enabled currentVersion="0.57.0" />);
    fireEvent.click(screen.getByRole("button", { name: "Buscar actualizaciones" }));
    fireEvent.click(await screen.findByRole("button", { name: "Descargar actualización" }));

    expect(await screen.findByText("Descargados: 512 B")).toBeInTheDocument();
    expect(screen.queryByRole("progressbar")).not.toBeInTheDocument();
  });
});
