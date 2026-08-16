// Friendly names for the system args agents can request as "context". Source of
// truth is the engine: the dispatcher fills each agent-declared arg `flag` from
// a matching `event.data` key (apps/engine/src/services/dispatcher/mod.rs::
// build_arg_map, kebab→snake), and api/agents.rs::trigger validates spec/branch.
// These are the injectable top-level scalar keys.
export interface SystemContext {
  flag: string;
  name: string;
}

export const SYSTEM_CONTEXTS: SystemContext[] = [
  { flag: "--task-id", name: "Task" },
  { flag: "--variant-id", name: "Variant" },
  { flag: "--status", name: "Status" },
  { flag: "--branch", name: "Branch" },
  { flag: "--spec", name: "Spec" },
];

export function contextLabel(flag: string): string {
  return SYSTEM_CONTEXTS.find((c) => c.flag === flag)?.name ?? flag;
}
