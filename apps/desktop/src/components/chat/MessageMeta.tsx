import { Check, Copy, GitFork } from "lucide-react";
import { useContext, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { useToast } from "../../contexts/ToastContext";
import { WorkspaceContext } from "../../contexts/WorkspaceContext";
import { formatDuration } from "../../lib/toolDisplay";
import { chatDraftKey } from "../../queries/conversation";
import { client, errorMessage } from "../../services/client";
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
  const [cloning, setCloning] = useState(false);
  const ws = useContext(WorkspaceContext);
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const toast = useToast();

  // A new chat with the transcript up to here; a user message becomes its unsent draft.
  async function clone() {
    if (!id || cloning) return;
    setCloning(true);
    try {
      const own = msg.role === "user";
      const through = own ? msg.seq - 1 : msg.seq;
      const fork = await client.conversation.fork(ws, id, through < 0 ? undefined : through);
      if (own) localStorage.setItem(chatDraftKey(ws, fork.id), JSON.stringify(msg.content));
      navigate(`/conversations/${fork.id}`);
    } catch (e) {
      toast.error(errorMessage(e));
    } finally {
      setCloning(false);
    }
  }

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
        <button
          type="button"
          onClick={clone}
          disabled={cloning}
          title="Clone chat from here"
          className="p-1 rounded hover:bg-surface hover:text-fg transition-colors disabled:opacity-50"
        >
          <GitFork size={12} />
        </button>
        <TimeAgo iso={msg.ts} />
      </div>
    </div>
  );
}
