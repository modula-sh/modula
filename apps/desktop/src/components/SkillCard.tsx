// Toggleable agent-skill card. Locked (hidden) skills render active and non-clickable.
interface SkillCardProps {
  name: string;
  description: string;
  active: boolean;
  locked?: boolean;
  onToggle?: () => void;
}

export function SkillCard({ name, description, active, locked, onToggle }: SkillCardProps) {
  return (
    <button
      type="button"
      onClick={locked ? undefined : onToggle}
      disabled={locked}
      className={[
        "text-left rounded-xl border p-3 transition-colors",
        active ? "border-border-focus bg-surface-2" : "border-border bg-surface",
        locked ? "cursor-default" : "cursor-pointer hover:border-border-focus",
      ].join(" ")}
    >
      <div className="flex items-center justify-between gap-2">
        <span className={`text-sm font-medium text-fg ${active ? "" : "opacity-40"}`}>{name}</span>
        {locked && (
          <span className="text-[10px] uppercase tracking-wide text-fg-subtle">always on</span>
        )}
      </div>
      <div className={`mt-1 text-[11px] leading-snug text-fg-muted ${active ? "" : "opacity-40"}`}>
        {description}
      </div>
    </button>
  );
}
