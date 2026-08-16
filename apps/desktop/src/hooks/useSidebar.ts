import { useCallback, useRef, useState } from "react";

const STORAGE_KEY = "modula.sidebar.open";

const NAV_OPEN_WIDTH = 264;
const CONTENT_MIN = 760;

// Collapse when the content area would fall below CONTENT_MIN with the nav open.
// regionWidth is nav + content (window − the right drawers); subtracting the
// nav's *open* width (not its live width) keeps the decision from flipping back
// when collapsing itself frees space.
function contentTooSmall(regionWidth: number): boolean {
  return regionWidth - NAV_OPEN_WIDTH < CONTENT_MIN;
}

function readPreference(): boolean {
  if (typeof window === "undefined") return true;
  try {
    if (window.localStorage.getItem(STORAGE_KEY) === "false") return false;
  } catch {
    // localStorage unavailable (private mode) — fall through.
  }
  return true;
}

// No drawer is open at mount, so window width stands in for the region.
function initialNarrow(): boolean {
  return typeof window !== "undefined" && contentTooSmall(window.innerWidth);
}

export function useSidebar(): {
  open: boolean;
  setOpen: (v: boolean) => void;
  toggle: () => void;
  notifyRegionWidth: (width: number) => void;
} {
  const [open, setOpenState] = useState<boolean>(() => readPreference() && !initialNarrow());
  const narrowRef = useRef(initialNarrow());

  // Explicit user toggles persist; width-driven changes below are transient.
  const setOpen = useCallback((v: boolean) => {
    setOpenState(v);
    try {
      window.localStorage.setItem(STORAGE_KEY, String(v));
    } catch {
      // localStorage unavailable — choice will not persist across reloads.
    }
  }, []);

  const toggle = useCallback(() => setOpen(!open), [open, setOpen]);

  // Layout feeds the live region width. Act on band crossings only: collapse on
  // entry, restore the saved preference on exit — so an explicit toggle wins.
  const notifyRegionWidth = useCallback((width: number) => {
    const next = contentTooSmall(width);
    if (next === narrowRef.current) return;
    narrowRef.current = next;
    setOpenState(next ? false : readPreference());
  }, []);

  return { open, setOpen, toggle, notifyRegionWidth };
}
