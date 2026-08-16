import { useQuery } from "@tanstack/react-query";
import { client } from "../services/client";

export const wikiKeys = {
  tree: (ws: string) => ["wiki", ws, "tree"] as const,
  file: (ws: string, path: string) => ["wiki", ws, "file", path] as const,
};

export function useWikiTree(ws: string) {
  return useQuery({
    queryKey: wikiKeys.tree(ws),
    queryFn: () => client.wiki.tree(ws),
    enabled: !!ws,
  });
}

export function useWikiFile(ws: string, path: string | null) {
  return useQuery({
    queryKey: wikiKeys.file(ws, path ?? ""),
    queryFn: () => client.wiki.file(ws, path!),
    enabled: !!ws && !!path,
  });
}
