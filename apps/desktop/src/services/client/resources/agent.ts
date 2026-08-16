import type { AgentDetail, AgentSkill } from "../../../types";
import { call } from "../invoke";
import type { AgentWriteBody } from "../types";

export class AgentResource {
  get(ws: string, id: string) {
    return call<AgentDetail>("agent_get", { workspaceId: ws, agentId: id });
  }

  skills(ws: string) {
    return call<AgentSkill[]>("agent_skills", { workspaceId: ws });
  }

  create(ws: string, body: AgentWriteBody) {
    return call<{ id: string }>("agent_create", { workspaceId: ws, body });
  }

  update(ws: string, id: string, body: AgentWriteBody) {
    return call<{ id: string }>("agent_update", { workspaceId: ws, agentId: id, body });
  }

  delete(ws: string, id: string) {
    return call<void>("agent_delete", { workspaceId: ws, agentId: id });
  }

  trigger(ws: string, id: string, args?: Record<string, string>) {
    return call<{ pid: number }>("agent_trigger", { workspaceId: ws, agentId: id, args });
  }

  kill(ws: string, pid: number) {
    return call<void>("agent_kill", { workspaceId: ws, pid, escalate: false });
  }
}
