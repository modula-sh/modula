import { Check, Copy } from "lucide-react";
import { useState } from "react";
import { formatDuration } from "../../lib/toolDisplay";
import type { ConversationMessage, TurnStats } from "../../types";
import { TimeAgo } from "../TimeAgo";

/** The rest of what the provider reported for the turn, for the tooltip. */
function statsDetail(stats: TurnStats): string {
  const tokens = (n: number) => n.toLocaleString();
  return [
    stats.model,
    stats.input_tokens !== undefined && `${tokens(stats.input_tokens)} tokens in`,
    stats.output_tokens !== undefined && `${tokens(stats.output_tokens)} out`,
    stats.cost_usd !== undefined && `$${stats.cost_usd.toFixed(2)}`,
  ]
    .filter(Boolean)
    .join(" · ");
}

export function MessageMeta({ msg, align }: { msg: ConversationMessage; align: "start" | "end" }) {
  const [copied, setCopied] = useState(false);

  async function copy() {
    try {
      await navigator.clipboard.writeText(msg.content);
      setCopied(true);
      setTimeout(() => setCopied(false), 1200);
    } catch {
      /* clipboard blocked — ignore */
    }
  }

  const justify = align === "end" ? "justify-end" : "justify-start";
  const duration = msg.stats?.duration_ms;
  return (
    <div className={`flex items-center gap-2 text-xs font-inter text-fg-subtle ${justify}`}>
      {msg.stats && duration !== undefined && (
        <span title={statsDetail(msg.stats)}>Worked for {formatDuration(duration)}</span>
      )}
      <div className="flex items-center gap-2 opacity-0 group-hover:opacity-100">
        <button
          type="button"
          onClick={copy}
          title={copied ? "Copied" : "Copy"}
          className="p-1 rounded hover:bg-surface hover:text-fg transition-colors"
        >
          {copied ? <Check size={12} /> : <Copy size={12} />}
        </button>
        <TimeAgo iso={msg.ts} />
      </div>
    </div>
  );
}
