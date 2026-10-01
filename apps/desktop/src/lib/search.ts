import { BookOpen, Bot, Folder, Layers, LayoutGrid, MessageSquare, Server } from "lucide-react";
import type { SearchKind } from "../types";

/** The single place a new searchable entity has to be taught to the modal.
 * Icons match the sidebar's, so a result reads as the section it lives in. */
export const SEARCH_KINDS: Record<
  SearchKind,
  /** `path` is absent for a workspace, which is switched to rather than routed to. */
  { icon: typeof LayoutGrid; label: string; path?: (id: string) => string }
> = {
  workspace: { icon: Layers, label: "Workspaces" },
  task: { icon: LayoutGrid, label: "Tasks", path: (id) => `/tasks/${id}` },
  conversation: {
    icon: MessageSquare,
    label: "Chats",
    path: (id) => `/conversations/${id}`,
  },
  agent: { icon: Bot, label: "Agents", path: (id) => `/agents/edit/${id}` },
  project: { icon: Folder, label: "Projects", path: (id) => `/projects/edit/${id}` },
  provider: { icon: Server, label: "Providers", path: (id) => `/providers/edit/${id}` },
  wiki: { icon: BookOpen, label: "AI Wiki", path: (id) => `/wiki?path=${encodeURIComponent(id)}` },
};
