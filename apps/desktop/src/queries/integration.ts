import { useQuery } from "@tanstack/react-query";
import { client } from "../services/client";

export const integrationKeys = {
  all: (ws: string) => ["integrations", ws] as const,
  search: (ws: string, id: string, query: string, params: Record<string, unknown>) =>
    ["integrations", ws, id, "search", query, params] as const,
  repos: (ws: string, id: string) => ["integrations", ws, id, "repos"] as const,
};

export function useIntegrations(ws: string) {
  return useQuery({
    queryKey: integrationKeys.all(ws),
    queryFn: () => client.integration.all(ws),
    enabled: !!ws,
  });
}

/** Type-ahead search; callers pass an already-debounced term so Query caches
 * per settled term. An empty term returns the integration's recent items. */
export function useIntegrationSearch(
  ws: string,
  id: string,
  query: string,
  params: Record<string, unknown>,
) {
  return useQuery({
    queryKey: integrationKeys.search(ws, id, query, params),
    queryFn: () => client.integration.search(ws, id, query, params),
    enabled: !!ws && !!id,
  });
}

/** Repos selectable in the import modal; only github has any. */
export function useIntegrationRepos(ws: string, id: string) {
  return useQuery({
    queryKey: integrationKeys.repos(ws, id),
    queryFn: () => client.integration.repos(ws, id),
    enabled: !!ws && id === "github",
  });
}
