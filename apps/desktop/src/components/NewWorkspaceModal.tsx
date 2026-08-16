import { useEffect, useState } from "react";
import type { Feedback } from "../hooks/useFeedback";
import { BaseModal } from "./BaseModal";
import { Button } from "./Button";
import { FeedbackText } from "./FeedbackText";
import { FieldRow } from "./FieldRow";
import { TextInput } from "./TextInput";

export function NewWorkspaceModal({
  open,
  busy,
  feedback,
  onCreate,
  onCancel,
}: {
  open: boolean;
  busy: boolean;
  feedback: Feedback | null;
  onCreate: (args: { name: string; description: string }) => void;
  onCancel: () => void;
}) {
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");

  useEffect(() => {
    if (open) {
      setName("");
      setDescription("");
    }
  }, [open]);

  const disabled = busy || !name.trim();
  const submit = () => onCreate({ name, description });
  const onEnter = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter" && !disabled) {
      e.preventDefault();
      submit();
    }
  };

  return (
    <BaseModal open={open} busy={busy} onCancel={onCancel}>
      <div className="text-sm font-semibold text-fg">New workspace</div>
      <div className="space-y-2.5">
        <FieldRow label="name">
          <TextInput
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="Display name"
            padded
            autoFocus
            className="w-full"
            onKeyDown={onEnter}
          />
        </FieldRow>
        <FieldRow label="description">
          <TextInput
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            placeholder="optional"
            padded
            className="w-full"
            onKeyDown={onEnter}
          />
        </FieldRow>
      </div>
      <div className="flex items-center gap-2 pt-1">
        <Button onClick={submit} disabled={disabled}>
          {busy ? "creating…" : "Create"}
        </Button>
        <Button onClick={onCancel} disabled={busy} tone="link">
          Cancel
        </Button>
        <FeedbackText feedback={feedback} />
      </div>
    </BaseModal>
  );
}
