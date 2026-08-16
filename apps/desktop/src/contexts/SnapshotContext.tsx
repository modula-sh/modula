import { createContext, useContext } from "react";
import type { Snapshot } from "../types";

/** Workspace state that's continuously refreshed by the SSE stream — tasks,
 * roadmap, running agents, logs, config. Provided once at the RootLayout
 * level; every view consumes it via useSnapshot() rather than receiving
 * snapshot data as props.
 *
 * `snap` is null during the brief window between workspace-switch and the
 * first SSE message; consumers should handle that case (most views are only
 * rendered after RootLayout's "connecting…" guard, so this is rare). */
interface SnapshotState {
  snap: Snapshot | null;
}

const SnapshotContext = createContext<SnapshotState>({ snap: null });

export function SnapshotProvider({
  value,
  children,
}: {
  value: SnapshotState;
  children: React.ReactNode;
}) {
  return <SnapshotContext.Provider value={value}>{children}</SnapshotContext.Provider>;
}

/** Returns the live snapshot. Components that can't tolerate `null` should
 * be rendered after RootLayout's connecting-state guard so this is non-null. */
export function useSnapshot(): SnapshotState {
  return useContext(SnapshotContext);
}
