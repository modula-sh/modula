import { Activity, useContext, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { Outlet, useLocation } from "react-router-dom";
import { RightPanel, RightPanelCard } from "../components/right-panel/RightPanel";
import { SearchModal } from "../components/SearchModal";
import { AsideCardProvider, AsideCardTarget } from "../contexts/AsideCardContext";
import { ChatSidebarTarget, ChatSidebarTargetProvider } from "../contexts/ChatSidebarContext";
import { HeaderSlotProvider } from "../contexts/HeaderSlotContext";
import { ModalPortalProvider } from "../contexts/ModalPortalContext";
import { PipelineContext } from "../contexts/PipelineContext";
import { RightPanelProvider } from "../contexts/RightPanelProvider";
import { useShortcut } from "../contexts/ShortcutsContext";
import { useSidebarContext } from "../contexts/SidebarContext";
import { SnapshotProvider, useSnapshot } from "../contexts/SnapshotContext";
import { useThemeContext } from "../contexts/ThemeContext";
import { WorkspaceContext, WorkspaceSwitcherContext } from "../contexts/WorkspaceContext";
import { getPipeline } from "../lib/pipeline";
import { useSnapshotQuery } from "../queries/snapshot";
import { windowButtons } from "../tauri/window";
import { Header } from "./Header";
import { Sidebar } from "./Sidebar";
import { Titlebar } from "./Titlebar";

/** Root route of a workspace's router. `<Activity>` keeps a hidden workspace's
 * DOM (scroll included) and state but tears down its effects, which re-run when
 * it is shown again. It sits inside the router so a navigation fired while
 * hidden still lands. */
export function WorkspaceLayout({ workspace }: { workspace: string }) {
  const { active } = useContext(WorkspaceSwitcherContext);
  return (
    <Activity mode={workspace === active ? "visible" : "hidden"}>
      <WorkspaceProviders workspace={workspace} />
    </Activity>
  );
}

/** While the snapshot is loading we still render the Header and Sidebar so nav
 * is usable; the body shows a "connecting…" hint until the first fetch lands. */
function WorkspaceProviders({ workspace }: { workspace: string }) {
  const snap = useSnapshotQuery(workspace).data ?? null;
  const pipeline = useMemo(() => getPipeline(snap), [snap]);

  return (
    <WorkspaceContext.Provider value={workspace}>
      <PipelineContext.Provider value={pipeline}>
        <SnapshotProvider value={{ snap }}>
          <RightPanelProvider>
            <ChatSidebarTargetProvider>
              <WorkspaceLayoutBody workspace={workspace} />
            </ChatSidebarTargetProvider>
          </RightPanelProvider>
        </SnapshotProvider>
      </PipelineContext.Provider>
    </WorkspaceContext.Provider>
  );
}

/** Body that runs INSIDE the workspace providers, so it can consume their
 * context (snapshot, location) without wrapping the whole tree. */
function WorkspaceLayoutBody({ workspace }: { workspace: string }) {
  const { workspaces, setWorkspace, refreshWorkspaces } = useContext(WorkspaceSwitcherContext);
  const { snap } = useSnapshot();
  const { glass } = useThemeContext();
  const location = useLocation();
  const { notifyRegionWidth } = useSidebarContext();
  const navRef = useRef<HTMLElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);
  const [searchOpen, setSearchOpen] = useState(false);

  useShortcut("mod+k", () => setSearchOpen(true));

  // Windows acrylic reads more solid than macOS vibrancy, so it needs less tint.
  let plate = "bg-chrome";
  if (glass) plate = windowButtons() === "system" ? "bg-chrome/80" : "bg-chrome/70";

  // Scroll to top on route change so deep-scrolled pages don't carry over.
  useEffect(() => {
    window.scrollTo(0, 0);
  }, [location.pathname]);

  // Hiding the workspace blurs whatever it had focused; put focus back on return.
  const lastFocused = useRef<HTMLElement | null>(null);
  useLayoutEffect(() => {
    if (lastFocused.current?.isConnected) lastFocused.current.focus({ preventScroll: true });
  }, []);

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
    <ModalPortalProvider className={`h-screen flex flex-col relative ${plate}`}>
      <Titlebar />
      <AsideCardProvider>
        <div
          className="flex-1 flex min-h-0 pr-2 pb-2"
          onFocus={(e) => {
            if (!navRef.current?.contains(e.target)) lastFocused.current = e.target;
          }}
        >
          <Sidebar
            ref={navRef}
            workspace={workspace}
            workspaces={workspaces}
            onSwitchWorkspace={setWorkspace}
            onRefreshWorkspaces={refreshWorkspaces}
            onOpenSearch={() => setSearchOpen(true)}
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
            <ChatSidebarTarget />
            <RightPanel />
          </div>
          <AsideCardTarget />
          <RightPanelCard />
          {searchOpen && <SearchModal onClose={() => setSearchOpen(false)} />}
        </div>
      </AsideCardProvider>
    </ModalPortalProvider>
  );
}
