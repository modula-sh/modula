import { useEffect, useRef } from "react";

const MAX_HEIGHT = 400;

// Floating "Codex/ChatGPT-style" input shell: rounded card with a soft shadow,
// an auto-growing textarea, and a bottom row consumers fill with controls.
export function ChatInputShell({
  value,
  onChange,
  onSubmit,
  placeholder,
  autoFocus,
  bottomRow,
}: {
  value: string;
  onChange: (v: string) => void;
  /** Fired on ⌘↵ / ⌃↵. */
  onSubmit?: () => void;
  placeholder?: string;
  autoFocus?: boolean;
  bottomRow: React.ReactNode;
}) {
  const taRef = useRef<HTMLTextAreaElement>(null);

  // Auto-grow: reset to natural height each render, then cap at max.
  useEffect(() => {
    const ta = taRef.current;
    if (!ta) return;
    ta.style.height = "auto";
    ta.style.height = `${Math.min(ta.scrollHeight, MAX_HEIGHT)}px`;
  }, [value]);

  return (
    <div className="flex flex-col gap-1 p-2 bg-chat-input border border-chat-input-border/50 rounded-3xl shadow-panel">
      <textarea
        ref={taRef}
        className="bg-transparent rounded p-2 text-[14px] text-fg placeholder-fg-subtle font-geist focus:outline-none resize-none overflow-y-auto"
        rows={2}
        style={{ maxHeight: MAX_HEIGHT }}
        placeholder={placeholder}
        value={value}
        autoFocus={autoFocus}
        onChange={(e) => onChange(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
            e.preventDefault();
            onSubmit?.();
          }
        }}
      />
      <div className="flex flex-wrap items-center gap-2 px-1">{bottomRow}</div>
    </div>
  );
}
