import { Channel, invoke } from "@tauri-apps/api/core";
import { createContext, useCallback, useContext, useMemo, useRef, useState } from "react";
import type { MessagePart } from "../types";

export interface ChatStreamState {
  /** The reply so far, folded from the stream as the engine folds it to persist. */
  parts: MessagePart[];
  streaming: boolean;
  error: string | null;
  // Bumped when a queued message enters the run, which persists the reply so far.
  injected: number;
}

const EMPTY: ChatStreamState = {
  parts: [],
  streaming: false,
  error: null,
  injected: 0,
};

interface StreamApi {
  send(ws: string, convId: string, text: string, model?: string | null): Promise<void>;
  attach(ws: string, convId: string): Promise<void>;
  cancel(ws: string, convId: string): Promise<void>;
}

interface CtxValue {
  states: Record<string, ChatStreamState>;
  api: StreamApi;
}

const Ctx = createContext<CtxValue | null>(null);

// Typed events forwarded by the Tauri `conversation_send`/`conversation_attach`
// commands — the gRPC `ConvEvent` stream mapped to a tagged JSON shape.
type ConvEvent =
  | { kind: "session"; id: string }
  | { kind: "tooluse"; id: string; name: string; input: unknown; parent_id?: string }
  | { kind: "toolresult"; id: string; output: string; is_error: boolean; output_truncated: boolean }
  | { kind: "delta"; text: string }
  | { kind: "thinking"; text: string }
  | { kind: "user"; text: string }
  | { kind: "done" }
  | { kind: "error"; message: string }
  | { kind: "unknown" };

/** Text continues the trailing part of its kind; anything else starts a new one. */
function foldText(parts: MessagePart[], type: "text" | "thinking", text: string): MessagePart[] {
  const last = parts.at(-1);
  if (last?.type !== type) return [...parts, { type, text }];
  return [...parts.slice(0, -1), { type, text: last.text + text }];
}

/** A `tooluse` adds a running call; a `toolresult` completes the call it names. */
function foldTool(
  parts: MessagePart[],
  ev: Extract<ConvEvent, { kind: "tooluse" | "toolresult" }>,
): MessagePart[] {
  if (ev.kind === "tooluse") {
    const { kind: _, ...call } = ev;
    return [...parts, { type: "tool", ...call, status: "running" }];
  }
  return parts.map((p) =>
    p.type === "tool" && p.id === ev.id
      ? {
          ...p,
          status: ev.is_error ? "failed" : "completed",
          output: ev.output,
          output_truncated: ev.output_truncated,
        }
      : p,
  );
}

// App-level chat connection manager. Owns one gRPC stream per (workspace, conv)
// bridged through a Tauri `Channel`, and survives page navigation, so a turn
// that's in flight keeps running even when the user leaves the conversation
// view. Pages reattach with `attach()` on mount and pick up the in-flight buffer
// the engine replays. Dropping a channel detaches the client without cancelling
// the run; an explicit `cancel()` winds the run down.
export function ConversationStreamProvider({ children }: { children: React.ReactNode }) {
  const [states, setStates] = useState<Record<string, ChatStreamState>>({});
  // The active stream channel per conv, kept out of React reconciliation:
  // used to dedupe attaches and to detach on resend.
  const channels = useRef<Map<string, Channel<ConvEvent>>>(new Map());

  const patch = useCallback((convId: string, patcher: (s: ChatStreamState) => ChatStreamState) => {
    setStates((prev) => ({ ...prev, [convId]: patcher(prev[convId] ?? EMPTY) }));
  }, []);

  // Detach the active stream for a conv without cancelling the underlying run:
  // silencing the channel and dropping our reference lets the backend forward
  // fail, which detaches the client. Used before a resend and on teardown.
  const detach = useCallback((convId: string) => {
    const ch = channels.current.get(convId);
    if (ch) {
      ch.onmessage = () => {};
      channels.current.delete(convId);
    }
  }, []);

  // Build a channel whose handler folds ConvEvents into per-conv state. Shared
  // by send (immediate spinner) and attach (spinner only once an event lands).
  const openChannel = useCallback(
    (convId: string) => {
      const finishOk = () => {
        channels.current.delete(convId);
        patch(convId, () => EMPTY);
      };
      const finishErr = (msg: string) => {
        channels.current.delete(convId);
        patch(convId, () => ({ ...EMPTY, error: msg }));
      };
      const channel = new Channel<ConvEvent>();
      channel.onmessage = (ev) => {
        if (ev.kind === "tooluse" || ev.kind === "toolresult") {
          patch(convId, (s) => ({ ...s, parts: foldTool(s.parts, ev), streaming: true }));
        } else if (ev.kind === "delta" || ev.kind === "thinking") {
          const type = ev.kind === "delta" ? "text" : "thinking";
          patch(convId, (s) => ({
            ...s,
            parts: foldText(s.parts, type, ev.text),
            streaming: true,
          }));
        } else if (ev.kind === "user") {
          // The reply so far is persisted; calls still running carry into the next one.
          patch(convId, (s) => ({
            ...s,
            parts: s.parts.filter((p) => p.type === "tool" && p.status === "running"),
            streaming: true,
            injected: s.injected + 1,
          }));
        } else if (ev.kind === "done") {
          finishOk();
        } else if (ev.kind === "error") {
          finishErr(ev.message);
        }
      };
      channels.current.set(convId, channel);
      return channel;
    },
    [patch],
  );

  const send = useCallback<StreamApi["send"]>(
    async (ws, convId, text, model) => {
      detach(convId);
      patch(convId, () => ({ ...EMPTY, streaming: true }));
      const onEvent = openChannel(convId);
      try {
        await invoke("conversation_send", {
          workspaceId: ws,
          conversationId: convId,
          message: text,
          model: model ?? null,
          onEvent,
        });
      } catch (e) {
        patch(convId, () => ({ ...EMPTY, error: (e as Error).message ?? String(e) }));
      }
    },
    [detach, openChannel, patch],
  );

  const attach = useCallback<StreamApi["attach"]>(
    async (ws, convId) => {
      // Already streaming for this conv — the existing channel already covers us.
      if (channels.current.has(convId)) return;
      // Streaming flips on when the first real event arrives — see openChannel.
      // Setting it true here would flash the spinner on every navigation even when
      // there's no in-flight run.
      const onEvent = openChannel(convId);
      try {
        await invoke("conversation_attach", { workspaceId: ws, conversationId: convId, onEvent });
      } catch (e) {
        patch(convId, () => ({ ...EMPTY, error: (e as Error).message ?? String(e) }));
      }
    },
    [openChannel, patch],
  );

  const cancel = useCallback<StreamApi["cancel"]>(async (ws, convId) => {
    try {
      await invoke("conversation_cancel", { workspaceId: ws, conversationId: convId });
    } catch {
      // best-effort — engine will eventually wind the run down
    }
  }, []);

  const api = useMemo<StreamApi>(() => ({ send, attach, cancel }), [send, attach, cancel]);
  const value = useMemo<CtxValue>(() => ({ states, api }), [states, api]);

  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

/** The imperative half of the context, for callers that drive streams from
 * outside a conversation view — chiefly the engine event stream, which attaches
 * when a run starts somewhere else. */
export function useStreamApi(): StreamApi | null {
  return useContext(Ctx)?.api ?? null;
}

export function useStreamingConvIds(): string[] {
  const ctx = useContext(Ctx);
  if (!ctx) return [];
  return Object.entries(ctx.states)
    .filter(([, s]) => s.streaming)
    .map(([id]) => id);
}

export function useConversationStream(ws: string, convId: string) {
  const ctx = useContext(Ctx);
  if (!ctx) {
    throw new Error("useConversationStream must be used inside ConversationStreamProvider");
  }
  const state = ctx.states[convId] ?? EMPTY;
  return {
    parts: state.parts,
    streaming: state.streaming,
    error: state.error,
    injected: state.injected,
    send: useCallback(
      (text: string, model?: string | null) => ctx.api.send(ws, convId, text, model),
      [ctx.api, ws, convId],
    ),
    attach: useCallback(() => ctx.api.attach(ws, convId), [ctx.api, ws, convId]),
    cancel: useCallback(() => ctx.api.cancel(ws, convId), [ctx.api, ws, convId]),
  };
}
