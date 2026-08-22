import type { ConversationMessage } from "../../types";
import { MarkdownContent } from "../MarkdownContent";
import { MessageMeta } from "./MessageMeta";
import { ToolUseLine } from "./ToolUseLine";

export function MessageBubble({ msg }: { msg: ConversationMessage }) {
  if (msg.role === "user") {
    // Outer right-aligns; inner is sized to content so hover only covers
    // the bubble + meta (not the empty space to the left).
    return (
      <div className="flex justify-end">
        <div className="group flex flex-col items-end gap-1 min-w-0 max-w-[70%]">
          <div className="bg-surface rounded-2xl px-3.5 py-2 min-w-0">
            <p className="selectable text-[14px] text-fg font-geist whitespace-pre-wrap [overflow-wrap:anywhere]">
              {msg.content}
            </p>
          </div>
          <MessageMeta msg={msg} align="end" />
        </div>
      </div>
    );
  }
  return (
    <div className="group flex flex-col gap-1 min-w-0">
      {msg.tools?.map((tool, i) => (
        <ToolUseLine key={i} name={tool.name} input={tool.input} />
      ))}
      {msg.content && (
        <MarkdownContent
          value={msg.content}
          fontClass="font-geist"
          bodyClass="text-[14px]"
          className="selectable"
        />
      )}
      <MessageMeta msg={msg} align="start" />
    </div>
  );
}
