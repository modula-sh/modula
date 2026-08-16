import { useMutation } from "@tanstack/react-query";
import { ChevronDown } from "lucide-react";
import { useState } from "react";
import { useFeedback } from "../hooks/useFeedback";
import { client, errorMessage } from "../services/client";
import type { WorkspaceInfo } from "../types";
import { Button } from "./Button";
import { DropdownMenu } from "./DropdownMenu";
import { NewWorkspaceModal } from "./NewWorkspaceModal";

export function WorkspaceSwitcher({
  workspace,
  workspaces,
  onSwitch,
  onCreated,
}: {
  workspace: string;
  workspaces: WorkspaceInfo[];
  onSwitch: (ws: string) => void;
  onCreated: () => void;
}) {
  const [modalOpen, setModalOpen] = useState(false);
  const fb = useFeedback();
  const currentLabel = workspaces.find((w) => w.id === workspace)?.name ?? workspace;

  const create = useMutation({
    mutationFn: (body: { name: string; description?: string }) => client.workspace.create(body),
    onSuccess: (data) => {
      setModalOpen(false);
      onCreated();
      onSwitch(data.id);
    },
    onError: (e) => fb.err(errorMessage(e)),
  });
  const busy = create.isPending;

  function handleCreate({ name, description }: { name: string; description: string }) {
    const trimmed = name.trim();
    if (!trimmed) return;
    fb.clear();
    create.mutate({ name: trimmed, description: description.trim() || undefined });
  }

  return (
    <>
      <DropdownMenu
        panelClassName="w-72"
        trigger={({ open, toggle }) => (
          <button
            type="button"
            onClick={toggle}
            title="Switch workspace"
            className={
              "flex items-center justify-between gap-1.5 w-full px-2 py-1.5 rounded text-xs font-inter font-bold transition-colors " +
              (open ? "bg-surface-2/60 text-fg" : "bg-surface/60 text-fg hover:bg-surface-2/60")
            }
          >
            <span className="truncate">{currentLabel}</span>
            <ChevronDown
              size={12}
              className={`transition-transform text-fg-subtle shrink-0 ${open ? "rotate-180" : ""}`}
            />
          </button>
        )}
      >
        {({ close }) => (
          <WorkspaceMenuBody
            workspace={workspace}
            workspaces={workspaces}
            onSwitch={(id) => {
              onSwitch(id);
              close();
            }}
            onNewWorkspace={() => {
              close();
              setModalOpen(true);
            }}
          />
        )}
      </DropdownMenu>
      <NewWorkspaceModal
        open={modalOpen}
        busy={busy}
        feedback={fb.feedback}
        onCreate={handleCreate}
        onCancel={() => {
          if (busy) return;
          setModalOpen(false);
          fb.clear();
        }}
      />
    </>
  );
}

function WorkspaceMenuBody({
  workspace,
  workspaces,
  onSwitch,
  onNewWorkspace,
}: {
  workspace: string;
  workspaces: WorkspaceInfo[];
  onSwitch: (ws: string) => void;
  onNewWorkspace: () => void;
}) {
  return (
    <>
      <ul className="max-h-72 overflow-y-auto space-y-0.5">
        {workspaces.length === 0 && (
          <li className="px-2 py-2 text-xs text-fg-subtle">no workspaces</li>
        )}
        {workspaces.map((w) => (
          <li key={w.id}>
            <button
              onClick={() => onSwitch(w.id)}
              className={`w-full text-left px-2 py-1.5 rounded text-xs hover:bg-surface ${
                w.id === workspace ? "text-fg bg-surface" : "text-fg-muted"
              }`}
            >
              <span className="font-inter">{w.name}</span>
            </button>
          </li>
        ))}
      </ul>
      <div className="mt-0.5">
        <Button
          tone="link"
          onClick={onNewWorkspace}
          className="w-full text-left px-2 py-1.5 hover:bg-surface rounded normal-case"
        >
          + New workspace
        </Button>
      </div>
    </>
  );
}
