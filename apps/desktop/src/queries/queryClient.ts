import { QueryClient } from "@tanstack/react-query";

// Single module-scope client so the cache survives re-renders. Desktop app:
// focus refetches are noisy, so they're off; 30s staleTime kills re-open flashing.
export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 30_000,
      gcTime: 5 * 60_000,
      refetchOnWindowFocus: false,
      retry: 1,
    },
  },
});
