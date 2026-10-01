import { memo, useState } from "react";
import { oneLine } from "../../lib/logFormat";
import type { MessagePart, ToolCall } from "../../types";
import { MarkdownContent } from "../MarkdownContent";
import { ToolCallView } from "./ToolCallView";

/** Reasoning the provider chose to show, folded to a line until clicked. */
const Thinking = memo(function Thinking({ text }: { text: string }) {
  const [open, setOpen] = useState(false);
  const trimmed = text.trim();
  if (!trimmed) return null;
  return (
    <button
      type="button"
      onClick={() => setOpen((v) => !v)}
      className="text-left font-inter italic text-[13px] text-fg-subtle hover:text-fg-muted whitespace-pre-wrap"
    >
      {open ? trimmed : oneLine(trimmed, 160)}
    </button>
  );
});

/** A reply's text, thinking and tool calls in order; a sub-agent's calls sit under its spawner. */
export function MessageParts({ parts, live }: { parts: MessagePart[]; live?: boolean }) {
  const ids = new Set(parts.flatMap((p) => (p.type === "tool" ? [p.id] : [])));
  const nested = new Map<string, ToolCall[]>();
  for (const p of parts) {
    if (p.type !== "tool" || !p.parent_id || !ids.has(p.parent_id)) continue;
    nested.set(p.parent_id, [...(nested.get(p.parent_id) ?? []), p]);
  }

  return parts.map((part, i) => {
    if (part.type === "tool") {
      if (part.parent_id && ids.has(part.parent_id)) return null;
      return <ToolCallView key={part.id} tool={part} nested={nested.get(part.id)} live={live} />;
    }
    if (part.type === "thinking") return <Thinking key={i} text={part.text} />;
    return (
      <MarkdownContent
        key={i}
        value={part.text}
        fontClass="font-geist"
        bodyClass="text-[14px]"
        className="selectable"
      />
    );
  });
}
