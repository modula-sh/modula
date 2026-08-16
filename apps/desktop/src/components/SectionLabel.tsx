/** Uppercase header for a form/card section (e.g. rules, skills, schedule),
 *  with an optional helper line rendered directly beneath it.
 *  Single source of truth for section-header styling across the forms. */
export function SectionLabel({
  children,
  description,
  className = "",
}: {
  children: React.ReactNode;
  description?: React.ReactNode;
  className?: string;
}) {
  return (
    <div className={className || undefined}>
      <div className="text-[10px] uppercase tracking-wide text-fg">{children}</div>
      {description && <div className="mt-1 text-[10px] text-fg-subtle">{description}</div>}
    </div>
  );
}
