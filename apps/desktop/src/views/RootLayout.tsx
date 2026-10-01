import { Fragment, useEffect, useState } from "react";
import { RouterProvider } from "react-router-dom";
import { ChatSidebarProvider } from "../contexts/ChatSidebarContext";
import { ConversationStreamProvider } from "../contexts/ConversationStreamProvider";
import { ShortcutsProvider } from "../contexts/ShortcutsContext";
import { SidebarProvider } from "../contexts/SidebarContext";
import { ThemeProvider } from "../contexts/ThemeContext";
import { ToastProvider } from "../contexts/ToastContext";
import { WorkspaceSwitcherContext } from "../contexts/WorkspaceContext";
import { useEngineEvents } from "../hooks/useEngineEvents";
import { useSuppressContextMenu } from "../hooks/useSuppressContextMenu";
import { useTitlebarDrag } from "../hooks/useTitlebarDrag";
import { useWorkspaceState } from "../hooks/useWorkspaceState";
import { useLocalStorage } from "../lib/useLocalStorage";
import { workspaceRouter } from "../router";
import { Onboarding } from "./Onboarding";

/** App shell: the workspace selection plus the state every workspace shares. */
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

/** The engine watch, mounted inside `ConversationStreamProvider` because it
 * attaches a conversation stream when a run starts somewhere else. */
function EngineEvents({ workspace }: { workspace: string }) {
  useEngineEvents(workspace);
  return null;
}

function AppRoot({ wsState }: { wsState: ReturnType<typeof useWorkspaceState> }) {
  const { workspace, workspaces, setWorkspace, refreshWorkspaces } = wsState;

  return (
    <ToastProvider>
      <ShortcutsProvider>
        <SidebarProvider>
          <ChatSidebarProvider>
            <ConversationStreamProvider>
              <WorkspaceSwitcherContext.Provider
                value={{ active: workspace, workspaces, setWorkspace, refreshWorkspaces }}
              >
                <WorkspaceViews active={workspace} />
              </WorkspaceSwitcherContext.Provider>
            </ConversationStreamProvider>
          </ChatSidebarProvider>
        </SidebarProvider>
      </ShortcutsProvider>
    </ToastProvider>
  );
}

/** Every visited workspace stays mounted under its own router; `WorkspaceLayout`
 * hides the inactive ones. Each keeps its event watch so its cache stays current. */
function WorkspaceViews({ active }: { active: string }) {
  const [visited, setVisited] = useState<string[]>([]);
  if (active && !visited.includes(active)) setVisited([...visited, active]);

  return visited.map((ws) => (
    <Fragment key={ws}>
      <EngineEvents workspace={ws} />
      <RouterProvider router={workspaceRouter(ws)} />
    </Fragment>
  ));
}
