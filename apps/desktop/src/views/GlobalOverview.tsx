import { useContext } from "react";
import { AgentName } from "../components/AgentName";
import { Pill } from "../components/Pill";
import { PlanProgressBar } from "../components/PlanProgressBar";
import { Spinner } from "../components/Spinner";
import { useShortcut } from "../contexts/ShortcutsContext";
import { WorkspaceSwitcherContext } from "../contexts/WorkspaceContext";
import { pipelineStatusFor, toneDotClasses } from "../lib/pipeline";
import { planPercent } from "../lib/plan";
import { useOverview } from "../queries/overview";
import { workspaceRouter } from "../router";
import { errorMessage } from "../services/client";
import type {
  OverviewTask,
  OverviewVariant,
  OverviewWorkspace,
  PipelineStatus,
  PlanProgress,
  PlanTask,
} from "../types";

const NO_PLAN: PlanProgress = {
  tasks_total: 0,
  tasks_done: 0,
  tasks_skipped: 0,
  phases_total: 0,
};

/** Every workspace's active tasks, laid over the window below the title bar. */
export function GlobalOverview() {
  const { data, isLoading, error } = useOverview();
  const { setOverviewOpen } = useContext(WorkspaceSwitcherContext);

  useShortcut("escape", () => setOverviewOpen(false));

  return (
    <div className="fixed top-[35px] left-2 right-2 bottom-2 z-50 rounded-xl border border-edge bg-bg shadow-content overflow-y-auto font-inter">
      <div className="max-w-3xl mx-auto px-6 py-6 flex flex-col gap-6">
        <h1 className="text-sm font-medium text-fg">Global overview</h1>
        {isLoading ? (
          <Spinner />
        ) : error ? (
          <p className="text-xs text-red-600 dark:text-red-400">{errorMessage(error)}</p>
        ) : !data?.length ? (
          <p className="text-xs text-fg-subtle">No active tasks.</p>
        ) : (
          data.map((ws) => <WorkspaceSection key={ws.id} ws={ws} />)
        )}
      </div>
    </div>
  );
}

function WorkspaceSection({ ws }: { ws: OverviewWorkspace }) {
  return (
    <section className="flex flex-col gap-2">
      <h2 className="flex items-baseline gap-2 text-xs font-medium text-fg-muted">
        {ws.name}
        <span className="tabular-nums text-fg-subtle">{ws.tasks.length}</span>
      </h2>
      {ws.tasks.map((t) => (
        <TaskRow key={t.id} ws={ws} task={t} />
      ))}
    </section>
  );
}

function TaskRow({ ws, task }: { ws: OverviewWorkspace; task: OverviewTask }) {
  const { setWorkspace, setOverviewOpen } = useContext(WorkspaceSwitcherContext);
  const status = pipelineStatusFor(ws.pipeline, task.status);
  const best = bestVariant(task.variants);
  const agents = task.agents.filter((a, i, all) => all.findIndex((b) => b.name === a.name) === i);
  const reason = status?.error ? task.notes.split("\n")[0] || status.label : status?.label;

  return (
    <button
      type="button"
      onClick={() => {
        setWorkspace(ws.id);
        workspaceRouter(ws.id).navigate(`/tasks/${encodeURIComponent(task.id)}`);
        setOverviewOpen(false);
      }}
      className={`flex flex-col gap-2 w-full px-3 py-2.5 bg-card hover:bg-surface rounded-md border shadow-card text-left ${task.needs_human ? "border-yellow-300 dark:border-yellow-800" : "border-border/60"}`}
    >
      <div className="flex items-center gap-2 min-w-0">
        {task.needs_human && (
          <span className="w-2 h-2 rounded-full bg-yellow-500 shrink-0" title="Needs a human" />
        )}
        {task.external_id && (
          <span className="text-[11px] text-fg-subtle shrink-0">{task.external_id}</span>
        )}
        <span className="text-[13px] text-fg truncate">{task.title}</span>
        {agents.length > 0 && (
          <span className="ml-auto flex items-center gap-2 shrink-0">
            <span className="w-1.5 h-1.5 rounded-full bg-green-500 animate-pulse" />
            {agents.map((a) => (
              <AgentName
                key={a.name}
                name={a.name}
                iconSize="2xs"
                className="text-[11px]"
                provider={{ type: a.provider_type, name: a.provider_name }}
              />
            ))}
          </span>
        )}
      </div>
      {task.needs_human && reason && (
        <span className="text-[11px] text-yellow-700 dark:text-yellow-400 truncate">{reason}</span>
      )}
      <div className="flex items-center gap-3 flex-wrap text-[11px] text-fg-muted">
        <StationTrack pipeline={ws.pipeline} status={status} />
        {status && (
          <Pill size="sm" tone={status.tone}>
            {status.label}
          </Pill>
        )}
        <PlanProgressBar progress={best?.progress ?? NO_PLAN} status={best?.status ?? null} />
        {task.variants.length > 1 && (
          <span className="tabular-nums text-fg-subtle">
            {task.variants
              .map((v) => `v${v.position} ${planPercent(v.progress ?? NO_PLAN)}%`)
              .join(" · ")}
          </span>
        )}
      </div>
      {best && best.recent_tasks.length > 0 && <PlanWindow tasks={best.recent_tasks} />}
    </button>
  );
}

/** One segment per pipeline station: past filled, current in the status tone. */
function StationTrack({
  pipeline,
  status,
}: {
  pipeline: PipelineStatus[];
  status: PipelineStatus | null;
}) {
  const stations = [...new Set(pipeline.flatMap((p) => (p.station ? [p.station] : [])))];
  const current = status?.station ? stations.indexOf(status.station) : -1;

  return (
    <span className="flex items-center gap-1" title={status?.station ?? status?.label}>
      {stations.map((s, i) => {
        let cls = "border border-border";
        if (status?.error) cls = "border border-red-500";
        else if (i < current) cls = "bg-fg-subtle";
        else if (i === current) cls = toneDotClasses[status?.tone ?? "zinc"];
        return <span key={s} className={`w-5 h-1.5 rounded-full ${cls}`} />;
      })}
    </span>
  );
}

const MARK: Record<PlanTask["status"], string> = {
  done: "✓",
  in_progress: "●",
  pending: "○",
  skipped: "–",
};

function PlanWindow({ tasks }: { tasks: PlanTask[] }) {
  return (
    <span className="flex flex-col text-[11px]">
      {tasks.map((t) => (
        <span
          key={t.id}
          className={`flex items-center gap-2 px-1.5 py-0.5 rounded min-w-0 ${t.status === "in_progress" ? "bg-surface text-fg" : "text-fg-muted"}`}
        >
          <span className="w-3 shrink-0 text-center text-fg-subtle">{MARK[t.status]}</span>
          <span className="tabular-nums text-fg-subtle shrink-0">{t.id}</span>
          <span className={`truncate ${t.status === "skipped" ? "line-through" : ""}`}>
            {t.title}
          </span>
        </span>
      ))}
    </span>
  );
}

function bestVariant(variants: OverviewVariant[]): OverviewVariant | undefined {
  let best: OverviewVariant | undefined;
  for (const v of variants) {
    if (!best || planPercent(v.progress ?? NO_PLAN) > planPercent(best.progress ?? NO_PLAN)) {
      best = v;
    }
  }
  return best;
}
