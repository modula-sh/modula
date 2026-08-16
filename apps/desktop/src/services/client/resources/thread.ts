import type { ThreadsResponse } from "../../../types";
import { call } from "../invoke";
import type { EditCommentBody, PostCommentBody } from "../types";

export class ThreadResource {
  get(ws: string, taskId: string) {
    return call<ThreadsResponse>("thread_get", { workspaceId: ws, taskId });
  }

  postComment(ws: string, taskId: string, body: PostCommentBody) {
    return call<void>("thread_append", {
      workspaceId: ws,
      taskId,
      content: body.content,
      variant: body.variant,
    });
  }

  editComment(ws: string, taskId: string, entryId: number, body: EditCommentBody) {
    return call<void>("thread_edit", {
      workspaceId: ws,
      taskId,
      entryId,
      content: body.content,
      author: body.author,
    });
  }

  deleteComment(ws: string, taskId: string, entryId: number, author: string) {
    return call<void>("thread_delete", { workspaceId: ws, taskId, entryId, author });
  }
}
