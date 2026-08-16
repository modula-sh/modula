import { platform } from "@tauri-apps/plugin-os";

// `platform()` reads window.__TAURI_OS_PLUGIN_INTERNALS__, which only exists
// inside the Tauri webview. Calling it in browser dev mode (localhost:9100)
// throws and crashes the render, so guard on __TAURI_INTERNALS__ (like
// ./windowChrome.ts and ../components/openUrl.ts) and return null in a browser.
export function safePlatform(): string | null {
  const w = window as unknown as { __TAURI_INTERNALS__?: unknown };
  return w.__TAURI_INTERNALS__ ? platform() : null;
}
