import { BaseModal } from "./BaseModal";
import { Button } from "./Button";

/** Yes/no confirmation. For text-input prompts, use `PromptModal`. */
export function ConfirmModal({
  open,
  title,
  body,
  confirmLabel = "Confirm",
  busy = false,
  onConfirm,
  onCancel,
}: {
  open: boolean;
  title: string;
  body: React.ReactNode;
  confirmLabel?: string;
  busy?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  return (
    <BaseModal open={open} busy={busy} onCancel={onCancel}>
      <div className="text-sm font-semibold text-fg">{title}</div>
      <div className="text-xs text-fg space-y-1.5">{body}</div>
      <div className="flex items-center gap-2 pt-1">
        <Button onClick={onConfirm} disabled={busy}>
          {busy ? "working…" : confirmLabel}
        </Button>
        <Button onClick={onCancel} disabled={busy} tone="link">
          Cancel
        </Button>
      </div>
    </BaseModal>
  );
}
