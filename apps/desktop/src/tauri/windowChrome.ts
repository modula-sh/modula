import type { Theme } from "../hooks/useTheme";

// Sync the native window chrome to the app theme. Only runs inside the Tauri
// webview (guarded on __TAURI_INTERNALS__ like ./../components/openUrl.ts); in
// browser dev mode it is a no-op so the Tauri APIs are never loaded.
//
// - setBackgroundColor removes the white flash behind the webview during a live
//   resize and backs the fullscreen titlebar with the theme color.
// - setTheme drives the native window appearance so the system titlebar /
//   window controls (incl. the macOS fullscreen titlebar) match the theme.
export async function syncWindowChrome(theme: Theme): Promise<void> {
  const w = window as unknown as { __TAURI_INTERNALS__?: unknown };
  if (!w.__TAURI_INTERNALS__) return;
  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    const win = getCurrentWindow();
    const bg = readBgColor();
    if (bg) await win.setBackgroundColor(bg);
    await win.setTheme(theme);
  } catch (err) {
    console.warn("windowChrome: failed to sync native window chrome", err);
  }
}

// Read the live `--color-bg` token ("r g b") so the native color never drifts
// from index.css. Returns null if the token is missing or unparseable.
function readBgColor(): [number, number, number] | null {
  const raw = getComputedStyle(document.documentElement).getPropertyValue("--color-bg").trim();
  if (!raw) return null;
  const parts = raw.split(/\s+/).map(Number);
  if (parts.length !== 3 || parts.some((n) => !Number.isFinite(n))) return null;
  return [parts[0], parts[1], parts[2]];
}
