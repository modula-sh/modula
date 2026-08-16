import { useQuery } from "@tanstack/react-query";
import { client } from "../services/client";

export const diffKeys = {
  variant: (ws: string, task: string, variant: string) => ["diffs", ws, task, variant] as const,
  pr: (ws: string, task: string, variant: string) => ["diffs", ws, task, variant, "pr"] as const,
};

export function useVariantDiff(ws: string, task: string, variant: string) {
  return useQuery({
    queryKey: diffKeys.variant(ws, task, variant),
    queryFn: () => client.diff.variant(ws, task, variant),
    enabled: !!ws && !!task && !!variant,
  });
}

export function useVariantPr(ws: string, task: string, variant: string) {
  return useQuery({
    queryKey: diffKeys.pr(ws, task, variant),
    queryFn: () => client.diff.variantPr(ws, task, variant),
    enabled: !!ws && !!task && !!variant,
    staleTime: 60_000,
  });
}
