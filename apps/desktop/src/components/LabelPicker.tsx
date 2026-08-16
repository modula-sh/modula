import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Plus } from "lucide-react";
import { useContext, useState } from "react";
import { WorkspaceContext } from "../contexts/WorkspaceContext";
import { useFeedback } from "../hooks/useFeedback";
import { labelKeys, useLabels } from "../queries/label";
import { client, errorMessage } from "../services/client";
import type { Label } from "../types";
import { DropdownMenu } from "./DropdownMenu";
import { FeedbackText } from "./FeedbackText";

const INPUT_CLASS =
  "w-full bg-transparent border-0 border-b border-border px-2 py-1.5 mb-0.5 text-xs font-inter text-fg placeholder-fg-subtle focus:outline-none";
const OPTION_CLASS =
  "block w-full text-left truncate px-2 py-1.5 rounded text-xs font-inter text-fg-muted hover:bg-surface";

// Search the existing label pool or create a new one, then attach it to the task.
export function LabelPicker({ taskId, attached }: { taskId: string; attached: Label[] }) {
  const ws = useContext(WorkspaceContext);
  const queryClient = useQueryClient();
  const { data: labels = [] } = useLabels(ws);
  const fb = useFeedback();
  const [query, setQuery] = useState("");

  // Create invalidates the pool so the new label is offered immediately.
  const createLabel = useMutation({
    mutationFn: (name: string) => client.label.create(ws, { name }),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: labelKeys.list(ws) }),
  });
  const attachLabel = useMutation({
    mutationFn: (labelId: string) => client.label.attach(ws, taskId, labelId),
  });

  const attachedIds = new Set(attached.map((l) => l.id));
  const q = query.trim().toLowerCase();
  const filtered = labels.filter((l) => !attachedIds.has(l.id) && l.name.toLowerCase().includes(q));
  const exact = labels.some((l) => l.name.toLowerCase() === q);

  async function attach(labelId: string, close: () => void) {
    try {
      await attachLabel.mutateAsync(labelId);
      setQuery("");
      close();
    } catch (e) {
      fb.err(errorMessage(e));
    }
  }

  async function createAndAttach(close: () => void) {
    try {
      const { id } = await createLabel.mutateAsync(query.trim());
      await attach(id, close);
    } catch (e) {
      fb.err(errorMessage(e));
    }
  }

  return (
    <DropdownMenu
      panelClassName="w-48"
      trigger={({ toggle }) => (
        <button
          type="button"
          onClick={toggle}
          className="inline-flex items-center gap-1 px-[8.5px] py-[4.5px] text-xs font-inter rounded-full border border-dashed border-border text-fg-muted hover:text-fg hover:bg-surface"
        >
          <Plus size={10} /> Add label
        </button>
      )}
    >
      {({ close }) => (
        <>
          <input
            type="text"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Search or create…"
            autoFocus
            className={INPUT_CLASS}
          />
          <ul className="max-h-72 overflow-y-auto space-y-0.5">
            {q && !exact && (
              <li>
                <button
                  type="button"
                  onClick={() => createAndAttach(close)}
                  className={OPTION_CLASS}
                >
                  Create "{query.trim()}"
                </button>
              </li>
            )}
            {filtered.map((l) => (
              <li key={l.id}>
                <button type="button" onClick={() => attach(l.id, close)} className={OPTION_CLASS}>
                  {l.name}
                </button>
              </li>
            ))}
            {filtered.length === 0 && (!q || exact) && (
              <li className="px-2 py-2 text-xs text-fg-subtle">no labels</li>
            )}
          </ul>
          <FeedbackText feedback={fb.feedback} />
        </>
      )}
    </DropdownMenu>
  );
}
