// Open a URL in the system browser. Uses Tauri's shell plugin when running
// inside the Tauri webview; falls back to `window.open` in browser dev mode.
export async function openUrl(url: string): Promise<void> {
  const w = window as unknown as { __TAURI_INTERNALS__?: unknown };
  if (w.__TAURI_INTERNALS__) {
    const { open } = await import("@tauri-apps/plugin-shell");
    await open(url);
    return;
  }
  window.open(url, "_blank", "noopener,noreferrer");
}
