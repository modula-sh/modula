import { createBrowserRouter, Navigate } from "react-router-dom";
import { AgentEditPage } from "./views/AgentEdit";
import { AgentsView } from "./views/Agents";
import { ConversationDetailPage, ConversationsView } from "./views/Conversations";
import { LogPage, LogsView } from "./views/Logs";
import { OverviewRoute } from "./views/OverviewRoute";
import { ProjectEditPage } from "./views/ProjectEdit";
import { ProjectsView } from "./views/Projects";
import { ProviderEditPage } from "./views/ProviderEdit";
import { ProvidersView } from "./views/Providers";
import { RoadmapView } from "./views/Roadmap";
import { RootLayout } from "./views/RootLayout";
import { SettingsView } from "./views/Settings";
import { TaskDetailPage, TasksView } from "./views/Tasks";
import { UsageView } from "./views/Usage";
import { WikiView } from "./views/Wiki";

/** All routes. Single source of truth. The layout route owns app-wide state
 * (workspace, snapshot, selection, pipeline) and renders the Header; child
 * routes render into its <Outlet />.
 *
 * URL conventions:
 *   /                  — redirects to /tasks
 *   /tasks           — tasks pane + roadmap pane (no selection)
 *   /tasks/:id       — tasks pane + detail for the selected task
 *   /roadmap           — kanban
 *   /agents            — agents tile list (default tab)
 *   /agents/new        — create agent
 *   /agents/edit/:id   — edit an existing agent
 *   /agents/logs       — runs list (logs tab)
 *   /agents/logs/:log  — streamed view of one run's log
 *   /agents/usage      — usage tab (cost + tokens per claude agent run)
 *   /projects          — projects card list
 *   /projects/new      — create project
 *   /projects/edit/:name — edit an existing project
 *   /providers         — providers CRUD
 *   /overview          — three.js operations grid
 */
export const router = createBrowserRouter([
  {
    path: "/",
    element: <RootLayout />,
    children: [
      { index: true, element: <Navigate to="/tasks" replace /> },
      {
        path: "tasks",
        children: [
          { index: true, element: <TasksView /> },
          { path: ":id", element: <TaskDetailPage /> },
        ],
      },
      { path: "roadmap", element: <RoadmapView /> },
      {
        path: "agents",
        children: [
          { index: true, element: <AgentsView /> },
          { path: "new", element: <AgentEditPage /> },
          { path: "edit/:id", element: <AgentEditPage /> },
          { path: "logs", element: <LogsView /> },
          { path: "logs/:log", element: <LogPage /> },
          { path: "usage", element: <UsageView /> },
        ],
      },
      {
        path: "projects",
        children: [
          { index: true, element: <ProjectsView /> },
          { path: "new", element: <ProjectEditPage /> },
          { path: "edit/:id", element: <ProjectEditPage /> },
        ],
      },
      {
        path: "providers",
        children: [
          { index: true, element: <ProvidersView /> },
          { path: "new", element: <ProviderEditPage /> },
          { path: "edit/:id", element: <ProviderEditPage /> },
        ],
      },
      { path: "wiki", element: <WikiView /> },
      { path: "overview", element: <OverviewRoute /> },
      { path: "settings", element: <SettingsView /> },
      {
        path: "conversations",
        children: [
          { index: true, element: <ConversationsView /> },
          { path: ":id", element: <ConversationDetailPage /> },
        ],
      },

      // Catch-all → tasks.
      { path: "*", element: <Navigate to="/tasks" replace /> },
    ],
  },
]);
