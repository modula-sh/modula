import type { RefObject } from "react";
import type { InFlightTool } from "../../contexts/ConversationStreamProvider";
import type { ConversationMessage } from "../../types";
import { MarkdownContent } from "../MarkdownContent";
import { MessageBubble } from "./MessageBubble";
import { ToolUseLine } from "./ToolUseLine";

export function MessageList({
  messages,
  inFlightText,
  inFlightTools,
  streaming,
  error,
  bottomRef,
  scrollContainerRef,
  onScroll,
}: {
  messages: ConversationMessage[];
  inFlightText: string;
  inFlightTools: InFlightTool[];
  streaming: boolean;
  error: string | null;
  bottomRef: RefObject<HTMLDivElement | null>;
  scrollContainerRef: RefObject<HTMLDivElement | null>;
  onScroll: (e: React.UIEvent<HTMLDivElement>) => void;
}) {
  return (
    <div
      ref={scrollContainerRef}
      className="flex-1 overflow-y-auto px-[50px] [scrollbar-gutter:stable]"
      onScroll={onScroll}
    >
      <div className="max-w-[800px] mx-auto py-4 pb-40 space-y-4">
        {messages.map((msg, i) => (
          <MessageBubble key={i} msg={msg} />
        ))}

        {(inFlightTools.length > 0 || inFlightText) && (
          <div className="flex flex-col gap-1">
            {inFlightTools.map((tool, i) => (
              <ToolUseLine key={i} name={tool.name} input={tool.input} />
            ))}
            {inFlightText && (
              <div>
                <MarkdownContent
                  value={inFlightText}
                  fontClass="font-geist"
                  bodyClass="text-[14px]"
                />
                <span className="inline-block w-1.5 h-3 bg-fg-subtle animate-pulse ml-0.5 align-bottom" />
              </div>
            )}
          </div>
        )}

        {streaming && !inFlightText && inFlightTools.length === 0 && (
          <p className="font-inter italic text-fg-subtle text-[14px] animate-pulse">Thinking…</p>
        )}

        {error && <p className="text-red-400 text-xs px-1">{error}</p>}
        <div ref={bottomRef} />
      </div>
    </div>
  );
}
