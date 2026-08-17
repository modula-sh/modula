import { useCallback, useEffect, useState } from "react";
import { syncWindowChrome } from "../tauri/window";

export type Theme = "light" | "dark";

const STORAGE_KEY = "modula.theme";

function readInitialTheme(): Theme {
  if (typeof window === "undefined") return "dark";
  try {
    const saved = window.localStorage.getItem(STORAGE_KEY);
    if (saved === "light" || saved === "dark") return saved;
  } catch {
    // localStorage unavailable (private mode) — fall through.
  }
  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

/** Theme state with persistence. Mirrors the inline init script in index.html
 * (which runs synchronously before first paint to avoid a flash). */
export function useTheme(): {
  theme: Theme;
  setTheme: (t: Theme) => void;
  toggle: () => void;
} {
  const [theme, setThemeState] = useState<Theme>(readInitialTheme);

  useEffect(() => {
    const root = document.documentElement;
    // Suppress transitions for one frame so colors flip instantly, not lag.
    root.classList.add("theme-switching");
    root.dataset.theme = theme;
    const raf = window.requestAnimationFrame(() => {
      root.classList.remove("theme-switching");
    });
    try {
      window.localStorage.setItem(STORAGE_KEY, theme);
    } catch {
      // localStorage unavailable — choice will not persist across reloads.
    }
    // Sync native window chrome (bg color + appearance) after dataset.theme is
    // set so the computed --color-bg token is current. No-op outside Tauri.
    void syncWindowChrome(theme);
    return () => window.cancelAnimationFrame(raf);
  }, [theme]);

  const setTheme = useCallback((t: Theme) => setThemeState(t), []);
  const toggle = useCallback(() => setThemeState((t) => (t === "dark" ? "light" : "dark")), []);

  return { theme, setTheme, toggle };
}
