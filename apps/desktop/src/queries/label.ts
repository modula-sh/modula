import { useQuery } from "@tanstack/react-query";
import { client } from "../services/client";

export const labelKeys = {
  list: (ws: string, type = "task") => ["labels", ws, type] as const,
};

export function useLabels(ws: string, type = "task") {
  return useQuery({
    queryKey: labelKeys.list(ws, type),
    queryFn: () => client.label.list(ws, type),
    enabled: !!ws,
  });
}
