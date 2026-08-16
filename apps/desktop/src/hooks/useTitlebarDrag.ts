import { useEffect } from "react";
import { startDragging } from "../lib/windowDrag";

/** Height (px) of the top strip that acts as the window grab area. */
const DRAG_ZONE_HEIGHT = 28;

/** Elements inside the drag zone that should keep their own click behaviour
 * instead of starting a window drag. */
const INTERACTIVE = "button, a, input, select, textarea, [role='button']";

/** Mounts the app-wide window drag handler on `document`: a mousedown in the
 * top {@link DRAG_ZONE_HEIGHT}px (on non-interactive elements) starts a native
 * window drag, since the macOS overlay titlebar has no real drag region.
 *
 * Because the listener lives on `document`, this hook only needs to be mounted
 * once at the top of the tree (see RootLayout) to cover every screen —
 * onboarding included — without repeating the logic per view. On Windows/Linux
 * the native title bar handles its own drag and this is effectively a no-op. */
export function useTitlebarDrag(): void {
  useEffect(() => {
    const onMouseDown = (e: MouseEvent) => {
      if (e.buttons !== 1 || e.clientY > DRAG_ZONE_HEIGHT) return;
      const target = e.target as HTMLElement | null;
      if (target?.closest(INTERACTIVE)) return;
      startDragging();
    };
    document.addEventListener("mousedown", onMouseDown);
    return () => document.removeEventListener("mousedown", onMouseDown);
  }, []);
}
