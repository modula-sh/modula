/** Label left, input right-justified — the card-bound row style (Settings, onboarding, modals). */
export function FieldRow({
  label,
  description,
  inputCol = "1/3",
  children,
}: {
  label: string;
  description?: string;
  inputCol?: "1/3" | "1/2";
  children: React.ReactNode;
}) {
  const gridCols = inputCol === "1/2" ? "grid-cols-2" : "grid-cols-3";
  const labelSpan = inputCol === "1/2" ? "" : "col-span-2";
  return (
    <div className="py-3 border-b border-border/60 last:border-b-0">
      <div className={`grid ${gridCols} items-center gap-4`}>
        <div className={labelSpan}>
          <div className="space-y-1">
            <div className="text-fg font-inter text-xs">{label}</div>
            {description && (
              <div className="text-fg-subtle text-[11px] leading-snug">{description}</div>
            )}
          </div>
        </div>
        <div className="flex justify-end">{children}</div>
      </div>
    </div>
  );
}
