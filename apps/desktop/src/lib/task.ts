import type { PipelineTone, VariantStatus } from "../types";

export function variantStatusTone(s: VariantStatus | string | null | undefined): PipelineTone {
  switch (s) {
    case "ready_for_workers":
      return "blue";
    case "in_progress":
      return "purple";
    case "ready_for_review":
      return "blue";
    case "in_review":
      return "yellow";
    case "rework":
      return "red";
    case "accepted":
      return "green";
    default:
      return "zinc";
  }
}

/** Human-readable label for a task source (used in "open in X ↗" links and
 * field labels). Falls back to 'EXTERNAL' for legacy rows that have no
 * `source`, since we can't know which integration they came from. */
export function sourceLabel(source: string | null | undefined): string {
  const s = (source ?? "").trim();
  if (!s) return "EXTERNAL";
  return s.toUpperCase();
}

/** Map a (free-form) external-tracker status string to a Tailwind text-color
 * class. Works for any source — JIRA, Linear, GitHub Issues, etc. — since
 * the strings ("open", "in progress", "blocked", "closed", …) are conventional
 * across trackers. Rendered as inline lighter text next to the task id. */
export function externalStatusTextClass(s: string): string {
  const v = s.trim().toLowerCase();
  if (v === "to do" || v === "todo" || v === "open" || v === "backlog" || v === "new")
    return "text-fg-muted";
  if (v === "in progress" || v === "in-progress" || v === "doing") return "text-blue-400";
  if (v === "review" || v === "in review" || v === "qa" || v === "testing" || v === "code review")
    return "text-yellow-400";
  if (v === "done" || v === "closed" || v === "resolved" || v === "complete" || v === "completed")
    return "text-green-400";
  if (
    v === "blocked" ||
    v === "on hold" ||
    v === "cancelled" ||
    v === "canceled" ||
    v === "rejected"
  )
    return "text-red-400";
  return "text-fg-muted";
}
