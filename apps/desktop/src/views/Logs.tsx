import { LayoutGrid, MoreHorizontal } from "lucide-react";
import { useContext, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { DropdownMenu } from "../components/DropdownMenu";
import { FeedbackText } from "../components/FeedbackText";
import { Pill } from "../components/Pill";
import { RunStatusIcon } from "../components/RunStatusIcon";
import { TabsNav } from "../components/TabsNav";
import { TimeAgo } from "../components/TimeAgo";
import { useSnapshot } from "../contexts/SnapshotContext";
import { WorkspaceContext } from "../contexts/WorkspaceContext";
import { useFeedback } from "../hooks/useFeedback";
import { client, errorMessage } from "../services/client";
import type { AgentRun } from "../types";
import { LogViewer } from "./LogViewer";

/** Build the URL for a run's streamed log page. */
export function logPath(logName: string) {
  return `/agents/logs/${encodeURIComponent(logName)}`;
}

export function LogsView() {
  const ws = useContext(WorkspaceContext);
  const { snap } = useSnapshot();
  const navigate = useNavigate();
  const runs = snap?.runs ?? [];
  // `run.task` / `run.variant` carry UUIDs (the agent's `--task-id` /
  // `--variant-id` flags); resolve them to the human-facing labels —
  // the task's external id ("MOD-001") and the variant's "Variant N".
  const externalById = new Map((snap?.tasks ?? []).map((t) => [t.id, t.external_id ?? null]));
  const variantLabelById = new Map(
    (snap?.tasks ?? []).flatMap((t) =>
      t.variants.map((v) => [v.id, `Variant ${v.position}`] as const),
    ),
  );
  // Map each live process's run id → pid so a running run can offer Kill.
  const pidByRunId = new Map((snap?.agents ?? []).map((a) => [a.run_id, a.pid] as const));

  return (
    <div className="flex-1 flex flex-col overflow-hidden">
      <TabsNav />
      <main className="flex-1 p-3 overflow-hidden">
        <section className="flex flex-col h-full overflow-hidden border border-border rounded">
          <div className="h-10 shrink-0 flex items-center gap-2 px-3 border-b border-border">
            <span className="text-fg-muted text-xs uppercase tracking-wide">Runs</span>
            <span className="text-fg-subtle text-xs">({runs.length})</span>
          </div>
          <div className="divide-y divide-border overflow-y-auto flex-1">
            {runs.length === 0 && <div className="px-3 py-4 text-fg-subtle text-xs">no runs</div>}
            {runs.map((r) => (
              <RunRow
                key={r.id}
                run={r}
                ws={ws}
                pid={pidByRunId.get(r.id) ?? null}
                taskLabel={r.task ? (externalById.get(r.task) ?? r.task) : null}
                variantLabel={r.variant ? (variantLabelById.get(r.variant) ?? r.variant) : null}
                onOpen={() => r.log_path && navigate(logPath(r.log_path))}
              />
            ))}
          </div>
        </section>
      </main>
    </div>
  );
}

/** Full-page streamed view of a single run's log. The header breadcrumb
 *  (Agents / Logs / <name>) provides navigation back to the runs list. */
export function LogPage() {
  const { log } = useParams<{ log: string }>();
  const name = log ? decodeURIComponent(log) : "";
  return (
    <div className="flex-1 flex flex-col overflow-hidden">
      <LogViewer name={name} />
    </div>
  );
}

function RunRow({
  run,
  ws,
  pid,
  taskLabel,
  variantLabel,
  onOpen,
}: {
  run: AgentRun;
  ws: string;
  /** Live process id when the run is still running, else null (not killable). */
  pid: number | null;
  taskLabel: string | null;
  variantLabel: string | null;
  onOpen: () => void;
}) {
  const ts = run.finished_at ?? run.started_at ?? run.created_at;
  const args = run.data?.args ?? {};
  const fallback = !run.task ? (args.branch ?? args.spec ?? null) : null;
  const disabled = !run.log_path;
  const fb = useFeedback();
  const [busy, setBusy] = useState(false);

  async function handleKill() {
    if (pid == null) return;
    setBusy(true);
    fb.clear();
    try {
      await client.agent.kill(ws, pid);
      // Snapshot stream flips the run within ~2s; success flash bridges the gap.
      fb.ok("killed", { clearAfter: 2000 });
    } catch (e: unknown) {
      fb.err(errorMessage(e), { clearAfter: 5000 });
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="flex items-center gap-2 px-3 py-2 hover:bg-surface/50">
      <button
        onClick={onOpen}
        disabled={disabled}
        className="flex items-center gap-2 text-left flex-1 min-w-0 disabled:cursor-not-allowed disabled:opacity-60"
      >
        <RunStatusIcon status={run.status} />
        <Pill>{run.agent_name}</Pill>
        {taskLabel && (
          <Pill>
            <LayoutGrid size={12} />
            {taskLabel}
          </Pill>
        )}
        {variantLabel && <Pill>{variantLabel}</Pill>}
        {run.loop_total > 1 && (
          <Pill>
            i{run.loop_iter}/{run.loop_total}
          </Pill>
        )}
        {fallback && <span className="text-[11px] text-fg-subtle">{fallback}</span>}
      </button>
      <FeedbackText feedback={fb.feedback} />
      <TimeAgo iso={ts} className="text-[11px] text-fg-subtle whitespace-nowrap" />
      {pid != null && (
        <DropdownMenu
          panelClassName="w-40"
          trigger={({ open, toggle }) => (
            <button
              type="button"
              onClick={toggle}
              title="More actions"
              className={
                "shrink-0 p-1 rounded text-fg-subtle hover:text-fg hover:bg-surface-2 transition-colors " +
                (open ? "bg-surface-2 text-fg" : "")
              }
            >
              <MoreHorizontal size={16} />
            </button>
          )}
        >
          {({ close }) => (
            <ul className="space-y-0.5">
              <li>
                <button
                  type="button"
                  onClick={() => {
                    close();
                    handleKill();
                  }}
                  disabled={busy}
                  className="w-full text-left px-2 py-1.5 rounded text-xs text-red-400 hover:bg-surface disabled:opacity-50 disabled:cursor-not-allowed"
                >
                  {busy ? "killing…" : "Kill"}
                </button>
              </li>
            </ul>
          )}
        </DropdownMenu>
      )}
    </div>
  );
}
