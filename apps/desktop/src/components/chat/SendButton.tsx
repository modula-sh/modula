import { ArrowUp, Square } from "lucide-react";

// Circular send button. When `busy` is true it renders a stop square instead.
export function SendButton({
  onClick,
  disabled,
  busy,
}: {
  onClick: () => void;
  disabled?: boolean;
  busy?: boolean;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      title={busy ? "Stop" : "Send"}
      className="w-8 h-8 rounded-full bg-fg text-bg flex items-center justify-center hover:bg-fg-muted disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
    >
      {busy ? <Square size={12} fill="currentColor" /> : <ArrowUp size={16} />}
    </button>
  );
}
