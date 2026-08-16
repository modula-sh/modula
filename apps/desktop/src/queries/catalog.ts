import { useQuery } from "@tanstack/react-query";
import { client } from "../services/client";

export const catalogKeys = {
  providerCatalog: () => ["provider-catalog"] as const,
};

// The provider catalog is static build-time data; never goes stale in a session.
export function useProviderCatalog() {
  return useQuery({
    queryKey: catalogKeys.providerCatalog(),
    queryFn: () => client.catalog.providerCatalog(),
    staleTime: Number.POSITIVE_INFINITY,
  });
}
