import { useEffect, useRef } from "react";
import { AI_ASSIST_PLACEHOLDER, AI_ASSIST_SUBMIT_LABEL, AI_ASSIST_TITLE } from "../lib/aiAssist";
import { BaseModal } from "./BaseModal";
import { Button } from "./Button";
import { DropdownSelect } from "./DropdownMenu";

/** Prompt + provider picker for the field assist. Presentational —
 * the wrapper owns the call. */
export function AiAssistModal({
  open,
  prompt,
  onPromptChange,
  providerId,
  onProviderChange,
  providers,
  onSubmit,
  onCancel,
}: {
  open: boolean;
  prompt: string;
  onPromptChange: (v: string) => void;
  providerId: string;
  onProviderChange: (v: string) => void;
  providers: { id: string; name: string }[];
  onSubmit: () => void;
  onCancel: () => void;
}) {
  const ref = useRef<HTMLTextAreaElement>(null);
  const disabled = !prompt.trim() || !providerId;

  useEffect(() => {
    if (!open) return;
    const el = ref.current;
    el?.focus();
    el?.setSelectionRange(el.value.length, el.value.length);
  }, [open]);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${el.scrollHeight}px`;
  }, [open, prompt]);

  return (
    <BaseModal open={open} onCancel={onCancel}>
      <div className="text-sm font-semibold text-fg">{AI_ASSIST_TITLE}</div>
      <hr className="-mx-4 border-edge" />
      <textarea
        ref={ref}
        value={prompt}
        onChange={(e) => onPromptChange(e.target.value)}
        placeholder={AI_ASSIST_PLACEHOLDER}
        className="w-full min-h-[5rem] bg-transparent text-sm text-fg placeholder:text-fg-subtle resize-none outline-none overflow-hidden"
        onKeyDown={(e) => {
          if (e.key === "Enter" && (e.metaKey || e.ctrlKey) && !disabled) {
            e.preventDefault();
            onSubmit();
          }
        }}
      />
      <div className="flex items-center gap-2 pt-1">
        <Button onClick={onSubmit} disabled={disabled}>
          {AI_ASSIST_SUBMIT_LABEL}
        </Button>
        <Button onClick={onCancel} tone="link">
          Cancel
        </Button>
        <DropdownSelect
          value={providerId}
          onChange={onProviderChange}
          options={providers.map((p) => ({ value: p.id, label: p.name }))}
          placeholder="No provider"
          className="ml-auto"
        />
      </div>
    </BaseModal>
  );
}
