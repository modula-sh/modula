import { getCurrentWindow } from "@tauri-apps/api/window";

export function startDragging(): void {
  try {
    getCurrentWindow().startDragging();
  } catch {
    // no-op in browser mode (Tauri internals unavailable)
  }
}
