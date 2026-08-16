import { call } from "../invoke";

export class RoadmapResource {
  setStatus(ws: string, taskId: string, status: string) {
    return call<void>("roadmap_set_status", { workspaceId: ws, taskId, status });
  }
}
