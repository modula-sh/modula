import { X } from "lucide-react";
import type { QueuedMessage } from "../../types";

// Messages waiting for the in-flight run to end, listed above the composer.
export function QueuedMessages({
  queued,
  onRemove,
}: {
  queued: QueuedMessage[];
  onRemove: (id: string) => void;
}) {
  if (queued.length === 0) return null;
  return (
    <div className="flex flex-col items-end gap-1 mb-2">
      {queued.map((q) => (
        <div
          key={q.id}
          className="flex items-center gap-2 max-w-full px-3 py-1.5 bg-chat-input border border-chat-input-border/50 rounded-2xl shadow-panel"
        >
          <span className="min-w-0 truncate text-[13px] text-fg-subtle font-geist">
            {q.content}
          </span>
          <button
            type="button"
            onClick={() => onRemove(q.id)}
            title="Remove from queue"
            className="shrink-0 text-fg-subtle hover:text-fg transition-colors"
          >
            <X size={14} />
          </button>
        </div>
      ))}
    </div>
  );
}
