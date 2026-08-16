import { useQuery } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { projectKeys } from "../../queries/project";
import { client } from "../../services/client";

const DEBOUNCE_MS = 300;

const EMPTY = { branches: [] as string[], defaultBranch: null as string | null, isGit: false };

/** Probe `path` for its git branches. The query key is a debounced copy of the
 * path, so typing settles for 300 ms before fetching and Query caches per path
 * (re-typing a seen path resolves instantly). An empty path yields the
 * disabled/empty state without fetching. */
export function useRepoBranches(ws: string, path: string) {
  const trimmed = path.trim();
  const [debounced, setDebounced] = useState(trimmed);
  useEffect(() => {
    const timer = setTimeout(() => setDebounced(trimmed), DEBOUNCE_MS);
    return () => clearTimeout(timer);
  }, [trimmed]);

  const { data, isFetching } = useQuery({
    queryKey: projectKeys.repoBranches(ws, debounced),
    queryFn: () => client.project.repoBranches(ws, debounced),
    enabled: !!debounced,
  });

  if (!trimmed) return { ...EMPTY, loading: false };
  const settled = trimmed === debounced && !isFetching && data !== undefined;
  return {
    branches: data?.branches ?? EMPTY.branches,
    defaultBranch: data?.default_branch ?? EMPTY.defaultBranch,
    isGit: data?.is_git ?? EMPTY.isGit,
    loading: !settled,
  };
}
