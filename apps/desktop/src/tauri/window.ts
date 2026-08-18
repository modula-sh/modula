import { platform } from "@tauri-apps/plugin-os";
import type { Theme } from "../hooks/useTheme";

// Everything the app does to its own native window. All of it needs the same
// guard: the Tauri APIs throw outside the webview (browser dev mode).

function inTauri(): boolean {
  const w = window as unknown as { __TAURI_INTERNALS__?: unknown };
  return !!w.__TAURI_INTERNALS__;
}

/** Who draws the window buttons — the only platform difference the UI cares
 * about. `system` is macOS (traffic lights, native double-click zoom), `app` is
 * the undecorated Windows/Linux window, `none` is a browser. */
export function windowButtons(): "system" | "app" | "none" {
  if (!inTauri()) return "none";
  return platform() === "macos" ? "system" : "app";
}

/** `close` goes through CloseRequested, which the Rust shell intercepts to hide
 * to the tray (src-tauri/src/lib.rs). */
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

/** Resize is the only signal — there is no maximize event. Resolves to an
 * unsubscribe fn. */
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

/** The background color kills the white flash behind the webview during a live
 * resize; setTheme drives the native appearance. */
export async function syncWindowChrome(theme: Theme): Promise<void> {
  if (!inTauri()) return;
  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    const win = getCurrentWindow();
    // Read live so the native color never drifts from index.css.
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
