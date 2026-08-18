import { useCallback, useEffect, useState } from "react";
import { applyNativeTheme } from "../tauri/window";

export type ThemeMode = "light" | "dark";
export type Theme = "light" | "dark" | "light-glass" | "dark-glass";

export const THEMES: { value: Theme; label: string }[] = [
  { value: "light", label: "Light" },
  { value: "dark", label: "Dark" },
  { value: "light-glass", label: "Light Glass" },
  { value: "dark-glass", label: "Dark Glass" },
];

const STORAGE_KEY = "modula.theme";

function readInitialTheme(): Theme {
  if (typeof window === "undefined") return "dark";
  try {
    const saved = window.localStorage.getItem(STORAGE_KEY);
    if (THEMES.some((t) => t.value === saved)) return saved as Theme;
  } catch {
    // localStorage unavailable (private mode) — fall through.
  }
  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

/** Theme state with persistence. Glass is a second axis over light/dark: `mode`
 * drives the color tokens, `glass` the window effect and the translucent base
 * plate. index.html repeats the mode lookup before first paint. */
export function useTheme(): {
  theme: Theme;
  mode: ThemeMode;
  glass: boolean;
  setTheme: (t: Theme) => void;
  toggle: () => void;
} {
  const [theme, setThemeState] = useState<Theme>(readInitialTheme);
  const mode: ThemeMode = theme === "light" || theme === "light-glass" ? "light" : "dark";
  const glass = theme === "light-glass" || theme === "dark-glass";

  useEffect(() => {
    const root = document.documentElement;
    // Suppress transitions for one frame so colors flip instantly, not lag.
    root.classList.add("theme-switching");
    root.dataset.theme = mode;
    const raf = window.requestAnimationFrame(() => {
      root.classList.remove("theme-switching");
    });
    try {
      window.localStorage.setItem(STORAGE_KEY, theme);
    } catch {
      // localStorage unavailable — choice will not persist across reloads.
    }
    void applyNativeTheme(mode, glass);
    return () => window.cancelAnimationFrame(raf);
  }, [theme, mode, glass]);

  const setTheme = useCallback((t: Theme) => setThemeState(t), []);

  /** Flips light↔dark, staying on whichever glass variant is in use. */
  const toggle = useCallback(
    () =>
      setThemeState((t) => {
        switch (t) {
          case "light":
            return "dark";
          case "dark":
            return "light";
          case "light-glass":
            return "dark-glass";
          case "dark-glass":
            return "light-glass";
        }
      }),
    [],
  );

  return { theme, mode, glass, setTheme, toggle };
}
