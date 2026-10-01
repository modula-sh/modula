import { createContext, useContext } from "react";
import type { Snapshot } from "../types";

/** Workspace state kept live by the engine event stream — tasks, roadmap,
 * running agents, logs, config. Provided once per workspace by
 * WorkspaceLayout; every view consumes it via useSnapshot() rather than
 * receiving snapshot data as props.
 *
 * `snap` is null until a workspace's first snapshot fetch lands; consumers
 * should handle that case (most views are only rendered after
 * WorkspaceLayout's "connecting…" guard, so this is rare). */
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
 * be rendered after WorkspaceLayout's connecting-state guard so this is non-null. */
export function useSnapshot(): SnapshotState {
  return useContext(SnapshotContext);
}
