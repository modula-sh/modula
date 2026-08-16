import { createContext, useContext, useMemo, useState } from "react";
import { useLocalStorage } from "../lib/useLocalStorage";
import type { ConversationContext } from "../types";

// Published by the chat view so the right-sidebar can render at RootLayout level.
export type ChatSidebarConfig = {
  workspace: string;
  context: ConversationContext;
  refreshNonce: number;
} | null;

interface ChatSidebarValue {
  open: boolean;
  toggle: () => void;
  config: ChatSidebarConfig;
  setConfig: (config: ChatSidebarConfig) => void;
}

const Ctx = createContext<ChatSidebarValue | null>(null);

export function ChatSidebarProvider({ children }: { children: React.ReactNode }) {
  const [open, setOpen] = useLocalStorage("modula.chat.right-sidebar-open", true);
  const [config, setConfig] = useState<ChatSidebarConfig>(null);

  const value = useMemo<ChatSidebarValue>(
    () => ({ open, toggle: () => setOpen((v) => !v), config, setConfig }),
    [open, setOpen, config],
  );
  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

export function useChatSidebar() {
  const ctx = useContext(Ctx);
  if (!ctx) throw new Error("useChatSidebar must be used inside ChatSidebarProvider");
  return ctx;
}
