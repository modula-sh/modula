import { BookOpen, Bot, Folder, LayoutGrid, MessageSquare, Server } from "lucide-react";
import type { SearchKind } from "../types";

/** The single place a new searchable entity has to be taught to the modal.
 * Icons match the sidebar's, so a result reads as the section it lives in. */
export const SEARCH_KINDS: Record<
  SearchKind,
  { icon: typeof LayoutGrid; label: string; path: (id: string) => string }
> = {
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
