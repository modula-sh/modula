// Shared types — interfaces used by more than one file. View-local form/state
// types live next to the view that owns them.

export interface WorkspaceInfo {
  id: string;
  name: string;
  description?: string | null;
  path: string;
  created_at?: string;
}

export type Approved = boolean | null;

/** A workflow status, sourced from the engine's `pipeline` table (read via
 * `ConfigService.Get`). The frontend never hardcodes status keys — order,
 * labels, and colors all come from config so a workspace can redefine its
 * pipeline without code changes. */
export type PipelineTone = "zinc" | "yellow" | "red" | "blue" | "purple" | "green" | "orange";

export interface PipelineStatus {
  key: string;
  label: string;
  tone: PipelineTone;
  station?: string | null;
  terminal?: boolean;
  error?: boolean;
}

export interface Task {
  id: string;
  /** External system identifier (e.g. "JIRA-123"). Null for internal tasks. */
  external_id?: string | null;
  title: string;
  status?: string;
  /** Where this task came from: `jira` / `linear` / `internal` / etc.
   * Defaults to "external" when missing on legacy rows that pre-date the
   * field. The dashboard's "open in <SOURCE>" link only renders for
   * non-internal sources. Downstream agents do NOT switch on this field. */
  source?: string;
  /** Link back to the source system. Only set for external tasks. */
  url?: string;
  approved: Approved;
  description?: string;
  /** Per-task execution settings. Both default to null; the Researcher
   * reads max_variants (default 1 when null) to cap variant count, and the
   * Worker / Code-Reviewer read worktree (default true when null) to decide
   * whether to use a per-variant `.worktrees/<branch>` (true) or commit
   * directly on `base_branch` in the project's main checkout (false). When
   * `worktree` is false, `max_variants` is implicitly forced to 1 since
   * multiple variants would collide on the shared base branch. */
  max_variants?: number | null;
  worktree?: boolean | null;
  synced_at: string;
  created_at?: string;
  variants: Variant[];
  /** Optional so older payloads (pre-labels) still parse. */
  labels?: Label[];
}

export interface Label {
  id: string;
  name: string;
}

export interface Integration {
  /** One of the fixed integration ids: github, jira, linear. */
  id: string;
  data: Record<string, unknown>;
}

/** An item in an external system; `key` is its human identifier there
 * (PROJ-123, owner/repo#42, ENG-123). */
export interface ExternalItem {
  key: string;
  title: string;
  description: string;
  url: string;
  state: string;
}

export type VariantStatus =
  | "ready_for_workers"
  | "in_progress"
  | "ready_for_review"
  | "in_review"
  | "rework"
  | "accepted";

export interface Variant {
  id: string;
  /** Display order assigned at creation. Use this for labels like "Variant 1"
   * rather than rendering the UUID. */
  position: number;
  /** `null` until promoted; a freshly-registered variant has no status. */
  status: VariantStatus | null;
}

export interface RoadmapItem {
  task: string;
  /** Free-form status string — must match a `key` in `pipeline_statuses`. */
  status: string;
  depends_on: string[];
  notes: string;
}

export interface Agent {
  pid: number;
  /** The `agent_runs` row this process belongs to (maps a run → its pid). */
  run_id: number;
  name: string;
  started_at: string;
  task?: string | null;
  variant?: string | null;
  spec?: string | null;
  branch?: string | null;
}

export type RunStatus = "running" | "completed" | "failed";

/** A row from the `agent_runs` table. The dispatcher / manual `trigger` /
 * cron scheduler each insert one row per agent dispatch. `log_path` is the
 * basename of the log file the spawned claude wrote to (inside `<ws>/logs/`).
 */
export interface AgentRun {
  id: number;
  agent_name: string;
  event_id: number | null;
  status: RunStatus;
  attempts: number;
  data: { args?: Record<string, string>; trigger?: string } & Record<string, unknown>;
  /** `data.args["task-id"]` / `["variant-id"]` surfaced flat by the engine
   * so the dashboard doesn't have to know the kebab-case flag names. */
  task: string | null;
  variant: string | null;
  started_at: string | null;
  finished_at: string | null;
  created_at: string;
  log_path: string | null;
  loop_iter: number;
  loop_total: number;
  loop_group_id: number | null;
}

/** One usage record per finished claude agent run. Extracted from the
 * `result` event at the end of the run's log; runs without that event
 * (in-flight, crashed, or non-claude) are omitted. */
export interface UsageRecord {
  run_id: number;
  log: string; // log filename — click-through to LogViewer
  agent: string;
  mtime: string;
  duration_ms: number;
  cost_usd: number;
  tokens: {
    input: number;
    output: number;
    cache_creation: number;
    cache_read: number;
  };
}

export interface Branch {
  project: ProjectConfigEntry;
  branch: string;
  /** Variant this branch belongs to (its position), or null for a task-scoped
   *  branch. Parsed server-side so the UI need not know the branch-name format. */
  variant_position: number | null;
  worktree: string;
  head: string;
  /** Commits on this branch not on the project's `base_branch`. `null` when
   *  git couldn't resolve the range (e.g. base not fetched yet). */
  commits: number | null;
}

export interface FilePatch {
  path: string;
  additions: number;
  deletions: number;
  diff: string;
}

export interface ProjectDiff {
  name: string;
  branch: string;
  base_branch: string;
  range: string;
  files: number;
  insertions: number;
  deletions: number;
  patches: FilePatch[];
}

export interface VariantDiffs {
  task: string;
  task_title?: string;
  variant: string;
  variant_status: VariantStatus | null;
  mode: "worktree" | "direct";
  projects: ProjectDiff[];
}

export interface ProjectPr {
  name: string;
  create_url: string | null;
  pr_url: string | null;
  pr_number: number | null;
}

export interface VariantPr {
  projects: ProjectPr[];
}

export type ThreadKind = "comment" | "verdict" | "rework" | string;

export interface ThreadEntry {
  id: number;
  ts?: string;
  author: string;
  kind: ThreadKind;
  round?: number;
  content?: string;
  verdict?: string;
  affected_variants?: string[];
}

export interface ThreadsResponse {
  task: string;
  task_thread: ThreadEntry[];
  variant_threads: Record<string, ThreadEntry[]>;
}

export interface WorkspaceLimits {
  max_spawns_per_run: number;
}

/** Lean provider shape inside `snap.config.providers`. Disk-existence checks
 * and MCP server counts are NOT included — fetch `/providers` for that. */
export interface ProviderConfigEntry {
  id: string;
  name: string;
  type: string;
  config_dir: string;
  description: string | null;
}

/** Lean project shape inside `snap.config.projects`. CLAUDE.md excerpts and
 * live worktree listings are NOT included — fetch `/projects` for that. */
export interface ProjectConfigEntry {
  id: string;
  name: string;
  path: string;
  base_branch: string;
}

export interface WorkspaceConfig {
  limits: WorkspaceLimits;
  pipeline: PipelineStatus[];
  providers: ProviderConfigEntry[];
  projects: ProjectConfigEntry[];
  agents: AgentConfig[];
}

export interface ConversationContext {
  project?: string;
  task?: string;
  variant?: string;
  [key: string]: string | undefined;
}

export interface ConversationSummary {
  id: string;
  title: string;
  provider_id: string;
  model: string | null;
  context: ConversationContext;
  updated_at: string;
}

export interface ConversationMessage {
  role: "user" | "assistant" | "system";
  content: string;
  tools?: { name: string; input: unknown }[];
  ts: string;
}

export interface QueuedMessage {
  id: string;
  content: string;
}

export interface ConversationDetail extends ConversationSummary {
  session_id: string | null;
  messages: ConversationMessage[];
  queued: QueuedMessage[];
  created_at: string;
  /** A provider run is in flight engine-side, even if this window isn't streaming it. */
  running: boolean;
}

export interface Snapshot {
  tasks: Task[];
  roadmap: RoadmapItem[];
  agents: Agent[];
  runs: AgentRun[];
  config: WorkspaceConfig;
  conversations: ConversationSummary[];
  ts: string;
}

export interface AgentArgDef {
  flag: string;
  required?: boolean;
  help?: string;
}

export interface AgentSchedule {
  cron: string;
  timezone?: string;
  enabled?: boolean;
}

export interface AgentLoop {
  type: "fixed";
  amount: number;
}

/** Per-task agent settings row from
 *  `GET /tasks/{id}/agent-settings`. */
export interface TaskAgentSetting {
  agent_id: string;
  loop: AgentLoop;
}

export interface AgentConfig {
  id: string;
  name: string;
  description: string;
  manual: boolean;
  schedule: AgentSchedule | null;
  /** JSON array of expression strings; the central dispatcher tick routes
   *  matching events to this agent. */
  rules: string[];
  args: AgentArgDef[];
  next_fire: string | null;
  provider_id: string | null;
  model?: string | null;
  /** When true, a task-scoped event (e.g. a `task.update` carrying
   *  `pipeline_status`) is fanned out by the engine into one
   *  spawn per variant of the task. */
  spawn_per_variant: boolean;
  /** Opted-in skill slugs. Hidden skills are injected at spawn time
   *  regardless and are not listed here. */
  skills: string[];
}

/** A skill row from `AgentService.ListSkills` — reusable prompt fragments
 *  assembled into the agent prompt at spawn time. `hidden` skills are
 *  non-optional (always injected); the rest are opt-in. */
export interface AgentSkill {
  slug: string;
  name: string;
  description: string;
  hidden: boolean;
  position: number;
}

/** Per-agent detail from `AgentService.Get`. Extends AgentConfig with the
 * prompt body (sourced from `agents.prompt`). */
export interface AgentDetail extends AgentConfig {
  prompt?: string | null;
}

export interface LogEntry {
  kind: "init" | "text" | "tool" | "result" | "raw";
  text: string;
  detail?: string;
  toolName?: string;
  primary?: string;
  continuation?: string[];
}

export interface ProviderSummary {
  id: string;
  name: string;
  type: string;
  description: string | null;
  config_dir: string | null;
  config_dir_exists: boolean;
  mcp_server_count: number;
  mcp_endpoints: string[];
  agents_using: string[];
}

export interface ProviderMcpServer {
  name: string;
  type: string | null;
  url: string | null;
  command: string | null;
  needs_auth: boolean;
}

export interface ProviderProject {
  path: string;
  mcp_servers: ProviderMcpServer[];
  count: number;
}

/** A managed HTTP MCP server on the provider's config file, round-tripped by
 * the edit form. `auth_token` is the raw `Authorization` header value. */
export interface ProviderMcpEntry {
  key: string;
  url: string;
  auth_token: string | null;
}

export interface ProviderDetail extends ProviderSummary {
  agents_using: string[];
  config_exists: boolean;
  projects: ProviderProject[];
  mcp_servers: ProviderMcpEntry[];
  needs_auth: Record<string, unknown>;
}

export interface Project {
  id: string;
  name: string;
  path: string;
  base_branch: string | null;
  exists: boolean;
  worktrees: string[];
}

/** Modula Remote host state. Host-global — nothing here is workspace-scoped.
 * `enabled && !running` means the endpoint failed to bind; `last_error` says why. */
export interface RemoteStatus {
  enabled: boolean;
  running: boolean;
  password_set: boolean;
  node_id: string;
  direct_addresses: string[];
  connected_devices: number;
  last_error: string;
}

export interface RemoteDevice {
  id: string;
  name: string;
  platform: string;
  scope: string;
  created_at: string;
  last_seen_at: string | null;
  connected: boolean;
}

/** `expires_at` is unix epoch seconds. */
export interface PairingCode {
  qr_payload: string;
  expires_at: number;
}
