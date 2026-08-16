import { useQuery } from "@tanstack/react-query";
import { client } from "../services/client";

export const usageKeys = {
  all: (ws: string) => ["usage", ws] as const,
};

export function useUsage(ws: string) {
  return useQuery({
    queryKey: usageKeys.all(ws),
    queryFn: () => client.usage.all(ws),
  });
}
