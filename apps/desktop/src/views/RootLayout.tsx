import { useEffect, useMemo, useRef, useState } from "react";
import { Outlet, useLocation } from "react-router-dom";
import { ChatRightSidebar } from "../components/chat/ChatRightSidebar";
import { RightPanel } from "../components/right-panel/RightPanel";
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
  const regionRef = useRef<HTMLDivElement>(null);

  // Scroll to top on route change so deep-scrolled pages don't carry over.
  useEffect(() => {
    window.scrollTo(0, 0);
  }, [location.pathname]);

  // Auto-collapse the nav off the sidebar+content region. It wraps the nav and
  // content but not the right drawers, so as a flex-1 sibling of those drawers
  // its width is exactly window − drawers — and independent of the nav's own
  // width, so collapsing never feeds back into the measurement.
  const ready = !!snap;
  useEffect(() => {
    const region = regionRef.current;
    if (!region) return;
    const measure = () => notifyRegionWidth(region.offsetWidth);
    const ro = new ResizeObserver(measure);
    ro.observe(region);
    measure();
    return () => ro.disconnect();
  }, [notifyRegionWidth, ready]);

  if (!snap) {
    return (
      <div className="h-screen flex">
        <Sidebar
          workspace={workspace}
          workspaces={workspaces}
          onSwitchWorkspace={setWorkspace}
          onRefreshWorkspaces={refreshWorkspaces}
        />
        <div className="flex-1 flex flex-col min-w-0">
          <Header />
          <div className="flex-1 flex items-center justify-center text-fg-subtle">connecting…</div>
        </div>
      </div>
    );
  }

  return (
    <ModalPortalProvider className="h-screen flex relative">
      <div ref={regionRef} className="flex-1 flex min-w-0">
        <Sidebar
          workspace={workspace}
          workspaces={workspaces}
          onSwitchWorkspace={setWorkspace}
          onRefreshWorkspaces={refreshWorkspaces}
        />
        <HeaderSlotProvider>
          <div className="flex-1 flex flex-col min-w-0">
            <Header />
            <div className="flex-1 flex flex-col min-h-0 relative">
              <Outlet />
            </div>
          </div>
        </HeaderSlotProvider>
      </div>
      <ChatRightSidebarHost />
      <RightPanel />
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
