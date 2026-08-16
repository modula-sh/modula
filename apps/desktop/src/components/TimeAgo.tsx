import { DateTime } from "luxon";

export function TimeAgo({
  iso,
  className,
}: {
  iso: string | null | undefined;
  className?: string;
}) {
  const cls = `tabular-nums ${className ?? ""}`;
  if (!iso) return <span className={cls}>-</span>;
  const dt = DateTime.fromISO(iso);
  if (!dt.isValid) {
    return (
      <span className={cls} title={iso}>
        {iso}
      </span>
    );
  }
  const now = DateTime.now();
  return (
    <span className={cls} title={dt.toFormat("LLL d, yyyy h:mm a ZZZZ")}>
      {formatRelative(dt, now)}
    </span>
  );
}

function plural(n: number, unit: string): string {
  return `${n} ${unit}${n === 1 ? "" : "s"}`;
}

function formatRelative(dt: DateTime, now: DateTime): string {
  const diffMs = now.diff(dt).milliseconds;
  const future = diffMs < 0;
  const absDays = Math.abs(diffMs) / 86_400_000;
  if (absDays >= 30) {
    return dt.year === now.year ? dt.toFormat("LLL d") : dt.toFormat("LLL d, yyyy");
  }
  const wrap = (s: string) => (future ? `in ${s}` : `${s} ago`);
  const seconds = Math.abs(diffMs) / 1000;
  if (seconds < 60) return future ? "in moments" : "just now";
  if (seconds < 3600) return wrap(plural(Math.floor(seconds / 60), "minute"));
  if (seconds < 86_400) return wrap(plural(Math.floor(seconds / 3600), "hour"));
  if (absDays < 7) return wrap(plural(Math.floor(absDays), "day"));
  return wrap(plural(Math.floor(absDays / 7), "week"));
}
