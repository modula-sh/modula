import { useQuery } from "@tanstack/react-query";
import { client } from "../services/client";

export const agentKeys = {
  detail: (ws: string, id: string) => ["agents", ws, "detail", id] as const,
  skills: (ws: string) => ["agents", ws, "skills"] as const,
};

export function useAgent(ws: string, id: string | undefined) {
  return useQuery({
    queryKey: agentKeys.detail(ws, id ?? ""),
    queryFn: () => client.agent.get(ws, id!),
    enabled: !!ws && !!id,
  });
}

// Skills catalog is static build-time data; never goes stale in a session.
export function useAgentSkills(ws: string) {
  return useQuery({
    queryKey: agentKeys.skills(ws),
    queryFn: () => client.agent.skills(ws),
    enabled: !!ws,
    staleTime: Number.POSITIVE_INFINITY,
  });
}
