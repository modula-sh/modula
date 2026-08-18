import { createContext, useContext } from "react";
import { type Theme, type ThemeMode, useTheme } from "../hooks/useTheme";

interface ThemeState {
  theme: Theme;
  mode: ThemeMode;
  glass: boolean;
  setTheme: (t: Theme) => void;
  toggle: () => void;
}

const ThemeContext = createContext<ThemeState>({
  theme: "dark",
  mode: "dark",
  glass: false,
  setTheme: () => {},
  toggle: () => {},
});

export function ThemeProvider({ children }: { children: React.ReactNode }) {
  const value = useTheme();
  return <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>;
}

export function useThemeContext(): ThemeState {
  return useContext(ThemeContext);
}
