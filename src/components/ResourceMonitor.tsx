import { memo, useEffect, useState } from "react";

import { getResourceUsage, type ResourceUsage } from "../bridge";

type ResourceMonitorState =
  | { kind: "disabled" }
  | { kind: "loading" }
  | { kind: "ready"; usage: ResourceUsage }
  | { kind: "error" };

export interface ResourceMonitorProps {
  enabled: boolean;
  fetchUsage?: () => Promise<ResourceUsage>;
  pollIntervalMs?: number;
}

function clampMeter(value: number, maximum: number): number {
  if (!Number.isFinite(value) || maximum <= 0) return 0;
  return Math.min(Math.max(value, 0), maximum);
}

function formatCpu(value: number): string {
  return `${Math.max(value, 0).toFixed(1)}%`;
}

function formatProcessMemory(bytes: number): string {
  return `${Math.round(Math.max(bytes, 0) / 1024 / 1024)} MB`;
}

function formatSystemMemory(usedBytes: number, totalBytes: number): string {
  const used = Math.max(usedBytes, 0) / 1024 / 1024 / 1024;
  const total = Math.max(totalBytes, 0) / 1024 / 1024 / 1024;
  return `${used.toFixed(1)} / ${total.toFixed(1)} GB`;
}

export const ResourceMonitor = memo(function ResourceMonitor({
  enabled,
  fetchUsage = getResourceUsage,
  pollIntervalMs = 2000,
}: ResourceMonitorProps) {
  const [state, setState] = useState<ResourceMonitorState>(
    enabled ? { kind: "loading" } : { kind: "disabled" },
  );

  useEffect(() => {
    if (!enabled) {
      setState({ kind: "disabled" });
      return;
    }

    let cancelled = false;
    setState({ kind: "loading" });

    const refresh = async () => {
      try {
        const usage = await fetchUsage();
        if (!cancelled) setState({ kind: "ready", usage });
      } catch {
        if (!cancelled) setState({ kind: "error" });
      }
    };

    void refresh();
    const timer = window.setInterval(() => void refresh(), pollIntervalMs);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, [enabled, fetchUsage, pollIntervalMs]);

  const usage = state.kind === "ready" ? state.usage : null;
  const processCpu = usage?.processCpuPercentage ?? 0;
  const systemCpu = usage?.systemCpuPercentage ?? 0;
  const systemMemoryTotal = usage?.systemMemoryTotalBytes ?? 0;
  const systemMemoryUsed = usage?.systemMemoryUsedBytes ?? 0;
  const statusText = state.kind === "disabled"
    ? "Disponible en la app de escritorio"
    : state.kind === "error"
      ? "No disponible ahora"
      : state.kind === "loading"
        ? "Midiendo…"
        : "Actualizado en vivo";

  return (
    <div className="resource-monitor" role="group" aria-label="Consumo de recursos">
      <div className="resource-monitor__heading">
        <p className="resource-monitor__title">Tu entorno</p>
        <span className="resource-monitor__status">{statusText}</span>
      </div>

      <div className="resource-monitor__metric">
        <div className="resource-monitor__label">
          <span>CPU</span>
          <strong>{usage ? formatCpu(processCpu) : "—"}</strong>
        </div>
        <meter
          className="resource-monitor__meter"
          min={0}
          max={100}
          value={clampMeter(processCpu, 100)}
          aria-label={`CPU de Columnia: ${usage ? formatCpu(processCpu) : "no disponible"}`}
        />
        <div className="resource-monitor__system">
          <span>Equipo</span>
          <strong>{usage ? formatCpu(systemCpu) : "—"}</strong>
        </div>
      </div>

      <div className="resource-monitor__metric">
        <div className="resource-monitor__label">
          <span>RAM</span>
          <strong>{usage ? formatProcessMemory(usage.processMemoryBytes) : "—"}</strong>
        </div>
        <meter
          className="resource-monitor__meter"
          min={0}
          max={Math.max(systemMemoryTotal, 1)}
          value={clampMeter(systemMemoryUsed, Math.max(systemMemoryTotal, 1))}
          aria-label={`RAM del equipo: ${usage ? formatSystemMemory(systemMemoryUsed, systemMemoryTotal) : "no disponible"}`}
        />
        <div className="resource-monitor__system">
          <span>Equipo</span>
          <strong>{usage ? formatSystemMemory(systemMemoryUsed, systemMemoryTotal) : "—"}</strong>
        </div>
      </div>
    </div>
  );
});
