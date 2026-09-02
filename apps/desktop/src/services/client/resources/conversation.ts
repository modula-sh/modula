import type { ConversationDetail, QueuedMessage } from "../../../types";
import { call } from "../invoke";
import type { CreateConversationBody } from "../types";

export class ConversationResource {
  get(ws: string, id: string) {
    return call<ConversationDetail>("conversation_get", { workspaceId: ws, conversationId: id });
  }

  create(ws: string, body: CreateConversationBody) {
    return call<{ id: string }>("conversation_create", {
      workspaceId: ws,
      providerId: body.provider_id,
      title: body.title,
      model: body.model,
      context: body.context,
    });
  }

  rename(ws: string, id: string, title: string) {
    return call<void>("conversation_rename", { workspaceId: ws, conversationId: id, title });
  }

  delete(ws: string, id: string) {
    return call<void>("conversation_delete", { workspaceId: ws, conversationId: id });
  }

  enqueue(ws: string, id: string, message: string) {
    return call<QueuedMessage[]>("conversation_enqueue", {
      workspaceId: ws,
      conversationId: id,
      message,
    });
  }

  dequeue(ws: string, id: string, queuedId: string) {
    return call<QueuedMessage[]>("conversation_dequeue", {
      workspaceId: ws,
      conversationId: id,
      queuedId,
    });
  }
}
