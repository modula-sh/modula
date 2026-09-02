import { useMutation, useQueryClient } from "@tanstack/react-query";
import { ChevronDown, ChevronRight, FilePlus, FolderPlus, Pencil, Trash2 } from "lucide-react";
import { useCallback, useContext, useEffect, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { Button } from "../components/Button";
import { ConfirmModal } from "../components/ConfirmModal";
import { HeaderSlot } from "../components/HeaderSlot";
import { MarkdownEditor } from "../components/MarkdownEditor";
import { PromptModal } from "../components/PromptModal";
import { Spinner } from "../components/Spinner";
import { WorkspaceContext } from "../contexts/WorkspaceContext";
import { useWikiFile, useWikiTree, wikiKeys } from "../queries/wiki";
import type { WikiFile, WikiFileBody, WikiNode } from "../services/client";
import { client, errorMessage } from "../services/client";

/** Wiki view — left tree, right editor. The wiki is the workspace's
 * agent-maintained knowledge base at `<ws>/wiki/`. This page is the human's
 * read/edit surface for it (agents go through the filesystem directly).
 *
 * No tabs by design — exactly one file is open at a time. */

/** One state shape covers all four mutation modals. `delete` is the only
 * one without an input field; the rest collect a path string. */
type Modal =
  | { kind: "new-file"; value: string }
  | { kind: "new-folder"; value: string }
  | { kind: "rename"; node: WikiNode; value: string }
  | { kind: "delete"; node: WikiNode }
  | null;

/** Drag-and-drop state + handlers passed to the tree. Bundled into one
 * object so individual nodes don't need to enumerate six separate props. */
type DragState = {
  draggingPath: string | null;
  dragOverPath: string | null;
  onStart: (path: string) => void;
  onEnd: () => void;
  onOver: (path: string | null) => void;
  onDrop: (source: string, target: string) => void;
  isValidTarget: (target: string) => boolean;
};

// Storage paths always carry the real extension (`foo.md`). Display paths
// drop the `.md` for markdown files — Obsidian's convention. Other
// extensions (png, pdf, …) stay visible as a corner pill.

function lastSegment(p: string): string {
  const i = p.lastIndexOf("/");
  return i === -1 ? p : p.slice(i + 1);
}

function fileExtension(name: string): string {
  const seg = lastSegment(name);
  const i = seg.lastIndexOf(".");
  return i > 0 ? seg.slice(i + 1) : "";
}

/** Strip the file extension from the last path segment.
 * `general/photo.png` → `general/photo`; `notes.md` → `notes`;
 * `script` (no extension) → unchanged. */
function stripExtension(s: string): string {
  const seg = lastSegment(s);
  const i = seg.lastIndexOf(".");
  if (i <= 0) return s;
  return s.slice(0, -(seg.length - i));
}

/** Apply Obsidian's rename rule: if the typed value carries no extension
 * in its last segment, append `fallbackExt` (with the leading dot). */
function ensureExtension(typed: string, fallbackExt: string): string {
  if (!fallbackExt) return typed;
  if (fileExtension(typed)) return typed;
  return `${typed}.${fallbackExt}`;
}

export function WikiView() {
  const ws = useContext(WorkspaceContext);
  const queryClient = useQueryClient();
  const { data: tree } = useWikiTree(ws);
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const pathParam = useSearchParams()[0].get("path");

  // A search result deep-links here; re-select whenever the param changes.
  useEffect(() => {
    if (pathParam) setSelectedPath(pathParam);
  }, [pathParam]);
  const { data: file } = useWikiFile(ws, selectedPath);
  const [draft, setDraft] = useState<string | null>(null);
  const [modal, setModal] = useState<Modal>(null);
  const [modalBusy, setModalBusy] = useState(false);
  const [draggingPath, setDraggingPath] = useState<string | null>(null);
  const [dragOverPath, setDragOverPath] = useState<string | null>(null);

  // `draft` is the editable copy; `original` is the server copy from the query.
  const original = file?.content ?? null;
  const content = draft;
  const dirty = content !== null && original !== null && content !== original;

  // Sync the editable draft from the server copy whenever it (re)loads, and
  // clear it when no file is selected (the query is disabled, `file` undefined).
  useEffect(() => {
    setDraft(file ? file.content : null);
  }, [file]);

  // Reset selection on workspace switch — the wiki is workspace-scoped.
  useEffect(() => {
    setSelectedPath(null);
  }, [ws]);

  const invalidateTree = () => queryClient.invalidateQueries({ queryKey: wikiKeys.tree(ws) });

  const saveMutation = useMutation({
    mutationFn: (body: WikiFileBody) => client.wiki.saveFile(ws, body),
    // Reflect the saved content into the cache so `original` updates without a
    // round-trip; this clears the dirty flag.
    onSuccess: (_void, body) =>
      queryClient.setQueryData<WikiFile>(wikiKeys.file(ws, body.path), {
        path: body.path,
        content: body.content,
      }),
    onError: (e) => alert(errorMessage(e)),
  });
  const busy = saveMutation.isPending;

  function save() {
    if (selectedPath === null || content === null) return;
    saveMutation.mutate({ path: selectedPath, content });
  }

  // Modal openers — store enough state for the modal to render. The actual
  // mutation runs in `onModalConfirm`, which closes the modal on success and
  // leaves it open (with the error alerted) on failure so the user can retry.
  const openNewFile = () => setModal({ kind: "new-file", value: "" });
  const openNewFolder = () => setModal({ kind: "new-folder", value: "" });
  const openRename = (node: WikiNode) =>
    setModal({ kind: "rename", node, value: stripExtension(node.path) });
  const openDelete = (node: WikiNode) => setModal({ kind: "delete", node });

  // Remap or drop the selection when a rename/delete/move affects the open
  // file (or one of its ancestors). Shared between rename, delete, and DnD.
  function isSelectionAffectedBy(path: string): boolean {
    return selectedPath === path || (selectedPath !== null && selectedPath.startsWith(`${path}/`));
  }

  // Reject self-drops, drops onto own descendants (creates a cycle), and
  // drops back onto the source's existing parent (no-op that would 409).
  function isValidDropTarget(target: string): boolean {
    if (draggingPath === null) return false;
    if (target === draggingPath) return false;
    if (target.startsWith(`${draggingPath}/`)) return false;
    const sourceParent = draggingPath.includes("/")
      ? draggingPath.slice(0, draggingPath.lastIndexOf("/"))
      : "";
    if (sourceParent === target) return false;
    return true;
  }

  async function moveNode(source: string, target: string) {
    if (!isValidDropTarget(target)) return;
    const name = source.slice(source.lastIndexOf("/") + 1);
    const next = target ? `${target}/${name}` : name;
    try {
      await client.wiki.rename(ws, source, next);
      if (isSelectionAffectedBy(source)) {
        const tail = selectedPath!.slice(source.length);
        setSelectedPath(next + tail);
      }
      await invalidateTree();
    } catch (e) {
      alert(errorMessage(e));
    }
  }

  const drag: DragState = {
    draggingPath,
    dragOverPath,
    onStart: setDraggingPath,
    onEnd: () => {
      setDraggingPath(null);
      setDragOverPath(null);
    },
    onOver: setDragOverPath,
    onDrop: moveNode,
    isValidTarget: isValidDropTarget,
  };

  async function onModalConfirm() {
    if (!modal) return;
    setModalBusy(true);
    try {
      if (modal.kind === "new-file") {
        // Default new files to `.md` (Obsidian convention); if the user
        // typed an explicit extension, honour it.
        const path = ensureExtension(modal.value.trim(), "md");
        await client.wiki.createFile(ws, path);
        await invalidateTree();
        setSelectedPath(path);
      } else if (modal.kind === "new-folder") {
        await client.wiki.createFolder(ws, modal.value.trim());
        await invalidateTree();
      } else if (modal.kind === "rename") {
        // Preserve original extension when the user types no extension.
        // Folders have no extension, so this is a no-op for them.
        const origExt = modal.node.type === "file" ? fileExtension(modal.node.name) : "";
        const next = ensureExtension(modal.value.trim(), origExt);
        if (next && next !== modal.node.path) {
          await client.wiki.rename(ws, modal.node.path, next);
          if (isSelectionAffectedBy(modal.node.path)) {
            const tail = selectedPath!.slice(modal.node.path.length);
            setSelectedPath(next + tail);
          }
          await invalidateTree();
        }
      } else if (modal.kind === "delete") {
        await client.wiki.delete(ws, modal.node.path);
        if (isSelectionAffectedBy(modal.node.path)) setSelectedPath(null);
        await invalidateTree();
      }
      setModal(null);
    } catch (e) {
      alert(errorMessage(e));
    } finally {
      setModalBusy(false);
    }
  }

  return (
    <main className="flex-1 flex overflow-hidden">
      <WikiTree
        tree={tree ?? null}
        selectedPath={selectedPath}
        onSelect={setSelectedPath}
        onNewFile={openNewFile}
        onNewFolder={openNewFolder}
        onRename={openRename}
        onDelete={openDelete}
        drag={drag}
      />
      <WikiEditor
        path={selectedPath}
        content={content}
        dirty={dirty}
        busy={busy}
        onChange={setDraft}
        onSave={save}
      />
      {modal && modal.kind === "delete" && (
        <ConfirmModal
          open
          busy={modalBusy}
          title={modalTitle(modal)}
          body={<DeleteBody node={modal.node} />}
          confirmLabel="Delete"
          onConfirm={onModalConfirm}
          onCancel={() => setModal(null)}
        />
      )}
      {modal && modal.kind !== "delete" && (
        <PromptModal
          open
          busy={modalBusy}
          title={modalTitle(modal)}
          value={modal.value}
          onChange={(v) => setModal({ ...modal, value: v })}
          placeholder={modalPlaceholder(modal.kind)}
          confirmLabel={modalConfirmLabel(modal)}
          onConfirm={onModalConfirm}
          onCancel={() => setModal(null)}
        />
      )}
    </main>
  );
}

function modalTitle(modal: Modal): string {
  if (!modal) return "";
  if (modal.kind === "new-file") return "New file";
  if (modal.kind === "new-folder") return "New folder";
  if (modal.kind === "rename") return `Rename ${modal.node.type === "dir" ? "folder" : "file"}`;
  return `Delete ${modal.node.type === "dir" ? "folder" : "file"}`;
}

function modalConfirmLabel(modal: { kind: "new-file" | "new-folder" | "rename" }): string {
  return modal.kind === "rename" ? "Rename" : "Create";
}

function modalPlaceholder(kind: "new-file" | "new-folder" | "rename"): string {
  if (kind === "new-file") return "e.g. general/notes";
  if (kind === "new-folder") return "e.g. general/notes";
  return "new path";
}

function DeleteBody({ node }: { node: WikiNode }) {
  return (
    <>
      <p>
        <span className="font-mono text-fg">{node.path}</span>
        {node.type === "dir" ? " and everything inside it" : ""} will be permanently deleted.
      </p>
      <p className="text-fg-subtle">This cannot be undone.</p>
    </>
  );
}

function WikiTree({
  tree,
  selectedPath,
  onSelect,
  onNewFile,
  onNewFolder,
  onRename,
  onDelete,
  drag,
}: {
  tree: WikiNode[] | null;
  selectedPath: string | null;
  onSelect: (path: string) => void;
  onNewFile: () => void;
  onNewFolder: () => void;
  onRename: (node: WikiNode) => void;
  onDelete: (node: WikiNode) => void;
  drag: DragState;
}) {
  return (
    <aside className="w-64 shrink-0 border-r border-edge flex flex-col font-inter">
      <div className="h-10 shrink-0 px-3 border-b border-edge flex items-center justify-center gap-2">
        <button
          onClick={onNewFile}
          title="New file"
          className="p-1.5 rounded text-fg-muted hover:text-fg hover:bg-surface transition-colors"
        >
          <FilePlus size={16} />
        </button>
        <button
          onClick={onNewFolder}
          title="New folder"
          className="p-1.5 rounded text-fg-muted hover:text-fg hover:bg-surface transition-colors"
        >
          <FolderPlus size={16} />
        </button>
      </div>
      <div
        className={
          "flex-1 overflow-y-auto px-2 py-2" +
          (drag.dragOverPath === "" ? " ring-1 ring-inset ring-border-focus" : "")
        }
        onDragOver={(e) => {
          if (drag.isValidTarget("")) {
            e.preventDefault();
            drag.onOver("");
          }
        }}
        onDragLeave={(e) => {
          if (!e.currentTarget.contains(e.relatedTarget as Node | null)) {
            drag.onOver(null);
          }
        }}
        onDrop={(e) => {
          if (drag.draggingPath && drag.isValidTarget("")) {
            e.preventDefault();
            drag.onDrop(drag.draggingPath, "");
          }
          drag.onEnd();
        }}
      >
        {tree === null ? (
          <div className="px-1 py-1">
            <Spinner />
          </div>
        ) : tree.length === 0 ? (
          <div className="px-1 py-1 text-sm text-fg-subtle">empty wiki</div>
        ) : (
          tree.map((node) => (
            <WikiTreeNode
              key={node.path}
              node={node}
              depth={0}
              selectedPath={selectedPath}
              onSelect={onSelect}
              onRename={onRename}
              onDelete={onDelete}
              drag={drag}
            />
          ))
        )}
      </div>
    </aside>
  );
}

function WikiTreeNode({
  node,
  depth,
  selectedPath,
  onSelect,
  onRename,
  onDelete,
  drag,
}: {
  node: WikiNode;
  depth: number;
  selectedPath: string | null;
  onSelect: (path: string) => void;
  onRename: (node: WikiNode) => void;
  onDelete: (node: WikiNode) => void;
  drag: DragState;
}) {
  const [expanded, setExpanded] = useState(true);
  const isSelected = node.type === "file" && selectedPath === node.path;
  const indent = depth * 16 + 8;
  const isDragOver = node.type === "dir" && drag.dragOverPath === node.path;

  // Drop-target highlight overrides hover/selected so the user sees one
  // clear visual signal during a drag.
  const stateClass = isDragOver
    ? "bg-surface-2 ring-1 ring-inset ring-border-focus text-fg"
    : isSelected
      ? "bg-surface-2 text-fg"
      : "text-fg-muted hover:bg-surface hover:text-fg";
  const rowClass =
    "group flex items-center gap-1.5 text-sm cursor-pointer py-1 pr-2 rounded transition-colors " +
    stateClass;

  // Shared drag-source handlers — used by both folders and files.
  const dragSourceProps = {
    draggable: true,
    onDragStart: (e: React.DragEvent) => {
      e.stopPropagation();
      e.dataTransfer.setData("text/plain", node.path);
      e.dataTransfer.effectAllowed = "move";
      drag.onStart(node.path);
    },
    onDragEnd: drag.onEnd,
  };

  if (node.type === "dir") {
    return (
      <div
        onDragOver={(e) => {
          if (drag.isValidTarget(node.path)) {
            e.preventDefault();
            e.stopPropagation();
            drag.onOver(node.path);
          }
        }}
        onDragLeave={(e) => {
          if (!e.currentTarget.contains(e.relatedTarget as Node | null)) {
            drag.onOver(null);
          }
        }}
        onDrop={(e) => {
          e.stopPropagation();
          if (drag.draggingPath && drag.isValidTarget(node.path)) {
            e.preventDefault();
            drag.onDrop(drag.draggingPath, node.path);
          }
          drag.onEnd();
        }}
      >
        <div
          {...dragSourceProps}
          className={rowClass}
          style={{ paddingLeft: `${indent}px` }}
          onClick={() => setExpanded((v) => !v)}
        >
          {expanded ? (
            <ChevronDown size={14} className="text-fg-subtle shrink-0" />
          ) : (
            <ChevronRight size={14} className="text-fg-subtle shrink-0" />
          )}
          <span className="flex-1 truncate">{node.name}</span>
          <NodeActions node={node} onRename={onRename} onDelete={onDelete} />
        </div>
        {expanded &&
          node.children?.map((child) => (
            <WikiTreeNode
              key={child.path}
              node={child}
              depth={depth + 1}
              selectedPath={selectedPath}
              onSelect={onSelect}
              onRename={onRename}
              onDelete={onDelete}
              drag={drag}
            />
          ))}
      </div>
    );
  }

  const ext = fileExtension(node.name);
  const showExtPill = ext !== "" && ext.toLowerCase() !== "md";
  return (
    <div
      {...dragSourceProps}
      className={rowClass}
      style={{ paddingLeft: `${indent + 20}px` }}
      onClick={() => onSelect(node.path)}
    >
      <span className="flex-1 truncate">{stripExtension(node.name)}</span>
      {showExtPill && (
        <span className="shrink-0 text-[10px] font-mono uppercase tracking-wide text-fg-subtle bg-surface px-1.5 py-0.5 rounded">
          {ext.toUpperCase()}
        </span>
      )}
      <NodeActions node={node} onRename={onRename} onDelete={onDelete} />
    </div>
  );
}

function NodeActions({
  node,
  onRename,
  onDelete,
}: {
  node: WikiNode;
  onRename: (node: WikiNode) => void;
  onDelete: (node: WikiNode) => void;
}) {
  return (
    <span className="opacity-0 group-hover:opacity-100 flex items-center gap-0.5 transition-opacity">
      <button
        onClick={(e) => {
          e.stopPropagation();
          onRename(node);
        }}
        title="Rename"
        className="text-fg-subtle hover:text-fg p-0.5"
      >
        <Pencil size={12} />
      </button>
      <button
        onClick={(e) => {
          e.stopPropagation();
          onDelete(node);
        }}
        title="Delete"
        className="text-fg-subtle hover:text-red-400 p-0.5"
      >
        <Trash2 size={12} />
      </button>
    </span>
  );
}

const VIM_KEY = "modula.wiki.vim";
const readVim = () => {
  try {
    return localStorage.getItem(VIM_KEY) === "true";
  } catch {
    return false;
  }
};

function WikiEditor({
  path,
  content,
  dirty,
  busy,
  onChange,
  onSave,
}: {
  path: string | null;
  content: string | null;
  dirty: boolean;
  busy: boolean;
  onChange: (v: string) => void;
  onSave: () => void;
}) {
  const [vimMode, setVimMode] = useState(readVim);
  const toggleVim = useCallback(() => {
    setVimMode((v) => {
      const next = !v;
      try {
        localStorage.setItem(VIM_KEY, String(next));
      } catch {
        // ignore (private mode etc)
      }
      return next;
    });
  }, []);

  // ⌘/Ctrl-S to save while editing.
  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if ((e.metaKey || e.ctrlKey) && e.key === "s" && dirty && !busy) {
        e.preventDefault();
        onSave();
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [dirty, busy, onSave]);

  if (path === null) {
    return (
      <section className="flex-1 flex items-center justify-center text-fg-subtle text-xs">
        Select a file to view or edit
      </section>
    );
  }

  if (content === null) {
    return (
      <section className="flex-1 flex items-center justify-center">
        <Spinner size={24} />
      </section>
    );
  }

  return (
    <section className="flex-1 flex flex-col">
      <HeaderSlot>
        <span className="flex-1 min-w-0 text-xs font-mono text-fg truncate" title={path}>
          {stripExtension(path)}
        </span>
        {dirty && (
          <span className="text-[11px] text-fg-subtle uppercase tracking-wider">unsaved</span>
        )}
        <button
          type="button"
          onClick={toggleVim}
          title={`Vim mode: ${vimMode ? "on" : "off"}`}
          className={
            "text-[10px] uppercase tracking-wider px-2 py-0.5 rounded border transition-colors " +
            (vimMode
              ? "bg-surface-2 text-fg border-border-focus"
              : "text-fg-subtle border-border hover:text-fg hover:bg-surface")
          }
        >
          vim
        </button>
        <Button onClick={onSave} disabled={!dirty || busy}>
          {busy ? "Saving…" : "Save"}
        </Button>
      </HeaderSlot>
      <MarkdownEditor
        key={`${path}-${vimMode ? "vim" : "normal"}`}
        value={content}
        onChange={onChange}
        onSave={onSave}
        vimMode={vimMode}
        className="flex-1 min-h-0 bg-bg text-fg font-inter"
      />
    </section>
  );
}
