import { QueryClient } from "@tanstack/react-query";

// Single module-scope client so the cache survives re-renders. Desktop app:
// focus refetches are noisy, so they're off; 30s staleTime kills re-open flashing.
// Hidden workspaces drop their observers, so a finite gcTime would evict what
// they show.
export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 30_000,
      gcTime: Number.POSITIVE_INFINITY,
      refetchOnWindowFocus: false,
      retry: 1,
    },
  },
});
