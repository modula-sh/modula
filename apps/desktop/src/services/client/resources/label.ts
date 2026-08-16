import type { Label } from "../../../types";
import { call } from "../invoke";
import type { CreateLabelBody } from "../types";

export class LabelResource {
  list(ws: string, type = "task") {
    return call<Label[]>("label_list", { workspaceId: ws, labelType: type });
  }

  create(ws: string, body: CreateLabelBody) {
    return call<{ id: string }>("label_create", {
      workspaceId: ws,
      name: body.name,
      labelType: body.type ?? "task",
    });
  }

  attach(ws: string, taskId: string, labelId: string) {
    return call<void>("label_attach", { workspaceId: ws, taskId, labelId });
  }

  detach(ws: string, taskId: string, labelId: string) {
    return call<void>("label_detach", { workspaceId: ws, taskId, labelId });
  }
}
