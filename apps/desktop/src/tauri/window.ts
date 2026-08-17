import { platform } from "@tauri-apps/plugin-os";
import type { Theme } from "../hooks/useTheme";

// Everything the app does to its own native window. It all needs the same
// guard — `window.__TAURI_INTERNALS__` exists only inside the Tauri webview and
// the APIs throw in browser dev mode (localhost:9100) — so it lives in one
// module instead of being rediscovered per call site.

function inTauri(): boolean {
  const w = window as unknown as { __TAURI_INTERNALS__?: unknown };
  return !!w.__TAURI_INTERNALS__;
}

/** Who draws the window buttons — the only platform difference the UI cares
 * about, so callers branch on this rather than on the OS name:
 * - `system` — macOS draws the traffic lights over our title bar (which leaves
 *   a gutter for them) and zooms on a title-bar double-click by itself.
 * - `app` — Windows and Linux run undecorated, so we draw the caption buttons.
 * - `none` — a browser already has chrome; we draw nothing. */
export function windowButtons(): "system" | "app" | "none" {
  if (!inTauri()) return "none";
  return platform() === "macos" ? "system" : "app";
}

/** Drive the native window. One entry point so the dynamic import, the guard
 * and the failure path are written once. `close` goes through CloseRequested,
 * which the Rust shell intercepts to hide to the tray (src-tauri/src/lib.rs). */
export async function windowAction(
  action: "minimize" | "toggle-maximize" | "close" | "drag",
): Promise<void> {
  if (!inTauri()) return;
  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    const win = getCurrentWindow();
    switch (action) {
      case "minimize":
        await win.minimize();
        break;
      case "toggle-maximize":
        await win.toggleMaximize();
        break;
      case "close":
        await win.close();
        break;
      case "drag":
        await win.startDragging();
        break;
    }
  } catch (err) {
    console.warn(`window: ${action} failed`, err);
  }
}

/** Report the maximized state now and on every resize (there is no dedicated
 * maximize event), so the caption button can swap to the restore glyph.
 * Resolves to an unsubscribe fn; a no-op outside the Tauri webview. */
export async function watchMaximized(onChange: (maximized: boolean) => void): Promise<() => void> {
  if (!inTauri()) return () => {};
  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    const win = getCurrentWindow();
    onChange(await win.isMaximized());
    return await win.onResized(() => {
      void win.isMaximized().then(onChange);
    });
  } catch (err) {
    console.warn("window: failed to watch maximized state", err);
    return () => {};
  }
}

/** Sync the native window to the app theme: the background color kills the
 * white flash behind the webview during a live resize, and setTheme drives the
 * native appearance (incl. the macOS fullscreen title bar). */
export async function syncWindowChrome(theme: Theme): Promise<void> {
  if (!inTauri()) return;
  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    const win = getCurrentWindow();
    // --color-chrome is the base plate behind the title bar and sidebar; read it
    // live ("r g b") so the native color never drifts from index.css.
    const raw = getComputedStyle(document.documentElement)
      .getPropertyValue("--color-chrome")
      .trim();
    const rgb = raw.split(/\s+/).map(Number);
    if (rgb.length === 3 && rgb.every((n) => Number.isFinite(n))) {
      await win.setBackgroundColor([rgb[0], rgb[1], rgb[2]]);
    }
    await win.setTheme(theme);
  } catch (err) {
    console.warn("window: failed to sync native chrome", err);
  }
}
