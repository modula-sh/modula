import { type QueryClient, useQuery } from "@tanstack/react-query";
import { client } from "../services/client";
import type { ConversationDetail, ConversationMessage } from "../types";

export const conversationKeys = {
  detail: (ws: string, id: string) => ["conversations", ws, "detail", id] as const,
};

/** Messages per page, for the tail and for each older page. */
export const MESSAGE_PAGE = 30;

/** Older pages already loaded stay ahead of a fresh tail it adjoins or overlaps. */
function withTail(loaded: ConversationMessage[] | undefined, tail: ConversationMessage[]) {
  const first = tail[0]?.seq;
  const last = loaded?.at(-1)?.seq;
  if (!loaded || first === undefined || last === undefined || first > last + 1) return tail;
  return [...loaded.filter((m) => m.seq < first), ...tail];
}

/** The conversation and its transcript's tail; `loadOlderMessages` extends it backwards. */
export function useConversation(ws: string, id: string | undefined) {
  return useQuery({
    queryKey: conversationKeys.detail(ws, id ?? ""),
    queryFn: async ({ client: queryClient, queryKey }) => {
      const fresh = await client.conversation.get(ws, id!, MESSAGE_PAGE);
      const loaded = queryClient.getQueryData<ConversationDetail>(queryKey)?.messages;
      return { ...fresh, messages: withTail(loaded, fresh.messages) };
    },
    enabled: !!ws && !!id,
  });
}

/** Fetch the page before the oldest loaded message into the cached conversation. */
export async function loadOlderMessages(queryClient: QueryClient, ws: string, id: string) {
  const key = conversationKeys.detail(ws, id);
  const first = queryClient.getQueryData<ConversationDetail>(key)?.messages[0]?.seq;
  if (!first) return;
  const older = await client.conversation.messages(ws, id, first, MESSAGE_PAGE);
  queryClient.setQueryData<ConversationDetail>(key, (conv) =>
    conv && conv.messages[0]?.seq === first
      ? { ...conv, messages: [...older, ...conv.messages] }
      : conv,
  );
}
