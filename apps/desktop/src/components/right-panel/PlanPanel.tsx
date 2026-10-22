import { CheckCircle2, Circle, CircleDot, MinusCircle } from "lucide-react";
import { useEffect } from "react";
import { useRightPanel } from "../../contexts/RightPanelProvider";
import { useSnapshot } from "../../contexts/SnapshotContext";
import { toneDotClasses } from "../../lib/pipeline";
import { planFinished } from "../../lib/plan";
import { usePlan } from "../../queries/plan";
import type { PipelineTone, PlanPhase, PlanTaskStatus } from "../../types";
import { CollapsibleSection } from "../CollapsibleSection";
import { MarkdownContent } from "../MarkdownContent";
import { PlanProgressBar } from "../PlanProgressBar";
import { Spinner } from "../Spinner";

const TASK_ICON: Record<PlanTaskStatus, typeof Circle> = {
  pending: Circle,
  in_progress: CircleDot,
  done: CheckCircle2,
  skipped: MinusCircle,
};

const PHASE_STATUS: Record<"done" | "current" | "pending", { label: string; tone: PipelineTone }> =
  {
    done: { label: "done", tone: "green" },
    current: { label: "in progress", tone: "purple" },
    pending: { label: "pending", tone: "zinc" },
  };

function isFinished(status: PlanTaskStatus) {
  return status === "done" || status === "skipped";
}

export function PlanPanel({
  workspace,
  task,
  variant,
}: {
  workspace: string;
  task: string;
  variant: string;
}) {
  const { data: plan, isError } = usePlan(workspace, task, variant);
  const { snap } = useSnapshot();
  const { setTitle } = useRightPanel();
  const v = snap?.tasks.find((t) => t.id === task)?.variants.find((x) => x.id === variant);
  const progress = v?.plan_progress;
  const position = v?.position;

  useEffect(() => {
    if (position === undefined) return;
    setTitle(
      <>
        <span className="text-fg-subtle">Plan</span> · Variant {position}
      </>,
    );
  }, [position, setTitle]);

  if (isError) {
    return <div className="p-6 text-fg-subtle text-xs">No plan for this variant.</div>;
  }
  if (!plan) {
    return (
      <div className="flex-1 flex items-center justify-center p-6">
        <Spinner size={24} />
      </div>
    );
  }

  const prose: [string, string][] = [
    ["Problem", plan.problem],
    ["Approach", plan.approach],
    [
      "Projects",
      plan.projects
        .map(
          (p) =>
            `**${p.name}**\n${p.changes.map((c) => `- \`${c.path}\`: ${c.change}`).join("\n")}`,
        )
        .join("\n\n"),
    ],
    ["Risks", plan.risks],
    ["Test plan", plan.test_plan],
    [
      "Summaries",
      (plan.summaries ?? [])
        .map(
          (s) =>
            `**${s.kind === "done" ? "Done" : `Rework round ${s.round ?? ""}`}**\n\n${s.content}`,
        )
        .join("\n\n"),
    ],
  ];

  return (
    <div className="text-xs">
      {progress && (
        <div className="flex items-center gap-2 px-5 pt-5">
          <PlanProgressBar progress={progress} status={v?.status ?? null} />
          <span className="tabular-nums text-fg-muted">
            {planFinished(progress)}/{progress.tasks_total} tasks
          </span>
        </div>
      )}
      {plan.phases.map((phase) => (
        <PhaseSection
          key={phase.id}
          phase={phase}
          currentPhaseId={progress?.current_phase_id}
          currentTaskId={progress?.current_task_id}
        />
      ))}
      <div className="border-t border-border mt-5">
        {prose
          .filter(([, value]) => value.trim())
          .map(([label, value]) => (
            <CollapsibleSection key={label} label={label} defaultOpen={false}>
              <MarkdownContent value={value} />
            </CollapsibleSection>
          ))}
      </div>
    </div>
  );
}

function PhaseSection({
  phase,
  currentPhaseId,
  currentTaskId,
}: {
  phase: PlanPhase;
  currentPhaseId?: string | null;
  currentTaskId?: string | null;
}) {
  const finished = phase.tasks.filter((t) => isFinished(t.status)).length;
  const state =
    finished === phase.tasks.length ? "done" : phase.id === currentPhaseId ? "current" : "pending";
  const details: [string, string | undefined][] = [
    ["Scope", phase.scope],
    ["Approach", phase.approach],
    ["Notes", phase.notes],
    ["Deviations", phase.deviations],
  ];
  const filled = details.filter(([, value]) => value?.trim());

  return (
    <section className="mt-5">
      <div className="flex items-center gap-2 px-5">
        <span className="font-inter font-semibold text-[13px] text-fg truncate">
          {phase.id}. {phase.title}
        </span>
        <span className="flex items-center gap-1.5 text-fg-muted shrink-0">
          <span
            className={`inline-block w-2 h-2 rounded-full ${toneDotClasses[PHASE_STATUS[state].tone]}`}
          />
          {PHASE_STATUS[state].label}
        </span>
        <span className="ml-auto tabular-nums text-fg-subtle shrink-0">
          {finished}/{phase.tasks.length}
        </span>
      </div>
      <ul className="mt-2 px-3 space-y-0.5">
        {phase.tasks.map((t) => {
          const Icon = TASK_ICON[t.status];
          const current = t.id === currentTaskId;
          return (
            <li
              key={t.id}
              className={`flex items-start gap-2 px-2 py-1 rounded ${current ? "bg-surface" : ""}`}
            >
              <Icon size={14} className="shrink-0 mt-px text-fg-muted" />
              <span className="min-w-0">
                <span className={isFinished(t.status) ? "text-fg-muted" : "text-fg"}>
                  <span className="tabular-nums text-fg-subtle">{t.id}</span> {t.title}
                </span>
                {t.status === "skipped" && t.note && (
                  <span className="block text-fg-subtle">{t.note}</span>
                )}
              </span>
            </li>
          );
        })}
      </ul>
      {filled.length > 0 && (
        <CollapsibleSection label="Details" defaultOpen={false}>
          {filled.map(([label, value]) => (
            <div key={label}>
              <div className="text-[10px] uppercase tracking-wide text-fg-subtle mb-1">{label}</div>
              <MarkdownContent value={value ?? ""} />
            </div>
          ))}
        </CollapsibleSection>
      )}
    </section>
  );
}
