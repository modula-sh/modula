import { useState } from "react";
import { useFeedback } from "../../hooks/useFeedback";
import { client, errorMessage } from "../../services/client";
import type { WorkspaceInfo } from "../../types";
import { FeedbackText } from "../FeedbackText";
import { FieldRow } from "../FieldRow";
import { LargeButton } from "../LargeButton";
import { TextInput } from "../TextInput";
import { OnboardingActions } from "./OnboardingActions";
import { OnboardingTitle } from "./OnboardingTitle";

export function CreateWorkspace({
  onCreated,
  onBack,
}: {
  onCreated: (ws: WorkspaceInfo) => void;
  onBack?: () => void;
}) {
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [busy, setBusy] = useState(false);
  const fb = useFeedback();
  const disabled = busy || !name.trim();

  async function submit() {
    if (disabled) return;
    setBusy(true);
    fb.clear();
    const trimmedName = name.trim();
    const trimmedDesc = description.trim();
    try {
      const data = await client.workspace.create({
        name: trimmedName,
        description: trimmedDesc || undefined,
      });
      onCreated({
        id: data.id,
        name: data.name,
        path: data.path,
        description: trimmedDesc || null,
      });
    } catch (e: unknown) {
      fb.err(errorMessage(e));
      setBusy(false);
    }
  }

  const onEnter = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter" && !disabled) {
      e.preventDefault();
      submit();
    }
  };

  return (
    <>
      <OnboardingTitle>Create workspace</OnboardingTitle>
      <section className="w-[32rem] border border-card-border/50 bg-card rounded-xl px-3 font-inter">
        <FieldRow label="name" description="Display name shown in the dashboard." inputCol="1/2">
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
        <FieldRow
          label="description"
          description="Optional notes about this workspace."
          inputCol="1/2"
        >
          <TextInput
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            placeholder="Optional"
            padded
            className="w-full"
            onKeyDown={onEnter}
          />
        </FieldRow>
      </section>
      <OnboardingActions onBack={onBack} className="mt-4">
        <LargeButton onClick={submit} disabled={disabled}>
          {busy ? "Creating…" : "Create"}
        </LargeButton>
      </OnboardingActions>
      <FeedbackText feedback={fb.feedback} />
    </>
  );
}
