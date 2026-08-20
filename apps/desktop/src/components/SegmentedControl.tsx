/** Shared track for a pair of `Button tone="tab"` options so they read as one control. */
export function SegmentedControl({ children }: { children: React.ReactNode }) {
  return (
    <div className="inline-flex items-center gap-0.5 p-0.5 bg-surface rounded-md">{children}</div>
  );
}
