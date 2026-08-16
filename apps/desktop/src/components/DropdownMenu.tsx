import { ChevronDown } from "lucide-react";
import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";

interface DropdownMenuProps {
  /** Renders the click target. Receives the open state and a toggle callback. */
  trigger: (props: { open: boolean; toggle: () => void }) => React.ReactNode;
  /** Panel content. Receives a `close` callback so option-select handlers can dismiss. */
  children: (props: { close: () => void }) => React.ReactNode;
  /** Tailwind width class for the panel. Defaults to `w-56`. */
  panelClassName?: string;
  /** Floor the panel width at the trigger's width; it still grows to `panelClassName`. */
  matchTriggerWidth?: boolean;
}

// Generic trigger + positioned panel. Position is `fixed` (anchored to the
// trigger's bounding rect) so the panel can escape ancestor `overflow-hidden`.
// Heuristic upper bound for the panel height. MenuOptions caps its list at
// max-h-72 (288px); add a little for padding and the optional footer slot.
const PANEL_MAX_HEIGHT = 320;

export function DropdownMenu({
  trigger,
  children,
  panelClassName = "w-56",
  matchTriggerWidth = false,
}: DropdownMenuProps) {
  const [open, setOpen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState<{
    top: number;
    left: number;
    width: number;
    openUp: boolean;
    openLeft: boolean;
  } | null>(null);

  // Estimate the panel width from its Tailwind width class (e.g. `w-56` → 224px)
  // so we can decide horizontal direction before the panel renders. Tailwind's
  // spacing unit is 0.25rem (4px). Falls back to the `w-56` default.
  const widthMatch = panelClassName.match(/(?:^|\s)w-(\d+)(?:\s|$)/);
  const panelWidth = widthMatch ? Number(widthMatch[1]) * 4 : 224;

  useEffect(() => {
    if (!open) return;
    function onDoc(e: MouseEvent) {
      if (!containerRef.current?.contains(e.target as Node)) setOpen(false);
    }
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") setOpen(false);
    }
    document.addEventListener("mousedown", onDoc);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDoc);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  const updatePosition = useCallback(() => {
    if (!triggerRef.current) return;
    const r = triggerRef.current.getBoundingClientRect();
    const spaceBelow = window.innerHeight - r.bottom;
    const openUp = spaceBelow < PANEL_MAX_HEIGHT && r.top > spaceBelow;
    // Flip horizontally when a left-anchored panel would overflow the right
    // edge — anchor the panel's right side to the trigger's right side instead.
    const effectiveWidth = matchTriggerWidth ? Math.max(panelWidth, r.width) : panelWidth;
    const spaceRight = window.innerWidth - r.left;
    const openLeft = spaceRight < effectiveWidth && r.right > spaceRight;
    setPos({
      top: openUp ? r.top - 4 : r.bottom + 4,
      left: openLeft ? r.right : r.left,
      width: r.width,
      openUp,
      openLeft,
    });
  }, [panelWidth, matchTriggerWidth]);

  // Measure before paint so the panel appears at its final spot (no open-jump),
  // then keep it anchored: fixed coords go stale on scroll/resize, so recompute.
  // Capture-phase scroll catches ancestor scroll containers too.
  useLayoutEffect(() => {
    if (!open) return;
    updatePosition();
    window.addEventListener("scroll", updatePosition, true);
    window.addEventListener("resize", updatePosition);
    return () => {
      window.removeEventListener("scroll", updatePosition, true);
      window.removeEventListener("resize", updatePosition);
    };
  }, [open, updatePosition]);

  return (
    <div className="relative" ref={containerRef}>
      <div ref={triggerRef}>{trigger({ open, toggle: () => setOpen((v) => !v) })}</div>
      {open && pos && (
        <div
          className={`fixed bg-bg border border-border rounded-lg shadow-lg z-50 p-1 ${panelClassName}`}
          style={{
            top: pos.top,
            left: pos.left,
            minWidth: matchTriggerWidth ? pos.width : undefined,
            // Anchor at the trigger edge, then shift the panel by its own size so
            // we don't need to measure it: up by its height when opening up, left
            // by its width when opening left.
            transform:
              `${pos.openLeft ? "translateX(-100%)" : ""} ${pos.openUp ? "translateY(-100%)" : ""}`.trim() ||
              undefined,
          }}
        >
          {children({ close: () => setOpen(false) })}
        </div>
      )}
    </div>
  );
}

export interface MenuOption {
  value: string;
  label: string;
  selected?: boolean;
  disabled?: boolean;
  icon?: React.ReactNode;
}

// Drop-in `<select>` replacement using DropdownMenu. The `pill` variant
// (default) is a borderless, content-sized chip matching the chat input's model
// dropdown; the `field` variant is a bordered form control matching `TextInput`
// (`padded` for the larger row height, width supplied via `className`).
export function DropdownSelect({
  value,
  options,
  onChange,
  disabled,
  placeholder,
  className = "",
  panelClassName = "w-56",
  title,
  filled,
  variant = "pill",
  mono,
  padded = false,
}: {
  value: string;
  options: MenuOption[];
  onChange: (v: string) => void;
  disabled?: boolean;
  placeholder?: string;
  className?: string;
  panelClassName?: string;
  title?: string;
  /** Always show the pill background, instead of only on hover. */
  filled?: boolean;
  variant?: "pill" | "field";
  mono?: boolean;
  padded?: boolean;
}) {
  const current = options.find((o) => o.value === value);
  const label = current?.label ?? placeholder ?? "";
  const triggerClass =
    variant === "field"
      ? `flex items-center justify-between gap-1 bg-surface border border-border rounded text-xs text-fg px-2 ${padded ? "py-1.5" : "py-1"} focus:outline-none focus:border-border-focus disabled:opacity-60${mono ? " font-mono" : ""} ${className}`
      : `flex items-center gap-1 rounded-full text-xs font-inter transition-colors ${filled ? "px-[8.5px] py-[4.5px] bg-surface-2 text-fg" : "px-3 py-1.5 bg-transparent text-fg-muted"} enabled:hover:bg-surface-2 enabled:hover:text-fg focus:outline-none disabled:opacity-50 disabled:cursor-not-allowed ${className}`;
  return (
    <DropdownMenu
      panelClassName={panelClassName}
      matchTriggerWidth={variant === "field"}
      trigger={({ open, toggle }) => (
        <button
          type="button"
          onClick={toggle}
          disabled={disabled}
          title={title}
          className={triggerClass}
        >
          <span className="flex items-center gap-1.5 min-w-0">
            {current?.icon && <span className="shrink-0">{current.icon}</span>}
            <span className="truncate min-w-0">{label}</span>
          </span>
          <ChevronDown
            size={12}
            className={`shrink-0 transition-transform ${open ? "rotate-180" : ""}`}
          />
        </button>
      )}
    >
      {({ close }) => (
        <MenuOptions
          options={options.map((o) => ({ ...o, selected: o.value === value }))}
          onSelect={(v) => {
            onChange(v);
            close();
          }}
        />
      )}
    </DropdownMenu>
  );
}

// Option count above which the filter box is shown.
const SEARCH_THRESHOLD = 8;

// Standard option list for use inside DropdownMenu's panel. Shows a search box
// that filters by label once there are more than SEARCH_THRESHOLD options.
export function MenuOptions({
  options,
  onSelect,
  empty = "no options",
}: {
  options: MenuOption[];
  onSelect: (value: string) => void;
  empty?: string;
}) {
  const [query, setQuery] = useState("");
  const searchable = options.length > SEARCH_THRESHOLD;
  const q = query.trim().toLowerCase();
  const filtered =
    searchable && q ? options.filter((o) => o.label.toLowerCase().includes(q)) : options;
  return (
    <>
      {searchable && (
        <input
          type="text"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Search…"
          autoFocus
          className="w-full bg-transparent border-0 border-b border-border px-2 py-1.5 mb-0.5 text-xs font-inter text-fg placeholder-fg-subtle focus:outline-none"
        />
      )}
      <ul className="max-h-72 overflow-y-auto space-y-0.5">
        {filtered.length === 0 && <li className="px-2 py-2 text-xs text-fg-subtle">{empty}</li>}
        {filtered.map((o) => (
          <li key={o.value}>
            <button
              type="button"
              disabled={o.disabled}
              onClick={() => onSelect(o.value)}
              className={`flex items-center gap-1.5 w-full text-left px-2 py-1.5 rounded text-xs font-inter enabled:hover:bg-surface disabled:opacity-40 disabled:cursor-not-allowed ${
                o.selected ? "text-fg bg-surface" : "text-fg-muted"
              }`}
            >
              {o.icon && <span className="shrink-0">{o.icon}</span>}
              <span className="truncate min-w-0">{o.label}</span>
            </button>
          </li>
        ))}
      </ul>
    </>
  );
}
