import { useEffect, useMemo, useRef, useState } from "react";
import { Outlet, useLocation } from "react-router-dom";
import { ChatRightSidebar } from "../components/chat/ChatRightSidebar";
import { RightPanel } from "../components/right-panel/RightPanel";
import { AsideCardProvider, AsideCardTarget } from "../contexts/AsideCardContext";
import { ChatSidebarProvider, useChatSidebar } from "../contexts/ChatSidebarContext";
import { ConversationStreamProvider } from "../contexts/ConversationStreamProvider";
import { HeaderSlotProvider } from "../contexts/HeaderSlotContext";
import { ModalPortalProvider } from "../contexts/ModalPortalContext";
import { PipelineContext } from "../contexts/PipelineContext";
import { RightPanelProvider } from "../contexts/RightPanelProvider";
import { SidebarProvider, useSidebarContext } from "../contexts/SidebarContext";
import { SnapshotProvider, useSnapshot } from "../contexts/SnapshotContext";
import { ThemeProvider } from "../contexts/ThemeContext";
import { ToastProvider } from "../contexts/ToastContext";
import { WorkspaceContext } from "../contexts/WorkspaceContext";
import { useEngineEvents } from "../hooks/useEngineEvents";
import { useSuppressContextMenu } from "../hooks/useSuppressContextMenu";
import { useTitlebarDrag } from "../hooks/useTitlebarDrag";
import { useWorkspaceState } from "../hooks/useWorkspaceState";
import { getPipeline } from "../lib/pipeline";
import { useLocalStorage } from "../lib/useLocalStorage";
import { useSnapshotQuery } from "../queries/snapshot";
import { Header } from "./Header";
import { Onboarding } from "./Onboarding";
import { Sidebar } from "./Sidebar";
import { Titlebar } from "./Titlebar";

/** Top-level layout route. Owns the long-lived state (workspace + SSE stream)
 * and exposes it to descendants via context. Always renders the Header and
 * Sidebar; the outlet renders whichever child route matched.
 *
 * While the snapshot is loading we still render the Header and Sidebar so nav
 * is usable; the body shows a "connecting…" hint until the first SSE message
 * arrives. */
export function RootLayout() {
  // Native window drag + double-click-to-maximize for the overlay titlebar.
  // Mounted here, above both the onboarding and app trees, so every screen is
  // draggable without each view repeating the logic.
  useTitlebarDrag();

  // Suppress the webview's default context menu app-wide.
  useSuppressContextMenu();

  const [onboarded, setOnboarded] = useLocalStorage<boolean>("modula.onboarded", false);
  const wsState = useWorkspaceState();

  // Decide once on load; Onboarding then owns the flow until onComplete.
  const [mode, setMode] = useState<"loading" | "onboarding" | "app">("loading");
  useEffect(() => {
    if (mode !== "loading" || !wsState.loaded) return;
    const needsOnboarding = !onboarded || wsState.workspaces.length === 0;
    setMode(needsOnboarding ? "onboarding" : "app");
  }, [mode, wsState.loaded, wsState.workspaces.length, onboarded]);

  return (
    <ThemeProvider>
      {mode === "loading" ? null : mode === "onboarding" ? (
        <Onboarding
          onComplete={() => {
            setOnboarded(true);
            setMode("app");
          }}
          wsState={wsState}
        />
      ) : (
        <AppRoot wsState={wsState} />
      )}
    </ThemeProvider>
  );
}

function AppRoot({ wsState }: { wsState: ReturnType<typeof useWorkspaceState> }) {
  const { workspace, workspaces, setWorkspace, refreshWorkspaces } = wsState;
  const snap = useSnapshotQuery(workspace).data ?? null;
  useEngineEvents(workspace);
  const pipeline = useMemo(() => getPipeline(snap), [snap]);

  return (
    <ToastProvider>
      <SidebarProvider>
        <WorkspaceContext.Provider value={workspace}>
          <PipelineContext.Provider value={pipeline}>
            <SnapshotProvider value={{ snap }}>
              <ConversationStreamProvider>
                <RightPanelProvider>
                  <ChatSidebarProvider>
                    <RootLayoutBody
                      workspace={workspace}
                      workspaces={workspaces}
                      setWorkspace={setWorkspace}
                      refreshWorkspaces={refreshWorkspaces}
                    />
                  </ChatSidebarProvider>
                </RightPanelProvider>
              </ConversationStreamProvider>
            </SnapshotProvider>
          </PipelineContext.Provider>
        </WorkspaceContext.Provider>
      </SidebarProvider>
    </ToastProvider>
  );
}

/** Body that runs INSIDE all the providers, so it can consume their context
 * (snapshot, location) without wrapping the whole tree. */
function RootLayoutBody({
  workspace,
  workspaces,
  setWorkspace,
  refreshWorkspaces,
}: {
  workspace: string;
  workspaces: ReturnType<typeof useWorkspaceState>["workspaces"];
  setWorkspace: (ws: string) => void;
  refreshWorkspaces: () => void;
}) {
  const { snap } = useSnapshot();
  const location = useLocation();
  const { notifyRegionWidth } = useSidebarContext();
  const navRef = useRef<HTMLElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);

  // Scroll to top on route change so deep-scrolled pages don't carry over.
  useEffect(() => {
    window.scrollTo(0, 0);
  }, [location.pathname]);

  // Nav + content = window minus the drawers and aside card. Summing the two
  // keeps it constant as the nav animates, so collapsing never feeds back in.
  useEffect(() => {
    const nav = navRef.current;
    const content = contentRef.current;
    if (!nav || !content) return;
    const measure = () => notifyRegionWidth(nav.offsetWidth + content.offsetWidth);
    const ro = new ResizeObserver(measure);
    ro.observe(nav);
    ro.observe(content);
    measure();
    return () => ro.disconnect();
  }, [notifyRegionWidth]);

  return (
    // Base plate: title bar and sidebar sit flat on it, the content card above it.
    <ModalPortalProvider className="h-screen flex flex-col relative bg-chrome">
      <Titlebar />
      <AsideCardProvider>
        <div className="flex-1 flex min-h-0 pr-2 pb-2">
          <Sidebar
            ref={navRef}
            workspace={workspace}
            workspaces={workspaces}
            onSwitchWorkspace={setWorkspace}
            onRefreshWorkspaces={refreshWorkspaces}
          />
          <div className="flex-1 flex min-w-0 rounded-xl border border-edge bg-bg shadow-content overflow-hidden">
            <HeaderSlotProvider>
              <div ref={contentRef} className="flex-1 flex flex-col min-w-0">
                <Header />
                <div className="flex-1 flex flex-col min-h-0 relative">
                  {snap ? (
                    <Outlet />
                  ) : (
                    <div className="flex-1 flex items-center justify-center text-fg-subtle">
                      connecting…
                    </div>
                  )}
                </div>
              </div>
            </HeaderSlotProvider>
            <ChatRightSidebarHost />
            <RightPanel />
          </div>
          <AsideCardTarget />
        </div>
      </AsideCardProvider>
    </ModalPortalProvider>
  );
}

/** Chat right-sidebar as a layout-level sibling of the content column, like RightPanel. */
function ChatRightSidebarHost() {
  const { open, config } = useChatSidebar();
  if (!open || !config) return null;
  return (
    <ChatRightSidebar
      workspace={config.workspace}
      context={config.context}
      refreshNonce={config.refreshNonce}
    />
  );
}
