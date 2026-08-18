import { useEffect } from "react";
import { windowAction, windowButtons } from "../tauri/window";

/** Height (px) of the grab area: the Titlebar row, on every platform. */
const DRAG_ZONE_HEIGHT = 35;

/** Windows' default double-click speed. */
const DOUBLE_CLICK_MS = 500;

/** Elements inside the drag zone that should keep their own click behaviour
 * instead of starting a window drag. */
const INTERACTIVE = "button, a, input, select, textarea, [role='button']";

/** One `document` listener covers every screen, onboarding included. macOS zooms
 * on a title-bar double-click itself; elsewhere we pair the clicks, because the
 * native drag loop takes the mouse and the webview never sees a `dblclick`. */
export function useTitlebarDrag(): void {
  useEffect(() => {
    const nativeZoom = windowButtons() === "system";
    let lastDown = 0;
    const onMouseDown = (e: MouseEvent) => {
      if (e.buttons !== 1 || e.clientY > DRAG_ZONE_HEIGHT) return;
      const target = e.target as HTMLElement | null;
      if (target?.closest(INTERACTIVE)) return;
      if (!nativeZoom) {
        const paired = e.timeStamp - lastDown < DOUBLE_CLICK_MS;
        lastDown = paired ? 0 : e.timeStamp;
        if (paired) {
          void windowAction("toggle-maximize");
          return;
        }
      }
      void windowAction("drag");
    };
    document.addEventListener("mousedown", onMouseDown);
    return () => document.removeEventListener("mousedown", onMouseDown);
  }, []);
}
