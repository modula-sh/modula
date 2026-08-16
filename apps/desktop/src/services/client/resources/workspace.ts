import type { WorkspaceInfo } from "../../../types";
import { call } from "../invoke";
import type { CreateWorkspaceBody, WorkspaceCreated } from "../types";

export class WorkspaceResource {
  all() {
    return call<WorkspaceInfo[]>("workspace_list");
  }

  create(body: CreateWorkspaceBody) {
    return call<WorkspaceCreated>("workspace_create", {
      name: body.name,
      description: body.description,
    });
  }
}
