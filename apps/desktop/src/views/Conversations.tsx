import { useQueryClient } from "@tanstack/react-query";
import { PanelRightClose, PanelRightOpen } from "lucide-react";
import { useCallback, useContext, useEffect, useMemo, useRef, useState } from "react";
import { useLocation, useNavigate, useParams } from "react-router-dom";
import { AiAssist, AiAssistTrigger } from "../components/AiAssist";
import { Button } from "../components/Button";
import { ChatInput } from "../components/chat/ChatInput";
import { ChatInputShell } from "../components/chat/ChatInputShell";
import { ChatRightSidebar } from "../components/chat/ChatRightSidebar";
import { ContextPills } from "../components/chat/ContextPills";
import { MessageList } from "../components/chat/MessageList";
import { QueuedMessages } from "../components/chat/QueuedMessages";
import { SendButton } from "../components/chat/SendButton";
import { TERMINAL_MODE_KEY, TerminalView } from "../components/chat/TerminalView";
import { DropdownSelect } from "../components/DropdownMenu";
import { HeaderSlot } from "../components/HeaderSlot";
import { IconButton } from "../components/IconButton";
import { ChatSidebarPortal, useChatSidebar } from "../contexts/ChatSidebarContext";
import { useConversationStream } from "../contexts/ConversationStreamProvider";
import { useSnapshot } from "../contexts/SnapshotContext";
import { WorkspaceContext } from "../contexts/WorkspaceContext";
import { ProviderTypeIcon } from "../lib/providerTypes";
import { useLocalStorage } from "../lib/useLocalStorage";
import { useResetOnChange } from "../lib/useResetOnChange";
import { useProviderCatalog } from "../queries/catalog";
import {
  chatDraftKey,
  conversationKeys,
  loadOlderMessages,
  MESSAGE_PAGE,
  useConversation,
} from "../queries/conversation";
import { client, errorMessage } from "../services/client";
import type { ConversationContext, ConversationMessage, Task } from "../types";

// Mirror the engine's `derive_title`: first non-empty line, trimmed, capped at 120 chars.
function deriveTitle(msg: string): string {
  const line =
    msg
      .split("\n")
      .map((l) => l.trim())
      .find((l) => l.length > 0) ?? "";
  return [...line].slice(0, 120).join("");
}

// ─── Landing (/conversations) ──────────────────────────────────────────────

export function ConversationsView() {
  const ws = useContext(WorkspaceContext);
  const { snap } = useSnapshot();
  const navigate = useNavigate();
  const draftKey = `modula.draft.${ws}.new`;
  const [message, setMessage] = useLocalStorage(draftKey, "");
  const [providerId, setProviderId] = useState("");
  const [model, setModel] = useState<string | null>(null);
  const [scopeProject, setScopeProject] = useState("");
  const [scopeTask, setScopeTask] = useState("");
  const [scopeVariant, setScopeVariant] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const { data: catalog = [] } = useProviderCatalog();

  const providers = snap?.config?.providers ?? [];
  const projects = snap?.config?.projects ?? [];
  const tasks: Task[] = snap?.tasks ?? [];

  useEffect(() => {
    if (providers.length > 0 && !providerId) {
      setProviderId(providers[0].id);
    }
  }, [providers, providerId]);

  // Reset model when provider changes — the previous pick may not exist for the new type.
  useResetOnChange(providerId, () => setModel(null));

  const providerType = providers.find((p) => p.id === providerId)?.type ?? null;
  const availableModels = providerType
    ? (catalog.find((c) => c.id === providerType)?.models ?? [])
    : [];

  const selectedTask = tasks.find((t) => t.id === scopeTask);
  const variantOptions = selectedTask?.variants ?? [];

  function buildContext(): ConversationContext {
    const ctx: ConversationContext = {};
    if (scopeProject) ctx.project = scopeProject;
    if (scopeTask) ctx.task = scopeTask;
    if (scopeVariant) ctx.variant = scopeVariant;
    return ctx;
  }

  async function handleSubmit() {
    if (!message.trim() || !providerId || busy) return;
    setBusy(true);
    setError(null);
    try {
      const { id } = await client.conversation.create(ws, {
        provider_id: providerId,
        title: deriveTitle(message),
        model: model ?? undefined,
        context: buildContext(),
        // The engine falls back to chat for providers without a terminal launch.
        mode: localStorage.getItem(TERMINAL_MODE_KEY) === "true" ? "terminal" : "chat",
      });
      localStorage.removeItem(draftKey);
      navigate(`/conversations/${id}`, { state: { initialMessage: message.trim() } });
    } catch (e) {
      setError(errorMessage(e));
      setBusy(false);
    }
  }

  const currentContext = buildContext();

  return (
    <main className="flex-1 overflow-y-auto flex items-center justify-center px-4">
      <div className="w-full max-w-2xl flex flex-col gap-3">
        <h1 className="font-inter text-xl font-medium text-fg text-center mb-3">
          What should we work on?
        </h1>
        {(currentContext.project || currentContext.task || currentContext.variant) && (
          <ContextPills context={currentContext} />
        )}
        <AiAssist value={message} onChange={setMessage} fieldLabel="chat message">
          <ChatInputShell
            value={message}
            onChange={setMessage}
            onSubmit={handleSubmit}
            placeholder="Ask anything about this workspace…"
            autoFocus
            bottomRow={
              <>
                <DropdownSelect
                  value={providerId}
                  onChange={setProviderId}
                  options={providers.map((p) => ({ value: p.id, label: p.name }))}
                  disabled={busy}
                />
                <DropdownSelect
                  value={model ?? ""}
                  onChange={(v) => setModel(v === "" ? null : v)}
                  options={[
                    { value: "", label: "Default Model" },
                    ...availableModels.map((m) => ({ value: m.id, label: m.label })),
                  ]}
                  disabled={busy}
                  title="Model, applies to the first message"
                />
                <DropdownSelect
                  value={scopeProject}
                  onChange={setScopeProject}
                  placeholder="No project"
                  options={[
                    { value: "", label: "No project" },
                    ...projects.map((p) => ({ value: p.id, label: p.name })),
                  ]}
                  disabled={busy}
                />
                <DropdownSelect
                  value={scopeTask}
                  onChange={(v) => {
                    setScopeTask(v);
                    setScopeVariant("");
                  }}
                  placeholder="No task"
                  options={[
                    { value: "", label: "No task" },
                    ...tasks.map((t) => ({
                      value: t.id,
                      label: t.external_id
                        ? `${t.external_id}: ${t.title.slice(0, 28)}`
                        : t.title.slice(0, 40),
                    })),
                  ]}
                  disabled={busy}
                />
                {scopeTask && (
                  <DropdownSelect
                    value={scopeVariant}
                    onChange={setScopeVariant}
                    placeholder="No variant"
                    options={[
                      { value: "", label: "No variant" },
                      ...variantOptions.map((v) => ({
                        value: v.id,
                        label: `Variant ${v.position}`,
                      })),
                    ]}
                    disabled={busy}
                  />
                )}
                <AiAssistTrigger />
                <div className="ml-auto">
                  <SendButton
                    onClick={handleSubmit}
                    disabled={!message.trim() || !providerId || busy}
                  />
                </div>
              </>
            }
          />
        </AiAssist>
        {error && <p className="text-red-400 text-xs">{error}</p>}
      </div>
    </main>
  );
}

// ─── Thread (/conversations/:id) ───────────────────────────────────────────

const NO_CONTEXT: ConversationContext = {};

export function ConversationDetailPage() {
  const ws = useContext(WorkspaceContext);
  const { snap } = useSnapshot();
  const { id } = useParams<{ id: string }>();
  const location = useLocation();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const { data: conv, isError: notFound } = useConversation(ws, id);
  // Messages are pulled from the server. They stay correct across navigation
  // because the run keeps persisting even while the page is unmounted; the
  // query refetches on mount and is invalidated when a turn ends here.
  const [messages, setMessages] = useState<ConversationMessage[]>([]);
  const { data: catalog = [] } = useProviderCatalog();
  // null = "provider default"; the server persists this on the next send.
  // Mirrored to localStorage so a picked-but-unsent model survives navigation.
  const [selectedModel, setSelectedModel] = useLocalStorage<string | null>(
    `modula.chat.model.${ws}.${id ?? ""}`,
    null,
  );
  const { open: sidebarOpen, toggle: toggleSidebar } = useChatSidebar();
  const scrollRef = useRef<HTMLDivElement>(null);
  const initialFiredRef = useRef(false);
  const initialMsg: string | undefined = (location.state as { initialMessage?: string } | null)
    ?.initialMessage;

  // Sync server-loaded messages into local state (which also holds optimistic
  // sends); on first load adopt the conversation's persisted model.
  useResetOnChange(conv, () => {
    setMessages(conv ? conv.messages : []);
    if (conv) setSelectedModel((prev) => (prev === null ? conv.model : prev));
  });

  const { parts, streaming, error, injected, send, attach, cancel } = useConversationStream(
    ws,
    id ?? "",
  );

  // The first message rendered; scrolling to the top moves it back a page.
  const [fromSeq, setFromSeq] = useState<number | null>(null);
  useResetOnChange(id, () => setFromSeq(null));
  if (fromSeq === null && messages.length > 0) {
    setFromSeq(messages[Math.max(0, messages.length - MESSAGE_PAGE)].seq);
  }
  const shown = useMemo(() => messages.filter((m) => m.seq >= (fromSeq ?? 0)), [messages, fromSeq]);
  const loadingOlder = useRef(false);
  const showOlder = useCallback(async () => {
    if (fromSeq === null || !id || loadingOlder.current) return;
    const next = Math.max(0, fromSeq - MESSAGE_PAGE);
    loadingOlder.current = true;
    try {
      if ((messages[0]?.seq ?? 0) > next) await loadOlderMessages(queryClient, ws, id);
      setFromSeq(next);
    } finally {
      loadingOlder.current = false;
    }
  }, [fromSeq, messages, queryClient, ws, id]);

  // Unset until the conversation loads; terminal mode has no chat stream.
  const mode = conv?.mode;
  useEffect(() => {
    if (ws && id && mode === "chat") attach();
  }, [attach, ws, id, mode]);

  // Invalidate whenever a turn ends here so the just-persisted assistant message
  // lands, and bump the sidebar nonce so its diff/commit panels refetch too.
  const [sidebarNonce, setSidebarNonce] = useState(0);
  const prevStreamingRef = useRef(false);
  useEffect(() => {
    if (prevStreamingRef.current && !streaming) {
      if (ws && id) {
        queryClient.invalidateQueries({ queryKey: conversationKeys.detail(ws, id) });
      }
      setSidebarNonce((n) => n + 1);
    }
    prevStreamingRef.current = streaming;
  }, [streaming, ws, id, queryClient]);

  useEffect(() => {
    if (injected && ws && id) {
      queryClient.invalidateQueries({ queryKey: conversationKeys.detail(ws, id) });
    }
  }, [injected, ws, id, queryClient]);

  const provider = conv
    ? (snap?.config?.providers.find((p) => p.id === conv.provider_id) ?? null)
    : null;
  const providerType = provider?.type ?? null;
  const availableModels = providerType
    ? (catalog.find((c) => c.id === providerType)?.models ?? [])
    : [];

  // The local stream only turns on once a delta lands, so a run this window
  // joined mid-tool-use needs the engine's own view to route to the queue.
  const inFlight = streaming || !!conv?.running;

  const handleSend = useCallback(
    (text: string) => {
      // Optimistic — show the user message immediately; the refetch on stream-end
      // will reconcile with the server's persisted copy.
      setMessages((prev) => [
        ...prev,
        {
          role: "user",
          content: text,
          ts: new Date().toISOString(),
          seq: (prev.at(-1)?.seq ?? -1) + 1,
        },
      ]);
      scrollRef.current?.scrollTo({ top: 0 });
      send(text, selectedModel);
    },
    [send, selectedModel],
  );

  // The queue lives engine-side; refetch so a change made here or on the phone
  // lands the same way.
  const refetchQueue = useCallback(() => {
    if (ws && id) queryClient.invalidateQueries({ queryKey: conversationKeys.detail(ws, id) });
  }, [queryClient, ws, id]);

  // ChatInput clears the box before the RPC resolves, so a failure has to be shown
  // or the message is gone with no signal.
  const [queueError, setQueueError] = useState<string | null>(null);
  // Stream errors are held per conversation id; this one is not, so drop it on navigation.
  useResetOnChange(id, () => setQueueError(null));
  const settleQueue = useCallback(
    (p: Promise<unknown>) => {
      p.then(() => setQueueError(null))
        .catch((e) => setQueueError(errorMessage(e)))
        .finally(refetchQueue);
    },
    [refetchQueue],
  );

  const handleQueue = useCallback(
    (text: string) => {
      if (!id) return;
      settleQueue(client.conversation.enqueue(ws, id, text));
    },
    [ws, id, settleQueue],
  );

  const handleDequeue = useCallback(
    (queuedId: string) => {
      if (!id) return;
      settleQueue(client.conversation.dequeue(ws, id, queuedId));
    },
    [ws, id, settleQueue],
  );

  useEffect(() => {
    if (conv?.mode === "chat" && initialMsg && !initialFiredRef.current) {
      initialFiredRef.current = true;
      handleSend(initialMsg);
    }
  }, [conv, initialMsg, handleSend]);

  if (notFound) {
    return (
      <main className="flex-1 flex flex-col items-center justify-center gap-3 text-fg-subtle">
        <p className="text-sm">Conversation not found.</p>
        <Button onClick={() => navigate("/conversations")}>← New conversation</Button>
      </main>
    );
  }

  const ctx = conv?.context;
  const hasCtx = !!(ctx?.project || ctx?.task || ctx?.variant);
  const rawTitle = conv ? conv.title || "Untitled conversation" : "";
  const title = rawTitle.length > 50 ? `${rawTitle.slice(0, 50)}…` : rawTitle;

  return (
    <div className="flex-1 flex flex-col overflow-hidden">
      <HeaderSlot>
        <span
          className="flex-1 min-w-0 text-fg text-xs font-inter font-medium truncate"
          title={rawTitle}
        >
          {title}
        </span>
        {hasCtx && <ContextPills context={ctx!} />}
        {conv && (
          <span className="inline-flex items-center gap-1.5 text-fg-subtle text-xs">
            <ProviderTypeIcon type={providerType} size="xs" />
            <span>{provider?.name ?? conv.provider_id}</span>
          </span>
        )}
        <IconButton
          onClick={toggleSidebar}
          aria-expanded={sidebarOpen}
          title={sidebarOpen ? "Hide sidebar" : "Show sidebar"}
        >
          {sidebarOpen ? (
            <PanelRightClose size={16} className="icon-crisp" />
          ) : (
            <PanelRightOpen size={16} className="icon-crisp" />
          )}
        </IconButton>
      </HeaderSlot>
      <ChatSidebarPortal>
        <ChatRightSidebar
          workspace={ws}
          context={conv?.context ?? NO_CONTEXT}
          refreshNonce={sidebarNonce}
        />
      </ChatSidebarPortal>

      <div className="flex-1 flex overflow-hidden">
        {conv?.mode === "terminal" ? (
          <TerminalView ws={ws} conversationId={conv.id} initialPrompt={initialMsg} />
        ) : (
          <div className="flex-1 flex flex-col overflow-hidden relative min-w-0">
            <MessageList
              messages={shown}
              hasOlder={(fromSeq ?? 0) > 0}
              onReachTop={showOlder}
              parts={parts}
              streaming={streaming}
              error={error ?? queueError}
              scrollRef={scrollRef}
            />

            <div
              aria-hidden
              className="pointer-events-none absolute left-0 right-[8px] bottom-0 h-[220px] bg-gradient-to-t from-bg via-bg to-transparent"
            />

            <div className="absolute bottom-4 left-0 right-[8px] px-[50px]">
              <div className="max-w-[800px] mx-auto">
                <QueuedMessages queued={conv?.queued ?? []} onRemove={handleDequeue} />
                <ChatInput
                  onSend={handleSend}
                  onQueue={handleQueue}
                  onCancel={cancel}
                  streaming={inFlight}
                  models={availableModels}
                  selectedModel={selectedModel}
                  onModelChange={setSelectedModel}
                  draftKey={chatDraftKey(ws, id ?? "")}
                />
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
