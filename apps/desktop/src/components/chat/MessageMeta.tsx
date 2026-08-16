import { Check, Copy } from "lucide-react";
import { useState } from "react";
import type { ConversationMessage } from "../../types";
import { TimeAgo } from "../TimeAgo";

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
  return (
    <div
      className={`flex items-center gap-2 text-xs font-inter text-fg-subtle opacity-0 group-hover:opacity-100 ${justify}`}
    >
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
  );
}
