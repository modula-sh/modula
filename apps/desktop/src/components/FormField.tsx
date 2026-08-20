/** Stacked label + description + control — the flat row style for the edit forms (Settings uses FieldRow). */
export function FormField({
  label,
  labelAccessory,
  description,
  headerAccessory,
  horizontal = false,
  children,
}: {
  label: string;
  /** Rendered beside the label, e.g. a live identicon of the value. */
  labelAccessory?: React.ReactNode;
  description?: string;
  /** Right-aligned control in the header row, e.g. a Builder/Raw toggle. */
  headerAccessory?: React.ReactNode;
  /** Control beside the label instead of beneath it; for switches. */
  horizontal?: boolean;
  children: React.ReactNode;
}) {
  const labelBlock = (
    <div className="flex items-center gap-3">
      {labelAccessory}
      <div className="space-y-1">
        <div className="text-fg font-inter text-xs">{label}</div>
        {description && (
          <div className="text-fg-subtle text-[11px] leading-snug">{description}</div>
        )}
      </div>
    </div>
  );
  if (horizontal) {
    return (
      <div className="py-4 first:pt-0 last:pb-0 flex items-center justify-between gap-4">
        {labelBlock}
        <div className="shrink-0">{children}</div>
      </div>
    );
  }
  return (
    <div className="py-4 first:pt-0 last:pb-0 space-y-2">
      {headerAccessory ? (
        <div className="flex items-start justify-between gap-4">
          <div className="min-w-0 flex-1">{labelBlock}</div>
          <div className="shrink-0">{headerAccessory}</div>
        </div>
      ) : (
        labelBlock
      )}
      {children}
    </div>
  );
}
