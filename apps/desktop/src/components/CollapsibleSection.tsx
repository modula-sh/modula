import { ChevronDown, ChevronUp } from "lucide-react";
import { useState } from "react";

export function CollapsibleSection({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  const [open, setOpen] = useState(true);
  return (
    <div>
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        aria-expanded={open}
        // Tighter under the label when open so it reads as attached to its content.
        className={`w-full flex items-center justify-between gap-2 p-5 ${open ? "pb-2" : ""} text-[10px] uppercase tracking-wide text-fg-subtle hover:text-fg transition-colors`}
      >
        <span>{label}</span>
        {open ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
      </button>
      {open && <div className="px-5 pb-5 space-y-3">{children}</div>}
    </div>
  );
}
