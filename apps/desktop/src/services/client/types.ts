// Request-body DTOs and response shapes for the service layer. Response
// interfaces that already live in `src/types.ts` are reused from there; this
// file only adds bodies shaped inline at call sites today plus response shapes
// that were previously declared locally in components.

import type {
  AgentArgDef,
  AgentLoop,
  AgentSchedule,
  Approved,
  ConversationContext,
  FilePatch,
  ProviderMcpEntry,
} from "../../types";

export interface WorkspaceCreated {
  id: string;
  name: string;
  path: string;
}

export interface CreateWorkspaceBody {
  name: string;
  description?: string;
}

export interface CreateTaskBody {
  title: string;
  description: string;
}

/** External upsert for the import flow — dedups on `(source, external_id)`. */
export interface UpsertTaskBody {
  source: string;
  external_id: string;
  title: string;
  description?: string;
  url?: string;
  source_data?: Record<string, unknown>;
}

export interface UpdateTaskBody {
  /** Internal-source tasks only; omit for external tasks (backend 403s on presence). */
  title?: string;
  description: string;
  approved: Approved;
  max_variants: number | null;
  worktree: boolean | null;
}

export interface AgentLoopBody {
  loop: AgentLoop;
}

export interface CreateLabelBody {
  name: string;
  type?: string;
}

export interface PostCommentBody {
  content: string;
  variant?: string;
}

export interface EditCommentBody {
  content: string;
  author: string;
}

export interface AgentWriteBody {
  /** Present on create, omitted on update. */
  name?: string;
  description: string;
  provider_id: string;
  model: string | null;
  manual: boolean;
  schedule: AgentSchedule | null;
  rules: string[];
  args: AgentArgDef[];
  prompt: string;
  spawn_per_variant: boolean;
  skills: string[];
}

export interface CreateProjectBody {
  name: string;
  path: string;
  base_branch: string;
}

export interface CloneProjectBody {
  name: string;
  path: string;
  git_url: string;
}

export interface UpdateProjectBody {
  path: string;
  base_branch: string;
}

export interface RepoBranches {
  is_git: boolean;
  branches: string[];
  default_branch: string | null;
}

export interface ProviderWriteBody {
  name: string;
  type: string;
  config_dir: string;
  description: string | null;
  /** Omitted leaves the config file untouched; present reconciles the managed
   * HTTP MCP servers to exactly this list. */
  mcp_servers?: ProviderMcpEntry[];
}

export interface GenerateTextBody {
  provider_id: string;
  model?: string;
  instruction: string;
  field_label?: string;
}

export interface CreateConversationBody {
  provider_id: string;
  title?: string;
  model?: string;
  context: ConversationContext;
}

export interface WikiNode {
  name: string;
  path: string;
  type: "file" | "dir";
  children?: WikiNode[];
}

export interface WikiFile {
  path: string;
  content: string;
}

export interface WikiFileBody {
  path: string;
  content: string;
}

export interface ToolStatus {
  id: string;
  installed: boolean;
}

export interface NumstatFile {
  path: string;
  additions: number;
  deletions: number;
}

export interface NumstatBlock {
  files: NumstatFile[];
  totals: { files: number; additions: number; deletions: number };
}

export interface WorkingDiff {
  staged: NumstatBlock;
  unstaged: NumstatBlock;
  untracked: NumstatBlock;
  branch: string | null;
}

export interface Commit {
  sha: string;
  short: string;
  author: string;
  time: number;
  subject: string;
}

export interface CommitsResponse {
  commits: Commit[];
}

export interface DiffTextResponse {
  staged: FilePatch[];
  unstaged: FilePatch[];
  untracked: FilePatch[];
  branch: string | null;
}
