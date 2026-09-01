import { memo, useEffect, useState } from "react";

import {
  getPerformanceSettings as defaultFetchPerformanceSettings,
  getResourceUsage,
  setPerformanceProfile as defaultApplyPerformanceProfile,
  type PerformanceProfile,
  type PerformanceSettings,
  type ResourceUsage,
} from "../bridge";
import {
  hasPerformanceProfilePreference,
  readPerformanceProfile,
  writePerformanceProfile,
} from "../features/settings/performanceModel";

type ResourceMonitorState =
  | { kind: "disabled" }
  | { kind: "loading" }
  | { kind: "ready"; usage: ResourceUsage }
  | { kind: "error" };

export interface ResourceMonitorProps {
  enabled: boolean;
  fetchUsage?: () => Promise<ResourceUsage>;
  pollIntervalMs?: number;
  fetchPerformanceSettings?: () => Promise<PerformanceSettings>;
  setPerformanceProfile?: (profile: PerformanceProfile) => Promise<PerformanceSettings>;
  performanceProfile?: PerformanceProfile;
  onPerformanceProfileChange?: (profile: PerformanceProfile) => void;
}

function clampMeter(value: number, maximum: number): number {
  if (!Number.isFinite(value) || maximum <= 0) return 0;
  return Math.min(Math.max(value, 0), maximum);
}

function formatSystemCpu(value: number): string {
  return `${Math.max(value, 0).toFixed(1)}%`;
}

function formatProcessCpu(value: number, logicalCpuCount: number): string {
  const logicalCpus = Math.max(Math.round(logicalCpuCount), 1);
  const usedCpus = Math.max(value, 0) / 100;
  return `${usedCpus.toFixed(1)} / ${logicalCpus} hilos`;
}

function formatProcessMemory(bytes: number): string {
  return `${Math.round(Math.max(bytes, 0) / 1024 / 1024)} MB`;
}

function formatSystemMemory(usedBytes: number, totalBytes: number): string {
  const used = Math.max(usedBytes, 0) / 1024 / 1024 / 1024;
  const total = Math.max(totalBytes, 0) / 1024 / 1024 / 1024;
  return `${used.toFixed(1)} / ${total.toFixed(1)} GB`;
}

function formatAvailableMemory(bytes: number | undefined): string {
  if (bytes === undefined || !Number.isFinite(bytes)) return "no disponible";
  return `${(Math.max(bytes, 0) / 1024 / 1024 / 1024).toFixed(1)} GB`;
}

export const ResourceMonitor = memo(function ResourceMonitor({
  enabled,
  fetchUsage = getResourceUsage,
  pollIntervalMs = 2000,
  fetchPerformanceSettings = defaultFetchPerformanceSettings,
  setPerformanceProfile = defaultApplyPerformanceProfile,
  performanceProfile: controlledPerformanceProfile,
  onPerformanceProfileChange,
}: ResourceMonitorProps) {
  const [state, setState] = useState<ResourceMonitorState>(
    enabled ? { kind: "loading" } : { kind: "disabled" },
  );
  const [performance, setPerformance] = useState<PerformanceSettings | null>(null);
  const [performanceProfile, setPerformanceProfileState] = useState<PerformanceProfile>(() =>
    controlledPerformanceProfile ?? readPerformanceProfile(),
  );
  const [performanceStatus, setPerformanceStatus] = useState<"idle" | "loading" | "ready" | "error">(
    "idle",
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

    const initialRefresh = window.setTimeout(() => void refresh(), 250);
    const timer = window.setInterval(() => void refresh(), pollIntervalMs);
    return () => {
      cancelled = true;
      window.clearTimeout(initialRefresh);
      window.clearInterval(timer);
    };
  }, [enabled, fetchUsage, pollIntervalMs]);

  useEffect(() => {
    if (controlledPerformanceProfile !== undefined) {
      setPerformanceProfileState(controlledPerformanceProfile);
    }
  }, [controlledPerformanceProfile]);

  useEffect(() => {
    if (!enabled) {
      setPerformanceStatus("idle");
      return;
    }

    let cancelled = false;
    const preferredProfile = controlledPerformanceProfile ?? readPerformanceProfile();
    const hasStoredPreference = controlledPerformanceProfile !== undefined || hasPerformanceProfilePreference();
    setPerformanceProfileState(preferredProfile);
    setPerformanceStatus("loading");

    const configure = async () => {
      try {
        const current = await fetchPerformanceSettings();
        if (cancelled) return;

        if (!hasStoredPreference) {
          setPerformance(current);
          setPerformanceStatus("ready");
          return;
        }

        if (current.applied && current.requestedProfile === preferredProfile) {
          setPerformance(current);
          setPerformanceStatus("ready");
          return;
        }

        const configured = await setPerformanceProfile(preferredProfile);
        if (!cancelled) {
          setPerformance(configured);
          setPerformanceStatus("ready");
        }
      } catch {
        if (!cancelled) setPerformanceStatus("error");
      }
    };

    void configure();
    return () => {
      cancelled = true;
    };
  }, [controlledPerformanceProfile, enabled, fetchPerformanceSettings, setPerformanceProfile]);

  const choosePerformanceProfile = (nextProfile: PerformanceProfile) => {
    setPerformanceProfileState(nextProfile);
    if (controlledPerformanceProfile === undefined) writePerformanceProfile(nextProfile);
    onPerformanceProfileChange?.(nextProfile);
    if (!enabled) return;

    setPerformanceStatus("loading");
    void setPerformanceProfile(nextProfile)
      .then((settings) => {
        setPerformance(settings);
        setPerformanceStatus("ready");
      })
      .catch(() => setPerformanceStatus("error"));
  };

  const usage = state.kind === "ready" ? state.usage : null;
  const processCpu = usage?.processCpuPercentage ?? 0;
  const systemCpu = usage?.systemCpuPercentage ?? 0;
  const logicalCpuCount = usage?.logicalCpuCount ?? 1;
  const systemMemoryTotal = usage?.systemMemoryTotalBytes ?? 0;
  const systemMemoryUsed = usage?.systemMemoryUsedBytes ?? 0;
  const systemMemoryAvailable = usage?.systemMemoryAvailableBytes;
  const gpu = usage?.gpu;
  const statusText = state.kind === "disabled"
    ? "Solo escritorio"
    : state.kind === "error"
      ? "No disponible ahora"
      : state.kind === "loading"
        ? "Midiendo…"
        : "Actualizado en vivo";
  const performanceText = !enabled
    ? "Solo disponible en la app de escritorio"
    : performanceStatus === "loading"
      ? "Aplicando antes de la próxima operación…"
      : performance?.applied && performance.activeThreads
        ? `Activo: ${performance.activeThreads} ${performance.activeThreads === 1 ? "hilo" : "hilos"}`
        : performance?.reason ?? "Se aplicará antes de la primera operación";

  return (
    <div className="resource-monitor" role="group" aria-label="Consumo de recursos">
      <div className="resource-monitor__heading">
        <p className="resource-monitor__title">Tu entorno</p>
        <span
          className="resource-monitor__status"
          aria-label={state.kind === "disabled" ? "Disponible en la app de escritorio" : undefined}
        >
          {statusText}
        </span>
      </div>

      <div className="resource-monitor__metric">
        <div className="resource-monitor__label">
          <span>CPU</span>
          <strong>{usage ? formatProcessCpu(processCpu, logicalCpuCount) : "—"}</strong>
        </div>
        <meter
          className="resource-monitor__meter"
          min={0}
          max={Math.max(logicalCpuCount, 1)}
          value={clampMeter(processCpu / 100, Math.max(logicalCpuCount, 1))}
          aria-label={`CPU de Columnia: ${usage ? formatProcessCpu(processCpu, logicalCpuCount) : "no disponible"}`}
        />
        <div className="resource-monitor__system">
          <span>Equipo</span>
          <strong>{usage ? formatSystemCpu(systemCpu) : "—"}</strong>
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
          value={clampMeter(usage?.processMemoryBytes ?? 0, Math.max(systemMemoryTotal, 1))}
          aria-label={`RAM de Columnia: ${usage ? formatProcessMemory(usage.processMemoryBytes) : "no disponible"}`}
        />
        <div className="resource-monitor__system">
          <span>Equipo</span>
          <strong>{usage ? formatSystemMemory(systemMemoryUsed, systemMemoryTotal) : "—"}</strong>
        </div>
        <div className="resource-monitor__system">
          <span>Disponible</span>
          <strong>{usage ? formatAvailableMemory(systemMemoryAvailable) : "—"}</strong>
        </div>
      </div>

      <div className="resource-monitor__metric resource-monitor__metric--gpu">
        <div className="resource-monitor__label">
          <span>GPU</span>
          <strong>{gpu?.status === "available" ? `${Math.max(gpu.usagePercentage ?? 0, 0).toFixed(1)}%` : "No disponible"}</strong>
        </div>
        <div className="resource-monitor__system">
          <span>{gpu?.status === "available" ? "Aceleración" : "Motor local"}</span>
          <strong>{gpu?.status === "available" ? "Activa" : "CPU"}</strong>
        </div>
      </div>

      <div className="resource-monitor__performance">
        <label htmlFor="resource-performance-profile">Modo de rendimiento</label>
        <select
          id="resource-performance-profile"
          value={performanceProfile}
          disabled={!enabled || performanceStatus === "loading"}
          onChange={(event) =>
            choosePerformanceProfile(event.target.value as PerformanceProfile)
          }
        >
          <option value="conservative">Ahorro · 1 hilo</option>
          <option value="balanced">Equilibrado · mitad de hilos</option>
          <option value="maximum">Máximo · todos los hilos</option>
        </select>
        <p aria-live="polite">{performanceText}</p>
      </div>
    </div>
  );
});
