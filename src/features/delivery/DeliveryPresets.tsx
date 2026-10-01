import { useEffect, useRef, useState } from "react";

import {
  cancelOperation,
  deleteDeliveryPreset,
  listDeliveryPresets,
  openDeliveryPreset,
  saveDeliveryPreset,
  type DatasetPreview,
  type DeliveryPreset,
  type DeliveryPresetSummary,
} from "../../bridge";

interface DeliveryPresetsProps {
  dataset: DatasetPreview;
  /** The current delivery settings as a preset with the given name. */
  currentPreset: (name: string) => DeliveryPreset;
  /** Applies a preset to Entregar; returns a notice about its database target, if any. */
  onApply: (preset: DeliveryPreset) => string | null;
}

/** «Presets de entrega guardados»: save, open, apply and delete local presets. */
export function DeliveryPresets({ dataset, currentPreset, onApply }: DeliveryPresetsProps) {
  const [presets, setPresets] = useState<DeliveryPresetSummary[]>([]);
  const [presetsLoaded, setPresetsLoaded] = useState(false);
  const [presetsLoading, setPresetsLoading] = useState(false);
  const [presetsCancellationPending, setPresetsCancellationPending] = useState(false);
  const [presetsCancelled, setPresetsCancelled] = useState(false);
  const [presetsError, setPresetsError] = useState<string | null>(null);
  const presetsRequestGeneration = useRef(0);
  const presetsCatalogInFlight = useRef(false);
  const presetsMounted = useRef(true);
  const [presetName, setPresetName] = useState("");
  const [selectedPresetId, setSelectedPresetId] = useState("");
  const [openedPreset, setOpenedPreset] = useState<DeliveryPreset | null>(null);
  const [presetNotice, setPresetNotice] = useState<string | null>(null);
  const [presetWorking, setPresetWorking] = useState(false);
  useEffect(() => {
    presetsMounted.current = true;
    return () => {
      presetsMounted.current = false;
      presetsRequestGeneration.current += 1;
      presetsCatalogInFlight.current = false;
    };
  }, []);
  async function refreshDeliveryPresets() {
    if (presetsCatalogInFlight.current) return;
    const request = ++presetsRequestGeneration.current;
    presetsCatalogInFlight.current = true;
    setPresetsLoading(true);
    setPresetsCancellationPending(false);
    setPresetsCancelled(false);
    setPresetsError(null);
    try {
      const nextPresets = await listDeliveryPresets();
      if (!presetsMounted.current || request !== presetsRequestGeneration.current) return;
      setPresets(nextPresets);
      setPresetsLoaded(true);
    } catch (error: unknown) {
      if (!presetsMounted.current || request !== presetsRequestGeneration.current) return;
      setPresetsError(error instanceof Error ? error.message : String(error));
    } finally {
      if (presetsMounted.current && request === presetsRequestGeneration.current) {
        presetsCatalogInFlight.current = false;
        setPresetsLoading(false);
      }
    }
  }

  async function cancelDeliveryPresetCatalog() {
    if (!presetsCatalogInFlight.current || presetsCancellationPending) return;
    const request = ++presetsRequestGeneration.current;
    setPresetsCancellationPending(true);
    setPresetsCancelled(true);
    setPresetsError(null);
    try {
      await cancelOperation("deliveryPresetCatalog");
    } catch (error: unknown) {
      if (presetsMounted.current && request === presetsRequestGeneration.current) {
        setPresetsError(error instanceof Error ? error.message : String(error));
      }
    } finally {
      if (presetsMounted.current && request === presetsRequestGeneration.current) {
        presetsCatalogInFlight.current = false;
        setPresetsLoading(false);
        setPresetsCancellationPending(false);
      }
    }
  }

  async function saveCurrentDeliveryPreset() {
    const trimmedName = presetName.trim();
    if (!trimmedName || presetWorking) return;
    const preset = currentPreset(trimmedName);
    setPresetWorking(true);
    setPresetNotice(null);
    setPresetsError(null);
    try {
      const summary = await saveDeliveryPreset(selectedPresetId || null, preset);
      setPresets((current) => [summary, ...current.filter((item) => item.id !== summary.id)]);
      setPresetsLoaded(true);
      setPresetsCancelled(false);
      setSelectedPresetId(summary.id);
      setOpenedPreset(preset);
      setPresetNotice("Preset guardado localmente. Las credenciales de conexión no se almacenan.");
    } catch (error: unknown) {
      setPresetsError(error instanceof Error ? error.message : String(error));
    } finally {
      setPresetWorking(false);
    }
  }

  async function loadSelectedDeliveryPreset() {
    if (!selectedPresetId || presetWorking) return;
    setPresetWorking(true);
    setPresetNotice(null);
    setPresetsError(null);
    try {
      const preset = await openDeliveryPreset(selectedPresetId);
      setOpenedPreset(preset);
      setPresetName(preset.name);
      setPresetNotice(null);
    } catch (error: unknown) {
      setPresetsError(error instanceof Error ? error.message : String(error));
    } finally {
      setPresetWorking(false);
    }
  }

  async function deleteSelectedDeliveryPreset() {
    if (!selectedPresetId || presetWorking) return;
    setPresetWorking(true);
    setPresetsError(null);
    setPresetNotice(null);
    try {
      await deleteDeliveryPreset(selectedPresetId);
      setPresets((current) => current.filter((item) => item.id !== selectedPresetId));
      setPresetsCancelled(false);
      setSelectedPresetId("");
      setOpenedPreset(null);
      setPresetName("");
      setPresetNotice("Preset eliminado del catálogo local.");
    } catch (error: unknown) {
      setPresetsError(error instanceof Error ? error.message : String(error));
    } finally {
      setPresetWorking(false);
    }
  }

  function applyOpenedDeliveryPreset() {
    if (!openedPreset) return;
    const currentColumns = dataset.columns.map((column) => column.name);
    const schemaMatches = JSON.stringify(openedPreset.selectedColumns) === JSON.stringify(currentColumns);
    if (!schemaMatches) {
      const missing = openedPreset.selectedColumns.filter((column) => !currentColumns.includes(column));
      const added = currentColumns.filter((column) => !openedPreset.selectedColumns.includes(column));
      setPresetNotice(`Esquema distinto: ${missing.length} columna(s) guardada(s) no aparecen y ${added.length} columna(s) nueva(s). Se aplicará la configuración a todas las columnas actuales.`);
    } else {
      setPresetNotice("Preset verificado contra el esquema actual.");
    }
    const databaseNotice = onApply(openedPreset);
    if (databaseNotice) setPresetNotice(databaseNotice);
  }

  const openedPresetSchemaMatches = openedPreset !== null
    && JSON.stringify(openedPreset.selectedColumns) === JSON.stringify(dataset.columns.map((column) => column.name));

  return (
          <details
            className="delivery-presets"
            onToggle={(event) => {
              if (event.currentTarget.open && !presetsLoaded && !presetsLoading
                && !presetsCancellationPending && !presetsCatalogInFlight.current) {
                void refreshDeliveryPresets();
              }
            }}
          >
            <summary>Presets de entrega guardados</summary>
            <p>Guarda formatos, protección y columnas para repetirlos. Las credenciales quedan fuera del preset.</p>
            <label>
              Preset de entrega local
              <select
                aria-label="Preset de entrega local"
                value={selectedPresetId}
                onChange={(event) => {
                  setSelectedPresetId(event.target.value);
                  setOpenedPreset(null);
                  setPresetNotice(null);
                  const summary = presets.find((item) => item.id === event.target.value);
                  if (summary) setPresetName(summary.name);
                }}
                disabled={presetWorking || presetsLoading}
              >
                <option value="">Selecciona un preset</option>
                {presets.map((preset) => (
                  <option key={preset.id} value={preset.id}>
                    {preset.name} · {preset.format} · {preset.selectedColumnCount} columnas
                  </option>
                ))}
              </select>
            </label>
            <label>
              Nombre del preset
              <input
                aria-label="Nombre del preset de entrega"
                value={presetName}
                maxLength={80}
                onChange={(event) => setPresetName(event.target.value)}
                disabled={presetWorking}
              />
            </label>
            <div className="delivery-presets__actions">
              <button type="button" className="secondary-action" onClick={() => void loadSelectedDeliveryPreset()} disabled={!selectedPresetId || presetWorking || presetsLoading}>
                {presetWorking ? "Procesando preset…" : "Abrir y verificar"}
              </button>
              <button type="button" className="secondary-action" onClick={() => void saveCurrentDeliveryPreset()} disabled={!presetName.trim() || presetWorking || presetsLoading || presetsCancellationPending}>
                Guardar preset
              </button>
              {selectedPresetId && (
                <button type="button" className="secondary-action" onClick={() => void deleteSelectedDeliveryPreset()} disabled={presetWorking || presetsLoading || presetsCancellationPending}>
                  Eliminar preset
                </button>
              )}
            </div>
            {presetsLoading && (
              <div className="notice">
                <p role="status">
                  {presetsCancellationPending ? "Cancelando carga del catálogo local…" : "Cargando catálogo local…"}
                </p>
                <button
                  type="button"
                  className="secondary-action"
                  onClick={() => void cancelDeliveryPresetCatalog()}
                  disabled={presetsCancellationPending}
                >
                  {presetsCancellationPending ? "Cancelando…" : "Cancelar carga"}
                </button>
              </div>
            )}
            {presetsCancelled && !presetsLoading && !presetsError && (
              <div className="notice" role="status">
                <span>Se canceló la carga de presets locales.</span>
                <button
                  type="button"
                  className="secondary-action"
                  onClick={() => void refreshDeliveryPresets()}
                  disabled={presetsCancellationPending}
                >
                  Reintentar catálogo
                </button>
              </div>
            )}
            {!presetsLoading && presetsLoaded && presets.length === 0 && <p role="note">Aún no hay presets locales.</p>}
            {presetsError && (
              <div className="notice notice--error" role="alert">
                <p>{presetsError}</p>
                <button type="button" className="secondary-action" onClick={() => void refreshDeliveryPresets()} disabled={presetsLoading || presetsCancellationPending}>Reintentar catálogo</button>
              </div>
            )}
            {openedPreset && (
              <div className="delivery-presets__verification" role="status">
                <p>
                  {openedPresetSchemaMatches
                    ? `Esquema verificado: ${openedPreset.selectedColumns.length} columnas, mismo orden.`
                    : "El esquema guardado no coincide exactamente con el dataset actual; revisa la diferencia antes de aplicar."}
                </p>
                {!openedPresetSchemaMatches && (
                  <p>
                    Faltan: {openedPreset.selectedColumns.filter((column) => !dataset.columns.some((current) => current.name === column)).join(", ") || "ninguna"}. Nuevas: {dataset.columns.map((column) => column.name).filter((column) => !openedPreset.selectedColumns.includes(column)).join(", ") || "ninguna"}.
                  </p>
                )}
                <button type="button" className="secondary-action" onClick={applyOpenedDeliveryPreset} disabled={presetWorking}>
                  {openedPresetSchemaMatches ? "Aplicar preset verificado" : "Aplicar tras revisar esquema"}
                </button>
              </div>
            )}
            {presetNotice && <p className="notice notice--success" role="status">{presetNotice}</p>}
          </details>
  );
}
