import { useQuery } from "@tanstack/react-query";
import { client } from "../services/client";

export const overviewKeys = {
  all: ["overview"] as const,
};

export function useOverview() {
  return useQuery({
    queryKey: overviewKeys.all,
    queryFn: () => client.overview.get(),
  });
}
