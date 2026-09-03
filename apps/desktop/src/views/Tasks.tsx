import { ChevronDown, ChevronRight, Download, Plus } from "lucide-react";
import { useContext, useEffect, useMemo, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { AiAssist, AiAssistTrigger } from "../components/AiAssist";
import { BaseModal } from "../components/BaseModal";
import { Button } from "../components/Button";
import { HeaderSlot } from "../components/HeaderSlot";
import { ImportTaskModal } from "../components/integrations/ImportTaskModal";
import { MarkdownEditor } from "../components/MarkdownEditor";
import { Pill } from "../components/Pill";
import { SourceIcon } from "../components/SourceIcon";
import { Spinner } from "../components/Spinner";
import { TimeAgo } from "../components/TimeAgo";
import { usePipeline } from "../contexts/PipelineContext";
import { useSnapshot } from "../contexts/SnapshotContext";
import { WorkspaceContext } from "../contexts/WorkspaceContext";
import { pipelineLabel, pipelineStatusFor, pipelineTone, toneDotClasses } from "../lib/pipeline";
import { externalStatusTextClass, variantStatusTone } from "../lib/task";
import { client, errorMessage } from "../services/client";
import type { Agent, RoadmapItem, Task } from "../types";
import { logPath } from "./Logs";
import { TaskDetail } from "./TaskDetail";

//
// `/tasks`     — list spans the full content view.
// `/tasks/:id` — full-page task detail (`TaskDetailPage` below).
//
// `TasksPane` keeps a prop-based interface so it can be reused outside
// the route if needed.

export function TasksView() {
  const navigate = useNavigate();
  const { snap } = useSnapshot();
  const [creating, setCreating] = useState(false);
  const [importing, setImporting] = useState(false);
  if (!snap) return null;
  return (
    <div className="flex-1 flex flex-col overflow-hidden">
      <main className="flex-1 p-4 overflow-hidden">
        <TasksPane
          tasks={snap.tasks}
          roadmap={snap.roadmap}
          agents={snap.agents}
          selected={null}
          onSelect={(id) => {
            if (id) navigate(`/tasks/${encodeURIComponent(id)}`);
          }}
          creating={creating}
          setCreating={setCreating}
          importing={importing}
          setImporting={setImporting}
        />
      </main>
    </div>
  );
}

export function TaskDetailPage() {
  const navigate = useNavigate();
  const { id } = useParams<{ id: string }>();
  const { snap } = useSnapshot();
  if (!snap) return null;

  const task = id ? (snap.tasks.find((t) => t.id === id) ?? null) : null;
  if (!task) {
    return (
      <main className="flex-1 flex items-center justify-center text-fg-muted">task not found</main>
    );
  }
  const roadmap = snap.roadmap.find((r) => r.task === task.id);

  return (
    <main className="flex-1 overflow-hidden">
      <TaskDetail
        task={task}
        roadmap={roadmap}
        agents={snap.agents}
        runs={snap.runs}
        onOpenLog={(logName) => navigate(logPath(logName))}
      />
    </main>
  );
}

export function TasksPane({
  tasks,
  roadmap,
  agents,
  selected,
  onSelect,
  creating,
  setCreating,
  importing,
  setImporting,
}: {
  tasks: Task[];
  roadmap: RoadmapItem[];
  agents: Agent[];
  selected: string | null;
  onSelect: (id: string | null) => void;
  creating: boolean;
  setCreating: (v: boolean) => void;
  importing: boolean;
  setImporting: (v: boolean) => void;
}) {
  const ws = useContext(WorkspaceContext);
  const pipeline = usePipeline();
  const roadmapByTask = useMemo(() => {
    const m = new Map<string, RoadmapItem>();
    roadmap.forEach((r) => m.set(r.task, r));
    return m;
  }, [roadmap]);

  // Task ids with a live agent process working on them. Mirrors the matching
  // used in TaskDetail's "agents working on this task".
  const busyTasks = useMemo(() => {
    const s = new Set<string>();
    for (const t of tasks) {
      const idLower = t.id.toLowerCase();
      const busy = agents.some(
        (a) =>
          a.task === t.id ||
          (a.spec ?? "").includes(t.id) ||
          (a.branch ?? "").toLowerCase().includes(idLower),
      );
      if (busy) s.add(t.id);
    }
    return s;
  }, [tasks, agents]);

  const maxIdLen = useMemo(
    () => tasks.reduce((m, t) => Math.max(m, (t.external_id ?? "").length || 8), 4),
    [tasks],
  );
  // +18px reserves room for the source icon + gap so the id never truncates.
  const gridTemplateColumns = `calc(${maxIdLen + 2}ch + 18px) minmax(0,1fr) 120px 140px 140px 72px`;

  const sections = useMemo(() => {
    const backlog: Task[] = [];
    const active: Task[] = [];
    const done: Task[] = [];
    const declined: Task[] = [];
    for (const t of tasks) {
      const r = roadmapByTask.get(t.id);
      const terminal = pipelineStatusFor(pipeline, r?.status)?.terminal === true;
      if (t.approved === false) declined.push(t);
      else if (t.approved === null) backlog.push(t);
      else if (terminal) done.push(t);
      else active.push(t);
    }
    return { backlog, active, done, declined };
  }, [tasks, roadmapByTask, pipeline]);

  const renderRow = (t: Task) => {
    const r = roadmapByTask.get(t.id);
    const isSelected = selected === t.id;
    return (
      <button
        key={t.id}
        onClick={() => onSelect(isSelected ? null : t.id)}
        style={{ gridTemplateColumns }}
        className={`grid items-center gap-3 w-full text-left px-3 py-2 rounded hover:bg-surface/50 ${isSelected ? "bg-surface" : ""}`}
      >
        <span className="flex items-center gap-1.5 min-w-0">
          <SourceIcon source={t.source} className="w-3 h-3 shrink-0" />
          <span className="font-inter text-fg-subtle tabular-nums truncate leading-none">
            {t.external_id}
          </span>
        </span>
        <span className="flex items-center gap-2 min-w-0">
          {busyTasks.has(t.id) && <Spinner size={12} className="shrink-0" />}
          <span className="font-inter text-fg truncate">{t.title}</span>
          {(t.labels ?? []).map((l) => (
            <Pill key={l.id} size="sm" variant="flat">
              {l.name}
            </Pill>
          ))}
        </span>
        <span className="truncate">
          {t.status && (
            <span className={`font-inter text-[11px] ${externalStatusTextClass(t.status)}`}>
              {t.status}
            </span>
          )}
        </span>
        <span className="flex items-center gap-2 font-inter text-[13px] text-fg-muted truncate">
          {t.variants.map((v) => (
            <span key={v.id} className="flex items-center gap-1.5">
              <span
                className={`inline-block w-2 h-2 rounded-full shrink-0 ${toneDotClasses[variantStatusTone(v.status)]}`}
              />
              v{v.position}
            </span>
          ))}
        </span>
        <span className="min-w-0">
          {r ? (
            <span className="font-inter flex items-center gap-1.5 text-[13px] text-fg truncate">
              <span
                className={`inline-block w-2.5 h-2.5 rounded-full shrink-0 ${toneDotClasses[pipelineTone(pipeline, r.status)]}`}
              />
              {pipelineLabel(pipeline, r.status)}
            </span>
          ) : t.approved === null ? (
            <span className="font-inter flex items-center gap-1.5 text-[13px] text-fg-muted truncate">
              <span
                className={`inline-block w-2.5 h-2.5 rounded-full shrink-0 ${toneDotClasses.zinc}`}
              />
              Pending
            </span>
          ) : t.approved === false ? (
            <span className="font-inter flex items-center gap-1.5 text-[13px] text-fg-muted truncate">
              <span
                className={`inline-block w-2.5 h-2.5 rounded-full shrink-0 ${toneDotClasses.red}`}
              />
              Rejected
            </span>
          ) : null}
        </span>
        <span className="text-right">
          {t.synced_at && (
            <TimeAgo iso={t.synced_at} className="font-inter text-[11px] text-fg-subtle" />
          )}
        </span>
      </button>
    );
  };

  return (
    <section className="flex flex-col h-full overflow-hidden">
      <NewTaskForm
        workspace={ws}
        open={creating}
        onCreated={(id) => {
          setCreating(false);
          onSelect(id);
        }}
        onCancel={() => setCreating(false)}
      />
      {importing && (
        <ImportTaskModal
          workspace={ws}
          onCreated={(id) => {
            setImporting(false);
            onSelect(id);
          }}
          onCancel={() => setImporting(false)}
        />
      )}
      <div className="overflow-y-auto flex-1">
        <div className="space-y-2 pb-2">
          <HeaderSlot>
            <Button className="ml-auto" onClick={() => setImporting(true)}>
              <Download size={12} />
              Import Task
            </Button>
            <Button onClick={() => setCreating(true)}>
              <Plus size={12} />
              Create Task
            </Button>
          </HeaderSlot>
          <div className="flex items-center">
            <button
              type="button"
              className="inline-flex items-center gap-1.5 px-3 py-1 rounded-full bg-surface-2 border border-border-focus/20 text-xs text-fg transition-colors"
            >
              <span className="font-inter">Assigned</span>
            </button>
          </div>
          <TaskSection label="Backlog" tasks={sections.backlog} defaultOpen renderRow={renderRow} />
          <TaskSection label="Active" tasks={sections.active} defaultOpen renderRow={renderRow} />
          <TaskSection label="Done" tasks={sections.done} renderRow={renderRow} />
          <TaskSection label="Declined" tasks={sections.declined} renderRow={renderRow} />
        </div>
      </div>
    </section>
  );
}

function TaskSection({
  label,
  tasks,
  defaultOpen,
  renderRow,
}: {
  label: string;
  tasks: Task[];
  defaultOpen?: boolean;
  renderRow: (t: Task) => React.ReactNode;
}) {
  const [open, setOpen] = useState(defaultOpen ?? false);
  const count = tasks.length;
  return (
    <div>
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        aria-expanded={open}
        className="group w-full flex items-center gap-2 px-3 py-1.5 rounded bg-surface-2/30 text-fg-subtle hover:text-fg transition-colors"
      >
        {open ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
        <span className="font-inter text-[13px] font-medium text-fg">{label}</span>
        <span className="font-inter text-xs text-fg-subtle tabular-nums">{count}</span>
      </button>
      {open && count > 0 && <div className="mt-2">{tasks.map(renderRow)}</div>}
    </div>
  );
}

function NewTaskForm({
  workspace,
  open,
  onCreated,
  onCancel,
}: {
  workspace: string;
  open: boolean;
  onCreated: (newId: string) => void;
  onCancel: () => void;
}) {
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (open) {
      setTitle("");
      setDescription("");
      setError(null);
    }
  }, [open]);

  async function save() {
    if (!title.trim()) return;
    setBusy(true);
    setError(null);
    try {
      const out = await client.task.create(workspace, {
        title: title.trim(),
        description: description.trim() || "",
      });
      onCreated(out.id);
    } catch (e: unknown) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <BaseModal open={open} busy={busy} onCancel={onCancel} panelClassName="w-[44rem] min-h-[22rem]">
      <div className="text-base font-semibold text-fg">New task</div>
      <input
        value={title}
        onChange={(e) => setTitle(e.target.value)}
        placeholder="Title"
        autoFocus
        onKeyDown={(e) => {
          if (e.key === "Enter" && (e.metaKey || e.ctrlKey) && title.trim()) save();
        }}
        className="w-full bg-transparent text-fg placeholder-fg-subtle text-[18px] font-medium focus:outline-none"
      />
      <AiAssist
        value={description}
        onChange={setDescription}
        fieldLabel="task description"
        className="flex-1 min-h-0 flex flex-col"
      >
        <div className="flex justify-end empty:hidden">
          <AiAssistTrigger />
        </div>
        <div className="relative flex-1 min-h-0">
          <MarkdownEditor
            value={description}
            onChange={setDescription}
            onSave={save}
            padding="0"
            autoGrow
            className="w-full text-fg font-inter min-h-[60px] max-h-[50vh] overflow-y-auto"
          />
          {description.length === 0 && (
            <div className="pointer-events-none absolute top-0 left-0 text-fg-subtle font-inter text-[14px] leading-[1.7]">
              Add description…
            </div>
          )}
        </div>
      </AiAssist>
      <div className="flex items-center gap-2 mt-auto">
        <span className="ml-auto flex items-center gap-2">
          <Button onClick={onCancel} disabled={busy} tone="link">
            Cancel
          </Button>
          <Button onClick={save} disabled={busy || !title.trim()}>
            {busy ? "creating…" : "Create"}
          </Button>
        </span>
      </div>
      {error && <div className="text-[11px] text-red-400">{error}</div>}
    </BaseModal>
  );
}
