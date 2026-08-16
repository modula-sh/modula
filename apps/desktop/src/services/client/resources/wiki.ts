import { call } from "../invoke";
import type { WikiFile, WikiFileBody, WikiNode } from "../types";

export class WikiResource {
  tree(ws: string) {
    return call<WikiNode[]>("wiki_tree", { workspaceId: ws });
  }

  file(ws: string, path: string) {
    return call<WikiFile>("wiki_file", { workspaceId: ws, path });
  }

  saveFile(ws: string, body: WikiFileBody) {
    return call<void>("wiki_write_file", {
      workspaceId: ws,
      path: body.path,
      content: body.content,
    });
  }

  createFile(ws: string, path: string) {
    return call<void>("wiki_create_file", { workspaceId: ws, path, content: "" });
  }

  createFolder(ws: string, path: string) {
    return call<void>("wiki_create_folder", { workspaceId: ws, path });
  }

  rename(ws: string, from: string, to: string) {
    return call<void>("wiki_rename", { workspaceId: ws, from, to });
  }

  delete(ws: string, path: string) {
    return call<void>("wiki_delete", { workspaceId: ws, path });
  }
}
