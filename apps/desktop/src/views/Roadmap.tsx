import { useMemo } from "react";
import { useNavigate } from "react-router-dom";
import { AgentIdenticon } from "../components/AgentIdenticon";
import { Pill } from "../components/Pill";
import { Spinner } from "../components/Spinner";
import { usePipeline } from "../contexts/PipelineContext";
import { useSnapshot } from "../contexts/SnapshotContext";
import { toneDotClasses } from "../lib/pipeline";
import { externalStatusTextClass, variantStatusTone } from "../lib/task";
import type { Agent, RoadmapItem, Task } from "../types";

export function RoadmapView() {
  const pipeline = usePipeline();
  const navigate = useNavigate();
  const { snap } = useSnapshot();
  const tasks = snap?.tasks ?? [];
  const roadmap = snap?.roadmap ?? [];
  const agents = snap?.agents ?? [];

  // Click a card → jump to the Tasks view with this task pre-opened.
  function onSelectTask(id: string) {
    navigate(`/tasks/${encodeURIComponent(id)}`);
  }
  const taskById = useMemo(() => {
    const m = new Map<string, Task>();
    tasks.forEach((t) => m.set(t.id, t));
    return m;
  }, [tasks]);

  const itemsByStatus = useMemo(() => {
    const m = new Map<string, RoadmapItem[]>();
    roadmap.forEach((r) => {
      const list = m.get(r.status) ?? [];
      list.push(r);
      m.set(r.status, list);
    });
    return m;
  }, [roadmap]);

  return (
    <main className="flex-1 overflow-x-auto overflow-y-hidden p-3">
      <div className="flex gap-3 h-full min-w-max">
        {pipeline.map((p) => {
          const items = itemsByStatus.get(p.key) ?? [];
          return (
            <section
              key={p.key}
              className="flex flex-col w-72 shrink-0 rounded-lg overflow-hidden border border-border/50"
            >
              <div className="px-3 py-2 flex items-center justify-between gap-2 shrink-0">
                <div className="font-inter text-[13px] text-fg flex items-center gap-2 min-w-0">
                  <span
                    className={`inline-block w-2.5 h-2.5 rounded-full shrink-0 ${toneDotClasses[p.tone]}`}
                  />
                  <span className="truncate">{p.label}</span>
                </div>
                <span className="text-xs text-fg-subtle">{items.length}</span>
              </div>
              <div className="flex-1 overflow-y-auto px-2 pb-2 flex flex-col gap-2">
                {items.map((r) => (
                  <RoadmapCard
                    key={r.task}
                    item={r}
                    task={taskById.get(r.task)}
                    taskById={taskById}
                    agents={agents}
                    onClick={() => onSelectTask(r.task)}
                    onSelectDep={onSelectTask}
                  />
                ))}
              </div>
            </section>
          );
        })}
      </div>
    </main>
  );
}

function RoadmapCard({
  item,
  task,
  taskById,
  agents,
  onClick,
  onSelectDep,
}: {
  item: RoadmapItem;
  task?: Task;
  taskById: Map<string, Task>;
  agents: Agent[];
  onClick: () => void;
  onSelectDep: (id: string) => void;
}) {
  const taskAgents = task
    ? agents.filter(
        (a) =>
          a.task === task.id ||
          (a.spec ?? "").includes(task.id) ||
          (a.branch ?? "").toLowerCase().includes(task.id.toLowerCase()),
      )
    : [];
  return (
    <button
      onClick={onClick}
      className="block w-full text-left px-3 py-2 bg-card hover:bg-surface rounded-md min-w-0 border border-border/60 shadow-card"
    >
      <div className="flex items-center gap-2 min-w-0 flex-wrap">
        {task?.external_id && (
          <span className="text-[11px] text-fg-subtle shrink-0">{task.external_id}</span>
        )}
        {task?.status && (
          <span className={`text-[11px] truncate ${externalStatusTextClass(task.status)}`}>
            {task.status}
          </span>
        )}
        {taskAgents.length > 0 && (
          <div className="ml-auto flex items-center gap-1 shrink-0">
            <Spinner size={12} />
            {taskAgents.map((a) => (
              <span
                key={a.pid}
                title={`${a.name} (pid ${a.pid})`}
                className="inline-flex items-center justify-center w-5 h-5 rounded-md bg-surface-2 text-fg border border-border shrink-0"
              >
                <AgentIdenticon id={a.name} size={16} />
              </span>
            ))}
          </div>
        )}
      </div>
      {task?.title && (
        <div className="font-inter text-fg text-[13px] mt-2 line-clamp-2">{task.title}</div>
      )}
      {task && task.variants.length > 0 && (
        <div className="mt-2 flex items-center gap-2 flex-wrap font-inter text-[12px] text-fg-muted">
          {task.variants.map((v) => (
            <span key={v.id} className="flex items-center gap-1.5">
              <span
                className={`inline-block w-2 h-2 rounded-full shrink-0 ${toneDotClasses[variantStatusTone(v.status)]}`}
              />
              v{v.position}
            </span>
          ))}
        </div>
      )}
      {item.depends_on.length > 0 && (
        <div className="mt-2 flex items-center gap-1 flex-wrap text-[10px]">
          {item.depends_on.map((d) => (
            <span
              key={d}
              role="link"
              tabIndex={0}
              title={d}
              className="cursor-pointer hover:opacity-80 transition-opacity"
              onClick={(e) => {
                e.stopPropagation();
                onSelectDep(d);
              }}
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  e.stopPropagation();
                  onSelectDep(d);
                }
              }}
            >
              <Pill>{taskById.get(d)?.external_id}</Pill>
            </span>
          ))}
        </div>
      )}
    </button>
  );
}
