import { createContext, useContext, useMemo } from "react";
import { createPortal } from "react-dom";
import { useKeptElement } from "../lib/useKeptElement";
import { useLocalStorage } from "../lib/useLocalStorage";

interface ChatSidebarValue {
  open: boolean;
  toggle: () => void;
}

const Ctx = createContext<ChatSidebarValue | null>(null);

/** App-wide open/closed preference for the chat right-sidebar. */
export function ChatSidebarProvider({ children }: { children: React.ReactNode }) {
  const [open, setOpen] = useLocalStorage("modula.chat.right-sidebar-open", true);

  const value = useMemo<ChatSidebarValue>(
    () => ({ open, toggle: () => setOpen((v) => !v) }),
    [open, setOpen],
  );
  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

export function useChatSidebar() {
  const ctx = useContext(Ctx);
  if (!ctx) throw new Error("useChatSidebar must be used inside ChatSidebarProvider");
  return ctx;
}

const TargetCtx = createContext<{
  el: HTMLElement | null;
  setEl: (el: HTMLElement | null) => void;
}>({ el: null, setEl: () => {} });

/** Per-workspace slot the chat view portals its right-sidebar into. */
export function ChatSidebarTargetProvider({ children }: { children: React.ReactNode }) {
  const [el, setEl] = useKeptElement<HTMLElement>();
  return <TargetCtx.Provider value={{ el, setEl }}>{children}</TargetCtx.Provider>;
}

/** Layout-level sibling of the content column, like RightPanel. */
export function ChatSidebarTarget() {
  return <div ref={useContext(TargetCtx).setEl} className="contents" />;
}

/** Portaled from the chat view, so the sidebar lives exactly as long as the view. */
export function ChatSidebarPortal({ children }: { children: React.ReactNode }) {
  const { el } = useContext(TargetCtx);
  const { open } = useChatSidebar();
  if (!el || !open) return null;
  return createPortal(children, el);
}
