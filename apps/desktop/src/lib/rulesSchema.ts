// Declarative schema for the agent-rules builder: the selectable comparison
// operators, event paths (keys), and per-key enum value options. Kept as a
// plain JSON-serialisable const so it can later be served from the backend.
//
// Source of truth is the engine: the expression grammar lives in
// apps/engine/src/services/dispatcher/expr.rs and the event shapes are emitted
// across apps/engine/src/services/ (the broadcast bus in services/events). The
// grammar supports `==`/`!=` against quoted strings plus the bare booleans
// true/false; enum/freeform values here serialise quoted, bool values as bare
// literals (see lib/rules.ts).

export type RuleOp = "==" | "!=";

export const RULE_OPERATORS: { value: RuleOp; label: string }[] = [
  { value: "==", label: "equals" },
  { value: "!=", label: "not equals" },
];

export interface RuleKey {
  path: string;
  label: string;
  // "bool" keys compare against the unquoted literals true/false (the engine
  // grammar's bool values); see lib/rules.ts for how they serialise.
  valueKind: "enum" | "freeform" | "bool";
  values?: string[];
}

// Variant statuses (apps/engine/src/services/db/variants.rs) and pipeline
// statuses (apps/engine/src/services/db/pipeline.rs). Pipeline statuses are the default
// seeded set; a workspace may customise them, but these cover the stock
// install and are safe defaults for the dropdown.
const VARIANT_STATUSES = [
  "ready_for_workers",
  "in_progress",
  "ready_for_review",
  "in_review",
  "rework",
  "accepted",
];

const PIPELINE_STATUSES = [
  "planning",
  "ready_for_research",
  "researching",
  "needs_clarification",
  "ready_for_workers",
  "in_progress",
  "ready_for_review",
  "in_review",
  "ready_for_acceptance",
  "accepted",
  "blocked",
];

// Ordered most-common-first. event.type leads because nearly every rule keys
// off it.
export const RULE_KEYS: RuleKey[] = [
  {
    path: "event.type",
    label: "event type",
    valueKind: "enum",
    values: [
      "task.create",
      "task.update",
      "task.delete",
      "task.reset",
      "variant.update",
      "thread.append",
      "conversation.create",
      "conversation.update",
      "conversation.delete",
    ],
  },
  {
    path: "event.data.status",
    label: "variant status",
    valueKind: "enum",
    values: VARIANT_STATUSES,
  },
  {
    path: "event.data.pipeline_status",
    label: "pipeline status",
    valueKind: "enum",
    values: PIPELINE_STATUSES,
  },
  {
    path: "event.data.kind",
    label: "thread entry kind",
    valueKind: "enum",
    values: ["comment", "question", "verdict", "rework"],
  },
  {
    path: "event.data.author",
    label: "thread author",
    valueKind: "enum",
    values: [
      "human",
      "researcher",
      "worker",
      "code-reviewer",
      "reviewer",
      "project-manager",
      "jira-scan",
      "linear-scan",
      "github-scan",
    ],
  },
  {
    path: "event.data.verdict",
    label: "thread verdict",
    valueKind: "enum",
    values: ["ACCEPT", "REQUEST_CHANGES", "APPROVE", "KICK_BACK"],
  },
  {
    path: "event.data.source",
    label: "task source",
    valueKind: "enum",
    values: ["internal", "jira", "linear", "github"],
  },
  // Rides flat on both task.create and task.update.
  {
    path: "event.data.approved",
    label: "approved",
    valueKind: "bool",
    values: ["true", "false"],
  },
  { path: "event.data.task_id", label: "task id", valueKind: "freeform" },
  { path: "event.data.variant_id", label: "variant id", valueKind: "freeform" },
  { path: "event.data.id", label: "conversation id", valueKind: "freeform" },
];

export function ruleKeyFor(path: string): RuleKey | undefined {
  return RULE_KEYS.find((k) => k.path === path);
}

// Display label for a key/op/enum value: letters only, lowercased, non-letter
// runs collapsed to a space. Not for freeform values — it would strip ids.
export function optLabel(s: string): string {
  return s
    .replace(/[^a-zA-Z]+/g, " ")
    .trim()
    .toLowerCase();
}
