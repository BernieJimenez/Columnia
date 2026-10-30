import { useEffect, useState } from "react";

import {
  applyDensityPreference,
  readDensityPreference,
  writeDensityPreference,
  type DensityPreference,
} from "../features/settings/densityModel";
import {
  applyThemePreference,
  readThemePreference,
  writeThemePreference,
  type ThemePreference,
} from "../features/settings/themeModel";

const THEME_OPTIONS: readonly {
  value: ThemePreference;
  label: string;
  description: string;
}[] = [
  { value: "system", label: "Sistema", description: "Usa la preferencia del equipo" },
  { value: "light", label: "Claro", description: "Lienzo luminoso" },
  { value: "dark", label: "Oscuro", description: "Panel de baja luz" },
  { value: "paper", label: "Papel", description: "Tonos cálidos para sesiones largas" },
  { value: "ocean", label: "Océano", description: "Azules frescos y definidos" },
  { value: "slate", label: "Pizarra", description: "Neutros sobrios con acento azul" },
];

const DENSITY_OPTIONS: readonly { value: DensityPreference; label: string; description: string }[] = [
  { value: "comfortable", label: "Normal", description: "Espaciado amplio y texto holgado" },
  { value: "compact", label: "Compacta", description: "Más filas y columnas a la vista; los botones mantienen su tamaño" },
];

export function ThemeSwitcher() {
  const [preference, setPreference] = useState<ThemePreference>(() => readThemePreference());

  useEffect(() => {
    applyThemePreference(preference);
    if (preference !== "system" || typeof window.matchMedia !== "function") return undefined;
    // Follow the OS while "Sistema" is selected.
    const query = window.matchMedia("(prefers-color-scheme: dark)");
    const follow = () => applyThemePreference(preference);
    query.addEventListener("change", follow);
    return () => query.removeEventListener("change", follow);
  }, [preference]);

  const [density, setDensity] = useState<DensityPreference>(() => readDensityPreference());
  const chooseDensity = (next: DensityPreference) => {
    setDensity(next);
    writeDensityPreference(next);
    applyDensityPreference(next);
  };

  const chooseTheme = (nextPreference: ThemePreference) => {
    setPreference(nextPreference);
    writeThemePreference(nextPreference);
    applyThemePreference(nextPreference);
  };

  return (
    <section className="theme-switcher" aria-labelledby="theme-switcher-title">
      <div className="theme-switcher__heading">
        <span className="theme-switcher__eyebrow">Apariencia</span>
        <div>
          <h2 id="theme-switcher-title">Tema</h2>
          <p>Elige cómo quieres ver tu estación.</p>
        </div>
      </div>

      <div className="theme-switcher__options" role="group" aria-label="Tema de la interfaz">
        {THEME_OPTIONS.map((option) => (
          <button
            key={option.value}
            type="button"
            className={preference === option.value ? "theme-switcher__option theme-switcher__option--active" : "theme-switcher__option"}
            data-theme-option={option.value}
            aria-pressed={preference === option.value}
            title={option.description}
            onClick={() => chooseTheme(option.value)}
          >
            <span className="theme-switcher__swatch" aria-hidden="true">
              <span />
              <span />
              <span />
            </span>
            <span className="theme-switcher__label">{option.label}</span>
          </button>
        ))}
      </div>
      <p className="theme-switcher__status" aria-live="polite">
        {THEME_OPTIONS.find((option) => option.value === preference)?.description}
      </p>

      <h3 className="theme-switcher__subheading" id="density-title">Densidad</h3>
      <div className="theme-switcher__density" role="group" aria-labelledby="density-title">
        {DENSITY_OPTIONS.map((option) => (
          <button
            key={option.value}
            type="button"
            className={density === option.value ? "theme-switcher__option theme-switcher__option--active" : "theme-switcher__option"}
            aria-pressed={density === option.value}
            onClick={() => chooseDensity(option.value)}
          >
            <span className="theme-switcher__label">{option.label}</span>
          </button>
        ))}
      </div>
      <p className="theme-switcher__status" aria-live="polite">
        {DENSITY_OPTIONS.find((option) => option.value === density)?.description}
      </p>
    </section>
  );
}
