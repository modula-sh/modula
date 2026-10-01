import { createContext } from "react";
import type { WorkspaceInfo } from "../types";

/** The workspace a subtree belongs to. Each visited workspace mounts its own
 * layout, so this is fixed for the life of that subtree. */
export const WorkspaceContext = createContext<string>("");

/** The active workspace, the list, and the switcher, shared by every layout. */
interface WorkspaceSwitcherValue {
  active: string;
  workspaces: WorkspaceInfo[];
  setWorkspace: (ws: string) => void;
  refreshWorkspaces: () => void;
}

export const WorkspaceSwitcherContext = createContext<WorkspaceSwitcherValue>({
  active: "",
  workspaces: [],
  setWorkspace: () => {},
  refreshWorkspaces: () => {},
});
