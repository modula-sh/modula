import { useEffect, useState } from "react";
import type { Feedback } from "../hooks/useFeedback";
import { contextLabel } from "../lib/contextArgs";
import type { AgentConfig } from "../types";
import { BaseModal } from "./BaseModal";
import { Button } from "./Button";
import { FeedbackText } from "./FeedbackText";
import { FieldRow } from "./FieldRow";
import { TextInput } from "./TextInput";

export function RunAgentModal({
  open,
  agent,
  busy,
  feedback,
  onRun,
  onCancel,
}: {
  open: boolean;
  agent: AgentConfig;
  busy: boolean;
  feedback: Feedback | null;
  onRun: (args: Record<string, string>) => void;
  onCancel: () => void;
}) {
  const [inputs, setInputs] = useState<Record<string, string>>({});

  useEffect(() => {
    if (open) setInputs({});
  }, [open]);

  const requiredOk = agent.args.every((a) => {
    if (!a.required) return true;
    return (inputs[a.flag.replace(/^-+/, "")] ?? "").trim() !== "";
  });
  const disabled = busy || !requiredOk;

  return (
    <BaseModal open={open} busy={busy} onCancel={onCancel}>
      <div>
        <div className="text-sm font-semibold text-fg">Run {agent.name}</div>
        {agent.description && <p className="text-xs text-fg-muted mt-0.5">{agent.description}</p>}
      </div>

      {agent.args.length === 0 ? (
        <p className="text-xs text-fg-subtle italic">no arguments required</p>
      ) : (
        <div className="space-y-2.5">
          {agent.args.map((arg) => {
            const key = arg.flag.replace(/^-+/, "");
            return (
              <FieldRow key={arg.flag} label={contextLabel(arg.flag)}>
                <TextInput
                  value={inputs[key] ?? ""}
                  onChange={(e) => setInputs((s) => ({ ...s, [key]: e.target.value }))}
                  placeholder={arg.help ?? arg.flag}
                  title={arg.help ?? ""}
                  padded
                  mono
                  className="w-full"
                  autoFocus={agent.args[0]?.flag === arg.flag}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" && !disabled) {
                      e.preventDefault();
                      onRun(inputs);
                    }
                  }}
                />
              </FieldRow>
            );
          })}
        </div>
      )}

      <div className="flex items-center gap-2 pt-1">
        <Button onClick={() => onRun(inputs)} disabled={disabled}>
          {busy ? "spawning…" : "Run"}
        </Button>
        <Button onClick={onCancel} disabled={busy} tone="link">
          Cancel
        </Button>
        <FeedbackText feedback={feedback} />
      </div>
    </BaseModal>
  );
}
