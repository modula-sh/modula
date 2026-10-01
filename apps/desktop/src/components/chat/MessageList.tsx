import { type RefObject, useEffect, useMemo, useRef } from "react";
import type { ConversationMessage, MessagePart } from "../../types";
import { MessageBubble } from "./MessageBubble";
import { MessageParts } from "./MessageParts";

export function MessageList({
  messages,
  hasOlder,
  onReachTop,
  parts,
  streaming,
  error,
  scrollRef,
}: {
  messages: ConversationMessage[];
  /** Messages older than the first one shown exist. */
  hasOlder: boolean;
  onReachTop: () => void;
  /** The reply in flight. */
  parts: MessagePart[];
  streaming: boolean;
  error: string | null;
  scrollRef: RefObject<HTMLDivElement | null>;
}) {
  // Held apart from the in-flight tail so streaming doesn't rebuild the
  // history's elements — React skips those subtrees on an unchanged reference.
  const history = useMemo(
    () => messages.map((msg) => <MessageBubble key={msg.seq} msg={msg} />),
    [messages],
  );

  // Re-observed per page: an observer only reports a change, and the top may still be in reach.
  const topRef = useRef<HTMLDivElement>(null);
  const firstSeq = messages[0]?.seq;
  useEffect(() => {
    const top = topRef.current;
    if (!top || !hasOlder || firstSeq === undefined) return;
    const observer = new IntersectionObserver(
      ([entry]) => {
        if (entry.isIntersecting) onReachTop();
      },
      { root: scrollRef.current, rootMargin: "800px 0px 0px 0px" },
    );
    observer.observe(top);
    return () => observer.disconnect();
  }, [hasOlder, firstSeq, onReachTop, scrollRef]);

  const last = parts.at(-1);
  return (
    // Reversed column: the scroll origin is the bottom, so it opens on the latest message.
    <div
      ref={scrollRef}
      className="flex-1 flex flex-col-reverse overflow-y-auto px-[50px] [scrollbar-gutter:stable]"
    >
      <div className="w-full max-w-[800px] mx-auto py-4 pb-40 space-y-4">
        <div ref={topRef} />
        {history}

        {parts.length > 0 && (
          <div className="flex flex-col gap-1 min-w-0">
            <MessageParts parts={parts} live />
          </div>
        )}

        {streaming && last?.type === "text" && (
          <span className="inline-block w-1.5 h-3 bg-fg-subtle animate-pulse ml-0.5 align-bottom" />
        )}

        {streaming && (!last || (last.type === "tool" && last.status !== "running")) && (
          <p className="font-inter italic text-fg-subtle text-[14px] animate-pulse">Thinking…</p>
        )}

        {error && <p className="text-red-400 text-xs px-1">{error}</p>}
      </div>
    </div>
  );
}
