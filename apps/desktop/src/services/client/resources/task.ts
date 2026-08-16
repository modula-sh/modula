import type { Branch, TaskAgentSetting } from "../../../types";
import { call } from "../invoke";
import type { AgentLoopBody, CreateTaskBody, UpdateTaskBody, UpsertTaskBody } from "../types";

export class TaskResource {
  create(ws: string, body: CreateTaskBody) {
    return call<{ id: string }>("task_create", {
      workspaceId: ws,
      title: body.title,
      description: body.description,
    });
  }

  upsert(ws: string, body: UpsertTaskBody) {
    return call<{ id: string; external_id: string; upserted: string }>("task_upsert", {
      workspaceId: ws,
      source: body.source,
      externalId: body.external_id,
      title: body.title,
      description: body.description,
      sourceData: body.source_data,
      url: body.url,
    });
  }

  update(ws: string, id: string, body: UpdateTaskBody) {
    return call<void>("task_update", {
      workspaceId: ws,
      taskId: id,
      title: body.title,
      description: body.description,
      approved: body.approved,
      maxVariants: body.max_variants,
      worktree: body.worktree,
    });
  }

  delete(ws: string, id: string) {
    return call<void>("task_delete", { workspaceId: ws, taskId: id });
  }

  reset(ws: string, id: string) {
    return call<void>("task_reset", { workspaceId: ws, taskId: id });
  }

  branches(ws: string, id: string) {
    return call<Branch[]>("project_task_branches", { workspaceId: ws, taskId: id });
  }

  agentSettings(ws: string, id: string) {
    return call<TaskAgentSetting[]>("task_agent_settings", { workspaceId: ws, taskId: id });
  }

  saveAgentSetting(ws: string, id: string, agentId: string, body: AgentLoopBody) {
    return call<void>("task_agent_setting_set", {
      workspaceId: ws,
      taskId: id,
      agentId,
      amount: body.loop.amount,
    });
  }

  deleteAgentSetting(ws: string, id: string, agentId: string) {
    return call<void>("task_agent_setting_delete", { workspaceId: ws, taskId: id, agentId });
  }
}
