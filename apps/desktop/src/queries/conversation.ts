import { useQuery } from "@tanstack/react-query";
import { client } from "../services/client";

export const conversationKeys = {
  detail: (ws: string, id: string) => ["conversations", ws, "detail", id] as const,
};

export function useConversation(ws: string, id: string | undefined) {
  return useQuery({
    queryKey: conversationKeys.detail(ws, id ?? ""),
    queryFn: () => client.conversation.get(ws, id!),
    enabled: !!ws && !!id,
  });
}
