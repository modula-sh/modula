import { BaseModal } from "./BaseModal";
import { Button } from "./Button";
import { TextInput } from "./TextInput";

/** Text-input prompt. Enter submits when the trimmed value is non-empty;
 * Escape and overlay-click cancel. For yes/no confirmation, use `ConfirmModal`. */
export function PromptModal({
  open,
  title,
  value,
  onChange,
  placeholder,
  confirmLabel = "Save",
  busy = false,
  autoFocus = true,
  type = "text",
  error,
  onConfirm,
  onCancel,
}: {
  open: boolean;
  title: string;
  value: string;
  onChange: (v: string) => void;
  placeholder?: string;
  confirmLabel?: string;
  busy?: boolean;
  autoFocus?: boolean;
  type?: "text" | "password";
  /** Shown inline so the modal can stay open on a rejected submit. */
  error?: string | null;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  const disabled = busy || !value.trim();
  return (
    <BaseModal open={open} busy={busy} onCancel={onCancel}>
      <div className="text-sm font-semibold text-fg">{title}</div>
      <TextInput
        autoFocus={autoFocus}
        type={type}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        onKeyDown={(e) => {
          if (e.key === "Enter" && !disabled) {
            e.preventDefault();
            onConfirm();
          }
        }}
        padded
        className="w-full"
      />
      <div className="flex items-center gap-2 pt-1">
        <Button onClick={onConfirm} disabled={disabled}>
          {busy ? "working…" : confirmLabel}
        </Button>
        <Button onClick={onCancel} disabled={busy} tone="link">
          Cancel
        </Button>
      </div>
      {error && <div className="text-[11px] text-red-400">{error}</div>}
    </BaseModal>
  );
}
