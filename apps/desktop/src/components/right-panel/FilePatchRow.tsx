import { ChevronDown, ChevronRight } from "lucide-react";
import { useState } from "react";
import { DiffBody } from "../DiffBody";

export interface FilePatch {
  path: string;
  diff: string;
  additions: number;
  deletions: number;
}

export interface FilePatchAction {
  icon: React.ReactNode;
  title: string;
  onClick: () => void;
}

// Collapsible single-file diff row used by every panel that lists patches.
// Stores its open/closed state locally and registers a ref under `refKey` so
// the host panel can scroll to it.
export function FilePatchRow({
  file,
  refKey,
  fileRefs,
  defaultOpen,
  action,
}: {
  file: FilePatch;
  refKey: string;
  fileRefs: React.RefObject<Map<string, HTMLDivElement>>;
  defaultOpen?: boolean;
  action?: FilePatchAction;
}) {
  const [open, setOpen] = useState(defaultOpen ?? false);
  return (
    <div
      ref={(el) => {
        const m = fileRefs.current;
        if (!m) return;
        if (el) m.set(refKey, el);
        else m.delete(refKey);
      }}
    >
      <div className="w-full flex items-center gap-2 px-3 py-1.5 text-[11px] font-mono text-fg-muted">
        <button
          type="button"
          onClick={() => setOpen((v) => !v)}
          className="flex-1 min-w-0 flex items-center gap-2 hover:text-fg text-left"
        >
          {open ? (
            <ChevronDown size={12} className="shrink-0" />
          ) : (
            <ChevronRight size={12} className="shrink-0" />
          )}
          <span className="font-inter truncate flex-1 min-w-0">{file.path}</span>
          <span className="shrink-0 flex gap-2">
            <span className="text-green-500">+{file.additions}</span>
            <span className="text-red-500">−{file.deletions}</span>
          </span>
        </button>
        {action && (
          <button
            type="button"
            onClick={action.onClick}
            title={action.title}
            className="shrink-0 p-1 rounded text-fg-subtle hover:text-fg hover:bg-surface-2 transition-colors"
          >
            {action.icon}
          </button>
        )}
      </div>
      {open && <DiffBody text={file.diff} path={file.path} />}
    </div>
  );
}
