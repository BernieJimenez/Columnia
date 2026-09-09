import { useEffect, useState } from "react";

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

export function ThemeSwitcher() {
  const [preference, setPreference] = useState<ThemePreference>(() => readThemePreference());

  useEffect(() => {
    applyThemePreference(preference);
  }, [preference]);

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
    </section>
  );
}
