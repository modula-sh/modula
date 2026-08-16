import { useQuery } from "@tanstack/react-query";
import { client } from "../services/client";

export const projectKeys = {
  all: (ws: string) => ["projects", ws] as const,
  detail: (ws: string, id: string) => ["projects", ws, "detail", id] as const,
  repoBranches: (ws: string, path: string) => ["projects", ws, "repo-branches", path] as const,
  diff: (ws: string, id: string, branch?: string) =>
    ["projects", ws, "diff", id, branch ?? ""] as const,
  diffText: (ws: string, id: string, branch?: string) =>
    ["projects", ws, "diff-text", id, branch ?? ""] as const,
  commits: (ws: string, id: string, branch?: string, since?: string) =>
    ["projects", ws, "commits", id, branch ?? "", since ?? ""] as const,
  commitDiff: (ws: string, id: string, sha: string, branch?: string) =>
    ["projects", ws, "commits", id, branch ?? "", sha] as const,
};

export function useProjects(ws: string) {
  return useQuery({
    queryKey: projectKeys.all(ws),
    queryFn: () => client.project.all(ws),
    // Accepted polling exception: the engine emits no project create/update/delete
    // event (WorkspaceEvent has no project variant), so `useEngineEvents` can't
    // invalidate this off the stream. Same rationale as the git working-tree polls.
    refetchInterval: 5_000,
  });
}

export function useProject(ws: string, id: string | undefined) {
  return useQuery({
    queryKey: projectKeys.detail(ws, id ?? ""),
    queryFn: () => client.project.get(ws, id!),
    enabled: !!ws && !!id,
  });
}

export function useProjectDiff(ws: string, id: string, branch?: string) {
  return useQuery({
    queryKey: projectKeys.diff(ws, id, branch),
    queryFn: () => client.project.diff(ws, id, branch),
    enabled: !!ws && !!id,
    refetchInterval: 2_000,
  });
}

// Full patch text for the right-panel diff view; polls so staging done elsewhere
// (or by background agents) reflects without a manual refresh.
export function useDiffText(ws: string, id: string, branch?: string) {
  return useQuery({
    queryKey: projectKeys.diffText(ws, id, branch),
    queryFn: () => client.project.diffText(ws, id, branch),
    enabled: !!ws && !!id,
    refetchInterval: 2_000,
  });
}

export function useProjectCommits(ws: string, id: string, branch?: string, since?: string) {
  return useQuery({
    queryKey: projectKeys.commits(ws, id, branch, since),
    queryFn: () => client.project.commits(ws, id, { branch, since }),
    enabled: !!ws && !!id,
    refetchInterval: 2_000,
  });
}

export function useCommitDiff(
  ws: string,
  id: string,
  sha: string,
  branch: string | undefined,
  enabled: boolean,
) {
  return useQuery({
    queryKey: projectKeys.commitDiff(ws, id, sha, branch),
    queryFn: () => client.project.commitDiff(ws, id, sha, branch),
    enabled: enabled && !!ws && !!id,
    staleTime: Number.POSITIVE_INFINITY,
  });
}
