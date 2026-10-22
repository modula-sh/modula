import { useQuery } from "@tanstack/react-query";
import { client } from "../services/client";

export const overviewKeys = {
  all: ["overview"] as const,
};

export function useOverview() {
  return useQuery({
    queryKey: overviewKeys.all,
    queryFn: () => client.overview.get(),
    // Events are only watched while open, so a cached copy may be behind.
    refetchOnMount: "always",
  });
}
