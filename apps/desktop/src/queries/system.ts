import { useQuery } from "@tanstack/react-query";
import { client } from "../services/client";

export const systemKeys = {
  tools: () => ["system", "tools"] as const,
};

export function useSystemTools() {
  return useQuery({
    queryKey: systemKeys.tools(),
    queryFn: () => client.system.tools(),
  });
}
