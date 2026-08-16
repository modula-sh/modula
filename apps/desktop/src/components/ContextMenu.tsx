import { useEffect } from "react";

export interface ContextMenuItem {
  label: string;
  icon?: React.ReactNode;
  onClick: () => void;
  destructive?: boolean;
}

// Right-click popover. Position is fixed at the cursor (`x`, `y`). Styling
// mirrors DropdownMenu's option list so the two feel consistent.
export function ContextMenu({
  x,
  y,
  items,
  onClose,
}: {
  x: number;
  y: number;
  items: ContextMenuItem[];
  onClose: () => void;
}) {
  useEffect(() => {
    function onDoc() {
      onClose();
    }
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") onClose();
    }
    document.addEventListener("mousedown", onDoc);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDoc);
      document.removeEventListener("keydown", onKey);
    };
  }, [onClose]);

  return (
    <div
      className="fixed bg-bg border border-border rounded-lg shadow-lg z-50 w-44 p-1"
      style={{ top: y, left: x }}
      // Block the doc-level close so clicking inside the menu doesn't dismiss it.
      onMouseDown={(e) => e.stopPropagation()}
    >
      <ul className="max-h-72 overflow-y-auto space-y-0.5">
        {items.map((item, i) => (
          <li key={i}>
            <button
              type="button"
              onClick={() => {
                item.onClick();
                onClose();
              }}
              className={`w-full flex items-center gap-2 text-left px-2 py-1.5 rounded text-xs font-inter hover:bg-surface ${
                item.destructive ? "text-red-500" : "text-fg-muted"
              }`}
            >
              {item.icon && <span className="shrink-0">{item.icon}</span>}
              <span>{item.label}</span>
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}
