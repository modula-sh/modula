import { Check, Loader2, X } from "lucide-react";
import type { RunStatus } from "../types";

export function RunStatusIcon({ status, size = 14 }: { status: RunStatus; size?: number }) {
  if (status === "running") {
    return (
      <Loader2 size={size} className="animate-spin text-fg-muted shrink-0" aria-label="running" />
    );
  }
  if (status === "completed") {
    return <Check size={size} className="text-fg-muted shrink-0" aria-label="completed" />;
  }
  return <X size={size} className="text-red-400 shrink-0" aria-label="failed" />;
}
