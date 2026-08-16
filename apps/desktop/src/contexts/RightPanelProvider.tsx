import { createContext, useCallback, useContext, useMemo, useState } from "react";
import { useLocation } from "react-router-dom";

// Discriminated union of every content the panel can render. New surfaces
// just add a new variant here and a matching case in RightPanel's switch.
export type RightPanelContent =
  | {
      type: "diff";
      workspace: string;
      project: string;
      branch?: string;
      focusFile?: string;
      focusGroup?: "staged" | "unstaged" | "untracked";
    }
  | {
      type: "branch-diff";
      workspace: string;
      task: string;
      variant: string;
    };

export interface RightPanelState {
  open: boolean;
  content: RightPanelContent | null;
  /** Panels can override the shell title via `setTitle` once their data lands. */
  title?: React.ReactNode;
  /** Optional control rendered next to the close button (e.g. a PR link). */
  action?: React.ReactNode;
}

const EMPTY: RightPanelState = { open: false, content: null };

interface CtxValue {
  state: RightPanelState;
  open: (content: RightPanelContent) => void;
  close: () => void;
  setTitle: (title: React.ReactNode) => void;
  setAction: (action: React.ReactNode) => void;
}

const Ctx = createContext<CtxValue | null>(null);

// Per-route-path panel memory: leaving and coming back to the same pathname
// restores whatever the panel was showing. Designed to play nicely with future
// back/forward navigation since restoration is keyed on the route, not on a
// nav action.
export function RightPanelProvider({ children }: { children: React.ReactNode }) {
  const location = useLocation();
  const [byPath, setByPath] = useState<Record<string, RightPanelState>>({});

  const state = byPath[location.pathname] ?? EMPTY;

  const open = useCallback(
    (content: RightPanelContent) =>
      setByPath((prev) => ({ ...prev, [location.pathname]: { open: true, content } })),
    [location.pathname],
  );
  const close = useCallback(
    () =>
      setByPath((prev) => ({
        ...prev,
        [location.pathname]: { ...(prev[location.pathname] ?? EMPTY), open: false },
      })),
    [location.pathname],
  );
  const setTitle = useCallback(
    (title: React.ReactNode) =>
      setByPath((prev) => ({
        ...prev,
        [location.pathname]: { ...(prev[location.pathname] ?? EMPTY), title },
      })),
    [location.pathname],
  );
  const setAction = useCallback(
    (action: React.ReactNode) =>
      setByPath((prev) => ({
        ...prev,
        [location.pathname]: { ...(prev[location.pathname] ?? EMPTY), action },
      })),
    [location.pathname],
  );

  const value = useMemo<CtxValue>(
    () => ({ state, open, close, setTitle, setAction }),
    [state, open, close, setTitle, setAction],
  );
  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

export function useRightPanel() {
  const ctx = useContext(Ctx);
  if (!ctx) throw new Error("useRightPanel must be used inside RightPanelProvider");
  return ctx;
}
