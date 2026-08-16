import { Link } from "react-router-dom";
import { useSnapshot } from "../../contexts/SnapshotContext";
import type { ConversationContext } from "../../types";

const PILL =
  "inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full border border-border bg-surface-2 text-fg text-[11px] font-inter whitespace-nowrap";
const PILL_LINK = `${PILL} hover:border-border-focus transition-colors`;

export function ContextPills({ context }: { context: ConversationContext }) {
  const { snap } = useSnapshot();
  if (!context.project && !context.task && !context.variant) return null;
  const project = snap?.config?.projects?.find((p) => p.id === context.project);
  const task = snap?.tasks?.find((t) => t.id === context.task);
  const variant = task?.variants.find((v) => v.id === context.variant);
  const taskHref = context.task ? `/tasks/${context.task}` : null;
  const taskLabel = task
    ? task.external_id
      ? `${task.external_id}: ${task.title}`
      : task.title
    : context.task;
  return (
    <div className="flex flex-wrap gap-1.5">
      {context.project && <span className={PILL}>{project?.name ?? context.project}</span>}
      {context.task && taskHref && (
        <Link to={taskHref} className={`${PILL_LINK} max-w-[280px]`} title={taskLabel}>
          <span className="truncate">{taskLabel}</span>
        </Link>
      )}
      {context.variant && taskHref && (
        <Link to={taskHref} className={PILL_LINK}>
          {variant ? `Variant ${variant.position}` : context.variant}
        </Link>
      )}
    </div>
  );
}
