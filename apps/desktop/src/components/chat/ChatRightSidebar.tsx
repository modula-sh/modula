import { useQueries, useQueryClient } from "@tanstack/react-query";
import { ChevronDown, ChevronRight, ChevronUp, FileDiff, GitCommitHorizontal } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { Link } from "react-router-dom";
import { useRightPanel } from "../../contexts/RightPanelProvider";
import { useSnapshot } from "../../contexts/SnapshotContext";
import { projectKeys, useCommitDiff } from "../../queries/project";
import { useTaskBranches } from "../../queries/task";
import type {
  Commit,
  CommitsResponse,
  NumstatBlock,
  NumstatFile,
  WorkingDiff,
} from "../../services/client";
import { client } from "../../services/client";
import type { ConversationContext, ProjectConfigEntry } from "../../types";
import { Spinner } from "../Spinner";
import { TimeAgo } from "../TimeAgo";

interface ProjectTarget {
  project: ProjectConfigEntry;
  branch?: string;
  /** When the conversation is variant-scoped, list only commits ahead of base. */
  since?: string;
}

// Right-side metadata pane: per-project working-tree diffs + recent commits,
// scoped to whatever the conversation is about.
export function ChatRightSidebar({
  workspace,
  context,
  refreshNonce,
}: {
  workspace: string;
  context: ConversationContext;
  /** Bump to force a refetch (e.g. after a turn ends). */
  refreshNonce: number;
}) {
  const { snap } = useSnapshot();
  const { open: openPanel } = useRightPanel();
  const queryClient = useQueryClient();

  const isVariantScope = !!context.task && !!context.variant;
  const { data: branchRows, isLoading: branchesLoading } = useTaskBranches(
    workspace,
    isVariantScope ? (context.task ?? "") : "",
  );

  // Resolve which projects (and, for variant scope, which branch) the
  // conversation is about, matching the branch tagged with this variant's
  // position. The UI stays agnostic to the branch-name format.
  const targets = useMemo<ProjectTarget[]>(() => {
    const allProjects = snap?.config?.projects ?? [];
    const scoped = context.project
      ? allProjects.filter((p) => p.id === context.project)
      : allProjects;
    if (!isVariantScope) return scoped.map((project) => ({ project }));
    const variant = snap?.tasks
      .find((t) => t.id === context.task)
      ?.variants.find((v) => v.id === context.variant);
    if (!variant) return [];
    const branchByProject = new Map<string, string>();
    for (const r of branchRows ?? []) {
      if (r.variant_position === variant.position) {
        branchByProject.set(r.project.id, r.branch);
      }
    }
    return scoped.flatMap((project) => {
      const branch = branchByProject.get(project.id);
      return branch ? [{ project, branch, since: project.base_branch }] : [];
    });
  }, [snap, context, isVariantScope, branchRows]);

  const diffQueries = useQueries({
    queries: targets.map((t) => ({
      queryKey: projectKeys.diff(workspace, t.project.id, t.branch),
      queryFn: () => client.project.diff(workspace, t.project.id, t.branch),
      refetchInterval: 2_000,
    })),
  });
  const commitQueries = useQueries({
    queries: targets.map((t) => ({
      queryKey: projectKeys.commits(workspace, t.project.id, t.branch, t.since),
      queryFn: () =>
        client.project.commits(workspace, t.project.id, { branch: t.branch, since: t.since }),
      refetchInterval: 2_000,
    })),
  });

  const diffs: Record<string, WorkingDiff> = {};
  const commits: Record<string, CommitsResponse> = {};
  targets.forEach((t, i) => {
    const d = diffQueries[i]?.data;
    if (d) diffs[t.project.id] = d;
    const c = commitQueries[i]?.data;
    if (c) commits[t.project.id] = c;
  });

  // A turn ending is an external signal that the working tree likely changed —
  // refetch diffs/commits immediately instead of waiting for the next poll.
  useEffect(() => {
    if (refreshNonce === 0) return;
    queryClient.invalidateQueries({ queryKey: ["projects", workspace, "diff"] });
    queryClient.invalidateQueries({ queryKey: ["projects", workspace, "commits"] });
  }, [refreshNonce, workspace, queryClient]);

  const diffProjects = targets.filter((t) => {
    const d = diffs[t.project.id];
    return (
      d &&
      (d.staged.files.length > 0 || d.unstaged.files.length > 0 || d.untracked.files.length > 0)
    );
  });
  const commitProjects = targets.filter((t) => (commits[t.project.id]?.commits.length ?? 0) > 0);

  // Real branch name from a resolved target (null until one resolves).
  const variantBranch = targets.find((t) => t.branch)?.branch ?? null;

  // Variant-scoped, but no live worktree resolved its branch (e.g. merged and
  // cleaned up). Show an explanatory state instead of an empty pane.
  const variant = isVariantScope
    ? snap?.tasks.find((t) => t.id === context.task)?.variants.find((v) => v.id === context.variant)
    : undefined;
  // Wait for the branches query to settle so we don't flash this while loading.
  const variantUnresolved = isVariantScope && !variantBranch && !branchesLoading;

  if (diffProjects.length === 0 && commitProjects.length === 0 && !isVariantScope) return null;

  return (
    <aside className="w-72 shrink-0 flex flex-col border-l border-edge overflow-hidden">
      <div className="shrink-0 h-12 flex items-center gap-2 px-3 border-b border-edge">
        <span className="text-xs text-fg font-inter font-medium truncate">
          {variantBranch ?? "Changes"}
        </span>
      </div>
      <div className="flex-1 overflow-y-auto">
        {variantUnresolved && (
          <div className="p-3 flex flex-col gap-2 text-xs font-inter text-fg-subtle">
            <p>No active worktree for this variant. It may have been merged and cleaned up.</p>
            {variant?.status && (
              <p>
                Status: <span className="text-fg">{variant.status}</span>
              </p>
            )}
            {context.task && (
              <Link
                to={`/tasks/${context.task}`}
                className="w-full flex items-center justify-center gap-2 px-3 py-2 rounded transition-colors text-[13px] font-medium bg-surface-2/30 text-fg-subtle hover:text-fg hover:bg-surface-2/50"
              >
                View task
              </Link>
            )}
          </div>
        )}
        {isVariantScope && variantBranch && (
          <div className="p-3">
            <button
              type="button"
              onClick={() =>
                openPanel({
                  type: "branch-diff",
                  workspace,
                  task: context.task!,
                  variant: context.variant!,
                })
              }
              className="w-full flex items-center gap-1 px-3 py-2 text-xs font-inter text-fg hover:bg-surface border border-border rounded-lg shadow-panel transition-colors"
            >
              <FileDiff size={12} className="text-fg-subtle shrink-0" />
              <span className="text-fg-subtle shrink-0">Changes</span>
              <span className="truncate flex-1 min-w-0" title={variantBranch}>
                {variantBranch}
              </span>
              <span className="text-fg-subtle shrink-0">→</span>
            </button>
          </div>
        )}
        {diffProjects.length > 0 && (
          <PanelSection label="Changes" icon={<FileDiff size={12} />}>
            <div className="space-y-2">
              {diffProjects.map((t) => (
                <ProjectDiffs
                  key={t.project.id}
                  projectLabel={t.project.name}
                  branch={t.branch ?? diffs[t.project.id]?.branch ?? null}
                  data={diffs[t.project.id]!}
                  onFileClick={(group, path) =>
                    openPanel({
                      type: "diff",
                      workspace,
                      project: t.project.id,
                      branch: t.branch,
                      focusFile: path,
                      focusGroup: group,
                    })
                  }
                />
              ))}
            </div>
          </PanelSection>
        )}
        {commitProjects.length > 0 && (
          <PanelSection label="Commits" icon={<GitCommitHorizontal size={12} />}>
            <div className="space-y-2">
              {commitProjects.map((t) => (
                <ProjectCommits
                  key={t.project.id}
                  workspace={workspace}
                  projectId={t.project.id}
                  projectLabel={t.project.name}
                  branch={t.branch}
                  displayBranch={t.branch ?? diffs[t.project.id]?.branch ?? null}
                  data={commits[t.project.id]!}
                />
              ))}
            </div>
          </PanelSection>
        )}
      </div>
    </aside>
  );
}

function PanelSection({
  label,
  icon,
  children,
  defaultOpen = true,
}: {
  label: string;
  icon?: React.ReactNode;
  children: React.ReactNode;
  defaultOpen?: boolean;
}) {
  const [open, setOpen] = useState(defaultOpen);
  return (
    <div className="border-b border-edge">
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        aria-expanded={open}
        className="w-full flex items-center justify-between gap-2 px-3 py-2 text-[10px] uppercase tracking-wide text-fg-subtle hover:text-fg transition-colors"
      >
        <span className="flex items-center gap-1.5">
          {icon}
          {label}
        </span>
        {open ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
      </button>
      {open && <div className="px-3 pb-3 space-y-3">{children}</div>}
    </div>
  );
}

function ProjectHeader({
  label,
  branch,
  open,
  onToggle,
  trailing,
}: {
  label: string;
  branch?: string | null;
  open: boolean;
  onToggle: () => void;
  trailing?: React.ReactNode;
}) {
  return (
    <button
      type="button"
      onClick={onToggle}
      className="w-full flex items-center gap-2 text-[11px] text-fg-subtle hover:text-fg"
    >
      {open ? (
        <ChevronUp size={12} className="shrink-0" />
      ) : (
        <ChevronDown size={12} className="shrink-0" />
      )}
      <span className="font-mono font-semibold shrink-0 text-left">{label}</span>
      {branch && (
        <span className="font-mono text-fg-subtle/60 truncate min-w-0" title={branch}>
          {branch}
        </span>
      )}
      <span className="flex-1" />
      {trailing}
    </button>
  );
}

function NumstatList({
  files,
  onFileClick,
}: {
  files: NumstatFile[];
  onFileClick?: (path: string) => void;
}) {
  return (
    <ul className="space-y-0.5">
      {files.map((f) => {
        const stats = (
          <span className="shrink-0 flex gap-2">
            <span className="text-green-500">+{f.additions}</span>
            <span className="text-red-500">−{f.deletions}</span>
          </span>
        );
        if (!onFileClick) {
          return (
            <li key={f.path} className="flex items-center gap-2 text-[11px] text-fg-muted">
              <span className="font-inter truncate flex-1 min-w-0" title={f.path}>
                {f.path}
              </span>
              {stats}
            </li>
          );
        }
        return (
          <li key={f.path}>
            <button
              type="button"
              onClick={() => onFileClick(f.path)}
              className="w-full flex items-center gap-2 text-[11px] text-fg-muted hover:text-fg text-left"
            >
              <span className="font-inter truncate flex-1 min-w-0" title={f.path}>
                {f.path}
              </span>
              {stats}
            </button>
          </li>
        );
      })}
    </ul>
  );
}

function DiffGroup({
  label,
  block,
  onFileClick,
}: {
  label: string;
  block: NumstatBlock;
  onFileClick?: (path: string) => void;
}) {
  const [open, setOpen] = useState(true);
  if (block.files.length === 0) return null;
  return (
    <div>
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        className="w-full flex items-center gap-2 text-[10px] uppercase tracking-wide text-fg-subtle/60 hover:text-fg mb-1"
      >
        {open ? (
          <ChevronDown size={12} className="shrink-0" />
        ) : (
          <ChevronRight size={12} className="shrink-0" />
        )}
        <span className="flex-1 text-left">{label}</span>
        <span className="shrink-0 flex gap-2">
          <span className="text-green-500">+{block.totals.additions}</span>
          <span className="text-red-500">−{block.totals.deletions}</span>
        </span>
      </button>
      {open && <NumstatList files={block.files} onFileClick={onFileClick} />}
    </div>
  );
}

type DiffGroupName = "staged" | "unstaged" | "untracked";

function ProjectDiffs({
  projectLabel,
  branch,
  data,
  onFileClick,
}: {
  projectLabel: string;
  branch?: string | null;
  data: WorkingDiff;
  onFileClick?: (group: DiffGroupName, path: string) => void;
}) {
  const [open, setOpen] = useState(true);
  const totals = {
    additions:
      data.staged.totals.additions +
      data.unstaged.totals.additions +
      data.untracked.totals.additions,
    deletions: data.staged.totals.deletions + data.unstaged.totals.deletions,
  };
  return (
    <div>
      <ProjectHeader
        label={projectLabel}
        branch={branch}
        open={open}
        onToggle={() => setOpen((v) => !v)}
        trailing={
          <span className="shrink-0 flex gap-2">
            <span className="text-green-500">+{totals.additions}</span>
            <span className="text-red-500">−{totals.deletions}</span>
          </span>
        }
      />
      {open && (
        <div className="mt-1 space-y-2">
          <DiffGroup
            label="Staged"
            block={data.staged}
            onFileClick={onFileClick ? (p) => onFileClick("staged", p) : undefined}
          />
          <DiffGroup
            label="Unstaged"
            block={data.unstaged}
            onFileClick={onFileClick ? (p) => onFileClick("unstaged", p) : undefined}
          />
          <DiffGroup
            label="Untracked"
            block={data.untracked}
            onFileClick={onFileClick ? (p) => onFileClick("untracked", p) : undefined}
          />
        </div>
      )}
    </div>
  );
}

function ProjectCommits({
  workspace,
  projectId,
  projectLabel,
  branch,
  displayBranch,
  data,
}: {
  workspace: string;
  projectId: string;
  projectLabel: string;
  branch?: string;
  displayBranch?: string | null;
  data: CommitsResponse;
}) {
  const [open, setOpen] = useState(true);
  return (
    <div>
      <ProjectHeader
        label={projectLabel}
        branch={displayBranch}
        open={open}
        onToggle={() => setOpen((v) => !v)}
      />
      {open && (
        <ul className="mt-1 space-y-2">
          {data.commits.map((c) => (
            <CommitRow
              key={c.sha}
              workspace={workspace}
              projectId={projectId}
              branch={branch}
              commit={c}
            />
          ))}
        </ul>
      )}
    </div>
  );
}

function CommitRow({
  workspace,
  projectId,
  branch,
  commit,
}: {
  workspace: string;
  projectId: string;
  branch?: string;
  commit: Commit;
}) {
  const [open, setOpen] = useState(false);
  const { data: diff, isFetching } = useCommitDiff(workspace, projectId, commit.sha, branch, open);

  return (
    <li className="text-[11px] text-fg-muted">
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        className="w-full flex items-center gap-2 text-left hover:text-fg"
      >
        {open ? (
          <ChevronDown size={12} className="shrink-0" />
        ) : (
          <ChevronRight size={12} className="shrink-0" />
        )}
        <span className="font-mono text-fg-subtle/60 shrink-0" title={`by ${commit.author}`}>
          {commit.short}
        </span>
        <span className="font-inter truncate flex-1 min-w-0" title={commit.subject}>
          {commit.subject}
        </span>
        <TimeAgo
          iso={new Date(commit.time * 1000).toISOString()}
          className="shrink-0 text-fg-subtle/60 text-[10px]"
        />
      </button>
      {open && (
        <div className="mt-1 pl-5">
          {isFetching && !diff && <Spinner />}
          {diff && diff.files.length > 0 && <NumstatList files={diff.files} />}
          {diff && diff.files.length === 0 && !isFetching && (
            <span className="text-fg-subtle">no file changes</span>
          )}
        </div>
      )}
    </li>
  );
}
