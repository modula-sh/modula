import {
  BookOpen,
  Bot,
  ChevronDown,
  ChevronUp,
  Eye,
  Folder,
  LayoutGrid,
  Loader2,
  Map,
  Pencil,
  PenSquare,
  Server,
  Settings,
  Trash2,
} from "lucide-react";
import { useEffect, useState } from "react";
import { NavLink, useLocation, useNavigate } from "react-router-dom";
import { ConfirmModal } from "../components/ConfirmModal";
import { ContextMenu } from "../components/ContextMenu";
import { PromptModal } from "../components/PromptModal";
import { SnakeDots } from "../components/SnakeDots";
import { ThemeToggle } from "../components/ThemeToggle";
import { UpdateCard } from "../components/UpdateCard";
import { WorkspaceSwitcher } from "../components/WorkspaceSwitcher";
import { useStreamingConvIds } from "../contexts/ConversationStreamProvider";
import { useSidebarContext } from "../contexts/SidebarContext";
import { useSnapshot } from "../contexts/SnapshotContext";
import { useAppUpdate } from "../hooks/useAppUpdate";
import { client } from "../services/client";
import type { ConversationSummary, WorkspaceInfo } from "../types";

export type NavItem = {
  to: string;
  label: string;
  icon?: React.ReactNode;
  end?: boolean;
  children?: NavItem[];
  /** When true, children render at the same indent as the parent instead of one level deeper. */
  flatChildren?: boolean;
  /** Placeholder shown under a flat-children section when expanded and empty. */
  emptyLabel?: string;
  /** When true, the section starts expanded. */
  defaultOpen?: boolean;
  /** When true, render the label as-is (no uppercase / wide tracking nav styling). */
  preserveCase?: boolean;
  /** Optional element pinned to the row's right edge. Hidden when the sidebar is collapsed. */
  trailing?: React.ReactNode;
  /** Optional small, muted second line under the label (e.g. a task's external id). */
  subtitle?: string;
  /** Right-click handler. Receives the original mouse event. */
  onContextMenu?: (e: React.MouseEvent) => void;
};

type NavAction = {
  icon: React.ReactNode;
  onClick: (e: React.MouseEvent) => void;
  title: string;
};

export const NAV_ITEMS: NavItem[] = [
  { to: "/tasks", label: "Tasks", icon: <LayoutGrid size={16} /> },
  { to: "/roadmap", label: "Roadmap", icon: <Map size={16} /> },
  { to: "/agents", label: "Agents", icon: <Bot size={16} /> },
  { to: "/projects", label: "Projects", icon: <Folder size={16} /> },
  { to: "/providers", label: "Providers", icon: <Server size={16} /> },
  { to: "/wiki", label: "AI Wiki", icon: <BookOpen size={16} /> },
  { to: "/overview", label: "Overview", icon: <Eye size={16} /> },
];

function SidebarItem({
  item,
  depth,
  sidebarOpen,
  action,
}: {
  item: NavItem;
  depth: number;
  sidebarOpen: boolean;
  action?: NavAction;
}) {
  const location = useLocation();

  const hasActiveChild =
    item.children?.some(
      (child) => location.pathname === child.to || location.pathname.startsWith(`${child.to}/`),
    ) ?? false;

  const [expanded, setExpanded] = useState(hasActiveChild || !!item.defaultOpen);

  useEffect(() => {
    if (hasActiveChild) setExpanded(true);
  }, [hasActiveChild]);

  const indent = depth > 0 ? "pl-8 pr-3" : "px-3";
  const rowLayout = sidebarOpen
    ? `flex items-center gap-2 ${indent}`
    : "flex items-center justify-center";

  if (item.children != null) {
    // Section-style header (no icon, no hover bg, chevron-on-hover, sits next to label).
    if (item.flatChildren) {
      if (!sidebarOpen) return null;
      return (
        <div className="flex flex-col min-h-0 flex-1">
          <div className="group flex items-center w-full shrink-0">
            <button
              type="button"
              onClick={() => setExpanded((v) => !v)}
              className="flex-1 flex items-center gap-2 pl-3 py-2 text-fg-subtle/70 hover:text-fg-subtle transition-colors"
            >
              <span className="text-xs uppercase tracking-wider font-medium">{item.label}</span>
              <span className="opacity-0 group-hover:opacity-100 transition-opacity">
                {expanded ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
              </span>
            </button>
            {action && (
              <button
                type="button"
                onClick={action.onClick}
                title={action.title}
                className="shrink-0 p-1 mr-1 rounded hover:bg-surface-2 text-fg-subtle hover:text-fg transition-colors"
              >
                {action.icon}
              </button>
            )}
          </div>
          {expanded && item.children.length === 0 && item.emptyLabel && (
            <div className="px-3 py-2 text-xs text-fg-subtle/70">{item.emptyLabel}</div>
          )}
          {expanded && item.children.length > 0 && (
            <div className="space-y-1 overflow-y-auto overflow-x-hidden min-h-0 flex-1 no-scrollbar">
              {item.children.map((child) => (
                <SidebarItem key={child.to} item={child} depth={depth} sidebarOpen={sidebarOpen} />
              ))}
            </div>
          )}
        </div>
      );
    }
    const rowColor = hasActiveChild ? "text-fg" : "text-fg-subtle hover:text-fg";
    return (
      <div>
        {sidebarOpen ? (
          // Open sidebar: expander + optional action as siblings (no nested buttons).
          <div className="flex items-center w-full">
            <button
              type="button"
              onClick={() => setExpanded((v) => !v)}
              className={
                `flex-1 flex items-center gap-2 pl-3 py-2 rounded transition-colors hover:bg-surface-2/50 ` +
                rowColor
              }
            >
              {item.icon && <span className="shrink-0">{item.icon}</span>}
              <span className="flex-1 text-left text-[13px] font-medium">{item.label}</span>
              {expanded ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
            </button>
            {action && (
              <button
                type="button"
                onClick={action.onClick}
                title={action.title}
                className="shrink-0 p-1 mr-1 rounded hover:bg-surface-2 text-fg-subtle hover:text-fg transition-colors"
              >
                {action.icon}
              </button>
            )}
          </div>
        ) : (
          // Collapsed sidebar: single centered button, no action.
          <button
            type="button"
            onClick={() => setExpanded((v) => !v)}
            title={item.label}
            className={`w-full flex items-center justify-center py-2 rounded transition-colors ${rowColor}`}
          >
            <span className="shrink-0">{item.icon}</span>
            <span className="sr-only">{item.label}</span>
          </button>
        )}
        {expanded &&
          sidebarOpen &&
          item.children.map((child) => (
            <SidebarItem
              key={child.to}
              item={child}
              depth={item.flatChildren ? depth : depth + 1}
              sidebarOpen={sidebarOpen}
            />
          ))}
      </div>
    );
  }

  return (
    <NavLink
      to={item.to}
      end={item.end}
      title={!sidebarOpen ? item.label : undefined}
      onContextMenu={item.onContextMenu}
      className={({ isActive }) =>
        `${rowLayout} py-2 rounded transition-colors ` +
        (isActive
          ? "bg-surface-2/50 text-fg"
          : "text-fg-subtle hover:text-fg hover:bg-surface-2/30")
      }
    >
      {item.icon && <span className="shrink-0">{item.icon}</span>}
      {sidebarOpen ? (
        <>
          <span className="min-w-0 flex-1 flex flex-col justify-center leading-tight gap-0.5">
            <span className="truncate text-[13px] font-medium">{item.label}</span>
            {item.subtitle && (
              <span className="truncate text-[10px] font-normal uppercase tracking-wide text-fg-subtle/70">
                {item.subtitle}
              </span>
            )}
          </span>
          {item.trailing && <span className="shrink-0">{item.trailing}</span>}
        </>
      ) : (
        <span className="sr-only">{item.label}</span>
      )}
    </NavLink>
  );
}

export function Sidebar({
  workspace,
  workspaces,
  onSwitchWorkspace,
  onRefreshWorkspaces,
  ref,
}: {
  workspace: string;
  workspaces: WorkspaceInfo[];
  onSwitchWorkspace: (ws: string) => void;
  onRefreshWorkspaces: () => void;
  /** RootLayout measures the nav's live width for the auto-collapse decision. */
  ref?: React.Ref<HTMLElement>;
}) {
  const { open } = useSidebarContext();
  const { snap } = useSnapshot();
  const navigate = useNavigate();
  const location = useLocation();
  const streamingIds = useStreamingConvIds();
  const streaming = new Set(streamingIds);
  const appUpdate = useAppUpdate();

  const [menu, setMenu] = useState<{ x: number; y: number; conv: ConversationSummary } | null>(
    null,
  );
  const [renameTarget, setRenameTarget] = useState<ConversationSummary | null>(null);
  const [renameValue, setRenameValue] = useState("");
  const [renaming, setRenaming] = useState(false);
  const [deleteTarget, setDeleteTarget] = useState<ConversationSummary | null>(null);
  const [deleting, setDeleting] = useState(false);

  async function commitRename() {
    if (!renameTarget) return;
    setRenaming(true);
    try {
      await client.conversation.rename(workspace, renameTarget.id, renameValue.trim());
      setRenameTarget(null);
    } finally {
      setRenaming(false);
    }
  }

  async function commitDelete() {
    if (!deleteTarget) return;
    setDeleting(true);
    try {
      await client.conversation.delete(workspace, deleteTarget.id);
      if (location.pathname.startsWith(`/conversations/${deleteTarget.id}`)) {
        navigate("/conversations");
      }
      setDeleteTarget(null);
    } finally {
      setDeleting(false);
    }
  }

  const agentCount = snap?.agents?.length ?? 0;
  const navItems: NavItem[] = NAV_ITEMS.map((it) =>
    it.to === "/agents" && agentCount > 0
      ? {
          ...it,
          trailing: (
            <span className="flex items-center gap-1 text-fg-subtle tabular-nums text-xs">
              <Loader2 size={12} className="animate-spin" />
              {agentCount}
            </span>
          ),
        }
      : it,
  );

  const conversationChildren: NavItem[] = (snap?.conversations ?? []).map((c) => {
    const projectName = c.context.project
      ? snap?.config?.projects?.find((p) => p.id === c.context.project)?.name
      : undefined;
    const task = c.context.task ? snap?.tasks?.find((t) => t.id === c.context.task) : undefined;
    const taskExternalId = task?.external_id ?? undefined;
    const variant = c.context.variant
      ? task?.variants.find((v) => v.id === c.context.variant)
      : undefined;
    const variantLabel = variant ? `v${variant.position}` : undefined;
    const taskPart = [taskExternalId, variantLabel].filter(Boolean).join(" ") || undefined;
    const subtitle = [projectName, taskPart].filter(Boolean).join(" · ") || undefined;
    return {
      to: `/conversations/${c.id}`,
      label: c.title || "Untitled",
      subtitle,
      icon: streaming.has(c.id) ? <SnakeDots /> : undefined,
      preserveCase: true,
      onContextMenu: (e) => {
        e.preventDefault();
        setMenu({ x: e.clientX, y: e.clientY, conv: c });
      },
    };
  });

  const conversationsItem: NavItem = {
    to: "/conversations",
    label: "Chats",
    end: true,
    children: conversationChildren,
    flatChildren: true,
    emptyLabel: "No chats",
    defaultOpen: true,
  };

  const conversationsAction: NavAction = {
    icon: <PenSquare size={13} />,
    onClick: (e) => {
      e.stopPropagation();
      navigate("/conversations");
    },
    title: "New chat",
  };

  return (
    <>
      {/* Transparent: reads as part of the base plate the content card sits on. */}
      <aside
        ref={ref}
        id="sidebar"
        className={`shrink-0 flex flex-col font-inter select-none transition-[width] duration-200 ease-out overflow-hidden ${open ? "w-[264px]" : "w-12"}`}
      >
        <div className="shrink-0 p-2 pb-4">
          {open ? (
            <div className="flex items-center gap-2 pl-[7px]">
              <WorkspaceTile workspace={workspace} workspaces={workspaces} />
              <div className="flex-1 min-w-0">
                <WorkspaceSwitcher
                  workspace={workspace}
                  workspaces={workspaces}
                  onSwitch={onSwitchWorkspace}
                  onCreated={onRefreshWorkspaces}
                />
              </div>
            </div>
          ) : (
            <div className="flex justify-center">
              <WorkspaceTile workspace={workspace} workspaces={workspaces} />
            </div>
          )}
        </div>
        <nav className="flex-1 flex flex-col min-h-0 p-2 overflow-x-hidden">
          <div className="shrink-0 space-y-0.5">
            {navItems.map((item) => (
              <SidebarItem key={item.to} item={item} depth={0} sidebarOpen={open} />
            ))}
          </div>
          <div className="pt-4 flex-1 min-h-0 flex flex-col">
            <SidebarItem
              key="/conversations"
              item={conversationsItem}
              depth={0}
              sidebarOpen={open}
              action={conversationsAction}
            />
          </div>
        </nav>
        {open && <UpdateCard {...appUpdate} />}
        <div className="p-2">
          {open ? (
            <div className="flex items-center gap-1">
              <div className="flex-1 min-w-0">
                <SidebarItem
                  item={{ to: "/settings", label: "Settings", icon: <Settings size={16} /> }}
                  depth={0}
                  sidebarOpen={open}
                />
              </div>
              <ThemeToggle />
            </div>
          ) : (
            <SidebarItem
              item={{ to: "/settings", label: "Settings", icon: <Settings size={16} /> }}
              depth={0}
              sidebarOpen={open}
            />
          )}
        </div>
      </aside>
      {menu && (
        <ContextMenu
          x={menu.x}
          y={menu.y}
          items={[
            {
              label: "Rename",
              icon: <Pencil size={12} />,
              onClick: () => {
                setRenameValue(menu.conv.title || "Untitled");
                setRenameTarget(menu.conv);
              },
            },
            {
              label: "Delete",
              icon: <Trash2 size={12} />,
              destructive: true,
              onClick: () => setDeleteTarget(menu.conv),
            },
          ]}
          onClose={() => setMenu(null)}
        />
      )}
      <PromptModal
        open={!!renameTarget}
        title="Rename conversation"
        value={renameValue}
        onChange={setRenameValue}
        busy={renaming}
        confirmLabel="Save"
        onConfirm={commitRename}
        onCancel={() => setRenameTarget(null)}
      />
      <ConfirmModal
        open={!!deleteTarget}
        title="Delete conversation"
        body={
          <p>
            Delete <span className="font-mono">{deleteTarget?.title || "this conversation"}</span>?
            This cannot be undone.
          </p>
        }
        busy={deleting}
        confirmLabel="Delete"
        onConfirm={commitDelete}
        onCancel={() => setDeleteTarget(null)}
      />
    </>
  );
}

function WorkspaceTile({
  workspace,
  workspaces,
}: {
  workspace: string;
  workspaces: WorkspaceInfo[];
}) {
  const name = workspaces.find((w) => w.id === workspace)?.name ?? workspace;
  return (
    <div className="w-7 h-7 shrink-0 rounded border border-border flex items-center justify-center font-mono text-[10px] text-fg uppercase">
      {name.slice(0, 2)}
    </div>
  );
}
