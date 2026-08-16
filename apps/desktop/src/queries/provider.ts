import { useQuery } from "@tanstack/react-query";
import { client } from "../services/client";

export const providerKeys = {
  all: (ws: string) => ["providers", ws] as const,
  detail: (ws: string, id: string) => ["providers", ws, "detail", id] as const,
};

export function useProviders(ws: string) {
  return useQuery({
    queryKey: providerKeys.all(ws),
    queryFn: () => client.provider.all(ws),
  });
}

export function useProvider(ws: string, id: string | undefined) {
  return useQuery({
    queryKey: providerKeys.detail(ws, id ?? ""),
    queryFn: () => client.provider.get(ws, id!),
    enabled: !!ws && !!id,
  });
}
