import { useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { useWorkspaces, workspaceKeys } from "../queries/workspace";
import type { WorkspaceInfo } from "../types";

const WS_STORAGE_KEY = "modula.workspace";

/** Track the active workspace + the full list of workspaces from the backend.
 * The list is server state (React Query); the active id is client UI state
 * persisted to localStorage. Auto-corrects to the first available workspace if
 * the saved id is gone (e.g. workspace was deleted out-of-band). */
export function useWorkspaceState() {
  const queryClient = useQueryClient();
  const { data, isPending } = useWorkspaces();
  const workspaces = data ?? [];
  const [workspace, setWorkspaceState] = useState<string>(
    () => localStorage.getItem(WS_STORAGE_KEY) || "",
  );

  useEffect(() => {
    if (workspaces.length > 0 && !workspaces.some((w) => w.id === workspace)) {
      setWorkspaceState(workspaces[0].id);
      localStorage.setItem(WS_STORAGE_KEY, workspaces[0].id);
    }
  }, [workspaces, workspace]);

  function setWorkspace(ws: string) {
    localStorage.setItem(WS_STORAGE_KEY, ws);
    setWorkspaceState(ws);
  }
  function refreshWorkspaces() {
    queryClient.invalidateQueries({ queryKey: workspaceKeys.all() });
  }
  function addWorkspace(ws: WorkspaceInfo) {
    queryClient.setQueryData<WorkspaceInfo[]>(workspaceKeys.all(), (list) =>
      list?.some((w) => w.id === ws.id) ? list : [...(list ?? []), ws],
    );
  }
  return {
    workspace,
    workspaces,
    loaded: !isPending,
    setWorkspace,
    refreshWorkspaces,
    addWorkspace,
  };
}
