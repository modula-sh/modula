import { toneDotClasses } from "../lib/pipeline";
import { planPercent } from "../lib/plan";
import { variantStatusTone } from "../lib/task";
import type { PlanProgress, VariantStatus } from "../types";

export function PlanProgressBar({
  progress,
  status,
  compact = false,
}: {
  progress: PlanProgress;
  status: VariantStatus | null;
  compact?: boolean;
}) {
  const percent = planPercent(progress);
  return (
    <span className="inline-flex items-center gap-1.5 shrink-0" title={`${percent}% of plan`}>
      <span
        className={`${compact ? "w-8 h-0.5" : "w-16 h-1"} rounded-full bg-surface-2 overflow-hidden`}
      >
        <span
          className={`block h-full ${toneDotClasses[variantStatusTone(status)]}`}
          style={{ width: `${percent}%` }}
        />
      </span>
      {!compact && <span className="tabular-nums text-fg-muted">{percent}%</span>}
    </span>
  );
}
