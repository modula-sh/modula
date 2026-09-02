import { Channel, invoke } from "@tauri-apps/api/core";
import { createContext, useCallback, useContext, useMemo, useRef, useState } from "react";

export interface InFlightTool {
  name: string;
  input: unknown;
}

export interface ChatStreamState {
  inFlightText: string;
  inFlightTools: InFlightTool[];
  streaming: boolean;
  error: string | null;
}

const EMPTY: ChatStreamState = {
  inFlightText: "",
  inFlightTools: [],
  streaming: false,
  error: null,
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
  | { kind: "tooluse"; name: string; input: unknown }
  | { kind: "delta"; text: string }
  | { kind: "done" }
  | { kind: "error"; message: string }
  | { kind: "unknown" };

// App-level chat connection manager. Owns one gRPC stream per (workspace, conv)
// bridged through a Tauri `Channel`, and survives page navigation, so a turn
// that's in flight keeps running even when the user leaves the conversation
// view. Pages reattach with `attach()` on mount and pick up the in-flight buffer
// the engine replays. Dropping a channel detaches the client without cancelling
// the run; an explicit `cancel()` winds the run down.
export function ConversationStreamProvider({ children }: { children: React.ReactNode }) {
  const [states, setStates] = useState<Record<string, ChatStreamState>>({});
  // Refs keep per-conv async state out of React reconciliation: the active
  // stream channel per conv (used to dedupe attaches and detach on resend)
  // and per-conv accumulators for inFlight text + tools.
  const channels = useRef<Map<string, Channel<ConvEvent>>>(new Map());
  const textAcc = useRef<Map<string, string>>(new Map());
  const toolsAcc = useRef<Map<string, InFlightTool[]>>(new Map());

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
        textAcc.current.delete(convId);
        toolsAcc.current.delete(convId);
        channels.current.delete(convId);
        patch(convId, () => EMPTY);
      };
      const finishErr = (msg: string) => {
        textAcc.current.delete(convId);
        toolsAcc.current.delete(convId);
        channels.current.delete(convId);
        patch(convId, () => ({ ...EMPTY, error: msg }));
      };
      const channel = new Channel<ConvEvent>();
      channel.onmessage = (ev) => {
        if (ev.kind === "tooluse") {
          const tools = [
            ...(toolsAcc.current.get(convId) ?? []),
            { name: ev.name, input: ev.input },
          ];
          toolsAcc.current.set(convId, tools);
          patch(convId, (s) => ({ ...s, inFlightTools: tools, streaming: true }));
        } else if (ev.kind === "delta") {
          const acc = (textAcc.current.get(convId) ?? "") + ev.text;
          textAcc.current.set(convId, acc);
          patch(convId, (s) => ({ ...s, inFlightText: acc, streaming: true }));
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
      textAcc.current.set(convId, "");
      toolsAcc.current.set(convId, []);
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
    inFlightText: state.inFlightText,
    inFlightTools: state.inFlightTools,
    streaming: state.streaming,
    error: state.error,
    send: useCallback(
      (text: string, model?: string | null) => ctx.api.send(ws, convId, text, model),
      [ctx.api, ws, convId],
    ),
    attach: useCallback(() => ctx.api.attach(ws, convId), [ctx.api, ws, convId]),
    cancel: useCallback(() => ctx.api.cancel(ws, convId), [ctx.api, ws, convId]),
  };
}
