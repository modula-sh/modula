import { useQuery } from "@tanstack/react-query";
import { client } from "../services/client";

export const snapshotKeys = {
  all: (ws: string) => ["snapshot", ws] as const,
};

/** The assembled workspace snapshot (tasks, roadmap, agents, runs, config,
 * conversations). Fetched once per workspace over the gRPC bridge and kept
 * live by `useEngineEvents` invalidation rather than the old SSE poll. */
export function useSnapshotQuery(ws: string) {
  return useQuery({
    queryKey: snapshotKeys.all(ws),
    queryFn: () => client.snapshot.get(ws),
    enabled: !!ws,
  });
}
