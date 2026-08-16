/** Label + description + input row used inside form sections.
 *  - Default: label/description on the left (2/3), input right-justified (1/3).
 *  - `inputCol="1/2"`: 1/2 + 1/2 split (wider input column).
 *  - `fullWidth`: label/description stacked above a full-width input
 *    (textareas, prompts, long-form inputs).
 *  Each row carries its own vertical padding + a light bottom divider so the
 *  enclosing section doesn't need `space-y-*`. */
export function FieldRow({
  label,
  labelAccessory,
  description,
  fullWidth = false,
  inputCol = "1/3",
  children,
}: {
  label: string;
  /** Optional element rendered inline beside the label text (e.g. a live
   *  preview of the value being edited, like an identicon). */
  labelAccessory?: React.ReactNode;
  description?: string;
  fullWidth?: boolean;
  inputCol?: "1/3" | "1/2";
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
  const gridCols = inputCol === "1/2" ? "grid-cols-2" : "grid-cols-3";
  const labelSpan = inputCol === "1/2" ? "" : "col-span-2";
  return (
    <div className="py-3 border-b border-border/60 last:border-b-0">
      {fullWidth ? (
        <div className="space-y-2">
          {labelBlock}
          {children}
        </div>
      ) : (
        <div className={`grid ${gridCols} items-center gap-4`}>
          <div className={labelSpan}>{labelBlock}</div>
          <div className="flex justify-end">{children}</div>
        </div>
      )}
    </div>
  );
}
