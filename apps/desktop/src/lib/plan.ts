import type { Agent, PlanProgress, Task } from "../types";

export function planFinished(p: PlanProgress): number {
  return p.tasks_done + p.tasks_skipped;
}

export function planPercent(p: PlanProgress): number {
  return p.tasks_total === 0 ? 0 : Math.round((planFinished(p) / p.tasks_total) * 100);
}

/** Plan progress of the variant an agent is running; null for task-level agents. */
export function agentProgress(agent: Agent, task: Task | undefined): PlanProgress | null {
  if (!agent.variant) return null;
  return task?.variants.find((v) => v.id === agent.variant)?.plan_progress ?? null;
}
