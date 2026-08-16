import { Plus } from "lucide-react";
import { useState } from "react";
import { SYSTEM_CONTEXTS } from "../lib/contextArgs";
import { DropdownMenu } from "./DropdownMenu";

const INPUT_CLASS =
  "w-full bg-transparent border-0 border-b border-border px-2 py-1.5 mb-0.5 text-xs font-inter text-fg placeholder-fg-subtle focus:outline-none";
const OPTION_CLASS =
  "block w-full text-left truncate px-2 py-1.5 rounded text-xs font-inter text-fg-muted hover:bg-surface";

// Pick a system arg from the fixed pool to add as context. Controlled: the
// parent owns the args array via `selected` (flags already added) + `onAdd`.
export function ContextPicker({
  selected,
  onAdd,
}: {
  selected: string[];
  onAdd: (flag: string) => void;
}) {
  const [query, setQuery] = useState("");
  const q = query.trim().toLowerCase();
  const filtered = SYSTEM_CONTEXTS.filter(
    (c) => !selected.includes(c.flag) && c.name.toLowerCase().includes(q),
  );

  return (
    <DropdownMenu
      panelClassName="w-48"
      trigger={({ toggle }) => (
        <button
          type="button"
          onClick={toggle}
          className="inline-flex items-center gap-1 px-2 py-0.5 text-[11px] rounded-full border border-dashed border-border text-fg-muted hover:text-fg hover:bg-surface"
        >
          <Plus size={11} /> Add context
        </button>
      )}
    >
      {({ close }) => (
        <>
          <input
            type="text"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Search…"
            autoFocus
            className={INPUT_CLASS}
          />
          <ul className="max-h-72 overflow-y-auto space-y-0.5">
            {filtered.map((c) => (
              <li key={c.flag}>
                <button
                  type="button"
                  onClick={() => {
                    onAdd(c.flag);
                    setQuery("");
                    close();
                  }}
                  className={OPTION_CLASS}
                >
                  {c.name}
                </button>
              </li>
            ))}
            {filtered.length === 0 && (
              <li className="px-2 py-2 text-xs text-fg-subtle">no contexts</li>
            )}
          </ul>
        </>
      )}
    </DropdownMenu>
  );
}
