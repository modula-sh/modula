import { useQuery } from "@tanstack/react-query";
import { client } from "../services/client";

export const threadKeys = {
  detail: (ws: string, taskId: string) => ["threads", ws, taskId] as const,
};

export function useThreads(ws: string, taskId: string) {
  return useQuery({
    queryKey: threadKeys.detail(ws, taskId),
    queryFn: () => client.thread.get(ws, taskId),
    enabled: !!ws && !!taskId,
  });
}
