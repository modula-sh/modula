import { useQuery } from "@tanstack/react-query";
import { client } from "../services/client";

export const workspaceKeys = {
  all: () => ["workspaces"] as const,
};

export function useWorkspaces() {
  return useQuery({
    queryKey: workspaceKeys.all(),
    queryFn: () => client.workspace.all(),
  });
}
