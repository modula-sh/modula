import { useQuery } from "@tanstack/react-query";
import { client } from "../services/client";

export const taskKeys = {
  branches: (ws: string, id: string) => ["tasks", ws, id, "branches"] as const,
  agentSettings: (ws: string, id: string) => ["tasks", ws, id, "agent-settings"] as const,
};

export function useTaskBranches(ws: string, id: string) {
  return useQuery({
    queryKey: taskKeys.branches(ws, id),
    queryFn: () => client.task.branches(ws, id),
    enabled: !!ws && !!id,
  });
}

export function useTaskAgentSettings(ws: string, id: string) {
  return useQuery({
    queryKey: taskKeys.agentSettings(ws, id),
    queryFn: () => client.task.agentSettings(ws, id),
    enabled: !!ws && !!id,
  });
}
