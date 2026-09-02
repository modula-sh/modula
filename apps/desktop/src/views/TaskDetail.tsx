import { useMutation, useQueryClient } from "@tanstack/react-query";
import {
  ChevronDown,
  ChevronRight,
  FileDiff,
  Folder,
  GitPullRequest,
  MoreHorizontal,
  RotateCcw,
  Trash2,
} from "lucide-react";
import { useContext, useEffect, useMemo, useRef, useState } from "react";
import { Link } from "react-router-dom";
import { AgentIdenticon } from "../components/AgentIdenticon";
import { AgentName } from "../components/AgentName";
import { AiAssist } from "../components/AiAssist";
import { Button } from "../components/Button";
import { CollapsibleSection } from "../components/CollapsibleSection";
import { ConfirmModal } from "../components/ConfirmModal";
import { ChatInputShell } from "../components/chat/ChatInputShell";
import { DropdownMenu, DropdownSelect } from "../components/DropdownMenu";
import { FeedbackText } from "../components/FeedbackText";
import { HeaderSlot } from "../components/HeaderSlot";
import { IconButton } from "../components/IconButton";
import { LabelPicker } from "../components/LabelPicker";
import { MarkdownContent } from "../components/MarkdownContent";
import { MarkdownEditor } from "../components/MarkdownEditor";
import { openUrl } from "../components/openUrl";
import { Pill } from "../components/Pill";
import { RunStatusIcon } from "../components/RunStatusIcon";
import { SourceIcon, sourceDisplayName } from "../components/SourceIcon";
import { Spinner } from "../components/Spinner";
import { TextInput } from "../components/TextInput";
import { TimeAgo } from "../components/TimeAgo";
import { AsideCard } from "../contexts/AsideCardContext";
import { usePipeline } from "../contexts/PipelineContext";
import { useRightPanel } from "../contexts/RightPanelProvider";
import { useSnapshot } from "../contexts/SnapshotContext";
import { useToast } from "../contexts/ToastContext";
import { WorkspaceContext } from "../contexts/WorkspaceContext";
import { useFeedback } from "../hooks/useFeedback";
import { toneDotClasses } from "../lib/pipeline";
import { externalStatusTextClass, sourceLabel, variantStatusTone } from "../lib/task";
import { useVariantPr } from "../queries/diff";
import { taskKeys, useTaskAgentSettings, useTaskBranches } from "../queries/task";
import { threadKeys, useThreads } from "../queries/thread";
import {
  client,
  type EditCommentBody,
  errorMessage,
  type PostCommentBody,
} from "../services/client";
import type {
  Agent,
  AgentRun,
  Approved,
  Branch,
  RoadmapItem,
  Task,
  TaskAgentSetting,
  ThreadEntry,
  ThreadsResponse,
  Variant,
  VariantStatus,
} from "../types";

interface TaskForm {
  title: string;
  description: string;
  approved: Approved;
  max_variants: number | null;
  worktree: boolean | null;
}

function taskToForm(t: Task): TaskForm {
  return {
    title: t.title,
    description: t.description ?? "",
    approved: t.approved,
    max_variants: t.max_variants ?? null,
    worktree: t.worktree ?? null,
  };
}

// Below this the left pane gets too cramped to keep the metadata card.
const ASIDE_MIN_WIDTH = 836;
/** The aside card plus the gap to the content card (w-[272px] + ml-2). */
const ASIDE_TOTAL_WIDTH = 280;
/** Debounce before remeasuring, so the card does not toggle during a resize. */
const SETTLE_MS = 100;

export function TaskDetail({
  task,
  roadmap,
  agents,
  runs,
  onOpenLog,
}: {
  task: Task;
  roadmap?: RoadmapItem;
  agents: Agent[];
  runs: AgentRun[];
  onOpenLog: (logPath: string) => void;
}) {
  const taskAgents = agents.filter(
    (a) =>
      a.task === task.id ||
      (a.spec ?? "").includes(task.id) ||
      (a.branch ?? "").toLowerCase().includes(task.id.toLowerCase()),
  );
  const taskIdLower = task.id.toLowerCase();
  // A run belongs to this task if any of its args (task / spec / branch)
  // references the task id.
  const taskRuns = runs.filter((r) => {
    const args = r.data?.args ?? {};
    if (args.task === task.id) return true;
    for (const v of Object.values(args)) {
      if (typeof v === "string" && v.toLowerCase().includes(taskIdLower)) return true;
    }
    return false;
  });
  // `run.variant` carries the variant UUID (the agent's `--variant-id` flag);
  // resolve it to the friendly "Variant N" label for display.
  const variantLabelById = new Map(
    task.variants.map((v) => [v.id, `Variant ${v.position}`] as const),
  );
  const ws = useContext(WorkspaceContext);
  const toast = useToast();
  const { snap } = useSnapshot();
  const queryClient = useQueryClient();
  const detachLabel = useMutation({
    mutationFn: (labelId: string) => client.label.detach(ws, task.id, labelId),
  });
  // Resolve `depends_on` task UUIDs to their friendly labels from the snapshot.
  const taskById = useMemo(() => {
    const m = new Map<string, Task>();
    (snap?.tasks ?? []).forEach((t) => m.set(t.id, t));
    return m;
  }, [snap?.tasks]);

  const { data: branchesData, isLoading: branchesLoading } = useTaskBranches(ws, task.id);
  const branches = useMemo<Branch[]>(
    () => (Array.isArray(branchesData) ? branchesData : []),
    [branchesData],
  );

  // SSE→Query bridge: the live snapshot tick is a signal that branches/threads
  // may have changed (agent activity). Invalidate just this task's keys so they
  // refetch while keeping previous data on screen — no flash.
  useEffect(() => {
    if (!snap?.ts) return;
    queryClient.invalidateQueries({ queryKey: taskKeys.branches(ws, task.id) });
    queryClient.invalidateQueries({ queryKey: threadKeys.detail(ws, task.id) });
  }, [snap?.ts, ws, task.id, queryClient]);

  const branchesByProject = useMemo(() => {
    const m = new Map<string, { name: string; list: Branch[] }>();
    for (const b of branches) {
      const entry = m.get(b.project.id) ?? { name: b.project.name, list: [] };
      entry.list.push(b);
      m.set(b.project.id, entry);
    }
    return Array.from(m.entries()).map(([id, { name, list }]) => ({ id, name, list }));
  }, [branches]);

  // Editable form state — all five fields auto-save with debounce. Re-sync
  // from the snapshot only when the form is NOT dirty; otherwise a snapshot
  // arriving while the user is still typing would clobber the in-flight edits.
  const [form, setForm] = useState<TaskForm>(() => taskToForm(task));
  // Direct mode forces max_variants to 1 in the wire payload (and the input is
  // displayed as locked-1 below). Backend also enforces this on write.
  const directMode = form.worktree === false;
  const effectiveMax = directMode ? 1 : form.max_variants;
  const dirty =
    form.title !== task.title ||
    form.description !== (task.description ?? "") ||
    form.approved !== task.approved ||
    effectiveMax !== (task.max_variants ?? null) ||
    form.worktree !== (task.worktree ?? null);
  const dirtyRef = useRef(dirty);
  dirtyRef.current = dirty;
  // Switching tasks always resets the form to the new task's values.
  useEffect(() => {
    setForm(taskToForm(task));
  }, [task.id]);
  // For same-id updates (snapshot reflecting our own save, or someone else's
  // edit), only sync if the form has no in-flight local edits.
  useEffect(() => {
    if (dirtyRef.current) return;
    setForm(taskToForm(task));
  }, [task.title, task.description, task.approved, task.max_variants, task.worktree]);
  const invalidMax =
    !directMode &&
    form.max_variants !== null &&
    (!Number.isInteger(form.max_variants) || form.max_variants < 1 || form.max_variants > 10);
  // Internal tasks must keep a non-empty title; an emptied title would 400 on save.
  const invalidTitle = task.source === "internal" && form.title.trim() === "";

  function update<K extends keyof TaskForm>(key: K, value: TaskForm[K]) {
    setForm((f) => ({ ...f, [key]: value }));
  }

  const saveRef = useRef<() => Promise<void>>(async () => {});
  saveRef.current = async () => {
    try {
      await client.task.update(ws, task.id, {
        // Only internal tasks may set a title; sending it for an external task
        // 403s the whole PUT, so include the key only when editable.
        ...(task.source === "internal" ? { title: form.title } : {}),
        description: form.description,
        approved: form.approved,
        max_variants: effectiveMax,
        worktree: form.worktree,
      });
    } catch (e) {
      toast.error(`Save failed: ${errorMessage(e)}`);
    }
  };

  // Debounced auto-save: re-arm a timer on every change; fire when settled.
  useEffect(() => {
    if (!dirty || invalidMax || invalidTitle) return;
    const handle = setTimeout(() => {
      saveRef.current();
    }, 600);
    return () => clearTimeout(handle);
  }, [form, dirty, invalidMax, invalidTitle]);

  const { data: threads } = useThreads(ws, task.id);

  // The card sits outside this container, so add its width back before comparing
  // — otherwise showing it shrinks the container and immediately hides it again.
  const containerRef = useRef<HTMLDivElement>(null);
  const [showAside, setShowAside] = useState(true);
  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    const measure = () =>
      setShowAside((shown) => el.offsetWidth + (shown ? ASIDE_TOTAL_WIDTH : 0) >= ASIDE_MIN_WIDTH);
    // Opening a drawer also collapses the nav, whose width transition retriggers
    // this every frame; debounce so a resize is acted on once, not per frame.
    let timer: ReturnType<typeof setTimeout>;
    const ro = new ResizeObserver(() => {
      clearTimeout(timer);
      timer = setTimeout(measure, SETTLE_MS);
    });
    ro.observe(el);
    measure();
    return () => {
      clearTimeout(timer);
      ro.disconnect();
    };
  }, []);

  return (
    <>
      <div ref={containerRef} className="h-full overflow-y-auto px-8 py-6 space-y-8">
        <div>
          {task.external_id && (
            <span className="text-fg-subtle font-medium">{task.external_id}</span>
          )}
          {task.source === "internal" ? (
            <input
              aria-label="task title"
              className="text-fg text-xl font-semibold font-inter mt-1 bg-transparent border-none outline-none focus:outline-none p-0 w-full"
              value={form.title}
              onChange={(e) => update("title", e.target.value)}
              onBlur={() => {
                if (form.title.trim() === "") update("title", task.title);
              }}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  e.preventDefault();
                  e.currentTarget.blur();
                }
              }}
            />
          ) : (
            <div className="text-fg text-xl font-semibold font-inter mt-1">{task.title}</div>
          )}
        </div>
        <div>
          <div className="text-[10px] uppercase tracking-wide text-fg-subtle mb-1">description</div>
          <div className="rounded-md border border-transparent focus-within:border-border-focus p-0 focus-within:p-4 transition-[padding,border-color] duration-150">
            <AiAssist
              value={form.description}
              onChange={(v) => update("description", v)}
              fieldLabel="task description"
            >
              <MarkdownEditor
                value={form.description}
                onChange={(v) => update("description", v)}
                onSave={() => saveRef.current()}
                padding="0"
                autoGrow
                className="text-fg font-inter"
              />
            </AiAssist>
          </div>
        </div>
        <div>
          <div className="text-[10px] uppercase tracking-wide text-fg-subtle">variants</div>
          {task.variants.length === 0 ? (
            <div className="text-fg-subtle text-xs">none yet</div>
          ) : (
            <ul className="space-y-2 mt-1">
              {task.variants.map((v) => (
                <li
                  key={v.id}
                  className="flex items-center gap-2 text-xs flex-wrap border border-border rounded px-2 py-1.5"
                >
                  <span className="font-inter font-semibold w-20 text-fg">
                    Variant {v.position}
                  </span>
                  <span className="font-inter flex items-center gap-1.5 text-[13px] text-fg">
                    <span
                      className={`inline-block w-2.5 h-2.5 rounded-full shrink-0 ${toneDotClasses[variantStatusTone(v.status)]}`}
                    />
                    {formatVariantStatus(v.status)}
                  </span>
                  <VariantActions taskId={task.id} variant={v.id} status={v.status} runs={runs} />
                </li>
              ))}
            </ul>
          )}
        </div>
        <div>
          <div className="text-[10px] uppercase tracking-wide text-fg-subtle mb-1">
            agents working on this task
          </div>
          {taskAgents.length === 0 ? (
            <div className="text-fg-subtle text-xs">none running</div>
          ) : (
            <ul className="space-y-1">
              {taskAgents.map((a) => (
                <li key={a.pid} className="text-xs text-fg flex items-center gap-2">
                  <span className="inline-flex items-center justify-center w-6 h-6 rounded-[3px] bg-surface-2 text-fg border border-border shrink-0">
                    <AgentIdenticon id={a.name} size={16} />
                  </span>
                  <span className="font-inter font-semibold">{a.name}</span>
                  <TimeAgo iso={a.started_at} className="text-fg-subtle" />
                </li>
              ))}
            </ul>
          )}
        </div>
        <div>
          <div className="text-[10px] uppercase tracking-wide text-fg-subtle mb-1">runs</div>
          {taskRuns.length === 0 ? (
            <div className="text-fg-subtle text-xs">none</div>
          ) : (
            <ul className="divide-y divide-border border border-border rounded overflow-hidden">
              {taskRuns.map((r) => {
                const ts = r.finished_at ?? r.started_at ?? r.created_at;
                const disabled = !r.log_path;
                return (
                  <li key={r.id}>
                    <button
                      type="button"
                      onClick={() => r.log_path && onOpenLog(r.log_path)}
                      disabled={disabled}
                      className="w-full flex items-center gap-2 p-1.5 hover:bg-surface/50 text-left disabled:cursor-not-allowed disabled:opacity-60"
                    >
                      <RunStatusIcon status={r.status} size={12} />
                      <AgentName name={r.agent_name} iconSize="2xs" className="text-xs mx-1" />
                      {r.variant && (
                        <Pill size="sm">{variantLabelById.get(r.variant) ?? r.variant}</Pill>
                      )}
                      <TimeAgo
                        iso={ts}
                        className="ml-auto font-inter text-[11px] text-fg-subtle whitespace-nowrap"
                      />
                    </button>
                  </li>
                );
              })}
            </ul>
          )}
        </div>
        {threads && <ReviewThreads threads={threads} taskId={task.id} variants={task.variants} />}
        <TaskActionsMenu taskId={task.id} />
      </div>

      {showAside && (
        <AsideCard>
          {/* Section 1 — source / status / synced / created */}
          <CollapsibleSection label="Details">
            <Field label="source">
              <div className="flex items-center justify-between gap-2">
                <span className="flex items-center gap-1.5 text-fg-muted capitalize leading-none">
                  <SourceIcon source={task.source} className="w-3.5 h-3.5 shrink-0" />
                  {sourceDisplayName(task.source) || "external"}
                </span>
                {task.source !== "internal" && task.url && (
                  <a
                    href={task.url}
                    target="_blank"
                    rel="noreferrer"
                    className="text-blue-400 hover:underline"
                  >
                    open in {sourceLabel(task.source)} ↗
                  </a>
                )}
              </div>
            </Field>
            {task.source !== "internal" && task.status && (
              <Field label="status">
                <span className={externalStatusTextClass(task.status)}>{task.status}</span>
              </Field>
            )}
            {task.source !== "internal" && (
              <Field label="synced">
                <TimeAgo iso={task.synced_at} className="inline-block first-letter:uppercase" />
              </Field>
            )}
            <Field label="created">
              <TimeAgo iso={task.created_at} className="inline-block first-letter:uppercase" />
            </Field>
          </CollapsibleSection>

          {/* Section 2 — approval / roadmap */}
          <CollapsibleSection label="Workflow">
            <Field label="approval">
              <DropdownSelect
                filled
                value={form.approved === true ? "true" : form.approved === false ? "false" : ""}
                onChange={(v) =>
                  update("approved", v === "true" ? true : v === "false" ? false : null)
                }
                options={[
                  { value: "", label: "Pending" },
                  { value: "true", label: "Approved" },
                  { value: "false", label: "Rejected" },
                ]}
              />
            </Field>
            <Field label="roadmap">
              {roadmap ? (
                <RoadmapStatusCell taskId={task.id} status={roadmap.status} />
              ) : (
                <span className="text-fg-subtle">not on roadmap</span>
              )}
            </Field>
          </CollapsibleSection>

          <CollapsibleSection label="Labels">
            <div className="flex flex-wrap items-center gap-1">
              {(task.labels ?? []).map((l) => (
                <Pill key={l.id} variant="flat">
                  {l.name}
                  <button
                    type="button"
                    onClick={() => detachLabel.mutate(l.id)}
                    className="text-fg-subtle hover:text-fg"
                    aria-label={`Remove ${l.name}`}
                  >
                    ×
                  </button>
                </Pill>
              ))}
              <LabelPicker taskId={task.id} attached={task.labels ?? []} />
            </div>
          </CollapsibleSection>

          {/* Section 3 — execution */}
          <CollapsibleSection label="Execution">
            <div>
              <label className="text-[10px] uppercase tracking-wide text-fg-subtle block mb-0.5">
                worktree
              </label>
              <DropdownSelect
                filled
                value={form.worktree === false ? "false" : "true"}
                onChange={(v) => update("worktree", v === "false" ? false : null)}
                options={[
                  { value: "true", label: "Yes" },
                  { value: "false", label: "No" },
                ]}
              />
            </div>
            {!directMode && (
              <div>
                <label className="text-[10px] uppercase tracking-wide text-fg-subtle block mb-0.5">
                  max variants
                </label>
                <TextInput
                  value={form.max_variants == null ? "1" : String(form.max_variants)}
                  onChange={(e) => {
                    const v = e.target.value.trim();
                    update("max_variants", v === "" ? null : Number(v));
                  }}
                  mono
                  padded
                  className="w-full"
                  inputMode="numeric"
                />
              </div>
            )}
            {invalidMax && (
              <div className="text-[11px] text-red-400">
                max_variants must be an integer between 1 and 10
              </div>
            )}
          </CollapsibleSection>

          {/* Section 4 — depends on + branches */}
          <CollapsibleSection label="Branches">
            {roadmap && roadmap.depends_on.length > 0 && (
              <div>
                <div className="text-[10px] uppercase tracking-wide text-fg-subtle">depends on</div>
                <div className="flex flex-wrap gap-1">
                  {roadmap.depends_on.map((d) => (
                    <Link
                      key={d}
                      to={`/tasks/${encodeURIComponent(d)}`}
                      title={d}
                      className="hover:opacity-80 transition-opacity"
                    >
                      <Pill>{taskById.get(d)?.external_id}</Pill>
                    </Link>
                  ))}
                </div>
              </div>
            )}
            <div>
              <div className="text-[10px] uppercase tracking-wide text-fg-subtle mb-1">
                branches
              </div>
              {branchesLoading && branches.length === 0 ? (
                <Spinner />
              ) : branchesByProject.length === 0 ? (
                <div className="text-fg-subtle text-xs">none yet</div>
              ) : (
                <ul className="space-y-2 min-w-0">
                  {branchesByProject.map(({ id, name, list }) => (
                    <li key={id} className="min-w-0">
                      <Pill>
                        <Folder size={10} />
                        {name}
                      </Pill>
                      <ul className="mt-1 space-y-1 min-w-0">
                        {list.map((b) => (
                          <li
                            key={`${b.project.id}:${b.branch}`}
                            className="text-[11px] flex items-start gap-2 text-fg-muted min-w-0"
                            title={b.worktree}
                          >
                            <span className="text-fg-subtle shrink-0">⌥</span>
                            <div className="min-w-0 flex-1">
                              <div className="font-mono text-fg truncate" title={b.branch}>
                                {b.branch}
                              </div>
                              <div className="flex items-center gap-2 text-fg-subtle">
                                {b.commits !== null && (
                                  <span className="whitespace-nowrap shrink-0">
                                    {b.commits} commit{b.commits === 1 ? "" : "s"}
                                  </span>
                                )}
                                <span className="font-mono shrink-0">{b.head}</span>
                              </div>
                            </div>
                          </li>
                        ))}
                      </ul>
                    </li>
                  ))}
                </ul>
              )}
            </div>
          </CollapsibleSection>

          {/* Section 5 — per-task agent settings */}
          <CollapsibleSection label="Agent settings">
            <AgentSettingsSection taskId={task.id} />
          </CollapsibleSection>
        </AsideCard>
      )}
    </>
  );
}

/** Per-task agent settings: one config per agent (loop amount today), with
 * add / update / delete. Available agents come from the live snapshot. */
function AgentSettingsSection({ taskId }: { taskId: string }) {
  const ws = useContext(WorkspaceContext);
  const { snap } = useSnapshot();
  const queryClient = useQueryClient();
  const configAgents = snap?.config.agents ?? [];
  const { data } = useTaskAgentSettings(ws, taskId);
  const settings = Array.isArray(data) ? data : [];
  const fb = useFeedback();

  const invalidate = () =>
    queryClient.invalidateQueries({ queryKey: taskKeys.agentSettings(ws, taskId) });

  const saveSetting = useMutation({
    mutationFn: ({ agentId, amount }: { agentId: string; amount: number }) =>
      client.task.saveAgentSetting(ws, taskId, agentId, { loop: { type: "fixed", amount } }),
    onSuccess: invalidate,
    onError: (e) => fb.err(errorMessage(e)),
  });
  const removeSetting = useMutation({
    mutationFn: (agentId: string) => client.task.deleteAgentSetting(ws, taskId, agentId),
    onSuccess: invalidate,
    onError: (e) => fb.err(errorMessage(e)),
  });

  const nameOf = (id: string) => configAgents.find((a) => a.id === id)?.name ?? id;
  const configured = new Set(settings.map((s) => s.agent_id));
  const available = configAgents.filter((a) => !configured.has(a.id));

  const save = (agentId: string, amount: number) => {
    fb.clear();
    saveSetting.mutate({ agentId, amount });
  };
  const remove = (agentId: string) => {
    fb.clear();
    removeSetting.mutate(agentId);
  };

  return (
    <>
      <div className="text-[11px] text-fg-subtle mb-1.5">Per-agent settings for this task.</div>
      {settings.length === 0 ? (
        <div className="text-fg-subtle text-xs">no agents configured</div>
      ) : (
        <ul className="space-y-2">
          {settings.map((s) => (
            <AgentSettingCard
              key={s.agent_id}
              name={nameOf(s.agent_id)}
              setting={s}
              onSaveLoop={(n) => save(s.agent_id, n)}
              onDelete={() => remove(s.agent_id)}
            />
          ))}
        </ul>
      )}
      {available.length > 0 && (
        <div className="mt-2">
          <DropdownSelect
            filled
            value=""
            onChange={(v) => v && save(v, 1)}
            options={[
              { value: "", label: "Add agent" },
              ...available.map((a) => ({ value: a.id, label: a.name })),
            ]}
          />
        </div>
      )}
      <FeedbackText feedback={fb.feedback} />
    </>
  );
}

/** One agent as an accordion row: a caret/name toggle that expands to reveal
 *  all of the agent's per-task settings (ralph-loop today; more later), with a
 *  delete control on the far right. */
function AgentSettingCard({
  name,
  setting,
  onSaveLoop,
  onDelete,
}: {
  name: string;
  setting: TaskAgentSetting;
  onSaveLoop: (amount: number) => void;
  onDelete: () => void;
}) {
  const [open, setOpen] = useState(true);
  return (
    <li>
      <div className="flex items-center gap-1">
        <button
          type="button"
          onClick={() => setOpen((v) => !v)}
          aria-expanded={open}
          className="flex flex-1 items-center gap-1.5 min-w-0 text-fg-muted hover:text-fg transition-colors"
        >
          {open ? (
            <ChevronDown size={14} className="shrink-0" />
          ) : (
            <ChevronRight size={14} className="shrink-0" />
          )}
          <span className="text-xs font-medium truncate" title={name}>
            {name}
          </span>
        </button>
        <Button tone="link" onClick={onDelete} title="remove agent">
          <Trash2 size={12} />
        </Button>
      </div>
      {open && (
        <div className="space-y-1.5 mt-1.5 pl-[18px]">
          <AgentLoopSetting amount={setting.loop.amount} onSave={onSaveLoop} />
        </div>
      )}
    </li>
  );
}

/** A single setting row under an agent: label on the left, control on the
 *  right. Mirror this shape when adding further per-agent settings. */
function AgentLoopSetting({
  amount,
  onSave,
}: {
  amount: number;
  onSave: (amount: number) => void;
}) {
  const [val, setVal] = useState(String(amount));
  useEffect(() => setVal(String(amount)), [amount]);
  const commit = () => {
    const n = Math.max(1, Math.min(100, parseInt(val, 10) || 1));
    setVal(String(n));
    if (n !== amount) onSave(n);
  };
  return (
    <div className="flex items-center gap-2">
      <span className="text-fg-muted text-[11px] flex-1">loop iterations</span>
      <TextInput
        type="number"
        min={1}
        max={100}
        value={val}
        onChange={(e) => setVal(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => {
          if (e.key === "Enter") (e.target as HTMLInputElement).blur();
        }}
        mono
        padded
        className="w-16"
        inputMode="numeric"
      />
    </div>
  );
}

function TaskActionsMenu({ taskId }: { taskId: string }) {
  const ws = useContext(WorkspaceContext);
  const [busy, setBusy] = useState(false);
  const [pending, setPending] = useState<"reset" | "delete" | null>(null);
  const fb = useFeedback();

  const modalProps =
    pending &&
    {
      reset: {
        title: `Reset task?`,
        label: "Reset task",
        body: (
          <>
            <p>This will:</p>
            <ul className="list-disc pl-4 space-y-0.5">
              <li>
                delete every git worktree + branch for <span className="font-mono">{taskId}</span>{" "}
                across all projects
              </li>
              <li>
                delete any <span className="font-mono">modula/{taskId.toLowerCase()}-*/start</span>{" "}
                tags (direct-mode markers)
              </li>
              <li>delete the spec folder, task thread, and all matching runs</li>
              <li>remove the roadmap row</li>
            </ul>
            <p>
              Then clear <span className="font-mono">variants</span> on the task. The task row
              itself stays: title, description, approval, and execution settings are kept.
            </p>
            <p className="text-fg-subtle">
              Code committed in direct mode (<span className="font-mono">worktree=false</span>) on{" "}
              <span className="font-mono">base_branch</span> is NOT reverted. This cannot be undone.
            </p>
          </>
        ),
      },
      delete: {
        title: `Delete task?`,
        label: "Delete task",
        body: <p>Are you sure?</p>,
      },
    }[pending];

  async function apply() {
    if (!pending) return;
    setBusy(true);
    fb.clear();
    try {
      if (pending === "reset") {
        await client.task.reset(ws, taskId);
        fb.ok("reset complete", { clearAfter: 6000 });
      } else {
        await client.task.delete(ws, taskId);
        fb.ok("deleted", { clearAfter: 6000 });
      }
      setPending(null);
    } catch (e: unknown) {
      fb.err(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <>
      <HeaderSlot>
        <div className="ml-auto flex items-center gap-2">
          <FeedbackText feedback={fb.feedback} />
          <DropdownMenu
            panelClassName="w-48"
            trigger={({ open, toggle }) => (
              <IconButton
                onClick={toggle}
                disabled={busy}
                title="Task actions"
                className={open ? "bg-fg/10 text-fg" : ""}
              >
                <MoreHorizontal size={16} />
              </IconButton>
            )}
          >
            {({ close }) => (
              <ul className="space-y-0.5">
                <li>
                  <button
                    type="button"
                    onClick={() => {
                      setPending("reset");
                      close();
                    }}
                    className="flex items-center gap-2 w-full text-left truncate px-2 py-1.5 rounded text-xs font-inter text-fg-muted hover:bg-surface"
                  >
                    <RotateCcw size={12} className="shrink-0" />
                    Reset
                  </button>
                </li>
                <li>
                  <button
                    type="button"
                    onClick={() => {
                      setPending("delete");
                      close();
                    }}
                    className="flex items-center gap-2 w-full text-left truncate px-2 py-1.5 rounded text-xs font-inter text-fg-muted hover:bg-surface"
                  >
                    <Trash2 size={12} className="shrink-0" />
                    Delete
                  </button>
                </li>
              </ul>
            )}
          </DropdownMenu>
        </div>
      </HeaderSlot>
      <ConfirmModal
        open={!!modalProps}
        title={modalProps?.title ?? ""}
        confirmLabel={modalProps?.label}
        busy={busy}
        onConfirm={apply}
        onCancel={() => setPending(null)}
        body={modalProps?.body}
      />
    </>
  );
}

function formatVariantStatus(s: VariantStatus | string | null | undefined): string {
  if (!s) return "Unassigned";
  return s.replace(/_/g, " ").replace(/\b\w/g, (c) => c.toUpperCase());
}

const VARIANT_STATUSES: VariantStatus[] = [
  "ready_for_workers",
  "in_progress",
  "ready_for_review",
  "in_review",
  "rework",
  "accepted",
];

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div>
      <div className="text-[10px] uppercase tracking-wide text-fg-subtle">{label}</div>
      <div className="text-fg text-xs">{children}</div>
    </div>
  );
}

function VariantActions({
  taskId,
  variant,
  status,
  runs,
}: {
  taskId: string;
  variant: string;
  status: VariantStatus | null;
  runs: AgentRun[];
}) {
  const ws = useContext(WorkspaceContext);
  const { open: openPanel } = useRightPanel();
  const { data: pr } = useVariantPr(ws, taskId, variant);
  const [busy, setBusy] = useState(false);
  const fb = useFeedback();

  const hasActiveRun = runs.some((r) => r.variant === variant && r.status === "running");

  async function setStatus(next: string) {
    if (next === status) return;
    setBusy(true);
    fb.clear();
    try {
      await client.variant.setStatus(ws, taskId, variant, next);
    } catch (e: unknown) {
      fb.err(errorMessage(e), { clearAfter: 5000 });
    } finally {
      setBusy(false);
    }
  }

  return (
    <span className="flex items-center gap-1 ml-auto">
      <Button
        onClick={() =>
          openPanel({ type: "branch-diff", workspace: ws, task: taskId, variant }, "card")
        }
        className="!py-0.5 !text-[10px]"
      >
        <FileDiff size={12} />
        Changes
      </Button>
      {pr?.projects
        .filter((p) => p.pr_url)
        .map((p) => (
          <Button
            key={p.name}
            onClick={() => p.pr_url && openUrl(p.pr_url)}
            title={p.name}
            className="!py-0.5 !text-[10px]"
          >
            <GitPullRequest size={12} />
            {p.pr_number ? `#${p.pr_number}` : "PR"}
          </Button>
        ))}
      <DropdownSelect
        value={status ?? ""}
        placeholder="Unassigned"
        onChange={setStatus}
        disabled={busy || hasActiveRun}
        title={hasActiveRun ? "A run is in progress for this variant" : "Set variant status"}
        options={VARIANT_STATUSES.map((s) => ({
          value: s,
          label: formatVariantStatus(s),
        }))}
      />
      <FeedbackText feedback={fb.feedback} />
    </span>
  );
}

function RoadmapStatusCell({ taskId, status }: { taskId: string; status: string }) {
  const ws = useContext(WorkspaceContext);
  const pipeline = usePipeline();
  const [busy, setBusy] = useState(false);
  const fb = useFeedback();

  async function onChange(next: string) {
    if (next === status) return;
    setBusy(true);
    fb.clear();
    try {
      await client.roadmap.setStatus(ws, taskId, next);
    } catch (e: unknown) {
      fb.err(errorMessage(e), { clearAfter: 5000 });
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="space-y-1">
      <DropdownSelect
        filled
        value={status}
        onChange={onChange}
        disabled={busy}
        options={pipeline.map((p) => ({ value: p.key, label: p.label }))}
      />
      <FeedbackText feedback={fb.feedback} />
    </div>
  );
}

function ReviewThreads({
  threads,
  taskId,
  variants,
}: {
  threads: ThreadsResponse;
  taskId: string;
  variants: Variant[];
}) {
  const variantNames = Object.keys(threads.variant_threads).sort();
  // Thread keys are variant UUIDs; map them to the friendly "Variant N" label
  // for the tab text (the value passed to the API stays the UUID).
  const variantLabelById = new Map(variants.map((v) => [v.id, `Variant ${v.position}`] as const));
  type Tab = "task" | string;
  const initialTab: Tab = threads.task_thread.length > 0 ? "task" : (variantNames[0] ?? "task");
  const [tab, setTab] = useState<Tab>(initialTab);

  const visibleEntries =
    tab === "task" ? threads.task_thread : (threads.variant_threads[tab] ?? []);

  return (
    <div className="space-y-6">
      <div className="space-y-1.5">
        <div className="text-[10px] uppercase tracking-wide text-fg-subtle">thread</div>
        <div className="flex items-center gap-1 flex-wrap">
          <ReviewTab
            active={tab === "task"}
            count={threads.task_thread.length}
            onClick={() => setTab("task")}
          >
            task
          </ReviewTab>
          {variantNames.map((v) => (
            <ReviewTab
              key={v}
              active={tab === v}
              count={threads.variant_threads[v].length}
              onClick={() => setTab(v)}
            >
              {variantLabelById.get(v) ?? v}
            </ReviewTab>
          ))}
        </div>
      </div>
      {visibleEntries.length === 0 ? (
        <div className="text-fg-subtle text-xs">no entries</div>
      ) : (
        <ul className="space-y-6">
          {visibleEntries.map((e) => (
            <ThreadEntryView key={e.id} entry={e} taskId={taskId} />
          ))}
        </ul>
      )}
      <ThreadComposer taskId={taskId} variant={tab === "task" ? null : tab} />
    </div>
  );
}

function ThreadComposer({ taskId, variant }: { taskId: string; variant: string | null }) {
  const ws = useContext(WorkspaceContext);
  const queryClient = useQueryClient();
  const [content, setContent] = useState("");
  const fb = useFeedback();

  const postComment = useMutation({
    mutationFn: (body: PostCommentBody) => client.thread.postComment(ws, taskId, body),
    onSuccess: () => {
      setContent("");
      queryClient.invalidateQueries({ queryKey: threadKeys.detail(ws, taskId) });
    },
    onError: (e) => fb.err(errorMessage(e), { clearAfter: 5000 }),
  });

  function post() {
    const trimmed = content.trim();
    if (!trimmed) return;
    fb.clear();
    postComment.mutate({ content: trimmed, ...(variant ? { variant } : {}) });
  }

  return (
    <AiAssist value={content} onChange={setContent} fieldLabel="comment">
      <ChatInputShell
        value={content}
        onChange={setContent}
        onSubmit={post}
        placeholder="Add a comment…"
        bottomRow={
          <>
            <FeedbackText feedback={fb.feedback} />
            <div className="ml-auto">
              <Button onClick={post} disabled={postComment.isPending || !content.trim()}>
                {postComment.isPending ? "posting…" : "Post"}
              </Button>
            </div>
          </>
        }
      />
    </AiAssist>
  );
}

function ReviewTab({
  active,
  count,
  onClick,
  children,
}: {
  active: boolean;
  count: number;
  onClick: () => void;
  children: React.ReactNode;
}) {
  // Slightly smaller than NavButton; otherwise same active/inactive treatment.
  return (
    <Button
      tone="tab"
      active={active}
      onClick={onClick}
      className="!px-2 !py-0.5 !text-[11px] flex items-center gap-1.5"
    >
      {children}
      <span className="text-fg-subtle">·</span>
      <span className={active ? "text-fg-muted" : "text-fg-subtle"}>{count}</span>
    </Button>
  );
}

function authorNameClass(a: string): string {
  if (a === "reviewer") return "text-purple-300";
  if (a === "code-reviewer") return "text-blue-300";
  if (a === "worker") return "text-yellow-300";
  if (a === "human") return "text-green-300";
  return "text-fg";
}

function verdictTone(
  v: string | undefined,
): "zinc" | "green" | "yellow" | "red" | "blue" | "purple" {
  if (!v) return "zinc";
  if (v === "ACCEPT" || v === "APPROVE") return "green";
  if (v === "REQUEST_CHANGES" || v === "KICK_BACK") return "red";
  return "zinc";
}

function kindTone(k: string): "zinc" | "green" | "yellow" | "red" | "blue" | "purple" {
  if (k === "question") return "yellow";
  if (k === "rework") return "purple";
  return "zinc";
}

function ThreadEntryView({ entry, taskId }: { entry: ThreadEntry; taskId: string }) {
  const ws = useContext(WorkspaceContext);
  const queryClient = useQueryClient();
  const fb = useFeedback();
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(entry.content ?? "");
  const [confirmOpen, setConfirmOpen] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const invalidate = () =>
    queryClient.invalidateQueries({ queryKey: threadKeys.detail(ws, taskId) });

  const editComment = useMutation({
    mutationFn: (body: EditCommentBody) => client.thread.editComment(ws, taskId, entry.id, body),
    onSuccess: () => {
      setEditing(false);
      invalidate();
    },
    onError: (e) => fb.err(errorMessage(e), { clearAfter: 5000 }),
  });

  const deleteComment = useMutation({
    mutationFn: () => client.thread.deleteComment(ws, taskId, entry.id, "human"),
    onSuccess: () => {
      setConfirmOpen(false);
      invalidate();
    },
    onError: (e) => fb.err(errorMessage(e), { clearAfter: 5000 }),
  });

  // Grow the borderless editor to fit its content so it reads like the comment.
  useEffect(() => {
    const el = textareaRef.current;
    if (!editing || !el) return;
    el.style.height = "auto";
    el.style.height = `${el.scrollHeight}px`;
  }, [editing, draft]);

  // On entering edit mode, focus with the caret at the end.
  useEffect(() => {
    if (!editing) return;
    const el = textareaRef.current;
    if (!el) return;
    el.focus();
    el.setSelectionRange(el.value.length, el.value.length);
  }, [editing]);

  function startEdit() {
    setDraft(entry.content ?? "");
    setEditing(true);
  }

  const trimmedDraft = draft.trim();
  const unchanged = trimmedDraft === (entry.content ?? "").trim();
  function save() {
    if (!trimmedDraft || unchanged) return;
    fb.clear();
    editComment.mutate({ content: trimmedDraft, author: "human" });
  }

  // `kind: comment` is the default and not worth a chip; verdict gets its own
  // ACCEPT/REQUEST_CHANGES pill below; only `question` and `rework` warrant a chip.
  const showKindChip = entry.kind === "question" || entry.kind === "rework";
  // The owner of a plain comment may edit/delete it; the menu is the UI half of
  // the engine's owner-only rule (the dashboard user acts as "human").
  const canMutate = entry.author === "human" && entry.kind === "comment";
  return (
    <li className="space-y-1">
      <div className="flex items-center gap-2 flex-wrap text-[11px]">
        <span className={`font-semibold text-xs ${authorNameClass(entry.author)}`}>
          {entry.author}
        </span>
        <TimeAgo iso={entry.ts} className="text-fg-subtle" />
        {showKindChip && <Pill tone={kindTone(entry.kind)}>{entry.kind}</Pill>}
        {entry.verdict && <Pill tone={verdictTone(entry.verdict)}>{entry.verdict}</Pill>}
        {entry.round !== undefined && <span className="text-fg-subtle">round {entry.round}</span>}
        {entry.affected_variants && entry.affected_variants.length > 0 && (
          <span className="text-fg-subtle">
            → <span className="text-fg">{entry.affected_variants.join(", ")}</span>
          </span>
        )}
        {canMutate && !editing && (
          <div className="ml-auto">
            <DropdownMenu
              panelClassName="w-32"
              trigger={({ toggle }) => (
                <IconButton onClick={toggle} title="Comment actions">
                  <MoreHorizontal size={14} />
                </IconButton>
              )}
            >
              {({ close }) => (
                <ul className="space-y-0.5">
                  <li>
                    <button
                      type="button"
                      onClick={() => {
                        startEdit();
                        close();
                      }}
                      className="block w-full text-left px-2 py-1.5 rounded text-xs font-inter text-fg-muted hover:bg-surface"
                    >
                      Edit
                    </button>
                  </li>
                  <li>
                    <button
                      type="button"
                      onClick={() => {
                        setConfirmOpen(true);
                        close();
                      }}
                      className="block w-full text-left px-2 py-1.5 rounded text-xs font-inter text-red-300 hover:bg-surface"
                    >
                      Delete
                    </button>
                  </li>
                </ul>
              )}
            </DropdownMenu>
          </div>
        )}
      </div>
      {editing ? (
        <div className="space-y-1.5">
          <textarea
            ref={textareaRef}
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            className="w-full bg-transparent border-0 p-0 text-[13px] text-fg leading-relaxed resize-none focus:outline-none focus:ring-0"
          />
          <div className="flex items-center gap-2">
            <Button onClick={save} disabled={editComment.isPending || !trimmedDraft || unchanged}>
              {editComment.isPending ? "saving…" : "Save"}
            </Button>
            <Button tone="link" onClick={() => setEditing(false)} disabled={editComment.isPending}>
              Cancel
            </Button>
          </div>
        </div>
      ) : (
        entry.content && <MarkdownContent value={entry.content} className="selectable" />
      )}
      <FeedbackText feedback={fb.feedback} />
      <ConfirmModal
        open={confirmOpen}
        title="Delete this comment?"
        body="You cannot undo this action"
        confirmLabel="Delete"
        busy={deleteComment.isPending}
        onConfirm={() => deleteComment.mutate()}
        onCancel={() => setConfirmOpen(false)}
      />
    </li>
  );
}
