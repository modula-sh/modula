import { platform } from "@tauri-apps/plugin-os";
import type { ThemeMode } from "../hooks/useTheme";

// Everything the app does to its own native window. All of it needs the same
// guard: the Tauri APIs throw outside the webview (browser dev mode).

function inTauri(): boolean {
  const w = window as unknown as { __TAURI_INTERNALS__?: unknown };
  return !!w.__TAURI_INTERNALS__;
}

/** Who draws the window buttons. `system` is macOS (traffic lights, native
 * double-click zoom), `app` is the undecorated Windows/Linux window, `none` is
 * a browser. */
export function windowButtons(): "system" | "app" | "none" {
  if (!inTauri()) return "none";
  return isMac() ? "system" : "app";
}

/** macOS, including browser dev mode, where the Tauri APIs are unavailable.
 * Keyboard shortcuts need this even when no native window is involved. */
export function isMac(): boolean {
  return inTauri() ? platform() === "macos" : /Mac/i.test(navigator.userAgent);
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

/** Native side of the theme. No background color is set: the window is
 * transparent so the backdrop shows through. */
export async function applyNativeTheme(mode: ThemeMode, glass: boolean): Promise<void> {
  if (!inTauri()) return;
  try {
    const { getCurrentWindow, Effect } = await import("@tauri-apps/api/window");
    const win = getCurrentWindow();
    await win.setTheme(mode);
    if (!glass) return await win.clearEffects();
    // `system` is macOS. fullScreenUI frosts harder than sidebar; Windows
    // acrylic has no adjustable strength.
    await win.setEffects({
      effects: [windowButtons() === "system" ? Effect.FullScreenUI : Effect.Acrylic],
    });
  } catch (err) {
    console.warn("window: failed to apply native theme", err);
  }
}
