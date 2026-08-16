import { useQueryClient } from "@tanstack/react-query";
import { PanelRightClose, PanelRightOpen } from "lucide-react";
import { useCallback, useContext, useEffect, useLayoutEffect, useRef, useState } from "react";
import { useLocation, useNavigate, useParams } from "react-router-dom";
import { Button } from "../components/Button";
import { ChatInput } from "../components/chat/ChatInput";
import { ChatInputShell } from "../components/chat/ChatInputShell";
import { ContextPills } from "../components/chat/ContextPills";
import { MessageList } from "../components/chat/MessageList";
import { SendButton } from "../components/chat/SendButton";
import { DropdownSelect } from "../components/DropdownMenu";
import { HeaderSlot } from "../components/HeaderSlot";
import { IconButton } from "../components/IconButton";
import { useChatSidebar } from "../contexts/ChatSidebarContext";
import { useConversationStream } from "../contexts/ConversationStreamProvider";
import { useSnapshot } from "../contexts/SnapshotContext";
import { WorkspaceContext } from "../contexts/WorkspaceContext";
import { ProviderTypeIcon } from "../lib/providerTypes";
import { useLocalStorage } from "../lib/useLocalStorage";
import { useProviderCatalog } from "../queries/catalog";
import { conversationKeys, useConversation } from "../queries/conversation";
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
  useEffect(() => {
    setModel(null);
  }, [providerId]);

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
                    ...variantOptions.map((v) => ({ value: v.id, label: `Variant ${v.position}` })),
                  ]}
                  disabled={busy}
                />
              )}
              <div className="ml-auto">
                <SendButton
                  onClick={handleSubmit}
                  disabled={!message.trim() || !providerId || busy}
                />
              </div>
            </>
          }
        />
        {error && <p className="text-red-400 text-xs">{error}</p>}
      </div>
    </main>
  );
}

// ─── Thread (/conversations/:id) ───────────────────────────────────────────

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
  const {
    open: sidebarOpen,
    toggle: toggleSidebar,
    setConfig: setChatSidebarConfig,
  } = useChatSidebar();
  const bottomRef = useRef<HTMLDivElement>(null);
  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const userScrolledRef = useRef(false);
  const initialFiredRef = useRef(false);
  const initialPositionedRef = useRef(false);
  const lastLength = useRef(0);
  const scrollKey = `modula.chat.scroll.${ws}.${id ?? ""}`;
  const saveScroll = useCallback(() => {
    const el = scrollContainerRef.current;
    if (!el || !initialPositionedRef.current) return;
    try {
      localStorage.setItem(scrollKey, String(el.scrollTop));
    } catch {}
  }, [scrollKey]);
  const initialMsg: string | undefined = (location.state as { initialMessage?: string } | null)
    ?.initialMessage;

  // Sync server-loaded messages into local state (which also holds optimistic
  // sends); on first load adopt the conversation's persisted model.
  useEffect(() => {
    setMessages(conv ? conv.messages : []);
    if (conv) setSelectedModel((prev) => (prev === null ? conv.model : prev));
  }, [conv]);

  const { inFlightText, inFlightTools, streaming, error, send, attach, cancel } =
    useConversationStream(ws, id ?? "");

  useEffect(() => {
    if (ws && id) attach();
  }, [attach, ws, id]);

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

  // Publish config for the layout-level right-sidebar; cleared on unmount.
  useEffect(() => {
    setChatSidebarConfig({
      workspace: ws,
      context: conv?.context ?? {},
      refreshNonce: sidebarNonce,
    });
  }, [
    ws,
    conv?.context?.project,
    conv?.context?.task,
    conv?.context?.variant,
    sidebarNonce,
    setChatSidebarConfig,
  ]);
  useEffect(() => () => setChatSidebarConfig(null), [setChatSidebarConfig]);

  const provider = conv
    ? (snap?.config?.providers.find((p) => p.id === conv.provider_id) ?? null)
    : null;
  const providerType = provider?.type ?? null;
  const availableModels = providerType
    ? (catalog.find((c) => c.id === providerType)?.models ?? [])
    : [];

  const handleSend = useCallback(
    (text: string) => {
      // Optimistic — show the user message immediately; the refetch on stream-end
      // will reconcile with the server's persisted copy.
      setMessages((prev) => [
        ...prev,
        { role: "user", content: text, ts: new Date().toISOString() },
      ]);
      send(text, selectedModel);
    },
    [send, selectedModel],
  );

  useEffect(() => {
    if (conv && initialMsg && !initialFiredRef.current) {
      initialFiredRef.current = true;
      handleSend(initialMsg);
    }
  }, [conv, initialMsg, handleSend]);

  useEffect(() => {
    initialPositionedRef.current = false;
    userScrolledRef.current = false;
    lastLength.current = 0;
  }, [ws, id]);

  // Position synchronously before paint to avoid a top-then-bottom scroll flash.
  useLayoutEffect(() => {
    if (initialPositionedRef.current || messages.length === 0) return;
    const el = scrollContainerRef.current;
    if (!el) return;
    const max = Math.max(0, el.scrollHeight - el.clientHeight);
    let target = max;
    try {
      const raw = localStorage.getItem(scrollKey);
      if (raw != null) {
        const parsed = parseInt(raw, 10);
        if (Number.isFinite(parsed) && parsed < max - 64) target = Math.min(parsed, max);
      }
    } catch {}
    el.scrollTop = target;
    initialPositionedRef.current = true;
    lastLength.current = messages.length;
    userScrolledRef.current = target < max - 64;
  }, [messages, scrollKey]);

  // Gated on initial position so it can't fight the layout effect on mount.
  useEffect(() => {
    if (!initialPositionedRef.current) return;
    if (messages.length !== lastLength.current || inFlightText || inFlightTools.length > 0) {
      lastLength.current = messages.length;
      if (!userScrolledRef.current) {
        bottomRef.current?.scrollIntoView({ behavior: "smooth" });
      }
    }
  }, [messages, inFlightText, inFlightTools]);

  const scrollSaveQueuedRef = useRef(false);
  const handleScroll = useCallback(
    (e: React.UIEvent<HTMLDivElement>) => {
      const el = e.currentTarget;
      const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 64;
      userScrolledRef.current = !atBottom;
      if (!initialPositionedRef.current || scrollSaveQueuedRef.current) return;
      scrollSaveQueuedRef.current = true;
      requestAnimationFrame(() => {
        scrollSaveQueuedRef.current = false;
        saveScroll();
      });
    },
    [saveScroll],
  );

  useEffect(() => () => saveScroll(), [saveScroll]);

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
          {sidebarOpen ? <PanelRightClose size={16} /> : <PanelRightOpen size={16} />}
        </IconButton>
      </HeaderSlot>

      <div className="flex-1 flex overflow-hidden">
        <div className="flex-1 flex flex-col overflow-hidden relative min-w-0">
          <MessageList
            messages={messages}
            inFlightText={inFlightText}
            inFlightTools={inFlightTools}
            streaming={streaming}
            error={error}
            bottomRef={bottomRef}
            scrollContainerRef={scrollContainerRef}
            onScroll={handleScroll}
          />

          <div
            aria-hidden
            className="pointer-events-none absolute left-0 right-[8px] bottom-0 h-[220px] bg-gradient-to-t from-bg via-bg to-transparent"
          />

          <div className="absolute bottom-4 left-0 right-[8px] px-[50px]">
            <div className="max-w-[1100px] mx-auto">
              <ChatInput
                onSend={handleSend}
                onCancel={cancel}
                streaming={streaming}
                models={availableModels}
                selectedModel={selectedModel}
                onModelChange={setSelectedModel}
                draftKey={`modula.draft.${ws}.conv.${id}`}
              />
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
