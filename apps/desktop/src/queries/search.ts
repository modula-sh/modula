import { useQuery } from "@tanstack/react-query";
import { client } from "../services/client";

export const searchKeys = {
  query: (ws: string, q: string) => ["search", ws, q] as const,
};

export function useSearch(ws: string, q: string) {
  return useQuery({
    queryKey: searchKeys.query(ws, q),
    queryFn: () => client.search.query(ws, q),
    enabled: !!ws && q.length > 0,
    staleTime: 15_000,
  });
}
