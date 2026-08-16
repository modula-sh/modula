import { Channel, invoke } from "@tauri-apps/api/core";
import { useContext, useEffect, useRef, useState } from "react";
import { WorkspaceContext } from "../contexts/WorkspaceContext";
import { formatToolUse } from "../lib/logFormat";
import type { LogEntry } from "../types";

export function LogViewer({ name }: { name: string }) {
  const ws = useContext(WorkspaceContext);
  const [entries, setEntries] = useState<LogEntry[]>([]);
  const scrollRef = useRef<HTMLDivElement>(null);
  const stickToBottom = useRef(true);

  useEffect(() => {
    if (!ws) return;
    setEntries([]);
    // One newline-terminated line per chunk from the engine's log-tail stream.
    const channel = new Channel<string>();
    channel.onmessage = (chunk) => {
      const parsed = parseEvent(chunk.trimEnd());
      if (parsed.length === 0) return;
      setEntries((prev) => [...prev, ...parsed]);
    };
    invoke("log_stream", { workspaceId: ws, logName: name, onChunk: channel }).catch(() => {
      // Stream ended or the engine went away; nothing to surface here.
    });
    // Detaching the channel ends the tail on the engine without affecting the run.
    return () => {
      channel.onmessage = () => {};
    };
  }, [ws, name]);

  // Auto-scroll to the newest entry only while the user is parked at the
  // bottom. Scrolling up unsticks; scrolling back to the bottom re-sticks.
  useEffect(() => {
    if (!stickToBottom.current) return;
    const el = scrollRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [entries]);

  function onScroll() {
    const el = scrollRef.current;
    if (!el) return;
    const distFromBottom = el.scrollHeight - el.scrollTop - el.clientHeight;
    stickToBottom.current = distFromBottom < 40;
  }

  return (
    <div
      ref={scrollRef}
      onScroll={onScroll}
      className="selectable flex-1 min-h-0 overflow-y-auto px-4 py-3 space-y-1.5 leading-relaxed"
    >
      {entries.length === 0 && <div className="text-fg-subtle">waiting for events…</div>}
      {entries.map((e, i) => (
        <LogLine key={i} e={e} />
      ))}
    </div>
  );
}

function parseEvent(raw: string): LogEntry[] {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  let ev: any;
  try {
    ev = JSON.parse(raw);
  } catch {
    return [{ kind: "raw", text: raw }];
  }
  const out: LogEntry[] = [];
  if (ev.type === "system" && ev.subtype === "init") {
    out.push({
      kind: "init",
      text: `session ${ev.session_id ?? "?"} cwd=${ev.cwd ?? "?"}`,
    });
  } else if (ev.type === "assistant") {
    for (const c of ev.message?.content ?? []) {
      if (c.type === "text" && (c.text ?? "").trim()) {
        out.push({ kind: "text", text: c.text });
      } else if (c.type === "tool_use") {
        const { primary, continuation } = formatToolUse(c.name, c.input);
        out.push({
          kind: "tool",
          text: c.name,
          toolName: c.name,
          primary,
          continuation,
        });
      }
    }
  } else if (ev.type === "result") {
    const tokens = `${ev.usage?.input_tokens ?? 0}/${ev.usage?.output_tokens ?? 0}`;
    out.push({
      kind: "result",
      text: `DONE · ${ev.subtype ?? "?"} · ${ev.duration_ms ?? 0}ms · tokens ${tokens}`,
      detail: ev.result ?? undefined,
    });
  }
  return out;
}

function LogLine({ e }: { e: LogEntry }) {
  if (e.kind === "init") {
    return (
      <div className="text-blue-400">
        <span className="text-blue-600">── INIT </span>
        {e.text}
      </div>
    );
  }
  if (e.kind === "text") {
    return (
      <div className="text-fg">
        <span className="text-fg-subtle">▸ </span>
        <span className="whitespace-pre-wrap">{e.text}</span>
      </div>
    );
  }
  if (e.kind === "tool") {
    return (
      <div>
        <div className="text-fg-muted">
          <span className="text-fg-subtle">→ </span>
          <span className="font-mono">{e.primary ?? e.text}</span>
        </div>
        {(e.continuation ?? []).map((line, i) => (
          <div key={i} className="text-fg-subtle pl-4">
            <span className="text-fg-muted"> ⎿ </span>
            <span className="font-mono whitespace-pre-wrap">{line}</span>
          </div>
        ))}
      </div>
    );
  }
  if (e.kind === "result") {
    return (
      <div className="text-green-300 mt-2 border-t border-border pt-2">
        <span className="text-green-600">═══ </span>
        <span className="font-semibold">{e.text}</span>
        {e.detail && <pre className="whitespace-pre-wrap text-fg mt-1">{e.detail}</pre>}
      </div>
    );
  }
  return <div className="text-fg-subtle">{e.text}</div>;
}
