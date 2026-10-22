import { useQuery } from "@tanstack/react-query";
import { client } from "../services/client";

export const planKeys = {
  task: (ws: string, task: string) => ["plans", ws, task] as const,
  variant: (ws: string, task: string, variant: string) => ["plans", ws, task, variant] as const,
};

export function usePlan(ws: string, task: string, variant: string, enabled = true) {
  return useQuery({
    queryKey: planKeys.variant(ws, task, variant),
    queryFn: () => client.plan.get(ws, task, variant),
    enabled: enabled && !!ws && !!task && !!variant,
    retry: false,
  });
}
